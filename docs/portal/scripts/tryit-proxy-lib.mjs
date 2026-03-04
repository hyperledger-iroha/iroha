import {createHash} from 'node:crypto';
import {readFile} from 'node:fs/promises';
import {createServer} from 'node:http';
import {Readable} from 'node:stream';
import {pipeline} from 'node:stream/promises';

import {verifyOpenApiSignature} from './lib/openapi-signature.mjs';

const DEFAULT_LOGGER = {
  info: (...args) => console.info(...args),
  warn: (...args) => console.warn(...args),
  error: (...args) => console.error(...args),
};

const now =
  typeof performance !== 'undefined' && typeof performance.now === 'function'
    ? () => performance.now()
    : () => Date.now();

const ALLOWED_METHODS = new Set(['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD']);
const ALLOWED_CORS_HEADERS = Object.freeze([
  'Content-Type',
  'Authorization',
  'X-TryIt-Auth',
  'X-TryIt-Client',
  'X-Request-ID',
  'X-Sora-Request-ID',
  'X-Sora-Trace-Id',
  'X-Torii-Request-ID',
]);
const FORWARDED_HEADERS = [
  'accept',
  'content-type',
  'if-none-match',
  'if-match',
  'x-request-id',
  'x-sora-request-id',
  'x-sora-trace-id',
  'x-torii-request-id',
];

function isString(value) {
  return typeof value === 'string';
}

function normalizeOriginEntry(origin) {
  if (typeof origin !== 'string') {
    throw new Error('allowed origins must be strings');
  }
  const trimmed = origin.trim();
  if (trimmed === '') {
    throw new Error('allowed origin entries must be non-empty strings');
  }
  let parsed;
  try {
    parsed = new URL(trimmed);
  } catch (error) {
    throw new Error(`invalid origin "${origin}": ${error.message}`);
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new Error(`origin must use http or https: ${origin}`);
  }
  return parsed.origin;
}

function normalizeAllowedOrigins(origins) {
  if (!origins) {
    return new Set();
  }
  const entries =
    origins instanceof Set
      ? origins
      : Array.isArray(origins)
        ? origins
        : null;
  if (!entries) {
    throw new Error('allowedOrigins must be a Set or array of origin strings');
  }
  const result = new Set();
  for (const entry of entries) {
    result.add(normalizeOriginEntry(entry));
  }
  return result;
}

export function parseListenAddress(value) {
  const trimmed = (value ?? '').trim();
  if (trimmed === '') {
    throw new Error('listen address must be a non-empty string');
  }

  const ipv6Match = trimmed.match(/^\[([^\]]+)\]:(\d+)$/);
  if (ipv6Match) {
    return {
      host: ipv6Match[1],
      port: parsePort(ipv6Match[2]),
    };
  }

  const lastColon = trimmed.lastIndexOf(':');
  if (lastColon !== -1 && trimmed.indexOf(':') !== lastColon) {
    // Likely an IPv6 literal without brackets.
    throw new Error(
      `invalid listen address "${trimmed}". Use [ipv6]:port format for IPv6 literals.`,
    );
  }

  if (lastColon !== -1) {
    return {
      host: trimmed.slice(0, lastColon) || '0.0.0.0',
      port: parsePort(trimmed.slice(lastColon + 1)),
    };
  }

  return {
    host: '0.0.0.0',
    port: parsePort(trimmed),
  };
}

function parsePort(port) {
  const value = Number.parseInt(port, 10);
  if (!Number.isInteger(value) || value < 1 || value > 65_535) {
    throw new Error(`invalid port: ${port}`);
  }
  return value;
}

export function parseOrigins(value) {
  if (!value) {
    return new Set();
  }
  if (typeof value !== 'string') {
    return normalizeAllowedOrigins(value);
  }

  const entries = value
    .split(',')
    .map((origin) => origin.trim())
    .filter(Boolean);
  return normalizeAllowedOrigins(entries);
}

function selectLogger(logger) {
  if (!logger) {
    return DEFAULT_LOGGER;
  }
  return {
    info:
      typeof logger.info === 'function'
        ? logger.info.bind(logger)
        : DEFAULT_LOGGER.info,
    warn:
      typeof logger.warn === 'function'
        ? logger.warn.bind(logger)
        : DEFAULT_LOGGER.warn,
    error:
      typeof logger.error === 'function'
        ? logger.error.bind(logger)
        : DEFAULT_LOGGER.error,
  };
}

