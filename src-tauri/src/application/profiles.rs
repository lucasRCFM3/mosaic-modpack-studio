use crate::{
    domain::*,
    error::{AppError, AppResult},
    infrastructure::store::JsonStore,
};
use chrono::Utc;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
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
        mut mods: Vec<InstalledMod>,
        graph: HashMap<String, (InstallReason, Vec<ProjectRef>)>,
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
                for item in &mut mods {
                    if profile.mods.iter().any(|existing| {
                        existing.as_ref() == item.as_ref()
                            && matches!(existing.reason, InstallReason::Requested)
                    }) {
                        item.reason = InstallReason::Requested;
                    }
                }
                let incoming: HashSet<_> = mods.iter().map(|item| item.as_ref().key()).collect();
                profile
                    .mods
                    .retain(|item| !incoming.contains(&item.as_ref().key()));
                profile.mods.extend(mods);
                for item in &mut profile.mods {
                    if let Some((reason, dependencies)) = graph.get(&item.as_ref().key()) {
                        if !matches!(item.reason, InstallReason::Requested)
                            || matches!(reason, InstallReason::Requested)
                        {
                            item.reason = *reason;
                        }
                        item.required_dependencies = Some(dependencies.clone());
                    }
                }
                profile.updated_at = Utc::now().to_rfc3339();
                Ok(profile.clone())
            })
            .await?
    }

    pub async fn remove_mods(
        &self,
        id: &str,
        project_keys: &HashSet<String>,
        expected_updated_at: &str,
        graph: &HashMap<String, Vec<ProjectRef>>,
    ) -> AppResult<ModpackProfile> {
        let profile = self.get(id).await?;
        if profile.updated_at != expected_updated_at {
            return Err(AppError::Message(
                "O modpack mudou durante a verificação. Tente remover novamente.".into(),
            ));
        }
        let root = PathBuf::from(&profile.instance_path).join("mods");
        let trash = root.join(".mosaic-trash").join(Uuid::new_v4().to_string());
        let candidates: AppResult<Vec<_>> = profile
            .mods
            .iter()
            .filter(|item| project_keys.contains(&item.as_ref().key()))
            .map(|item| {
                let registered = Path::new(&item.filename);
                let filename = registered
                    .file_name()
                    .ok_or_else(|| AppError::Message("Nome de arquivo inválido.".into()))?;
                if registered != Path::new(filename) {
                    return Err(AppError::Message(
                        "Um arquivo registrado possui um caminho inseguro.".into(),
                    ));
                }
                Ok((root.join(filename), trash.join(filename)))
            })
            .collect();
        let mut moved = Vec::new();
        for (source, staged) in candidates? {
            if !tokio::fs::try_exists(&source).await? {
                continue;
            }
            if moved.is_empty() {
                tokio::fs::create_dir_all(&trash).await?;
            }
            if let Err(error) = tokio::fs::rename(&source, &staged).await {
                restore_files(&moved).await;
                return Err(error.into());
            }
            moved.push((source, staged));
        }
        let result = self
            .store
            .update(|database| {
                let stored = database
                    .profiles
                    .iter_mut()
                    .find(|profile| profile.id == id)
                    .ok_or_else(|| AppError::Message("Perfil não encontrado.".into()))?;
                if stored.updated_at != expected_updated_at {
                    return Err(AppError::Message(
                        "O modpack mudou durante a verificação. Tente remover novamente.".into(),
                    ));
                }
                stored
                    .mods
                    .retain(|item| !project_keys.contains(&item.as_ref().key()));
                for item in &mut stored.mods {
                    if let Some(dependencies) = graph.get(&item.as_ref().key()) {
                        item.required_dependencies = Some(dependencies.clone());
                    }
                }
                stored.updated_at = Utc::now().to_rfc3339();
                Ok(stored.clone())
            })
            .await
            .and_then(|result| result);
        match result {
            Ok(updated) => {
                if !moved.is_empty() {
                    let _ = tokio::fs::remove_dir_all(&trash).await;
                }
                Ok(updated)
            }
            Err(error) => {
                restore_files(&moved).await;
                Err(error)
            }
        }
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
            generated_by: "Mosaic Modpack Studio 0.4.1",
            generated_at: Utc::now().to_rfc3339(),
            profile: self.get(id).await?,
        };
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(destination, serde_json::to_vec_pretty(&lockfile)?).await?;
        Ok(())
    }

    pub async fn export_mod_list(&self, id: &str, destination: &Path) -> AppResult<()> {
        let profile = self.get(id).await?;
        if let Some(parent) = destination.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(destination, render_mod_list(&profile.mods)).await?;
        Ok(())
    }
}

async fn restore_files(files: &[(PathBuf, PathBuf)]) {
    for (original, staged) in files.iter().rev() {
        let _ = tokio::fs::rename(staged, original).await;
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

fn render_mod_list(mods: &[InstalledMod]) -> String {
    let mut entries: Vec<_> = mods
        .iter()
        .map(|item| {
            let name = item.name.split_whitespace().collect::<Vec<_>>().join(" ");
            format!("{} ({name})", item.filename)
        })
        .collect();
    entries.sort_by_key(|entry| entry.to_lowercase());
    if entries.is_empty() {
        String::new()
    } else {
        format!("{}\r\n", entries.join("\r\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::{render_mod_list, validate_profile_metadata};
    use crate::domain::{InstallReason, InstalledMod, ProviderId};

    #[test]
    fn validates_editable_profile_metadata() {
        assert!(validate_profile_metadata("Meu modpack", "Uma descrição").is_ok());
        assert!(validate_profile_metadata("   ", "").is_err());
        assert!(validate_profile_metadata("Pack", &"a".repeat(301)).is_err());
    }

    #[test]
    fn renders_a_stable_plain_text_mod_list() {
        let mod_entry = |filename: &str, name: &str| InstalledMod {
            provider: ProviderId::Modrinth,
            project_id: filename.into(),
            name: name.into(),
            version_id: "version".into(),
            version_number: "1".into(),
            filename: filename.into(),
            installed_at: String::new(),
            reason: InstallReason::Requested,
            hashes: Vec::new(),
            enabled: true,
            required_dependencies: Some(Vec::new()),
        };
        let mods = vec![
            mod_entry("sodium.jar", "Sodium"),
            mod_entry("JEI12021.23-forge.jar", "Just Enough\nItems"),
        ];

        assert_eq!(
            render_mod_list(&mods),
            "JEI12021.23-forge.jar (Just Enough Items)\r\nsodium.jar (Sodium)\r\n"
        );
    }
}
