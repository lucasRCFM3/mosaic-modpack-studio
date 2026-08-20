import { describe, expect, it } from 'vitest';
import type { ModpackProfile, ProjectSummary } from '../../shared/domain';
import { addCatalogCandidate, initialPresetCandidates, initialPresetSelection } from './presets';

const profile = {
  mods: [{ provider: 'modrinth', projectId: 'installed', name: 'Installed', reason: 'requested' }],
} as ModpackProfile;
const catalogProject = {
  provider: 'modrinth', projectId: 'not-installed', name: 'Not Installed',
} as ProjectSummary;

describe('editor de predefinições', () => {
  it('permite adicionar um projeto não instalado vindo do catálogo', () => {
    const candidates = addCatalogCandidate(initialPresetCandidates(profile), catalogProject);
    expect(candidates.map(({ project }) => project.projectId)).toEqual(['installed', 'not-installed']);
  });

  it('não duplica o mesmo projeto ao adicionar novamente', () => {
    const once = addCatalogCandidate([], catalogProject);
    expect(addCatalogCandidate(once, catalogProject)).toHaveLength(1);
  });

  it('começa uma nova predefinição sem marcar mods instalados', () => {
    expect(initialPresetCandidates(profile)).toHaveLength(1);
    expect(initialPresetSelection()).toEqual(new Set());
  });
});
