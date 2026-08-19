import { useEffect, useState } from 'react';
import type { ModPreset, ModpackProfile, PresetEntry, ProjectSummary, SavePresetInput } from '../../shared/domain';
import { projectKey } from '../../shared/domain';
import { addCatalogCandidate, initialPresetCandidates } from '../lib/presets';
import { Icon } from './Icon';

export function PresetDialog({ profile, preset, onClose, onSave }: {
  profile: ModpackProfile;
  preset?: ModPreset;
  onClose: () => void;
  onSave: (input: SavePresetInput, presetId?: string) => Promise<void>;
}) {
  const [candidates, setCandidates] = useState(() => initialPresetCandidates(profile, preset));
  const [name, setName] = useState(preset?.name ?? 'Mods essenciais');
  const [description, setDescription] = useState(preset?.description ?? 'Minha base padrão para novos modpacks.');
  const [selected, setSelected] = useState(() => new Set(
    preset?.entries.map((entry) => projectKey(entry.project))
      ?? profile.mods.filter((mod) => mod.reason === 'requested').map(projectKey),
  ));
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<ProjectSummary[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchWarning, setSearchWarning] = useState<string>();
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const normalized = query.trim();
    if (normalized.length < 2) {
      setResults([]);
      setSearchWarning(undefined);
      setSearching(false);
      return;
    }
    let cancelled = false;
    const timeout = window.setTimeout(async () => {
      setSearching(true);
      try {
        const result = await window.mosaic.catalog.search({
          query: normalized,
          ...profile.target,
          providers: ['modrinth', 'curseforge'],
          side: 'any',
          sort: 'relevance',
          limit: 12,
        });
        if (!cancelled) {
          setResults(result.projects);
          setSearchWarning(result.warnings[0]);
        }
      } catch (error) {
        if (!cancelled) setSearchWarning(error instanceof Error ? error.message : 'A busca falhou.');
      } finally {
        if (!cancelled) setSearching(false);
      }
    }, 350);
    return () => {
      cancelled = true;
      window.clearTimeout(timeout);
    };
  }, [query, profile.target]);

  const toggle = (entry: PresetEntry) => setSelected((current) => {
    const next = new Set(current);
    const key = projectKey(entry.project);
    if (next.has(key)) next.delete(key); else next.add(key);
    return next;
  });
  const addProject = (project: ProjectSummary) => {
    setCandidates((current) => addCatalogCandidate(current, project));
    setSelected((current) => new Set(current).add(projectKey(project)));
  };
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
        <div className="preset-catalog-search">
          <div className="preset-picker-title"><div><strong>Buscar no catálogo</strong><small>Resultados para Minecraft {profile.target.minecraftVersion} · {profile.target.loader}</small></div>{searching ? <span className="spinner"/> : null}</div>
          <label className="search-input"><Icon name="search"/><input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Busque qualquer mod, mesmo não instalado…"/></label>
          {searchWarning ? <div className="preset-search-warning"><Icon name="alert"/>{searchWarning}</div> : null}
          {query.trim().length >= 2 && !searching ? <div className="preset-search-results">
            {results.length ? results.map((project) => {
              const added = selected.has(projectKey(project));
              return <div key={projectKey(project)}><span className="plan-icon">{project.iconUrl ? <img src={project.iconUrl} alt=""/> : project.name.slice(0, 1)}</span><span><strong>{project.name}</strong><small>{project.provider} · por {project.author}</small></span><button className={`button ${added ? 'secondary' : 'primary'}`} disabled={added} onClick={() => addProject(project)}>{added ? <><Icon name="check"/> Adicionado</> : <><Icon name="plus"/> Adicionar</>}</button></div>;
            }) : <p>Nenhum resultado compatível para este perfil.</p>}
          </div> : null}
        </div>
        <div className="preset-picker-title"><div><strong>Mods da predefinição</strong><small>Selecione projetos, não dependências acidentais.</small></div><span>{selected.size} selecionado{selected.size === 1 ? '' : 's'}</span></div>
        <div className="preset-picker">
          {candidates.length ? candidates.map((entry) => <label key={projectKey(entry.project)}>
            <input type="checkbox" checked={selected.has(projectKey(entry.project))} onChange={() => toggle(entry)}/>
            <span className="plan-icon">{entry.name.slice(0, 1)}</span>
            <span><strong>{entry.name}</strong><small>{entry.project.provider} · {entry.reason}</small></span>
          </label>) : <div className="preset-picker-empty"><Icon name="search"/><span><strong>Busque um mod acima</strong><small>Você não precisa instalá-lo antes de criar a predefinição.</small></span></div>}
        </div>
      </div>
      <footer><button className="button ghost" onClick={onClose}>Cancelar</button><button className="button primary" disabled={!name.trim() || selected.size === 0 || busy} onClick={() => void save()}>{busy ? <span className="spinner dark"/> : <Icon name="check"/>} {preset ? 'Salvar alterações' : 'Criar predefinição'}</button></footer>
    </section>
  </div>;
}
