#!/usr/bin/env node

import process from 'node:process';
import path from 'node:path';
import {mkdir, rename, writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';

const DEFAULT_PROXY_URL = 'http://localhost:8787';
const LABEL_NAME_RE = /^[A-Za-z_][A-Za-z0-9_]*$/;
const DEFAULT_METRIC_LABELS = Object.freeze({job: 'tryit-proxy'});

export class ProbeError extends Error {
  constructor(message, {details} = {}) {
    super(message);
    this.name = 'ProbeError';
    this.details = details ?? null;
  }
}

function normalisePath(value) {
  const trimmed = (value ?? '').trim();
  if (trimmed === '') {
    return '';
  }
  if (trimmed.startsWith('http://') || trimmed.startsWith('https://')) {
    return trimmed;
  }
  return trimmed.startsWith('/') ? trimmed : `/${trimmed}`;
}

function toInt(value, fallback) {
  if (value == null || value === '') {
    return fallback;
  }
  const parsed = Number.parseInt(value, 10);
  return Number.isNaN(parsed) ? fallback : parsed;
}

export function parseLabelOverrides(value) {
  const overrides = {};
  const raw = (value ?? '').trim();
  if (raw === '') {
    return overrides;
  }

  for (const part of raw.split(',')) {
    const [rawName, ...rest] = part.split('=');
    if (!rawName || rest.length === 0) {
      continue;
    }
    const name = rawName.trim();
    if (!LABEL_NAME_RE.test(name)) {
      continue;
    }
    const labelValue = rest.join('=').trim();
    if (labelValue === '') {
      continue;
    }
    overrides[name] = labelValue;
  }

  return overrides;
}

function escapeLabelValue(value) {
  return value.replace(/\\/g, '\\\\').replace(/\n/g, '\\n').replace(/"/g, '\\"');
}

export function formatPrometheusLabels(labels) {
  if (!labels) {
    return '';
  }
  const entries = [];
  for (const [key, value] of Object.entries(labels)) {
    if (!LABEL_NAME_RE.test(key)) {
      continue;
    }
    if (value === undefined || value === null || value === '') {
      continue;
    }
    entries.push([key, String(value)]);
  }

  if (entries.length === 0) {
    return '';
  }

  entries.sort((a, b) => a[0].localeCompare(b[0]));
  const rendered = entries
    .map(([key, value]) => `${key}="${escapeLabelValue(value)}"`)
    .join(',');
  return `{${rendered}}`;
}

function buildMetricsPayload({labels, success, durationSeconds}) {
  const safeDuration =
    Number.isFinite(durationSeconds) && durationSeconds >= 0 ? durationSeconds : 0;
  const labelString = formatPrometheusLabels(labels);
  return [
    '# HELP probe_success 1 indicates success, 0 indicates failure',
    '# TYPE probe_success gauge',
    `probe_success${labelString} ${success ? 1 : 0}`,
    '# HELP probe_duration_seconds Duration of the probe in seconds',
    '# TYPE probe_duration_seconds gauge',
    `probe_duration_seconds${labelString} ${safeDuration.toFixed(3)}`,
    '',
  ].join('\n');
}

export async function writeProbeMetrics({
  metricsPath,
  labels,
  success,
  durationSeconds,
}) {
  if (!metricsPath) {
    return;
  }
  const payload = buildMetricsPayload({labels, success, durationSeconds});
  const directory = path.dirname(metricsPath);
  await mkdir(directory, {recursive: true});

  const tempPath = path.join(
    directory,
    `.tryit-probe-${process.pid}-${Date.now()}.tmp`,
  );
  await writeFile(tempPath, payload);
  await rename(tempPath, metricsPath);
}

function computeDurationSeconds(startTime) {
  if (typeof startTime !== 'bigint') {
    return 0;
  }
  const delta = process.hrtime.bigint() - startTime;
  return Number(delta) / 1_000_000_000;
}

export async function runProbe({
  proxyUrl,
  samplePath,
  method,
  timeoutMs,
  token,
  fetchImpl = globalThis.fetch,
}) {
  if (typeof fetchImpl !== 'function') {
    throw new ProbeError('fetch implementation is not available');
  }

  const headers = new Headers({
    'User-Agent': 'iroha-docs-tryit-probe/0.1',
  });

  const healthUrl = `${proxyUrl}/healthz`;
  let healthResponse;
  try {
    healthResponse = await fetchWithTimeout(healthUrl, {
      timeoutMs,
      headers,
      fetchImpl,
    });
  } catch (error) {
    throw new ProbeError(
      `healthz request failed: ${error.message ?? String(error)}`,
    );
  }

  if (!healthResponse.ok) {
    throw new ProbeError(
      `healthz returned ${healthResponse.status} ${healthResponse.statusText}`,
      {details: await safeReadBody(healthResponse)},
    );
  }

  if (samplePath === '') {
    return;
  }

  const requestUrl = samplePath.startsWith('http')
    ? samplePath
    : `${proxyUrl}/proxy${samplePath}`;

  const requestHeaders = new Headers({
    'X-TryIt-Client': 'tryit-probe',
    Accept: 'application/json',
  });
  if (token.trim() !== '') {
    requestHeaders.set('X-TryIt-Auth', token.trim());
  }

  let proxyResponse;
  try {
    proxyResponse = await fetchWithTimeout(requestUrl, {
      method,
      timeoutMs,
      headers: requestHeaders,
      fetchImpl,
    });
  } catch (error) {
    throw new ProbeError(
      `sample request failed: ${error.message ?? String(error)}`,
    );
  }

  if (!proxyResponse.ok) {
    const detail = await safeReadBody(proxyResponse);
    throw new ProbeError(
      `sample request failed with ${proxyResponse.status} ${proxyResponse.statusText}`,
      {details: detail},
    );
  }
}

export async function verifyMetricsEndpoint({
  metricsUrl,
  timeoutMs,
  fetchImpl = globalThis.fetch,
}) {
  if (!metricsUrl) {
    throw new ProbeError('metricsUrl is required');
  }
  let response;
  try {
    response = await fetchWithTimeout(metricsUrl, {
      timeoutMs,
      fetchImpl,
      headers: {'User-Agent': 'iroha-docs-tryit-probe/0.1'},
    });
  } catch (error) {
    throw new ProbeError(
      `metrics request failed: ${error.message ?? String(error)}`,
    );
  }

  if (!response.ok) {
    throw new ProbeError(
      `metrics endpoint returned ${response.status} ${response.statusText}`,
      {details: await safeReadBody(response)},
    );
  }

  const body = await safeReadBody(response);
  if (!/tryit_proxy_requests_total/.test(body)) {
    throw new ProbeError('metrics payload missing tryit_proxy_requests_total', {
      details: body.slice(0, 256),
    });
  }
}

async function fetchWithTimeout(url, {timeoutMs, fetchImpl, ...options}) {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await fetchImpl(url, {...options, signal: controller.signal});
  } finally {
    clearTimeout(timeout);
  }
}

async function safeReadBody(response) {
  try {
    return await response.text();
  } catch (error) {
    return error?.message ?? String(error);
  }
}

function logProbeError(error) {
  if (error instanceof ProbeError) {
    console.error(`[tryit-proxy-probe] ${error.message}`);
    if (error.details) {
      console.error(error.details);
    }
    return;
  }
  console.error('[tryit-proxy-probe] unexpected error:', error);
}

async function main() {
  const proxyUrl = (process.env.TRYIT_PROXY_PUBLIC_URL ?? DEFAULT_PROXY_URL).replace(
    /\/$/,
    '',
  );
  const samplePath = normalisePath(process.env.TRYIT_PROXY_SAMPLE_PATH ?? '');
  const method = (process.env.TRYIT_PROXY_SAMPLE_METHOD ?? 'GET').toUpperCase();
  const timeoutMs = toInt(process.env.TRYIT_PROXY_PROBE_TIMEOUT_MS, 5_000);
  const token = process.env.TRYIT_PROXY_PROBE_TOKEN ?? '';
  const metricsPath = (process.env.TRYIT_PROXY_PROBE_METRICS_FILE ?? '').trim();
  const labelOverrides = parseLabelOverrides(
    process.env.TRYIT_PROXY_PROBE_LABELS ?? '',
  );
  const labels = {
    ...DEFAULT_METRIC_LABELS,
    instance: proxyUrl,
    ...labelOverrides,
  };
  const metricsProbeUrl = (process.env.TRYIT_PROXY_PROBE_METRICS_URL ?? '').trim();

  const startTime = process.hrtime.bigint();
  let success = false;
  try {
    await runProbe({
      proxyUrl,
      samplePath,
      method,
      timeoutMs,
      token,
      fetchImpl: globalThis.fetch,
    });
    if (metricsProbeUrl !== '') {
      await verifyMetricsEndpoint({
        metricsUrl: metricsProbeUrl,
        timeoutMs,
        fetchImpl: globalThis.fetch,
      });
    }
    success = true;
    console.log('[tryit-proxy-probe] proxy healthy');
  } catch (error) {
    logProbeError(error);
  } finally {
    if (metricsPath) {
      await writeProbeMetrics({
        metricsPath,
        labels,
        success,
        durationSeconds: computeDurationSeconds(startTime),
      }).catch((error) => {
        console.error(
          `[tryit-proxy-probe] failed to write metrics: ${error.message ?? error}`,
        );
      });
    }
  }

  if (!success) {
    process.exit(1);
  }
}

const isCli =
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url);

if (isCli) {
  main().catch((error) => {
    console.error('[tryit-proxy-probe] unexpected failure:', error);
    process.exit(1);
  });
}
