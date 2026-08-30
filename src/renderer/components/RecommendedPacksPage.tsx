import { useEffect, useMemo, useState } from 'react';
import type { ModpackProfile, RecommendationFeed, RecommendationScope } from '../../shared/domain';
import { feedMatches } from '../lib/recommendations';
import { Icon } from './Icon';

const downloads = (value: number) => value >= 1_000_000
  ? `${(value / 1_000_000).toFixed(value >= 10_000_000 ? 0 : 1)} mi`
  : value >= 1_000 ? `${(value / 1_000).toFixed(0)} mil` : String(value);

export function RecommendedPacksPage({
  profile,
  feed,
  history,
  loading,
  loadingId,
  onLoad,
  onSelectFeed,
  onPreview,
}: {
  profile?: ModpackProfile;
  feed?: RecommendationFeed;
  history: RecommendationFeed[];
  loading: boolean;
  loadingId?: string;
  onLoad: (scope: RecommendationScope, force?: boolean) => void;
  onSelectFeed: (id: string) => void;
  onPreview: (id: string) => void;
}) {
  const [scope, setScope] = useState<RecommendationScope>('currentProfile');
  const [query, setQuery] = useState('');
  useEffect(() => { onLoad(scope); }, [scope, profile?.id]);
  const matchingHistory = useMemo(
    () => history.filter((item) => feedMatches(item, scope, profile?.target)),
    [history, scope, profile?.target.minecraftVersion, profile?.target.loader],
  );
  const activeIndex = matchingHistory.findIndex(({ id }) => id === feed?.id);
  const filtered = (feed?.packs ?? []).filter((pack) => {
    const needle = query.trim().toLocaleLowerCase();
    return !needle || `${pack.name} ${pack.summary} ${pack.tags.join(' ')}`.toLocaleLowerCase().includes(needle);
  });
  const official = filtered.filter(({ kind }) => kind === 'official');
  const mosaic = filtered.filter(({ kind }) => kind === 'mosaic');

  const changeScope = (next: RecommendationScope) => {
    setScope(next);
    setQuery('');
  };

  return <section className="page recommendations-page">
    <div className="recommendations-hero">
      <div><span className="eyebrow">CURADORIA INTELIGENTE</span><h1>Encontre a próxima ideia.</h1><p>Explore packs oficiais e coleções modulares do Mosaic. Veja os mods, escolha o que aproveitar e deixe a compatibilidade por nossa conta.</p></div>
      <span className="recommendations-orbit"><Icon name="sparkles"/><i/><i/><i/></span>
    </div>

    <div className="recommendation-controls">
      <div className="scope-tabs" role="tablist" aria-label="Escopo das recomendações">
        <button className={scope === 'currentProfile' ? 'active' : ''} onClick={() => changeScope('currentProfile')}><Icon name="box"/><span>Para seu modpack</span><small>{profile ? `${profile.target.minecraftVersion} · ${profile.target.loader}` : 'Selecione um perfil'}</small></button>
        <button className={scope === 'allVersions' ? 'active' : ''} onClick={() => changeScope('allVersions')}><Icon name="compass"/><span>Todas as versões</span><small>Inspire-se sem filtro</small></button>
      </div>
      <label className="recommendation-search"><Icon name="search"/><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Pesquisar nas sugestões desta rodada…"/></label>
      <div className="recommendation-history">
        <button className="icon-button" title="Rodada mais recente" disabled={activeIndex <= 0} onClick={() => onSelectFeed(matchingHistory[activeIndex - 1]?.id)}><Icon name="chevron"/></button>
        <span><strong>{matchingHistory.length ? activeIndex + 1 : 0}</strong> de {matchingHistory.length}<small>histórico</small></span>
        <button className="icon-button next" title="Rodada anterior" disabled={activeIndex < 0 || activeIndex >= matchingHistory.length - 1} onClick={() => onSelectFeed(matchingHistory[activeIndex + 1]?.id)}><Icon name="chevron"/></button>
        <button className="button secondary" disabled={loading || (scope === 'currentProfile' && !profile)} onClick={() => onLoad(scope, true)}>{loading ? <span className="spinner"/> : <Icon name="refresh"/>} Nova rodada</button>
      </div>
    </div>

    {feed?.warnings.length ? <div className="recommendation-warning"><Icon name="alert"/><span>{feed.warnings.join(' ')}</span></div> : null}
    {loading && !feed ? <div className="empty-state compact"><span className="spinner"/><h3>Buscando boas ideias…</h3><p>Consultando os catálogos e combinando com o seu destino.</p></div> : null}

    {mosaic.length ? <RecommendationSection title="Coleções Mosaic" description="Receitas modulares adaptadas à versão e ao loader escolhidos." packs={mosaic} loadingId={loadingId} onPreview={onPreview}/> : null}
    {official.length ? <RecommendationSection title="Modpacks oficiais" description="Projetos publicados por seus autores na Modrinth e CurseForge." packs={official} loadingId={loadingId} onPreview={onPreview}/> : null}
    {!loading && feed && !filtered.length ? <div className="empty-state compact"><span><Icon name="search"/></span><h3>Nenhuma sugestão com esse termo</h3><p>Limpe a pesquisa ou gere uma nova rodada.</p></div> : null}
  </section>;
}

function RecommendationSection({ title, description, packs, loadingId, onPreview }: {
  title: string;
  description: string;
  packs: RecommendationFeed['packs'];
  loadingId?: string;
  onPreview: (id: string) => void;
}) {
  return <section className="recommendation-section">
    <header><div><h2>{title}</h2><p>{description}</p></div><span>{packs.length} sugest{packs.length === 1 ? 'ão' : 'ões'}</span></header>
    <div className="recommendation-grid">{packs.map((pack) => <article className={`recommendation-card ${pack.kind}`} key={pack.id}>
      <div className="recommendation-cover">{pack.iconUrl ? <img src={pack.iconUrl} alt=""/> : <Icon name={pack.kind === 'mosaic' ? 'sparkles' : 'package'}/>}<span>{pack.kind === 'mosaic' ? 'COLEÇÃO MOSAIC' : pack.provider === 'modrinth' ? 'MODRINTH' : 'CURSEFORGE'}</span></div>
      <div className="recommendation-card-body"><small>{pack.reason}</small><h3>{pack.name}</h3><p>{pack.summary || 'Abra para conhecer os mods desta seleção.'}</p><div className="recommendation-tags">{pack.tags.slice(0, 3).map((tag) => <span key={tag}>{tag}</span>)}</div></div>
      <footer><span>{pack.kind === 'official' ? <><Icon name="download"/>{downloads(pack.downloads)} downloads</> : <><Icon name="shield"/>Curadoria compatível</>}</span><button className="button secondary" disabled={loadingId === pack.id} onClick={() => onPreview(pack.id)}>{loadingId === pack.id ? <span className="spinner"/> : <Icon name="external"/>} Ver mods</button></footer>
    </article>)}</div>
  </section>;
}
