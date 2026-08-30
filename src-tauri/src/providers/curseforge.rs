use super::{ModProvider, ProviderModpackContent, ProviderSearchResult};
use crate::{
    domain::*,
    error::{AppError, AppResult},
    infrastructure::secrets::SecretStore,
};
use async_trait::async_trait;
use serde::{Deserialize, Deserializer};
use std::{
    collections::HashSet,
    io::{Cursor, Read},
    sync::Arc,
};

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
    pagination: Option<Pagination>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintEnvelope {
    data: FingerprintData,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintData {
    exact_matches: Vec<FingerprintMatch>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FingerprintMatch {
    file: RawFile,
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
    summary: Option<String>,
    #[serde(default, deserialize_with = "null_to_default")]
    authors: Vec<RawAuthor>,
    logo: Option<RawLogo>,
    links: Option<RawLinks>,
    download_count: Option<u64>,
    date_modified: Option<String>,
    #[serde(default, deserialize_with = "null_to_default")]
    categories: Vec<RawCategory>,
    #[serde(default, deserialize_with = "null_to_default")]
    latest_files_indexes: Vec<RawFileIndex>,
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
    website_url: Option<String>,
}
#[derive(Deserialize)]
struct RawCategory {
    name: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFileIndex {
    game_version: String,
    #[serde(default)]
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
    #[serde(default, deserialize_with = "null_to_default")]
    hashes: Vec<RawHash>,
    file_date: String,
    file_length: u64,
    download_count: Option<u64>,
    download_url: Option<String>,
    #[serde(default, deserialize_with = "null_to_default")]
    game_versions: Vec<String>,
    #[serde(default, deserialize_with = "null_to_default")]
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

#[derive(Deserialize)]
struct CursePackManifest {
    minecraft: CursePackMinecraft,
    #[serde(default)]
    files: Vec<CursePackFile>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CursePackMinecraft {
    version: String,
    #[serde(default)]
    mod_loaders: Vec<CursePackLoader>,
}

#[derive(Deserialize)]
struct CursePackLoader {
    id: String,
    #[serde(default)]
    primary: bool,
}

#[derive(Deserialize)]
struct CursePackFile {
    #[serde(rename = "projectID", alias = "projectId")]
    project_id: u64,
}

impl CurseForgeProvider {
    pub fn new(secrets: Arc<SecretStore>) -> AppResult<Self> {
        Ok(Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .user_agent("mosaic-modpack-studio/0.15.0 (tauri; rust)")
                .build()?,
            secrets,
        })
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> AppResult<T> {
        let key = self.secrets.get_curseforge_key()?.ok_or_else(|| {
            AppError::Message(
                "Configure sua chave da CurseForge em Ajustes para usar este catálogo.".into(),
            )
        })?;
        let response = self
            .client
            .get(format!("{BASE_URL}{path}"))
            .header("x-api-key", key)
            .header("Accept", "application/json")
            .header("Accept-Encoding", "identity")
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
        let body = response.bytes().await?;
        let mut deserializer = serde_json::Deserializer::from_slice(&body);
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            AppError::Message(format!(
                "A CurseForge enviou uma resposta incompatível no campo `{}`: {}",
                error.path(),
                error.inner()
            ))
        })
    }

    async fn post_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        body: serde_json::Value,
    ) -> AppResult<T> {
        let key = self.secrets.get_curseforge_key()?.ok_or_else(|| {
            AppError::Message(
                "Configure sua chave da CurseForge em Ajustes para usar este catálogo.".into(),
            )
        })?;
        let response = self
            .client
            .post(format!("{BASE_URL}{path}"))
            .header("x-api-key", key)
            .header("Accept", "application/json")
            .header("Accept-Encoding", "identity")
            .json(&body)
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
        let body = response.bytes().await?;
        let mut deserializer = serde_json::Deserializer::from_slice(&body);
        serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
            AppError::Message(format!(
                "A CurseForge enviou uma resposta incompatível no campo `{}`: {}",
                error.path(),
                error.inner()
            ))
        })
    }

    fn project_from_raw(&self, value: RawMod) -> ProjectSummary {
        let indexes = value.latest_files_indexes;
        let website_url = value
            .links
            .and_then(|links| links.website_url)
            .unwrap_or_else(|| {
                format!(
                    "https://www.curseforge.com/minecraft/mc-mods/{}",
                    value.slug
                )
            });
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
            summary: value.summary.unwrap_or_default(),
            author: value
                .authors
                .into_iter()
                .map(|author| author.name)
                .collect::<Vec<_>>()
                .join(", "),
            icon_url: value.logo.and_then(|logo| logo.thumbnail_url),
            website_url,
            downloads: value.download_count.unwrap_or_default(),
            updated_at: value.date_modified.unwrap_or_default(),
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

    fn version_from_raw(&self, value: RawFile, loaders: Vec<ModLoader>) -> ProjectVersion {
        ProjectVersion {
            provider: ProviderId::Curseforge,
            project_id: value.mod_id.to_string(),
            version_id: value.id.to_string(),
            name: value.display_name.clone(),
            version_number: value.display_name,
            minecraft_versions: value.game_versions,
            loaders,
            channel: channel_from_id(value.release_type),
            published_at: value.file_date,
            downloads: value.download_count.unwrap_or_default(),
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

    async fn pack_download_url(&self, project_id: &str, file: &RawFile) -> AppResult<String> {
        if let Some(url) = file.download_url.clone() {
            return Ok(url);
        }
        let response: Envelope<String> = self
            .get(
                &format!("mods/{project_id}/files/{}/download-url", file.id),
                &[],
            )
            .await?;
        Ok(response.data)
    }

    async fn download_pack_archive(&self, url: &str) -> AppResult<Vec<u8>> {
        let response = self.client.get(url).send().await?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Message(format!(
                "Não foi possível baixar o manifesto do modpack: HTTP {status}."
            )));
        }
        const MAX_ARCHIVE_SIZE: u64 = 96 * 1024 * 1024;
        if response
            .content_length()
            .is_some_and(|size| size > MAX_ARCHIVE_SIZE)
        {
            return Err(AppError::Message(
                "O arquivo do modpack é grande demais para a prévia (limite de 96 MB).".into(),
            ));
        }
        let bytes = response.bytes().await?;
        if bytes.len() as u64 > MAX_ARCHIVE_SIZE {
            return Err(AppError::Message(
                "O arquivo do modpack excedeu o limite seguro de 96 MB.".into(),
            ));
        }
        Ok(bytes.to_vec())
    }
}

#[async_trait]
impl ModProvider for CurseForgeProvider {
    fn id(&self) -> ProviderId {
        ProviderId::Curseforge
    }
    fn is_enabled(&self) -> bool {
        matches!(self.secrets.get_curseforge_key(), Ok(Some(_)))
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
        let projects: Vec<_> = response
            .data
            .into_iter()
            .map(|item| self.project_from_raw(item))
            .collect();
        Ok(ProviderSearchResult {
            total: response
                .pagination
                .map(|pagination| pagination.total_count)
                .unwrap_or_else(|| projects.len() as u64),
            projects,
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
            .map(|file| self.version_from_raw(file, vec![target.loader])))
    }

    async fn get_version_by_id(&self, _version_id: &str) -> AppResult<ProjectVersion> {
        Err(AppError::Message(
            "A CurseForge exige o ID do projeto junto com o arquivo.".into(),
        ))
    }

    async fn get_version_by_hash(
        &self,
        _hash: Option<&FileHash>,
        fingerprint: Option<u32>,
    ) -> AppResult<Option<ProjectVersion>> {
        let Some(fingerprint) = fingerprint else {
            return Ok(None);
        };
        let response: FingerprintEnvelope = self
            .post_json(
                "fingerprints",
                serde_json::json!({ "fingerprints": [fingerprint] }),
            )
            .await?;
        Ok(response
            .data
            .exact_matches
            .into_iter()
            .next()
            .map(|matched| {
                let mut file = matched.file;
                let loaders = loaders_from_game_versions(&file.game_versions);
                file.game_versions
                    .retain(|value| loader_from_name(value).is_none());
                self.version_from_raw(file, loaders)
            }))
    }

    async fn project_url(&self, project_id: &str) -> AppResult<String> {
        Ok(self.get_project(project_id).await?.website_url)
    }

    async fn search_modpacks(
        &self,
        target: Option<&ProfileTarget>,
        offset: u32,
        limit: u32,
    ) -> AppResult<ProviderSearchResult> {
        let mut query = vec![
            ("gameId", "432".into()),
            ("classId", "4471".into()),
            ("sortField", "6".into()),
            ("sortOrder", "desc".into()),
            ("index", offset.to_string()),
            ("pageSize", limit.min(20).to_string()),
        ];
        if let Some(target) = target {
            query.push(("gameVersion", target.minecraft_version.clone()));
            query.push(("modLoaderType", loader_id(target.loader).to_string()));
        }
        let response: ListEnvelope<RawMod> = self.get("mods/search", &query).await?;
        let projects: Vec<_> = response
            .data
            .into_iter()
            .map(|item| self.project_from_raw(item))
            .collect();
        Ok(ProviderSearchResult {
            total: response
                .pagination
                .map(|pagination| pagination.total_count)
                .unwrap_or(projects.len() as u64),
            projects,
        })
    }

    async fn get_modpack_content(
        &self,
        project_id: &str,
        requested_target: Option<&ProfileTarget>,
    ) -> AppResult<ProviderModpackContent> {
        let mut query = vec![("pageSize", "50".into())];
        if let Some(target) = requested_target {
            query.push(("gameVersion", target.minecraft_version.clone()));
        }
        let response: ListEnvelope<RawFile> = self
            .get(&format!("mods/{project_id}/files"), &query)
            .await?;
        let mut files = response.data;
        files.retain(|file| {
            requested_target.is_none_or(|target| {
                target
                    .release_channels
                    .contains(&channel_from_id(file.release_type))
                    && file.game_versions.contains(&target.minecraft_version)
                    && {
                        let loaders = loaders_from_game_versions(&file.game_versions);
                        loaders.is_empty() || loaders.contains(&target.loader)
                    }
            })
        });
        files.sort_by(|a, b| b.file_date.cmp(&a.file_date));
        let file = files.into_iter().next().ok_or_else(|| {
            AppError::Message("Este modpack não possui uma versão compatível para analisar.".into())
        })?;
        let url = self.pack_download_url(project_id, &file).await?;
        let archive = self.download_pack_archive(&url).await?;
        let (manifest, has_overrides) =
            tokio::task::spawn_blocking(move || parse_curse_pack(&archive))
                .await
                .map_err(|error| AppError::Message(format!("Falha ao ler o modpack: {error}")))??;
        let target = requested_target
            .cloned()
            .unwrap_or_else(|| target_from_curse_pack(&manifest));
        let source_file_count = manifest.files.len();
        let mut seen = HashSet::new();
        let projects = manifest
            .files
            .into_iter()
            .filter_map(|entry| {
                let reference = ProjectRef {
                    provider: ProviderId::Curseforge,
                    project_id: entry.project_id.to_string(),
                };
                seen.insert(reference.key()).then_some(reference)
            })
            .collect::<Vec<_>>();
        let unresolved_file_count = source_file_count.saturating_sub(projects.len());
        Ok(ProviderModpackContent {
            target,
            projects,
            source_file_count,
            unresolved_file_count,
            has_overrides,
            warnings: Vec::new(),
        })
    }
}

fn parse_curse_pack(bytes: &[u8]) -> AppResult<(CursePackManifest, bool)> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| AppError::Message(format!("Arquivo de modpack inválido: {error}")))?;
    let has_overrides = archive.file_names().any(|name| {
        let normalized = name.replace('\\', "/").to_lowercase();
        normalized.starts_with("overrides/") || normalized.starts_with("client-overrides/")
    });
    let mut raw = String::new();
    archive
        .by_name("manifest.json")
        .map_err(|_| AppError::Message("O arquivo não contém manifest.json.".into()))?
        .read_to_string(&mut raw)?;
    Ok((serde_json::from_str(&raw)?, has_overrides))
}

