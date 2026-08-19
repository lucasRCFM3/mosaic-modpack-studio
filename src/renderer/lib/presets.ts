import type { ModPreset, ModpackProfile, PresetEntry, ProjectSummary } from '../../shared/domain';
import { projectKey } from '../../shared/domain';

export interface PresetCandidate extends PresetEntry {
  reason: string;
}

export function initialPresetCandidates(profile: ModpackProfile, preset?: ModPreset): PresetCandidate[] {
  const entries = new Map<string, PresetCandidate>();
  preset?.entries.forEach((entry) => entries.set(projectKey(entry.project), { ...entry, reason: 'Predefinição atual' }));
  profile.mods.forEach((mod) => entries.set(projectKey(mod), {
    project: { provider: mod.provider, projectId: mod.projectId },
    name: mod.name,
    reason: mod.reason === 'requested' ? 'Escolhido por você' : mod.reason === 'optional' ? 'Dependência opcional' : 'Dependência automática',
  }));
  return [...entries.values()];
}

export function addCatalogCandidate(current: PresetCandidate[], project: ProjectSummary): PresetCandidate[] {
  const key = projectKey(project);
  if (current.some((entry) => projectKey(entry.project) === key)) return current;
  return [...current, {
    project: { provider: project.provider, projectId: project.projectId },
    name: project.name,
    reason: 'Adicionado pelo catálogo',
  }];
}
