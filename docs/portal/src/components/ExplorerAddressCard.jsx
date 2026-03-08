import React, {useState} from 'react';

const SAMPLE_ADDRESS = {
  ih58: '6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw',
  compressed: 'sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE',
  warning: 'Compressed addresses are legacy bridge compatibility only; prefer IH58.',
};

export default function ExplorerAddressCard() {
  const [status, setStatus] = useState(null);

  async function handleCopy(mode, literal, warning) {
    try {
      await navigator.clipboard.writeText(literal);
    } catch (_err) {
      // navigator.clipboard may be unavailable on some browsers; fall back to execCommand.
      const textArea = document.createElement('textarea');
      textArea.value = literal;
      textArea.style.position = 'fixed';
      textArea.style.left = '-9999px';
      document.body.appendChild(textArea);
      textArea.focus();
      textArea.select();
      document.execCommand('copy');
      document.body.removeChild(textArea);
    }
    recordTelemetry(mode);
    setStatus({mode, warning});
  }

  const domainHelperId = 'explorer-address-domain-hint';

  return (
    <div className="card shadow--md explorer-address-card" role="region" aria-label="Explorer address preview">
      <div className="card__header">
        <h3>Explorer copy instrumentation</h3>
        <p id={domainHelperId}>
          IH58 literals are global and selector-free. Domain/dataspace access is granted on-chain.
        </p>
      </div>
      <div className="card__body" aria-describedby={domainHelperId}>
        <dl>
          <dt>IH58 literal (preferred)</dt>
          <dd>
            <code>{SAMPLE_ADDRESS.ih58}</code>
          </dd>
          <dt>Compressed Sora-only literal (second-best)</dt>
          <dd>
            <code>{SAMPLE_ADDRESS.compressed}</code>
            <div className="text--warning">{SAMPLE_ADDRESS.warning}</div>
          </dd>
        </dl>

        <div className="button-group" role="group" aria-label="Copy controls">
          <button
            className="button button--primary"
            data-copy-mode="ih58"
            aria-pressed="false"
            aria-label="Copy IH58 address (preferred, safe to share)"
            onClick={() => handleCopy('ih58', SAMPLE_ADDRESS.ih58, null)}>
            Copy IH58 (preferred)
          </button>
          <button
            className="button button--secondary"
            data-copy-mode="compressed"
            aria-pressed="false"
            aria-label="Copy compressed Sora-only address (second-best; warn recipients)"
            onClick={() => handleCopy('compressed', SAMPLE_ADDRESS.compressed, SAMPLE_ADDRESS.warning)}>
            Copy compressed (`sora`, second-best)
          </button>
        </div>

        <figure className="margin-top--md" role="img" aria-label={`IH58 QR for ${SAMPLE_ADDRESS.ih58}`}>
          <img src="/img/sns/address_copy_ios.svg" alt="IH58 QR reference" />
          <figcaption>QR payloads must encode IH58 strings (preferred).</figcaption>
        </figure>
        <output
          aria-live="polite"
          className="margin-top--md"
          hidden={!status}
          data-copy-status={status?.mode ?? undefined}>
          {status ? (
            <span>
              Copied {status.mode} address.{status.warning ? ` ${status.warning}` : ''}
            </span>
          ) : null}
        </output>
      </div>
    </div>
  );
}

function recordTelemetry(mode) {
  if (typeof window !== 'undefined') {
    const detail = {mode, timestamp: Date.now()};
    window.dispatchEvent(new CustomEvent('iroha:address-copy', {detail}));
    if (window?.irohaCopyTelemetry?.track) {
      window.irohaCopyTelemetry.track(detail);
    }
  }
  // eslint-disable-next-line no-console
  console.info(`[ExplorerAddressCard] address_copy_mode=${mode}`);
}
