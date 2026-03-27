import React, {useState} from 'react';

const SAMPLE_ADDRESS = {
  i105: 'sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE',
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
          i105 literals are global and selector-free. Domain/dataspace access is granted on-chain.
        </p>
      </div>
      <div className="card__body" aria-describedby={domainHelperId}>
        <dl>
          <dt>i105 account ID</dt>
          <dd>
            <code>{SAMPLE_ADDRESS.i105}</code>
          </dd>
        </dl>

        <div className="button-group" role="group" aria-label="Copy controls">
          <button
            className="button button--primary"
            data-copy-mode="i105"
            aria-pressed="false"
            aria-label="Copy i105 account ID"
            onClick={() => handleCopy('i105', SAMPLE_ADDRESS.i105, null)}>
            Copy i105
          </button>
        </div>

        <figure className="margin-top--md" role="img" aria-label={`i105 QR for ${SAMPLE_ADDRESS.i105}`}>
          <img src="/img/sns/address_copy_ios.svg" alt="i105 QR reference" />
          <figcaption>QR payloads must encode i105 account IDs.</figcaption>
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
