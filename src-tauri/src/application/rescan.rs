use super::{
    file_integrity::{curseforge_fingerprint_file, hash_file},
    profiles::ProfileService,
};
use crate::{
    domain::*,
    error::{AppError, AppResult},
    providers::ProviderRegistry,
};
use chrono::Utc;
use futures_util::{StreamExt, stream};
use serde_json::Value;
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct ProfileRescanService {
    profiles: Arc<ProfileService>,
    providers: Arc<ProviderRegistry>,
    plans: RwLock<HashMap<String, StoredRescanPlan>>,
}

#[derive(Clone)]
struct StoredRescanPlan {
    public: RescanProfilePlan,
    expected_updated_at: String,
    mods: Vec<InstalledMod>,
    file_snapshot: Vec<JarSnapshot>,
}

#[derive(Clone, PartialEq, Eq)]
struct JarSnapshot {
    filename: String,
    sha1: String,
}

#[derive(Clone)]
struct ScannedJar {
    filename: String,
    hashes: Vec<FileHash>,
    fingerprint: u32,
    target_hint: JarTargetHint,
}

struct IdentifiedJar {
    item: InstalledMod,
    versions: Vec<ProjectVersion>,
    aliases: Vec<ProjectRef>,
    warnings: Vec<String>,
    target_hint: JarTargetHint,
}

#[derive(Clone, Default)]
struct JarTargetHint {
    minecraft_versions: Vec<String>,
    loader: Option<ModLoader>,
}

#[derive(Default)]
struct PartialTarget {
    minecraft_version: Option<String>,
    loader: Option<ModLoader>,
    sources: Vec<String>,
}

impl ProfileRescanService {
    pub fn new(profiles: Arc<ProfileService>, providers: Arc<ProviderRegistry>) -> Self {
        Self {
            profiles,
            providers,
            plans: RwLock::new(HashMap::new()),
        }
    }

    pub async fn preview(
        &self,
        profile_id: &str,
        selected_folder: &Path,
    ) -> AppResult<RescanProfilePlan> {
        let profile = self.profiles.get(profile_id).await?;
        let instance_path = resolve_instance_path(selected_folder).await?;
        let jars = scan_mods_folder(&instance_path.join("mods")).await?;
        let scanned_files = jars.len();
        let file_snapshot = snapshot_from_scanned_jars(&jars);
        let providers = self.providers.clone();
        let identified: Vec<_> = stream::iter(jars)
            .map(|jar| {
                let providers = providers.clone();
                async move { identify_jar(&providers, jar).await }
            })
            .buffer_unordered(6)
            .collect()
            .await;

        let mut identified = identified.into_iter().collect::<AppResult<Vec<_>>>()?;
        let configured = detect_instance_target(&instance_path).await;
        let (detected_target, detection_source, detection_warning) =
            detect_target(&profile.target, configured, &identified);
        let mut warnings = Vec::new();
        if let Some(warning) = detection_warning {
            warnings.push(warning);
        }
        for item in &identified {
            warnings.extend(item.warnings.clone());
            if !item.versions.is_empty()
                && !item.versions.iter().any(|version| {
                    (version.minecraft_versions.is_empty()
                        || version
                            .minecraft_versions
                            .contains(&detected_target.minecraft_version))
                        && (version.loaders.is_empty()
                            || version.loaders.contains(&detected_target.loader))
                })
            {
                warnings.push(format!(
                    "{} foi identificado, mas não declara compatibilidade com Minecraft {} / {}.",
                    item.item.filename,
                    detected_target.minecraft_version,
                    detected_target.loader.as_str()
                ));
            }
        }

        let (mods, recognized, local_only) = build_mod_index(&mut identified, &mut warnings);
        compact_warnings(&mut warnings);
        let plan_id = Uuid::new_v4().to_string();
        let public = RescanProfilePlan {
            id: plan_id.clone(),
            profile_id: profile.id.clone(),
            instance_path: instance_path.to_string_lossy().into_owned(),
            previous_target: profile.target.clone(),
            detected_target,
            detection_source,
            scanned_files,
            recognized,
            local_only,
            warnings,
        };
        let stored = StoredRescanPlan {
            public: public.clone(),
            expected_updated_at: profile.updated_at,
            mods,
            file_snapshot,
        };
        let mut plans = self.plans.write().await;
        plans.retain(|_, plan| plan.public.profile_id != profile_id);
        if plans.len() >= 8 {
            if let Some(oldest) = plans.keys().next().cloned() {
                plans.remove(&oldest);
            }
        }
        plans.insert(plan_id, stored);
        Ok(public)
    }

