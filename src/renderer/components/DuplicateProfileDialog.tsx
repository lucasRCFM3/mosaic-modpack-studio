import { useState } from 'react';
import type { DuplicateProfileInput, DuplicateProfileMode, ModpackProfile } from '../../shared/domain';
import { Icon } from './Icon';

export function DuplicateProfileDialog({ profile, onClose, onDuplicate }: {
  profile: ModpackProfile;
  onClose: () => void;
  onDuplicate: (id: string, input: DuplicateProfileInput) => Promise<void>;
}) {
  const [name, setName] = useState(() => duplicateName(profile.name));
  const [description, setDescription] = useState(profile.description);
  const [path, setPath] = useState('');
  const [mode, setMode] = useState<DuplicateProfileMode>('full');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  const duplicate = async () => {
    setBusy(true);
    setError(undefined);
    try {
      await onDuplicate(profile.id, {
        name: name.trim(),
        description: description.trim() || undefined,
        instancePath: path || undefined,
        mode,
      });
      onClose();
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Não foi possível duplicar o modpack.');
    } finally {
      setBusy(false);
    }
  };

  return <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && !busy && onClose()}>
    <section className="modal duplicate-modal" role="dialog" aria-modal="true" aria-labelledby="duplicate-profile-title">
      <header><div className="modal-symbol"><Icon name="copy"/></div><div><span className="eyebrow">DUPLICAR MODPACK</span><h2 id="duplicate-profile-title">Crie uma cópia independente.</h2><p>O original permanece intacto. Minecraft, loader, mods, versões e relações de dependência serão preservados.</p></div><button className="icon-button close" disabled={busy} onClick={onClose} aria-label="Fechar"><Icon name="x"/></button></header>
      <div className="form-grid duplicate-form">
        <label className="wide"><span>NOME DA CÓPIA</span><input autoFocus value={name} maxLength={80} disabled={busy} onChange={(event) => setName(event.target.value)} placeholder="Ex.: Meu modpack — testes"/></label>
        <label className="wide"><span>DESCRIÇÃO <em>OPCIONAL</em></span><textarea value={description} maxLength={300} rows={3} disabled={busy} onChange={(event) => setDescription(event.target.value)} placeholder="Descrição da nova cópia."/><small>{description.length}/300 caracteres</small></label>
        <fieldset className="duplicate-mode-grid">
          <legend>MODO DA CÓPIA</legend>
          <label className={mode === 'full' ? 'selected' : ''}><input type="radio" name="duplicate-mode" value="full" checked={mode === 'full'} disabled={busy} onChange={() => setMode('full')}/><span><strong><Icon name="copy"/> Cópia completa <em>RECOMENDADO</em></strong><small>Copia toda a instância: mods, configs, saves, resource packs, scripts e demais arquivos.</small></span></label>
          <label className={mode === 'modsOnly' ? 'selected' : ''}><input type="radio" name="duplicate-mode" value="modsOnly" checked={mode === 'modsOnly'} disabled={busy} onChange={() => setMode('modsOnly')}/><span><strong><Icon name="package"/> Cópia limpa</strong><small>Copia apenas os mods registrados. Saves, configs, logs e arquivos manuais ficam de fora.</small></span></label>
        </fieldset>
        <label className="wide"><span>PASTA DA NOVA INSTÂNCIA <em>OPCIONAL</em></span><div className="folder-input"><input value={path} readOnly disabled={busy} placeholder="O Mosaic criará uma pasta separada e segura"/><button disabled={busy} onClick={async () => setPath(await window.mosaic.profiles.chooseFolder() ?? path)}><Icon name="folder"/> Escolher pasta vazia</button></div><small className="field-hint">Se escolher manualmente, a pasta deve estar vazia e fora da instância original.</small></label>
        {error ? <div className="inline-error wide"><Icon name="alert"/> {error}</div> : null}
      </div>
      <footer><button className="button ghost" disabled={busy} onClick={onClose}>Cancelar</button><span className="duplicate-source">Origem: {profile.name}</span><button className="button primary" disabled={!name.trim() || busy} onClick={() => void duplicate()}>{busy ? <><span className="spinner dark"/> Copiando arquivos…</> : <><Icon name="copy"/> Criar cópia</>}</button></footer>
    </section>
  </div>;
}

function duplicateName(name: string): string {
  const suffix = ' (cópia)';
  return `${name.slice(0, 80 - suffix.length).trimEnd()}${suffix}`;
}
