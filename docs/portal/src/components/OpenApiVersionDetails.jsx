import React from 'react';

function formatSignedState(metadata) {
  if (!metadata) {
    return 'Unknown';
  }
  if (!metadata.signed) {
    return 'Unsigned';
  }
  return metadata.signatureAlgorithm
    ? `Signed (${metadata.signatureAlgorithm})`
    : 'Signed';
}

function formatTimestamp(value) {
  if (!value) {
    return 'Unknown';
  }
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }
  return parsed.toLocaleString();
}

function formatBytes(bytes) {
  if (typeof bytes !== 'number' || !Number.isFinite(bytes)) {
    return 'Unknown';
  }
  return `${bytes.toLocaleString()} bytes`;
}

function manifestHref(path) {
  if (!path) {
    return null;
  }
  const trimmed = path.replace(/^\/+/, '');
  return `/openapi/${trimmed}`;
}

export default function OpenApiVersionDetails({metadata}) {
  if (!metadata) {
    return null;
  }
  const manifestUrl = manifestHref(metadata.manifestPath);
  const signatureState = formatSignedState(metadata);
  return (
    <dl className="openapi-version-details">
      <dt>Snapshot path</dt>
      <dd>
        <code>{metadata.path}</code>
      </dd>
      <dt>File size</dt>
      <dd>{formatBytes(metadata.bytes)}</dd>
      <dt>Last updated</dt>
      <dd>{formatTimestamp(metadata.updatedAt)}</dd>
      <dt>SHA-256</dt>
      <dd>
        <code className="openapi-version-details__hash">{metadata.sha256}</code>
      </dd>
      {metadata.blake3 && (
        <>
          <dt>BLAKE3</dt>
          <dd>
            <code className="openapi-version-details__hash">{metadata.blake3}</code>
          </dd>
        </>
      )}
      {manifestUrl && (
        <>
          <dt>Signed manifest</dt>
          <dd>
            <a
              href={manifestUrl}
              target="_blank"
              rel="noopener noreferrer"
              className="openapi-version-details__manifest-link"
            >
              <code>{metadata.manifestPath}</code>
            </a>
          </dd>
        </>
      )}
      <dt>Signature</dt>
      <dd>
        {signatureState}
        {metadata.signaturePublicKeyHex && (
          <div className="openapi-version-details__signature-block">
            <div>Public key</div>
            <code className="openapi-version-details__hash">
              {metadata.signaturePublicKeyHex}
            </code>
          </div>
        )}
        {metadata.signatureHex && (
          <div className="openapi-version-details__signature-block">
            <div>Signature</div>
            <code className="openapi-version-details__hash">
              {metadata.signatureHex}
            </code>
          </div>
        )}
      </dd>
    </dl>
  );
}
