use crate::{
    application::profiles::ProfileService,
    domain::*,
    error::{AppError, AppResult},
};
use chrono::Utc;
use futures_util::{StreamExt, stream};
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha512};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct DownloadManager {
    profiles: Arc<ProfileService>,
    client: reqwest::Client,
}

enum NodeOutcome {
    Installed(InstalledMod),
    Skipped,
    Failed(InstallFailure),
}
enum HashState {
    Sha512(Sha512),
    Sha1(Sha1),
    Md5(Md5),
}

impl HashState {
    fn new(algorithm: HashAlgorithm) -> Self {
        match algorithm {
            HashAlgorithm::Sha512 => Self::Sha512(Sha512::new()),
            HashAlgorithm::Sha1 => Self::Sha1(Sha1::new()),
            HashAlgorithm::Md5 => Self::Md5(Md5::new()),
        }
    }
    fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Sha512(hash) => hash.update(bytes),
            Self::Sha1(hash) => hash.update(bytes),
            Self::Md5(hash) => hash.update(bytes),
        }
    }
    fn finish(self) -> String {
        match self {
            Self::Sha512(hash) => hex::encode(hash.finalize()),
            Self::Sha1(hash) => hex::encode(hash.finalize()),
            Self::Md5(hash) => hex::encode(hash.finalize()),
        }
    }
}

