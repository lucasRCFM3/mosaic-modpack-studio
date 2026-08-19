import { useMemo, useState } from 'react';
import type { ModPreset, ModpackProfile, PresetEntry, SavePresetInput } from '../../shared/domain';
import { projectKey } from '../../shared/domain';
import { Icon } from './Icon';

export function PresetDialog({ profile, preset, onClose, onSave }: {
  profile: ModpackProfile;
  preset?: ModPreset;
  onClose: () => void;
  onSave: (input: SavePresetInput, presetId?: string) => Promise<void>;
}) {
  const candidates = useMemo(() => {
    const entries = new Map<string, PresetEntry & { reason: string }>();
    preset?.entries.forEach((entry) => entries.set(projectKey(entry.project), { ...entry, reason: 'Predefinição atual' }));
    profile.mods.forEach((mod) => entries.set(projectKey(mod), {
      project: { provider: mod.provider, projectId: mod.projectId },
      name: mod.name,
      reason: mod.reason === 'requested' ? 'Escolhido por você' : mod.reason === 'optional' ? 'Dependência opcional' : 'Dependência automática',
    }));
    return [...entries.values()];
  }, [preset, profile.mods]);
  const [name, setName] = useState(preset?.name ?? 'Mods essenciais');
  const [description, setDescription] = useState(preset?.description ?? 'Minha base padrão para novos modpacks.');
  const [selected, setSelected] = useState(() => new Set(
    preset?.entries.map((entry) => projectKey(entry.project))
      ?? profile.mods.filter((mod) => mod.reason === 'requested').map(projectKey),
  ));
  const [busy, setBusy] = useState(false);

  const toggle = (entry: PresetEntry) => setSelected((current) => {
    const next = new Set(current);
    const key = projectKey(entry.project);
    if (next.has(key)) next.delete(key); else next.add(key);
    return next;
  });
  const save = async () => {
    setBusy(true);
    try {
      await onSave({
        name: name.trim(),
        description: description.trim(),
        entries: candidates.filter((entry) => selected.has(projectKey(entry.project))).map(({ project, name: entryName }) => ({ project, name: entryName })),
      }, preset?.id);
      onClose();
    } finally { setBusy(false); }
  };

  return <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="modal preset-modal" role="dialog" aria-modal="true" aria-labelledby="preset-dialog-title">
      <header><div className="modal-symbol"><Icon name="layers"/></div><div><span className="eyebrow">{preset ? 'EDITAR PREDEFINIÇÃO' : 'NOVA PREDEFINIÇÃO'}</span><h2 id="preset-dialog-title">Uma base, qualquer modpack.</h2><p>As versões compatíveis serão escolhidas somente quando você aplicar esta lista.</p></div><button className="icon-button close" onClick={onClose} aria-label="Fechar"><Icon name="x"/></button></header>
      <div className="preset-form">
        <div className="form-grid preset-fields">
          <label><span>NOME</span><input autoFocus value={name} maxLength={80} onChange={(event) => setName(event.target.value)}/></label>
          <label><span>DESCRIÇÃO <em>OPCIONAL</em></span><input value={description} maxLength={300} onChange={(event) => setDescription(event.target.value)}/></label>
        </div>
        <div className="preset-picker-title"><div><strong>Mods da predefinição</strong><small>Selecione projetos, não dependências acidentais.</small></div><span>{selected.size} selecionado{selected.size === 1 ? '' : 's'}</span></div>
        <div className="preset-picker">
          {candidates.length ? candidates.map((entry) => <label key={projectKey(entry.project)}>
            <input type="checkbox" checked={selected.has(projectKey(entry.project))} onChange={() => toggle(entry)}/>
            <span className="plan-icon">{entry.name.slice(0, 1)}</span>
            <span><strong>{entry.name}</strong><small>{entry.project.provider} · {entry.reason}</small></span>
          </label>) : <div className="preset-picker-empty"><Icon name="box"/><span><strong>Este modpack ainda não tem mods</strong><small>Instale sua base uma vez e volte para salvá-la como predefinição.</small></span></div>}
        </div>
      </div>
      <footer><button className="button ghost" onClick={onClose}>Cancelar</button><button className="button primary" disabled={!name.trim() || selected.size === 0 || busy} onClick={() => void save()}>{busy ? <span className="spinner dark"/> : <Icon name="check"/>} {preset ? 'Salvar alterações' : 'Criar predefinição'}</button></footer>
    </section>
  </div>;
}
