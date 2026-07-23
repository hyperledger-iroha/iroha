import assert from 'node:assert/strict';
import {mkdtemp, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {fileURLToPath} from 'node:url';

import {
  parseLabelOverrides,
  formatPrometheusLabels,
  writeProbeMetrics,
  runProbe,
  ProbeError,
  verifyMetricsEndpoint,
} from '../tryit-proxy-probe.mjs';

const REPO_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../..',
);
const PUBLISHING_MONITOR_LOCALES = Object.freeze([
  'am',
  'ar',
  'az',
  'ba',
  'dz',
  'es',
  'fr',
  'he',
  'hy',
  'ja',
  'ka',
  'kk',
  'mn',
  'my',
  'pt',
  'ru',
  'ur',
  'uz',
  'zh-hans',
  'zh-hant',
]);

test('parseLabelOverrides keeps valid key/value pairs', () => {
  const overrides = parseLabelOverrides('job=prod,env=qa,invalid,1bad=value');
  assert.deepEqual(overrides, {
    job: 'prod',
    env: 'qa',
  });

  const formatted = formatPrometheusLabels({
    instance: 'http://proxy',
    ...overrides,
  });
  assert.equal(formatted, '{env="qa",instance="http://proxy",job="prod"}');
});

test('writeProbeMetrics emits Prometheus textfile payload', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'tryit-probe-metrics-'));
  try {
    const metricsPath = path.join(dir, 'probe.prom');
    await writeProbeMetrics({
      metricsPath,
      labels: {job: 'tryit-proxy', instance: 'https://docs'},
      success: true,
      durationSeconds: 0.2345,
    });
    const contents = await readFile(metricsPath, 'utf8');
    assert.match(contents, /# HELP probe_success/);
    assert.match(contents, /probe_success\{.*instance="https:\/\/docs".*\} 1/);
    assert.match(contents, /probe_duration_seconds\{.*\} 0\.\d{3}/);
  } finally {
    await rm(dir, {recursive: true, force: true});
  }
});

test('runProbe succeeds when health and sample endpoints return 200', async () => {
  const responses = [
    createResponse(200, 'ok'),
    createResponse(200, '{"status":"ok"}'),
  ];
  const calls = [];
  const fetchStub = async (url, options) => {
    calls.push({url, method: options?.method ?? 'GET'});
    return responses.shift();
  };

  await runProbe({
    proxyUrl: 'https://proxy.test',
    samplePath: '/v1/status',
    method: 'GET',
    timeoutMs: 1_000,
    token: '',
    fetchImpl: fetchStub,
  });

  assert.equal(calls.length, 2);
  assert.equal(calls[0].url, 'https://proxy.test/healthz');
  assert.equal(calls[1].url, 'https://proxy.test/proxy/v1/status');
});

test('checked-in monitor sample path receives exactly one proxy prefix', async () => {
  const configPath = path.join(REPO_ROOT, 'configs', 'docs_monitor.json');
  const config = JSON.parse(await readFile(configPath, 'utf8'));
  const calls = [];
  const fetchStub = async (url) => {
    calls.push(url);
    return createResponse(200, 'ok');
  };

  await runProbe({
    proxyUrl: config.tryIt.proxyUrl,
    samplePath: config.tryIt.samplePath,
    method: config.tryIt.method,
    timeoutMs: config.tryIt.timeoutMs,
    token: '',
    fetchImpl: fetchStub,
  });

  assert.equal(calls.length, 2);
  assert.equal(calls[1], `${config.tryIt.proxyUrl}/proxy${config.tryIt.samplePath}`);
  const proxySegments = new URL(calls[1]).pathname
    .split('/')
    .filter((segment) => segment === 'proxy');
  assert.equal(proxySegments.length, 1);
});

test('publishing monitor docs keep the sample path contract synchronized', async () => {
  const sourceDirectory = path.join(REPO_ROOT, 'docs', 'portal', 'docs', 'devportal');
  const i18nDirectory = path.join(REPO_ROOT, 'docs', 'portal', 'i18n');
  const sourcePaths = [
    path.join(sourceDirectory, 'publishing-monitoring.md'),
    ...PUBLISHING_MONITOR_LOCALES.map((locale) =>
      path.join(sourceDirectory, `publishing-monitoring.${locale}.md`),
    ),
    ...PUBLISHING_MONITOR_LOCALES.map((locale) =>
      path.join(
        i18nDirectory,
        locale,
        'docusaurus-plugin-content-docs',
        'current',
        'devportal',
        'publishing-monitoring.md',
      ),
    ),
  ];
  const expectedSamplePath =
    '"samplePath": "/v1/accounts/<i105-account-id>/assets?limit=1"';
  const expectedContract =
    '> `samplePath=/v1/...` → probe `+ /proxy` → `/proxy/v1/...`';

  for (const sourcePath of sourcePaths) {
    const source = await readFile(sourcePath, 'utf8');
    const relativePath = path.relative(REPO_ROOT, sourcePath);
    assert.equal(
      source.split(expectedSamplePath).length - 1,
      1,
      `${relativePath}: expected one Torii-relative samplePath`,
    );
    assert.equal(
      source.split(expectedContract).length - 1,
      1,
      `${relativePath}: expected one proxy-prefix contract note`,
    );
    assert.doesNotMatch(
      source,
      /"samplePath": "\/proxy\//u,
      `${relativePath}: samplePath must omit the proxy prefix`,
    );
  }
});

test('runProbe throws ProbeError when the sample request fails', async () => {
  const fetchStub = async (url) => {
    if (url.endsWith('/healthz')) {
      return createResponse(200, 'ok');
    }
    return createResponse(503, 'nope', 'ServiceUnavailable');
  };

  await assert.rejects(
    () =>
      runProbe({
        proxyUrl: 'https://proxy.test',
        samplePath: '/v1/status',
        method: 'POST',
        timeoutMs: 1_000,
        token: '',
        fetchImpl: fetchStub,
      }),
    (error) => error instanceof ProbeError && /sample request failed/i.test(error.message),
  );
});

test('verifyMetricsEndpoint succeeds when payload contains counters', async () => {
  let called = false;
  const fetchStub = async () => {
    called = true;
    return createResponse(
      200,
      '# HELP tryit_proxy_requests_total\\ntryit_proxy_requests_total{method=\"GET\"} 42',
    );
  };
  await verifyMetricsEndpoint({
    metricsUrl: 'http://localhost:9798/metrics',
    timeoutMs: 1_000,
    fetchImpl: fetchStub,
  });
  assert.ok(called);
});

test('verifyMetricsEndpoint rejects payloads without counters', async () => {
  const fetchStub = async () => createResponse(200, '# HELP other_metric x');
  await assert.rejects(
    () =>
      verifyMetricsEndpoint({
        metricsUrl: 'http://localhost:9798/metrics',
        timeoutMs: 1_000,
        fetchImpl: fetchStub,
      }),
    (error) => error instanceof ProbeError && /metrics payload missing/i.test(error.message),
  );
});

function createResponse(status, body, statusText = 'OK') {
  return {
    status,
    statusText,
    ok: status >= 200 && status < 300,
    async text() {
      return body;
    },
  };
}
