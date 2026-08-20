import { projectKey, type ProjectRef, type ProjectSummary } from '../../shared/domain';

export { projectKey };

function isStoredProject(value: unknown): value is ProjectSummary {
  if (!value || typeof value !== 'object') return false;
  const project = value as Partial<ProjectSummary>;
  return (project.provider === 'modrinth' || project.provider === 'curseforge')
    && typeof project.projectId === 'string'
    && project.projectId.length > 0
    && typeof project.name === 'string'
    && project.name.length > 0;
}

export function addProjectToQueue(queue: ProjectSummary[], project: ProjectSummary): ProjectSummary[] {
  const key = projectKey(project);
  return queue.some((item) => projectKey(item) === key) ? queue : [...queue, project];
}

export function removeProjectFromQueue(queue: ProjectSummary[], project: ProjectRef): ProjectSummary[] {
  const key = projectKey(project);
  return queue.filter((item) => projectKey(item) !== key);
}

export function removeProjectsFromQueue(queue: ProjectSummary[], projects: ProjectRef[]): ProjectSummary[] {
  const keys = new Set(projects.map(projectKey));
  return queue.filter((item) => !keys.has(projectKey(item)));
}

export function loadInstallQueues(raw: string | null): Record<string, ProjectSummary[]> {
  if (!raw) return {};
  try {
    const parsed: unknown = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object' || Array.isArray(parsed)) return {};
    return Object.fromEntries(
      Object.entries(parsed)
        .filter(([, value]) => Array.isArray(value))
        .map(([profileId, value]) => [profileId, (value as unknown[]).filter(isStoredProject).slice(0, 100)])
        .filter(([, projects]) => (projects as ProjectSummary[]).length > 0),
    ) as Record<string, ProjectSummary[]>;
  } catch {
    return {};
  }
}
