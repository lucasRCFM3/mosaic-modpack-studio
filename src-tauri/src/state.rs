use crate::{
    application::{
        catalog::CatalogService, download::DownloadManager, presets::PresetService,
        profiles::ProfileService, resolver::DependencyResolver,
    },
    error::AppResult,
    infrastructure::{secrets::SecretStore, store::JsonStore},
    providers::ProviderRegistry,
};
use std::sync::Arc;
use tauri::{AppHandle, Manager};

pub struct AppState {
    pub store: Arc<JsonStore>,
    pub secrets: Arc<SecretStore>,
    pub catalog: Arc<CatalogService>,
    pub profiles: Arc<ProfileService>,
    pub presets: Arc<PresetService>,
    pub resolver: Arc<DependencyResolver>,
    pub downloads: Arc<DownloadManager>,
    pub providers: Arc<ProviderRegistry>,
}

impl AppState {
    pub async fn initialize(app: &AppHandle) -> AppResult<Self> {
        let app_data = app.path().app_data_dir().map_err(|error| {
            crate::error::AppError::Message(format!("Pasta de dados indisponível: {error}"))
        })?;
        let documents = app.path().document_dir().map_err(|error| {
            crate::error::AppError::Message(format!("Pasta de documentos indisponível: {error}"))
        })?;
        let store = Arc::new(JsonStore::load(app_data.join("mosaic-state.json")).await?);
        let secrets = Arc::new(SecretStore::new());
        let providers = Arc::new(ProviderRegistry::new(secrets.clone())?);
        let profiles = Arc::new(ProfileService::new(store.clone(), documents.join("Mosaic")));
        profiles.ensure_default().await?;
        let presets = Arc::new(PresetService::new(store.clone()));
        let catalog = Arc::new(CatalogService::new(providers.clone()));
        let resolver = Arc::new(DependencyResolver::new(providers.clone()));
        let downloads = Arc::new(DownloadManager::new(profiles.clone())?);
        Ok(Self {
            store,
            secrets,
            catalog,
            profiles,
            presets,
            resolver,
            downloads,
            providers,
        })
    }
}