    pub async fn apply(&self, profile_id: &str, plan_id: &str) -> AppResult<RescanProfileResult> {
        let stored = self
            .plans
            .read()
            .await
            .get(plan_id)
            .cloned()
            .ok_or_else(|| {
                AppError::Message(
                    "Esta análise expirou. Escolha a pasta novamente antes de substituir.".into(),
                )
            })?;
        if stored.public.profile_id != profile_id {
            return Err(AppError::Message(
                "A análise não pertence ao modpack selecionado.".into(),
            ));
        }
        let current_snapshot =
            snapshot_mods_folder(&Path::new(&stored.public.instance_path).join("mods")).await?;
        if current_snapshot != stored.file_snapshot {
            return Err(AppError::Message(
                "A pasta de mods mudou depois da análise. Nada foi substituído; faça uma nova análise para usar os arquivos atuais.".into(),
            ));
        }
        let profile = self
            .profiles
            .replace_scanned_instance(
                profile_id,
                Path::new(&stored.public.instance_path),
                stored.public.detected_target.clone(),
                stored.mods,
                &stored.expected_updated_at,
            )
            .await?;
        self.plans.write().await.remove(plan_id);
        Ok(RescanProfileResult {
            profile,
            scanned_files: stored.public.scanned_files,
            recognized: stored.public.recognized,
            local_only: stored.public.local_only,
            warnings: stored.public.warnings,
        })
    }
}

fn snapshot_from_scanned_jars(jars: &[ScannedJar]) -> Vec<JarSnapshot> {
    let mut snapshot: Vec<_> = jars
        .iter()
        .filter_map(|jar| {
            Some(JarSnapshot {
                filename: jar.filename.to_lowercase(),
                sha1: jar
                    .hashes
                    .iter()
                    .find(|hash| matches!(hash.algorithm, HashAlgorithm::Sha1))?
                    .value
                    .clone(),
            })
        })
        .collect();
    snapshot.sort_by(|left, right| left.filename.cmp(&right.filename));
    snapshot
}

async fn snapshot_mods_folder(mods_path: &Path) -> AppResult<Vec<JarSnapshot>> {
    let mut entries = tokio::fs::read_dir(mods_path).await.map_err(|error| {
        AppError::Message(format!("A pasta mods não pôde ser conferida: {error}"))
    })?;
    let mut snapshot = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        if !entry.file_type().await?.is_file() || entry.file_type().await?.is_symlink() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().into_owned();
        if !filename.to_lowercase().ends_with(".jar") {
            continue;
        }
        snapshot.push(JarSnapshot {
            filename: filename.to_lowercase(),
            sha1: hash_file(&entry.path(), HashAlgorithm::Sha1).await?,
        });
    }
    snapshot.sort_by(|left, right| left.filename.cmp(&right.filename));
    Ok(snapshot)
}

async fn resolve_instance_path(selected_folder: &Path) -> AppResult<PathBuf> {
    let selected = tokio::fs::canonicalize(selected_folder)
        .await
        .map_err(|error| AppError::Message(format!("A pasta escolhida não existe: {error}")))?;
    if !tokio::fs::metadata(&selected).await?.is_dir() {
        return Err(AppError::Message(
            "Escolha uma pasta, não um arquivo.".into(),
        ));
    }
    let child_mods = selected.join("mods");
    if matches!(tokio::fs::metadata(&child_mods).await, Ok(metadata) if metadata.is_dir()) {
        return Ok(selected);
    }
    if selected
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.eq_ignore_ascii_case("mods"))
    {
        return selected.parent().map(Path::to_owned).ok_or_else(|| {
            AppError::Message("A pasta mods escolhida não possui pasta pai válida.".into())
        });
    }
    Err(AppError::Message(
        "Escolha a pasta da instância que contém mods, ou a própria pasta chamada mods.".into(),
    ))
}

async fn scan_mods_folder(mods_path: &Path) -> AppResult<Vec<ScannedJar>> {
    let mut entries = tokio::fs::read_dir(mods_path)
        .await
        .map_err(|error| AppError::Message(format!("A pasta mods não pôde ser lida: {error}")))?;
    let mut jars = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let metadata = entry.metadata().await?;
        if !metadata.is_file() || entry.file_type().await?.is_symlink() {
            continue;
        }
        let filename = entry.file_name().to_string_lossy().into_owned();
        if !filename.to_lowercase().ends_with(".jar") {
            continue;
        }
        let path = entry.path();
        let sha512 = hash_file(&path, HashAlgorithm::Sha512).await?;
        let sha1 = hash_file(&path, HashAlgorithm::Sha1).await?;
        let fingerprint = curseforge_fingerprint_file(&path).await?;
        let target_hint = inspect_jar_target(path.clone()).await;
        jars.push(ScannedJar {
            filename,
            hashes: vec![
                FileHash {
                    algorithm: HashAlgorithm::Sha512,
                    value: sha512,
                },
                FileHash {
                    algorithm: HashAlgorithm::Sha1,
                    value: sha1,
                },
            ],
            fingerprint,
            target_hint,
        });
    }
    jars.sort_by(|left, right| {
        left.filename
            .to_lowercase()
            .cmp(&right.filename.to_lowercase())
    });
    Ok(jars)
}

