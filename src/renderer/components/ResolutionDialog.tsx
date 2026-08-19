import type { InstallProgress, ProjectRef, ResolutionPlan } from '../../shared/domain';
import { Icon } from './Icon';

const bytes = (value: number) => value < 1024 * 1024 ? `${(value / 1024).toFixed(0)} KB` : `${(value / 1024 / 1024).toFixed(1)} MB`;

export function ResolutionDialog({ plan, installing, updatingPlan, progress, onClose, onToggleOptional, onInstall }: {
  plan: ResolutionPlan;
  installing: boolean;
  updatingPlan: boolean;
  progress: Record<string, InstallProgress>;
  onClose: () => void;
  onToggleOptional: (project: ProjectRef) => void;
  onInstall: () => void;
}) {
  const required = plan.nodes.filter(({ reason }) => reason === 'required');
  const completed = Object.values(progress).filter(({ state }) => state === 'installed' || state === 'skipped').length;
  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !installing && onClose()}>
    <section className="modal resolution-modal" role="dialog" aria-modal="true" aria-labelledby="resolution-title">
      <header><div className="modal-symbol"><Icon name="layers"/></div><div><span className="eyebrow">PLANO DE INSTALAÇÃO</span><h2 id="resolution-title">{plan.canInstall ? 'Tudo compatível. Pode instalar.' : 'Encontramos incompatibilidades.'}</h2><p>{plan.canInstall ? 'Revise exatamente o que entrará no seu modpack.' : 'Nenhum arquivo será instalado enquanto houver bloqueios.'}</p></div><button className="icon-button close" disabled={installing} onClick={onClose}><Icon name="x"/></button></header>

      <div className="plan-target"><span><Icon name="box"/></span><div><small>DESTINO</small><strong>Minecraft {plan.target.minecraftVersion} · {plan.target.loader}</strong></div><div><small>DOWNLOAD</small><strong>{bytes(plan.downloadableBytes)}</strong></div><div><small>ARQUIVOS</small><strong>{plan.nodes.length}</strong></div></div>

      {plan.issues.length ? <div className="issue-list">{plan.issues.map((issue, index) => <div key={`${issue.code}-${index}`} className={issue.severity}><Icon name="alert"/><span><strong>{issue.severity === 'error' ? 'Ação necessária' : 'Atenção'}</strong>{issue.message}</span>{issue.project ? <button onClick={() => void window.mosaic.mods.openProject(issue.project!)}><Icon name="external"/></button> : null}</div>)}</div> : <div className="safe-banner"><Icon name="shield"/><span><strong>Nenhum conflito encontrado</strong>O grafo de dependências é compatível com o perfil atual.</span></div>}

      <div className="plan-list">
        <div className="plan-list-title"><span>ARQUIVOS SELECIONADOS</span><span>{required.length} dependência{required.length === 1 ? '' : 's'} automática{required.length === 1 ? '' : 's'}</span></div>
        {plan.nodes.map((node) => {
          const event = progress[node.key];
          const percent = event?.totalBytes ? Math.min(100, event.receivedBytes / event.totalBytes * 100) : 0;
          return <div className="plan-item" key={node.key}>
            <span className="plan-icon">{node.project.iconUrl ? <img src={node.project.iconUrl} alt=""/> : node.project.name.slice(0, 1)}</span>
            <div className="plan-copy"><strong>{node.project.name}</strong><small>{node.version.versionNumber} · {node.version.channel}</small>{event && installing ? <i><span style={{ width: `${percent}%` }}/></i> : null}</div>
            <span className={`reason ${node.reason}`}>{node.alreadyInstalled ? 'Já instalado' : node.reason === 'requested' ? 'Solicitado' : node.reason === 'optional' ? 'Opcional' : 'Dependência'}</span>
            {event ? <span className={`progress-state ${event.state}`}>{event.state === 'installed' || event.state === 'skipped' ? <Icon name="check"/> : event.state === 'failed' ? <Icon name="alert"/> : <span className="spinner"/>}</span> : null}
          </div>;
        })}
      </div>

      {plan.optionalDependencies.length ? <div className="optional-list">
        <div className="plan-list-title"><span>DEPENDÊNCIAS OPCIONAIS</span><span>Desmarcadas por padrão</span></div>
        {plan.optionalDependencies.map((optional) => <label key={`${optional.project.provider}:${optional.project.projectId}`}>
          <input type="checkbox" checked={optional.selected} disabled={installing || updatingPlan} onChange={() => onToggleOptional(optional.project)}/>
          <span><strong>{optional.name}</strong><small>Sugerida por {optional.parentKey}</small></span>
          {updatingPlan ? <span className="spinner"/> : <em>{optional.selected ? 'Será instalada' : 'Não instalar'}</em>}
        </label>)}
      </div> : null}

      <footer><button className="button ghost" disabled={installing || updatingPlan} onClick={onClose}>Cancelar</button><div className="install-summary">{installing ? `${completed} de ${plan.nodes.length} processados` : updatingPlan ? 'Recalculando grafo…' : plan.canInstall ? 'Pronto para instalar' : 'Resolva os bloqueios acima'}</div><button className="button primary" disabled={!plan.canInstall || installing || updatingPlan} onClick={onInstall}>{installing ? <><span className="spinner dark"/> Instalando…</> : <><Icon name="download"/> Instalar {plan.nodes.length} arquivo{plan.nodes.length === 1 ? '' : 's'}</>}</button></footer>
    </section>
  </div>;
}
