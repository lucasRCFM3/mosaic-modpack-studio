import { useCallback, useEffect, useMemo, useState } from 'react';
import type {
  AppSettings,
  CatalogSearchResult,
  CreateProfileInput,
  InstallProgress,
  ModPreset,
  ModpackProfile,
  ProjectRef,
  ProjectSummary,
  ResolutionPlan,
  SaveSettingsInput,
  SavePresetInput,
  SearchFilters,
  UpdateProfileInput,
} from '../../shared/domain';
import { toggleOptionalProject } from '../lib/dependencies';

const fallbackVersions = ['1.21.8', '1.21.7', '1.21.5', '1.21.4', '1.21.1', '1.20.6', '1.20.4', '1.20.1', '1.19.2', '1.18.2', '1.16.5'];
const emptyResult: CatalogSearchResult = {
  projects: [], total: 0, warnings: [],
  providers: {
    modrinth: { enabled: true, ok: false },
    curseforge: { enabled: false, ok: false },
  },
};

export function useMosaic() {
  const [profiles, setProfiles] = useState<ModpackProfile[]>([]);
  const [presets, setPresets] = useState<ModPreset[]>([]);
  const [currentProfileId, setCurrentProfileId] = useState(() => localStorage.getItem('mosaic:last-profile') ?? '');
  const [settings, setSettings] = useState<AppSettings>({ curseForgeConfigured: false, includeOptionalDependencies: false, downloadConcurrency: 3, telemetry: false });
  const [gameVersions, setGameVersions] = useState(fallbackVersions);
  const [filters, setFilters] = useState<SearchFilters>({
    query: '', minecraftVersion: '1.21.1', loader: 'fabric', releaseChannels: ['release', 'beta'],
    providers: ['modrinth', 'curseforge'], side: 'any', sort: 'relevance', limit: 24,
  });
  const [catalog, setCatalog] = useState<CatalogSearchResult>(emptyResult);
  const [searching, setSearching] = useState(true);
  const [resolvingKey, setResolvingKey] = useState<string>();
  const [resolvingPresetId, setResolvingPresetId] = useState<string>();
  const [plan, setPlan] = useState<ResolutionPlan>();
  const [resolutionSource, setResolutionSource] = useState<{ kind: 'project'; project: ProjectRef } | { kind: 'preset'; presetId: string }>();
  const [installing, setInstalling] = useState(false);
  const [updatingPlan, setUpdatingPlan] = useState(false);
  const [progress, setProgress] = useState<Record<string, InstallProgress>>({});
  const [notice, setNotice] = useState<{ tone: 'success' | 'error' | 'info'; text: string }>();

  const currentProfile = useMemo(
    () => profiles.find(({ id }) => id === currentProfileId) ?? profiles[0],
    [profiles, currentProfileId],
  );

  const refreshProfiles = useCallback(async () => {
    const loaded = await window.mosaic.profiles.list();
    setProfiles(loaded);
    setCurrentProfileId((selected) => loaded.some(({ id }) => id === selected) ? selected : loaded[0]?.id ?? '');
  }, []);

  useEffect(() => {
    void Promise.all([
      refreshProfiles(),
      window.mosaic.presets.list().then(setPresets),
      window.mosaic.settings.get().then(setSettings),
      window.mosaic.catalog.gameVersions().then((versions) => setGameVersions(versions.slice(0, 40))).catch(() => undefined),
    ]).catch((error) => setNotice({ tone: 'error', text: error.message }));
    return window.mosaic.mods.onProgress((event) => setProgress((current) => ({ ...current, [event.projectKey]: event })));
  }, [refreshProfiles]);

  useEffect(() => {
    if (!currentProfile) return;
    localStorage.setItem('mosaic:last-profile', currentProfile.id);
    setFilters((current) => ({ ...current, ...currentProfile.target }));
  }, [currentProfile?.id]);

  useEffect(() => {
    const timeout = window.setTimeout(async () => {
      setSearching(true);
      try {
        setCatalog(await window.mosaic.catalog.search(filters));
      } catch (error) {
        setNotice({ tone: 'error', text: error instanceof Error ? error.message : 'A busca falhou.' });
      } finally {
        setSearching(false);
      }
    }, filters.query ? 380 : 80);
    return () => window.clearTimeout(timeout);
  }, [filters]);

  useEffect(() => {
    if (!notice) return;
    const timeout = window.setTimeout(() => setNotice(undefined), 4500);
    return () => window.clearTimeout(timeout);
  }, [notice]);

  const chooseProfile = (id: string) => setCurrentProfileId(id);

  const createProfile = async (input: CreateProfileInput) => {
    const created = await window.mosaic.profiles.create(input);
    await refreshProfiles();
    setCurrentProfileId(created.id);
    setNotice({ tone: 'success', text: `${created.name} está pronto para receber mods.` });
  };

  const removeProfile = async (id: string) => {
    await window.mosaic.profiles.remove(id);
    await refreshProfiles();
    setNotice({ tone: 'info', text: 'Perfil removido. Os arquivos da instância foram preservados.' });
  };

  const updateProfile = async (id: string, input: UpdateProfileInput) => {
    const updated = await window.mosaic.profiles.update(id, input);
    setProfiles((current) => current.map((profile) => profile.id === updated.id ? updated : profile));
    setNotice({ tone: 'success', text: 'Dados do modpack atualizados.' });
  };

  const resolveProject = async (project: ProjectSummary) => {
    if (!currentProfile) return;
    setResolvingKey(`${project.provider}:${project.projectId}`);
    try {
      let resolved = await window.mosaic.mods.resolve(currentProfile.id, project, []);
      if (settings.includeOptionalDependencies && resolved.optionalDependencies.length) {
        resolved = await window.mosaic.mods.resolve(currentProfile.id, project, resolved.optionalDependencies.map(({ project }) => project));
      }
      setResolutionSource({ kind: 'project', project });
      setPlan(resolved);
    } catch (error) {
      setNotice({ tone: 'error', text: error instanceof Error ? error.message : 'Não foi possível resolver as dependências.' });
    } finally {
      setResolvingKey(undefined);
    }
  };

  const toggleOptionalDependency = async (project: ProjectRef) => {
    if (!currentProfile || !plan || !resolutionSource) return;
    const next = toggleOptionalProject(plan, project);
    setUpdatingPlan(true);
    try {
      setPlan(resolutionSource.kind === 'project'
        ? await window.mosaic.mods.resolve(currentProfile.id, resolutionSource.project, next)
        : await window.mosaic.presets.resolve(currentProfile.id, resolutionSource.presetId, next));
    }
    catch (error) { setNotice({ tone: 'error', text: error instanceof Error ? error.message : 'Não foi possível atualizar as opcionais.' }); }
    finally { setUpdatingPlan(false); }
  };

  const installPlan = async () => {
    if (!currentProfile || !plan) return;
    setInstalling(true);
    setProgress({});
    try {
      const result = await window.mosaic.mods.install(currentProfile.id, plan.id);
      setProfiles((current) => current.map((profile) => profile.id === result.profile.id ? result.profile : profile));
      setPlan(undefined);
      setResolutionSource(undefined);
      const failed = result.failed.length ? ` ${result.failed.length} falharam.` : '';
      setNotice({ tone: result.failed.length ? 'error' : 'success', text: `${result.installed} mods instalados, ${result.skipped} já estavam prontos.${failed}` });
    } catch (error) {
      setNotice({ tone: 'error', text: error instanceof Error ? error.message : 'A instalação falhou.' });
    } finally {
      setInstalling(false);
    }
  };

  const removeMod = async (ref: ProjectRef) => {
    if (!currentProfile) return;
    const updated = await window.mosaic.mods.remove(currentProfile.id, ref);
    setProfiles((current) => current.map((profile) => profile.id === updated.id ? updated : profile));
    setNotice({ tone: 'info', text: 'Mod removido da instância.' });
  };

  const saveSettings = async (input: SaveSettingsInput) => {
    const saved = await window.mosaic.settings.save(input);
    setSettings(saved);
    setNotice({ tone: 'success', text: 'Ajustes salvos com segurança.' });
  };

  const exportProfile = async () => {
    if (!currentProfile) return;
    const path = await window.mosaic.profiles.export(currentProfile.id);
    if (path) setNotice({ tone: 'success', text: 'Lockfile exportado com sucesso.' });
  };

  const savePreset = async (input: SavePresetInput, presetId?: string) => {
    const saved = presetId
      ? await window.mosaic.presets.update(presetId, input)
      : await window.mosaic.presets.create(input);
    setPresets((current) => [saved, ...current.filter((preset) => preset.id !== saved.id)]);
    setNotice({ tone: 'success', text: presetId ? 'Predefinição atualizada.' : 'Predefinição criada.' });
  };

  const removePreset = async (presetId: string) => {
    await window.mosaic.presets.remove(presetId);
    setPresets((current) => current.filter((preset) => preset.id !== presetId));
    setNotice({ tone: 'info', text: 'Predefinição removida. Nenhum mod instalado foi alterado.' });
  };

  const resolvePreset = async (preset: ModPreset) => {
    if (!currentProfile) return;
    setResolvingPresetId(preset.id);
    try {
      let resolved = await window.mosaic.presets.resolve(currentProfile.id, preset.id, []);
      if (settings.includeOptionalDependencies && resolved.optionalDependencies.length) {
        resolved = await window.mosaic.presets.resolve(currentProfile.id, preset.id, resolved.optionalDependencies.map(({ project }) => project));
      }
      setResolutionSource({ kind: 'preset', presetId: preset.id });
      setPlan(resolved);
    } catch (error) {
      setNotice({ tone: 'error', text: error instanceof Error ? error.message : 'Não foi possível verificar a predefinição.' });
    } finally { setResolvingPresetId(undefined); }
  };

  const installedKeys = useMemo(() => new Set(currentProfile?.mods.map((mod) => `${mod.provider}:${mod.projectId}`) ?? []), [currentProfile]);

  return {
    profiles, currentProfile, presets, settings, gameVersions, filters, setFilters, catalog, searching,
    resolvingKey, resolvingPresetId, plan, setPlan, installing, updatingPlan, progress, notice, setNotice, installedKeys,
    chooseProfile, createProfile, updateProfile, removeProfile, savePreset, removePreset, resolvePreset, resolveProject, toggleOptionalDependency, installPlan, removeMod, saveSettings, exportProfile,
  };
}
