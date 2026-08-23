use crate::{
    application::{
        dependency_reconciliation::reconcile_required_dependencies, profiles::ProfileService,
    },
    domain::*,
    error::{AppError, AppResult},
    providers::ProviderRegistry,
};
use futures_util::{StreamExt, stream};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};

pub struct DependencyRemovalService {
    profiles: Arc<ProfileService>,
    providers: Arc<ProviderRegistry>,
}

impl DependencyRemovalService {
    pub fn new(profiles: Arc<ProfileService>, providers: Arc<ProviderRegistry>) -> Self {
        Self {
            profiles,
            providers,
        }
    }

    pub async fn remove(
        &self,
        profile_id: &str,
        project: &ProjectRef,
    ) -> AppResult<RemoveModResult> {
        let profile = self.profiles.get(profile_id).await?;
        let target_key = project.key();
        if !profile
            .mods
            .iter()
            .any(|item| item.as_ref().key() == target_key)
        {
            return Err(AppError::Message(
                "Este mod não está mais instalado.".into(),
            ));
        }

        let graph_snapshot = self.load_graph(&profile).await?;
        let unmanaged_mod_files = count_unmanaged_mod_files(&profile).await?;
        let mut decision = plan_removal(&profile.mods, &graph_snapshot.graph, &target_key)?;
        if graph_snapshot.verification_failures > 0 {
            let target_reason = profile
                .mods
                .iter()
                .find(|item| item.as_ref().key() == target_key)
                .map(|item| item.reason);
            if !matches!(target_reason, Some(InstallReason::Requested)) {
                return Err(AppError::Message(
                    "Não foi possível verificar o grafo completo. Por segurança, este mod não foi removido porque pode ser dependência de outro conteúdo.".into(),
                ));
            }
        }
        if unmanaged_mod_files > 0 || graph_snapshot.verification_failures > 0 {
            let dependencies: Vec<_> = decision
                .remove
                .iter()
                .filter(|key| key.as_str() != target_key)
                .cloned()
                .collect();
            for key in dependencies {
                decision.remove.remove(&key);
                decision.retained.insert(key);
            }
        }
        let removed = mods_for_keys(&profile.mods, &decision.remove);
        let retained_shared = mods_for_keys(&profile.mods, &decision.retained);
        let updated = self
            .profiles
            .remove_mods(
                profile_id,
                &decision.remove,
                &profile.updated_at,
                &graph_snapshot.graph,
            )
            .await?;

        Ok(RemoveModResult {
            profile: updated,
            removed,
            retained_shared,
            unmanaged_mod_files,
            dependency_verification_failures: graph_snapshot.verification_failures,
        })
    }

    async fn load_graph(&self, profile: &ModpackProfile) -> AppResult<DependencyGraphSnapshot> {
        let mut graph = HashMap::new();
        for item in &profile.mods {
            if let Some(dependencies) = &item.required_dependencies {
                graph.insert(item.as_ref().key(), dependencies.clone());
            }
        }

        let providers = self.providers.clone();
        let target = profile.target.clone();
        let resolved: Vec<_> = stream::iter(profile.mods.clone())
            .map(|item| {
                let providers = providers.clone();
                let target = target.clone();
                async move {
                    let key = item.as_ref().key();
                    let result = direct_required_dependencies(&providers, &target, &item).await;
                    (item.name, key, result)
                }
            })
            .buffer_unordered(6)
            .collect()
            .await;

        let mut verification_failures = 0;
        for (_name, key, result) in resolved {
            match result {
                Ok(dependencies) => {
                    let stored = graph.entry(key).or_default();
                    for dependency in dependencies {
                        if !stored.contains(&dependency) {
                            stored.push(dependency);
                        }
                    }
                }
                Err(_) => verification_failures += 1,
            }
        }
        Ok(DependencyGraphSnapshot {
            graph,
            verification_failures,
        })
    }
}

struct DependencyGraphSnapshot {
    graph: HashMap<String, Vec<ProjectRef>>,
    verification_failures: usize,
}

