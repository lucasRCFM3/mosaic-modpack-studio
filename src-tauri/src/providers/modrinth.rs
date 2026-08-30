use super::{ModProvider, ProviderModpackContent, ProviderSearchResult};
use crate::{
    domain::*,
    error::{AppError, AppResult},
};
use async_trait::async_trait;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    io::{Cursor, Read},
};

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
    #[serde(default)]
    environment: Option<String>,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MrpackIndex {
    files: Vec<MrpackFile>,
    dependencies: HashMap<String, String>,
}

#[derive(Deserialize)]
struct MrpackFile {
    path: String,
    hashes: MrpackHashes,
    #[serde(default)]
    env: Option<MrpackEnvironment>,
}

#[derive(Deserialize)]
struct MrpackHashes {
    sha1: String,
}

#[derive(Deserialize)]
struct MrpackEnvironment {
    client: String,
}

impl ModrinthProvider {
    pub fn new() -> AppResult<Self> {
        let client = reqwest::Client::builder()
            .user_agent("mosaic-modpack-studio/0.15.0 (tauri; rust)")
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
        self.project_from_hit_with_type(hit, "mod")
    }

    fn project_from_hit_with_type(&self, hit: SearchHit, project_type: &str) -> ProjectSummary {
        ProjectSummary {
            provider: ProviderId::Modrinth,
            project_id: hit.project_id,
            slug: hit.slug.clone(),
            name: hit.title,
            summary: hit.description,
            author: hit.author,
            icon_url: hit.icon_url,
            website_url: format!("https://modrinth.com/{project_type}/{}", hit.slug),
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

    async fn download_archive(&self, url: &str) -> AppResult<Vec<u8>> {
        let response = self.client.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Message(format!(
                "Não foi possível baixar o índice do modpack: HTTP {status}."
            )));
        }
        const MAX_ARCHIVE_SIZE: u64 = 64 * 1024 * 1024;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_ARCHIVE_SIZE)
        {
            return Err(AppError::Message(
                "O arquivo do modpack é grande demais para a prévia (limite de 64 MB).".into(),
            ));
        }
        let bytes = response.bytes().await?;
        if bytes.len() as u64 > MAX_ARCHIVE_SIZE {
            return Err(AppError::Message(
                "O arquivo do modpack excedeu o limite seguro de 64 MB.".into(),
            ));
        }
        Ok(bytes.to_vec())
    }

    async fn versions_by_sha1(&self, hashes: &[String]) -> AppResult<HashMap<String, RawVersion>> {
        let mut versions = HashMap::new();
        for chunk in hashes.chunks(100) {
            let response = self
                .client
                .post(format!("{BASE_URL}version_files"))
                .json(&serde_json::json!({ "hashes": chunk, "algorithm": "sha1" }))
                .send()
                .await?;
            let status = response.status();
            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(AppError::Message(format!(
                    "Modrinth respondeu HTTP {status} ao identificar o conteúdo: {}",
                    truncate(&body)
                )));
            }
            versions.extend(response.json::<HashMap<String, RawVersion>>().await?);
        }
        Ok(versions)
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
            version.game_versions.contains(&target.minecraft_version)
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

    async fn get_version_by_hash(
        &self,
        hash: Option<&FileHash>,
        _fingerprint: Option<u32>,
    ) -> AppResult<Option<ProjectVersion>> {
        let Some(hash) = hash
            .filter(|hash| matches!(hash.algorithm, HashAlgorithm::Sha1 | HashAlgorithm::Sha512))
        else {
            return Ok(None);
        };
        let algorithm = match hash.algorithm {
            HashAlgorithm::Sha1 => "sha1",
            HashAlgorithm::Sha512 => "sha512",
            HashAlgorithm::Md5 => return Ok(None),
        };
        let response = self
            .client
            .get(format!(
                "{BASE_URL}version_file/{}",
                urlencoding(&hash.value)
            ))
            .query(&[("algorithm", algorithm)])
            .send()
            .await?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Message(format!(
                "Modrinth respondeu HTTP {status}: {}",
                truncate(&body)
            )));
        }
        let version: RawVersion = response.json().await?;
        Ok(Some(self.version_from_raw(version)))
    }

    async fn get_version_side(&self, version_id: &str) -> AppResult<ProjectSide> {
        let version: RawVersion = self
            .get(&format!("version/{}", urlencoding(version_id)), &[])
            .await?;
        Ok(version
            .environment
            .as_deref()
            .map(side_from_environment)
            .unwrap_or(ProjectSide::Unknown))
    }

    async fn project_url(&self, project_id: &str) -> AppResult<String> {
        Ok(self.get_project(project_id).await?.website_url)
    }
    async fn game_versions(&self) -> AppResult<Vec<String>> {
        ModrinthProvider::game_versions(self).await
    }

    async fn search_modpacks(
        &self,
        target: Option<&ProfileTarget>,
        offset: u32,
        limit: u32,
    ) -> AppResult<ProviderSearchResult> {
        let mut facets = vec![vec!["project_type:modpack".to_string()]];
        if let Some(target) = target {
            facets.push(vec![format!("versions:{}", target.minecraft_version)]);
            facets.push(vec![format!("categories:{}", target.loader.as_str())]);
        }
        let response: SearchResponse = self
            .get(
                "search",
                &[
                    ("facets", serde_json::to_string(&facets)?),
                    ("index", "downloads".into()),
                    ("offset", offset.to_string()),
                    ("limit", limit.min(20).to_string()),
                ],
            )
            .await?;
        Ok(ProviderSearchResult {
            total: response.total_hits,
            projects: response
                .hits
                .into_iter()
                .map(|hit| self.project_from_hit_with_type(hit, "modpack"))
                .collect(),
        })
    }

    async fn get_modpack_content(
        &self,
        project_id: &str,
        requested_target: Option<&ProfileTarget>,
    ) -> AppResult<ProviderModpackContent> {
        let mut versions: Vec<RawVersion> = self
            .get(
                &format!("project/{}/version", urlencoding(project_id)),
                &[("include_changelog", "false".into())],
            )
            .await?;
        versions.retain(|version| {
            requested_target.is_none_or(|target| {
                version.game_versions.contains(&target.minecraft_version)
                    && version
                        .loaders
                        .iter()
                        .any(|loader| loader == target.loader.as_str())
                    && target.release_channels.contains(&version.version_type)
            })
        });
        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        let version = versions.into_iter().next().ok_or_else(|| {
            AppError::Message("Este modpack não possui uma versão compatível para analisar.".into())
        })?;
        let archive_file = version
            .files
            .iter()
            .find(|file| file.primary && file.filename.ends_with(".mrpack"))
            .or_else(|| {
                version
                    .files
                    .iter()
                    .find(|file| file.filename.ends_with(".mrpack"))
            })
            .ok_or_else(|| AppError::Message("A versão não contém um arquivo .mrpack.".into()))?;
        let archive = self.download_archive(&archive_file.url).await?;
        let (index, has_overrides) = tokio::task::spawn_blocking(move || parse_mrpack(&archive))
            .await
            .map_err(|error| AppError::Message(format!("Falha ao ler o modpack: {error}")))??;
        let target = requested_target
            .cloned()
            .unwrap_or_else(|| target_from_mrpack(&index));
        let mut optional_files = 0;
        let hashes: Vec<_> = index
            .files
            .iter()
            .filter(|file| file.path.replace('\\', "/").starts_with("mods/"))
            .filter(|file| {
                let optional = file
                    .env
                    .as_ref()
                    .is_some_and(|env| env.client == "optional");
                optional_files += usize::from(optional);
                !file
                    .env
                    .as_ref()
                    .is_some_and(|env| env.client == "unsupported")
            })
            .map(|file| file.hashes.sha1.clone())
            .collect();
        let source_file_count = hashes.len();
        let versions = self.versions_by_sha1(&hashes).await?;
        let mut seen = HashSet::new();
        let projects = versions
            .into_values()
            .filter_map(|version| {
                let reference = ProjectRef {
                    provider: ProviderId::Modrinth,
                    project_id: version.project_id,
                };
                seen.insert(reference.key()).then_some(reference)
            })
            .collect::<Vec<_>>();
        let unresolved_file_count = source_file_count.saturating_sub(projects.len());
        let mut warnings = Vec::new();
        if optional_files > 0 {
            warnings.push(format!(
                "O pack possui {optional_files} arquivo(s) opcional(is); eles aparecem na seleção para você decidir durante a resolução."
            ));
        }
        if unresolved_file_count > 0 {
            warnings.push(format!(
                "{unresolved_file_count} arquivo(s) do índice não puderam ser associados a um projeto público da Modrinth."
            ));
        }
        Ok(ProviderModpackContent {
            target,
            projects,
            source_file_count,
            unresolved_file_count,
            has_overrides,
            warnings,
        })
    }
}

