import React, {useMemo, useState} from 'react';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import {formatProxyAuth, normaliseProxyBase} from '../utils/tryItProxy';
import OAuthDeviceLogin from './OAuthDeviceLogin';

const DEFAULT_METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE'];

function normalisePath(input) {
  const trimmed = input.trim();
  if (trimmed === '') {
    return '/';
  }
  if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
    return trimmed;
  }
  return trimmed.startsWith('/') ? trimmed : `/${trimmed}`;
}

function methodAllowsBody(method) {
  return method !== 'GET' && method !== 'HEAD';
}

function formatBodyPreview(text, contentType) {
  if (!text) {
    return '';
  }
  if (contentType && contentType.includes('application/json')) {
    try {
      return JSON.stringify(JSON.parse(text), null, 2);
    } catch (_error) {
      return text;
    }
  }
  return text;
}

export default function TryItConsole() {
  const {siteConfig} = useDocusaurusContext();
  const tryItConfig = siteConfig.customFields?.tryIt ?? {};
  const oauthConfig = siteConfig.customFields?.oauth ?? {};

  const proxyUrl = useMemo(
    () => normaliseProxyBase(tryItConfig.proxyUrl),
    [tryItConfig.proxyUrl]
  );

  const initialPath = tryItConfig.sampleRequest?.path ?? '/v1/status';
  const initialMethod = tryItConfig.sampleRequest?.method ?? 'GET';
  const initialBody = tryItConfig.sampleRequest?.body ?? '';
  const methods = tryItConfig.allowedMethods ?? DEFAULT_METHODS;

  const [method, setMethod] = useState(initialMethod);
  const [path, setPath] = useState(initialPath);
  const [token, setToken] = useState('');
  const [body, setBody] = useState(initialBody);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState('');
  const [response, setResponse] = useState(null);
  const oauthEnabled = Boolean(
    oauthConfig.deviceCodeUrl && oauthConfig.tokenUrl && oauthConfig.clientId
  );

  const proxyConfigured = proxyUrl !== '';
  const canSubmit = proxyConfigured && path.trim() !== '' && !isLoading;

  async function handleSubmit(event) {
    event.preventDefault();
    if (!canSubmit) {
      return;
    }

    setIsLoading(true);
    setError('');
    setResponse(null);

    const target = `${proxyUrl}/proxy${normalisePath(path)}`;
    const headers = new Headers({
      Accept: 'application/json',
      'X-TryIt-Client': 'docs-portal',
    });

    const proxyAuth = formatProxyAuth(token);
    if (proxyAuth !== '') {
      headers.set('X-TryIt-Auth', proxyAuth);
    }

    let requestBody;
    if (methodAllowsBody(method) && body.trim() !== '') {
      headers.set('Content-Type', 'application/json');
      requestBody = body;
    }

    const startedAt = performance.now();

    try {
      const result = await fetch(target, {
        method,
        headers,
        body: requestBody,
      });
      const text = await result.text();

      setResponse({
        status: result.status,
        statusText: result.statusText,
        durationMs: performance.now() - startedAt,
        headers: Array.from(result.headers.entries()),
        body: formatBodyPreview(text, result.headers.get('content-type') ?? ''),
      });

      if (!result.ok) {
        setError(`Request failed with status ${result.status}`);
      }
    } catch (err) {
      setError(`Encountered error while calling proxy: ${err instanceof Error ? err.message : String(err)}`);
    } finally {
      setIsLoading(false);
    }
  }

  return (
    <section className="card shadow--md">
      <div className="card__header">
        <h3>Try it sandbox</h3>
        <p>
          Send authenticated requests through the staging proxy. Configure{' '}
          <code>TRYIT_PROXY_PUBLIC_URL</code> when building the portal so the widget knows where to send
          traffic.
        </p>
      </div>
      <div className="card__body">
        {!proxyConfigured && (
          <div className="alert alert--warning" role="alert">
            The proxy URL is not configured. Run <code>npm run tryit-proxy</code> locally and export{' '}
            <code>TRYIT_PROXY_PUBLIC_URL</code> (e.g., <code>http://localhost:8787</code>) before using the
            console.
          </div>
        )}
        {oauthEnabled && (
          <OAuthDeviceLogin config={oauthConfig} tokenValue={token} onTokenChange={setToken} />
        )}
        <form onSubmit={handleSubmit}>
          <div className="row">
            <div className="col col--2">
              <label className="form-label" htmlFor="tryit-method">
                Method
              </label>
              <select
                id="tryit-method"
                className="form-select"
                value={method}
                onChange={(event) => setMethod(event.target.value)}
              >
                {methods.map((option) => (
                  <option key={option} value={option}>
                    {option}
                  </option>
                ))}
              </select>
            </div>
            <div className="col col--7">
              <label className="form-label" htmlFor="tryit-path">
                Request path
              </label>
              <input
                id="tryit-path"
                className="form-input"
                type="text"
                value={path}
                onChange={(event) => setPath(event.target.value)}
                placeholder="/v1/status"
                required
              />
            </div>
            <div className="col col--3">
              <label className="form-label" htmlFor="tryit-token">
                Bearer token
              </label>
              <input
                id="tryit-token"
                className="form-input"
                type="text"
                value={token}
                onChange={(event) => setToken(event.target.value)}
                placeholder="Optional"
              />
            </div>
          </div>
          {methodAllowsBody(method) && (
            <div className="margin-top--md">
              <label className="form-label" htmlFor="tryit-body">
                JSON body
              </label>
              <textarea
                id="tryit-body"
                className="form-input"
                rows={8}
                spellCheck={false}
                value={body}
                onChange={(event) => setBody(event.target.value)}
                placeholder='{"foo":"bar"}'
              />
            </div>
          )}
          <div className="margin-top--md">
            <button className="button button--primary" type="submit" disabled={!canSubmit}>
              {isLoading ? 'Sending…' : 'Send request'}
            </button>
          </div>
        </form>
        {error && (
          <div className="alert alert--danger margin-top--md" role="alert">
            {error}
          </div>
        )}
        {response && (
          <div className="margin-top--md">
            <h4 className="margin-bottom--sm">
              Response {response.status} {response.statusText}{' '}
              <small className="text--secondary">({Math.round(response.durationMs)} ms)</small>
            </h4>
            <details open>
              <summary className="margin-bottom--sm">Headers</summary>
              <pre className="thin-scrollbar">
                {response.headers.length > 0
                  ? response.headers.map(([name, value]) => `${name}: ${value}`).join('\n')
                  : '[none]'}
              </pre>
            </details>
            <details open className="margin-top--md">
              <summary className="margin-bottom--sm">Body</summary>
              <pre className="thin-scrollbar">{response.body || '[empty]'}</pre>
            </details>
          </div>
        )}
      </div>
    </section>
  );
}
