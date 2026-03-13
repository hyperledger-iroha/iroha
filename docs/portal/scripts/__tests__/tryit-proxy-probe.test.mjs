import assert from 'node:assert/strict';
import {mkdtemp, readFile, rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
  parseLabelOverrides,
  formatPrometheusLabels,
  writeProbeMetrics,
  runProbe,
  ProbeError,
  verifyMetricsEndpoint,
} from '../tryit-proxy-probe.mjs';

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
    samplePath: '/v2/status',
    method: 'GET',
    timeoutMs: 1_000,
    token: '',
    fetchImpl: fetchStub,
  });

  assert.equal(calls.length, 2);
  assert.ok(calls[0].url.endsWith('/healthz'));
  assert.ok(calls[1].url.endsWith('/proxy/v2/status'));
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
        samplePath: '/v2/status',
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
