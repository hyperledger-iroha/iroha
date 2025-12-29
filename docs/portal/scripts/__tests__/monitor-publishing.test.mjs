import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, writeFile, readFile, rm} from 'node:fs/promises';
import path from 'node:path';
import {tmpdir} from 'node:os';

import {
  loadMonitorConfig,
  monitorPortal,
  monitorTryIt,
  monitorBinding,
  monitorDnsRecords,
  runMonitor,
  writeEvidenceBundle,
  renderPrometheusMetrics,
} from '../monitor-publishing.mjs';

test('loadMonitorConfig parses config file', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'monitor-config-'));
  const file = path.join(dir, 'config.json');
  await writeFile(file, JSON.stringify({portal: {baseUrl: 'https://docs.sora'}}), 'utf8');
  const config = await loadMonitorConfig(file);
  assert.equal(config.portal.baseUrl, 'https://docs.sora');
  await rm(dir, {recursive: true, force: true});
});

test('monitorPortal reports release mismatches', async () => {
  const config = {
    baseUrl: 'https://docs.sora',
    paths: ['/'],
    expectRelease: 'release-A',
  };
  const result = await monitorPortal(config, {
    probePathImpl: async () => ({
      url: 'https://docs.sora/',
      status: 200,
      ok: true,
      release: 'release-B',
      durationMs: 5,
      security: {},
    }),
    validateSecurityImpl: () => [],
  });
  assert.equal(result.ok, false);
  assert.equal(result.entries.length, 1);
  assert.match(result.entries[0].failures[0], /release mismatch/);
});

test('monitorPortal rejects insecure baseUrl without opt-in', async () => {
  const result = await monitorPortal({baseUrl: 'http://localhost:3000'});
  assert.equal(result.skipped, false);
  assert.equal(result.ok, false);
  assert.equal(result.entries.length, 1);
  assert.match(result.entries[0].failures[0], /insecure baseUrl rejected/);
});

test('monitorPortal allows insecure baseUrl when explicitly enabled', async () => {
  let probed = false;
  const result = await monitorPortal(
    {baseUrl: 'http://localhost:3000', allowInsecureHttp: true},
    {
      probePathImpl: async () => {
        probed = true;
        return {
          url: 'http://localhost:3000/',
          status: 200,
          ok: true,
          release: 'dev',
          durationMs: 3,
          security: {},
        };
      },
      validateSecurityImpl: () => [],
    },
  );
  assert.equal(result.ok, true);
  assert.equal(probed, true);
});

test('monitorTryIt surfaces probe failures', async () => {
  const config = {
    proxyUrl: 'https://tryit.sora',
    samplePath: '/proxy/status',
  };
  const result = await monitorTryIt(config, {
    runProbeImpl: async () => {
      throw new Error('probe failed');
    },
  });
  assert.equal(result.ok, false);
  assert.match(result.error, /probe failed/);
});

test('monitorBinding wraps successful verification', async () => {
  const summary = {
    url: 'https://docs.sora/.well-known/sorafs/manifest',
    statusCode: 200,
    headers: {},
    proof: {alias: 'docs.sora.link'},
  };
  let captured;
  const result = await monitorBinding(
    {url: summary.url, contentCid: 'bafytestcid'},
    {
      verifyBindingImpl: async (options) => {
        captured = options;
        return summary;
      },
    },
  );
  assert.equal(result.ok, true);
  assert.equal(result.entries.length, 1);
  assert.equal(result.entries[0].ok, true);
  assert.deepEqual(result.entries[0].summary, summary);
  assert.equal(captured.expectedContentCid, 'bafytestcid');
});

test('monitorBinding supports multiple bindings and host validation', async () => {
  const results = [
    {
      url: 'https://docs.sora/.well-known/sorafs/manifest',
      headers: {'sora-name': 'docs.sora', 'sora-proof-status': 'ok', 'sora-content-cid': 'cid1'},
      proof: {manifest: 'man1'},
    },
    {
      url: 'https://docs.sora/static/openapi.json',
      headers: {'sora-name': 'docs.sora', 'sora-proof-status': 'ok', 'sora-content-cid': 'cid2'},
      proof: {manifest: 'man2'},
    },
  ];

  const result = await monitorBinding(
    [
      {label: 'site', url: results[0].url, manifest: 'man1', expectHost: 'docs.sora'},
      {label: 'openapi', url: results[1].url, manifest: 'man2', expectHost: 'docs.sora'},
    ],
    {
      verifyBindingImpl: async ({url}) => results.find((entry) => entry.url === url),
    },
  );

  assert.equal(result.ok, true);
  assert.equal(result.entries.length, 2);
  assert.deepEqual(
    result.entries.map((entry) => entry.label),
    ['site', 'openapi'],
  );
  assert.deepEqual(
    result.entries.map((entry) => entry.manifest),
    ['man1', 'man2'],
  );
});