async fn count_unmanaged_mod_files(profile: &ModpackProfile) -> AppResult<usize> {
    let registered: HashSet<_> = profile
        .mods
        .iter()
        .map(|item| item.filename.to_lowercase())
        .collect();
    let root = PathBuf::from(&profile.instance_path).join("mods");
    let mut entries = match tokio::fs::read_dir(root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error.into()),
    };
    let mut count = 0;
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_file() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().to_lowercase();
        if filename.ends_with(".jar") && !registered.contains(&filename) {
            count += 1;
        }
    }
    Ok(count)
}

async fn direct_required_dependencies(
    providers: &ProviderRegistry,
    target: &ProfileTarget,
    item: &InstalledMod,
) -> AppResult<Vec<ProjectRef>> {
    let provider = providers.get(item.provider);
    let project = provider.get_project(&item.project_id).await?;
    let mut version = provider
        .get_compatible_version(&item.project_id, target, Some(&item.version_id))
        .await?
        .ok_or_else(|| {
            AppError::Message(format!(
                "a versão instalada {} não está mais disponível",
                item.version_number
            ))
        })?;
    if let Some(reconciled) =
        reconcile_required_dependencies(providers, &project, &version, target).await
    {
        version.dependencies = reconciled.dependencies;
    }
    let mut dependencies = Vec::new();
    for dependency in version
        .dependencies
        .into_iter()
        .filter(|dependency| matches!(dependency.dependency_type, DependencyType::Required))
    {
        let provider = dependency.provider.ok_or_else(|| {
            AppError::Message("uma dependência obrigatória não informou o provedor".into())
        })?;
        let project_id = match dependency.project_id {
            Some(project_id) => project_id,
            None => {
                let version_id = dependency.version_id.ok_or_else(|| {
                    AppError::Message(
                        "uma dependência obrigatória não informou projeto nem versão".into(),
                    )
                })?;
                providers
                    .get(provider)
                    .get_version_by_id(&version_id)
                    .await?
                    .project_id
            }
        };
        let project = ProjectRef {
            provider,
            project_id,
        };
        if !dependencies.contains(&project) {
            dependencies.push(project);
        }
    }
    Ok(dependencies)
}

struct RemovalDecision {
    remove: HashSet<String>,
    retained: HashSet<String>,
}

