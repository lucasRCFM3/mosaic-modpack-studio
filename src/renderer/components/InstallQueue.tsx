import { useEffect, useState } from 'react';
import type { ProjectRef, ProjectSummary } from '../../shared/domain';
import { filterInstallQueue } from '../lib/install-queue';
import { Icon } from './Icon';

export function InstallQueue({ projects, busy, onRemove, onClear, onInstall }: {
  projects: ProjectSummary[];
  busy: boolean;
  onRemove: (project: ProjectRef) => void;
  onClear: () => void;
  onInstall: () => void;
}) {
  const [query, setQuery] = useState('');
  const filteredProjects = filterInstallQueue(projects, query);
  useEffect(() => {
    if (!projects.length) setQuery('');
  }, [projects.length]);
  return <section className={`install-queue ${projects.length ? 'active' : 'empty'}`}>
    <header>
      <span className="queue-symbol"><Icon name="package"/></span>
      <div><strong>Lista de instalação</strong><small>{projects.length ? `${projects.length} mod${projects.length === 1 ? '' : 's'} aguardando — dependências serão resolvidas juntas` : 'Adicione vários mods e instale tudo em um único plano'}</small></div>
      {projects.length ? <button className="button ghost" disabled={busy} onClick={onClear}>Limpar</button> : null}
      <button className="button primary" disabled={!projects.length || busy} onClick={onInstall}>{busy ? <><span className="spinner dark"/> Verificando…</> : <><Icon name="download"/> Instalar todos</>}</button>
    </header>
    {projects.length ? <div className="install-queue-search"><Icon name="search"/><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Pesquisar na lista…" aria-label="Pesquisar mods na lista de instalação"/><span>{filteredProjects.length} de {projects.length}</span>{query ? <button type="button" className="icon-button" onClick={() => setQuery('')} aria-label="Limpar pesquisa"><Icon name="x"/></button> : null}</div> : null}
    {projects.length ? filteredProjects.length ? <div className="install-queue-items">{filteredProjects.map((project) => <div className="install-queue-item" key={`${project.provider}:${project.projectId}`}>
      <span className="queue-mod-icon">{project.iconUrl ? <img src={project.iconUrl} alt=""/> : project.name.slice(0, 1)}</span>
      <span><strong>{project.name}</strong><small>{project.provider === 'modrinth' ? 'Modrinth' : 'CurseForge'}</small></span>
      <button className="icon-button" disabled={busy} onClick={() => onRemove(project)} aria-label={`Remover ${project.name} da lista`} title="Remover da lista"><Icon name="x"/></button>
    </div>)}</div> : <div className="install-queue-no-results"><Icon name="search"/> Nenhum mod da lista corresponde a “{query}”.</div> : null}
  </section>;
}
