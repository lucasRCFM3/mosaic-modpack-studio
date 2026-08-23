import { projectKey, type InstalledMod } from '../../shared/domain';

const reasonLabels: Record<InstalledMod['reason'], string> = {
  requested: 'escolhido solicitado manual você',
  required: 'dependência obrigatória automático',
  optional: 'dependência opcional',
};

export function filterInstalledMods(mods: InstalledMod[], query: string): InstalledMod[] {
  const normalized = normalize(query);
  if (!normalized) return mods;
  const usage = buildDependencyUsage(mods);
  return mods.filter((mod) => [
    mod.name,
    mod.filename,
    mod.versionNumber,
    mod.provider,
    reasonLabels[mod.reason],
    ...(usage.get(projectKey(mod))?.rootDependents.map(({ name }) => name) ?? []),
  ].some((value) => normalize(value).includes(normalized)));
}

export interface DependencyUsage {
  directDependents: InstalledMod[];
  rootDependents: InstalledMod[];
}

export function buildDependencyUsage(mods: InstalledMod[]): Map<string, DependencyUsage> {
  const installedByKey = new Map(mods.map((mod) => [projectKey(mod), mod]));
  const directByDependency = new Map<string, InstalledMod[]>();

  for (const owner of mods) {
    for (const dependency of owner.requiredDependencies ?? []) {
      const dependencyKey = projectKey(dependency);
      if (!installedByKey.has(dependencyKey)) continue;
      const dependents = directByDependency.get(dependencyKey) ?? [];
      if (!dependents.some((mod) => projectKey(mod) === projectKey(owner))) dependents.push(owner);
      directByDependency.set(dependencyKey, dependents);
    }
  }

  return new Map(mods.map((mod) => {
    const directDependents = directByDependency.get(projectKey(mod)) ?? [];
    const ancestors = collectAncestors(mod, directByDependency);
    const rootDependents = ancestors.filter(({ reason }) => reason !== 'required');
    return [projectKey(mod), {
      directDependents: sortByName(directDependents),
      rootDependents: sortByName(rootDependents.length ? rootDependents : directDependents),
    }];
  }));
}

export function formatDependencyUsers(mods: InstalledMod[], limit = 3): string {
  const names = [...new Set(mods.map(({ name }) => name))];
  if (names.length <= limit) return names.join(', ');
  return `${names.slice(0, limit).join(', ')} e mais ${names.length - limit}`;
}

function collectAncestors(mod: InstalledMod, directByDependency: Map<string, InstalledMod[]>): InstalledMod[] {
  const reached = new Map<string, InstalledMod>();
  const pending = [...(directByDependency.get(projectKey(mod)) ?? [])];
  while (pending.length) {
    const dependent = pending.pop()!;
    const key = projectKey(dependent);
    if (key === projectKey(mod) || reached.has(key)) continue;
    reached.set(key, dependent);
    pending.push(...(directByDependency.get(key) ?? []));
  }
  return [...reached.values()];
}

function sortByName(mods: InstalledMod[]): InstalledMod[] {
  return [...mods].sort((left, right) => left.name.localeCompare(right.name, 'pt-BR'));
}

function normalize(value: string): string {
  return value.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLocaleLowerCase('pt-BR').trim();
}
