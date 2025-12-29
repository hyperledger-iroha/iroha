import React, {useEffect, useMemo, useRef, useState} from 'react';
import BrowserOnly from '@docusaurus/BrowserOnly';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import {specUrlForVersion, useOpenApiVersions} from '../utils/openapiVersioning';
import {
  buildProxiedUrl,
  formatProxyAuth,
  normaliseProxyBase,
} from '../utils/tryItProxy';
import OpenApiVersionDetails from './OpenApiVersionDetails';

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

const RAPIDOC_CDN = 'https://unpkg.com/rapidoc@9.3.4/dist/rapidoc-min.js';

function useRapiDocScript() {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    if (typeof window === 'undefined') {
      return;
    }
    if (window.customElements?.get('rapi-doc')) {
      setReady(true);
      return;
    }

    let cancelled = false;
    const script = document.createElement('script');
    script.src = RAPIDOC_CDN;
    script.async = true;
    script.onload = () => {
      if (!cancelled) {
        setReady(true);
      }
    };
    script.onerror = () => {
      if (!cancelled) {
        setReady(false);
      }
    };

    document.body.appendChild(script);

    return () => {
      cancelled = true;
      if (script.parentNode) {
        script.parentNode.removeChild(script);
      }
    };
  }, []);

  return ready;
}

function RapiDocInner({defaultToken, specUrl, proxyUrl}) {
  const ready = useRapiDocScript();
  const docRef = useRef(null);
  const [token, setToken] = useState(defaultToken);
  const proxyBase = useMemo(() => normaliseProxyBase(proxyUrl), [proxyUrl]);

  useEffect(() => {
    if (!ready || !docRef.current) {
      return;
    }
    docRef.current.setAttribute('api-key-name', 'X-TryIt-Auth');
    docRef.current.setAttribute('api-key-location', 'header');
    docRef.current.setAttribute('allow-authentication', 'true');
    docRef.current.setAttribute('allow-server-selection', 'true');
    docRef.current.setAttribute('allow-try', 'true');
    docRef.current.setAttribute('show-header', 'false');
    docRef.current.setAttribute('render-style', 'read');
    docRef.current.setAttribute('theme', 'light');
    docRef.current.setAttribute('schema-style', 'table');
  }, [ready]);

  useEffect(() => {
    if (!docRef.current) {
      return;
    }
    const auth = formatProxyAuth(token);
    if (proxyBase) {
      docRef.current.setAttribute('server-url', `${proxyBase}/proxy`);
    } else {
      docRef.current.removeAttribute('server-url');
    }
    if (auth) {
      docRef.current.setAttribute('api-key-value', auth);
    } else {
      docRef.current.removeAttribute('api-key-value');
    }
  }, [token, proxyBase]);

  useEffect(() => {
    if (!docRef.current) {
      return;
    }
    const element = docRef.current;
    const handleBeforeTry = (event) => {
      const request = event?.detail?.request;
      if (!request) {
        return;
      }
      request.headers ??= {};
      request.headers['X-TryIt-Client'] = 'docs-portal-rapidoc';
      const auth = formatProxyAuth(token);
      if (auth) {
        request.headers['X-TryIt-Auth'] = auth;
      } else {
        delete request.headers['X-TryIt-Auth'];
      }
      if (proxyBase) {
        const proxied = buildProxiedUrl(proxyBase, request.url);
        if (proxied) {
          request.url = proxied;
        }
      }
    };

    element.addEventListener('before-try', handleBeforeTry);
    return () => {
      element.removeEventListener('before-try', handleBeforeTry);
    };
  }, [token, proxyBase]);

  useEffect(() => {
    if (docRef.current) {
      docRef.current.setAttribute('spec-url', specUrl);
    }
  }, [specUrl]);

  if (!ready) {
    return <div>Loading RapiDoc…</div>;
  }
  if (!customElements?.get('rapi-doc')) {
    return <div role="alert">Failed to load RapiDoc bundle.</div>;
  }

  return (
    <>
      <div className="margin-bottom--md">
        <label className="form-label" htmlFor="rapidoc-token">
          Bearer token
        </label>
        <input
          id="rapidoc-token"
          className="form-input"
          type="text"
          placeholder="Optional"
          value={token}
          onChange={(event) => setToken(event.target.value)}
        />
      </div>
      <rapi-doc ref={docRef} spec-url={specUrl} />
    </>
  );
}

export default function RapiDocPanel() {
  const {siteConfig} = useDocusaurusContext();
  const tryIt = siteConfig.customFields?.tryIt ?? {};
  const defaultToken = tryIt.defaultBearer ?? '';
  const configSpecUrl =
    typeof tryIt.specUrl === 'string' ? tryIt.specUrl.trim() : '';
  const proxyUrl = normaliseProxyBase(tryIt.proxyUrl);
  const versioningEnabled = configSpecUrl === '';
  const {versions, loading, error, generatedAt, metadataByVersion} = useOpenApiVersions({
    enabled: versioningEnabled,
  });
  const [selectedVersion, setSelectedVersion] = useState('latest');

  useEffect(() => {
    if (!versioningEnabled) {
      return;
    }
    if (!versions.includes(selectedVersion)) {
      setSelectedVersion(versions[0] ?? 'latest');
    }
  }, [versioningEnabled, versions, selectedVersion]);

  const specUrl = versioningEnabled
    ? specUrlForVersion(selectedVersion)
    : configSpecUrl;
  const generatedLabel = useMemo(
    () => formatGeneratedLabel(generatedAt),
    [generatedAt]
  );
  const versionItems = useMemo(() => {
    if (!versioningEnabled) {
      return [];
    }
    return versions.map((version) => ({
      value: version,
      label: version === 'latest' ? 'Latest (tracked)' : version,
    }));
  }, [versioningEnabled, versions]);

  const selectedMetadata = versioningEnabled
    ? metadataByVersion.get(selectedVersion) ?? null
    : null;

  return (
    <section className="card shadow--md">
      <div className="card__header">
        <h3>RapiDoc</h3>
        <p>
          Alternate OpenAPI explorer with an emphasis on compact read-only layouts. Provide a bearer
          token to populate the authorization header for interactive requests.
        </p>
      </div>
      <div className="card__body">
        {versioningEnabled && (
          <div className="margin-bottom--md">
            <label className="form-label" htmlFor="rapidoc-spec-version">
              OpenAPI snapshot
            </label>
            <select
              id="rapidoc-spec-version"
              className="form-select"
              value={selectedVersion}
              disabled={loading || versionItems.length === 0}
              onChange={(event) => setSelectedVersion(event.target.value)}
            >
              {versionItems.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
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
          </div>
        )}
        {versioningEnabled && <OpenApiVersionDetails metadata={selectedMetadata} />}
        <BrowserOnly fallback={<div>Loading RapiDoc…</div>}>
          {() => (
            <RapiDocInner
              defaultToken={defaultToken}
              specUrl={specUrl}
              proxyUrl={proxyUrl}
            />
          )}
        </BrowserOnly>
        {!proxyUrl && (
          <p className="text--warning margin-top--sm">
            Configure <code>TRYIT_PROXY_PUBLIC_URL</code> to enable proxy-backed requests.
          </p>
        )}
      </div>
    </section>
  );
}