async fn identify_jar(providers: &ProviderRegistry, jar: ScannedJar) -> AppResult<IdentifiedJar> {
    let mut warnings = Vec::new();
    let target_hint = jar.target_hint.clone();
    let mut matches = Vec::new();
    for provider_id in [ProviderId::Modrinth, ProviderId::Curseforge] {
        let provider = providers.get(provider_id);
        if !provider.is_enabled() {
            continue;
        }
        let mut version = None;
        let hashes = if provider_id == ProviderId::Curseforge {
            &jar.hashes[..jar.hashes.len().min(1)]
        } else {
            &jar.hashes
        };
        for hash in hashes {
            match provider
                .get_version_by_hash(Some(hash), Some(jar.fingerprint))
                .await
            {
                Ok(Some(found)) => {
                    version = Some(found);
                    break;
                }
                Ok(None) => {}
                Err(error) => {
                    warnings.push(format!(
                        "{} não pôde ser identificado em {}: {}",
                        jar.filename,
                        provider_id.as_str(),
                        error
                    ));
                    break;
                }
            }
        }
        if let Some(version) = version {
            matches.push((provider_id, version));
        }
    }
    for (provider_id, selected_version) in &matches {
        let provider = providers.get(*provider_id);
        let project = match provider.get_project(&selected_version.project_id).await {
            Ok(project) => project,
            Err(error) => {
                warnings.push(format!(
                    "{} foi reconhecido em {}, mas o projeto não pôde ser carregado: {}",
                    jar.filename,
                    provider_id.as_str(),
                    error
                ));
                continue;
            }
        };
        let versions: Vec<_> = matches.iter().map(|(_, version)| version.clone()).collect();
        let aliases = versions
            .iter()
            .map(|version| ProjectRef {
                provider: version.provider,
                project_id: version.project_id.clone(),
            })
            .collect();
        let item = installed_from_versions(project, selected_version, &versions, jar);
        return Ok(IdentifiedJar {
            item,
            versions,
            aliases,
            warnings,
            target_hint,
        });
    }
    Ok(IdentifiedJar {
        item: local_installed_mod(jar),
        versions: Vec::new(),
        aliases: Vec::new(),
        warnings,
        target_hint,
    })
}

fn installed_from_versions(
    project: ProjectSummary,
    selected_version: &ProjectVersion,
    versions: &[ProjectVersion],
    jar: ScannedJar,
) -> InstalledMod {
    let mut required_dependencies: Vec<_> = versions
        .iter()
        .flat_map(|version| &version.dependencies)
        .filter(|dependency| matches!(dependency.dependency_type, DependencyType::Required))
        .filter_map(|dependency| {
            Some(ProjectRef {
                provider: dependency.provider?,
                project_id: dependency.project_id.clone()?,
            })
        })
        .collect();
    required_dependencies.sort_by_key(ProjectRef::key);
    required_dependencies.dedup_by(|left, right| left.key() == right.key());
    InstalledMod {
        provider: project.provider,
        project_id: project.project_id,
        name: project.name,
        version_id: selected_version.version_id.clone(),
        version_number: selected_version.version_number.clone(),
        filename: jar.filename,
        installed_at: Utc::now().to_rfc3339(),
        reason: InstallReason::Requested,
        hashes: jar.hashes,
        enabled: true,
        required_dependencies: Some(required_dependencies),
    }
}

fn local_installed_mod(jar: ScannedJar) -> InstalledMod {
    let sha1 = jar
        .hashes
        .iter()
        .find(|hash| matches!(hash.algorithm, HashAlgorithm::Sha1))
        .map(|hash| hash.value.clone())
        .unwrap_or_else(|| jar.filename.clone());
    InstalledMod {
        provider: ProviderId::Local,
        project_id: format!("sha1:{sha1}"),
        name: display_name_from_filename(&jar.filename),
        version_id: sha1,
        version_number: "Arquivo local".into(),
        filename: jar.filename,
        installed_at: Utc::now().to_rfc3339(),
        reason: InstallReason::Requested,
        hashes: jar.hashes,
        enabled: true,
        required_dependencies: Some(Vec::new()),
    }
}

