use super::{
    file_integrity::{hash_file, preferred_hash},
    profiles::ProfileService,
    provider_fallback::find_equivalent_project,
};
use crate::{
    domain::*,
    error::{AppError, AppResult},
    providers::ProviderRegistry,
};
use chrono::Utc;
use futures_util::{StreamExt, stream};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct ModOrganizationService {
    profiles: Arc<ProfileService>,
    providers: Arc<ProviderRegistry>,
    plans: RwLock<HashMap<String, StoredOrganizationPlan>>,
}

#[derive(Clone)]
struct StoredOrganizationPlan {
    profile_id: String,
    expected_updated_at: String,
    items: Vec<StoredOrganizationItem>,
}

#[derive(Clone)]
struct StoredOrganizationItem {
    item: ModOrganizationItem,
    hashes: Vec<FileHash>,
}

impl ModOrganizationService {
    pub fn new(profiles: Arc<ProfileService>, providers: Arc<ProviderRegistry>) -> Self {
        Self {
            profiles,
            providers,
            plans: RwLock::new(HashMap::new()),
        }
    }

    pub async fn preview(&self, profile_id: &str) -> AppResult<ModOrganizationPlan> {
        let profile = self.profiles.get(profile_id).await?;
        if profile.mods.is_empty() {
            return Err(AppError::Message(
                "Este modpack não possui mods instalados para organizar.".into(),
            ));
        }
        let providers = self.providers.clone();
        let target = profile.target.clone();
        let mut items: Vec<_> = stream::iter(profile.mods.clone())
            .map(|installed| {
                let providers = providers.clone();
                let target = target.clone();
                async move { classify_mod(&providers, &target, installed).await }
            })
            .buffer_unordered(6)
            .collect()
            .await;
        items.sort_by(|left, right| {
            left.item
                .name
                .to_lowercase()
                .cmp(&right.item.name.to_lowercase())
        });

        let id = Uuid::new_v4().to_string();
        let stored = StoredOrganizationPlan {
            profile_id: profile.id,
            expected_updated_at: profile.updated_at,
            items: items.clone(),
        };
        let mut plans = self.plans.write().await;
        plans.insert(id.clone(), stored);
        if plans.len() > 20 {
            if let Some(oldest) = plans.keys().next().cloned() {
                plans.remove(&oldest);
            }
        }
        Ok(ModOrganizationPlan {
            id,
            items: items.into_iter().map(|entry| entry.item).collect(),
        })
    }

    pub async fn export(
        &self,
        profile_id: &str,
        plan_id: &str,
        assignments: Vec<ModOrganizationAssignment>,
        destination_parent: &Path,
    ) -> AppResult<ModOrganizationResult> {
        let plan = self
            .plans
            .read()
            .await
            .get(plan_id)
            .cloned()
            .ok_or_else(|| {
                AppError::Message(
                    "A classificação expirou. Analise os mods novamente antes de organizar.".into(),
                )
            })?;
        if plan.profile_id != profile_id {
            return Err(AppError::Message(
                "Esta classificação pertence a outro modpack.".into(),
            ));
        }
        let profile = self.profiles.get(profile_id).await?;
        if profile.updated_at != plan.expected_updated_at {
            return Err(AppError::Message(
                "O modpack mudou depois da análise. Classifique os mods novamente.".into(),
            ));
        }
        let assignment_map: HashMap<_, _> = assignments
            .into_iter()
            .map(|assignment| (assignment.project.key(), assignment.side))
            .collect();
        let result =
            write_organized_export(&profile, &plan.items, &assignment_map, destination_parent)
                .await?;
        let current_profile = match self.profiles.get(profile_id).await {
            Ok(profile) => profile,
            Err(error) => {
                let _ = tokio::fs::remove_dir_all(&result.destination).await;
                return Err(error);
            }
        };
        if current_profile.updated_at != plan.expected_updated_at {
            let _ = tokio::fs::remove_dir_all(&result.destination).await;
            return Err(AppError::Message(
                "O modpack mudou durante a exportação. As pastas geradas foram descartadas.".into(),
            ));
        }
        self.plans.write().await.remove(plan_id);
        Ok(result)
    }
}

