import { useState } from 'react';
import type { ModPreset } from '../shared/domain';
import { CreateProfileDialog } from './components/CreateProfileDialog';
import { DiscoverPage } from './components/DiscoverPage';
import { DuplicateProfileDialog } from './components/DuplicateProfileDialog';
import { EditProfileDialog } from './components/EditProfileDialog';
import { DownloadsPage } from './components/DownloadsPage';
import { Icon } from './components/Icon';
import { ModpackPage } from './components/ModpackPage';
import { ModOrganizationDialog } from './components/ModOrganizationDialog';
import { PresetDialog } from './components/PresetDialog';
import { PresetsPage } from './components/PresetsPage';
import { ResolutionDialog } from './components/ResolutionDialog';
import { RescanProfileDialog } from './components/RescanProfileDialog';
import { SettingsPage } from './components/SettingsPage';
import { Sidebar, type ViewId } from './components/Sidebar';
import { Topbar } from './components/Topbar';
import { useMosaic } from './hooks/useMosaic';

export default function App() {
  const mosaic = useMosaic();
  const [view, setView] = useState<ViewId>('discover');
  const [creating, setCreating] = useState(false);
  const [editing, setEditing] = useState(false);
  const [duplicating, setDuplicating] = useState(false);
  const [presetDraft, setPresetDraft] = useState<ModPreset | 'new'>();
  return <div className="app-shell">
    <Sidebar view={view} onView={setView} profiles={mosaic.profiles} currentProfile={mosaic.currentProfile} onProfile={mosaic.chooseProfile} onCreateProfile={() => setCreating(true)}/>
    <main className="workspace">
      <Topbar profile={mosaic.currentProfile} onCreate={() => setCreating(true)}/>
      <div className="page-scroll">
        {view === 'discover' ? <DiscoverPage profile={mosaic.currentProfile} filters={mosaic.filters} setFilters={mosaic.setFilters} versions={mosaic.gameVersions} catalog={mosaic.catalog} searching={mosaic.searching} resolvingKey={mosaic.resolvingKey} installedKeys={mosaic.installedKeys} queuedProjects={mosaic.queuedProjects} queuedKeys={mosaic.queuedKeys} resolvingBatch={mosaic.resolvingBatch} onAdd={mosaic.resolveProject} onQueue={mosaic.addToInstallQueue} onRemoveFromQueue={mosaic.removeFromInstallQueue} onClearQueue={mosaic.clearInstallQueue} onInstallQueue={mosaic.resolveInstallQueue}/> : null}
        {view === 'modpack' ? <ModpackPage profile={mosaic.currentProfile} classifyingMods={mosaic.classifyingMods} rescanningProfile={mosaic.rescanningProfile} onDiscover={() => setView('discover')} onEdit={() => setEditing(true)} onDuplicate={() => setDuplicating(true)} onRescan={() => void mosaic.previewProfileRescan()} onOrganize={() => void mosaic.previewModOrganization()} onRemove={(ref) => { if (window.confirm(`Remover ${'name' in ref ? ref.name : 'este mod'} da instância?`)) void mosaic.removeMod(ref); }} onExport={() => void mosaic.exportProfile()} onExportModList={() => void mosaic.exportModList()} onDeleteProfile={() => { if (mosaic.currentProfile && window.confirm(`Remover o perfil “${mosaic.currentProfile.name}”? Os arquivos no disco serão preservados.`)) void mosaic.removeProfile(mosaic.currentProfile.id); }}/> : null}
        {view === 'presets' ? <PresetsPage presets={mosaic.presets} profile={mosaic.currentProfile} resolvingPresetId={mosaic.resolvingPresetId} onCreate={() => setPresetDraft('new')} onEdit={setPresetDraft} onApply={(preset) => void mosaic.resolvePreset(preset)} onRemove={(preset) => { if (window.confirm(`Excluir a predefinição “${preset.name}”? Os modpacks não serão alterados.`)) void mosaic.removePreset(preset.id); }} onDiscover={() => setView('discover')}/> : null}
        {view === 'downloads' ? <DownloadsPage progress={mosaic.progress}/> : null}
        {view === 'settings' ? <SettingsPage settings={mosaic.settings} onSave={mosaic.saveSettings}/> : null}
      </div>
    </main>
    {mosaic.plan ? <ResolutionDialog plan={mosaic.plan} installing={mosaic.installing} updatingPlan={mosaic.updatingPlan} progress={mosaic.progress} onClose={() => mosaic.setPlan(undefined)} onToggleOptional={(project) => void mosaic.toggleOptionalDependency(project)} onSetAllOptional={(selected) => void mosaic.setAllOptionalDependencies(selected)} onInstall={() => void mosaic.installPlan()}/> : null}
    {creating ? <CreateProfileDialog versions={mosaic.gameVersions} onClose={() => setCreating(false)} onCreate={mosaic.createProfile}/> : null}
    {editing && mosaic.currentProfile ? <EditProfileDialog profile={mosaic.currentProfile} onClose={() => setEditing(false)} onSave={mosaic.updateProfile}/> : null}
    {duplicating && mosaic.currentProfile ? <DuplicateProfileDialog profile={mosaic.currentProfile} onClose={() => setDuplicating(false)} onDuplicate={mosaic.duplicateProfile}/> : null}
    {mosaic.organizationPlan ? <ModOrganizationDialog plan={mosaic.organizationPlan} busy={mosaic.organizingMods} onClose={() => mosaic.setOrganizationPlan(undefined)} onExport={mosaic.exportModOrganization}/> : null}
    {mosaic.rescanPlan ? <RescanProfileDialog plan={mosaic.rescanPlan} busy={mosaic.applyingRescan} onClose={() => mosaic.setRescanPlan(undefined)} onApply={mosaic.applyProfileRescan}/> : null}
    {presetDraft && mosaic.currentProfile ? <PresetDialog profile={mosaic.currentProfile} preset={presetDraft === 'new' ? undefined : presetDraft} onClose={() => setPresetDraft(undefined)} onSave={mosaic.savePreset}/> : null}
    {mosaic.notice ? <div className={`toast ${mosaic.notice.tone}`}><Icon name={mosaic.notice.tone === 'error' ? 'alert' : mosaic.notice.tone === 'success' ? 'check' : 'sparkles'}/><span>{mosaic.notice.text}</span><button onClick={() => mosaic.setNotice(undefined)}><Icon name="x"/></button></div> : null}
  </div>;
}