function compactMeta(meta) {
  const result = {};
  for (const [key, value] of Object.entries(meta)) {
    if (value !== undefined && value !== null) {
      result[key] = value;
    }
  }
  return result;
}

function emitLog(logger, level, message, meta) {
  const target = logger[level] ?? logger.info ?? DEFAULT_LOGGER.info;
  target('[tryit-proxy]', message, compactMeta(meta));
}

const SYNC_HINT = "Run 'npm run sync-openapi -- --latest' from docs/portal/.";
const DEFAULT_DURATION_BUCKETS_MS = Object.freeze([50, 100, 250, 500, 1000, 2500, 5000]);

function normaliseDurationBuckets(durationBucketsMs) {
  const values = Array.from(durationBucketsMs ?? []);
  if (values.length === 0) {
    throw new Error("durationBucketsMs must contain at least one bucket boundary");
  }
  const unique = Array.from(
    new Set(
      values
        .map((value) => Number(value))
        .filter((value) => Number.isFinite(value) && value > 0),
    ),
  ).sort((a, b) => a - b);
  if (unique.length === 0) {
    throw new Error("durationBucketsMs must include positive numeric boundaries");
  }
  return unique;
}

export function createMetricsCollector({
  nowFn = () => Date.now(),
  durationBucketsMs = DEFAULT_DURATION_BUCKETS_MS,
} = {}) {
  const requestsByMethod = new Map();
  const responsesByStatus = new Map();
  const outcomes = new Map();
  const durationBuckets = normaliseDurationBuckets(durationBucketsMs);
  const durationBucketCounts = Array(durationBuckets.length + 1).fill(0);
  const state = {
    startTimeMs: nowFn(),
    inflight: 0,
    maxInflight: 0,
    durationCount: 0,
    durationSumMs: 0,
    durationMaxMs: 0,
    rateLimited: 0,
    originBlocked: 0,
    methodBlocked: 0,
    upstreamErrors: 0,
    upstreamTimeouts: 0,
    streamFailures: 0,
    bodyTooLarge: 0,
  };

  function trackRequest(method) {
    const normalisedMethod = (method ?? "").toUpperCase() || "UNKNOWN";
    requestsByMethod.set(
      normalisedMethod,
      (requestsByMethod.get(normalisedMethod) ?? 0) + 1,
    );
    state.inflight += 1;
    state.maxInflight = Math.max(state.maxInflight, state.inflight);
    const startedAt = nowFn();
    let ended = false;
    return {
      end(status, outcome) {
        if (ended) {
          return;
        }
        ended = true;
        state.inflight = Math.max(0, state.inflight - 1);
        const durationMs = Math.max(0, nowFn() - startedAt);
        state.durationCount += 1;
        state.durationSumMs += durationMs;
        state.durationMaxMs = Math.max(state.durationMaxMs, durationMs);
        const bucketIndex =
          durationBuckets.findIndex((boundary) => durationMs <= boundary) ?? -1;
        const targetIndex = bucketIndex === -1 ? durationBucketCounts.length - 1 : bucketIndex;
        durationBucketCounts[targetIndex] = (durationBucketCounts[targetIndex] ?? 0) + 1;
        if (typeof status === "number") {
          responsesByStatus.set(
            status,
            (responsesByStatus.get(status) ?? 0) + 1,
          );
        }
        if (outcome) {
          outcomes.set(outcome, (outcomes.get(outcome) ?? 0) + 1);
        }
      },
    };
  }

  function snapshot() {
    return {
      startTimeMs: state.startTimeMs,
      inflight: state.inflight,
      maxInflight: state.maxInflight,
      durationCount: state.durationCount,
      durationSumMs: state.durationSumMs,
      durationMaxMs: state.durationMaxMs,
      rateLimited: state.rateLimited,
      originBlocked: state.originBlocked,
      methodBlocked: state.methodBlocked,
      upstreamErrors: state.upstreamErrors,
      upstreamTimeouts: state.upstreamTimeouts,
      streamFailures: state.streamFailures,
      bodyTooLarge: state.bodyTooLarge,
      requestsByMethod: Array.from(requestsByMethod.entries()),
      responsesByStatus: Array.from(responsesByStatus.entries()),
      outcomes: Array.from(outcomes.entries()),
      durationBucketsMs: Array.from(durationBuckets),
      durationBucketCounts: Array.from(durationBucketCounts),
    };
  }

  function formatPrometheus() {
    const snap = snapshot();
    const lines = [];
    lines.push("# HELP tryit_proxy_requests_total Requests handled by method");
    lines.push("# TYPE tryit_proxy_requests_total counter");
    if (snap.requestsByMethod.length === 0) {
      lines.push('tryit_proxy_requests_total{method="none"} 0');
    } else {
      for (const [method, count] of snap.requestsByMethod) {
        lines.push(
          `tryit_proxy_requests_total{method="${escapeLabelValue(method)}"} ${count}`,
        );
      }
    }
    lines.push("# HELP tryit_proxy_responses_total Responses broken down by status");
    lines.push("# TYPE tryit_proxy_responses_total counter");
    if (snap.responsesByStatus.length === 0) {
      lines.push('tryit_proxy_responses_total{status="none"} 0');
    } else {
      for (const [status, count] of snap.responsesByStatus) {
        lines.push(
          `tryit_proxy_responses_total{status="${escapeLabelValue(String(status))}"} ${count}`,
        );
      }
    }
    lines.push("# HELP tryit_proxy_outcomes_total Proxy outcomes");
    lines.push("# TYPE tryit_proxy_outcomes_total counter");
    if (snap.outcomes.length === 0) {
      lines.push('tryit_proxy_outcomes_total{outcome="none"} 0');
    } else {
      for (const [outcome, count] of snap.outcomes) {
        lines.push(
          `tryit_proxy_outcomes_total{outcome="${escapeLabelValue(outcome)}"} ${count}`,
        );
      }
    }
    lines.push("# HELP tryit_proxy_rate_limited_total Requests rejected by the rate limiter");
    lines.push("# TYPE tryit_proxy_rate_limited_total counter");
    lines.push(`tryit_proxy_rate_limited_total ${snap.rateLimited}`);
    lines.push("# HELP tryit_proxy_origin_block_total Requests blocked by origin policy");
    lines.push("# TYPE tryit_proxy_origin_block_total counter");
    lines.push(`tryit_proxy_origin_block_total ${snap.originBlocked}`);
    lines.push("# HELP tryit_proxy_method_block_total Requests rejected for invalid methods");
    lines.push("# TYPE tryit_proxy_method_block_total counter");
    lines.push(`tryit_proxy_method_block_total ${snap.methodBlocked}`);
    lines.push("# HELP tryit_proxy_body_too_large_total Requests rejected for exceeding size limits");
    lines.push("# TYPE tryit_proxy_body_too_large_total counter");
    lines.push(`tryit_proxy_body_too_large_total ${snap.bodyTooLarge}`);
    lines.push("# HELP tryit_proxy_upstream_errors_total Upstream failures surfaced by the proxy");
    lines.push("# TYPE tryit_proxy_upstream_errors_total counter");
    lines.push(`tryit_proxy_upstream_errors_total ${snap.upstreamErrors}`);
    lines.push("# HELP tryit_proxy_upstream_timeouts_total Upstream timeouts seen by the proxy");
    lines.push("# TYPE tryit_proxy_upstream_timeouts_total counter");
    lines.push(`tryit_proxy_upstream_timeouts_total ${snap.upstreamTimeouts}`);
    lines.push("# HELP tryit_proxy_stream_failures_total Response stream failures");
    lines.push("# TYPE tryit_proxy_stream_failures_total counter");
    lines.push(`tryit_proxy_stream_failures_total ${snap.streamFailures}`);
    lines.push("# HELP tryit_proxy_inflight_requests Current requests being processed");
    lines.push("# TYPE tryit_proxy_inflight_requests gauge");
    lines.push(`tryit_proxy_inflight_requests ${snap.inflight}`);
    lines.push("# HELP tryit_proxy_request_duration_ms Request duration histogram in milliseconds");
    lines.push("# TYPE tryit_proxy_request_duration_ms histogram");
    const {durationBucketsMs = [], durationBucketCounts = []} = snap;
    let cumulative = 0;
    for (let idx = 0; idx < durationBucketsMs.length; idx += 1) {
      cumulative += durationBucketCounts[idx] ?? 0;
      lines.push(
        `tryit_proxy_request_duration_ms_bucket{le="${escapeLabelValue(
          durationBucketsMs[idx],
        )}"} ${cumulative}`,
      );
    }
    cumulative += durationBucketCounts[durationBucketsMs.length] ?? 0;
    lines.push(`tryit_proxy_request_duration_ms_bucket{le="+Inf"} ${cumulative}`);
    lines.push(`tryit_proxy_request_duration_ms_sum ${snap.durationSumMs.toFixed(3)}`);
    lines.push(`tryit_proxy_request_duration_ms_count ${snap.durationCount}`);
    lines.push("# HELP tryit_proxy_request_duration_ms_max Longest request duration observed in milliseconds");
    lines.push("# TYPE tryit_proxy_request_duration_ms_max gauge");
    lines.push(`tryit_proxy_request_duration_ms_max ${snap.durationMaxMs.toFixed(3)}`);
    lines.push("# HELP tryit_proxy_start_timestamp_ms Proxy start timestamp in ms since epoch");
    lines.push("# TYPE tryit_proxy_start_timestamp_ms gauge");
    lines.push(`tryit_proxy_start_timestamp_ms ${snap.startTimeMs}`);
    lines.push("");
    return lines.join("\n");
  }

  function recordRateLimit(tracker) {
    state.rateLimited += 1;
    tracker?.end(429, "rate_limited");
  }

  function recordOriginBlock(tracker) {
    state.originBlocked += 1;
    tracker?.end(403, "origin_forbidden");
  }

  function recordMethodBlock(tracker) {
    state.methodBlocked += 1;
    tracker?.end(405, "method_not_allowed");
  }

  function recordBodyLimit(tracker) {
    state.bodyTooLarge += 1;
    tracker?.end(413, "body_too_large");
  }

  function recordUpstreamError(tracker) {
    state.upstreamErrors += 1;
    tracker?.end(502, "upstream_error");
  }

  function recordUpstreamTimeout(tracker) {
    state.upstreamTimeouts += 1;
    tracker?.end(504, "upstream_timeout");
  }

  function recordStreamFailure(tracker, status) {
    state.streamFailures += 1;
    tracker?.end(status ?? 502, "stream_failure");
  }

  return {
    trackRequest,
    recordRateLimit,
    recordOriginBlock,
    recordMethodBlock,
    recordBodyLimit,
    recordUpstreamError,
    recordUpstreamTimeout,
    recordStreamFailure,
    snapshot,
    formatPrometheus,
  };
}

