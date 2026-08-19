import { useState } from 'react';
import type { CreateProfileInput, ModLoader } from '../../shared/domain';
import { Icon } from './Icon';

export function CreateProfileDialog({ versions, onClose, onCreate }: { versions: string[]; onClose: () => void; onCreate: (input: CreateProfileInput) => Promise<void> }) {
  const [name, setName] = useState('Meu modpack');
  const [version, setVersion] = useState(versions[0] ?? '1.21.1');
  const [loader, setLoader] = useState<ModLoader>('fabric');
  const [path, setPath] = useState('');
  const [busy, setBusy] = useState(false);
  const create = async () => {
    setBusy(true);
    try {
      await onCreate({ name, target: { minecraftVersion: version, loader, releaseChannels: ['release', 'beta'] }, instancePath: path || undefined });
      onClose();
    } finally { setBusy(false); }
  };
  return <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="modal create-modal" role="dialog" aria-modal="true">
      <header><div className="modal-symbol"><Icon name="plus"/></div><div><span className="eyebrow">NOVA INSTÂNCIA</span><h2>Comece com uma base limpa.</h2><p>Você poderá exportar e compartilhar o perfil depois.</p></div><button className="icon-button close" onClick={onClose}><Icon name="x"/></button></header>
      <div className="form-grid">
        <label className="wide"><span>NOME DO MODPACK</span><input autoFocus value={name} maxLength={80} onChange={(event) => setName(event.target.value)} placeholder="Ex.: Survival com amigos"/></label>
        <label><span>VERSÃO DO MINECRAFT</span><select value={version} onChange={(event) => setVersion(event.target.value)}>{versions.map((item) => <option key={item}>{item}</option>)}</select></label>
        <label><span>MOD LOADER</span><select value={loader} onChange={(event) => setLoader(event.target.value as ModLoader)}><option value="fabric">Fabric</option><option value="neoforge">NeoForge</option><option value="forge">Forge</option><option value="quilt">Quilt</option></select></label>
        <label className="wide"><span>PASTA DA INSTÂNCIA <em>OPCIONAL</em></span><div className="folder-input"><input value={path} readOnly placeholder="O Mosaic escolherá uma pasta segura"/><button onClick={async () => setPath(await window.mosaic.profiles.chooseFolder() ?? path)}><Icon name="folder"/> Escolher</button></div></label>
      </div>
      <footer><button className="button ghost" onClick={onClose}>Cancelar</button><button className="button primary" disabled={!name.trim() || busy} onClick={() => void create()}>{busy ? <span className="spinner dark"/> : <Icon name="plus"/>} Criar modpack</button></footer>
    </section>
  </div>;
}
