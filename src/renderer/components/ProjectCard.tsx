import type { ProjectSummary } from '../../shared/domain';
import { Icon } from './Icon';

const compact = new Intl.NumberFormat('pt-BR', { notation: 'compact', maximumFractionDigits: 1 });

export function ProjectCard({ project, installed, queued, busy, onAdd, onQueue, onOpen }: {
  project: ProjectSummary;
  installed: boolean;
  queued: boolean;
  busy: boolean;
  onAdd: () => void;
  onQueue: () => void;
  onOpen: () => void;
}) {
  return <article className="project-card">
    <div className="project-head">
      <button className="mod-icon" onClick={onOpen} aria-label={`Abrir ${project.name}`}>
        {project.iconUrl ? <img src={project.iconUrl} alt=""/> : <span>{project.name.slice(0, 2).toUpperCase()}</span>}
      </button>
      <div className="project-title"><span className={`provider ${project.provider}`}>{project.provider === 'modrinth' ? 'MODRINTH' : 'CURSEFORGE'}</span><h3>{project.name}</h3><small>por {project.author}</small></div>
      <button className="icon-button" onClick={onOpen} title="Abrir página oficial"><Icon name="external"/></button>
    </div>
    <p>{project.summary}</p>
    <div className="tags">{project.categories.slice(0, 3).map((category) => <span key={category}>{category}</span>)}</div>
    <footer>
      <span title="Downloads"><Icon name="download"/> {compact.format(project.downloads)}</span>
      <span title="Ambiente">{project.side === 'client' ? <Icon name="monitor"/> : project.side === 'server' ? <Icon name="server"/> : <Icon name="layers"/>} {project.side === 'both' ? 'Cliente + servidor' : project.side === 'client' ? 'Cliente' : project.side === 'server' ? 'Servidor' : 'Universal'}</span>
      {installed ? <button disabled className="installed"><Icon name="check"/> Instalado</button> : <div className="project-actions"><button disabled={busy || queued} className={queued ? 'queued' : 'queue'} onClick={onQueue}>{queued ? <><Icon name="check"/> Na lista</> : <><Icon name="plus"/> À lista</>}</button><button disabled={busy} onClick={onAdd}>{busy ? <><span className="spinner"/> Resolvendo</> : <><Icon name="download"/> Adicionar</>}</button></div>}
    </footer>
  </article>;
}
