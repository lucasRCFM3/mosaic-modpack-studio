import { useEffect, useState } from 'react';
import type { ModpackProfile, UpdateProfileInput } from '../../shared/domain';
import { Icon } from './Icon';

export function EditProfileDialog({ profile, onClose, onSave }: {
  profile: ModpackProfile;
  onClose: () => void;
  onSave: (id: string, input: UpdateProfileInput) => Promise<void>;
}) {
  const [name, setName] = useState(profile.name);
  const [description, setDescription] = useState(profile.description);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setName(profile.name);
    setDescription(profile.description);
  }, [profile]);

  const save = async () => {
    setBusy(true);
    try {
      await onSave(profile.id, { name: name.trim(), description: description.trim() });
      onClose();
    } finally {
      setBusy(false);
    }
  };

  return <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="modal create-modal" role="dialog" aria-modal="true" aria-labelledby="edit-profile-title">
      <header><div className="modal-symbol"><Icon name="settings"/></div><div><span className="eyebrow">EDITAR MODPACK</span><h2 id="edit-profile-title">Dê uma identidade ao seu pack.</h2><p>Versão e loader permanecem protegidos para não invalidar mods instalados.</p></div><button className="icon-button close" onClick={onClose} aria-label="Fechar"><Icon name="x"/></button></header>
      <div className="form-grid">
        <label className="wide"><span>NOME DO MODPACK</span><input autoFocus value={name} maxLength={80} onChange={(event) => setName(event.target.value)} placeholder="Ex.: Survival com amigos"/></label>
        <label className="wide"><span>DESCRIÇÃO <em>OPCIONAL</em></span><textarea value={description} maxLength={300} rows={4} onChange={(event) => setDescription(event.target.value)} placeholder="Conte o objetivo ou o estilo deste modpack."/><small>{description.length}/300 caracteres</small></label>
      </div>
      <footer><button className="button ghost" onClick={onClose}>Cancelar</button><button className="button primary" disabled={!name.trim() || busy} onClick={() => void save()}>{busy ? <span className="spinner dark"/> : <Icon name="check"/>} Salvar alterações</button></footer>
    </section>
  </div>;
}
