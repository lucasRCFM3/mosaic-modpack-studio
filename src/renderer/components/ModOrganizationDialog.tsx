import { useMemo, useState } from 'react';
import { projectKey, type ModOrganizationAssignment, type ModOrganizationPlan, type ProjectSide } from '../../shared/domain';
import { Icon } from './Icon';

const sideLabels: Record<ProjectSide, string> = {
  client: 'Cliente',
  server: 'Servidor',
  both: 'Cliente e Servidor',
  unknown: 'Não classificado',
};

export function ModOrganizationDialog({ plan, busy, onClose, onExport }: {
  plan: ModOrganizationPlan;
  busy: boolean;
  onClose: () => void;
  onExport: (planId: string, assignments: ModOrganizationAssignment[]) => Promise<boolean>;
}) {
  const [assignments, setAssignments] = useState<Record<string, ProjectSide>>(() => Object.fromEntries(plan.items.map((item) => [projectKey(item.project), item.side])));
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<'all' | ProjectSide>('all');
  const [error, setError] = useState<string>();
  const visible = useMemo(() => {
    const normalized = normalize(query);
    return plan.items.filter((item) => {
      const side = assignments[projectKey(item.project)] ?? item.side;
      return (filter === 'all' || side === filter)
        && (!normalized || normalize(`${item.name} ${item.filename}`).includes(normalized));
    });
  }, [assignments, filter, plan.items, query]);
  const counts = plan.items.reduce<Record<ProjectSide, number>>((current, item) => {
    current[assignments[projectKey(item.project)] ?? item.side] += 1;
    return current;
  }, { client: 0, server: 0, both: 0, unknown: 0 });

  const generate = async () => {
    setError(undefined);
    try {
      await onExport(plan.id, plan.items.map((item) => ({
        project: item.project,
        side: assignments[projectKey(item.project)] ?? item.side,
      })));
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Não foi possível gerar as pastas.');
    }
  };

  return <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && !busy && onClose()}>
    <section className="modal organization-modal" role="dialog" aria-modal="true" aria-labelledby="organization-title">
      <header><div className="modal-symbol"><Icon name="layers"/></div><div><span className="eyebrow">SEPARAR MODS POR AMBIENTE</span><h2 id="organization-title">Revise antes de gerar as pastas.</h2><p>Classificação obtida dos catálogos. Ajuste manualmente qualquer mod quando necessário.</p></div><button className="icon-button close" disabled={busy} onClick={onClose} aria-label="Fechar"><Icon name="x"/></button></header>
      <div className="organization-summary">
        {(['client', 'server', 'both', 'unknown'] as ProjectSide[]).map((side) => <button key={side} className={`${filter === side ? 'selected ' : ''}${side}`} onClick={() => setFilter(filter === side ? 'all' : side)}><strong>{counts[side]}</strong><span>{sideLabels[side]}</span></button>)}
      </div>
      <div className="organization-safety"><Icon name="shield"/><span><strong>A instância ativa não será alterada.</strong> O Mosaic cria cópias organizadas fora de <code>mods/</code>, evitando que Fabric, Forge, NeoForge ou Quilt encontrem JARs duplicados.</span></div>
      <div className="organization-toolbar"><label><Icon name="search"/><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Pesquisar na classificação…"/>{query ? <button className="icon-button" onClick={() => setQuery('')} aria-label="Limpar pesquisa"><Icon name="x"/></button> : null}</label><span>{visible.length} de {plan.items.length}</span></div>
      <div className="organization-list">
        {visible.length ? visible.map((item) => {
          const key = projectKey(item.project);
          return <div className="organization-row" key={key}><span className="organization-mod"><i>{item.name.slice(0, 1)}</i><span><strong>{item.name}</strong><small>{item.filename}</small></span></span><span className={`classification-source ${item.source}`}>{item.source === 'provider' ? 'Provedor' : item.source === 'crossProvider' ? 'Fonte cruzada' : 'Revisar'}</span><select value={assignments[key] ?? item.side} disabled={busy} onChange={(event) => setAssignments((current) => ({ ...current, [key]: event.target.value as ProjectSide }))}><option value="client">Cliente</option><option value="server">Servidor</option><option value="both">Cliente e Servidor</option><option value="unknown">Não classificado</option></select></div>;
        }) : <div className="organization-empty"><Icon name="search"/> Nenhum mod corresponde ao filtro atual.</div>}
      </div>
      {error ? <div className="inline-error organization-error"><Icon name="alert"/> {error}</div> : null}
      <footer><button className="button ghost" disabled={busy} onClick={onClose}>Cancelar</button><span>{counts.unknown ? `${counts.unknown} item${counts.unknown === 1 ? '' : 's'} ainda precisa${counts.unknown === 1 ? '' : 'm'} de revisão` : 'Todos os mods estão classificados'}</span><button className="button primary" disabled={busy} onClick={() => void generate()}>{busy ? <><span className="spinner dark"/> Copiando…</> : <><Icon name="folder"/> Escolher destino e gerar</>}</button></footer>
    </section>
  </div>;
}

function normalize(value: string): string {
  return value.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLocaleLowerCase('pt-BR').trim();
}
