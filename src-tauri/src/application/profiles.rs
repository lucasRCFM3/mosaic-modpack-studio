use crate::{
    domain::*,
    error::{AppError, AppResult},
    infrastructure::store::JsonStore,
};
use chrono::Utc;
use serde::Serialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;

pub struct ProfileService {
    store: Arc<JsonStore>,
    default_root: PathBuf,
}

impl ProfileService {
    pub fn new(store: Arc<JsonStore>, default_root: PathBuf) -> Self {
        Self {
            store,
            default_root,
        }
    }

    pub async fn ensure_default(&self) -> AppResult<()> {
        if self.list().await.is_empty() {
            self.create(CreateProfileInput {
                name: "Meu primeiro modpack".into(),
                description: Some("Uma base limpa para sua próxima aventura.".into()),
                target: ProfileTarget {
                    minecraft_version: "1.21.1".into(),
                    loader: ModLoader::Fabric,
                    release_channels: vec![ReleaseChannel::Release, ReleaseChannel::Beta],
                },
                instance_path: None,
            })
            .await?;
        }
        Ok(())
    }

    pub async fn list(&self) -> Vec<ModpackProfile> {
        let mut profiles = self.store.snapshot().await.profiles;
        profiles.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        profiles
    }

    pub async fn get(&self, id: &str) -> AppResult<ModpackProfile> {
        self.store
            .snapshot()
            .await
            .profiles
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| AppError::Message("Este perfil não existe mais.".into()))
    }

    pub async fn create(&self, input: CreateProfileInput) -> AppResult<ModpackProfile> {
        let name = input.name.trim();
        validate_profile_metadata(name, input.description.as_deref().unwrap_or_default())?;
        validate_target(&input.target)?;
        let id = Uuid::new_v4().to_string();
        let path = input
            .instance_path
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                self.default_root
                    .join(safe_folder_name(name))
                    .join(&id[..8])
            });
        tokio::fs::create_dir_all(path.join("mods")).await?;
        let now = Utc::now().to_rfc3339();
        let profile = ModpackProfile {
            id,
            name: name.into(),
            description: input.description.unwrap_or_default().trim().to_string(),
            target: input.target,
            instance_path: path.to_string_lossy().into_owned(),
            created_at: now.clone(),
            updated_at: now,
            mods: Vec::new(),
        };
        self.store
            .update(|database| database.profiles.push(profile.clone()))
            .await?;
        Ok(profile)
    }

    pub async fn update(&self, id: &str, input: UpdateProfileInput) -> AppResult<ModpackProfile> {
        let name = input.name.trim();
        let description = input.description.trim();
        validate_profile_metadata(name, description)?;
        self.store
            .update(|database| {
                let profile = database
                    .profiles
                    .iter_mut()
                    .find(|profile| profile.id == id)
                    .ok_or_else(|| AppError::Message("Perfil não encontrado.".into()))?;
                profile.name = name.into();
                profile.description = description.into();
                profile.updated_at = Utc::now().to_rfc3339();
                Ok(profile.clone())
            })
            .await?
    }

    pub async fn remove(&self, id: &str) -> AppResult<()> {
        self.store
            .update(|database| database.profiles.retain(|profile| profile.id != id))
            .await?;
        Ok(())
    }

    pub async fn record_installed(
        &self,
        id: &str,
        mods: Vec<InstalledMod>,
    ) -> AppResult<ModpackProfile> {
        self.store
            .update(|database| {
                let profile = database
                    .profiles
                    .iter_mut()
                    .find(|profile| profile.id == id)
                    .ok_or_else(|| {
                        AppError::Message("Perfil não encontrado durante a instalação.".into())
                    })?;
                let incoming: std::collections::HashSet<_> =
                    mods.iter().map(|item| item.as_ref().key()).collect();
                profile
                    .mods
                    .retain(|item| !incoming.contains(&item.as_ref().key()));
                profile.mods.extend(mods);
                profile.updated_at = Utc::now().to_rfc3339();
                Ok(profile.clone())
            })
            .await?
    }

    pub async fn remove_mod(&self, id: &str, project: &ProjectRef) -> AppResult<ModpackProfile> {
        let profile = self.get(id).await?;
        if let Some(item) = profile.mods.iter().find(|item| item.as_ref() == *project) {
            let root = PathBuf::from(&profile.instance_path).join("mods");
            let filename = Path::new(&item.filename)
                .file_name()
                .ok_or_else(|| AppError::Message("Nome de arquivo inválido.".into()))?;
            let file = root.join(filename);
            match tokio::fs::remove_file(file).await {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        self.store
            .update(|database| {
                let stored = database
                    .profiles
                    .iter_mut()
                    .find(|profile| profile.id == id)
                    .ok_or_else(|| AppError::Message("Perfil não encontrado.".into()))?;
                stored.mods.retain(|item| item.as_ref() != *project);
                stored.updated_at = Utc::now().to_rfc3339();
                Ok(stored.clone())
            })
            .await?
    }

    pub async fn export_lockfile(&self, id: &str, destination: &Path) -> AppResult<()> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Lockfile {
            format_version: u8,
            generated_by: &'static str,
            generated_at: String,
            profile: ModpackProfile,
        }
        let lockfile = Lockfile {
            format_version: 1,
            generated_by: "Mosaic Modpack Studio 0.3.1",
            generated_at: Utc::now().to_rfc3339(),
            profile: self.get(id).await?,
        };
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(destination, serde_json::to_vec_pretty(&lockfile)?).await?;
        Ok(())
    }
}

fn validate_target(target: &ProfileTarget) -> AppResult<()> {
    if target.minecraft_version.trim().is_empty() || target.minecraft_version.len() > 32 {
        return Err(AppError::Message("Versão do Minecraft inválida.".into()));
    }
    if target.release_channels.is_empty() {
        return Err(AppError::Message(
            "Selecione pelo menos um canal de lançamento.".into(),
        ));
    }
    Ok(())
}

fn validate_profile_metadata(name: &str, description: &str) -> AppResult<()> {
    if name.trim().is_empty() || name.chars().count() > 80 {
        return Err(AppError::Message(
            "O nome deve ter entre 1 e 80 caracteres.".into(),
        ));
    }
    if description.chars().count() > 300 {
        return Err(AppError::Message(
            "A descrição deve ter no máximo 300 caracteres.".into(),
        ));
    }
    Ok(())
}
fn safe_folder_name(value: &str) -> String {
    let result: String = value
        .to_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = result.trim_matches('-');
    if trimmed.is_empty() {
        "modpack".into()
    } else {
        trimmed.into()
    }
}

#[cfg(test)]
mod tests {
    use super::validate_profile_metadata;

    #[test]
    fn validates_editable_profile_metadata() {
        assert!(validate_profile_metadata("Meu modpack", "Uma descrição").is_ok());
        assert!(validate_profile_metadata("   ", "").is_err());
        assert!(validate_profile_metadata("Pack", &"a".repeat(301)).is_err());
    }
}
