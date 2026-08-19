import type { Dispatch, SetStateAction } from 'react';
import type { CatalogSearchResult, ModpackProfile, ProjectSummary, SearchFilters } from '../../shared/domain';
import { Icon } from './Icon';
import { ProjectCard } from './ProjectCard';

interface DiscoverPageProps {
  profile?: ModpackProfile;
  filters: SearchFilters;
  setFilters: Dispatch<SetStateAction<SearchFilters>>;
  versions: string[];
  catalog: CatalogSearchResult;
  searching: boolean;
  resolvingKey?: string;
  installedKeys: Set<string>;
  onAdd: (project: ProjectSummary) => void;
}

export function DiscoverPage({ profile, filters, setFilters, versions, catalog, searching, resolvingKey, installedKeys, onAdd }: DiscoverPageProps) {
  const toggleProvider = (provider: 'modrinth' | 'curseforge') => setFilters((current) => {
    const has = current.providers.includes(provider);
    if (has && current.providers.length === 1) return current;
    return { ...current, providers: has ? current.providers.filter((id) => id !== provider) : [...current.providers, provider] };
  });
  return <div className="page discover-page">
    <section className="hero-row">
      <div><span className="eyebrow"><Icon name="sparkles"/> CATÁLOGO UNIFICADO</span><h1>Encontre o mod certo.<br/><em>Sem quebrar o seu pack.</em></h1><p>Resultados compatíveis com <strong>Minecraft {profile?.target.minecraftVersion}</strong> e <strong>{profile?.target.loader}</strong>. As dependências vêm junto.</p></div>
      <div className="hero-stat"><span><Icon name="shield"/></span><div><strong>Instalação segura</strong><small>Versões e hashes verificados<br/>antes de tocar na instância.</small></div></div>
    </section>

    <section className="search-panel">
      <label className="search-input"><Icon name="search"/><input value={filters.query} onChange={(event) => setFilters((current) => ({ ...current, query: event.target.value }))} placeholder="Buscar mods, bibliotecas, otimização…"/><kbd>⌘ K</kbd></label>
      <div className="filter-row">
        <label>VERSÃO<select value={filters.minecraftVersion} onChange={(event) => setFilters((current) => ({ ...current, minecraftVersion: event.target.value }))}>{versions.map((version) => <option key={version}>{version}</option>)}</select></label>
        <label>LOADER<select value={filters.loader} onChange={(event) => setFilters((current) => ({ ...current, loader: event.target.value as SearchFilters['loader'] }))}><option value="fabric">Fabric</option><option value="forge">Forge</option><option value="neoforge">NeoForge</option><option value="quilt">Quilt</option></select></label>
        <label>AMBIENTE<select value={filters.side} onChange={(event) => setFilters((current) => ({ ...current, side: event.target.value as SearchFilters['side'] }))}><option value="any">Qualquer</option><option value="both">Cliente + servidor</option><option value="client">Somente cliente</option><option value="server">Somente servidor</option></select></label>
        <div className="provider-toggles"><span>FONTES</span><button className={filters.providers.includes('modrinth') ? 'on' : ''} onClick={() => toggleProvider('modrinth')}><i className="provider-dot modrinth"/> Modrinth</button><button className={filters.providers.includes('curseforge') ? 'on' : ''} onClick={() => toggleProvider('curseforge')}><i className="provider-dot curseforge"/> CurseForge</button></div>
      </div>
    </section>

    <section className="result-header">
      <div><h2>{filters.query ? `Resultados para “${filters.query}”` : 'Populares para o seu perfil'}</h2><p>{searching ? 'Consultando catálogos…' : `${catalog.projects.length} resultados compatíveis nesta página`}</p></div>
      <label>ORDENAR POR<select value={filters.sort} onChange={(event) => setFilters((current) => ({ ...current, sort: event.target.value as SearchFilters['sort'] }))}><option value="relevance">Relevância</option><option value="downloads">Mais baixados</option><option value="updated">Atualizados</option><option value="newest">Mais novos</option></select></label>
    </section>

    {!catalog.providers.curseforge.enabled && filters.providers.includes('curseforge') ? <div className="connector-note"><Icon name="alert"/><span><strong>CurseForge aguardando configuração</strong> — adicione uma chave oficial em Ajustes para combinar os dois catálogos.</span></div> : null}
    {catalog.warnings.map((warning) => <div className="connector-note error" key={warning}><Icon name="alert"/><span>{warning}</span></div>)}

    {searching && !catalog.projects.length ? <div className="card-grid">{Array.from({ length: 6 }, (_, index) => <div className="project-card skeleton" key={index}/>)}</div> : catalog.projects.length ? <div className={`card-grid ${searching ? 'refreshing' : ''}`}>
      {catalog.projects.map((project) => {
        const key = `${project.provider}:${project.projectId}`;
        return <ProjectCard key={key} project={project} installed={installedKeys.has(key)} busy={resolvingKey === key} onAdd={() => onAdd(project)} onOpen={() => void window.mosaic.mods.openProject(project)}/>;
      })}
    </div> : <div className="empty-state"><span><Icon name="search"/></span><h3>Nenhum mod compatível</h3><p>Tente outro termo, loader ou versão do Minecraft.</p></div>}
  </div>;
}
