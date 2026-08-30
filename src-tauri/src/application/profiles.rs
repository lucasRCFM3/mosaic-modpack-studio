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
    file_operations: tokio::sync::Mutex<()>,
}

impl ProfileService {
    pub fn new(store: Arc<JsonStore>, default_root: PathBuf) -> Self {
        Self {
            store,
            default_root,
            file_operations: tokio::sync::Mutex::new(()),
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

    pub async fn duplicate(
        &self,
        source_id: &str,
        input: DuplicateProfileInput,
    ) -> AppResult<DuplicateProfileResult> {
        let _operation = self.file_operations.lock().await;
        let source_profile = self.get(source_id).await?;
        let name = input.name.trim();
        let description = input.description.unwrap_or_default().trim().to_string();
        validate_profile_metadata(name, &description)?;

        let id = Uuid::new_v4().to_string();
        let requested_destination = input
            .instance_path
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                self.default_root
                    .join(safe_folder_name(name))
                    .join(&id[..8])
            });
        let source = tokio::fs::canonicalize(&source_profile.instance_path)
            .await
            .map_err(|error| {
                AppError::Message(format!(
                    "A pasta da instância original não pôde ser acessada: {error}"
                ))
            })?;
        let (destination, destination_existed) =
            prepare_duplicate_destination(&requested_destination).await?;
        validate_distinct_instance_paths(&source, &destination)?;
        validate_destination_is_unused(&self.store, source_id, &destination).await?;
        validate_registered_mod_files(&source_profile, &source).await?;

        let parent = destination.parent().ok_or_else(|| {
            AppError::Message("A pasta de destino não possui um diretório pai válido.".into())
        })?;
        let staging = parent.join(format!(".mosaic-duplicate-{id}.part"));
        let copy_result = match input.mode {
            DuplicateProfileMode::Full => copy_directory_tree(&source, &staging).await,
            DuplicateProfileMode::ModsOnly => {
                copy_registered_mods(&source_profile, &source, &staging).await
            }
        };
        let stats = match copy_result {
            Ok(stats) => stats,
            Err(error) => {
                let _ = tokio::fs::remove_dir_all(&staging).await;
                return Err(error);
            }
        };

        let current_source = match self.get(source_id).await {
            Ok(profile) => profile,
            Err(error) => {
                let _ = tokio::fs::remove_dir_all(&staging).await;
                return Err(error);
            }
        };
        if current_source.updated_at != source_profile.updated_at {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(AppError::Message(
                "O modpack original mudou durante a cópia. A operação foi cancelada; tente novamente."
                    .into(),
            ));
        }

        if let Err(error) =
            finalize_duplicate_directory(&staging, &destination, destination_existed).await
        {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(error);
        }

        let now = Utc::now().to_rfc3339();
        let profile = ModpackProfile {
            id,
            name: name.into(),
            description,
            target: source_profile.target,
            instance_path: destination.to_string_lossy().into_owned(),
            created_at: now.clone(),
            updated_at: now,
            mods: source_profile.mods,
        };
        if let Err(error) = self
            .store
            .update(|database| database.profiles.push(profile.clone()))
            .await
        {
            let _ = tokio::fs::remove_dir_all(&destination).await;
            if destination_existed {
                let _ = tokio::fs::create_dir_all(&destination).await;
            }
            return Err(error);
        }
        Ok(DuplicateProfileResult {
            profile,
            copied_files: stats.files,
            copied_bytes: stats.bytes,
        })
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

    pub async fn replace_scanned_instance(
        &self,
        id: &str,
        instance_path: &Path,
        target: ProfileTarget,
        mods: Vec<InstalledMod>,
        expected_updated_at: &str,
    ) -> AppResult<ModpackProfile> {
        let _operation = self.file_operations.lock().await;
        let instance_path = tokio::fs::canonicalize(instance_path)
            .await
            .map_err(|error| {
                AppError::Message(format!("A pasta escolhida não pôde ser acessada: {error}"))
            })?;
        let mods_path = instance_path.join("mods");
        if !tokio::fs::metadata(&mods_path).await?.is_dir() {
            return Err(AppError::Message(
                "A pasta escolhida precisa conter uma pasta mods válida.".into(),
            ));
        }
        validate_destination_is_unused(&self.store, id, &instance_path).await?;
        validate_target(&target)?;
        self.store
            .update(|database| {
                let profile = database
                    .profiles
                    .iter_mut()
                    .find(|profile| profile.id == id)
                    .ok_or_else(|| AppError::Message("Perfil não encontrado.".into()))?;
                if profile.updated_at != expected_updated_at {
                    return Err(AppError::Message(
                        "O modpack mudou enquanto a pasta era analisada. Faça uma nova análise antes de substituir.".into(),
                    ));
                }
                profile.instance_path = instance_path.to_string_lossy().into_owned();
                profile.target = target;
                profile.mods = mods;
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
            generated_by: "Mosaic Modpack Studio 0.14.0",
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

#[derive(Default)]
struct CopyStats {
    files: u64,
    bytes: u64,
}

async fn prepare_duplicate_destination(path: &Path) -> AppResult<(PathBuf, bool)> {
    let absolute = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    if tokio::fs::try_exists(&absolute).await? {
        if !tokio::fs::metadata(&absolute).await?.is_dir() {
            return Err(AppError::Message(
                "O destino escolhido existe, mas não é uma pasta.".into(),
            ));
        }
        if !directory_is_empty(&absolute).await? {
            return Err(AppError::Message(
                "Escolha uma pasta vazia para não sobrescrever arquivos existentes.".into(),
            ));
        }
        return Ok((tokio::fs::canonicalize(absolute).await?, true));
    }

    let parent = absolute.parent().ok_or_else(|| {
        AppError::Message("A pasta de destino não possui um diretório pai válido.".into())
    })?;
    tokio::fs::create_dir_all(parent).await?;
    let canonical_parent = tokio::fs::canonicalize(parent).await?;
    let name = absolute
        .file_name()
        .ok_or_else(|| AppError::Message("Escolha uma pasta de destino válida.".into()))?;
    Ok((canonical_parent.join(name), false))
}

async fn directory_is_empty(path: &Path) -> AppResult<bool> {
    Ok(tokio::fs::read_dir(path)
        .await?
        .next_entry()
        .await?
        .is_none())
}

fn validate_distinct_instance_paths(source: &Path, destination: &Path) -> AppResult<()> {
    if source == destination || source.starts_with(destination) || destination.starts_with(source) {
        return Err(AppError::Message(
            "A cópia precisa ficar fora da pasta original e não pode conter a instância original."
                .into(),
        ));
    }
    Ok(())
}

async fn validate_destination_is_unused(
    store: &JsonStore,
    source_id: &str,
    destination: &Path,
) -> AppResult<()> {
    for profile in store
        .snapshot()
        .await
        .profiles
        .into_iter()
        .filter(|profile| profile.id != source_id)
    {
        let path = PathBuf::from(profile.instance_path);
        let comparable = if tokio::fs::try_exists(&path).await? {
            tokio::fs::canonicalize(path).await?
        } else {
            path
        };
        if comparable == destination {
            return Err(AppError::Message(
                "Essa pasta já pertence a outro modpack do Mosaic.".into(),
            ));
        }
    }
    Ok(())
}

async fn validate_registered_mod_files(profile: &ModpackProfile, source: &Path) -> AppResult<()> {
    let mods = source.join("mods");
    for item in &profile.mods {
        let filename = safe_registered_filename(&item.filename)?;
        let path = mods.join(filename);
        let metadata = tokio::fs::symlink_metadata(&path).await.map_err(|error| {
            AppError::Message(format!(
                "O arquivo registrado de {} não pôde ser copiado: {error}",
                item.name
            ))
        })?;
        if !metadata.is_file() || metadata.file_type().is_symlink() {
            return Err(AppError::Message(format!(
                "O arquivo registrado de {} não é um arquivo regular seguro.",
                item.name
            )));
        }
    }
    Ok(())
}

fn safe_registered_filename(value: &str) -> AppResult<&std::ffi::OsStr> {
    let path = Path::new(value);
    let filename = path
        .file_name()
        .ok_or_else(|| AppError::Message("Um mod possui nome de arquivo inválido.".into()))?;
    if path != Path::new(filename) {
        return Err(AppError::Message(
            "Um mod registrado possui um caminho inseguro.".into(),
        ));
    }
    Ok(filename)
}

async fn copy_registered_mods(
    profile: &ModpackProfile,
    source: &Path,
    destination: &Path,
) -> AppResult<CopyStats> {
    let source_mods = source.join("mods");
    let destination_mods = destination.join("mods");
    tokio::fs::create_dir_all(&destination_mods).await?;
    let mut stats = CopyStats::default();
    for item in &profile.mods {
        let filename = safe_registered_filename(&item.filename)?;
        let bytes =
            tokio::fs::copy(source_mods.join(filename), destination_mods.join(filename)).await?;
        stats.files += 1;
        stats.bytes += bytes;
    }
    Ok(stats)
}

async fn copy_directory_tree(source: &Path, destination: &Path) -> AppResult<CopyStats> {
    tokio::fs::create_dir_all(destination).await?;
    let mut pending = vec![(source.to_owned(), destination.to_owned())];
    let mut stats = CopyStats::default();
    while let Some((current_source, current_destination)) = pending.pop() {
        let mut entries = tokio::fs::read_dir(&current_source).await?;
        while let Some(entry) = entries.next_entry().await? {
            let file_type = entry.file_type().await?;
            if file_type.is_symlink() {
                return Err(AppError::Message(format!(
                    "A instância contém um link simbólico que não pode ser copiado com segurança: {}",
                    entry.path().display()
                )));
            }
            let destination_path = current_destination.join(entry.file_name());
            if file_type.is_dir() {
                tokio::fs::create_dir(&destination_path).await?;
                pending.push((entry.path(), destination_path));
            } else if file_type.is_file() {
                let bytes = tokio::fs::copy(entry.path(), destination_path).await?;
                stats.files += 1;
                stats.bytes += bytes;
            } else {
                return Err(AppError::Message(format!(
                    "A instância contém um item especial que não pode ser copiado: {}",
                    entry.path().display()
                )));
            }
        }
    }
    Ok(stats)
}

async fn finalize_duplicate_directory(
    staging: &Path,
    destination: &Path,
    destination_existed: bool,
) -> AppResult<()> {
    if destination_existed {
        if !directory_is_empty(destination).await? {
            return Err(AppError::Message(
                "A pasta de destino recebeu arquivos durante a cópia. A operação foi cancelada."
                    .into(),
            ));
        }
        tokio::fs::remove_dir(destination).await?;
    } else if tokio::fs::try_exists(destination).await? {
        return Err(AppError::Message(
            "A pasta de destino passou a existir durante a cópia. Nada foi sobrescrito.".into(),
        ));
    }
    if let Err(error) = tokio::fs::rename(staging, destination).await {
        if destination_existed {
            let _ = tokio::fs::create_dir_all(destination).await;
        }
        return Err(error.into());
    }
    Ok(())
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
    let dependency_users = dependency_root_users(mods);
    let mut entries: Vec<_> = mods
        .iter()
        .map(|item| {
            let name = normalized_mod_name(item);
            let users = dependency_users
                .get(&item.as_ref().key())
                .filter(|users| !users.is_empty())
                .map(|users| {
                    let names = users
                        .iter()
                        .map(|user| normalized_mod_name(user))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!(" (DEPENDÊNCIA DE: {names})")
                })
                .unwrap_or_default();
            format!("{} ({name}){users}", item.filename)
        })
        .collect();
    entries.sort_by_key(|entry| entry.to_lowercase());
    if entries.is_empty() {
        String::new()
    } else {
        format!("{}\r\n", entries.join("\r\n"))
    }
}

fn dependency_root_users<'a>(mods: &'a [InstalledMod]) -> HashMap<String, Vec<&'a InstalledMod>> {
    let installed_by_key: HashMap<_, _> = mods
        .iter()
        .map(|item| (item.as_ref().key(), item))
        .collect();
    let mut direct_by_dependency: HashMap<String, Vec<&InstalledMod>> = HashMap::new();
    for owner in mods {
        for dependency in owner.required_dependencies.iter().flatten() {
            let key = dependency.key();
            if !installed_by_key.contains_key(&key) {
                continue;
            }
            let dependents = direct_by_dependency.entry(key).or_default();
            if !dependents
                .iter()
                .any(|current| current.as_ref().key() == owner.as_ref().key())
            {
                dependents.push(owner);
            }
        }
    }

    mods.iter()
        .map(|item| {
            let item_key = item.as_ref().key();
            let direct = direct_by_dependency
                .get(&item_key)
                .cloned()
                .unwrap_or_default();
            let mut reached = HashMap::new();
            let mut pending = direct.clone();
            while let Some(dependent) = pending.pop() {
                let key = dependent.as_ref().key();
                if key == item_key || reached.contains_key(&key) {
                    continue;
                }
                reached.insert(key.clone(), dependent);
                if let Some(parents) = direct_by_dependency.get(&key) {
                    pending.extend(parents.iter().copied());
                }
            }
            let mut roots: Vec<_> = reached
                .into_values()
                .filter(|dependent| !matches!(dependent.reason, InstallReason::Required))
                .collect();
            if roots.is_empty() {
                roots = direct;
            }
            roots.sort_by_key(|item| {
                (
                    normalized_mod_name(item).to_lowercase(),
                    item.as_ref().key(),
                )
            });
            roots.dedup_by(|left, right| left.as_ref().key() == right.as_ref().key());
            (item_key, roots)
        })
        .collect()
}

fn normalized_mod_name(item: &InstalledMod) -> String {
    item.name.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::{
        ProfileService, prepare_duplicate_destination, render_mod_list,
        validate_distinct_instance_paths, validate_profile_metadata,
    };
    use crate::{
        domain::{
            CreateProfileInput, DuplicateProfileInput, DuplicateProfileMode, InstallReason,
            InstalledMod, ModLoader, ProfileTarget, ProjectRef, ProviderId, ReleaseChannel,
        },
        infrastructure::store::JsonStore,
    };
    use std::{path::Path, sync::Arc};

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

    #[test]
    fn annotates_shared_and_transitive_dependencies_in_the_mod_list() {
        let entry = |project_id: &str, name: &str, reason: InstallReason, dependencies: &[&str]| {
            InstalledMod {
                provider: ProviderId::Modrinth,
                project_id: project_id.into(),
                name: name.into(),
                version_id: "version".into(),
                version_number: "1".into(),
                filename: format!("{project_id}.jar"),
                installed_at: String::new(),
                reason,
                hashes: Vec::new(),
                enabled: true,
                required_dependencies: Some(
                    dependencies
                        .iter()
                        .map(|project_id| ProjectRef {
                            provider: ProviderId::Modrinth,
                            project_id: (*project_id).into(),
                        })
                        .collect(),
                ),
            }
        };
        let mods = vec![
            entry("alpha", "Mod Alpha", InstallReason::Requested, &["library"]),
            entry("beta", "Mod\nBeta", InstallReason::Requested, &["core"]),
            entry("library", "Library", InstallReason::Required, &["core"]),
            entry("core", "Core", InstallReason::Required, &[]),
        ];

        assert_eq!(
            render_mod_list(&mods),
            concat!(
                "alpha.jar (Mod Alpha)\r\n",
                "beta.jar (Mod Beta)\r\n",
                "core.jar (Core) (DEPENDÊNCIA DE: Mod Alpha, Mod Beta)\r\n",
                "library.jar (Library) (DEPENDÊNCIA DE: Mod Alpha)\r\n",
            )
        );
    }

    #[test]
    fn dependency_annotations_tolerate_cycles() {
        let dependency = |project_id: &str| ProjectRef {
            provider: ProviderId::Modrinth,
            project_id: project_id.into(),
        };
        let entry = |project_id: &str, depends_on: &str| InstalledMod {
            provider: ProviderId::Modrinth,
            project_id: project_id.into(),
            name: project_id.to_uppercase(),
            version_id: "version".into(),
            version_number: "1".into(),
            filename: format!("{project_id}.jar"),
            installed_at: String::new(),
            reason: InstallReason::Required,
            hashes: Vec::new(),
            enabled: true,
            required_dependencies: Some(vec![dependency(depends_on)]),
        };

        let output = render_mod_list(&[entry("a", "b"), entry("b", "a")]);
        assert!(output.contains("a.jar (A) (DEPENDÊNCIA DE: B)"));
        assert!(output.contains("b.jar (B) (DEPENDÊNCIA DE: A)"));
    }

    #[tokio::test]
    async fn duplicates_the_complete_instance_without_changing_the_source() {
        let directory = tempfile::tempdir().unwrap();
        let service = test_service(directory.path()).await;
        let source_path = directory.path().join("source");
        let source = service
            .create(profile_input("Original", &source_path))
            .await
            .unwrap();
        tokio::fs::create_dir_all(source_path.join("config"))
            .await
            .unwrap();
        tokio::fs::write(source_path.join("mods").join("example.jar"), b"mod")
            .await
            .unwrap();
        tokio::fs::write(source_path.join("config").join("example.toml"), b"config")
            .await
            .unwrap();
        add_installed_mod(&service, &source.id, "example.jar").await;
        let destination = directory.path().join("complete-copy");

        let result = service
            .duplicate(
                &source.id,
                DuplicateProfileInput {
                    name: "Cópia completa".into(),
                    description: Some("Clone".into()),
                    instance_path: Some(destination.to_string_lossy().into_owned()),
                    mode: DuplicateProfileMode::Full,
                },
            )
            .await
            .unwrap();

        assert_eq!(result.copied_files, 2);
        assert_eq!(result.profile.mods.len(), 1);
        assert_eq!(
            tokio::fs::read(destination.join("mods/example.jar"))
                .await
                .unwrap(),
            b"mod"
        );
        assert_eq!(
            tokio::fs::read(destination.join("config/example.toml"))
                .await
                .unwrap(),
            b"config"
        );
        assert!(
            tokio::fs::try_exists(source_path.join("config/example.toml"))
                .await
                .unwrap()
        );
        assert_eq!(service.list().await.len(), 2);
    }

    #[tokio::test]
    async fn clean_duplicate_copies_only_registered_mods() {
        let directory = tempfile::tempdir().unwrap();
        let service = test_service(directory.path()).await;
        let source_path = directory.path().join("source");
        let source = service
            .create(profile_input("Original", &source_path))
            .await
            .unwrap();
        tokio::fs::create_dir_all(source_path.join("config"))
            .await
            .unwrap();
        tokio::fs::write(source_path.join("mods/managed.jar"), b"managed")
            .await
            .unwrap();
        tokio::fs::write(source_path.join("mods/manual.jar"), b"manual")
            .await
            .unwrap();
        tokio::fs::write(source_path.join("config/settings.toml"), b"settings")
            .await
            .unwrap();
        add_installed_mod(&service, &source.id, "managed.jar").await;
        let destination = directory.path().join("clean-copy");

        service
            .duplicate(
                &source.id,
                DuplicateProfileInput {
                    name: "Cópia limpa".into(),
                    description: None,
                    instance_path: Some(destination.to_string_lossy().into_owned()),
                    mode: DuplicateProfileMode::ModsOnly,
                },
            )
            .await
            .unwrap();

        assert!(
            tokio::fs::try_exists(destination.join("mods/managed.jar"))
                .await
                .unwrap()
        );
        assert!(
            !tokio::fs::try_exists(destination.join("mods/manual.jar"))
                .await
                .unwrap()
        );
        assert!(
            !tokio::fs::try_exists(destination.join("config/settings.toml"))
                .await
                .unwrap()
        );
    }

    #[test]
    fn refuses_overlapping_instance_paths() {
        let source = Path::new("C:/packs/original");
        assert!(validate_distinct_instance_paths(source, source).is_err());
        assert!(
            validate_distinct_instance_paths(source, Path::new("C:/packs/original/copy")).is_err()
        );
        assert!(validate_distinct_instance_paths(source, Path::new("C:/packs")).is_err());
        assert!(validate_distinct_instance_paths(source, Path::new("C:/copies/pack")).is_ok());
    }

    #[tokio::test]
    async fn refuses_a_non_empty_destination_without_touching_its_files() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("occupied");
        tokio::fs::create_dir(&destination).await.unwrap();
        tokio::fs::write(destination.join("keep.txt"), b"user data")
            .await
            .unwrap();

        assert!(prepare_duplicate_destination(&destination).await.is_err());
        assert_eq!(
            tokio::fs::read(destination.join("keep.txt")).await.unwrap(),
            b"user data"
        );
    }

    async fn test_service(root: &Path) -> ProfileService {
        let store = Arc::new(JsonStore::load(root.join("state.json")).await.unwrap());
        ProfileService::new(store, root.join("profiles"))
    }

    fn profile_input(name: &str, path: &Path) -> CreateProfileInput {
        CreateProfileInput {
            name: name.into(),
            description: None,
            target: ProfileTarget {
                minecraft_version: "1.20.1".into(),
                loader: ModLoader::Forge,
                release_channels: vec![ReleaseChannel::Release],
            },
            instance_path: Some(path.to_string_lossy().into_owned()),
        }
    }

    async fn add_installed_mod(service: &ProfileService, profile_id: &str, filename: &str) {
        service
            .store
            .update(|database| {
                database
                    .profiles
                    .iter_mut()
                    .find(|profile| profile.id == profile_id)
                    .unwrap()
                    .mods
                    .push(InstalledMod {
                        provider: ProviderId::Modrinth,
                        project_id: "example".into(),
                        name: "Example".into(),
                        version_id: "version".into(),
                        version_number: "1".into(),
                        filename: filename.into(),
                        installed_at: String::new(),
                        reason: InstallReason::Requested,
                        hashes: Vec::new(),
                        enabled: true,
                        required_dependencies: Some(Vec::new()),
                    });
            })
            .await
            .unwrap();
    }
}