fn target_from_curse_pack(manifest: &CursePackManifest) -> ProfileTarget {
    let loader = manifest
        .minecraft
        .mod_loaders
        .iter()
        .find(|loader| loader.primary)
        .or_else(|| manifest.minecraft.mod_loaders.first())
        .map(|loader| loader.id.to_lowercase())
        .map(|loader| {
            if loader.starts_with("neoforge-") {
                ModLoader::Neoforge
            } else if loader.starts_with("quilt-") {
                ModLoader::Quilt
            } else if loader.starts_with("fabric-") {
                ModLoader::Fabric
            } else {
                ModLoader::Forge
            }
        })
        .unwrap_or(ModLoader::Forge);
    ProfileTarget {
        minecraft_version: manifest.minecraft.version.clone(),
        loader,
        release_channels: vec![ReleaseChannel::Release, ReleaseChannel::Beta],
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
fn loader_from_name(value: &str) -> Option<ModLoader> {
    match value
        .trim()
        .to_lowercase()
        .replace(['-', '_', ' '], "")
        .as_str()
    {
        "forge" => Some(ModLoader::Forge),
        "fabric" => Some(ModLoader::Fabric),
        "quilt" => Some(ModLoader::Quilt),
        "neoforge" => Some(ModLoader::Neoforge),
        _ => None,
    }
}
fn loaders_from_game_versions(values: &[String]) -> Vec<ModLoader> {
    let mut loaders: Vec<_> = values
        .iter()
        .filter_map(|value| loader_from_name(value))
        .collect();
    loaders.sort_by_key(|loader| loader.as_str());
    loaders.dedup();
    loaders
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

fn null_to_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn accepts_nullable_optional_fields_from_search() {
        let body = br#"{
            "data": [{
                "id": 238222,
                "name": "Just Enough Items",
                "slug": "jei",
                "summary": null,
                "authors": null,
                "logo": null,
                "links": null,
                "downloadCount": null,
                "dateModified": null,
                "categories": null,
                "latestFilesIndexes": null
            }],
            "pagination": null
        }"#;

        let parsed: ListEnvelope<RawMod> = serde_json::from_slice(body).unwrap();

        assert_eq!(parsed.data.len(), 1);
        assert!(parsed.data[0].authors.is_empty());
        assert!(parsed.data[0].latest_files_indexes.is_empty());
        assert!(parsed.pagination.is_none());
    }

    #[test]
    fn accepts_file_indexes_without_a_mod_loader() {
        let body = br#"{
            "data": [{
                "id": 238222,
                "name": "Just Enough Items",
                "slug": "jei",
                "latestFilesIndexes": [
                    {"gameVersion": "1.20.1"},
                    {"gameVersion": "1.20.1", "modLoader": 4}
                ]
            }],
            "pagination": {"totalCount": 1}
        }"#;

        let parsed: ListEnvelope<RawMod> = serde_json::from_slice(body).unwrap();

        assert_eq!(parsed.data[0].latest_files_indexes[0].mod_loader, 0);
        assert_eq!(parsed.data[0].latest_files_indexes[1].mod_loader, 4);
    }

    #[test]
    fn extracts_loader_labels_from_fingerprint_results() {
        assert_eq!(
            loaders_from_game_versions(&["1.20.1".into(), "NeoForge".into()]),
            vec![ModLoader::Neoforge]
        );
        assert!(loader_from_name("1.20.1").is_none());
    }

    #[test]
    fn reports_the_json_path_for_an_incompatible_response() {
        let body = br#"{"data":[{"id":"invalid"}],"pagination":null}"#;
        let mut deserializer = serde_json::Deserializer::from_slice(body);
        let error =
            match serde_path_to_error::deserialize::<_, ListEnvelope<RawMod>>(&mut deserializer) {
                Ok(_) => panic!("a resposta inválida deveria falhar"),
                Err(error) => error,
            };

        assert_eq!(error.path().to_string(), "data[0].id");
    }

    #[test]
    fn reads_a_curseforge_manifest_and_its_primary_loader() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("manifest.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer
            .write_all(br#"{"minecraft":{"version":"1.20.1","modLoaders":[{"id":"forge-47.2.0","primary":true}]},"files":[{"projectID":123,"fileID":456,"required":true}]}"#)
            .unwrap();
        writer
            .start_file(
                "overrides/config/example.toml",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        writer.write_all(b"enabled=true").unwrap();
        let bytes = writer.finish().unwrap().into_inner();

        let (manifest, has_overrides) = parse_curse_pack(&bytes).unwrap();
        let target = target_from_curse_pack(&manifest);

        assert!(has_overrides);
        assert_eq!(manifest.files[0].project_id, 123);
        assert_eq!(target.loader, ModLoader::Forge);
        assert_eq!(target.minecraft_version, "1.20.1");
    }
}
