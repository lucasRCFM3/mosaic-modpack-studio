import type { ProjectRef, ResolutionPlan } from '../../shared/domain';

export const refKey = (project: ProjectRef): string => `${project.provider}:${project.projectId}`;

export function selectedOptionalProjects(plan: ResolutionPlan): ProjectRef[] {
  return plan.optionalDependencies.filter(({ selected }) => selected).map(({ project }) => project);
}

export function toggleOptionalProject(plan: ResolutionPlan, target: ProjectRef): ProjectRef[] {
  const selected = selectedOptionalProjects(plan);
  const key = refKey(target);
  return selected.some((project) => refKey(project) === key)
    ? selected.filter((project) => refKey(project) !== key)
    : [...selected, target];
}

export function setAllOptionalProjects(plan: ResolutionPlan, selected: boolean): ProjectRef[] {
  return selected ? plan.optionalDependencies.map(({ project }) => project) : [];
}