test('monitorBinding flags host mismatches and missing urls', async () => {
  const result = await monitorBinding(
    [
      {label: 'missing-url'},
      {label: 'wrong-host', url: 'https://docs.dev/.well-known/sorafs/manifest', expectHost: 'docs.sora'},
    ],
    {
      verifyBindingImpl: async () => ({headers: {}, proof: {}}),
    },
  );

  assert.equal(result.ok, false);
  assert.equal(result.entries.length, 2);
  assert.match(result.entries[0].error, /binding url not provided/);
  assert.match(result.entries[1].error, /host mismatch/);
});

test('monitorDnsRecords validates expected answers', async () => {
  const result = await monitorDnsRecords(
    {hostname: 'docs.sora', recordType: 'CNAME', expectedRecords: ['docs.sora.gw.sora.name']},
    {
      resolveRecordImpl: async () => ['docs.sora.gw.sora.name.'],
    },
  );

  assert.equal(result.ok, true);
  assert.equal(result.entries.length, 1);
  assert.deepEqual(result.entries[0].answers, ['docs.sora.gw.sora.name']);
});

test('monitorDnsRecords flags missing hostnames and mismatches', async () => {
  const noHost = await monitorDnsRecords({label: 'nohost'}, {resolveRecordImpl: async () => []});
  assert.equal(noHost.ok, false);
  assert.match(noHost.entries[0].failures[0], /hostname is required/);

  const mismatch = await monitorDnsRecords(
    {hostname: 'docs.sora', recordType: 'TXT', expectedIncludes: ['v=spf1 include:sora']},
    {
      resolveRecordImpl: async () => [['v=spf1 include:example']],
    },
  );
  assert.equal(mismatch.ok, false);
  assert.match(mismatch.entries[0].failures[0], /missing required record/);
});

test('runMonitor aggregates failures across sections', async () => {
  const config = {
    portal: {baseUrl: 'https://docs.sora'},
    tryIt: {proxyUrl: 'https://tryit.sora'},
    binding: {url: 'https://docs.sora/.well-known/sorafs/manifest'},
    dns: {hostname: 'docs.sora'},
  };
  const summary = await runMonitor(config, {
    portal: {
      probePathImpl: async () => ({
        url: 'https://docs.sora/',
        status: 200,
        ok: true,
        release: '',
        durationMs: 4,
        security: {},
      }),
      validateSecurityImpl: () => [],
    },
    tryIt: {
      runProbeImpl: async () => {},
      verifyMetricsImpl: async () => {},
    },
    binding: {
      verifyBindingImpl: async () => {
        throw new Error('missing alias binding');
      },
    },
    dns: {
      resolveRecordImpl: async () => {
        throw new Error('lookup failed');
      },
    },
  });
  assert.equal(summary.ok, false);
  assert.equal(summary.binding.ok, false);
  assert.equal(summary.binding.entries.length, 1);
  assert.match(summary.binding.entries[0].error, /missing alias binding/);
  assert.equal(summary.dns.ok, false);
});

test('writeEvidenceBundle writes section files and checksums', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'monitor-evidence-'));
  const bundleDir = path.join(dir, 'bundle');
  const summary = {
    timestamp: '2026-02-12T00:00:00.000Z',
    ok: false,
    portal: {target: 'portal', ok: true},
    tryIt: {target: 'tryit-proxy', ok: true},
    binding: {target: 'sorafs-binding', ok: false, error: 'missing alias'},
  };
  await writeEvidenceBundle(bundleDir, summary);
  const summaryPath = path.join(bundleDir, 'summary.json');
  const parsed = JSON.parse(await readFile(summaryPath, 'utf8'));
  assert.equal(parsed.timestamp, summary.timestamp);
  const checksumLines = await readFile(path.join(bundleDir, 'checksums.sha256'), 'utf8');
  assert.match(checksumLines, /summary\.json/);
  assert.match(checksumLines, /binding\.json/);
  await rm(dir, {recursive: true, force: true});
});

test('renderPrometheusMetrics emits gauges for sections', () => {
  const summary = {
    portal: {
      target: 'portal',
      ok: true,
      durationMs: 12,
      entries: [{path: '/', ok: true, durationMs: 4}],
    },
    tryIt: {target: 'tryit-proxy', ok: false, durationMs: 7},
    binding: {
      target: 'sorafs-binding',
      ok: true,
      entries: [{label: 'portal', ok: true, expectHost: 'docs.sora'}],
    },
    dns: {
      target: 'soradns',
      ok: true,
      entries: [{hostname: 'docs.sora', recordType: 'CNAME', ok: true}],
    },
  };
  const metrics = renderPrometheusMetrics(summary, {
    labels: {job: 'docs-monitor'},
  });
  assert.match(metrics, /docs_monitor_up\{[^}]*target="portal"/);
  assert.match(metrics, /docs_monitor_probe_duration_seconds\{[^}]*target="tryit-proxy"/);
  assert.match(metrics, /docs_monitor_entry_ok\{[^}]*path="\/"/);
  assert.match(metrics, /docs_monitor_entry_ok\{[^}]*record_type="CNAME"/);
});
