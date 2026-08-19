import type {
  AppSettings,
  CatalogSearchResult,
  CreateProfileInput,
  InstallProgress,
  InstallResult,
  ModpackProfile,
  ProjectRef,
  ResolutionPlan,
  SaveSettingsInput,
  SearchFilters,
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
