import { describe, expect, it } from 'vitest';
import type { ProjectSummary } from '../../shared/domain';
import { addProjectToQueue, filterInstallQueue, loadInstallQueues, removeProjectFromQueue, removeProjectsFromQueue } from './install-queue';

const project = (id: string) => ({ provider: 'modrinth', projectId: id, name: id }) as ProjectSummary;

describe('fila de instalação', () => {
  it('não adiciona o mesmo projeto duas vezes', () => {
    const first = addProjectToQueue([], project('sodium'));
    expect(addProjectToQueue(first, project('sodium'))).toBe(first);
  });

  it('remove um item ou todo o lote concluído', () => {
    const queue = [project('sodium'), project('iris'), project('modmenu')];
    expect(removeProjectFromQueue(queue, project('iris')).map(({ projectId }) => projectId)).toEqual(['sodium', 'modmenu']);
    expect(removeProjectsFromQueue(queue, [project('sodium'), project('modmenu')]).map(({ projectId }) => projectId)).toEqual(['iris']);
  });

  it('ignora persistência inválida', () => {
    expect(loadInstallQueues('{invalid')).toEqual({});
    expect(loadInstallQueues('[]')).toEqual({});
    expect(loadInstallQueues(JSON.stringify({ profile: [null, {}, project('sodium')] }))).toEqual({ profile: [project('sodium')] });
  });

  it('pesquisa a lista sem diferenciar acentos ou maiúsculas', () => {
    const queue = [
      { ...project('terrain'), name: 'Geração de Terreno', author: 'Lucas', slug: 'terrain', categories: ['worldgen'] },
      { ...project('sodium'), name: 'Sodium', author: 'CaffeineMC', slug: 'sodium', categories: ['optimization'] },
    ];
    expect(filterInstallQueue(queue, 'geracao')).toEqual([queue[0]]);
    expect(filterInstallQueue(queue, 'CAFFEINE')).toEqual([queue[1]]);
  });
});