fn plan_removal(
    mods: &[InstalledMod],
    graph: &HashMap<String, Vec<ProjectRef>>,
    target: &str,
) -> AppResult<RemovalDecision> {
    let installed: HashSet<_> = mods.iter().map(|item| item.as_ref().key()).collect();
    let target_closure = reachable([target.to_string()], graph, &installed);
    let all_other_mods = installed
        .iter()
        .filter(|key| key.as_str() != target)
        .cloned();
    if reachable(all_other_mods, graph, &installed).contains(target) {
        return Err(AppError::Message(
            "Este mod ainda é uma dependência obrigatória de outro conteúdo instalado. Remova primeiro o mod que depende dele.".into(),
        ));
    }

    let reasons: HashMap<_, _> = mods
        .iter()
        .map(|item| (item.as_ref().key(), item.reason))
        .collect();
    let mut remove = HashSet::from([target.to_string()]);
    loop {
        let mut changed = false;
        for key in target_closure.iter().filter(|key| key.as_str() != target) {
            if remove.contains(key) || !matches!(reasons.get(key), Some(InstallReason::Required)) {
                continue;
            }
            let still_referenced = graph.iter().any(|(owner, dependencies)| {
                installed.contains(owner)
                    && !remove.contains(owner)
                    && dependencies
                        .iter()
                        .any(|dependency| dependency.key() == *key)
            });
            if !still_referenced {
                remove.insert(key.clone());
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let retained = target_closure
        .into_iter()
        .filter(|key| key != target && !remove.contains(key))
        .collect();
    Ok(RemovalDecision { remove, retained })
}

fn reachable(
    roots: impl IntoIterator<Item = String>,
    graph: &HashMap<String, Vec<ProjectRef>>,
    installed: &HashSet<String>,
) -> HashSet<String> {
    let mut reached = HashSet::new();
    let mut pending: Vec<_> = roots.into_iter().collect();
    while let Some(key) = pending.pop() {
        if !installed.contains(&key) || !reached.insert(key.clone()) {
            continue;
        }
        if let Some(dependencies) = graph.get(&key) {
            pending.extend(dependencies.iter().map(ProjectRef::key));
        }
    }
    reached
}

fn mods_for_keys(mods: &[InstalledMod], keys: &HashSet<String>) -> Vec<InstalledMod> {
    mods.iter()
        .filter(|item| keys.contains(&item.as_ref().key()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(id: &str) -> ProjectRef {
        ProjectRef {
            provider: ProviderId::Modrinth,
            project_id: id.into(),
        }
    }

    fn installed(id: &str, reason: InstallReason) -> InstalledMod {
        InstalledMod {
            provider: ProviderId::Modrinth,
            project_id: id.into(),
            name: id.into(),
            version_id: "v1".into(),
            version_number: "1".into(),
            filename: format!("{id}.jar"),
            installed_at: String::new(),
            reason,
            hashes: Vec::new(),
            enabled: true,
            required_dependencies: Some(Vec::new()),
        }
    }

    #[test]
    fn removes_the_full_orphaned_required_chain() {
        let mods = vec![
            installed("root", InstallReason::Requested),
            installed("library", InstallReason::Required),
            installed("core", InstallReason::Required),
        ];
        let graph = HashMap::from([
            (project("root").key(), vec![project("library")]),
            (project("library").key(), vec![project("core")]),
        ]);

        let decision = plan_removal(&mods, &graph, &project("root").key()).unwrap();

        assert_eq!(decision.remove.len(), 3);
        assert!(decision.retained.is_empty());
    }

    #[test]
    fn preserves_a_dependency_shared_by_another_root() {
        let mods = vec![
            installed("first", InstallReason::Requested),
            installed("second", InstallReason::Requested),
            installed("library", InstallReason::Required),
        ];
        let graph = HashMap::from([
            (project("first").key(), vec![project("library")]),
            (project("second").key(), vec![project("library")]),
        ]);

        let decision = plan_removal(&mods, &graph, &project("first").key()).unwrap();

        assert_eq!(decision.remove, HashSet::from([project("first").key()]));
        assert_eq!(decision.retained, HashSet::from([project("library").key()]));
    }

    #[test]
    fn preserves_a_dependency_used_by_any_remaining_mod_regardless_of_reason() {
        let mods = vec![
            installed("root", InstallReason::Requested),
            installed("other-component", InstallReason::Required),
            installed("library", InstallReason::Required),
        ];
        let graph = HashMap::from([
            (project("root").key(), vec![project("library")]),
            (project("other-component").key(), vec![project("library")]),
        ]);

        let decision = plan_removal(&mods, &graph, &project("root").key()).unwrap();

        assert_eq!(decision.remove, HashSet::from([project("root").key()]));
        assert_eq!(decision.retained, HashSet::from([project("library").key()]));
    }

    #[test]
    fn refuses_to_remove_a_dependency_that_is_still_used() {
        let mods = vec![
            installed("root", InstallReason::Requested),
            installed("library", InstallReason::Required),
        ];
        let graph = HashMap::from([(project("root").key(), vec![project("library")])]);

        assert!(plan_removal(&mods, &graph, &project("library").key()).is_err());
    }

    #[tokio::test]
    async fn detects_mod_files_that_are_outside_the_lockfile() {
        let directory = tempfile::tempdir().unwrap();
        let mods_root = directory.path().join("mods");
        tokio::fs::create_dir_all(&mods_root).await.unwrap();
        tokio::fs::write(mods_root.join("managed.jar"), b"managed")
            .await
            .unwrap();
        tokio::fs::write(mods_root.join("manual.jar"), b"manual")
            .await
            .unwrap();
        let mut managed = installed("managed", InstallReason::Requested);
        managed.filename = "managed.jar".into();
        let profile = ModpackProfile {
            id: "profile".into(),
            name: "Pack".into(),
            description: String::new(),
            target: ProfileTarget {
                minecraft_version: "1.20.1".into(),
                loader: ModLoader::Fabric,
                release_channels: vec![ReleaseChannel::Release],
            },
            instance_path: directory.path().to_string_lossy().into_owned(),
            created_at: String::new(),
            updated_at: String::new(),
            mods: vec![managed],
        };

        assert_eq!(count_unmanaged_mod_files(&profile).await.unwrap(), 1);
    }
}
