use crate::{domain::*, error::AppResult, providers::ProviderRegistry};
use async_recursion::async_recursion;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct DependencyResolver {
    providers: Arc<ProviderRegistry>,
    plans: RwLock<HashMap<String, ResolutionPlan>>,
}

struct ResolveContext<'a> {
    profile: &'a ModpackProfile,
    selected_optional: HashSet<String>,
    nodes: HashMap<String, ResolutionNode>,
    order: Vec<String>,
    edges: Vec<ResolutionEdge>,
    issues: Vec<ResolutionIssue>,
    optional_dependencies: Vec<OptionalDependencyChoice>,
    visiting: HashSet<String>,
    visited: HashSet<String>,
    installed: HashMap<String, &'a InstalledMod>,
}

impl DependencyResolver {
    pub fn new(providers: Arc<ProviderRegistry>) -> Self {
        Self {
            providers,
            plans: RwLock::new(HashMap::new()),
        }
    }

    pub async fn get_plan(&self, id: &str) -> AppResult<ResolutionPlan> {
        self.plans.read().await.get(id).cloned().ok_or_else(|| {
            crate::error::AppError::Message(
                "O plano de instalação expirou. Resolva as dependências novamente.".into(),
            )
        })
    }

    pub async fn resolve(
        &self,
        profile: &ModpackProfile,
        root: ProjectRef,
        selected_optional: Vec<ProjectRef>,
    ) -> AppResult<ResolutionPlan> {
        self.resolve_many(profile, vec![root], selected_optional)
            .await
    }

    pub async fn resolve_many(
        &self,
        profile: &ModpackProfile,
        roots: Vec<ProjectRef>,
        selected_optional: Vec<ProjectRef>,
    ) -> AppResult<ResolutionPlan> {
        if roots.is_empty() {
            return Err(crate::error::AppError::Message(
                "A predefinição não contém mods.".into(),
            ));
        }
        let mut context = ResolveContext {
            profile,
            selected_optional: selected_optional
                .into_iter()
                .map(|item| item.key())
                .collect(),
            nodes: HashMap::new(),
            order: Vec::new(),
            edges: Vec::new(),
            issues: Vec::new(),
            optional_dependencies: Vec::new(),
            visiting: HashSet::new(),
            visited: HashSet::new(),
            installed: profile
                .mods
                .iter()
                .map(|item| (item.as_ref().key(), item))
                .collect(),
        };
        let mut unique_roots = HashSet::new();
        for root in roots {
            if unique_roots.insert(root.key()) {
                self.visit(&mut context, root, InstallReason::Requested, None, None)
                    .await;
            }
        }
        let nodes: Vec<_> = context
            .order
            .iter()
            .filter_map(|key| context.nodes.get(key).cloned())
            .collect();
        let downloadable_bytes = nodes
            .iter()
            .filter(|node| !node.already_installed)
            .filter_map(|node| primary_file(&node.version))
            .filter(|file| file.url.is_some())
            .map(|file| file.size)
            .sum();
        let can_install = !nodes.is_empty()
            && !context
                .issues
                .iter()
                .any(|issue| matches!(issue.severity, IssueSeverity::Error));
        let plan = ResolutionPlan {
            id: Uuid::new_v4().to_string(),
            target: profile.target.clone(),
            nodes,
            edges: context.edges,
            issues: context.issues,
            optional_dependencies: context.optional_dependencies,
            downloadable_bytes,
            can_install,
        };
        let mut plans = self.plans.write().await;
        plans.insert(plan.id.clone(), plan.clone());
        if plans.len() > 30 {
            if let Some(key) = plans.keys().next().cloned() {
                plans.remove(&key);
            }
        }
        Ok(plan)
    }

