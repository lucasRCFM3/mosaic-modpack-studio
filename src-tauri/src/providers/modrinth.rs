use super::{ModProvider, ProviderSearchResult};
use crate::{
    domain::*,
    error::{AppError, AppResult},
};
use async_trait::async_trait;
use serde::Deserialize;

const BASE_URL: &str = "https://api.modrinth.com/v2/";

pub struct ModrinthProvider {
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct SearchResponse {
    hits: Vec<SearchHit>,
    total_hits: u64,
}

#[derive(Deserialize)]
struct SearchHit {
    project_id: String,
    slug: String,
    title: String,
    description: String,
    author: String,
    icon_url: Option<String>,
    downloads: u64,
    date_modified: String,
    categories: Vec<String>,
    versions: Vec<String>,
    client_side: String,
    server_side: String,
    featured: Option<bool>,
}

#[derive(Deserialize)]
struct RawProject {
    id: String,
    slug: String,
    title: String,
    description: String,
    team: String,
    icon_url: Option<String>,
    downloads: u64,
    updated: String,
    categories: Vec<String>,
    game_versions: Vec<String>,
    loaders: Vec<String>,
    client_side: String,
    server_side: String,
}

#[derive(Deserialize)]
struct RawVersion {
    id: String,
    project_id: String,
    name: String,
    version_number: String,
    game_versions: Vec<String>,
    version_type: ReleaseChannel,
    loaders: Vec<String>,
    date_published: String,
    downloads: u64,
    files: Vec<RawFile>,
    dependencies: Vec<RawDependency>,
}

#[derive(Deserialize)]
struct RawFile {
    hashes: RawHashes,
    url: String,
    filename: String,
    primary: bool,
    size: u64,
}
#[derive(Deserialize)]
struct RawHashes {
    sha1: Option<String>,
    sha512: Option<String>,
}
#[derive(Deserialize)]
struct RawDependency {
    version_id: Option<String>,
    project_id: Option<String>,
    file_name: Option<String>,
    dependency_type: DependencyType,
}

impl ModrinthProvider {
    pub fn new() -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .user_agent("mosaic-modpack-studio/0.2.1 (tauri; rust)")
            .timeout(std::time::Duration::from_secs(20))
            .build()?;
        Ok(Self { client })
    }

    pub async fn game_versions(&self) -> AppResult<Vec<String>> {
        #[derive(Deserialize)]
        struct GameVersion {
            version: String,
            version_type: String,
            date: String,
        }
        let mut versions: Vec<GameVersion> = self.get("tag/game_version", &[]).await?;
        versions.retain(|item| item.version_type == "release");
        versions.sort_by(|a, b| b.date.cmp(&a.date));
        Ok(versions.into_iter().map(|item| item.version).collect())
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> AppResult<T> {
        let response = self
            .client
            .get(format!("{BASE_URL}{path}"))
            .query(query)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Message(format!(
                "Modrinth respondeu HTTP {status}: {}",
                truncate(&body)
            )));
        }
        Ok(response.json().await?)
    }

    fn project_from_hit(&self, hit: SearchHit) -> ProjectSummary {
        ProjectSummary {
            provider: ProviderId::Modrinth,
            project_id: hit.project_id,
            slug: hit.slug.clone(),
            name: hit.title,
            summary: hit.description,
            author: hit.author,
            icon_url: hit.icon_url,
            website_url: format!("https://modrinth.com/mod/{}", hit.slug),
            downloads: hit.downloads,
            updated_at: hit.date_modified,
            supported_loaders: normalize_loaders(&hit.categories),
            categories: hit.categories,
            supported_versions: hit.versions,
            side: side_of(&hit.client_side, &hit.server_side),
            featured: hit.featured,
        }
    }

    fn project_from_raw(&self, project: RawProject) -> ProjectSummary {
        ProjectSummary {
            provider: ProviderId::Modrinth,
            project_id: project.id,
            slug: project.slug.clone(),
            name: project.title,
            summary: project.description,
            author: project.team,
            icon_url: project.icon_url,
            website_url: format!("https://modrinth.com/mod/{}", project.slug),
            downloads: project.downloads,
            updated_at: project.updated,
            categories: project.categories,
            supported_versions: project.game_versions,
            supported_loaders: normalize_loaders(&project.loaders),
            side: side_of(&project.client_side, &project.server_side),
            featured: None,
        }
    }

    fn version_from_raw(&self, version: RawVersion) -> ProjectVersion {
        ProjectVersion {
            provider: ProviderId::Modrinth,
            project_id: version.project_id,
            version_id: version.id,
            name: version.name,
            version_number: version.version_number,
            minecraft_versions: version.game_versions,
            loaders: normalize_loaders(&version.loaders),
            channel: version.version_type,
            published_at: version.date_published,
            downloads: version.downloads,
            files: version
                .files
                .into_iter()
                .map(|file| DownloadFile {
                    filename: file.filename,
                    url: Some(file.url),
                    size: file.size,
                    primary: file.primary,
                    hashes: [
                        file.hashes.sha512.map(|value| FileHash {
                            algorithm: HashAlgorithm::Sha512,
                            value,
                        }),
                        file.hashes.sha1.map(|value| FileHash {
                            algorithm: HashAlgorithm::Sha1,
                            value,
                        }),
                    ]
                    .into_iter()
                    .flatten()
                    .collect(),
                })
                .collect(),
            dependencies: version
                .dependencies
                .into_iter()
                .map(|dependency| ModDependency {
                    provider: Some(ProviderId::Modrinth),
                    project_id: dependency.project_id,
                    version_id: dependency.version_id,
                    filename: dependency.file_name,
                    dependency_type: dependency.dependency_type,
                })
                .collect(),
        }
    }
}

