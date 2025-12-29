// Security helper utilities shared between the Docusaurus config and tests.

export const OAUTH_LIMITS = Object.freeze({
  minPollIntervalMs: 5_000,
  minDeviceCodeLifetimeSeconds: 300,
  maxDeviceCodeLifetimeSeconds: 900,
  minTokenLifetimeSeconds: 300,
  maxTokenLifetimeSeconds: 900,
});

export function extractOrigin(value, {label} = {}) {
  if (!value) {
    return null;
  }
  try {
    const parsed = new URL(value);
    return parsed.origin;
  } catch (_error) {
    const labelHint = label ? `${label} ` : "";
    throw new Error(
      `Invalid ${labelHint}URL '${value}'. Provide a fully qualified URL (https://...) or leave the variable empty.`,
    );
  }
}

export function normalizeConnectOrigin(value, {allowInsecure = false, label} = {}) {
  const origin = extractOrigin(value, {label});
  if (!origin) {
    return null;
  }
  if (!allowInsecure && !origin.startsWith('https://')) {
    throw new Error(
      `Insecure analytics/try-it origin '${origin}' rejected; set DOCS_SECURITY_ALLOW_INSECURE=1 only for local previews.`,
    );
  }
  return origin;
}

export function enforceOAuthConfig(config, {allowBypass = false, limits = OAUTH_LIMITS} = {}) {
  if (allowBypass) {
    return config;
  }

  const requiredFields = [
    ['deviceCodeUrl', 'DOCS_OAUTH_DEVICE_CODE_URL'],
    ['tokenUrl', 'DOCS_OAUTH_TOKEN_URL'],
    ['clientId', 'DOCS_OAUTH_CLIENT_ID'],
    ['scope', 'DOCS_OAUTH_SCOPE'],
  ];
  const missing = requiredFields
    .filter(([field]) => {
      const value = config[field];
      return typeof value !== 'string' || value.trim() === '';
    })
    .map(([, envVar]) => envVar);

  if (missing.length > 0) {
    throw new Error(
      `Missing required OAuth configuration: ${missing.join(
        ', ',
      )}. Provide the DOCS_OAUTH_* environment variables or set DOCS_OAUTH_ALLOW_INSECURE=1 for local-only builds.`,
    );
  }

  const violations = [];
  if (config.pollIntervalMs < limits.minPollIntervalMs) {
    violations.push(
      `DOCS_OAUTH_POLL_INTERVAL_MS must be >= ${limits.minPollIntervalMs} ms`,
    );
  }
  if (
    config.deviceCodeExpiresSeconds < limits.minDeviceCodeLifetimeSeconds ||
    config.deviceCodeExpiresSeconds > limits.maxDeviceCodeLifetimeSeconds
  ) {
    violations.push(
      `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS must be between ${limits.minDeviceCodeLifetimeSeconds} and ${limits.maxDeviceCodeLifetimeSeconds} seconds`,
    );
  }
  if (
    config.tokenLifetimeSeconds < limits.minTokenLifetimeSeconds ||
    config.tokenLifetimeSeconds > limits.maxTokenLifetimeSeconds
  ) {
    violations.push(
      `DOCS_OAUTH_TOKEN_TTL_SECONDS must be between ${limits.minTokenLifetimeSeconds} and ${limits.maxTokenLifetimeSeconds} seconds`,
    );
  }

  if (violations.length > 0) {
    throw new Error(
      `Invalid OAuth configuration:\n- ${violations.join(
        '\n- ',
      )}\nSet DOCS_OAUTH_ALLOW_INSECURE=1 only for local development.`,
    );
  }

  return config;
}

export function enforceTryItDefaultBearer({
  defaultBearer,
  allowDefaultBearer = false,
  allowInsecure = false,
} = {}) {
  if (!defaultBearer) {
    return '';
  }
  if (allowDefaultBearer || allowInsecure) {
    return defaultBearer;
  }
  throw new Error(
    'TRYIT_PROXY_DEFAULT_BEARER is set but neither DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1 nor DOCS_SECURITY_ALLOW_INSECURE=1 is enabled. Default bearer tokens must be an explicit, local-only opt-in.',
  );
}

export function buildSecurityHeaders({
  analyticsUrl,
  tryItUrl,
  deviceCodeUrl,
  tokenUrl,
  allowInsecure = false,
} = {}) {
  const connectSources = new Set(["'self'"]);
  for (const origin of [
    normalizeConnectOrigin(analyticsUrl, {
      allowInsecure,
      label: 'analytics endpoint',
    }),
    normalizeConnectOrigin(tryItUrl, {
      allowInsecure,
      label: 'try-it proxy URL',
    }),
    normalizeConnectOrigin(deviceCodeUrl, {
      allowInsecure,
      label: 'OAuth device-code URL',
    }),
    normalizeConnectOrigin(tokenUrl, {
      allowInsecure,
      label: 'OAuth token URL',
    }),
  ]) {
    if (origin) {
      connectSources.add(origin);
    }
  }

  const styleSources = ["'self'", "'unsafe-inline'", 'https://fonts.googleapis.com'];
  const fontSources = ["'self'", 'data:', 'https://fonts.gstatic.com'];
  const csp = [
    "default-src 'self'",
    `connect-src ${Array.from(connectSources).join(' ')}`,
    "img-src 'self' data: blob:",
    "script-src 'self' 'unsafe-inline' 'unsafe-eval' 'wasm-unsafe-eval'",
    `style-src ${styleSources.join(' ')}`,
    `font-src ${fontSources.join(' ')}`,
    "object-src 'none'",
    "frame-src 'none'",
    "frame-ancestors 'none'",
    "base-uri 'self'",
    "form-action 'self'",
    "manifest-src 'self'",
    "worker-src 'self'",
    "require-trusted-types-for 'script'",
    "trusted-types docsPortal default",
  ].join('; ');

  const permissionsPolicy = 'geolocation=(), microphone=(), camera=(), payment=()';
  const referrerPolicy = 'no-referrer';
  const crossOriginOpenerPolicy = 'same-origin';
  const crossOriginResourcePolicy = 'same-site';
  const originAgentCluster = '?1';

  return {
    csp,
    permissionsPolicy,
    referrerPolicy,
    crossOriginOpenerPolicy,
    crossOriginResourcePolicy,
    originAgentCluster,
  };
}

export function buildSecurityHeadTags(security = buildSecurityHeaders()) {
  return [
    {
      tagName: 'meta',
      attributes: {
        'http-equiv': 'Content-Security-Policy',
        content: security.csp,
      },
    },
    {
      tagName: 'meta',
      attributes: {
        'http-equiv': 'Permissions-Policy',
        content: security.permissionsPolicy,
      },
    },
    {
      tagName: 'meta',
      attributes: {
        name: 'referrer',
        content: security.referrerPolicy,
      },
    },
    {
      tagName: 'meta',
      attributes: {
        'http-equiv': 'Cross-Origin-Opener-Policy',
        content: security.crossOriginOpenerPolicy,
      },
    },
    {
      tagName: 'meta',
      attributes: {
        'http-equiv': 'Cross-Origin-Resource-Policy',
        content: security.crossOriginResourcePolicy,
      },
    },
    {
      tagName: 'meta',
      attributes: {
        'http-equiv': 'Origin-Agent-Cluster',
        content: security.originAgentCluster,
      },
    },
  ];
}
