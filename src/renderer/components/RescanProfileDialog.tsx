import { useState } from 'react';
import type { ModLoader, RescanProfilePlan } from '../../shared/domain';
import { Icon } from './Icon';

const loaderLabels: Record<ModLoader, string> = {
  fabric: 'Fabric',
  forge: 'Forge',
  neoforge: 'NeoForge',
  quilt: 'Quilt',
};

export function RescanProfileDialog({ plan, busy, onClose, onApply }: {
  plan: RescanProfilePlan;
  busy: boolean;
  onClose: () => void;
  onApply: (planId: string) => Promise<boolean>;
}) {
  const [error, setError] = useState<string>();
  const targetChanged = plan.previousTarget.minecraftVersion !== plan.detectedTarget.minecraftVersion
    || plan.previousTarget.loader !== plan.detectedTarget.loader;

  const apply = async () => {
    setError(undefined);
    try {
      await onApply(plan.id);
    } catch (caught) {
      setError(caught instanceof Error ? caught.message : 'Não foi possível substituir o modpack.');
    }
  };

  return <div className="modal-backdrop" onMouseDown={(event) => event.target === event.currentTarget && !busy && onClose()}>
    <section className="modal rescan-modal" role="dialog" aria-modal="true" aria-labelledby="rescan-profile-title">
      <header><div className="modal-symbol"><Icon name="refresh"/></div><div><span className="eyebrow">IMPORTAR E REINDEXAR INSTÂNCIA</span><h2 id="rescan-profile-title">Revise a detecção antes de substituir.</h2><p>O Mosaic trocará o caminho deste perfil e reconstruirá o índice. Nenhum arquivo da pasta escolhida será apagado.</p></div><button className="icon-button close" disabled={busy} onClick={onClose} aria-label="Fechar"><Icon name="x"/></button></header>
      <div className="rescan-content">
        <div className="rescan-path"><Icon name="folder"/><span><small>PASTA DETECTADA</small><strong>{plan.instancePath}</strong></span></div>
        <div className="rescan-targets">
          <div><small>ALVO ATUAL</small><strong>Minecraft {plan.previousTarget.minecraftVersion}</strong><span>{loaderLabels[plan.previousTarget.loader]}</span></div>
          <Icon name="chevron"/>
          <div className={targetChanged ? 'changed' : ''}><small>ALVO DETECTADO</small><strong>Minecraft {plan.detectedTarget.minecraftVersion}</strong><span>{loaderLabels[plan.detectedTarget.loader]}{targetChanged ? ' · será atualizado' : ' · confirmado'}</span></div>
        </div>
        <div className="rescan-source"><Icon name="shield"/><span><small>COMO FOI DETECTADO</small><strong>{plan.detectionSource}</strong></span></div>
        <div className="rescan-metrics">
          <div><strong>{plan.scannedFiles}</strong><span>JARs encontrados</span></div>
          <div><strong>{plan.recognized}</strong><span>Reconhecidos</span></div>
          <div className={plan.localOnly ? 'attention' : ''}><strong>{plan.localOnly}</strong><span>Arquivos locais</span></div>
          <div className={plan.warnings.length ? 'attention' : ''}><strong>{plan.warnings.length}</strong><span>Avisos</span></div>
        </div>
        <div className="rescan-safety"><Icon name="shield"/><span><strong>O perfil atual só muda depois da confirmação.</strong> JARs sem correspondência também entram no índice como arquivos locais, então continuam aparecendo no TXT e na organização de pastas.</span></div>
        {plan.warnings.length ? <details className="rescan-warnings"><summary><Icon name="alert"/> Ver avisos da análise ({plan.warnings.length})</summary><div>{plan.warnings.map((warning, index) => <p key={`${index}:${warning}`}>{warning}</p>)}</div></details> : null}
        {error ? <div className="inline-error"><Icon name="alert"/> {error}</div> : null}
      </div>
      <footer><button className="button ghost" disabled={busy} onClick={onClose}>Cancelar</button><span>Os arquivos antigos também serão preservados no disco.</span><button className="button primary" disabled={busy} onClick={() => void apply()}>{busy ? <><span className="spinner dark"/> Substituindo…</> : <><Icon name="refresh"/> Substituir e reindexar</>}</button></footer>
    </section>
  </div>;
}
