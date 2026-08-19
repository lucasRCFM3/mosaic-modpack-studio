import type { InstallProgress } from '../../shared/domain';
import { Icon } from './Icon';

export function DownloadsPage({ progress }: { progress: Record<string, InstallProgress> }) {
  const events = Object.values(progress);
  return <div className="page downloads-page"><section className="pack-heading"><div><span className="eyebrow"><Icon name="download"/> CENTRAL DE TRANSFERÊNCIAS</span><h1>Downloads</h1><p>Acompanhe download, verificação de integridade e instalação.</p></div></section>
    <section className="installed-panel downloads-panel"><header><div><h2>Sessão atual</h2><p>Até seis arquivos podem ser baixados em paralelo, conforme seus ajustes.</p></div><span className="secure-chip"><Icon name="shield"/> HTTPS + hashes</span></header>
      {events.length ? <div className="download-list">{events.map((event) => { const percent = event.totalBytes ? Math.min(100, event.receivedBytes / event.totalBytes * 100) : 0; return <div className="download-row" key={event.projectKey}><span className={`download-status ${event.state}`}>{event.state === 'installed' || event.state === 'skipped' ? <Icon name="check"/> : event.state === 'failed' ? <Icon name="alert"/> : <Icon name="download"/>}</span><div><strong>{event.filename}</strong><small>{event.message ?? ({ queued: 'Na fila', downloading: `Baixando · ${percent.toFixed(0)}%`, verifying: 'Verificando hash…', installed: 'Instalado com sucesso', skipped: 'Arquivo já estava íntegro', failed: 'Falha' }[event.state])}</small><i><span style={{ width: `${event.state === 'installed' || event.state === 'skipped' ? 100 : percent}%` }}/></i></div></div>; })}</div> : <div className="empty-state compact"><span><Icon name="download"/></span><h3>Nenhum download nesta sessão</h3><p>Quando você instalar um mod, o progresso aparecerá aqui.</p></div>}
    </section>
  </div>;
}