function escapeLabelValue(value) {
  return String(value)
    .replace(/\\/g, "\\\\")
    .replace(/"/g, '\\"')
    .replace(/\n/g, "\\n");
}

export function createRateLimiter({max, windowMs}) {
  if (!Number.isInteger(max) || max <= 0) {
    throw new Error('rate limiter max must be > 0');
  }
  if (!Number.isInteger(windowMs) || windowMs <= 0) {
    throw new Error('rate limiter window must be > 0');
  }

  const buckets = new Map();

  return {
    check(key, now = Date.now()) {
      const bucket = buckets.get(key);
      if (!bucket || bucket.resetAt <= now) {
        buckets.set(key, {count: 1, resetAt: now + windowMs});
        return {ok: true};
      }

      if (bucket.count >= max) {
        return {ok: false, retryAfter: Math.max(0, bucket.resetAt - now)};
      }

      bucket.count += 1;
      return {ok: true};
    },
    reset() {
      buckets.clear();
    },
  };
}

function shouldForwardBody(method) {
  return method !== 'GET' && method !== 'HEAD';
}

async function readRequestBody(req, maxBytes) {
  const chunks = [];
  let total = 0;

  for await (const chunk of req) {
    const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
    total += buffer.length;
    if (total > maxBytes) {
      const error = new Error('request entity too large');
      error.statusCode = 413;
      throw error;
    }
    chunks.push(buffer);
  }

  return Buffer.concat(chunks, total);
}

function normaliseHeaderValue(value) {
  if (Array.isArray(value)) {
    return value.join(', ');
  }
  if (isString(value)) {
    return value;
  }
  return undefined;
}

function assertSafeHeaderValue(name, value) {
  const normalised = normaliseHeaderValue(value);
  if (!normalised) {
    return undefined;
  }
  if (/[\r\n]/.test(normalised)) {
    const error = new Error(`invalid ${name} header: control characters are not allowed`);
    error.code = 'ERR_INVALID_HEADER';
    error.header = name;
    throw error;
  }
  return normalised;
}

const DEFAULT_CLIENT_ID = 'docs-portal';

function resolveClientId(raw, fallback) {
  const safe = assertSafeHeaderValue('x-tryit-client', raw);
  if (!safe) {
    return fallback;
  }
  const trimmed = safe.trim();
  if (trimmed === '') {
    return fallback;
  }
  return trimmed;
}

function buildUpstreamHeaders(req, {defaultBearer, allowClientAuth, clientId}) {
  const headers = new Headers();

  for (const name of FORWARDED_HEADERS) {
    const value = assertSafeHeaderValue(name, req.headers[name]);
    if (value) {
      headers.set(name, value);
    }
  }

  headers.set('user-agent', 'iroha-docs-tryit-proxy/0.1');
  const forwardedFor =
    assertSafeHeaderValue('x-forwarded-for', req.headers['x-forwarded-for']) ??
    req.socket.remoteAddress ??
    '';
  const forwardedProto =
    assertSafeHeaderValue('x-forwarded-proto', req.headers['x-forwarded-proto']) ?? 'https';
  headers.set('x-forwarded-for', forwardedFor);
  headers.set('x-forwarded-proto', forwardedProto);

  const override = assertSafeHeaderValue('x-tryit-auth', req.headers['x-tryit-auth']);
  const clientAuth = assertSafeHeaderValue('authorization', req.headers.authorization);
  let authSource = 'none';

  if (override && override.trim() !== '') {
    headers.set('authorization', formatBearer(override));
    authSource = 'override';
  } else if (defaultBearer && defaultBearer.trim() !== '') {
    headers.set('authorization', formatBearer(defaultBearer));
    authSource = 'default';
  } else if (allowClientAuth && clientAuth) {
    headers.set('authorization', clientAuth);
    authSource = 'client';
  }

  const resolvedClientId = resolveClientId(
    req.headers['x-tryit-client'],
    resolveClientId(clientId, DEFAULT_CLIENT_ID),
  );
  if (resolvedClientId) {
    headers.set('x-tryit-client', resolvedClientId);
  }

  return {headers, authSource, clientId: resolvedClientId};
}

function formatBearer(token) {
  const trimmed = token.trim();
  if (trimmed === '') {
    return '';
  }
  if (/^bearer\s+/i.test(trimmed)) {
    return trimmed;
  }
  return `Bearer ${trimmed}`;
}

function matchOrigin(origin, allowedOrigins) {
  if (!origin) {
    return null;
  }
  if (allowedOrigins.has('*')) {
    return '*';
  }
  if (allowedOrigins.has(origin)) {
    return origin;
  }
  return null;
}

function sendJson(res, statusCode, payload, corsOrigin) {
  if (corsOrigin) {
    res.setHeader('Access-Control-Allow-Origin', corsOrigin);
    res.setHeader('Vary', 'Origin');
  }
  res.setHeader('Content-Type', 'application/json');
  res.statusCode = statusCode;
  res.end(JSON.stringify(payload));
}

export function createProxyContext({
  target,
  allowedOrigins = new Set(),
  defaultBearer = '',
  allowClientAuth = false,
  clientId = DEFAULT_CLIENT_ID,
  rateLimiter,
  maxBodyBytes = 1_048_576,
  timeoutMs = 10_000,
  fetchImpl = globalThis.fetch,
  logger,
  metrics,
} = {}) {
  if (!target) {
    throw new Error('target is required');
  }
  if (!fetchImpl) {
    throw new Error('fetch implementation must be provided');
  }
  if (!rateLimiter) {
    throw new Error('rateLimiter must be provided');
  }

  const targetBase = new URL(target);
  if (!targetBase.protocol.startsWith('http')) {
    throw new Error(`unsupported target scheme: ${targetBase.protocol}`);
  }
  const normalizedOrigins = normalizeAllowedOrigins(allowedOrigins);

  return {
    targetBase,
    allowedOrigins: normalizedOrigins,
    defaultBearer,
    allowClientAuth,
    clientId,
    rateLimiter,
    maxBodyBytes,
    timeoutMs,
    fetchImpl,
    logger: selectLogger(logger),
    metrics,
  };
}

export async function proxyRequest(context, req, res) {
  const {
    targetBase,
    allowedOrigins,
    defaultBearer,
    allowClientAuth,
  rateLimiter,
  maxBodyBytes,
  timeoutMs,
  fetchImpl,
  logger,
  metrics,
  clientId,
  } = context;

  const origin = isString(req.headers.origin) ? req.headers.origin : '';
  const requestUrl = new URL(req.url ?? '/', 'http://proxy.local');
  const startTime = now();
  const baseMeta = {
    method: req.method ?? '',
    path: requestUrl.pathname,
    origin,
    remote: req.socket?.remoteAddress ?? 'unknown',
    requestId:
      req.headers['x-request-id'] ??
      req.headers['x-sora-request-id'] ??
      req.headers['x-torii-request-id'],
  };

  const tracker = metrics?.trackRequest(req.method ?? "");
  let logged = false;
  const logEvent = (level, message, status, extra = {}) => {
    if (logged) {
      return;
    }
    logged = true;
    const duration = now() - startTime;
    emitLog(logger, level, message, {
      ...baseMeta,
      status,
      durationMs: Math.round(duration * 1000) / 1000,
      ...extra,
    });
  };

  const corsOrigin = matchOrigin(origin, allowedOrigins);

  if (req.method === 'OPTIONS') {
    res.statusCode = 204;
    if (corsOrigin) {
      res.setHeader('Access-Control-Allow-Origin', corsOrigin);
      res.setHeader('Access-Control-Allow-Credentials', 'false');
      res.setHeader(
        'Access-Control-Allow-Headers',
        ALLOWED_CORS_HEADERS.join(', '),
      );
      res.setHeader(
        'Access-Control-Allow-Methods',
        Array.from(ALLOWED_METHODS).join(', '),
      );
      res.setHeader('Access-Control-Max-Age', '600');
    }
    logEvent('info', 'preflight', 204, {allowedOrigin: Boolean(corsOrigin)});
    res.end();
    tracker?.end(204, 'preflight');
    return;
  }

  if (!ALLOWED_METHODS.has(req.method ?? '')) {
    logEvent('warn', 'method_not_allowed', 405, {requestedMethod: req.method});
    sendJson(res, 405, {error: 'method_not_allowed'}, corsOrigin);
    metrics?.recordMethodBlock(tracker);
    return;
  }

  if (!corsOrigin) {
    logEvent('warn', 'origin_forbidden', 403, {});
    sendJson(res, 403, {error: 'origin_forbidden'}, null);
    metrics?.recordOriginBlock(tracker);
    return;
  }

  const clientKey = `${req.socket?.remoteAddress ?? 'unknown'}`;
  const rate = rateLimiter.check(clientKey);
  if (!rate.ok) {
    if (rate.retryAfter != null) {
      res.setHeader('Retry-After', Math.ceil(rate.retryAfter / 1000));
    }
    logEvent('warn', 'rate_limited', 429, {retryAfterMs: rate.retryAfter ?? 0});
    sendJson(res, 429, {error: 'rate_limited'}, corsOrigin);
     metrics?.recordRateLimit(tracker);
    return;
  }

  if (requestUrl.pathname === '/healthz') {
    logEvent('info', 'healthcheck', 200, {upstream: targetBase.origin});
    sendJson(
      res,
      200,
      {
        status: 'ok',
        target: targetBase.origin,
      },
      corsOrigin,
    );
    tracker?.end(200, 'healthz');
    return;
  }

  if (!requestUrl.pathname.startsWith('/proxy/')) {
    logEvent('warn', 'unknown_route', 404, {});
    sendJson(res, 404, {error: 'not_found'}, corsOrigin);
    tracker?.end(404, 'not_found');
    return;
  }

  const upstreamPath = requestUrl.pathname.slice('/proxy'.length) || '/';
  const upstreamUrl = new URL(upstreamPath + requestUrl.search, targetBase);
  let headers;
  let authSource;
  let resolvedClientId;
  try {
    ({headers, authSource, clientId: resolvedClientId} = buildUpstreamHeaders(req, {
      defaultBearer,
      allowClientAuth,
      clientId,
    }));
  } catch (error) {
    logEvent('warn', 'bad_request', 400, {
      stage: 'headers',
      error: error.message ?? 'bad_request',
    });
    sendJson(res, 400, {error: 'bad_request'}, corsOrigin);
    tracker?.end(400, 'client_error');
    return;
  }

  let body;
  let requestBytes = 0;
  try {
    if (shouldForwardBody(req.method ?? '')) {
      body = await readRequestBody(req, maxBodyBytes);
      requestBytes = body.length;
    }
  } catch (error) {
    const status = error.statusCode ?? 400;
    logEvent('warn', 'bad_request', status, {
      stage: 'read_body',
      error: error.message ?? 'bad_request',
    });
    sendJson(res, status, {error: error.message ?? 'bad_request'}, corsOrigin);
    if (status === 413) {
      metrics?.recordBodyLimit(tracker);
    } else {
      tracker?.end(status, 'client_error');
    }
    return;
  }

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  let upstreamResponse;
  try {
    upstreamResponse = await fetchImpl(upstreamUrl, {
      method: req.method,
      headers,
      body,
      redirect: 'manual',
      signal: controller.signal,
    });
  } catch (error) {
    clearTimeout(timeout);
    if (error.name === 'AbortError') {
      logEvent('error', 'upstream_timeout', 504, {timeoutMs});
      sendJson(res, 504, {error: 'upstream_timeout'}, corsOrigin);
      metrics?.recordUpstreamTimeout(tracker);
    } else {
      logEvent('error', 'upstream_error', 502, {
        error: error.message ?? String(error),
      });
      sendJson(
        res,
        502,
        {error: 'upstream_error', detail: error.message ?? String(error)},
        corsOrigin,
      );
      metrics?.recordUpstreamError(tracker);
    }
    return;
  }

  clearTimeout(timeout);

  res.statusCode = upstreamResponse.status;
  res.statusMessage = upstreamResponse.statusText;
  res.setHeader('Access-Control-Allow-Origin', corsOrigin);
  res.setHeader(
    'Access-Control-Expose-Headers',
    'Content-Type, X-Request-ID, X-Sora-Request-ID, X-Sora-Trace-Id',
  );
  res.setHeader('Vary', 'Origin');
  res.setHeader('X-TryIt-Upstream', upstreamUrl.origin);

  for (const [name, value] of upstreamResponse.headers.entries()) {
    const lower = name.toLowerCase();
    if (
      lower === 'content-type' ||
      lower === 'content-length' ||
      lower.startsWith('x-request-id') ||
      lower.startsWith('x-sora-')
    ) {
      res.setHeader(name, value);
    }
  }

  let responseBody = upstreamResponse.body;
  if (!responseBody) {
    res.end();
    logEvent('info', 'forward_success', upstreamResponse.status, {
      upstream: upstreamUrl.origin,
      authSource,
      clientId: resolvedClientId,
      requestBytes,
    });
    tracker?.end(upstreamResponse.status, 'success');
    return;
  }

  if (typeof responseBody.getReader === 'function') {
    responseBody = Readable.fromWeb(responseBody);
  }

  try {
    await pipeline(responseBody, res);
    logEvent('info', 'forward_success', upstreamResponse.status, {
      upstream: upstreamUrl.origin,
      authSource,
      clientId: resolvedClientId,
      requestBytes,
    });
    tracker?.end(upstreamResponse.status, 'success');
  } catch (error) {
    logEvent('warn', 'stream_failure', upstreamResponse.status, {
      error: error.message ?? String(error),
    });
    metrics?.recordStreamFailure(tracker, upstreamResponse.status);
    req.destroy?.(error);
  }
}

export function createProxyServer(options = {}) {
  const context = createProxyContext(options);
  const {logger} = context;
  const server = createServer((req, res) => {
    proxyRequest(context, req, res).catch((error) => {
      emitLog(logger, 'error', 'unhandled_exception', {
        error: error.message ?? String(error),
      });
      if (!res.headersSent) {
        sendJson(res, 500, {error: 'internal_error'}, null);
      } else {
        res.end();
      }
    });
  });

  server.on('clientError', (err, socket) => {
    socket.end('HTTP/1.1 400 Bad Request\r\n\r\n');
    emitLog(logger, 'warn', 'client_error', {
      error: err.message ?? String(err),
    });
  });

  server.on('error', (err) => {
    emitLog(logger, 'error', 'server_error', {
      error: err.message ?? String(err),
    });
  });

  return server;
}

export function createMetricsServer({
  collector,
  path = "/metrics",
  logger,
} = {}) {
  if (!collector) {
    throw new Error("collector is required");
  }
  const resolvedPath = path.startsWith("/") ? path : `/${path}`;
  const targetLogger = selectLogger(logger);
  const server = createServer((req, res) => {
    try {
      const requestUrl = new URL(req.url ?? "/", "http://metrics.local");
      if (
        (req.method !== "GET" && req.method !== "HEAD") ||
        requestUrl.pathname !== resolvedPath
      ) {
        res.statusCode = 404;
        res.setHeader("Content-Type", "text/plain");
        res.end("not_found");
        return;
      }
      const payload = collector.formatPrometheus();
      res.statusCode = 200;
      res.setHeader("Content-Type", "text/plain; version=0.0.4");
      if (req.method === "HEAD") {
        res.end();
        return;
      }
      res.end(payload);
    } catch (error) {
      emitLog(targetLogger, "error", "metrics_error", {
        error: error.message ?? String(error),
      });
      if (!res.headersSent) {
        res.statusCode = 500;
      }
      res.end("metrics_error");
    }
  });

  server.on("clientError", (err, socket) => {
    socket.end("HTTP/1.1 400 Bad Request\r\n\r\n");
    emitLog(targetLogger, "warn", "metrics_client_error", {
      error: err.message ?? String(err),
    });
  });

  server.on("error", (err) => {
    emitLog(targetLogger, "error", "metrics_server_error", {
      error: err.message ?? String(err),
    });
  });

  return server;
}

export async function verifySpecDigest({
  specPath,
  manifestPath,
  readFileImpl = readFile,
} = {}) {
  if (!specPath) {
    throw new Error('specPath is required');
  }
  if (!manifestPath) {
    throw new Error('manifestPath is required');
  }

  const [specBuffer, manifestRaw] = await Promise.all([
    readFileImpl(specPath),
    readFileImpl(manifestPath, 'utf8'),
  ]);

  let manifest;
  try {
    manifest = JSON.parse(manifestRaw);
  } catch (error) {
    throw new Error(
      `failed to parse OpenAPI manifest at ${manifestPath}: ${error.message ?? error}`,
    );
  }

  const artifact = manifest?.artifact;
  if (!artifact || typeof artifact !== 'object') {
    throw new Error(`OpenAPI manifest ${manifestPath} is missing artifact metadata`);
  }

  const expectedDigest = normaliseHex(
    artifact.sha256_hex ?? artifact.sha256Hex ?? artifact.sha256,
    'artifact.sha256_hex',
  );
  const actualDigest = createHash('sha256').update(specBuffer).digest('hex');

  if (actualDigest !== expectedDigest) {
    const hint = `OpenAPI spec digest mismatch for ${specPath}. Expected ${expectedDigest} but found ${actualDigest}. ${SYNC_HINT}`;
    const error = new Error(hint);
    error.code = 'ERR_SPEC_DIGEST_MISMATCH';
    error.expected = expectedDigest;
    error.actual = actualDigest;
    throw error;
  }

  if (Number.isFinite(artifact.bytes)) {
    const expectedBytes = Number(artifact.bytes);
    const actualBytes = specBuffer.length;
    if (actualBytes !== expectedBytes) {
      const error = new Error(
        `OpenAPI spec size mismatch for ${specPath}. Expected ${expectedBytes} bytes but found ${actualBytes}. ${SYNC_HINT}`,
      );
      error.code = 'ERR_SPEC_SIZE_MISMATCH';
      error.expectedBytes = expectedBytes;
      error.actualBytes = actualBytes;
      throw error;
    }
  }

  const signature = artifact.signature;
  if (!signature) {
    const error = new Error(
      `OpenAPI manifest ${manifestPath} is missing artifact.signature. ${SYNC_HINT}`,
    );
    error.code = 'ERR_SPEC_SIGNATURE_MISSING';
    throw error;
  }
  const signatureAlgorithm = signature.algorithm;
  const signaturePublicKey =
    signature.public_key_hex ?? signature.publicKeyHex ?? signature.public_key;
  const signatureHex =
    signature.signature_hex ?? signature.signatureHex ?? signature.value;
  try {
    verifyOpenApiSignature({
      algorithm: signatureAlgorithm,
      publicKeyHex: signaturePublicKey,
      signatureHex,
      payload: specBuffer,
    });
  } catch (error) {
    const err = new Error(
      `OpenAPI manifest signature verification failed: ${error.message ?? error}. ${SYNC_HINT}`,
    );
    err.code = 'ERR_SPEC_SIGNATURE_INVALID';
    throw err;
  }

  return {
    algorithm: 'sha256',
    bytes: specBuffer.length,
    expected: expectedDigest,
    actual: actualDigest,
  };
}

function normaliseHex(value, fieldName) {
  if (typeof value !== 'string' || value.trim() === '') {
    throw new Error(`OpenAPI manifest is missing ${fieldName}`);
  }
  return value.trim().toLowerCase();
}
