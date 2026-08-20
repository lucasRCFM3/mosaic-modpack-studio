use super::provider_fallback::{find_equivalent_project, projects_equivalent};
use crate::{domain::*, providers::ProviderRegistry};
use std::collections::HashSet;

pub struct ReconciledDependencies {
    pub dependencies: Vec<ModDependency>,
    pub added_projects: Vec<ProjectSummary>,
    pub metadata_provider: ProviderId,
}

pub async fn reconcile_required_dependencies(
    providers: &ProviderRegistry,
    project: &ProjectSummary,
    version: &ProjectVersion,
    target: &ProfileTarget,
) -> Option<ReconciledDependencies> {
    let metadata_provider = ProviderRegistry::alternate_id(project.provider);
    let (_, alternate_version) =
        find_equivalent_project(providers, project, target, metadata_provider, false).await?;
    let alternate_required: Vec<_> = alternate_version
        .dependencies
        .into_iter()
        .filter(|dependency| dependency.dependency_type == DependencyType::Required)
        .collect();
    if alternate_required.is_empty() {
        return None;
    }

    let mut known_projects = Vec::new();
    let mut known_refs: HashSet<_> = version
        .dependencies
        .iter()
        .filter(|dependency| dependency.dependency_type == DependencyType::Required)
        .filter_map(|dependency| Some((dependency.provider?, dependency.project_id.clone()?)))
        .collect();
    for dependency in version
        .dependencies
        .iter()
        .filter(|dependency| dependency.dependency_type == DependencyType::Required)
    {
        if let Some((_, summary)) = dependency_project(providers, dependency).await {
            known_projects.push(summary);
        }
    }

    let mut dependencies = version.dependencies.clone();
    let mut added_projects = Vec::new();
    for dependency in alternate_required {
        let Some((alternate_ref, alternate_summary)) =
            dependency_project(providers, &dependency).await
        else {
            continue;
        };
        if projects_equivalent(project, &alternate_summary)
            || known_projects
                .iter()
                .any(|known| projects_equivalent(known, &alternate_summary))
        {
            continue;
        }

        let preferred = find_equivalent_project(
            providers,
            &alternate_summary,
            target,
            project.provider,
            true,
        )
        .await;
        let (selected_ref, selected_summary, version_id) = match preferred {
            Some((summary, _)) => (
                ProjectRef {
                    provider: summary.provider,
                    project_id: summary.project_id.clone(),
                },
                summary,
                None,
            ),
            None => (alternate_ref, alternate_summary, dependency.version_id),
        };
        if known_refs.contains(&(selected_ref.provider, selected_ref.project_id.clone())) {
            continue;
        }
        if added_projects
            .iter()
            .any(|known| projects_equivalent(known, &selected_summary))
        {
            continue;
        }
        dependencies.push(ModDependency {
            provider: Some(selected_ref.provider),
            project_id: Some(selected_ref.project_id.clone()),
            version_id,
            filename: dependency.filename,
            dependency_type: DependencyType::Required,
        });
        known_projects.push(selected_summary.clone());
        added_projects.push(selected_summary);
        known_refs.insert((selected_ref.provider, selected_ref.project_id));
    }

    (!added_projects.is_empty()).then_some(ReconciledDependencies {
        dependencies,
        added_projects,
        metadata_provider,
    })
}

