import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, writeFile, readFile, rm} from 'node:fs/promises';
import path from 'node:path';
import {tmpdir} from 'node:os';

import {
  buildBindingCommand,
  loadMonitorConfig,
  monitorPortal,
  monitorTryIt,
  monitorBinding,
  monitorDnsRecords,
  runMonitor,
  writeEvidenceBundle,
  renderPrometheusMetrics,
} from '../monitor-publishing.mjs';

test('buildBindingCommand assembles xtask invocation', () => {
  const manifestJson = 'artifacts/sorafs/portal.manifest.json';
  const {args, command, bindingPath} = buildBindingCommand({
    bindingPath: 'artifacts/sorafs/portal.gateway.binding.json',
    alias: 'docs.sora.link',
    contentCid: 'bafytestcid',
    hostname: 'docs.sora.link',
    proofStatus: 'ok',
    manifestJson,
  });
  assert.equal(args[0], 'xtask');
  assert.equal(args[1], 'soradns-verify-binding');
  assert.equal(args[2], '--binding');
  assert.equal(bindingPath, path.resolve(process.cwd(), 'artifacts/sorafs/portal.gateway.binding.json'));
  assert.ok(args.includes('--alias'));
  assert.ok(args.includes('--content-cid'));
  assert.ok(args.includes('--hostname'));
  assert.ok(args.includes('--proof-status'));
  assert.ok(args.includes('--manifest-json'));
  assert.match(command, /cargo xtask soradns-verify-binding/);
  assert.equal(
    args[args.indexOf('--manifest-json') + 1],
    path.resolve(process.cwd(), manifestJson),
  );
});

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
    command: 'cargo xtask soradns-verify-binding --binding /tmp/portal.gateway.binding.json',
    stdout: '[soradns] verified gateway binding docs.sora.link -> bafytestcid',
    stderr: '',
  };
  let captured;
  const result = await monitorBinding(
    {bindingPath: '/tmp/portal.gateway.binding.json', contentCid: 'bafytestcid'},
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
  assert.equal(captured.contentCid, 'bafytestcid');
});

test('monitorBinding supports multiple bindings', async () => {
  const results = [
    {
      bindingPath: '/tmp/portal.gateway.binding.json',
      command: 'cargo xtask soradns-verify-binding --binding /tmp/portal.gateway.binding.json',
    },
    {
      bindingPath: '/tmp/openapi.gateway.binding.json',
      command: 'cargo xtask soradns-verify-binding --binding /tmp/openapi.gateway.binding.json',
    },
  ];

  const result = await monitorBinding(
    [
      {label: 'site', bindingPath: results[0].bindingPath, alias: 'docs.sora'},
      {label: 'openapi', bindingPath: results[1].bindingPath, alias: 'docs.sora'},
    ],
    {
      verifyBindingImpl: async ({bindingPath}) =>
        results.find((entry) => entry.bindingPath === bindingPath),
    },
  );

  assert.equal(result.ok, true);
  assert.equal(result.entries.length, 2);
  assert.deepEqual(
    result.entries.map((entry) => entry.label),
    ['site', 'openapi'],
  );
  assert.deepEqual(
    result.entries.map((entry) => entry.bindingPath),
    ['/tmp/portal.gateway.binding.json', '/tmp/openapi.gateway.binding.json'],
  );
});

test('monitorBinding flags missing bindings and verifier failures', async () => {
  const result = await monitorBinding(
    [
      {label: 'missing-path'},
      {label: 'invalid-binding', bindingPath: '/tmp/missing.gateway.binding.json'},
    ],
    {
      verifyBindingImpl: async ({bindingPath}) => {
        if (bindingPath.includes('missing.gateway.binding.json')) {
          throw new Error('binding not found');
        }
        return {command: 'cargo xtask soradns-verify-binding --binding ok'};
      },
    },
  );

  assert.equal(result.ok, false);
  assert.equal(result.entries.length, 2);
  assert.match(result.entries[0].error, /binding path not provided/);
  assert.match(result.entries[1].error, /binding not found/);
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
    binding: {bindingPath: '/tmp/portal.gateway.binding.json'},
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
        throw new Error('missing gateway binding');
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
  assert.match(summary.binding.entries[0].error, /missing gateway binding/);
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
      entries: [{label: 'portal', ok: true, hostname: 'docs.sora'}],
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
