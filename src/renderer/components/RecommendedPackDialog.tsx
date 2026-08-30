import { useEffect, useMemo, useRef, useState } from 'react';
import type { ModpackProfile, RecommendedPackDetails } from '../../shared/domain';
import { projectKey } from '../lib/install-queue';
import { Icon } from './Icon';

export function RecommendedPackDialog({ details, profile, busy, onClose, onAdd, onCreate }: {
  details: RecommendedPackDetails;
  profile?: ModpackProfile;
  busy: boolean;
  onClose: () => void;
  onAdd: (details: RecommendedPackDetails) => void;
  onCreate: (details: RecommendedPackDetails) => void;
}) {
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState(() => new Set(details.projects.map(projectKey)));
  const allRef = useRef<HTMLInputElement>(null);
  const installed = useMemo(() => new Set(profile?.mods.map(projectKey) ?? []), [profile]);
  const sameTarget = Boolean(profile
    && profile.target.minecraftVersion === details.target.minecraftVersion
    && profile.target.loader === details.target.loader);
  const filtered = details.projects.filter((project) => {
    const needle = query.trim().toLocaleLowerCase();
    return !needle || `${project.name} ${project.author} ${project.summary}`.toLocaleLowerCase().includes(needle);
  });
  useEffect(() => {
    if (allRef.current) allRef.current.indeterminate = selected.size > 0 && selected.size < details.projects.length;
  }, [selected.size, details.projects.length]);
  const selectedDetails = { ...details, projects: details.projects.filter((project) => selected.has(projectKey(project))) };
  const toggle = (key: string) => setSelected((current) => {
    const next = new Set(current);
    if (next.has(key)) next.delete(key); else next.add(key);
    return next;
  });
  const toggleAll = () => setSelected(selected.size === details.projects.length ? new Set() : new Set(details.projects.map(projectKey)));

  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && !busy && onClose()}>
    <section className="modal recommendation-modal" role="dialog" aria-modal="true" aria-labelledby="recommended-pack-title">
      <header><div className="modal-symbol"><Icon name={details.pack.kind === 'mosaic' ? 'sparkles' : 'package'}/></div><div><span className="eyebrow">{details.pack.kind === 'mosaic' ? 'COLEÇÃO MOSAIC' : 'MODPACK OFICIAL'}</span><h2 id="recommended-pack-title">{details.pack.name}</h2><p>{details.pack.summary}</p></div><button className="icon-button close" disabled={busy} onClick={onClose}><Icon name="x"/></button></header>

      <div className="recommended-target"><div><small>DESTINO DA SELEÇÃO</small><strong>Minecraft {details.target.minecraftVersion} · {details.target.loader}</strong></div><div><small>IDENTIFICADOS</small><strong>{details.projects.length} mods</strong></div><div><small>SELECIONADOS</small><strong>{selected.size}</strong></div></div>
      {details.pack.kind === 'official' ? <div className="recommended-import-note"><Icon name="shield"/><span><strong>Importação modular, não uma cópia exata.</strong> O Mosaic aproveita os mods e recalcula dependências. {details.hasOverrides ? 'Este pack também contém configs/overrides, que ainda não serão copiados.' : 'Nenhum override foi detectado no arquivo analisado.'}</span></div> : null}
      {details.warnings.length ? <details className="recommended-warnings"><summary><Icon name="alert"/>{details.warnings.length} observação{details.warnings.length === 1 ? '' : 'ões'} da análise</summary><div>{details.warnings.map((warning) => <p key={warning}>{warning}</p>)}</div></details> : null}

      <div className="recommended-toolbar"><label><Icon name="search"/><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Pesquisar dentro do pack…"/></label><label className="recommended-select-all"><input ref={allRef} type="checkbox" checked={selected.size === details.projects.length} disabled={busy} onChange={toggleAll}/><span>{selected.size === details.projects.length ? 'Desmarcar todos' : 'Marcar todos'}</span></label></div>
      <div className="recommended-mod-list">{filtered.map((project) => {
        const key = projectKey(project);
        return <label key={key} className={selected.has(key) ? 'selected' : ''}><input type="checkbox" checked={selected.has(key)} disabled={busy} onChange={() => toggle(key)}/><span className="recommended-mod-icon">{project.iconUrl ? <img src={project.iconUrl} alt=""/> : project.name.slice(0, 1)}</span><span><strong>{project.name}</strong><small>{project.author} · {project.provider === 'modrinth' ? 'Modrinth' : 'CurseForge'}</small></span>{installed.has(key) ? <em><Icon name="check"/>Já instalado</em> : null}</label>;
      })}{!filtered.length ? <div className="organization-empty"><Icon name="search"/>Nenhum mod encontrado.</div> : null}</div>

      <footer><button className="button ghost" disabled={busy} onClick={onClose}>Cancelar</button><span>{sameTarget ? `Adicionar ao “${profile?.name}”` : 'O perfil atual usa outro destino'}</span><button className="button secondary" disabled={busy || !selected.size || !sameTarget} title={sameTarget ? '' : 'Crie um perfil separado para não misturar versões'} onClick={() => onAdd(selectedDetails)}><Icon name="plus"/> Adicionar ao atual</button><button className="button primary" disabled={busy || !selected.size} onClick={() => onCreate(selectedDetails)}>{busy ? <><span className="spinner dark"/> Preparando…</> : <><Icon name="copy"/> Criar separado</>}</button></footer>
    </section>
  </div>;
}