fn build_mod_index(
    identified: &mut Vec<IdentifiedJar>,
    warnings: &mut Vec<String>,
) -> (Vec<InstalledMod>, usize, usize) {
    let mut seen_aliases = HashSet::new();
    let mut retained = Vec::new();
    for mut identified in identified.drain(..) {
        if identified.aliases.is_empty() {
            identified.aliases.push(identified.item.as_ref());
        }
        if identified
            .aliases
            .iter()
            .map(ProjectRef::key)
            .any(|key| seen_aliases.contains(&key))
        {
            warnings.push(format!(
                "{} foi ignorado porque outro arquivo do mesmo projeto já foi registrado.",
                identified.item.filename
            ));
            continue;
        }
        for alias in &identified.aliases {
            seen_aliases.insert(alias.key());
        }
        retained.push((identified.item, identified.aliases));
    }
    let mut canonical_refs = HashMap::new();
    for (item, aliases) in &retained {
        let canonical = item.as_ref();
        canonical_refs.insert(canonical.key(), canonical.clone());
        for alias in aliases {
            canonical_refs.insert(alias.key(), canonical.clone());
        }
    }
    let mut mods: Vec<_> = retained.into_iter().map(|(item, _)| item).collect();
    for item in &mut mods {
        if let Some(dependencies) = &mut item.required_dependencies {
            for dependency in dependencies.iter_mut() {
                if let Some(canonical) = canonical_refs.get(&dependency.key()) {
                    *dependency = canonical.clone();
                }
            }
            dependencies.sort_by_key(ProjectRef::key);
            dependencies.dedup_by(|left, right| left.key() == right.key());
        }
    }
    let installed_keys: HashSet<_> = mods.iter().map(|item| item.as_ref().key()).collect();
    let dependency_keys: HashSet<_> = mods
        .iter()
        .flat_map(|item| item.required_dependencies.iter().flatten())
        .map(ProjectRef::key)
        .filter(|key| installed_keys.contains(key))
        .collect();
    for item in &mut mods {
        if dependency_keys.contains(&item.as_ref().key()) {
            item.reason = InstallReason::Required;
        }
    }
    mods.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| {
                left.filename
                    .to_lowercase()
                    .cmp(&right.filename.to_lowercase())
            })
    });
    let recognized = mods.iter().filter(|item| item.provider.is_remote()).count();
    (mods, recognized, installed_keys.len() - recognized)
}

async fn detect_instance_target(instance_path: &Path) -> PartialTarget {
    let mut detected = PartialTarget::default();
    for (filename, label) in [
        ("minecraftinstance.json", "perfil do CurseForge"),
        ("mmc-pack.json", "perfil do Prism/MultiMC"),
        ("profile.json", "perfil do Modrinth"),
        ("instance.json", "configuração da instância"),
    ] {
        let path = instance_path.join(filename);
        let Ok(contents) = tokio::fs::read_to_string(&path).await else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&contents) else {
            continue;
        };
        let candidate = if filename == "mmc-pack.json" {
            target_from_mmc_pack(&value)
        } else {
            target_from_json(&value)
        };
        merge_target(&mut detected, candidate, label);
        if detected.minecraft_version.is_some() && detected.loader.is_some() {
            return detected;
        }
    }
    let cfg_path = instance_path.join("instance.cfg");
    if let Ok(contents) = tokio::fs::read_to_string(cfg_path).await {
        merge_target(
            &mut detected,
            target_from_instance_cfg(&contents),
            "configuração do Prism/MultiMC",
        );
    }
    detected
}

fn target_from_mmc_pack(value: &Value) -> (Option<String>, Option<ModLoader>) {
    let mut minecraft_version = None;
    let mut loader = None;
    for component in value
        .get("components")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let uid = component
            .get("uid")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_lowercase();
        let version = component
            .get("version")
            .and_then(Value::as_str)
            .map(str::to_string);
        if uid == "net.minecraft" {
            minecraft_version = version;
        } else if uid.contains("neoforge") {
            loader = Some(ModLoader::Neoforge);
        } else if uid.contains("fabric-loader") {
            loader = Some(ModLoader::Fabric);
        } else if uid.contains("quilt-loader") {
            loader = Some(ModLoader::Quilt);
        } else if uid.contains("forge") {
            loader = Some(ModLoader::Forge);
        }
    }
    (minecraft_version, loader)
}