async fn dependency_project(
    providers: &ProviderRegistry,
    dependency: &ModDependency,
) -> Option<(ProjectRef, ProjectSummary)> {
    let provider_id = dependency.provider?;
    let provider = providers.get(provider_id);
    let project_id = match dependency.project_id.clone() {
        Some(project_id) => project_id,
        None => {
            provider
                .get_version_by_id(dependency.version_id.as_deref()?)
                .await
                .ok()?
                .project_id
        }
    };
    let summary = provider.get_project(&project_id).await.ok()?;
    Some((
        ProjectRef {
            provider: provider_id,
            project_id,
        },
        summary,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::{AppError, AppResult},
        providers::{ModProvider, ProviderSearchResult},
    };
    use async_trait::async_trait;
    use std::{collections::HashMap, sync::Arc};

    struct FakeProvider {
        id: ProviderId,
        projects: HashMap<String, ProjectSummary>,
        versions: HashMap<String, ProjectVersion>,
    }

    #[async_trait]
    impl ModProvider for FakeProvider {
        fn id(&self) -> ProviderId {
            self.id
        }
        fn is_enabled(&self) -> bool {
            true
        }
        async fn search(&self, _filters: &SearchFilters) -> AppResult<ProviderSearchResult> {
            Ok(ProviderSearchResult {
                projects: self.projects.values().cloned().collect(),
                total: self.projects.len() as u64,
            })
        }
        async fn get_project(&self, project_id: &str) -> AppResult<ProjectSummary> {
            self.projects
                .get(project_id)
                .cloned()
                .ok_or_else(|| AppError::Message(format!("Projeto de teste ausente: {project_id}")))
        }
        async fn get_compatible_version(
            &self,
            project_id: &str,
            _target: &ProfileTarget,
            version_id: Option<&str>,
        ) -> AppResult<Option<ProjectVersion>> {
            Ok(self
                .versions
                .get(project_id)
                .cloned()
                .filter(|version| version_id.is_none_or(|expected| version.version_id == expected)))
        }
        async fn get_version_by_id(&self, version_id: &str) -> AppResult<ProjectVersion> {
            self.versions
                .values()
                .find(|version| version.version_id == version_id)
                .cloned()
                .ok_or_else(|| AppError::Message("Versão de teste ausente.".into()))
        }
        async fn project_url(&self, project_id: &str) -> AppResult<String> {
            Ok(self.get_project(project_id).await?.website_url)
        }
    }

    fn project(provider: ProviderId, id: &str, name: &str) -> ProjectSummary {
        ProjectSummary {
            provider,
            project_id: id.into(),
            slug: name.to_ascii_lowercase(),
            name: name.into(),
            summary: String::new(),
            author: "author".into(),
            icon_url: None,
            website_url: format!("https://example.com/{id}"),
            downloads: 1,
            updated_at: String::new(),
            categories: Vec::new(),
            supported_versions: vec!["1.20.1".into()],
            supported_loaders: vec![ModLoader::Forge],
            side: ProjectSide::Both,
            featured: None,
        }
    }

    fn version(
        provider: ProviderId,
        project_id: &str,
        dependencies: Vec<ModDependency>,
    ) -> ProjectVersion {
        ProjectVersion {
            provider,
            project_id: project_id.into(),
            version_id: format!("{project_id}-version"),
            name: "Version".into(),
            version_number: "1.0.0".into(),
            minecraft_versions: vec!["1.20.1".into()],
            loaders: vec![ModLoader::Forge],
            channel: ReleaseChannel::Release,
            published_at: String::new(),
            downloads: 1,
            files: vec![DownloadFile {
                filename: format!("{project_id}.jar"),
                url: Some("https://cdn.example.com/mod.jar".into()),
                size: 1,
                hashes: Vec::new(),
                primary: true,
            }],
            dependencies,
        }
    }

    fn registry(with_curseforge_dependency: bool) -> ProviderRegistry {
        let mr_tectonic = project(ProviderId::Modrinth, "tectonic-mr", "Tectonic");
        let mr_library = project(ProviderId::Modrinth, "library-mr", "Terrain Library");
        let cf_tectonic = project(ProviderId::Curseforge, "tectonic-cf", "Tectonic");
        let cf_library = project(ProviderId::Curseforge, "library-cf", "Terrain Library");
        let required = ModDependency {
            provider: Some(ProviderId::Modrinth),
            project_id: Some(mr_library.project_id.clone()),
            version_id: None,
            filename: None,
            dependency_type: DependencyType::Required,
        };
        let modrinth = FakeProvider {
            id: ProviderId::Modrinth,
            projects: HashMap::from([
                (mr_tectonic.project_id.clone(), mr_tectonic.clone()),
                (mr_library.project_id.clone(), mr_library.clone()),
            ]),
            versions: HashMap::from([
                (
                    mr_tectonic.project_id.clone(),
                    version(
                        ProviderId::Modrinth,
                        &mr_tectonic.project_id,
                        vec![required],
                    ),
                ),
                (
                    mr_library.project_id.clone(),
                    version(ProviderId::Modrinth, &mr_library.project_id, Vec::new()),
                ),
            ]),
        };
        let mut curseforge_projects =
            HashMap::from([(cf_tectonic.project_id.clone(), cf_tectonic.clone())]);
        let mut curseforge_versions = HashMap::from([(
            cf_tectonic.project_id.clone(),
            version(ProviderId::Curseforge, &cf_tectonic.project_id, Vec::new()),
        )]);
        if with_curseforge_dependency {
            curseforge_projects.insert(cf_library.project_id.clone(), cf_library.clone());
            curseforge_versions.insert(
                cf_library.project_id.clone(),
                version(ProviderId::Curseforge, &cf_library.project_id, Vec::new()),
            );
        }
        ProviderRegistry::from_test_providers(
            Arc::new(modrinth),
            Arc::new(FakeProvider {
                id: ProviderId::Curseforge,
                projects: curseforge_projects,
                versions: curseforge_versions,
            }),
        )
    }

    fn target() -> ProfileTarget {
        ProfileTarget {
            minecraft_version: "1.20.1".into(),
            loader: ModLoader::Forge,
            release_channels: vec![ReleaseChannel::Release],
        }
    }

    #[tokio::test]
    async fn fills_missing_metadata_and_prefers_the_original_provider() {
        let registry = registry(true);
        let project = registry
            .get(ProviderId::Curseforge)
            .get_project("tectonic-cf")
            .await
            .unwrap();
        let current = registry
            .get(ProviderId::Curseforge)
            .get_compatible_version("tectonic-cf", &target(), None)
            .await
            .unwrap()
            .unwrap();
        let reconciled = reconcile_required_dependencies(&registry, &project, &current, &target())
            .await
            .unwrap();
        assert_eq!(
            reconciled.dependencies[0].provider,
            Some(ProviderId::Curseforge)
        );
        assert_eq!(
            reconciled.dependencies[0].project_id.as_deref(),
            Some("library-cf")
        );
        assert_eq!(reconciled.metadata_provider, ProviderId::Modrinth);
    }

    #[tokio::test]
    async fn keeps_the_metadata_provider_when_the_original_source_has_no_match() {
        let registry = registry(false);
        let project = registry
            .get(ProviderId::Curseforge)
            .get_project("tectonic-cf")
            .await
            .unwrap();
        let current = registry
            .get(ProviderId::Curseforge)
            .get_compatible_version("tectonic-cf", &target(), None)
            .await
            .unwrap()
            .unwrap();
        let reconciled = reconcile_required_dependencies(&registry, &project, &current, &target())
            .await
            .unwrap();
        assert_eq!(
            reconciled.dependencies[0].provider,
            Some(ProviderId::Modrinth)
        );
        assert_eq!(
            reconciled.dependencies[0].project_id.as_deref(),
            Some("library-mr")
        );
    }
}
