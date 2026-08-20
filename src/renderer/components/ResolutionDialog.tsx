import { useEffect, useRef } from 'react';
import type { InstallProgress, ProjectRef, ResolutionPlan } from '../../shared/domain';
import { Icon } from './Icon';

const bytes = (value: number) => value < 1024 * 1024 ? `${(value / 1024).toFixed(0)} KB` : `${(value / 1024 / 1024).toFixed(1)} MB`;

export function ResolutionDialog({ plan, installing, updatingPlan, progress, onClose, onToggleOptional, onSetAllOptional, onInstall }: {
  plan: ResolutionPlan;
  installing: boolean;
  updatingPlan: boolean;
  progress: Record<string, InstallProgress>;
  onClose: () => void;
  onToggleOptional: (project: ProjectRef) => void;
  onSetAllOptional: (selected: boolean) => void;
  onInstall: () => void;
}) {
  const required = plan.nodes.filter(({ reason }) => reason === 'required');
  const completed = Object.values(progress).filter(({ state }) => state === 'installed' || state === 'skipped').length;
  const selectedOptionalCount = plan.optionalDependencies.filter(({ selected }) => selected).length;
  const allOptionalSelected = selectedOptionalCount === plan.optionalDependencies.length;
  const manualCount = plan.manualDownloads.length;
  const availableCount = plan.nodes.length;
  const selectAllRef = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (selectAllRef.current) selectAllRef.current.indeterminate = selectedOptionalCount > 0 && !allOptionalSelected;
  }, [selectedOptionalCount, allOptionalSelected]);
  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !installing && onClose()}>
    <section className="modal resolution-modal" role="dialog" aria-modal="true" aria-labelledby="resolution-title">
      <header><div className="modal-symbol"><Icon name="layers"/></div><div><span className="eyebrow">PLANO DE INSTALAÇÃO</span><h2 id="resolution-title">{plan.canInstall ? manualCount ? availableCount ? 'O restante está pronto para instalar.' : 'Instalação manual necessária.' : 'Tudo compatível. Pode instalar.' : 'Encontramos incompatibilidades.'}</h2><p>{plan.canInstall ? manualCount ? availableCount ? `${manualCount} mod${manualCount === 1 ? '' : 's'} exige${manualCount === 1 ? '' : 'm'} instalação manual e não bloqueia${manualCount === 1 ? '' : 'm'} os demais.` : 'Nenhuma das fontes permitiu o download automático; abra os itens abaixo para baixá-los.' : 'Revise exatamente o que entrará no seu modpack.' : 'Nenhum arquivo será instalado enquanto houver bloqueios.'}</p></div><button className="icon-button close" disabled={installing} onClick={onClose}><Icon name="x"/></button></header>

      <div className="plan-target"><span><Icon name="box"/></span><div><small>DESTINO</small><strong>Minecraft {plan.target.minecraftVersion} · {plan.target.loader}</strong></div><div><small>DOWNLOAD</small><strong>{bytes(plan.downloadableBytes)}</strong></div><div><small>ITENS</small><strong>{plan.nodes.length + manualCount}</strong></div></div>

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
        {plan.manualDownloads.map(({ project, reason }) => <div className="plan-item manual-item" key={`manual:${project.provider}:${project.projectId}`}>
          <span className="plan-icon">{project.iconUrl ? <img src={project.iconUrl} alt=""/> : project.name.slice(0, 1)}</span>
          <div className="plan-copy"><strong>{project.name}</strong><small>{reason === 'required' ? 'Dependência obrigatória' : reason === 'optional' ? 'Dependência opcional' : 'Mod solicitado'} · instalação manual pela {project.provider === 'modrinth' ? 'Modrinth' : 'CurseForge'}</small></div>
          <span className="reason manual">Manual</span>
          <button className="icon-button" onClick={() => void window.mosaic.mods.openProject(project)} title="Abrir página oficial"><Icon name="external"/></button>
        </div>)}
      </div>

      {plan.optionalDependencies.length ? <div className="optional-list">
        <div className="plan-list-title"><span>DEPENDÊNCIAS OPCIONAIS</span><label className="optional-select-all"><input ref={selectAllRef} type="checkbox" checked={allOptionalSelected} disabled={installing || updatingPlan} onChange={() => onSetAllOptional(!allOptionalSelected)}/><span>{allOptionalSelected ? 'Desmarcar todos' : 'Marcar todos'}</span></label></div>
        {plan.optionalDependencies.map((optional) => <label key={`${optional.project.provider}:${optional.project.projectId}`}>
          <input type="checkbox" checked={optional.selected} disabled={installing || updatingPlan} onChange={() => onToggleOptional(optional.project)}/>
          <span><strong>{optional.name}</strong><small>Sugerida por {optional.parentKey}</small></span>
          {updatingPlan ? <span className="spinner"/> : <em>{optional.selected ? 'Será instalada' : 'Não instalar'}</em>}
        </label>)}
      </div> : null}

      <footer><button className="button ghost" disabled={installing || updatingPlan} onClick={onClose}>Cancelar</button><div className="install-summary">{installing ? `${completed} de ${plan.nodes.length} processados` : updatingPlan ? 'Recalculando grafo…' : plan.canInstall ? manualCount ? `${availableCount} ${availableCount === 1 ? 'disponível' : 'disponíveis'} + ${manualCount} ${manualCount === 1 ? 'manual' : 'manuais'}` : 'Pronto para instalar' : 'Resolva os bloqueios acima'}</div><button className="button primary" disabled={!plan.canInstall || installing || updatingPlan} onClick={onInstall}>{installing ? <><span className="spinner dark"/> Instalando…</> : availableCount ? <><Icon name="download"/> Instalar {availableCount === 1 ? 'disponível' : 'disponíveis'}</> : <><Icon name="check"/> Concluir</>}</button></footer>
    </section>
  </div>;
}
