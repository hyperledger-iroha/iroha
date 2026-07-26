import React from 'react';
import Link from '@docusaurus/Link';

export default function SampleDownload({href, filename, description}) {
  if (!href) {
    return null;
  }

  return (
    <div className="sample-download-card">
      <div>
        <p className="sample-download-card__label">Sample download</p>
        {filename && (
          <div className="sample-download-card__filename">
            <code>{filename}</code>
          </div>
        )}
        {description && (
          <p className="sample-download-card__description">{description}</p>
        )}
      </div>
      <Link className="button button--primary" href={href} download>
        Download
      </Link>
    </div>
  );
}
