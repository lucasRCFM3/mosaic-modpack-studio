import { useEffect, useMemo, useState } from 'react';
import { projectKey, type InstalledMod, type ModpackProfile, type ProjectRef } from '../../shared/domain';
import { buildDependencyUsage, filterInstalledMods, formatDependencyUsers } from '../lib/installed-mods';
import { Icon } from './Icon';

export function ModpackPage({ profile, onDiscover, onEdit, onDuplicate, onRemove, onExport, onExportModList, onDeleteProfile }: {
  profile?: ModpackProfile;
  onDiscover: () => void;
  onEdit: () => void;
  onDuplicate: () => void;
  onRemove: (ref: ProjectRef) => void;
  onExport: () => void;
  onExportModList: () => void;
  onDeleteProfile: () => void;
}) {
  if (!profile) return null;
  return <ModpackContent profile={profile} onDiscover={onDiscover} onEdit={onEdit} onDuplicate={onDuplicate} onRemove={onRemove} onExport={onExport} onExportModList={onExportModList} onDeleteProfile={onDeleteProfile}/>;
}

function ModpackContent({ profile, onDiscover, onEdit, onDuplicate, onRemove, onExport, onExportModList, onDeleteProfile }: {
  profile: ModpackProfile;
  onDiscover: () => void;
  onEdit: () => void;
  onDuplicate: () => void;
  onRemove: (ref: ProjectRef) => void;
  onExport: () => void;
  onExportModList: () => void;
  onDeleteProfile: () => void;
}) {
  const [query, setQuery] = useState('');
  useEffect(() => setQuery(''), [profile.id]);
  const visibleMods = filterInstalledMods(profile.mods, query);
  const dependencyUsage = useMemo(() => buildDependencyUsage(profile.mods), [profile.mods]);
  return <div className="page modpack-page">
    <section className="pack-heading"><div><span className="eyebrow"><Icon name="box"/> INSTÂNCIA LOCAL</span><h1>{profile.name}</h1><p>{profile.description || 'Seu modpack modular, reproduzível e verificado.'}</p></div><div className="pack-actions"><button className="button secondary" onClick={onEdit}><Icon name="settings"/> Editar</button><button className="button secondary" onClick={onDuplicate}><Icon name="copy"/> Duplicar</button><button className="button secondary" onClick={onExportModList}><Icon name="export"/> Gerar TXT</button><button className="button secondary" onClick={onExport}><Icon name="export"/> Exportar lockfile</button><button className="button primary" onClick={onDiscover}><Icon name="plus"/> Adicionar mods</button></div></section>
    <div className="pack-metrics"><div><span><Icon name="package"/></span><div><strong>{profile.mods.length}</strong><small>Mods instalados</small></div></div><div><span><Icon name="layers"/></span><div><strong>{profile.mods.filter(({ reason }) => reason !== 'requested').length}</strong><small>Dependências automáticas</small></div></div><div><span><Icon name="shield"/></span><div><strong>{profile.mods.filter(({ hashes }) => hashes.length).length}</strong><small>Arquivos verificáveis</small></div></div><div><span><Icon name="hard-drive"/></span><div><strong>{profile.target.minecraftVersion}</strong><small>{profile.target.loader} · release + beta</small></div></div></div>
    <section className="installed-panel">
      <header><div><h2>Conteúdo instalado</h2><p>Ao remover um mod, dependências obrigatórias órfãs também são removidas; as compartilhadas são preservadas.</p></div><span className="path-chip"><Icon name="folder"/>{profile.instancePath}</span></header>
      {profile.mods.length ? <><div className="installed-toolbar"><label><Icon name="search"/><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Buscar por nome, versão, origem, motivo ou mod relacionado…"/>{query ? <button className="icon-button" onClick={() => setQuery('')} aria-label="Limpar busca"><Icon name="x"/></button> : null}</label><span>{visibleMods.length} de {profile.mods.length} mod{profile.mods.length === 1 ? '' : 's'}</span></div>{visibleMods.length ? <div className="installed-list"><div className="installed-table-head"><span>MOD</span><span>VERSÃO</span><span>ORIGEM</span><span>MOTIVO</span><span/></div>{visibleMods.map((mod) => <div className="installed-row" key={`${mod.provider}:${mod.projectId}`}><span className="installed-name"><i>{mod.name.slice(0, 1)}</i><strong>{mod.name}<small>{mod.filename}</small></strong></span><span>{mod.versionNumber}</span><span><em className={`provider ${mod.provider}`}>{mod.provider === 'modrinth' ? 'Modrinth' : 'CurseForge'}</em></span><InstalledReason mod={mod} dependents={dependencyUsage.get(projectKey(mod))?.rootDependents ?? []}/><span className="row-actions"><button title="Abrir página oficial" onClick={() => void window.mosaic.mods.openProject(mod)}><Icon name="external"/></button><button className="danger" title="Remover mod" onClick={() => onRemove(mod)}><Icon name="trash"/></button></span></div>)}</div> : <div className="empty-state compact installed-no-results"><span><Icon name="search"/></span><h3>Nenhum mod encontrado</h3><p>Tente outro nome, versão, origem, motivo ou mod relacionado.</p><button className="button secondary" onClick={() => setQuery('')}>Limpar busca</button></div>}</> : <div className="empty-state compact"><span><Icon name="box"/></span><h3>Este modpack ainda está vazio</h3><p>Explore os dois catálogos e adicione seu primeiro mod.</p><button className="button primary" onClick={onDiscover}><Icon name="compass"/> Explorar catálogo</button></div>}
    </section>
    <button className="delete-profile" onClick={onDeleteProfile}><Icon name="trash"/> Remover apenas este perfil (os arquivos serão preservados)</button>
  </div>;
}

function InstalledReason({ mod, dependents }: { mod: InstalledMod; dependents: InstalledMod[] }) {
  const label = mod.reason === 'requested' ? 'Escolhido por você' : mod.reason === 'optional' ? 'Dependência opcional' : 'Dependência obrigatória';
  if (!dependents.length && mod.reason === 'requested') return <span>{label}</span>;

  const names = formatDependencyUsers(dependents);
  const relationship = mod.reason === 'requested' ? 'Também usada por' : 'Usada por';
  const detail = dependents.length ? `${relationship}: ${names}` : 'Origem não registrada';
  return <span className="installed-reason" title={dependents.length ? `${relationship}: ${dependents.map(({ name }) => name).join(', ')}` : detail}>
    <span>{label}</span>
    <small>{detail}</small>
  </span>;
}
