use super::{ModProvider, ProviderSearchResult};
use crate::{
    domain::*,
    error::{AppError, AppResult},
    infrastructure::secrets::SecretStore,
};
use async_trait::async_trait;
use serde::Deserialize;
use std::sync::Arc;

const BASE_URL: &str = "https://api.curseforge.com/v1/";

pub struct CurseForgeProvider {
    client: reqwest::Client,
    secrets: Arc<SecretStore>,
}

#[derive(Deserialize)]
struct Envelope<T> {
    data: T,
}
#[derive(Deserialize)]
struct ListEnvelope<T> {
    data: Vec<T>,
    pagination: Pagination,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    total_count: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMod {
    id: u64,
    name: String,
    slug: String,
    summary: String,
    authors: Vec<RawAuthor>,
    logo: Option<RawLogo>,
    links: RawLinks,
    download_count: u64,
    date_modified: String,
    categories: Vec<RawCategory>,
    latest_files_indexes: Option<Vec<RawFileIndex>>,
}
#[derive(Deserialize)]
struct RawAuthor {
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLogo {
    thumbnail_url: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLinks {
    website_url: String,
}
#[derive(Deserialize)]
struct RawCategory {
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFileIndex {
    game_version: String,
    mod_loader: u8,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFile {
    id: u64,
    mod_id: u64,
    display_name: String,
    file_name: String,
    release_type: u8,
    hashes: Vec<RawHash>,
    file_date: String,
    file_length: u64,
    download_count: u64,
    download_url: Option<String>,
    game_versions: Vec<String>,
    dependencies: Vec<RawDependency>,
}
#[derive(Deserialize)]
struct RawHash {
    value: String,
    algo: u8,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDependency {
    mod_id: u64,
    relation_type: u8,
}

impl CurseForgeProvider {
    pub fn new(secrets: Arc<SecretStore>) -> AppResult<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()?,
            secrets,
        })
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> AppResult<T> {
        let key = self.secrets.get_curseforge_key().ok_or_else(|| {
            AppError::Message(
                "Configure sua chave da CurseForge em Ajustes para usar este catálogo.".into(),
            )
        })?;
        let response = self
            .client
            .get(format!("{BASE_URL}{path}"))
            .header("x-api-key", key)
            .header("Accept", "application/json")
            .query(query)
            .send()
            .await?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(AppError::Message(format!(
                "CurseForge respondeu HTTP {status}: {}",
                body.chars().take(180).collect::<String>()
            )));
        }
        Ok(response.json().await?)
    }

    fn project_from_raw(&self, value: RawMod) -> ProjectSummary {
        let indexes = value.latest_files_indexes.unwrap_or_default();
        let mut versions: Vec<_> = indexes
            .iter()
            .map(|index| index.game_version.clone())
            .collect();
        versions.sort();
        versions.dedup();
        let mut loaders: Vec<_> = indexes
            .iter()
            .filter_map(|index| loader_from_id(index.mod_loader))
            .collect();
        loaders.sort_by_key(|loader| loader.as_str());
        loaders.dedup();
        ProjectSummary {
            provider: ProviderId::Curseforge,
            project_id: value.id.to_string(),
            slug: value.slug,
            name: value.name,
            summary: value.summary,
            author: value
                .authors
                .into_iter()
                .map(|author| author.name)
                .collect::<Vec<_>>()
                .join(", "),
            icon_url: value.logo.and_then(|logo| logo.thumbnail_url),
            website_url: value.links.website_url,
            downloads: value.download_count,
            updated_at: value.date_modified,
            categories: value
                .categories
                .into_iter()
                .map(|category| category.name)
                .collect(),
            supported_versions: versions,
            supported_loaders: loaders,
            side: ProjectSide::Unknown,
            featured: None,
        }
    }

    fn version_from_raw(&self, value: RawFile, loader: ModLoader) -> ProjectVersion {
        ProjectVersion {
            provider: ProviderId::Curseforge,
            project_id: value.mod_id.to_string(),
            version_id: value.id.to_string(),
            name: value.display_name.clone(),
            version_number: value.display_name,
            minecraft_versions: value.game_versions,
            loaders: vec![loader],
            channel: channel_from_id(value.release_type),
            published_at: value.file_date,
            downloads: value.download_count,
            files: vec![DownloadFile {
                filename: value.file_name,
                url: value.download_url,
                size: value.file_length,
                primary: true,
                hashes: value
                    .hashes
                    .into_iter()
                    .filter_map(|hash| match hash.algo {
                        1 => Some(FileHash {
                            algorithm: HashAlgorithm::Sha1,
                            value: hash.value,
                        }),
                        2 => Some(FileHash {
                            algorithm: HashAlgorithm::Md5,
                            value: hash.value,
                        }),
                        _ => None,
                    })
                    .collect(),
            }],
            dependencies: value
                .dependencies
                .into_iter()
                .filter_map(|dependency| {
                    dependency_from_id(dependency.relation_type).map(|dependency_type| {
                        ModDependency {
                            provider: Some(ProviderId::Curseforge),
                            project_id: Some(dependency.mod_id.to_string()),
                            version_id: None,
                            filename: None,
                            dependency_type,
                        }
                    })
                })
                .collect(),
        }
    }
}

#[async_trait]
impl ModProvider for CurseForgeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Curseforge
    }
    fn is_enabled(&self) -> bool {
        self.secrets.get_curseforge_key().is_some()
    }

