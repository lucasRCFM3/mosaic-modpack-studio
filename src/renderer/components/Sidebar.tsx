import type { ModpackProfile } from '../../shared/domain';
import { Icon, type IconName } from './Icon';

export type ViewId = 'discover' | 'modpack' | 'presets' | 'downloads' | 'settings';

const navigation: Array<{ id: ViewId; label: string; icon: IconName }> = [
  { id: 'discover', label: 'Descobrir', icon: 'compass' },
  { id: 'modpack', label: 'Meu modpack', icon: 'box' },
  { id: 'presets', label: 'Predefinições', icon: 'layers' },
  { id: 'downloads', label: 'Downloads', icon: 'download' },
];

interface SidebarProps {
  view: ViewId;
  onView: (view: ViewId) => void;
  profiles: ModpackProfile[];
  currentProfile?: ModpackProfile;
  onProfile: (id: string) => void;
  onCreateProfile: () => void;
}

export function Sidebar({ view, onView, profiles, currentProfile, onProfile, onCreateProfile }: SidebarProps) {
  return <aside className="sidebar">
    <div className="brand">
      <span className="brand-mark"><span/><span/><span/><span/></span>
      <span><strong>Mosaic</strong><small>MODPACK STUDIO</small></span>
    </div>

    <nav className="primary-nav" aria-label="Navegação principal">
      {navigation.map((item) => <button key={item.id} className={view === item.id ? 'active' : ''} onClick={() => onView(item.id)}>
        <Icon name={item.icon}/><span>{item.label}</span>
        {item.id === 'modpack' && currentProfile?.mods.length ? <em>{currentProfile.mods.length}</em> : null}
      </button>)}
    </nav>

    <div className="side-section-title"><span>SEUS MODPACKS</span><button title="Criar modpack" onClick={onCreateProfile}><Icon name="plus"/></button></div>
    <div className="profile-list">
      {profiles.map((profile, index) => <button key={profile.id} className={profile.id === currentProfile?.id ? 'selected' : ''} onClick={() => onProfile(profile.id)}>
        <span className={`profile-gem gem-${index % 4}`}><Icon name="box"/></span>
        <span className="profile-copy"><strong>{profile.name}</strong><small>{profile.target.minecraftVersion} · {profile.target.loader}</small></span>
        {profile.id === currentProfile?.id ? <Icon name="chevron" className="profile-arrow"/> : null}
      </button>)}
    </div>

    <div className="sidebar-bottom">
      <button className={view === 'settings' ? 'active' : ''} onClick={() => onView('settings')}><Icon name="settings"/><span>Ajustes</span></button>
      <div className="health"><span className="health-dot"/><span><strong>Tudo certo</strong><small>Serviços operacionais</small></span></div>
    </div>
  </aside>;
}
