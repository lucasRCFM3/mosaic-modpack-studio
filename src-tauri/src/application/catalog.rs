use crate::{domain::*, error::AppResult, providers::ProviderRegistry};
use futures_util::future::join_all;
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

pub struct CatalogService {
    providers: Arc<ProviderRegistry>,
}

impl CatalogService {
    pub fn new(providers: Arc<ProviderRegistry>) -> Self {
        Self { providers }
    }

    pub async fn search(&self, filters: SearchFilters) -> AppResult<CatalogSearchResult> {
        validate_filters(&filters)?;
        let requested: HashSet<_> = filters.providers.iter().copied().collect();
        let mut statuses = HashMap::from([
            (
                "modrinth".into(),
                ProviderStatus {
                    enabled: true,
                    ok: false,
                    message: None,
                },
            ),
            (
                "curseforge".into(),
                ProviderStatus {
                    enabled: self.providers.get(ProviderId::Curseforge).is_enabled(),
                    ok: false,
                    message: None,
                },
            ),
        ]);
        let active: Vec<_> = filters
            .providers
            .iter()
            .copied()
            .filter_map(|id| {
                let provider = self.providers.get(id);
                provider.is_enabled().then_some(provider)
            })
            .collect();
        for id in [ProviderId::Modrinth, ProviderId::Curseforge] {
            let key = id.as_str().to_string();
            if !requested.contains(&id) {
                statuses.get_mut(&key).unwrap().message = Some("Desativado no filtro.".into());
            } else if !statuses[&key].enabled {
                statuses.get_mut(&key).unwrap().message =
                    Some("Configure o provedor em Ajustes.".into());
            }
        }
        let tasks = active.iter().map(|provider| {
            let provider = provider.clone();
            let filters = filters.clone();
            async move { (provider.id(), provider.search(&filters).await) }
        });
        let mut projects = Vec::new();
        let mut warnings = Vec::new();
        let mut total = 0;
        for (id, result) in join_all(tasks).await {
            let key = id.as_str().to_string();
            match result {
                Ok(result) => {
                    total += result.total;
                    projects.extend(result.projects);
                    statuses.insert(
                        key,
                        ProviderStatus {
                            enabled: true,
                            ok: true,
                            message: None,
                        },
                    );
                }
                Err(error) => {
                    let provider_name = if id == ProviderId::Modrinth {
                        "Modrinth"
                    } else {
                        "CurseForge"
                    };
                    warnings.push(format!("{provider_name}: {error}"));
                    statuses.insert(
                        key,
                        ProviderStatus {
                            enabled: true,
                            ok: false,
                            message: Some(error.to_string()),
                        },
                    );
                }
            }
        }
        projects.sort_by(|a, b| b.downloads.cmp(&a.downloads));
        Ok(CatalogSearchResult {
            projects,
            total,
            warnings,
            providers: statuses,
        })
    }

    pub async fn game_versions(&self) -> AppResult<Vec<String>> {
        self.providers
            .get(ProviderId::Modrinth)
            .game_versions()
            .await
    }
}

fn validate_filters(filters: &SearchFilters) -> AppResult<()> {
    if filters.query.len() > 200 {
        return Err(crate::error::AppError::Message(
            "A busca deve ter no máximo 200 caracteres.".into(),
        ));
    }
    if filters.minecraft_version.is_empty() || filters.minecraft_version.len() > 32 {
        return Err(crate::error::AppError::Message(
            "Versão do Minecraft inválida.".into(),
        ));
    }
    if filters.providers.is_empty() {
        return Err(crate::error::AppError::Message(
            "Selecione pelo menos um catálogo.".into(),
        ));
    }
    Ok(())
}
