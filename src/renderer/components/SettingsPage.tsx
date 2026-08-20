import { useEffect, useState } from 'react';
import type { AppSettings, SaveSettingsInput } from '../../shared/domain';
import { Icon } from './Icon';

export function SettingsPage({ settings, onSave }: { settings: AppSettings; onSave: (input: SaveSettingsInput) => Promise<void> }) {
  const [apiKey, setApiKey] = useState('');
  const [clearKey, setClearKey] = useState(false);
  const [optional, setOptional] = useState(settings.includeOptionalDependencies);
  const [concurrency, setConcurrency] = useState(settings.downloadConcurrency);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => { setOptional(settings.includeOptionalDependencies); setConcurrency(settings.downloadConcurrency); }, [settings]);
  const save = async () => {
    setBusy(true); setError('');
    try {
      await onSave({ curseForgeApiKey: apiKey || undefined, clearCurseForgeApiKey: clearKey, includeOptionalDependencies: optional, downloadConcurrency: concurrency });
      setApiKey(''); setClearKey(false);
    } catch (caught) { setError(caught instanceof Error ? caught.message : 'Não foi possível salvar.'); }
    finally { setBusy(false); }
  };
  return <div className="page settings-page"><section className="pack-heading"><div><span className="eyebrow"><Icon name="settings"/> PREFERÊNCIAS</span><h1>Ajustes</h1><p>Controle integrações, resolução e comportamento dos downloads.</p></div></section>
    <div className="settings-grid">
      <section className="settings-card provider-settings"><header><span className="settings-icon orange">CF</span><div><h2>CurseForge</h2><p>Catálogo complementar com autenticação oficial.</p></div><span className={`connection-badge ${settings.curseForgeConfigured ? 'connected' : ''}`}>{settings.curseForgeConfigured ? 'Conectado' : 'Não configurado'}</span></header>
        <div className="terms-note"><Icon name="alert"/><p><strong>Antes de conectar</strong>A CurseForge exige uma chave aprovada e seus termos limitam apps concorrentes. Use apenas uma chave que você tem autorização para usar. Alguns autores também bloqueiam downloads externos.</p></div>
        <label className="setting-field"><span>CHAVE DA API</span><input type="password" autoComplete="off" value={apiKey} disabled={clearKey} onChange={(event) => setApiKey(event.target.value)} placeholder={settings.curseForgeConfigured ? '•••••••••••••••• (já armazenada)' : 'Cole sua chave oficial aqui'}/><small>A chave é criptografada pelo cofre do sistema operacional e nunca vai para a interface depois de salva.</small></label>
        {settings.curseForgeConfigured ? <label className="check-row danger-check"><input type="checkbox" checked={clearKey} onChange={(event) => setClearKey(event.target.checked)}/><span>Remover a chave salva</span></label> : null}
        <a className="text-link" href="https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key">Como solicitar uma chave oficial <Icon name="external"/></a>
      </section>
      <section className="settings-card"><header><span className="settings-icon green"><Icon name="layers"/></span><div><h2>Resolução de dependências</h2><p>Como o Mosaic completa seu grafo de mods.</p></div></header>
        <label className="toggle-row"><span><strong>Pré-selecionar dependências opcionais</strong><small>Desativado por padrão. As obrigatórias sempre são resolvidas automaticamente.</small></span><input type="checkbox" checked={optional} onChange={(event) => setOptional(event.target.checked)}/><i/></label>
        <div className="setting-field"><span>DOWNLOADS SIMULTÂNEOS</span><div className="range-row"><input type="range" min="1" max="6" value={concurrency} onChange={(event) => setConcurrency(Number(event.target.value))}/><strong>{concurrency}</strong></div><small>Mais paralelismo é mais rápido, mas usa mais banda e conexões.</small></div>
        <div className="immutable-setting"><Icon name="shield"/><span><strong>Verificação de integridade sempre ativa</strong><small>Não pode ser desativada; arquivos com hash divergente são descartados.</small></span></div>
      </section>
      <section className="settings-card about-card"><header><span className="settings-icon violet"><span className="brand-mark tiny"><span/><span/><span/><span/></span></span><div><h2>Mosaic Modpack Studio</h2><p>Versão 0.3.3 · Rust + Tauri</p></div></header><p>Seus perfis, chaves e arquivos permanecem no seu computador. O Mosaic não possui telemetria e só fala diretamente com as APIs selecionadas.</p><div className="about-points"><span><Icon name="check"/> Segredos fora do renderer</span><span><Icon name="check"/> Downloads HTTPS atômicos</span><span><Icon name="check"/> Lockfile reproduzível</span></div></section>
    </div>
    {error ? <div className="inline-error"><Icon name="alert"/>{error}</div> : null}
    <div className="settings-footer"><span>Alterações entram em vigor na próxima busca ou instalação.</span><button className="button primary" disabled={busy} onClick={() => void save()}>{busy ? <span className="spinner dark"/> : <Icon name="check"/>} Salvar ajustes</button></div>
  </div>;
}
