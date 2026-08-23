use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderId {
    Modrinth,
    Curseforge,
}

impl ProviderId {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Modrinth => "modrinth",
            Self::Curseforge => "curseforge",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    Fabric,
    Forge,
    Neoforge,
    Quilt,
}

impl ModLoader {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::Neoforge => "neoforge",
            Self::Quilt => "quilt",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseChannel {
    Release,
    Beta,
    Alpha,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProjectSide {
    Client,
    Server,
    Both,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyType {
    Required,
    Optional,
    Incompatible,
    Embedded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileTarget {
    pub minecraft_version: String,
    pub loader: ModLoader,
    pub release_channels: Vec<ReleaseChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchFilters {
    pub query: String,
    pub minecraft_version: String,
    pub loader: ModLoader,
    pub release_channels: Vec<ReleaseChannel>,
    pub providers: Vec<ProviderId>,
    pub side: SearchSide,
    pub sort: SearchSort,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchSide {
    Any,
    Client,
    Server,
    Both,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchSort {
    Relevance,
    Downloads,
    Updated,
    Newest,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRef {
    pub provider: ProviderId,
    pub project_id: String,
}

impl ProjectRef {
    pub fn key(&self) -> String {
        format!("{}:{}", self.provider.as_str(), self.project_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub provider: ProviderId,
    pub project_id: String,
    pub slug: String,
    pub name: String,
    pub summary: String,
    pub author: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_url: Option<String>,
    pub website_url: String,
    pub downloads: u64,
    pub updated_at: String,
    pub categories: Vec<String>,
    pub supported_versions: Vec<String>,
    pub supported_loaders: Vec<ModLoader>,
    pub side: ProjectSide,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub featured: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHash {
    pub algorithm: HashAlgorithm,
    pub value: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgorithm {
    Sha1,
    Sha512,
    Md5,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadFile {
    pub filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    pub size: u64,
    pub hashes: Vec<FileHash>,
    pub primary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDependency {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<ProviderId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    #[serde(rename = "type")]
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersion {
    pub provider: ProviderId,
    pub project_id: String,
    pub version_id: String,
    pub name: String,
    pub version_number: String,
    pub minecraft_versions: Vec<String>,
    pub loaders: Vec<ModLoader>,
    pub channel: ReleaseChannel,
    pub published_at: String,
    pub downloads: u64,
    pub files: Vec<DownloadFile>,
    pub dependencies: Vec<ModDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderStatus {
    pub enabled: bool,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSearchResult {
    pub projects: Vec<ProjectSummary>,
    pub total: u64,
    pub warnings: Vec<String>,
    pub providers: HashMap<String, ProviderStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionNode {
    pub key: String,
    pub project: ProjectSummary,
    pub version: ProjectVersion,
    pub reason: InstallReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_key: Option<String>,
    pub already_installed: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallReason {
    Requested,
    Required,
    Optional,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionEdge {
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub dependency_type: DependencyType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResolutionIssueCode {
    NoCompatibleVersion,
    MissingDependencyId,
    IncompatibleMod,
    DependencyCycle,
    DependencyMetadataFallback,
    DistributionRestricted,
    ProviderFallback,
    ProviderError,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IssueSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionIssue {
    pub code: ResolutionIssueCode,
    pub severity: IssueSeverity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<ProjectRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OptionalDependencyChoice {
    pub project: ProjectRef,
    pub name: String,
    pub parent_key: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualDownload {
    pub project: ProjectSummary,
    pub reason: InstallReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolutionPlan {
    pub id: String,
    pub target: ProfileTarget,
    pub nodes: Vec<ResolutionNode>,
    pub edges: Vec<ResolutionEdge>,
    pub issues: Vec<ResolutionIssue>,
    pub optional_dependencies: Vec<OptionalDependencyChoice>,
    pub manual_downloads: Vec<ManualDownload>,
    pub downloadable_bytes: u64,
    pub can_install: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledMod {
    pub provider: ProviderId,
    pub project_id: String,
    pub name: String,
    pub version_id: String,
    pub version_number: String,
    pub filename: String,
    pub installed_at: String,
    pub reason: InstallReason,
    pub hashes: Vec<FileHash>,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_dependencies: Option<Vec<ProjectRef>>,
}

impl InstalledMod {
    pub fn as_ref(&self) -> ProjectRef {
        ProjectRef {
            provider: self.provider,
            project_id: self.project_id.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModpackProfile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub target: ProfileTarget,
    pub instance_path: String,
    pub created_at: String,
    pub updated_at: String,
    pub mods: Vec<InstalledMod>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub curse_forge_configured: bool,
    pub include_optional_dependencies: bool,
    pub download_concurrency: u8,
    pub telemetry: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub plan_id: String,
    pub project_key: String,
    pub filename: String,
    pub state: ProgressState,
    pub received_bytes: u64,
    pub total_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgressState {
    Queued,
    Downloading,
    Verifying,
    Installed,
    Skipped,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallFailure {
    pub project_key: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub profile: ModpackProfile,
    pub installed: usize,
    pub skipped: usize,
    pub failed: Vec<InstallFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoveModResult {
    pub profile: ModpackProfile,
    pub removed: Vec<InstalledMod>,
    pub retained_shared: Vec<InstalledMod>,
    pub unmanaged_mod_files: usize,
    pub dependency_verification_failures: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProfileInput {
    pub name: String,
    pub description: Option<String>,
    pub target: ProfileTarget,
    pub instance_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateProfileInput {
    pub name: String,
    pub description: Option<String>,
    pub instance_path: Option<String>,
    pub mode: DuplicateProfileMode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DuplicateProfileMode {
    Full,
    ModsOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateProfileResult {
    pub profile: ModpackProfile,
    pub copied_files: u64,
    pub copied_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProfileInput {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PresetEntry {
    pub project: ProjectRef,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub entries: Vec<PresetEntry>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavePresetInput {
    pub name: String,
    pub description: Option<String>,
    pub entries: Vec<PresetEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveSettingsInput {
    pub curse_forge_api_key: Option<String>,
    pub clear_curse_forge_api_key: Option<bool>,
    pub include_optional_dependencies: bool,
    pub download_concurrency: u8,
}