fn target_from_json(value: &Value) -> (Option<String>, Option<ModLoader>) {
    let minecraft_version = find_string(
        value,
        &[
            "gameVersion",
            "game_version",
            "minecraftVersion",
            "minecraft_version",
        ],
    )
    .filter(|value| looks_like_minecraft_version(value));
    let loader_name = find_string(
        value,
        &[
            "loader",
            "modLoader",
            "mod_loader",
            "baseModLoader",
            "loaderName",
        ],
    );
    let loader = loader_name
        .as_deref()
        .and_then(loader_from_text)
        .or_else(|| {
            value
                .get("baseModLoader")
                .and_then(|loader| loader.get("name"))
                .and_then(Value::as_str)
                .and_then(loader_from_text)
        });
    (minecraft_version, loader)
}

fn find_string(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(object) => {
            for key in keys {
                if let Some(found) = object.get(*key).and_then(Value::as_str) {
                    return Some(found.to_string());
                }
            }
            object.values().find_map(|child| find_string(child, keys))
        }
        Value::Array(values) => values.iter().find_map(|child| find_string(child, keys)),
        _ => None,
    }
}

fn target_from_instance_cfg(contents: &str) -> (Option<String>, Option<ModLoader>) {
    let mut minecraft_version = None;
    let mut loader = None;
    for line in contents.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_lowercase();
        let value = value.trim();
        if matches!(key.as_str(), "intendedversion" | "minecraftversion")
            && looks_like_minecraft_version(value)
        {
            minecraft_version = Some(value.to_string());
        }
        if key.contains("neoforge") {
            loader = Some(ModLoader::Neoforge);
        } else if key.contains("fabric") {
            loader = Some(ModLoader::Fabric);
        } else if key.contains("quilt") {
            loader = Some(ModLoader::Quilt);
        } else if key.contains("forge") {
            loader = Some(ModLoader::Forge);
        }
    }
    (minecraft_version, loader)
}

fn merge_target(
    target: &mut PartialTarget,
    candidate: (Option<String>, Option<ModLoader>),
    source: &str,
) {
    let mut used = false;
    if target.minecraft_version.is_none() && candidate.0.is_some() {
        target.minecraft_version = candidate.0;
        used = true;
    }
    if target.loader.is_none() && candidate.1.is_some() {
        target.loader = candidate.1;
        used = true;
    }
    if used && !target.sources.iter().any(|current| current == source) {
        target.sources.push(source.into());
    }
}

fn detect_target(
    fallback: &ProfileTarget,
    configured: PartialTarget,
    identified: &[IdentifiedJar],
) -> (ProfileTarget, String, Option<String>) {
    let mut counts: HashMap<(String, ModLoader), usize> = HashMap::new();
    let mut version_counts: HashMap<String, usize> = HashMap::new();
    let mut loader_counts: HashMap<ModLoader, usize> = HashMap::new();
    let mut has_provider_metadata = false;
    for version in identified.iter().flat_map(|item| &item.versions) {
        has_provider_metadata = true;
        for loader in &version.loaders {
            if !configured
                .loader
                .is_some_and(|expected| expected != *loader)
            {
                *loader_counts.entry(*loader).or_default() += 1;
            }
        }
        for minecraft_version in version
            .minecraft_versions
            .iter()
            .filter(|value| looks_like_minecraft_version(value))
        {
            if configured
                .minecraft_version
                .as_ref()
                .is_some_and(|expected| expected != minecraft_version)
            {
                continue;
            }
            *version_counts.entry(minecraft_version.clone()).or_default() += 1;
            for loader in &version.loaders {
                if configured
                    .loader
                    .is_some_and(|expected| expected != *loader)
                {
                    continue;
                }
                *counts
                    .entry((minecraft_version.clone(), *loader))
                    .or_default() += 1;
            }
        }
    }
    let mut has_jar_metadata = false;
    for hint in identified.iter().map(|item| &item.target_hint) {
        let Some(loader) = hint.loader else {
            continue;
        };
        has_jar_metadata = true;
        if configured.loader.is_some_and(|expected| expected != loader) {
            continue;
        }
        *loader_counts.entry(loader).or_default() += 2;
        for minecraft_version in &hint.minecraft_versions {
            if configured
                .minecraft_version
                .as_ref()
                .is_some_and(|expected| expected != minecraft_version)
            {
                continue;
            }
            *version_counts.entry(minecraft_version.clone()).or_default() += 2;
            *counts
                .entry((minecraft_version.clone(), loader))
                .or_default() += 2;
        }
    }
    let inferred = counts.into_iter().max_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then_with(|| compare_minecraft_versions(&(left.0).0, &(right.0).0))
    });
    let inferred_version = inferred
        .as_ref()
        .map(|((version, _), _)| version.clone())
        .or_else(|| {
            version_counts
                .into_iter()
                .max_by(|left, right| {
                    left.1
                        .cmp(&right.1)
                        .then_with(|| compare_minecraft_versions(&left.0, &right.0))
                })
                .map(|(version, _)| version)
        });
    let inferred_loader = inferred
        .as_ref()
        .map(|((_, loader), _)| *loader)
        .or_else(|| {
            loader_counts
                .into_iter()
                .max_by(|left, right| left.1.cmp(&right.1))
                .map(|(loader, _)| loader)
        });
    let minecraft_version = configured
        .minecraft_version
        .clone()
        .or_else(|| inferred_version.clone())
        .unwrap_or_else(|| fallback.minecraft_version.clone());
    let loader = configured
        .loader
        .or(inferred_loader)
        .unwrap_or(fallback.loader);
    let used_metadata = (configured.minecraft_version.is_none() && inferred_version.is_some())
        || (configured.loader.is_none() && inferred_loader.is_some());
    let config_has_version = configured.minecraft_version.is_some();
    let config_has_loader = configured.loader.is_some();
    let mut sources = configured.sources;
    if used_metadata {
        sources.push(
            match (has_jar_metadata, has_provider_metadata) {
                (true, true) => "metadados internos dos JARs e catálogos",
                (true, false) => "metadados internos dos JARs",
                _ => "metadados dos catálogos",
            }
            .into(),
        );
    }
    let complete = (config_has_version || inferred_version.is_some())
        && (config_has_loader || inferred_loader.is_some());
    let warning = (!complete).then(|| {
        "Não houve evidência suficiente para detectar todo o alvo; os campos ausentes foram mantidos a partir do perfil anterior.".into()
    });
    let source = if sources.is_empty() {
        "perfil anterior (nenhuma evidência automática encontrada)".into()
    } else {
        sources.join(" + ")
    };
    (
        ProfileTarget {
            minecraft_version,
            loader,
            release_channels: fallback.release_channels.clone(),
        },
        source,
        warning,
    )
}