async fn classify_mod(
    providers: &ProviderRegistry,
    target: &ProfileTarget,
    installed: InstalledMod,
) -> StoredOrganizationItem {
    if !installed.provider.is_remote() {
        return organization_item(
            installed,
            ProjectSide::Unknown,
            OrganizationClassificationSource::Unknown,
        );
    }
    let provider = providers.get(installed.provider);
    if provider.is_enabled() {
        if let Ok(side) = provider.get_version_side(&installed.version_id).await {
            if side != ProjectSide::Unknown {
                return organization_item(
                    installed,
                    side,
                    OrganizationClassificationSource::Provider,
                );
            }
        }
    }
    let primary = if provider.is_enabled() {
        provider.get_project(&installed.project_id).await.ok()
    } else {
        None
    };
    if let Some(project) = &primary {
        if project.side != ProjectSide::Unknown {
            return organization_item(
                installed,
                project.side,
                OrganizationClassificationSource::Provider,
            );
        }
    }

    let source = primary.unwrap_or_else(|| synthetic_summary(&installed));
    let alternate = ProviderRegistry::alternate_id(installed.provider);
    if let Some((project, version)) =
        find_equivalent_project(providers, &source, target, alternate, false).await
    {
        let alternate_provider = providers.get(alternate);
        let side = match alternate_provider
            .get_version_side(&version.version_id)
            .await
        {
            Ok(side) if side != ProjectSide::Unknown => side,
            _ => project.side,
        };
        if side != ProjectSide::Unknown {
            return organization_item(
                installed,
                side,
                OrganizationClassificationSource::CrossProvider,
            );
        }
    }
    organization_item(
        installed,
        ProjectSide::Unknown,
        OrganizationClassificationSource::Unknown,
    )
}

fn organization_item(
    installed: InstalledMod,
    side: ProjectSide,
    source: OrganizationClassificationSource,
) -> StoredOrganizationItem {
    let project = installed.as_ref();
    StoredOrganizationItem {
        hashes: installed.hashes,
        item: ModOrganizationItem {
            project,
            name: installed.name,
            filename: installed.filename,
            side,
            source,
        },
    }
}

fn synthetic_summary(installed: &InstalledMod) -> ProjectSummary {
    ProjectSummary {
        provider: installed.provider,
        project_id: installed.project_id.clone(),
        slug: String::new(),
        name: installed.name.clone(),
        summary: String::new(),
        author: String::new(),
        icon_url: None,
        website_url: String::new(),
        downloads: 0,
        updated_at: String::new(),
        categories: Vec::new(),
        supported_versions: Vec::new(),
        supported_loaders: Vec::new(),
        side: ProjectSide::Unknown,
        featured: None,
    }
}