impl DownloadManager {
    pub fn new(profiles: Arc<ProfileService>) -> AppResult<Self> {
        Ok(Self {
            profiles,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(180))
                .build()?,
        })
    }

    pub async fn install(
        &self,
        app: AppHandle,
        profile_id: &str,
        plan: ResolutionPlan,
        concurrency: u8,
    ) -> AppResult<InstallResult> {
        if !plan.can_install {
            return Err(AppError::Message(
                "Este plano contém conflitos que precisam ser resolvidos antes da instalação."
                    .into(),
            ));
        }
        let profile = self.profiles.get(profile_id).await?;
        if profile.target != plan.target {
            return Err(AppError::Message(
                "O perfil mudou depois da resolução. Gere um novo plano de instalação.".into(),
            ));
        }
        let install_graph = install_graph(&plan);
        let mods_root = PathBuf::from(&profile.instance_path).join("mods");
        tokio::fs::create_dir_all(&mods_root).await?;
        let previous: HashMap<_, _> = profile
            .mods
            .iter()
            .map(|item| (item.as_ref().key(), item.clone()))
            .collect();
        let already_installed = plan
            .nodes
            .iter()
            .filter(|node| node.already_installed)
            .count();
        let queue: Vec<_> = plan
            .nodes
            .into_iter()
            .filter(|node| !node.already_installed)
            .collect();
        for node in &queue {
            emit_progress(
                &app,
                progress(
                    &plan.id,
                    node,
                    ProgressState::Queued,
                    0,
                    primary_file(&node.version).map_or(0, |file| file.size),
                    None,
                ),
            );
        }
        let plan_id = plan.id.clone();
        let graph_for_downloads = install_graph.clone();
        let outcomes: Vec<_> = stream::iter(queue)
            .map(|node| {
                let app = app.clone();
                let root = mods_root.clone();
                let plan_id = plan_id.clone();
                let required_dependencies = graph_for_downloads
                    .get(&node.key)
                    .map(|(_, dependencies)| dependencies.clone())
                    .unwrap_or_default();
                async move {
                    match self
                        .install_node(&app, &plan_id, &node, &root, required_dependencies)
                        .await
                    {
                        Ok(Some(item)) => NodeOutcome::Installed(item),
                        Ok(None) => NodeOutcome::Skipped,
                        Err(error) => {
                            emit_progress(
                                &app,
                                progress(
                                    &plan_id,
                                    &node,
                                    ProgressState::Failed,
                                    0,
                                    0,
                                    Some(error.to_string()),
                                ),
                            );
                            NodeOutcome::Failed(InstallFailure {
                                project_key: node.key,
                                message: error.to_string(),
                            })
                        }
                    }
                }
            })
            .buffer_unordered(concurrency.clamp(1, 6) as usize)
            .collect()
            .await;
        let mut installed = Vec::new();
        let mut skipped = already_installed;
        let mut failed = Vec::new();
        for outcome in outcomes {
            match outcome {
                NodeOutcome::Installed(item) => installed.push(item),
                NodeOutcome::Skipped => skipped += 1,
                NodeOutcome::Failed(error) => failed.push(error),
            }
        }
        let updated_profile = self
            .profiles
            .record_installed(profile_id, installed.clone(), install_graph)
            .await?;
        for item in &installed {
            if let Some(old) = previous.get(&item.as_ref().key()) {
                if old.filename != item.filename {
                    remove_registered_file(&mods_root, &old.filename).await?;
                }
            }
        }
        Ok(InstallResult {
            profile: updated_profile,
            installed: installed.len(),
            skipped,
            failed,
        })
    }

    async fn install_node(
        &self,
        app: &AppHandle,
        plan_id: &str,
        node: &ResolutionNode,
        root: &Path,
        required_dependencies: Vec<ProjectRef>,
    ) -> AppResult<Option<InstalledMod>> {
        let file = primary_file(&node.version).ok_or_else(|| {
            AppError::Message("O provedor não retornou um arquivo instalável.".into())
        })?;
        let download_url = file.url.as_ref().ok_or_else(|| {
            AppError::Message("O provedor não disponibilizou uma URL de download.".into())
        })?;
        validate_download_url(download_url, node.project.provider)?;
        let filename = safe_filename(&file.filename)?;
        let destination = root.join(&filename);
        let preferred = preferred_hash(&file.hashes);
        if tokio::fs::try_exists(&destination).await? {
            if let Some(expected) = preferred {
                if hash_matches(&destination, expected).await? {
                    emit_progress(
                        app,
                        progress(
                            plan_id,
                            node,
                            ProgressState::Skipped,
                            file.size,
                            file.size,
                            Some("Arquivo já verificado no disco.".into()),
                        ),
                    );
                    return Ok(Some(installed_mod(
                        node,
                        filename,
                        file.hashes.clone(),
                        required_dependencies,
                    )));
                }
            }
        }
        let temporary = root.join(format!("{}.{}.part", filename, &plan_id[..8]));
        emit_progress(
            app,
            progress(
                plan_id,
                node,
                ProgressState::Downloading,
                0,
                file.size,
                None,
            ),
        );
        let transfer: AppResult<u64> = async {
            let response = self.client.get(download_url).send().await?;
            let status = response.status();
            if !status.is_success() {
                return Err(AppError::Message(format!(
                    "O download respondeu com HTTP {status}."
                )));
            }
            let mut output = tokio::fs::File::create(&temporary).await?;
            let mut stream = response.bytes_stream();
            let mut received = 0u64;
            let mut hasher = preferred.map(|hash| HashState::new(hash.algorithm));
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                output.write_all(&chunk).await?;
                received += chunk.len() as u64;
                if let Some(hash) = hasher.as_mut() {
                    hash.update(&chunk);
                }
                emit_progress(
                    app,
                    progress(
                        plan_id,
                        node,
                        ProgressState::Downloading,
                        received,
                        file.size,
                        None,
                    ),
                );
            }
            output.flush().await?;
            drop(output);
            if let (Some(expected), Some(hasher)) = (preferred, hasher) {
                emit_progress(
                    app,
                    progress(
                        plan_id,
                        node,
                        ProgressState::Verifying,
                        received,
                        file.size,
                        None,
                    ),
                );
                if !hasher.finish().eq_ignore_ascii_case(&expected.value) {
                    return Err(AppError::Message(format!(
                        "A verificação {:?} falhou.",
                        expected.algorithm
                    )));
                }
            }
            if tokio::fs::try_exists(&destination).await? {
                tokio::fs::remove_file(&destination).await?;
            }
            tokio::fs::rename(&temporary, &destination).await?;
            Ok(received)
        }
        .await;
        let received = match transfer {
            Ok(received) => received,
            Err(error) => {
                let _ = tokio::fs::remove_file(&temporary).await;
                return Err(error);
            }
        };
        emit_progress(
            app,
            progress(
                plan_id,
                node,
                ProgressState::Installed,
                received,
                file.size,
                None,
            ),
        );
        Ok(Some(installed_mod(
            node,
            filename,
            file.hashes.clone(),
            required_dependencies,
        )))
    }
}