async fn inspect_jar_target(path: PathBuf) -> JarTargetHint {
    tokio::task::spawn_blocking(move || inspect_jar_target_sync(&path))
        .await
        .unwrap_or_default()
}

fn inspect_jar_target_sync(path: &Path) -> JarTargetHint {
    let Ok(file) = File::open(path) else {
        return JarTargetHint::default();
    };
    let Ok(mut archive) = zip::ZipArchive::new(file) else {
        return JarTargetHint::default();
    };
    for (entry, loader) in [
        ("fabric.mod.json", ModLoader::Fabric),
        ("quilt.mod.json", ModLoader::Quilt),
        ("META-INF/neoforge.mods.toml", ModLoader::Neoforge),
        ("META-INF/mods.toml", ModLoader::Forge),
    ] {
        let Some(contents) = read_zip_text(&mut archive, entry) else {
            continue;
        };
        let minecraft_versions = if entry.ends_with(".json") {
            minecraft_versions_from_json(&contents)
        } else {
            minecraft_versions_from_mods_toml(&contents)
        };
        return JarTargetHint {
            minecraft_versions,
            loader: Some(loader),
        };
    }
    JarTargetHint::default()
}

fn read_zip_text(archive: &mut zip::ZipArchive<File>, name: &str) -> Option<String> {
    let mut entry = archive.by_name(name).ok()?;
    if entry.size() > 1024 * 1024 {
        return None;
    }
    let mut contents = String::new();
    entry.read_to_string(&mut contents).ok()?;
    Some(contents)
}

fn minecraft_versions_from_json(contents: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(contents) else {
        return Vec::new();
    };
    let mut constraints = Vec::new();
    collect_minecraft_constraints(&value, &mut constraints);
    let mut versions: Vec<_> = constraints
        .iter()
        .filter_map(|constraint| first_minecraft_version(constraint))
        .collect();
    versions.sort_by(|left, right| compare_minecraft_versions(right, left));
    versions.dedup();
    versions
}

fn collect_minecraft_constraints(value: &Value, constraints: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if let Some(minecraft) = object.get("minecraft") {
                collect_strings(minecraft, constraints);
            }
            if object
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| id.eq_ignore_ascii_case("minecraft"))
            {
                for key in ["versions", "version", "versionRange"] {
                    if let Some(found) = object.get(key) {
                        collect_strings(found, constraints);
                    }
                }
            }
            for child in object.values() {
                collect_minecraft_constraints(child, constraints);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_minecraft_constraints(child, constraints);
            }
        }
        _ => {}
    }
}

fn collect_strings(value: &Value, values: &mut Vec<String>) {
    match value {
        Value::String(value) => values.push(value.clone()),
        Value::Array(items) => {
            for item in items {
                collect_strings(item, values);
            }
        }
        _ => {}
    }
}