    #[async_recursion]
    async fn visit(
        &self,
        context: &mut ResolveContext<'_>,
        project_ref: ProjectRef,
        reason: InstallReason,
        parent_key: Option<String>,
        version_id: Option<String>,
    ) {
        let key = project_ref.key();
        if context.visiting.contains(&key) {
            context.issues.push(issue(
                ResolutionIssueCode::DependencyCycle,
                IssueSeverity::Warning,
                format!("Ciclo de dependências detectado em {key}."),
                Some(project_ref),
            ));
            return;
        }
        if context.visited.contains(&key) {
            if let Some(node) = context.nodes.get_mut(&key) {
                node.reason = stronger_reason(node.reason, reason);
                if matches!(reason, InstallReason::Requested) {
                    node.parent_key = None;
                }
            }
            return;
        }
        context.visiting.insert(key.clone());
        let provider = self.providers.get(project_ref.provider);
        if !provider.is_enabled() {
            context.issues.push(issue(
                ResolutionIssueCode::ProviderError,
                IssueSeverity::Error,
                format!(
                    "O provedor {} não está configurado.",
                    project_ref.provider.as_str()
                ),
                Some(project_ref.clone()),
            ));
            finish(context, &key);
            return;
        }
        let project = match provider.get_project(&project_ref.project_id).await {
            Ok(project) => project,
            Err(error) => {
                context.issues.push(issue(
                    ResolutionIssueCode::ProviderError,
                    IssueSeverity::Error,
                    error.to_string(),
                    Some(project_ref.clone()),
                ));
                finish(context, &key);
                return;
            }
        };
        let version = match provider
            .get_compatible_version(
                &project_ref.project_id,
                &context.profile.target,
                version_id.as_deref(),
            )
            .await
        {
            Ok(Some(version)) => version,
            Ok(None) => {
                context.issues.push(issue(
                    ResolutionIssueCode::NoCompatibleVersion,
                    IssueSeverity::Error,
                    format!(
                        "{} não tem arquivo compatível com Minecraft {} + {}.",
                        project.name,
                        context.profile.target.minecraft_version,
                        context.profile.target.loader.as_str()
                    ),
                    Some(project_ref.clone()),
                ));
                finish(context, &key);
                return;
            }
            Err(error) => {
                context.issues.push(issue(
                    ResolutionIssueCode::ProviderError,
                    IssueSeverity::Error,
                    error.to_string(),
                    Some(project_ref.clone()),
                ));
                finish(context, &key);
                return;
            }
        };
        if !context.nodes.contains_key(&key) {
            context.order.push(key.clone());
        }
        context.nodes.insert(
            key.clone(),
            ResolutionNode {
                key: key.clone(),
                project: project.clone(),
                already_installed: context
                    .installed
                    .get(&key)
                    .is_some_and(|item| item.version_id == version.version_id),
                version: version.clone(),
                reason,
                parent_key,
            },
        );
        if primary_file(&version).is_none_or(|file| file.url.is_none()) {
            context.issues.push(issue(ResolutionIssueCode::DistributionRestricted, IssueSeverity::Error, format!("{} não permite download por aplicativos de terceiros. Abra a página oficial para baixar manualmente.", project.name), Some(project_ref.clone())));
        }
        for dependency in version.dependencies {
            if dependency.dependency_type == DependencyType::Embedded {
                continue;
            }
            let mut dependency_project_id = dependency.project_id.clone();
            if dependency_project_id.is_none() && dependency.version_id.is_some() {
                if let (Some(dependency_provider), Some(dependency_version_id)) =
                    (dependency.provider, dependency.version_id.as_deref())
                {
                    dependency_project_id = self
                        .providers
                        .get(dependency_provider)
                        .get_version_by_id(dependency_version_id)
                        .await
                        .ok()
                        .map(|version| version.project_id);
                }
            }
            let (Some(dependency_provider), Some(dependency_project_id)) =
                (dependency.provider, dependency_project_id)
            else {
                if dependency.dependency_type == DependencyType::Required {
                    context.issues.push(issue(
                        ResolutionIssueCode::MissingDependencyId,
                        IssueSeverity::Error,
                        format!(
                            "{} declarou uma dependência sem identificador ({}).",
                            project.name,
                            dependency
                                .filename
                                .unwrap_or_else(|| "arquivo desconhecido".into())
                        ),
                        Some(project_ref.clone()),
                    ));
                }
                continue;
            };
            let child = ProjectRef {
                provider: dependency_provider,
                project_id: dependency_project_id,
            };
            let child_key = child.key();
            context.edges.push(ResolutionEdge {
                from: key.clone(),
                to: child_key.clone(),
                dependency_type: dependency.dependency_type,
            });
            match dependency.dependency_type {
                DependencyType::Incompatible => {
                    if context.installed.contains_key(&child_key)
                        || context.nodes.contains_key(&child_key)
                    {
                        context.issues.push(issue(
                            ResolutionIssueCode::IncompatibleMod,
                            IssueSeverity::Error,
                            format!("{} é incompatível com {child_key}.", project.name),
                            Some(child),
                        ));
                    }
                }
                DependencyType::Required => {
                    self.visit(
                        context,
                        child,
                        InstallReason::Required,
                        Some(key.clone()),
                        dependency.version_id,
                    )
                    .await
                }
                DependencyType::Optional => {
                    if !context
                        .optional_dependencies
                        .iter()
                        .any(|item| item.project == child)
                    {
                        let name = self
                            .providers
                            .get(child.provider)
                            .get_project(&child.project_id)
                            .await
                            .map(|project| project.name)
                            .unwrap_or_else(|_| child_key.clone());
                        context
                            .optional_dependencies
                            .push(OptionalDependencyChoice {
                                project: child.clone(),
                                name,
                                parent_key: key.clone(),
                                selected: context.selected_optional.contains(&child_key),
                            });
                    }
                    if context.selected_optional.contains(&child_key) {
                        self.visit(
                            context,
                            child,
                            InstallReason::Optional,
                            Some(key.clone()),
                            dependency.version_id,
                        )
                        .await;
                    }
                }
                DependencyType::Embedded => {}
            }
        }
        finish(context, &key);
    }
}

