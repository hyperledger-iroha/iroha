#!/usr/bin/env node

import {once} from 'node:events';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

import {
  createMetricsCollector,
  createMetricsServer,
  createProxyServer,
  createRateLimiter,
  parseListenAddress,
  parseOrigins,
  verifySpecDigest,
} from './tryit-proxy-lib.mjs';

const PORTAL_ROOT = fileURLToPath(new URL('../', import.meta.url));
const SPEC_PATH = path.join(PORTAL_ROOT, 'static/openapi/torii.json');
const MANIFEST_PATH = path.join(PORTAL_ROOT, 'static/openapi/manifest.json');

async function main() {
  const target = process.env.TRYIT_PROXY_TARGET;
  if (!target) {
    console.error('TRYIT_PROXY_TARGET must point to the Torii base URL (e.g., https://torii.example)');
    process.exitCode = 1;
    return;
  }

  const listen = safeParse(() => parseListenAddress(process.env.TRYIT_PROXY_LISTEN ?? '127.0.0.1:8787'));
  if (!listen) {
    process.exitCode = 1;
    return;
  }

  const allowedOrigins =
    parseOrigins(process.env.TRYIT_PROXY_ALLOWED_ORIGINS ?? 'http://localhost:3000');

  const rateLimiter = safeParse(() =>
    createRateLimiter({
      max: toInt(process.env.TRYIT_PROXY_RATE_LIMIT, 60),
      windowMs: toInt(process.env.TRYIT_PROXY_RATE_WINDOW_MS, 60_000),
    }),
  );
  if (!rateLimiter) {
    process.exitCode = 1;
    return;
  }

  if (!(await ensureSpecFresh())) {
    process.exitCode = 1;
    return;
  }

  const metricsCollector = createMetricsCollector();
  const server = createProxyServer({
    target,
    allowedOrigins,
    defaultBearer: process.env.TRYIT_PROXY_BEARER ?? '',
    allowClientAuth: process.env.TRYIT_PROXY_ALLOW_CLIENT_AUTH === '1',
    clientId: process.env.TRYIT_PROXY_CLIENT_ID ?? 'docs-portal',
    rateLimiter,
    maxBodyBytes: toInt(process.env.TRYIT_PROXY_MAX_BODY, 1_048_576),
    timeoutMs: toInt(process.env.TRYIT_PROXY_TIMEOUT_MS, 10_000),
    metrics: metricsCollector,
  });

  server.listen(listen.port, listen.host);
  await once(server, 'listening');

  const address = server.address();
  if (address && typeof address === 'object') {
    console.log(
      `[tryit-proxy] listening on http://${address.address}:${address.port} → ${target}`,
    );
  } else {
    console.log('[tryit-proxy] listening');
  }

  const metricsServers = [];
  const metricsListenRaw = (process.env.TRYIT_PROXY_METRICS_LISTEN ?? '').trim();
  if (metricsListenRaw !== '') {
    const metricsListen = safeParse(() => parseListenAddress(metricsListenRaw));
    if (!metricsListen) {
      server.close();
      process.exitCode = 1;
      return;
    }
    const metricsPath = process.env.TRYIT_PROXY_METRICS_PATH ?? '/metrics';
    const metricsServer = createMetricsServer({
      collector: metricsCollector,
      path: metricsPath,
    });
    metricsServer.listen(metricsListen.port, metricsListen.host);
    await once(metricsServer, 'listening');
    metricsServers.push(metricsServer);
    console.log(
      `[tryit-proxy] metrics on http://${metricsListen.host}:${metricsListen.port}${metricsPath}`,
    );
  }

  const activeServers = [server, ...metricsServers];
  process.on('SIGINT', () => shutdown(activeServers));
  process.on('SIGTERM', () => shutdown(activeServers));
}

function toInt(value, fallback) {
  if (!value) {
    return fallback;
  }
  const parsed = Number.parseInt(value, 10);
  if (Number.isNaN(parsed)) {
    return fallback;
  }
  return parsed;
}

function safeParse(fn) {
  try {
    return fn();
  } catch (error) {
    console.error('[tryit-proxy]', error.message ?? error);
    return null;
  }
}

function shutdown(servers) {
  console.log('[tryit-proxy] shutting down...');
  let remaining = servers.length;
  const closeHandler = () => {
    remaining -= 1;
    if (remaining <= 0) {
      process.exit(0);
    }
  };
  for (const srv of servers) {
    srv.close(closeHandler);
  }
}

async function ensureSpecFresh() {
  try {
    const {actual} = await verifySpecDigest({
      specPath: SPEC_PATH,
      manifestPath: MANIFEST_PATH,
    });
    console.log(`[tryit-proxy] OpenAPI spec verified (${actual})`);
    return true;
  } catch (error) {
    if (process.env.TRYIT_PROXY_ALLOW_STALE_SPEC === '1') {
      console.warn(
        `[tryit-proxy] WARNING: ${error.message} (TRYIT_PROXY_ALLOW_STALE_SPEC=1)`,
      );
      return true;
    }
    console.error(`[tryit-proxy] ${error.message ?? error}`);
    return false;
  }
}

main().catch((error) => {
  console.error('[tryit-proxy] fatal error:', error);
  process.exitCode = 1;
});
