import { describe, expect, it } from 'vitest';
import type { InstalledMod } from '../../shared/domain';
import { buildDependencyUsage, filterInstalledMods, formatDependencyUsers } from './installed-mods';

const mods = [
  { projectId: 'sodium', name: 'Sodium', filename: 'sodium.jar', versionNumber: '1.0.0', provider: 'modrinth', reason: 'requested', requiredDependencies: [{ provider: 'curseforge', projectId: 'fabric-api' }] },
  { projectId: 'fabric-api', name: 'Fabric API', filename: 'fabric-api.jar', versionNumber: '2.0.0', provider: 'curseforge', reason: 'required' },
] as InstalledMod[];

describe('filtro de mods instalados', () => {
  it('filtra por nome, versão e provedor', () => {
    expect(filterInstalledMods(mods, 'sodium.jar')).toEqual([mods[0]]);
    expect(filterInstalledMods(mods, '2.0.0')).toEqual([mods[1]]);
    expect(filterInstalledMods(mods, 'curseforge')).toEqual([mods[1]]);
  });

  it('aceita o motivo traduzido e preserva a lista quando vazio', () => {
    expect(filterInstalledMods(mods, 'obrigatória')).toEqual([mods[1]]);
    expect(filterInstalledMods(mods, 'dependencia')).toEqual([mods[1]]);
    expect(filterInstalledMods(mods, '')).toBe(mods);
  });

  it('encontra uma dependência pelo nome do mod que a utiliza', () => {
    expect(filterInstalledMods(mods, 'sodium')).toEqual(mods);
  });
});

describe('uso de dependências instaladas', () => {
  it('mostra os mods raiz mesmo em uma cadeia transitiva', () => {
    const chain = [
      { ...mods[0], requiredDependencies: [{ provider: 'modrinth', projectId: 'middle' }] },
      { ...mods[1], provider: 'modrinth', projectId: 'middle', name: 'Biblioteca intermediária', requiredDependencies: [{ provider: 'modrinth', projectId: 'core' }] },
      { ...mods[1], provider: 'modrinth', projectId: 'core', name: 'Núcleo' },
    ] as InstalledMod[];

    const usage = buildDependencyUsage(chain).get('modrinth:core');

    expect(usage?.directDependents.map(({ name }) => name)).toEqual(['Biblioteca intermediária']);
    expect(usage?.rootDependents.map(({ name }) => name)).toEqual(['Sodium']);
  });

  it('resume listas grandes sem esconder a quantidade restante', () => {
    expect(formatDependencyUsers([
      { name: 'A' }, { name: 'B' }, { name: 'C' }, { name: 'D' },
    ] as InstalledMod[], 2)).toBe('A, B e mais 2');
  });
});