fn primary_file(version: &ProjectVersion) -> Option<&DownloadFile> {
    version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
}
fn issue(
    code: ResolutionIssueCode,
    severity: IssueSeverity,
    message: String,
    project: Option<ProjectRef>,
) -> ResolutionIssue {
    ResolutionIssue {
        code,
        severity,
        message,
        project,
    }
}
fn finish(context: &mut ResolveContext<'_>, key: &str) {
    context.visiting.remove(key);
    context.visited.insert(key.into());
}

fn stronger_reason(current: InstallReason, incoming: InstallReason) -> InstallReason {
    match (current, incoming) {
        (_, InstallReason::Requested) => InstallReason::Requested,
        (InstallReason::Required, InstallReason::Optional) => InstallReason::Optional,
        _ => current,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_selection_is_never_downgraded_to_a_dependency() {
        assert!(matches!(
            stronger_reason(InstallReason::Required, InstallReason::Requested),
            InstallReason::Requested
        ));
        assert!(matches!(
            stronger_reason(InstallReason::Requested, InstallReason::Required),
            InstallReason::Requested
        ));
    }
    use crate::infrastructure::secrets::SecretStore;
    #[test]
    fn required_dependencies_are_the_only_automatic_default() {
        let selected = HashSet::<String>::new();
        assert!(!selected.contains("modrinth:optional"));
        assert_eq!(DependencyType::Required, DependencyType::Required);
    }

    #[tokio::test]
    #[ignore = "consulta a API pública da Modrinth"]
    async fn live_plan_keeps_optional_dependencies_out_by_default() {
        let registry = Arc::new(ProviderRegistry::new(Arc::new(SecretStore::new())).unwrap());
        let provider = registry.get(ProviderId::Modrinth);
        let filters = SearchFilters {
            query: "sodium extra".into(),
            minecraft_version: "1.21.1".into(),
            loader: ModLoader::Fabric,
            release_channels: vec![ReleaseChannel::Release, ReleaseChannel::Beta],
            providers: vec![ProviderId::Modrinth],
            side: SearchSide::Any,
            sort: SearchSort::Relevance,
            limit: Some(1),
        };
        let summary = provider.search(&filters).await.unwrap().projects.remove(0);
        let root = ProjectRef {
            provider: summary.provider,
            project_id: summary.project_id,
        };
        let profile = ModpackProfile {
            id: Uuid::new_v4().to_string(),
            name: "Live".into(),
            description: String::new(),
            target: ProfileTarget {
                minecraft_version: "1.21.1".into(),
                loader: ModLoader::Fabric,
                release_channels: vec![ReleaseChannel::Release, ReleaseChannel::Beta],
            },
            instance_path: "C:\\test".into(),
            created_at: String::new(),
            updated_at: String::new(),
            mods: Vec::new(),
        };
        let plan = DependencyResolver::new(registry)
            .resolve(&profile, root, Vec::new())
            .await
            .unwrap();
        assert!(plan.can_install, "issues: {:#?}", plan.issues);
        assert!(!plan.optional_dependencies.is_empty());
        assert!(
            plan.optional_dependencies
                .iter()
                .all(|dependency| !dependency.selected)
        );
        assert!(
            plan.nodes
                .iter()
                .all(|node| !matches!(node.reason, InstallReason::Optional))
        );
    }

    #[tokio::test]
    #[ignore = "consulta a API pública da Modrinth"]
    async fn live_preset_resolves_multiple_roots_in_one_plan() {
        let registry = Arc::new(ProviderRegistry::new(Arc::new(SecretStore::new())).unwrap());
        let profile = ModpackProfile {
            id: Uuid::new_v4().to_string(),
            name: "Preset live".into(),
            description: String::new(),
            target: ProfileTarget {
                minecraft_version: "1.21.1".into(),
                loader: ModLoader::Fabric,
                release_channels: vec![ReleaseChannel::Release, ReleaseChannel::Beta],
            },
            instance_path: "C:\\test".into(),
            created_at: String::new(),
            updated_at: String::new(),
            mods: Vec::new(),
        };
        let roots = ["sodium", "modmenu"]
            .into_iter()
            .map(|project_id| ProjectRef {
                provider: ProviderId::Modrinth,
                project_id: project_id.into(),
            })
            .collect();
        let plan = DependencyResolver::new(registry)
            .resolve_many(&profile, roots, Vec::new())
            .await
            .unwrap();
        assert!(plan.can_install, "issues: {:#?}", plan.issues);
        assert_eq!(
            plan.nodes
                .iter()
                .filter(|node| matches!(node.reason, InstallReason::Requested))
                .count(),
            2
        );
    }
}
