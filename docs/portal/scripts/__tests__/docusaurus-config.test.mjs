import assert from 'node:assert/strict';
import test from 'node:test';

const previousBypass = process.env.DOCS_OAUTH_ALLOW_INSECURE;
const previousSecurityBypass = process.env.DOCS_SECURITY_ALLOW_INSECURE;
process.env.DOCS_OAUTH_ALLOW_INSECURE = '1';
process.env.DOCS_SECURITY_ALLOW_INSECURE = '0';
const {
  enforceOAuthConfig,
  enforceTryItDefaultBearer,
  buildSecurityHeaders,
  buildSecurityHeadTags,
} = await import('../../config/security-helpers.js');
process.env.DOCS_OAUTH_ALLOW_INSECURE = previousBypass;
process.env.DOCS_SECURITY_ALLOW_INSECURE = previousSecurityBypass;

function baseOauthConfig(overrides = {}) {
  return {
    deviceCodeUrl: 'https://auth.example/device',
    tokenUrl: 'https://auth.example/token',
    clientId: 'docs-preview',
    scope: 'openid profile offline_access',
    audience: 'torii',
    pollIntervalMs: 5_000,
    deviceCodeExpiresSeconds: 600,
    tokenLifetimeSeconds: 900,
    ...overrides,
  };
}

test('enforceOAuthConfig rejects missing configuration', () => {
  assert.throws(
    () => enforceOAuthConfig(baseOauthConfig({deviceCodeUrl: ''})),
    /DOCS_OAUTH_DEVICE_CODE_URL/,
  );
  assert.throws(
    () => enforceOAuthConfig(baseOauthConfig({clientId: ''})),
    /DOCS_OAUTH_CLIENT_ID/,
  );
});

test('enforceOAuthConfig enforces TTL and poll ranges', () => {
  assert.throws(
    () => enforceOAuthConfig(baseOauthConfig({tokenLifetimeSeconds: 120})),
    /DOCS_OAUTH_TOKEN_TTL_SECONDS/,
  );
  assert.throws(
    () => enforceOAuthConfig(baseOauthConfig({deviceCodeExpiresSeconds: 1_200})),
    /DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS/,
  );
  assert.throws(
    () => enforceOAuthConfig(baseOauthConfig({pollIntervalMs: 1_000})),
    /DOCS_OAUTH_POLL_INTERVAL_MS/,
  );
});

test('enforceOAuthConfig returns config when valid', () => {
  const config = baseOauthConfig();
  assert.equal(enforceOAuthConfig(config), config);
});

test('buildSecurityHeaders includes HTTPS origins in connect-src', () => {
  const headers = buildSecurityHeaders({
    analyticsUrl: 'https://metrics.example.com/v2',
    tryItUrl: 'https://tryit.example.com/proxy',
  });
  assert.ok(
    headers.csp.includes(
      "connect-src 'self' https://metrics.example.com https://tryit.example.com",
    ),
  );
});

test('buildSecurityHeaders includes OAuth endpoints in connect-src', () => {
  const headers = buildSecurityHeaders({
    analyticsUrl: 'https://metrics.example.com/v2',
    tryItUrl: 'https://tryit.example.com/proxy',
    deviceCodeUrl: 'https://auth.example.com/device',
    tokenUrl: 'https://auth.example.com/token',
  });
  assert.ok(headers.csp.includes('https://auth.example.com'));
});

test('buildSecurityHeaders whitelists theme fonts', () => {
  const headers = buildSecurityHeaders({});
  assert.match(
    headers.csp,
    /style-src 'self' 'unsafe-inline' https:\/\/fonts\.googleapis\.com/,
  );
  assert.match(headers.csp, /font-src 'self' data: https:\/\/fonts\.gstatic\.com/);
});

test('buildSecurityHeaders rejects malformed URLs', () => {
  assert.throws(
    () => buildSecurityHeaders({analyticsUrl: 'not-a-url'}),
    /analytics endpoint URL/,
  );
  assert.throws(
    () => buildSecurityHeaders({tryItUrl: '://missing-scheme'}),
    /try-it proxy URL/,
  );
});

test('buildSecurityHeaders tolerates empty optional endpoints and keeps CSP guards', () => {
  const headers = buildSecurityHeaders({
    analyticsUrl: '',
    tryItUrl: '',
    deviceCodeUrl: '',
    tokenUrl: '',
  });
  assert.match(headers.csp, /connect-src 'self'/);
  assert.match(headers.csp, /frame-ancestors 'none'/);
  assert.match(headers.csp, /object-src 'none'/);
});

test('buildSecurityHeaders rejects non-HTTPS origins without explicit opt-in', () => {
  assert.throws(
    () => buildSecurityHeaders({analyticsUrl: 'http://localhost:4318'}),
    /DOCS_SECURITY_ALLOW_INSECURE=1/,
  );
  assert.throws(
    () =>
      buildSecurityHeaders({
        analyticsUrl: 'https://metrics.example.com',
        deviceCodeUrl: 'http://localhost:8080/device',
      }),
    /DOCS_SECURITY_ALLOW_INSECURE=1/,
  );
});

test('buildSecurityHeaders allows insecure origins when explicitly enabled', () => {
  const headers = buildSecurityHeaders({
    analyticsUrl: 'http://localhost:4318',
    allowInsecure: true,
  });
  assert.match(headers.csp, /connect-src 'self' http:\/\/localhost:4318/);
});

test('buildSecurityHeaders allows insecure OAuth endpoints when explicitly enabled', () => {
  const headers = buildSecurityHeaders({
    analyticsUrl: 'https://metrics.example.com',
    tryItUrl: 'https://tryit.example.com',
    deviceCodeUrl: 'http://localhost:8080/device',
    tokenUrl: 'http://127.0.0.1:8080/token',
    allowInsecure: true,
  });
  assert.match(headers.csp, /http:\/\/localhost:8080/);
  assert.match(headers.csp, /http:\/\/127.0.0.1:8080/);
});

test('enforceTryItDefaultBearer rejects default bearer without opt-in', () => {
  assert.throws(
    () =>
      enforceTryItDefaultBearer({
        defaultBearer: 'Bearer secret-token',
        allowDefaultBearer: false,
        allowInsecure: false,
      }),
    /DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1/,
  );
});

test('enforceTryItDefaultBearer allows explicit opt-in', () => {
  assert.equal(
    enforceTryItDefaultBearer({
      defaultBearer: 'Bearer dev-token',
      allowDefaultBearer: true,
      allowInsecure: false,
    }),
    'Bearer dev-token',
  );
});

function findHeadTag(tags, httpEquiv) {
  return tags.find(
    (tag) =>
      tag.tagName === 'meta' &&
      typeof tag.attributes?.['http-equiv'] === 'string' &&
      tag.attributes['http-equiv'].toLowerCase() === httpEquiv.toLowerCase(),
  );
}

test('buildSecurityHeadTags emits cross-origin guardrails', () => {
  const tags = buildSecurityHeadTags();

  assert.equal(
    findHeadTag(tags, 'Cross-Origin-Opener-Policy')?.attributes.content,
    'same-origin',
  );
  assert.equal(
    findHeadTag(tags, 'Cross-Origin-Resource-Policy')?.attributes.content,
    'same-site',
  );
  assert.equal(findHeadTag(tags, 'Origin-Agent-Cluster')?.attributes.content, '?1');
});