async fn write_organized_export(
    profile: &ModpackProfile,
    items: &[StoredOrganizationItem],
    assignments: &HashMap<String, ProjectSide>,
    destination_parent: &Path,
) -> AppResult<ModOrganizationResult> {
    let parent = tokio::fs::canonicalize(destination_parent)
        .await
        .map_err(|error| {
            AppError::Message(format!("A pasta de destino não pôde ser acessada: {error}"))
        })?;
    if !tokio::fs::metadata(&parent).await?.is_dir() {
        return Err(AppError::Message(
            "O destino escolhido não é uma pasta.".into(),
        ));
    }
    let mods_root = tokio::fs::canonicalize(PathBuf::from(&profile.instance_path).join("mods"))
        .await
        .map_err(|error| {
            AppError::Message(format!("A pasta de mods não pôde ser acessada: {error}"))
        })?;
    if parent.starts_with(&mods_root) {
        return Err(AppError::Message(
            "Escolha um destino fora da pasta ativa de mods para evitar JARs duplicados no loader."
                .into(),
        ));
    }
    let mut file_index = ModFileIndex::scan(&mods_root).await?;

    let destination = unique_destination(&parent, &profile.name).await?;
    let staging = parent.join(format!(".mosaic-organize-{}.part", Uuid::new_v4()));
    if let Err(error) = tokio::fs::create_dir(&staging).await {
        return Err(error.into());
    }
    for folder in [
        "Cliente",
        "Servidor",
        "Cliente e Servidor",
        "Não classificados",
    ] {
        if let Err(error) = tokio::fs::create_dir(staging.join(folder)).await {
            let _ = tokio::fs::remove_dir_all(&staging).await;
            return Err(error.into());
        }
    }

    let mut counts = [0usize; 4];
    let mut copied_files = 0u64;
    let mut copied_bytes = 0u64;
    let mut manifest_entries = Vec::new();
    let mut warnings = Vec::new();
    for stored in items {
        let item = &stored.item;
        let side = assignments
            .get(&item.project.key())
            .copied()
            .unwrap_or(item.side);
        let (folder, count_index) = side_folder(side);
        let filename = match safe_filename(&item.filename) {
            Ok(filename) => filename,
            Err(error) => {
                let _ = tokio::fs::remove_dir_all(&staging).await;
                return Err(error);
            }
        };
        let source = match file_index
            .resolve(&mods_root, filename, &stored.hashes)
            .await
        {
            Ok(Some(source)) => source,
            Ok(None) => {
                warnings.push(format!(
                    "{} foi ignorado porque o arquivo {} não existe mais na pasta de mods.",
                    item.name, item.filename
                ));
                continue;
            }
            Err(error) => {
                let _ = tokio::fs::remove_dir_all(&staging).await;
                return Err(error);
            }
        };
        let bytes = match tokio::fs::copy(&source, staging.join(folder).join(filename)).await {
            Ok(bytes) => bytes,
            Err(error) => {
                let _ = tokio::fs::remove_dir_all(&staging).await;
                return Err(error.into());
            }
        };
        counts[count_index] += 1;
        copied_files += 1;
        copied_bytes += bytes;
        manifest_entries.push(format!("{folder}/{} ({})", item.filename, item.name));
    }
    manifest_entries.sort_by_key(|entry| entry.to_lowercase());
    let manifest = format!(
        "Mosaic Modpack Studio 0.12.0\r\nModpack: {}\r\nMinecraft: {} · {}\r\nGerado em: {}\r\nArquivos ignorados: {}\r\n\r\n{}{}\r\n",
        profile.name,
        profile.target.minecraft_version,
        profile.target.loader.as_str(),
        Utc::now().to_rfc3339(),
        warnings.len(),
        manifest_entries.join("\r\n"),
        if warnings.is_empty() {
            String::new()
        } else {
            format!("\r\n\r\nAVISOS\r\n{}", warnings.join("\r\n"))
        }
    );
    if let Err(error) = tokio::fs::write(staging.join("manifesto.txt"), manifest).await {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error.into());
    }
    if let Err(error) = tokio::fs::rename(&staging, &destination).await {
        let _ = tokio::fs::remove_dir_all(&staging).await;
        return Err(error.into());
    }
    Ok(ModOrganizationResult {
        destination: destination.to_string_lossy().into_owned(),
        copied_files,
        copied_bytes,
        client: counts[0],
        server: counts[1],
        both: counts[2],
        unknown: counts[3],
        skipped_files: warnings.len(),
        warnings,
    })
}

struct ModFileIndex {
    files: Vec<PathBuf>,
    by_name: HashMap<String, Vec<PathBuf>>,
    hashes: HashMap<(PathBuf, HashAlgorithm), String>,
}

impl ModFileIndex {
    async fn scan(mods_root: &Path) -> AppResult<Self> {
        let mut files = Vec::new();
        let mut directories = vec![mods_root.to_path_buf()];
        while let Some(directory) = directories.pop() {
            let mut entries = tokio::fs::read_dir(&directory).await?;
            while let Some(entry) = entries.next_entry().await? {
                let file_type = entry.file_type().await?;
                if file_type.is_symlink() {
                    continue;
                }
                if file_type.is_dir() {
                    directories.push(entry.path());
                    continue;
                }
                if file_type.is_file()
                    && entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
                {
                    files.push(entry.path());
                }
            }
        }
        files.sort();
        let mut by_name: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for path in &files {
            if let Some(filename) = path.file_name() {
                by_name
                    .entry(filename.to_string_lossy().to_lowercase())
                    .or_default()
                    .push(path.clone());
            }
        }
        Ok(Self {
            files,
            by_name,
            hashes: HashMap::new(),
        })
    }

    async fn resolve(
        &mut self,
        mods_root: &Path,
        filename: &std::ffi::OsStr,
        known_hashes: &[FileHash],
    ) -> AppResult<Option<PathBuf>> {
        let registered = mods_root.join(filename);
        match tokio::fs::symlink_metadata(&registered).await {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                return Ok(Some(registered));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }

        let key = filename.to_string_lossy().to_lowercase();
        if let Some(matches) = self.by_name.get(&key) {
            if let Some(path) = matches.first() {
                return Ok(Some(path.clone()));
            }
        }

        let Some(expected) = preferred_hash(known_hashes) else {
            return Ok(None);
        };
        for path in &self.files {
            let cache_key = (path.clone(), expected.algorithm);
            let actual = if let Some(value) = self.hashes.get(&cache_key) {
                value.clone()
            } else {
                let value = hash_file(path, expected.algorithm).await?;
                self.hashes.insert(cache_key, value.clone());
                value
            };
            if actual.eq_ignore_ascii_case(&expected.value) {
                return Ok(Some(path.clone()));
            }
        }
        Ok(None)
    }
}