fn parse_mrpack(bytes: &[u8]) -> AppResult<(MrpackIndex, bool)> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| AppError::Message(format!("Arquivo .mrpack inválido: {error}")))?;
    let has_overrides = archive.file_names().any(|name| {
        let normalized = name.replace('\\', "/");
        normalized.starts_with("overrides/") || normalized.starts_with("client-overrides/")
    });
    let mut raw = String::new();
    archive
        .by_name("modrinth.index.json")
        .map_err(|_| AppError::Message("O .mrpack não contém modrinth.index.json.".into()))?
        .read_to_string(&mut raw)?;
    Ok((serde_json::from_str(&raw)?, has_overrides))
}

fn target_from_mrpack(index: &MrpackIndex) -> ProfileTarget {
    let loader = if index.dependencies.contains_key("neoforge") {
        ModLoader::Neoforge
    } else if index.dependencies.contains_key("forge") {
        ModLoader::Forge
    } else if index.dependencies.contains_key("quilt-loader") {
        ModLoader::Quilt
    } else {
        ModLoader::Fabric
    };
    ProfileTarget {
        minecraft_version: index
            .dependencies
            .get("minecraft")
            .cloned()
            .unwrap_or_else(|| "1.20.1".into()),
        loader,
        release_channels: vec![ReleaseChannel::Release, ReleaseChannel::Beta],
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
fn side_from_environment(environment: &str) -> ProjectSide {
    match environment {
        "client_only" | "client_only_server_optional" | "singleplayer_only" => ProjectSide::Client,
        "server_only" | "server_only_client_optional" | "dedicated_server_only" => {
            ProjectSide::Server
        }
        "client_and_server" | "client_or_server" | "client_or_server_prefers_both" => {
            ProjectSide::Both
        }
        _ => ProjectSide::Unknown,
    }
}
fn truncate(value: &str) -> String {
    value.chars().take(180).collect()
}
fn urlencoding(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn maps_modrinths_version_environments_to_export_categories() {
        assert_eq!(
            side_from_environment("client_only_server_optional"),
            ProjectSide::Client
        );
        assert_eq!(
            side_from_environment("dedicated_server_only"),
            ProjectSide::Server
        );
        assert_eq!(
            side_from_environment("client_and_server"),
            ProjectSide::Both
        );
        assert_eq!(side_from_environment("unknown"), ProjectSide::Unknown);
    }

    #[test]
    fn reads_the_official_mrpack_index_and_detects_overrides() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file(
                "modrinth.index.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer
            .write_all(br#"{"files":[{"path":"mods/example.jar","hashes":{"sha1":"abc"}}],"dependencies":{"minecraft":"1.21.1","fabric-loader":"0.16"}}"#)
            .unwrap();
        writer
            .start_file(
                "overrides/config/example.json",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"{}").unwrap();
        let bytes = writer.finish().unwrap().into_inner();

        let (index, has_overrides) = parse_mrpack(&bytes).unwrap();

        assert!(has_overrides);
        assert_eq!(index.files.len(), 1);
        assert_eq!(target_from_mrpack(&index).loader, ModLoader::Fabric);
        assert_eq!(target_from_mrpack(&index).minecraft_version, "1.21.1");
    }
}
