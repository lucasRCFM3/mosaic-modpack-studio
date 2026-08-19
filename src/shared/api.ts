import type {
  AppSettings,
  CatalogSearchResult,
  CreateProfileInput,
  InstallProgress,
  InstallResult,
  ModPreset,
  ModpackProfile,
  ProjectRef,
  ResolutionPlan,
  SaveSettingsInput,
  SearchFilters,
  SavePresetInput,
  UpdateProfileInput,
} from './domain';

export interface MosaicApi {
  catalog: {
    search(filters: SearchFilters): Promise<CatalogSearchResult>;
    gameVersions(): Promise<string[]>;
  };
  profiles: {
    list(): Promise<ModpackProfile[]>;
    create(input: CreateProfileInput): Promise<ModpackProfile>;
    update(profileId: string, input: UpdateProfileInput): Promise<ModpackProfile>;
    remove(profileId: string): Promise<void>;
    chooseFolder(): Promise<string | undefined>;
    export(profileId: string): Promise<string | undefined>;
  };
  presets: {
    list(): Promise<ModPreset[]>;
    create(input: SavePresetInput): Promise<ModPreset>;
    update(presetId: string, input: SavePresetInput): Promise<ModPreset>;
    remove(presetId: string): Promise<void>;
    resolve(profileId: string, presetId: string, selectedOptional?: ProjectRef[]): Promise<ResolutionPlan>;
  };
  mods: {
    resolve(profileId: string, project: ProjectRef, selectedOptional?: ProjectRef[]): Promise<ResolutionPlan>;
    install(profileId: string, planId: string): Promise<InstallResult>;
    remove(profileId: string, project: ProjectRef): Promise<ModpackProfile>;
    openProject(project: ProjectRef): Promise<void>;
    onProgress(listener: (progress: InstallProgress) => void): () => void;
  };
  settings: {
    get(): Promise<AppSettings>;
    save(input: SaveSettingsInput): Promise<AppSettings>;
  };
  app: {
    version(): Promise<string>;
  };
}

declare global {
  interface Window {
    mosaic: MosaicApi;
  }
}
