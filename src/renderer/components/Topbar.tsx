import type { ModpackProfile } from '../../shared/domain';
import { Icon } from './Icon';

export function Topbar({ profile, onCreate }: { profile?: ModpackProfile; onCreate: () => void }) {
  return <header className="topbar">
    <div className="active-pack">
      <span className="mini-gem"><Icon name="box"/></span>
      <span><small>MODPACK ATIVO</small><strong>{profile?.name ?? 'Carregando…'}</strong></span>
      {profile ? <span className="target-pill">{profile.target.minecraftVersion} · {profile.target.loader}</span> : null}
    </div>
    <div className="top-actions">
      <span className="verified"><Icon name="shield"/> Downloads verificados</span>
      <button className="button secondary" onClick={onCreate}><Icon name="plus"/> Novo modpack</button>
    </div>
  </header>;
}
