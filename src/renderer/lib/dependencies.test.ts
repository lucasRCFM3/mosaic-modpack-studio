import { describe, expect, it } from 'vitest';
import type { ResolutionPlan } from '../../shared/domain';
import { selectedOptionalProjects, setAllOptionalProjects, toggleOptionalProject } from './dependencies';

const optional = { provider: 'modrinth' as const, projectId: 'optional-library' };
const basePlan = {
  optionalDependencies: [{ project: optional, name: 'Optional Library', parentKey: 'modrinth:root', selected: false }],
} as ResolutionPlan;

describe('seleção de dependências opcionais', () => {
  it('começa sem opcionais selecionadas', () => expect(selectedOptionalProjects(basePlan)).toEqual([]));
  it('adiciona e remove uma opcional explicitamente', () => {
    expect(toggleOptionalProject(basePlan, optional)).toEqual([optional]);
    const selected = { ...basePlan, optionalDependencies: [{ ...basePlan.optionalDependencies[0], selected: true }] };
    expect(toggleOptionalProject(selected, optional)).toEqual([]);
  });
  it('marca e desmarca todas em uma única seleção', () => {
    expect(setAllOptionalProjects(basePlan, true)).toEqual([optional]);
    expect(setAllOptionalProjects(basePlan, false)).toEqual([]);
  });
});