#[async_trait]
impl ModProvider for ModrinthProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Modrinth
    }
    fn is_enabled(&self) -> bool {
        true
    }

    async fn search(&self, filters: &SearchFilters) -> AppResult<ProviderSearchResult> {
        let facets = serde_json::to_string(&vec![
            vec!["project_type:mod".to_string()],
            vec![format!("versions:{}", filters.minecraft_version)],
            vec![format!("categories:{}", filters.loader.as_str())],
        ])?;
        let index = match filters.sort {
            SearchSort::Relevance => "relevance",
            SearchSort::Downloads => "downloads",
            SearchSort::Updated => "updated",
            SearchSort::Newest => "newest",
        };
        let response: SearchResponse = self
            .get(
                "search",
                &[
                    ("query", filters.query.clone()),
                    ("facets", facets),
                    ("index", index.into()),
                    ("limit", filters.limit.unwrap_or(24).min(100).to_string()),
                ],
            )
            .await?;
        let mut projects: Vec<_> = response
            .hits
            .into_iter()
            .map(|hit| self.project_from_hit(hit))
            .collect();
        if filters.side != SearchSide::Any {
            projects.retain(|project| match filters.side {
                SearchSide::Client => {
                    matches!(project.side, ProjectSide::Client | ProjectSide::Both)
                }
                SearchSide::Server => {
                    matches!(project.side, ProjectSide::Server | ProjectSide::Both)
                }
                SearchSide::Both => project.side == ProjectSide::Both,
                SearchSide::Unknown => project.side == ProjectSide::Unknown,
                SearchSide::Any => true,
            });
        }
        Ok(ProviderSearchResult {
            projects,
            total: response.total_hits,
        })
    }

    async fn get_project(&self, project_id: &str) -> AppResult<ProjectSummary> {
        let project: RawProject = self
            .get(&format!("project/{}", urlencoding(project_id)), &[])
            .await?;
        Ok(self.project_from_raw(project))
    }

    async fn get_compatible_version(
        &self,
        project_id: &str,
        target: &ProfileTarget,
        version_id: Option<&str>,
    ) -> AppResult<Option<ProjectVersion>> {
        let mut versions: Vec<RawVersion> = if let Some(version_id) = version_id {
            vec![
                self.get(&format!("version/{}", urlencoding(version_id)), &[])
                    .await?,
            ]
        } else {
            self.get(
                &format!("project/{}/version", urlencoding(project_id)),
                &[
                    ("loaders", serde_json::to_string(&[target.loader.as_str()])?),
                    (
                        "game_versions",
                        serde_json::to_string(&[&target.minecraft_version])?,
                    ),
                    ("include_changelog", "false".into()),
                ],
            )
            .await?
        };
        versions.retain(|version| {
            version.project_id == project_id
                && version.game_versions.contains(&target.minecraft_version)
                && version
                    .loaders
                    .iter()
                    .any(|loader| loader == target.loader.as_str())
                && target.release_channels.contains(&version.version_type)
        });
        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        Ok(versions
            .into_iter()
            .next()
            .map(|version| self.version_from_raw(version)))
    }

    async fn get_version_by_id(&self, version_id: &str) -> AppResult<ProjectVersion> {
        let version = self
            .get(&format!("version/{}", urlencoding(version_id)), &[])
            .await?;
        Ok(self.version_from_raw(version))
    }

    async fn project_url(&self, project_id: &str) -> AppResult<String> {
        Ok(self.get_project(project_id).await?.website_url)
    }
    async fn game_versions(&self) -> AppResult<Vec<String>> {
        ModrinthProvider::game_versions(self).await
    }
}

fn normalize_loaders(values: &[String]) -> Vec<ModLoader> {
    values
        .iter()
        .filter_map(|value| match value.as_str() {
            "fabric" => Some(ModLoader::Fabric),
            "forge" => Some(ModLoader::Forge),
            "neoforge" => Some(ModLoader::Neoforge),
            "quilt" => Some(ModLoader::Quilt),
            _ => None,
        })
        .collect()
}
fn side_of(client: &str, server: &str) -> ProjectSide {
    match (client != "unsupported", server != "unsupported") {
        (true, true) => ProjectSide::Both,
        (true, false) => ProjectSide::Client,
        (false, true) => ProjectSide::Server,
        _ => ProjectSide::Unknown,
    }
}
fn truncate(value: &str) -> String {
    value.chars().take(180).collect()
}
fn urlencoding(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}
