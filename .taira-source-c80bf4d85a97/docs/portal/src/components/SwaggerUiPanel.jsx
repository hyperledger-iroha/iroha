import React, {useCallback, useEffect, useMemo, useRef, useState} from 'react';
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

function normaliseToken(value) {
  const trimmed = value.trim();
  if (!trimmed) {
    return '';
  }
  return trimmed.toLowerCase().startsWith('bearer ') ? trimmed : `Bearer ${trimmed}`;
}

function SwaggerUiInner({defaultToken, specUrl, docExpansion, maxDisplayedTags, proxyUrl}) {
  const [token, setToken] = useState(defaultToken);
  const tokenRef = useRef(defaultToken);
  const swaggerSystemRef = useRef(null);

  useEffect(() => {
    tokenRef.current = token;
  }, [token]);

  const requestInterceptor = useMemo(() => {
    return (request) => {
      const authorisation = normaliseToken(token);
      const proxyAuth = formatProxyAuth(token);
      request.headers ??= {};
      request.headers['X-TryIt-Client'] = 'docs-portal-swagger';
      if (proxyAuth) {
        request.headers['X-TryIt-Auth'] = proxyAuth;
      } else {
        delete request.headers['X-TryIt-Auth'];
      }
      if (proxyUrl) {
        const proxied = buildProxiedUrl(proxyUrl, request.url);
        if (proxied) {
          request.url = proxied;
        }
      }
      if (authorisation) {
        request.headers.Authorization = authorisation;
      } else {
        delete request.headers.Authorization;
      }
      return request;
    };
  }, [token, proxyUrl]);

  const applyAuthorization = useCallback((swaggerUI, rawToken) => {
    if (!swaggerUI) {
      return;
    }
    const authorisation = normaliseToken(rawToken ?? '');
    if (authorisation) {
      swaggerUI.preauthorizeApiKey?.('bearerAuth', authorisation);
    } else {
      swaggerUI.authActions?.logout?.(['bearerAuth']);
    }
  }, []);

  useEffect(() => {
    return () => {
      swaggerSystemRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (swaggerSystemRef.current) {
      applyAuthorization(swaggerSystemRef.current, token);
    }
  }, [token, applyAuthorization]);

  const handleSwaggerComplete = useCallback(
    (swaggerUI) => {
      swaggerSystemRef.current = swaggerUI;
      applyAuthorization(swaggerUI, tokenRef.current);
    },
    [applyAuthorization]
  );

  const [SwaggerUIComponent, setSwaggerUIComponent] = useState(null);

  useEffect(() => {
    let mounted = true;
    Promise.all([
      import('swagger-ui-react'),
      import('swagger-ui-react/swagger-ui.css'),
    ]).then(([module]) => {
      if (mounted) {
        setSwaggerUIComponent(() => module.default);
      }
    });
    return () => {
      mounted = false;
    };
  }, []);

  if (!SwaggerUIComponent) {
    return <div>Loading Swagger UI…</div>;
  }

  return (
    <>
      <div className="margin-bottom--md">
        <label className="form-label" htmlFor="swagger-token">
          Bearer token
        </label>
        <input
          id="swagger-token"
          className="form-input"
          type="text"
          placeholder="Optional"
          value={token}
          onChange={(event) => setToken(event.target.value)}
        />
      </div>
      <SwaggerUIComponent
        key={specUrl}
        url={specUrl}
        docExpansion={docExpansion}
        deepLinking
        displayRequestDuration
        requestInterceptor={requestInterceptor}
        onComplete={handleSwaggerComplete}
        defaultModelsExpandDepth={0}
        displayOperationId
        tryItOutEnabled
        syntaxHighlight={{activated: true}}
        persistAuthorization={false}
        requestSnippetsEnabled
        maxDisplayedTags={maxDisplayedTags}
      />
    </>
  );
}

export default function SwaggerUiPanel() {
  const {siteConfig} = useDocusaurusContext();
  const defaults = siteConfig.customFields?.tryIt ?? {};
  const defaultToken = defaults.defaultBearer ?? '';
  const configSpecUrl =
    typeof defaults.specUrl === 'string' ? defaults.specUrl.trim() : '';
  const proxyUrl = normaliseProxyBase(defaults.proxyUrl);
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
        <h3>Swagger UI</h3>
        <p>
          Explore the Torii OpenAPI surface interactively. Supply a bearer token to pre-authorise
          requests before trying operations.
        </p>
      </div>
      <div className="card__body">
        {versioningEnabled && (
          <div className="margin-bottom--md">
            <label className="form-label" htmlFor="swagger-spec-version">
              OpenAPI snapshot
            </label>
            <select
              id="swagger-spec-version"
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
        <BrowserOnly fallback={<div>Loading Swagger UI…</div>}>
          {() => (
            <SwaggerUiInner
              defaultToken={defaultToken}
              specUrl={specUrl}
              docExpansion="list"
              maxDisplayedTags={200}
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
