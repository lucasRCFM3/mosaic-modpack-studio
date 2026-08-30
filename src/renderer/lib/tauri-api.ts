import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { MosaicApi } from '../../shared/api';
import type { InstallProgress } from '../../shared/domain';

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try { return await invoke<T>(command, args); }
  catch (error) { throw new Error(typeof error === 'string' ? error : error instanceof Error ? error.message : 'O backend Rust recusou a operação.'); }
}

export const mosaicApi: MosaicApi = {
  catalog: {
    search: (filters) => call('catalog_search', { filters }),
    gameVersions: () => call('catalog_game_versions'),
  },
  recommendations: {
    feed: (profileId, scope, seed) => call('recommendations_feed', { profileId, scope, seed }),
    preview: (recommendationId, desiredModCount) => call('recommendations_preview', { recommendationId, desiredModCount }),
  },
  profiles: {
    list: () => call('profiles_list'),
    create: (input) => call('profiles_create', { input }),
    duplicate: (profileId, input) => call('profiles_duplicate', { profileId, input }),
    update: (profileId, input) => call('profiles_update', { profileId, input }),
    rescanPreview: (profileId) => call('profiles_rescan_preview', { profileId }),
    rescanApply: (profileId, planId) => call('profiles_rescan_apply', { profileId, planId }),
    remove: (profileId) => call('profiles_remove', { profileId }),
    chooseFolder: () => call('profiles_choose_folder'),
    export: (profileId) => call('profiles_export', { profileId }),
    exportModList: (profileId) => call('profiles_export_mod_list', { profileId }),
  },
  presets: {
    list: () => call('presets_list'),
    create: (input) => call('presets_create', { input }),
    update: (presetId, input) => call('presets_update', { presetId, input }),
    remove: (presetId) => call('presets_remove', { presetId }),
    resolve: (profileId, presetId, selectedOptional = []) => call('presets_resolve', { profileId, presetId, selectedOptional }),
  },
  mods: {
    organizationPreview: (profileId) => call('mods_organization_preview', { profileId }),
    organizationExport: (profileId, planId, assignments) => call('mods_organization_export', { profileId, planId, assignments }),
    resolve: (profileId, project, selectedOptional = []) => call('mods_resolve', { profileId, project, selectedOptional }),
    resolveMany: (profileId, projects, selectedOptional = []) => call('mods_resolve_many', { profileId, projects, selectedOptional }),
    install: (profileId, planId) => call('mods_install', { profileId, planId }),
    remove: (profileId, project) => call('mods_remove', { profileId, project }),
    openProject: (project) => call('mods_open_project', { project }),
    onProgress: (listener) => {
      let dispose: UnlistenFn | undefined;
      void listen<InstallProgress>('install:progress', (event) => listener(event.payload)).then((unlisten) => { dispose = unlisten; });
      return () => dispose?.();
    },
  },
  settings: {
    get: () => call('settings_get'),
    save: (input) => call('settings_save', { input }),
  },
  app: { version: () => call('app_version') },
};

export function installTauriBridge(): void { window.mosaic = mosaicApi; }
