import { describe, expect, it } from 'vitest';
import type { InstalledMod } from '../../shared/domain';
import { filterInstalledMods } from './installed-mods';

const mods = [
  { name: 'Sodium', filename: 'sodium.jar', versionNumber: '1.0.0', provider: 'modrinth', reason: 'requested' },
  { name: 'Fabric API', filename: 'fabric-api.jar', versionNumber: '2.0.0', provider: 'curseforge', reason: 'required' },
] as InstalledMod[];

describe('filtro de mods instalados', () => {
  it('filtra por nome, versão e provedor', () => {
    expect(filterInstalledMods(mods, 'sodium')).toEqual([mods[0]]);
    expect(filterInstalledMods(mods, '2.0.0')).toEqual([mods[1]]);
    expect(filterInstalledMods(mods, 'curseforge')).toEqual([mods[1]]);
  });

  it('aceita o motivo traduzido e preserva a lista quando vazio', () => {
    expect(filterInstalledMods(mods, 'obrigatória')).toEqual([mods[1]]);
    expect(filterInstalledMods(mods, '')).toBe(mods);
  });
});
