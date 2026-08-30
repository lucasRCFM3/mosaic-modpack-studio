mod curseforge;
mod modrinth;

use crate::{domain::*, error::AppResult, infrastructure::secrets::SecretStore};
use async_trait::async_trait;
use std::sync::Arc;

pub use curseforge::CurseForgeProvider;
pub use modrinth::ModrinthProvider;

#[derive(Debug)]
pub struct ProviderSearchResult {
    pub projects: Vec<ProjectSummary>,
    pub total: u64,
}

#[derive(Debug)]
pub struct ProviderModpackContent {
    pub target: ProfileTarget,
    pub projects: Vec<ProjectRef>,
    pub source_file_count: usize,
    pub unresolved_file_count: usize,
    pub has_overrides: bool,
    pub warnings: Vec<String>,
}

#[async_trait]
pub trait ModProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn is_enabled(&self) -> bool;
    async fn search(&self, filters: &SearchFilters) -> AppResult<ProviderSearchResult>;
    async fn get_project(&self, project_id: &str) -> AppResult<ProjectSummary>;
    async fn get_compatible_version(
        &self,
        project_id: &str,
        target: &ProfileTarget,
        version_id: Option<&str>,
    ) -> AppResult<Option<ProjectVersion>>;
    async fn get_version_by_id(&self, version_id: &str) -> AppResult<ProjectVersion>;
    async fn get_version_by_hash(
        &self,
        _hash: Option<&FileHash>,
        _fingerprint: Option<u32>,
    ) -> AppResult<Option<ProjectVersion>> {
        Ok(None)
    }
    async fn get_version_side(&self, _version_id: &str) -> AppResult<ProjectSide> {
        Ok(ProjectSide::Unknown)
    }
    async fn project_url(&self, project_id: &str) -> AppResult<String>;
    async fn game_versions(&self) -> AppResult<Vec<String>> {
        Err(crate::error::AppError::Message(
            "Este provedor não publica a lista de versões.".into(),
        ))
    }
    async fn search_modpacks(
        &self,
        _target: Option<&ProfileTarget>,
        _offset: u32,
        _limit: u32,
    ) -> AppResult<ProviderSearchResult> {
        Err(crate::error::AppError::Message(
            "Este provedor não oferece descoberta de modpacks.".into(),
        ))
    }
    async fn get_modpack_content(
        &self,
        _project_id: &str,
        _target: Option<&ProfileTarget>,
    ) -> AppResult<ProviderModpackContent> {
        Err(crate::error::AppError::Message(
            "Este provedor não oferece leitura de modpacks.".into(),
        ))
    }
}

pub struct ProviderRegistry {
    modrinth: Arc<dyn ModProvider>,
    curseforge: Arc<dyn ModProvider>,
}

impl ProviderRegistry {
    pub fn new(secrets: Arc<SecretStore>) -> AppResult<Self> {
        Ok(Self {
            modrinth: Arc::new(ModrinthProvider::new()?),
            curseforge: Arc::new(CurseForgeProvider::new(secrets)?),
        })
    }

    pub fn get(&self, id: ProviderId) -> Arc<dyn ModProvider> {
        match id {
            ProviderId::Modrinth => self.modrinth.clone(),
            ProviderId::Curseforge => self.curseforge.clone(),
            ProviderId::Local => {
                panic!("mods locais não possuem provedor remoto")
            }
        }
    }

    pub fn alternate_id(id: ProviderId) -> ProviderId {
        match id {
            ProviderId::Modrinth => ProviderId::Curseforge,
            ProviderId::Curseforge | ProviderId::Local => ProviderId::Modrinth,
        }
    }

    #[cfg(test)]
    pub fn from_test_providers(
        modrinth: Arc<dyn ModProvider>,
        curseforge: Arc<dyn ModProvider>,
    ) -> Self {
        Self {
            modrinth,
            curseforge,
        }
    }
}