fn safe_filename(value: &str) -> AppResult<&std::ffi::OsStr> {
    let path = Path::new(value);
    let filename = path
        .file_name()
        .ok_or_else(|| AppError::Message("Um mod possui um nome de arquivo inválido.".into()))?;
    if path != Path::new(filename) {
        return Err(AppError::Message(
            "Um mod registrado possui um caminho inseguro.".into(),
        ));
    }
    Ok(filename)
}

fn side_folder(side: ProjectSide) -> (&'static str, usize) {
    match side {
        ProjectSide::Client => ("Cliente", 0),
        ProjectSide::Server => ("Servidor", 1),
        ProjectSide::Both => ("Cliente e Servidor", 2),
        ProjectSide::Unknown => ("Não classificados", 3),
    }
}

async fn unique_destination(parent: &Path, profile_name: &str) -> AppResult<PathBuf> {
    let slug = safe_folder_name(profile_name);
    for suffix in 1..=100u8 {
        let name = if suffix == 1 {
            format!("{slug}-mods-organizados")
        } else {
            format!("{slug}-mods-organizados-{suffix}")
        };
        let candidate = parent.join(name);
        if !tokio::fs::try_exists(&candidate).await? {
            return Ok(candidate);
        }
    }
    Err(AppError::Message(
        "Há muitas exportações com o mesmo nome nessa pasta. Escolha outro destino.".into(),
    ))
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
    use super::*;

    fn profile(root: &Path) -> ModpackProfile {
        ModpackProfile {
            id: "profile".into(),
            name: "Pack de Teste".into(),
            description: String::new(),
            target: ProfileTarget {
                minecraft_version: "1.20.1".into(),
                loader: ModLoader::Fabric,
                release_channels: vec![ReleaseChannel::Release],
            },
            instance_path: root.to_string_lossy().into_owned(),
            created_at: String::new(),
            updated_at: String::new(),
            mods: Vec::new(),
        }
    }

    fn item(id: &str, filename: &str, side: ProjectSide) -> StoredOrganizationItem {
        StoredOrganizationItem {
            hashes: Vec::new(),
            item: ModOrganizationItem {
                project: ProjectRef {
                    provider: ProviderId::Modrinth,
                    project_id: id.into(),
                },
                name: id.into(),
                filename: filename.into(),
                side,
                source: OrganizationClassificationSource::Provider,
            },
        }
    }

    #[tokio::test]
    async fn exports_each_mod_to_its_environment_without_moving_the_original() {
        let directory = tempfile::tempdir().unwrap();
        let instance = directory.path().join("instance");
        let mods = instance.join("mods");
        let output = directory.path().join("output");
        tokio::fs::create_dir_all(&mods).await.unwrap();
        tokio::fs::create_dir(&output).await.unwrap();
        tokio::fs::write(mods.join("client.jar"), b"client")
            .await
            .unwrap();
        tokio::fs::write(mods.join("common.jar"), b"common")
            .await
            .unwrap();
        let items = vec![
            item("Client Mod", "client.jar", ProjectSide::Client),
            item("Common Mod", "common.jar", ProjectSide::Both),
        ];

        let result = write_organized_export(&profile(&instance), &items, &HashMap::new(), &output)
            .await
            .unwrap();
        let destination = PathBuf::from(result.destination);

        assert!(
            tokio::fs::try_exists(destination.join("Cliente/client.jar"))
                .await
                .unwrap()
        );
        assert!(
            tokio::fs::try_exists(destination.join("Cliente e Servidor/common.jar"))
                .await
                .unwrap()
        );
        assert!(
            tokio::fs::try_exists(mods.join("client.jar"))
                .await
                .unwrap()
        );
        assert_eq!(result.client, 1);
        assert_eq!(result.both, 1);
    }

    #[tokio::test]
    async fn applies_a_reviewed_manual_classification() {
        let directory = tempfile::tempdir().unwrap();
        let instance = directory.path().join("instance");
        let mods = instance.join("mods");
        let output = directory.path().join("output");
        tokio::fs::create_dir_all(&mods).await.unwrap();
        tokio::fs::create_dir(&output).await.unwrap();
        tokio::fs::write(mods.join("unknown.jar"), b"unknown")
            .await
            .unwrap();
        let unknown = item("Unknown", "unknown.jar", ProjectSide::Unknown);
        let assignments = HashMap::from([(unknown.item.project.key(), ProjectSide::Server)]);

        let result = write_organized_export(&profile(&instance), &[unknown], &assignments, &output)
            .await
            .unwrap();

        assert!(
            tokio::fs::try_exists(PathBuf::from(result.destination).join("Servidor/unknown.jar"))
                .await
                .unwrap()
        );
        assert_eq!(result.server, 1);
        assert_eq!(result.unknown, 0);
    }

    #[tokio::test]
    async fn refuses_to_export_inside_the_active_mods_directory() {
        let directory = tempfile::tempdir().unwrap();
        let instance = directory.path().join("instance");
        let mods = instance.join("mods");
        tokio::fs::create_dir_all(&mods).await.unwrap();
        tokio::fs::write(mods.join("client.jar"), b"client")
            .await
            .unwrap();
        let items = [item("Client Mod", "client.jar", ProjectSide::Client)];

        assert!(
            write_organized_export(&profile(&instance), &items, &HashMap::new(), &mods)
                .await
                .is_err()
        );
        assert!(
            tokio::fs::try_exists(mods.join("client.jar"))
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn recovers_a_registered_mod_from_a_subfolder_by_filename() {
        let directory = tempfile::tempdir().unwrap();
        let instance = directory.path().join("instance");
        let mods = instance.join("mods");
        let nested = mods.join("disabled");
        let output = directory.path().join("output");
        tokio::fs::create_dir_all(&nested).await.unwrap();
        tokio::fs::create_dir(&output).await.unwrap();
        tokio::fs::write(nested.join("client.jar"), b"client")
            .await
            .unwrap();

        let result = write_organized_export(
            &profile(&instance),
            &[item("Client Mod", "client.jar", ProjectSide::Client)],
            &HashMap::new(),
            &output,
        )
        .await
        .unwrap();

        assert_eq!(result.copied_files, 1);
        assert_eq!(result.skipped_files, 0);
    }

    #[tokio::test]
    async fn recovers_a_renamed_mod_by_its_verified_hash() {
        let directory = tempfile::tempdir().unwrap();
        let instance = directory.path().join("instance");
        let mods = instance.join("mods");
        let output = directory.path().join("output");
        tokio::fs::create_dir_all(&mods).await.unwrap();
        tokio::fs::create_dir(&output).await.unwrap();
        let renamed = mods.join("renamed.jar");
        tokio::fs::write(&renamed, b"same mod bytes").await.unwrap();
        let mut registered = item("Renamed Mod", "original.jar", ProjectSide::Both);
        registered.hashes.push(FileHash {
            algorithm: HashAlgorithm::Sha1,
            value: hash_file(&renamed, HashAlgorithm::Sha1).await.unwrap(),
        });

        let result =
            write_organized_export(&profile(&instance), &[registered], &HashMap::new(), &output)
                .await
                .unwrap();
        let copied = PathBuf::from(&result.destination).join("Cliente e Servidor/original.jar");

        assert_eq!(tokio::fs::read(copied).await.unwrap(), b"same mod bytes");
        assert_eq!(result.skipped_files, 0);
    }

    #[tokio::test]
    async fn skips_only_a_missing_registered_mod_and_finishes_the_export() {
        let directory = tempfile::tempdir().unwrap();
        let instance = directory.path().join("instance");
        let mods = instance.join("mods");
        let output = directory.path().join("output");
        tokio::fs::create_dir_all(&mods).await.unwrap();
        tokio::fs::create_dir(&output).await.unwrap();
        tokio::fs::write(mods.join("present.jar"), b"present")
            .await
            .unwrap();
        let items = [
            item("Present", "present.jar", ProjectSide::Both),
            item("Missing", "missing.jar", ProjectSide::Client),
        ];

        let result = write_organized_export(&profile(&instance), &items, &HashMap::new(), &output)
            .await
            .unwrap();

        assert_eq!(result.copied_files, 1);
        assert_eq!(result.skipped_files, 1);
        assert!(result.warnings[0].contains("Missing"));
    }
}