fn minecraft_versions_from_mods_toml(contents: &str) -> Vec<String> {
    let normalized: String = contents
        .chars()
        .filter(|character| !character.is_whitespace())
        .flat_map(char::to_lowercase)
        .collect();
    let Some(minecraft_index) = normalized.find("modid=\"minecraft\"") else {
        return Vec::new();
    };
    let window: String = normalized[minecraft_index..].chars().take(500).collect();
    first_minecraft_version(&window).into_iter().collect()
}

fn first_minecraft_version(value: &str) -> Option<String> {
    value
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .find(|candidate| looks_like_minecraft_version(candidate))
        .map(str::to_string)
}

fn loader_from_text(value: &str) -> Option<ModLoader> {
    let normalized = value.to_lowercase().replace(['-', '_', '.'], "");
    if normalized.contains("neoforge") {
        Some(ModLoader::Neoforge)
    } else if normalized.contains("fabric") {
        Some(ModLoader::Fabric)
    } else if normalized.contains("quilt") {
        Some(ModLoader::Quilt)
    } else if normalized.contains("forge") {
        Some(ModLoader::Forge)
    } else {
        None
    }
}

fn looks_like_minecraft_version(value: &str) -> bool {
    let parts: Vec<_> = value.split('.').collect();
    (2..=4).contains(&parts.len())
        && parts.iter().all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        })
}

fn compare_minecraft_versions(left: &str, right: &str) -> Ordering {
    let parse = |value: &str| {
        value
            .split('.')
            .map(|part| part.parse::<u32>().unwrap_or_default())
            .collect::<Vec<_>>()
    };
    parse(left).cmp(&parse(right))
}

fn compact_warnings(warnings: &mut Vec<String>) {
    warnings.sort();
    warnings.dedup();
    if warnings.len() > 40 {
        let hidden = warnings.len() - 40;
        warnings.truncate(40);
        warnings.push(format!(
            "Outros {} avisos semelhantes foram resumidos.",
            hidden
        ));
    }
}

