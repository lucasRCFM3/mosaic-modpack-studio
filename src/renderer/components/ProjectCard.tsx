import type { ProjectSummary } from '../../shared/domain';
import { Icon } from './Icon';

const compact = new Intl.NumberFormat('pt-BR', { notation: 'compact', maximumFractionDigits: 1 });

export function ProjectCard({ project, installed, busy, onAdd, onOpen }: {
  project: ProjectSummary;
  installed: boolean;
  busy: boolean;
  onAdd: () => void;
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
      <button disabled={busy || installed} className={installed ? 'installed' : ''} onClick={onAdd}>
        {busy ? <><span className="spinner"/> Resolvendo</> : installed ? <><Icon name="check"/> Instalado</> : <><Icon name="plus"/> Adicionar</>}
      </button>
    </footer>
  </article>;
}
