import React, {useEffect, useMemo, useRef, useState} from 'react';
import {specUrlForVersion, useOpenApiVersions} from '../utils/openapiVersioning';

const REDOC_CDN = 'https://cdn.jsdelivr.net/npm/redoc@2.1.3/bundles/redoc.standalone.js';

function formatGeneratedLabel(timestamp) {
  if (!timestamp) {
    return null;
  }
  const parsed = new Date(timestamp);
  if (Number.isNaN(parsed.getTime())) {
    return timestamp;
  }
  return parsed.toLocaleString();
}

function loadRedocScript() {
  if (window.Redoc) {
    return Promise.resolve();
  }

  return new Promise((resolve, reject) => {
    const existing = document.querySelector('script[data-redoc-loaded]');
    if (existing) {
      existing.addEventListener('load', () => resolve(), {once: true});
      existing.addEventListener('error', reject, {once: true});
      return;
    }

    const script = document.createElement('script');
    script.src = REDOC_CDN;
    script.async = true;
    script.dataset.redocLoaded = 'true';
    script.onload = () => resolve();
    script.onerror = reject;
    document.body.appendChild(script);
  });
}

export default function ToriiOpenApiPage() {
  const containerRef = useRef(null);
  const {versions, loading, error, generatedAt} = useOpenApiVersions();
  const [selectedVersion, setSelectedVersion] = useState('latest');

  useEffect(() => {
    if (!versions.includes(selectedVersion)) {
      setSelectedVersion(versions[0] ?? 'latest');
    }
  }, [versions, selectedVersion]);

  const specUrl = useMemo(() => specUrlForVersion(selectedVersion), [selectedVersion]);

  useEffect(() => {
    let cancelled = false;

    if (containerRef.current) {
      containerRef.current.innerHTML = '';
    }

    loadRedocScript()
      .then(() => {
        if (!cancelled && window.Redoc) {
          window.Redoc.init(
            specUrl,
            {
              scrollYOffset: '.navbar',
              hideLoading: true,
              expandResponses: '200,201'
            },
            containerRef.current
          );
        }
      })
      .catch((error) => {
        if (!cancelled && containerRef.current) {
          containerRef.current.innerHTML = `<pre style="color: var(--ifm-color-danger)">${String(
            error
          )}</pre>`;
        }
      });

    return () => {
      cancelled = true;
      if (containerRef.current) {
        containerRef.current.innerHTML = '';
      }
    };
  }, [specUrl]);

  const versionItems = useMemo(
    () =>
      versions.map((version) => ({
        value: version,
        label: version === 'latest' ? 'Latest (tracked)' : version,
      })),
    [versions]
  );
  const generatedLabel = useMemo(
    () => formatGeneratedLabel(generatedAt),
    [generatedAt]
  );

  return (
    <main className="container margin-vert--lg">
      <header className="margin-bottom--lg">
        <h1>Torii OpenAPI</h1>
        <p className="hero__subtitle">
          Reference documentation for the Torii REST API. The specification is generated from the
          latest `cargo xtask openapi` output.
        </p>
      </header>
      <section className="margin-bottom--lg">
        <label className="form-label" htmlFor="openapi-version">
          OpenAPI snapshot
        </label>
        <select
          id="openapi-version"
          className="form-select"
          value={selectedVersion}
          disabled={loading || versionItems.length === 0}
          onChange={(event) => setSelectedVersion(event.target.value)}
        >
          {versionItems.map((item) => (
            <option key={item.value} value={item.value}>
              {item.label}
            </option>
          ))}
        </select>
        {loading && (
          <p className="text--secondary margin-top--xs">Loading available versions…</p>
        )}
        {error && (
          <p className="text--danger margin-top--xs">
            Failed to load OpenAPI versions: {error.message}
          </p>
        )}
        {generatedLabel && !loading && !error && (
          <p className="text--secondary margin-top--xs">
            Version index updated {generatedLabel}
          </p>
        )}
      </section>
      <div ref={containerRef} />
    </main>
  );
}