fn display_name_from_filename(filename: &str) -> String {
    let stem = filename
        .strip_suffix(".jar")
        .or_else(|| filename.strip_suffix(".JAR"))
        .unwrap_or(filename);
    let mut name = stem
        .replace(['-', '_', '.'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if name.is_empty() {
        name = filename.to_string();
    }
    name
}

#[cfg(test)]
mod tests {
    use super::{
        IdentifiedJar, JarTargetHint, build_mod_index, compare_minecraft_versions,
        detect_instance_target, display_name_from_filename, inspect_jar_target_sync,
        loader_from_text, minecraft_versions_from_mods_toml, resolve_instance_path,
        snapshot_mods_folder,
    };
    use crate::domain::{FileHash, InstallReason, InstalledMod, ModLoader, ProjectRef, ProviderId};
    use std::{cmp::Ordering, io::Write};

    #[test]
    fn derives_a_readable_local_name_from_filename() {
        assert_eq!(
            display_name_from_filename("just-enough-items_1.20.1.jar"),
            "just enough items 1 20 1"
        );
    }

    #[test]
    fn recognizes_loader_names_without_confusing_neoforge_with_forge() {
        assert_eq!(
            loader_from_text("neoForge-21.1.1"),
            Some(ModLoader::Neoforge)
        );
        assert_eq!(
            loader_from_text("fabric-loader-0.16"),
            Some(ModLoader::Fabric)
        );
    }

    #[test]
    fn compares_minecraft_versions_numerically() {
        assert_eq!(
            compare_minecraft_versions("1.20.10", "1.20.2"),
            Ordering::Greater
        );
    }

    #[test]
    fn reads_fabric_metadata_from_a_jar() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("example.jar");
        let file = std::fs::File::create(&path).unwrap();
        let mut archive = zip::ZipWriter::new(file);
        archive
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(br#"{"schemaVersion":1,"depends":{"minecraft":">=1.20.1"}}"#)
            .unwrap();
        archive.finish().unwrap();

        let hint = inspect_jar_target_sync(&path);
        assert_eq!(hint.loader, Some(ModLoader::Fabric));
        assert_eq!(hint.minecraft_versions, vec!["1.20.1"]);
    }

    #[test]
    fn reads_the_minecraft_range_after_the_forge_dependency() {
        let versions = minecraft_versions_from_mods_toml(
            r#"
            [[dependencies.example]]
            modId="forge"
            versionRange="[47.2,)"
            [[dependencies.example]]
            modId="minecraft"
            versionRange="[1.20.1,1.21)"
            "#,
        );
        assert_eq!(versions, vec!["1.20.1"]);
    }

    #[test]
    fn canonicalizes_cross_provider_dependencies_during_import() {
        let root = installed(
            ProviderId::Modrinth,
            "root",
            vec![ProjectRef {
                provider: ProviderId::Curseforge,
                project_id: "library-cf".into(),
            }],
        );
        let library = installed(ProviderId::Modrinth, "library-mr", Vec::new());
        let mut identified = vec![
            IdentifiedJar {
                item: root,
                versions: Vec::new(),
                aliases: Vec::new(),
                warnings: Vec::new(),
                target_hint: JarTargetHint::default(),
            },
            IdentifiedJar {
                item: library,
                versions: Vec::new(),
                aliases: vec![
                    ProjectRef {
                        provider: ProviderId::Modrinth,
                        project_id: "library-mr".into(),
                    },
                    ProjectRef {
                        provider: ProviderId::Curseforge,
                        project_id: "library-cf".into(),
                    },
                ],
                warnings: Vec::new(),
                target_hint: JarTargetHint::default(),
            },
        ];
        let mut warnings = Vec::new();
        let (mods, _, _) = build_mod_index(&mut identified, &mut warnings);

        let root = mods.iter().find(|item| item.project_id == "root").unwrap();
        let dependency = &root.required_dependencies.as_ref().unwrap()[0];
        assert_eq!(dependency.provider, ProviderId::Modrinth);
        assert_eq!(dependency.project_id, "library-mr");
        let library = mods
            .iter()
            .find(|item| item.project_id == "library-mr")
            .unwrap();
        assert!(matches!(library.reason, InstallReason::Required));
    }

    #[tokio::test]
    async fn detects_a_curseforge_instance() {
        let temp = tempfile::tempdir().unwrap();
        tokio::fs::write(
            temp.path().join("minecraftinstance.json"),
            r#"{"gameVersion":"1.20.1","baseModLoader":{"name":"forge-47.2.0"}}"#,
        )
        .await
        .unwrap();
        let target = detect_instance_target(temp.path()).await;
        assert_eq!(target.minecraft_version.as_deref(), Some("1.20.1"));
        assert_eq!(target.loader, Some(ModLoader::Forge));
    }

    #[tokio::test]
    async fn detects_a_prism_instance() {
        let temp = tempfile::tempdir().unwrap();
        tokio::fs::write(
            temp.path().join("mmc-pack.json"),
            r#"{"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.neoforged","version":"21.1.0"}]}"#,
        )
        .await
        .unwrap();
        let target = detect_instance_target(temp.path()).await;
        assert_eq!(target.minecraft_version.as_deref(), Some("1.21.1"));
        assert_eq!(target.loader, Some(ModLoader::Neoforge));
    }

    #[tokio::test]
    async fn accepts_a_selected_mods_folder() {
        let temp = tempfile::tempdir().unwrap();
        let mods = temp.path().join("mods");
        tokio::fs::create_dir(&mods).await.unwrap();

        assert_eq!(
            resolve_instance_path(&mods).await.unwrap(),
            tokio::fs::canonicalize(temp.path()).await.unwrap()
        );
    }

    #[tokio::test]
    async fn accepts_an_instance_folder_containing_mods() {
        let temp = tempfile::tempdir().unwrap();
        tokio::fs::create_dir(temp.path().join("mods"))
            .await
            .unwrap();

        assert_eq!(
            resolve_instance_path(temp.path()).await.unwrap(),
            tokio::fs::canonicalize(temp.path()).await.unwrap()
        );
    }

    #[tokio::test]
    async fn detects_when_a_jar_changes_after_the_preview() {
        let temp = tempfile::tempdir().unwrap();
        let mods = temp.path().join("mods");
        tokio::fs::create_dir(&mods).await.unwrap();
        let jar = mods.join("example.jar");
        tokio::fs::write(&jar, b"before").await.unwrap();
        let before = snapshot_mods_folder(&mods).await.unwrap();

        tokio::fs::write(&jar, b"after").await.unwrap();
        let after = snapshot_mods_folder(&mods).await.unwrap();

        assert_ne!(before[0].sha1, after[0].sha1);
    }

    fn installed(
        provider: ProviderId,
        project_id: &str,
        dependencies: Vec<ProjectRef>,
    ) -> InstalledMod {
        InstalledMod {
            provider,
            project_id: project_id.into(),
            name: project_id.into(),
            version_id: "version".into(),
            version_number: "1".into(),
            filename: format!("{project_id}.jar"),
            installed_at: "now".into(),
            reason: InstallReason::Requested,
            hashes: Vec::<FileHash>::new(),
            enabled: true,
            required_dependencies: Some(dependencies),
        }
    }
}
