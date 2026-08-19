export const PROVIDERS = ['modrinth', 'curseforge'] as const;
export type ProviderId = (typeof PROVIDERS)[number];

export const LOADERS = ['fabric', 'forge', 'neoforge', 'quilt'] as const;
export type ModLoader = (typeof LOADERS)[number];
export type ReleaseChannel = 'release' | 'beta' | 'alpha';
export type ProjectSide = 'client' | 'server' | 'both' | 'unknown';
export type DependencyType = 'required' | 'optional' | 'incompatible' | 'embedded';

export interface ProfileTarget {
  minecraftVersion: string;
  loader: ModLoader;
  releaseChannels: ReleaseChannel[];
}

export interface SearchFilters extends ProfileTarget {
  query: string;
  providers: ProviderId[];
  side: 'any' | ProjectSide;
  sort: 'relevance' | 'downloads' | 'updated' | 'newest';
  limit?: number;
}

export interface ProjectRef {
  provider: ProviderId;
  projectId: string;
}

export interface ProjectSummary extends ProjectRef {
  slug: string;
  name: string;
  summary: string;
  author: string;
  iconUrl?: string;
  websiteUrl: string;
  downloads: number;
  updatedAt: string;
  categories: string[];
  supportedVersions: string[];
  supportedLoaders: ModLoader[];
  side: ProjectSide;
  featured?: boolean;
}

export interface FileHash {
  algorithm: 'sha1' | 'sha512' | 'md5';
  value: string;
}

export interface DownloadFile {
  filename: string;
  url?: string;
  size: number;
  hashes: FileHash[];
  primary: boolean;
}

export interface ModDependency extends Partial<ProjectRef> {
  versionId?: string;
  filename?: string;
  type: DependencyType;
}

export interface ProjectVersion extends ProjectRef {
  versionId: string;
  name: string;
  versionNumber: string;
  minecraftVersions: string[];
  loaders: ModLoader[];
  channel: ReleaseChannel;
  publishedAt: string;
  downloads: number;
  files: DownloadFile[];
  dependencies: ModDependency[];
}

export interface CatalogSearchResult {
  projects: ProjectSummary[];
  total: number;
  warnings: string[];
  providers: Record<ProviderId, { enabled: boolean; ok: boolean; message?: string }>;
}

export interface ResolutionNode {
  key: string;
  project: ProjectSummary;
  version: ProjectVersion;
  reason: 'requested' | 'required' | 'optional';
  parentKey?: string;
  alreadyInstalled: boolean;
}

export interface ResolutionEdge {
  from: string;
  to: string;
  type: DependencyType;
}

export type ResolutionIssueCode =
  | 'NO_COMPATIBLE_VERSION'
  | 'MISSING_DEPENDENCY_ID'
  | 'INCOMPATIBLE_MOD'
  | 'DEPENDENCY_CYCLE'
  | 'DISTRIBUTION_RESTRICTED'
  | 'PROVIDER_ERROR';

export interface ResolutionIssue {
  code: ResolutionIssueCode;
  severity: 'warning' | 'error';
  message: string;
  project?: ProjectRef;
}

export interface ResolutionPlan {
  id: string;
  target: ProfileTarget;
  nodes: ResolutionNode[];
  edges: ResolutionEdge[];
  issues: ResolutionIssue[];
  optionalDependencies: OptionalDependencyChoice[];
  downloadableBytes: number;
  canInstall: boolean;
}

export interface OptionalDependencyChoice {
  project: ProjectRef;
  name: string;
  parentKey: string;
  selected: boolean;
}

export interface InstalledMod extends ProjectRef {
  name: string;
  versionId: string;
  versionNumber: string;
  filename: string;
  installedAt: string;
  reason: ResolutionNode['reason'];
  hashes: FileHash[];
  enabled: boolean;
}

export interface ModpackProfile {
  id: string;
  name: string;
  description: string;
  target: ProfileTarget;
  instancePath: string;
  createdAt: string;
  updatedAt: string;
  mods: InstalledMod[];
}

export interface AppSettings {
  curseForgeConfigured: boolean;
  includeOptionalDependencies: boolean;
  downloadConcurrency: number;
  telemetry: false;
}

export interface InstallProgress {
  planId: string;
  projectKey: string;
  filename: string;
  state: 'queued' | 'downloading' | 'verifying' | 'installed' | 'skipped' | 'failed';
  receivedBytes: number;
  totalBytes: number;
  message?: string;
}

export interface InstallResult {
  profile: ModpackProfile;
  installed: number;
  skipped: number;
  failed: Array<{ projectKey: string; message: string }>;
}

export interface CreateProfileInput {
  name: string;
  description?: string;
  target: ProfileTarget;
  instancePath?: string;
}

export interface UpdateProfileInput {
  name: string;
  description: string;
}

export interface SaveSettingsInput {
  curseForgeApiKey?: string;
  clearCurseForgeApiKey?: boolean;
  includeOptionalDependencies: boolean;
  downloadConcurrency: number;
}

export const projectKey = (ref: ProjectRef): string => `${ref.provider}:${ref.projectId}`;