    async fn search(&self, filters: &SearchFilters) -> AppResult<ProviderSearchResult> {
        let sort_field = match filters.sort {
            SearchSort::Newest => 11,
            SearchSort::Updated => 3,
            SearchSort::Downloads => 6,
            SearchSort::Relevance => 2,
        };
        let response: ListEnvelope<RawMod> = self
            .get(
                "mods/search",
                &[
                    ("gameId", "432".into()),
                    ("classId", "6".into()),
                    ("gameVersion", filters.minecraft_version.clone()),
                    ("modLoaderType", loader_id(filters.loader).to_string()),
                    ("searchFilter", filters.query.clone()),
                    ("sortField", sort_field.to_string()),
                    ("sortOrder", "desc".into()),
                    ("pageSize", filters.limit.unwrap_or(24).min(50).to_string()),
                ],
            )
            .await?;
        Ok(ProviderSearchResult {
            projects: response
                .data
                .into_iter()
                .map(|item| self.project_from_raw(item))
                .collect(),
            total: response.pagination.total_count,
        })
    }

    async fn get_project(&self, project_id: &str) -> AppResult<ProjectSummary> {
        let response: Envelope<RawMod> = self.get(&format!("mods/{project_id}"), &[]).await?;
        Ok(self.project_from_raw(response.data))
    }

    async fn get_compatible_version(
        &self,
        project_id: &str,
        target: &ProfileTarget,
        version_id: Option<&str>,
    ) -> AppResult<Option<ProjectVersion>> {
        let mut files = if let Some(version_id) = version_id {
            let response: Envelope<RawFile> = self
                .get(&format!("mods/{project_id}/files/{version_id}"), &[])
                .await?;
            vec![response.data]
        } else {
            let response: ListEnvelope<RawFile> = self
                .get(
                    &format!("mods/{project_id}/files"),
                    &[
                        ("gameVersion", target.minecraft_version.clone()),
                        ("modLoaderType", loader_id(target.loader).to_string()),
                        ("pageSize", "50".into()),
                    ],
                )
                .await?;
            response.data
        };
        files.retain(|file| {
            file.game_versions.contains(&target.minecraft_version)
                && target
                    .release_channels
                    .contains(&channel_from_id(file.release_type))
        });
        files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
        Ok(files
            .into_iter()
            .next()
            .map(|file| self.version_from_raw(file, target.loader)))
    }

    async fn get_version_by_id(&self, _version_id: &str) -> AppResult<ProjectVersion> {
        Err(AppError::Message(
            "A CurseForge exige o ID do projeto junto com o arquivo.".into(),
        ))
    }

    async fn project_url(&self, project_id: &str) -> AppResult<String> {
        Ok(self.get_project(project_id).await?.website_url)
    }
}

fn loader_id(loader: ModLoader) -> u8 {
    match loader {
        ModLoader::Forge => 1,
        ModLoader::Fabric => 4,
        ModLoader::Quilt => 5,
        ModLoader::Neoforge => 6,
    }
}
fn loader_from_id(id: u8) -> Option<ModLoader> {
    match id {
        1 => Some(ModLoader::Forge),
        4 => Some(ModLoader::Fabric),
        5 => Some(ModLoader::Quilt),
        6 => Some(ModLoader::Neoforge),
        _ => None,
    }
}
fn channel_from_id(id: u8) -> ReleaseChannel {
    match id {
        2 => ReleaseChannel::Beta,
        3 => ReleaseChannel::Alpha,
        _ => ReleaseChannel::Release,
    }
}
fn dependency_from_id(id: u8) -> Option<DependencyType> {
    match id {
        1 | 6 => Some(DependencyType::Embedded),
        2 => Some(DependencyType::Optional),
        3 => Some(DependencyType::Required),
        5 => Some(DependencyType::Incompatible),
        _ => None,
    }
}
