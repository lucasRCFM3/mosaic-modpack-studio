use crate::{
    domain::{ModPreset, PresetEntry, SavePresetInput},
    error::{AppError, AppResult},
    infrastructure::store::JsonStore,
};
use chrono::Utc;
use std::{collections::HashSet, sync::Arc};
use uuid::Uuid;

pub struct PresetService {
    store: Arc<JsonStore>,
}

impl PresetService {
    pub fn new(store: Arc<JsonStore>) -> Self {
        Self { store }
    }

    pub async fn list(&self) -> Vec<ModPreset> {
        let mut presets = self.store.snapshot().await.presets;
        presets.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        presets
    }

    pub async fn get(&self, id: &str) -> AppResult<ModPreset> {
        self.store
            .snapshot()
            .await
            .presets
            .into_iter()
            .find(|preset| preset.id == id)
            .ok_or_else(|| AppError::Message("Esta predefinição não existe mais.".into()))
    }

    pub async fn create(&self, input: SavePresetInput) -> AppResult<ModPreset> {
        let (name, description, entries) = validate_input(input)?;
        let now = Utc::now().to_rfc3339();
        let preset = ModPreset {
            id: Uuid::new_v4().to_string(),
            name,
            description,
            entries,
            created_at: now.clone(),
            updated_at: now,
        };
        self.store
            .update(|database| database.presets.push(preset.clone()))
            .await?;
        Ok(preset)
    }

    pub async fn update(&self, id: &str, input: SavePresetInput) -> AppResult<ModPreset> {
        let (name, description, entries) = validate_input(input)?;
        self.store
            .update(|database| {
                let preset = database
                    .presets
                    .iter_mut()
                    .find(|preset| preset.id == id)
                    .ok_or_else(|| AppError::Message("Predefinição não encontrada.".into()))?;
                preset.name = name;
                preset.description = description;
                preset.entries = entries;
                preset.updated_at = Utc::now().to_rfc3339();
                Ok(preset.clone())
            })
            .await?
    }

    pub async fn remove(&self, id: &str) -> AppResult<()> {
        let exists = self
            .store
            .snapshot()
            .await
            .presets
            .iter()
            .any(|preset| preset.id == id);
        if !exists {
            return Err(AppError::Message("Predefinição não encontrada.".into()));
        }
        self.store
            .update(|database| database.presets.retain(|preset| preset.id != id))
            .await?;
        Ok(())
    }
}

fn validate_input(input: SavePresetInput) -> AppResult<(String, String, Vec<PresetEntry>)> {
    let name = input.name.trim();
    let description = input.description.unwrap_or_default().trim().to_string();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Message(
            "O nome da predefinição deve ter entre 1 e 80 caracteres.".into(),
        ));
    }
    if description.chars().count() > 300 {
        return Err(AppError::Message(
            "A descrição deve ter no máximo 300 caracteres.".into(),
        ));
    }
    if input.entries.is_empty() || input.entries.len() > 200 {
        return Err(AppError::Message(
            "Selecione entre 1 e 200 mods para a predefinição.".into(),
        ));
    }
    let mut seen = HashSet::new();
    let mut entries = Vec::with_capacity(input.entries.len());
    for mut entry in input.entries {
        if entry.project.project_id.trim().is_empty() || entry.project.project_id.len() > 128 {
            return Err(AppError::Message(
                "Uma referência de mod da predefinição é inválida.".into(),
            ));
        }
        entry.name = entry.name.trim().chars().take(120).collect();
        if entry.name.is_empty() {
            entry.name = entry.project.key();
        }
        if seen.insert(entry.project.key()) {
            entries.push(entry);
        }
    }
    Ok((name.into(), description, entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{ProjectRef, ProviderId};
    use crate::infrastructure::store::JsonStore;

    #[test]
    fn deduplicates_entries_without_losing_order() {
        let entry = PresetEntry {
            project: ProjectRef {
                provider: ProviderId::Modrinth,
                project_id: "sodium".into(),
            },
            name: "Sodium".into(),
        };
        let result = validate_input(SavePresetInput {
            name: "Essenciais".into(),
            description: None,
            entries: vec![entry.clone(), entry],
        })
        .unwrap();
        assert_eq!(result.2.len(), 1);
        assert_eq!(result.2[0].name, "Sodium");
    }

    #[tokio::test]
    async fn persists_updates_and_removal() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            JsonStore::load(directory.path().join("state.json"))
                .await
                .unwrap(),
        );
        let service = PresetService::new(store);
        let entry = PresetEntry {
            project: ProjectRef {
                provider: ProviderId::Modrinth,
                project_id: "sodium".into(),
            },
            name: "Sodium".into(),
        };
        let created = service
            .create(SavePresetInput {
                name: "Essenciais".into(),
                description: None,
                entries: vec![entry.clone()],
            })
            .await
            .unwrap();
        assert_eq!(service.list().await.len(), 1);
        let updated = service
            .update(
                &created.id,
                SavePresetInput {
                    name: "Base de desempenho".into(),
                    description: Some("Reutilizável".into()),
                    entries: vec![entry],
                },
            )
            .await
            .unwrap();
        assert_eq!(updated.name, "Base de desempenho");
        service.remove(&created.id).await.unwrap();
        assert!(service.list().await.is_empty());
    }
}