fn installed_mod(
    node: &ResolutionNode,
    filename: String,
    hashes: Vec<FileHash>,
    required_dependencies: Vec<ProjectRef>,
) -> InstalledMod {
    InstalledMod {
        provider: node.project.provider,
        project_id: node.project.project_id.clone(),
        name: node.project.name.clone(),
        version_id: node.version.version_id.clone(),
        version_number: node.version.version_number.clone(),
        filename,
        installed_at: Utc::now().to_rfc3339(),
        reason: node.reason,
        hashes,
        enabled: true,
        required_dependencies: Some(required_dependencies),
    }
}

fn install_graph(plan: &ResolutionPlan) -> HashMap<String, (InstallReason, Vec<ProjectRef>)> {
    let projects: HashMap<_, _> = plan
        .nodes
        .iter()
        .map(|node| {
            (
                node.key.clone(),
                ProjectRef {
                    provider: node.project.provider,
                    project_id: node.project.project_id.clone(),
                },
            )
        })
        .collect();
    let mut graph: HashMap<String, (InstallReason, Vec<ProjectRef>)> = plan
        .nodes
        .iter()
        .map(|node| {
            (
                ProjectRef {
                    provider: node.project.provider,
                    project_id: node.project.project_id.clone(),
                }
                .key(),
                (node.reason, Vec::new()),
            )
        })
        .collect();
    for edge in &plan.edges {
        if !matches!(edge.dependency_type, DependencyType::Required) {
            continue;
        }
        let (Some(owner), Some(project)) = (projects.get(&edge.from), projects.get(&edge.to))
        else {
            continue;
        };
        if let Some((_, dependencies)) = graph.get_mut(&owner.key()) {
            if !dependencies.contains(project) {
                dependencies.push(project.clone());
            }
        }
    }
    graph
}
fn primary_file(version: &ProjectVersion) -> Option<&DownloadFile> {
    version
        .files
        .iter()
        .find(|file| file.primary)
        .or_else(|| version.files.first())
}
fn preferred_hash(hashes: &[FileHash]) -> Option<&FileHash> {
    hashes
        .iter()
        .find(|hash| matches!(hash.algorithm, HashAlgorithm::Sha512))
        .or_else(|| {
            hashes
                .iter()
                .find(|hash| matches!(hash.algorithm, HashAlgorithm::Sha1))
        })
        .or_else(|| hashes.first())
}
async fn hash_matches(path: &Path, expected: &FileHash) -> AppResult<bool> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut buffer = vec![0u8; 64 * 1024];
    let mut hash = HashState::new(expected.algorithm);
    loop {
        let size = file.read(&mut buffer).await?;
        if size == 0 {
            break;
        }
        hash.update(&buffer[..size]);
    }
    Ok(hash.finish().eq_ignore_ascii_case(&expected.value))
}
fn safe_filename(value: &str) -> AppResult<String> {
    let name = Path::new(value)
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            AppError::Message("O nome do arquivo retornado pelo provedor é inválido.".into())
        })?;
    if name != value
        || !["jar", "zip", "mrpack"]
            .iter()
            .any(|extension| name.to_lowercase().ends_with(&format!(".{extension}")))
    {
        return Err(AppError::Message(
            "O arquivo retornado pelo provedor não é um mod válido.".into(),
        ));
    }
    Ok(name
        .chars()
        .map(|character| {
            if "<>:\"/\\|?*".contains(character) || character.is_control() {
                '_'
            } else {
                character
            }
        })
        .collect())
}
fn validate_download_url(value: &str, provider: ProviderId) -> AppResult<()> {
    let url = url::Url::parse(value)?;
    if url.scheme() != "https" {
        return Err(AppError::Message(
            "Download recusado: a origem não usa HTTPS.".into(),
        ));
    }
    let host = url.host_str().unwrap_or_default().to_lowercase();
    let allowed = match provider {
        ProviderId::Modrinth => host == "cdn.modrinth.com" || host.ends_with(".modrinth.com"),
        ProviderId::Curseforge => {
            host.ends_with(".forgecdn.net") || host.ends_with(".curseforge.com")
        }
    };
    if !allowed {
        return Err(AppError::Message(format!(
            "Download recusado: domínio não autorizado ({host})."
        )));
    }
    Ok(())
}
fn progress(
    plan_id: &str,
    node: &ResolutionNode,
    state: ProgressState,
    received_bytes: u64,
    total_bytes: u64,
    message: Option<String>,
) -> InstallProgress {
    InstallProgress {
        plan_id: plan_id.into(),
        project_key: node.key.clone(),
        filename: primary_file(&node.version)
            .map_or_else(|| node.project.name.clone(), |file| file.filename.clone()),
        state,
        received_bytes,
        total_bytes,
        message,
    }
}
fn emit_progress(app: &AppHandle, event: InstallProgress) {
    let _ = app.emit("install:progress", event);
}
async fn remove_registered_file(root: &Path, filename: &str) -> AppResult<()> {
    let Some(name) = Path::new(filename).file_name() else {
        return Ok(());
    };
    let path = root.join(name);
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(key: &str, id: &str, reason: InstallReason) -> ResolutionNode {
        ResolutionNode {
            key: key.into(),
            project: ProjectSummary {
                provider: ProviderId::Modrinth,
                project_id: id.into(),
                slug: id.into(),
                name: id.into(),
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
            },
            version: ProjectVersion {
                provider: ProviderId::Modrinth,
                project_id: id.into(),
                version_id: format!("{id}-version"),
                name: String::new(),
                version_number: "1".into(),
                minecraft_versions: vec!["1.20.1".into()],
                loaders: vec![ModLoader::Forge],
                channel: ReleaseChannel::Release,
                published_at: String::new(),
                downloads: 0,
                files: Vec::new(),
                dependencies: Vec::new(),
            },
            reason,
            parent_key: None,
            already_installed: false,
        }
    }

    #[test]
    fn graph_uses_the_actual_provider_keys_after_a_fallback() {
        let plan = ResolutionPlan {
            id: "plan".into(),
            target: ProfileTarget {
                minecraft_version: "1.20.1".into(),
                loader: ModLoader::Forge,
                release_channels: vec![ReleaseChannel::Release],
            },
            nodes: vec![
                node(
                    "curseforge:original-root",
                    "actual-root",
                    InstallReason::Requested,
                ),
                node(
                    "curseforge:original-library",
                    "actual-library",
                    InstallReason::Required,
                ),
            ],
            edges: vec![ResolutionEdge {
                from: "curseforge:original-root".into(),
                to: "curseforge:original-library".into(),
                dependency_type: DependencyType::Required,
            }],
            issues: Vec::new(),
            optional_dependencies: Vec::new(),
            manual_downloads: Vec::new(),
            downloadable_bytes: 0,
            can_install: true,
        };

        let graph = install_graph(&plan);

        assert!(!graph.contains_key("curseforge:original-root"));
        assert_eq!(
            graph["modrinth:actual-root"].1,
            vec![ProjectRef {
                provider: ProviderId::Modrinth,
                project_id: "actual-library".into(),
            }]
        );
    }
}
