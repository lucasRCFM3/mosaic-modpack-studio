import type { ModPreset, ModpackProfile } from '../../shared/domain';
import { Icon } from './Icon';

export function PresetsPage({ presets, profile, resolvingPresetId, onCreate, onEdit, onApply, onRemove, onDiscover }: {
  presets: ModPreset[];
  profile?: ModpackProfile;
  resolvingPresetId?: string;
  onCreate: () => void;
  onEdit: (preset: ModPreset) => void;
  onApply: (preset: ModPreset) => void;
  onRemove: (preset: ModPreset) => void;
  onDiscover: () => void;
}) {
  return <div className="page presets-page">
    <section className="pack-heading"><div><span className="eyebrow"><Icon name="layers"/> BIBLIOTECA REUTILIZÁVEL</span><h1>Predefinições</h1><p>Escolha mods instalados ou busque qualquer projeto do catálogo para montar sua base.</p></div><div className="pack-actions"><button className="button secondary" onClick={onDiscover}><Icon name="compass"/> Descobrir mods</button><button className="button primary" disabled={!profile} onClick={onCreate}><Icon name="plus"/> Nova predefinição</button></div></section>
    {presets.length ? <div className="preset-grid">{presets.map((preset) => <article className="preset-card" key={preset.id}>
      <header><span><Icon name="layers"/></span><div><h2>{preset.name}</h2><p>{preset.description || 'Sem descrição.'}</p></div><button className="icon-button" title="Editar predefinição" onClick={() => onEdit(preset)}><Icon name="settings"/></button></header>
      <div className="preset-entry-list">{preset.entries.slice(0, 6).map((entry) => <span key={`${entry.project.provider}:${entry.project.projectId}`}><i className={`provider-dot ${entry.project.provider}`}/>{entry.name}</span>)}{preset.entries.length > 6 ? <em>+{preset.entries.length - 6} outros</em> : null}</div>
      <footer><span>{preset.entries.length} mod{preset.entries.length === 1 ? '' : 's'} raiz</span><button className="icon-button danger" title="Excluir predefinição" onClick={() => onRemove(preset)}><Icon name="trash"/></button><button className="button primary" disabled={!profile || resolvingPresetId === preset.id} onClick={() => onApply(preset)}>{resolvingPresetId === preset.id ? <><span className="spinner dark"/> Verificando…</> : <><Icon name="download"/> Aplicar em {profile?.name ?? 'um modpack'}</>}</button></footer>
    </article>)}</div> : <div className="empty-state presets-empty"><span><Icon name="layers"/></span><h3>Nenhuma predefinição ainda</h3><p>Busque mods diretamente no catálogo e monte sua lista sem precisar instalá-los antes.</p><button className="button primary" disabled={!profile} onClick={onCreate}><Icon name="plus"/> Criar primeira predefinição</button></div>}
  </div>;
}
