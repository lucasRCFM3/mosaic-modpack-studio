import type { ModpackProfile, ProjectRef } from '../../shared/domain';
import { Icon } from './Icon';

export function ModpackPage({ profile, onDiscover, onRemove, onExport, onDeleteProfile }: {
  profile?: ModpackProfile;
  onDiscover: () => void;
  onRemove: (ref: ProjectRef) => void;
  onExport: () => void;
  onDeleteProfile: () => void;
}) {
  if (!profile) return null;
  return <div className="page modpack-page">
    <section className="pack-heading"><div><span className="eyebrow"><Icon name="box"/> INSTÂNCIA LOCAL</span><h1>{profile.name}</h1><p>{profile.description || 'Seu modpack modular, reproduzível e verificado.'}</p></div><div className="pack-actions"><button className="button secondary" onClick={onExport}><Icon name="export"/> Exportar lockfile</button><button className="button primary" onClick={onDiscover}><Icon name="plus"/> Adicionar mods</button></div></section>
    <div className="pack-metrics"><div><span><Icon name="package"/></span><div><strong>{profile.mods.length}</strong><small>Mods instalados</small></div></div><div><span><Icon name="layers"/></span><div><strong>{profile.mods.filter(({ reason }) => reason !== 'requested').length}</strong><small>Dependências automáticas</small></div></div><div><span><Icon name="shield"/></span><div><strong>{profile.mods.filter(({ hashes }) => hashes.length).length}</strong><small>Arquivos verificáveis</small></div></div><div><span><Icon name="hard-drive"/></span><div><strong>{profile.target.minecraftVersion}</strong><small>{profile.target.loader} · release + beta</small></div></div></div>
    <section className="installed-panel">
      <header><div><h2>Conteúdo instalado</h2><p>Os arquivos desta lista estão registrados no lockfile local.</p></div><span className="path-chip"><Icon name="folder"/>{profile.instancePath}</span></header>
      {profile.mods.length ? <div className="installed-list"><div className="installed-table-head"><span>MOD</span><span>VERSÃO</span><span>ORIGEM</span><span>MOTIVO</span><span/></div>{profile.mods.map((mod) => <div className="installed-row" key={`${mod.provider}:${mod.projectId}`}><span className="installed-name"><i>{mod.name.slice(0, 1)}</i><strong>{mod.name}<small>{mod.filename}</small></strong></span><span>{mod.versionNumber}</span><span><em className={`provider ${mod.provider}`}>{mod.provider === 'modrinth' ? 'Modrinth' : 'CurseForge'}</em></span><span>{mod.reason === 'requested' ? 'Escolhido por você' : mod.reason === 'optional' ? 'Dependência opcional' : 'Dependência obrigatória'}</span><span className="row-actions"><button title="Abrir página oficial" onClick={() => void window.mosaic.mods.openProject(mod)}><Icon name="external"/></button><button className="danger" title="Remover mod" onClick={() => onRemove(mod)}><Icon name="trash"/></button></span></div>)}</div> : <div className="empty-state compact"><span><Icon name="box"/></span><h3>Este modpack ainda está vazio</h3><p>Explore os dois catálogos e adicione seu primeiro mod.</p><button className="button primary" onClick={onDiscover}><Icon name="compass"/> Explorar catálogo</button></div>}
    </section>
    <button className="delete-profile" onClick={onDeleteProfile}><Icon name="trash"/> Remover apenas este perfil (os arquivos serão preservados)</button>
  </div>;
}
