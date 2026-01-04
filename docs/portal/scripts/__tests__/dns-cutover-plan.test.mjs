import assert from 'node:assert/strict';
import {mkdtemp, readFile, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';
import test from 'node:test';

import {
  createDescriptor,
  generatePlan,
  parseArgs,
} from '../generate-dns-cutover-plan.mjs';

const sampleReport = {
  release_tag: 'sorafs-42',
  release_source: 'docs-portal',
  alias_label: 'docs.sora.link',
  plan_path: '/tmp/plan.json',
  route_plan: {
    path: '/tmp/gateway.route_plan.json',
    headers_path: '/tmp/gateway.route.headers.txt',
    rollback_headers_path: '/tmp/gateway.route.rollback.headers.txt',
    alias: 'docs.sora',
    hostname: 'docs.sora.link',
    content_cid: 'bafgatewaycid',
    route_binding:
      'host=docs.sora.link;cid=bafgatewaycid;generated_at=2026-03-01T00:00:00Z',
    headers_template:
      'Sora-Route-Binding: host=docs.sora.link;cid=bafgatewaycid;generated_at=2026-03-01T00:00:00Z',
    generated_at: '2026-03-01T00:00:00Z',
  },
  manifest: {
    path: '/tmp/portal.manifest.to',
    blake3_hex: 'abcd1234',
    chunk_digest_sha3_hex: 'ff00ee',
  },
  car: {
    path: '/tmp/portal.car',
    car_digest_hex: 'beefcafe',
    size_bytes: 123,
  },
  signing: {
    bundle_path: '/tmp/portal.bundle.json',
    signature_path: '/tmp/portal.sig',
  },
  submission: {
    torii_url: 'https://torii.example',
    submitted_epoch: '20260101',
    authority: 'docs@publish',
    summary_path: '/tmp/submit.json',
    manifest_digest_hex: '0011aa',
  },
  alias_binding: {
    namespace: 'docs',
    name: 'sora',
    proof_path: '/tmp/proof.bin',
  },
  gateway_binding: {
    alias: 'docs.sora',
    hostname: 'docs.sora.link',
    content_cid: 'bafgatewaycid',
    proof_status: 'ok',
    json_path: '/tmp/portal.gateway.binding.json',
    headers_path: '/tmp/portal.gateway.headers.txt',
    headers: {
      'Sora-Name': 'docs.sora',
      'Sora-Content-CID': 'bafgatewaycid',
      'Sora-Proof-Status': 'ok',
      'Sora-Route-Binding':
        'host=docs.sora.link;cid=bafgatewaycid;generated_at=2026-03-01T00:00:00Z',
      'Content-Security-Policy': "default-src 'self'",
    },
    headers_text:
      "Sora-Name: docs.sora\nSora-Content-CID: bafgatewaycid\nSora-Route-Binding: host=docs.sora.link;cid=bafgatewaycid\n",
  },
};

test('parseArgs resolves defaults and env overrides', () => {
  const options = parseArgs([], {
    DNS_PIN_REPORT: './artifacts/custom/report.json',
    DNS_CUTOVER_PLAN: './artifacts/custom/plan.json',
    DNS_CHANGE_TICKET: 'OPS-1',
    DNS_HOSTNAME: 'docs.sora.link',
    DNS_CACHE_PURGE_ENDPOINT: 'https://cache/purge',
    DNS_PREVIOUS_PLAN: './artifacts/prev.json',
  });
  assert.equal(
    options.pinReportPath,
    resolve('artifacts/custom/report.json'),
  );
  assert.equal(options.outputPath, resolve('artifacts/custom/plan.json'));
  assert.equal(options.changeTicket, 'OPS-1');
  assert.equal(options.dnsHostname, 'docs.sora.link');
  assert.equal(options.cachePurgeEndpoint, 'https://cache/purge');
  assert.equal(options.previousPlanPath, resolve('artifacts/prev.json'));
});

test('createDescriptor captures alias and release metadata', () => {
  const descriptor = createDescriptor(sampleReport, {
    pinReportPath: '/tmp/report.json',
    changeTicket: 'OPS-2',
    cutoverWindow: '2026-03-21T15:00Z/2026-03-21T15:30Z',
    dnsHostname: 'docs.sora.link',
    dnsZone: 'sora.link',
    opsContact: 'ops@sora.link',
  });
  assert.equal(descriptor.alias.namespace, 'docs');
  assert.equal(descriptor.alias.name, 'sora');
  assert.equal(descriptor.alias.manifest_digest_hex, '0011aa');
  assert.equal(descriptor.release.tag, 'sorafs-42');
  assert.equal(descriptor.dns.hostname, 'docs.sora.link');
  assert.equal(descriptor.report_path, '/tmp/report.json');
  assert.ok(descriptor.gateway_binding);
  assert.equal(descriptor.gateway_binding.content_cid, 'bafgatewaycid');
  assert.equal(descriptor.gateway_binding.headers_path, '/tmp/portal.gateway.headers.txt');
  assert.ok(descriptor.route_promotion);
  assert.equal(descriptor.route_promotion.host, 'docs.sora.link');
  assert.equal(descriptor.route_promotion.content_cid, 'bafgatewaycid');
  assert.ok(descriptor.route_promotion.commands[0].includes('soradns-verify-binding'));
  assert.ok(descriptor.route_plan);
  assert.equal(descriptor.route_plan.path, '/tmp/gateway.route_plan.json');
  assert.equal(
    descriptor.route_plan.headers_path,
    '/tmp/gateway.route.headers.txt',
  );
  assert.ok(Array.isArray(descriptor.verification.commands));
  assert.ok(descriptor.verification.commands[0].includes('npm run probe:portal'));
});

test('createDescriptor adds cache invalidation and rollback metadata when provided', () => {
  const previousPlan = {
    alias: {
      namespace: 'docs',
      name: 'sora',
      manifest_digest_hex: 'deadbeef',
    },
    release: {tag: 'sorafs-41', source: 'docs-portal'},
    manifest: {blake3_hex: '0000ffff'},
  };
  const descriptor = createDescriptor(
    sampleReport,
    {
      pinReportPath: '/tmp/report.json',
      cachePurgeEndpoint: 'https://cache/purge',
      cachePurgeAuthEnv: 'CACHE_TOKEN',
      previousPlanPath: '/tmp/prev.json',
      dnsHostname: 'docs.sora.link',
    },
    previousPlan,
  );
  assert.ok(descriptor.cache_invalidation);
  assert.equal(
    descriptor.cache_invalidation.payload.manifest_digest_hex,
    '0011aa',
  );
  assert.ok(
    descriptor.cache_invalidation.command.includes('CACHE_TOKEN'),
  );
  assert.ok(descriptor.rollback);
  assert.equal(descriptor.rollback.manifest_digest_hex, 'deadbeef');
  assert.ok(descriptor.rollback.commands[0].includes('probe:portal'));
});

test('generatePlan writes descriptor to disk', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'dns-cutover-'));
  const reportPath = join(dir, 'portal.pin.report.json');
  await writeFile(reportPath, JSON.stringify(sampleReport), 'utf8');
  const outputPath = join(dir, 'plan.json');
  const descriptor = await generatePlan({
    pinReportPath: reportPath,
    outputPath,
    changeTicket: 'OPS-99',
    cutoverWindow: null,
    dnsHostname: null,
    dnsZone: null,
    opsContact: null,
  });
  assert.equal(descriptor.change_ticket, 'OPS-99');
  const written = JSON.parse(await readFile(outputPath, 'utf8'));
  assert.equal(written.alias.name, 'sora');
  assert.equal(written.alias.namespace, 'docs');
  assert.ok(written.gateway_binding);
  assert.equal(written.gateway_binding.alias, 'docs.sora');
});

test('generatePlan loads previous plan metadata when path provided', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'dns-cutover-prev-'));
  const reportPath = join(dir, 'portal.pin.report.json');
  await writeFile(reportPath, JSON.stringify(sampleReport), 'utf8');
  const previousPath = join(dir, 'prev.plan.json');
  await writeFile(
    previousPath,
    JSON.stringify({
      alias: {namespace: 'docs', name: 'sora', manifest_digest_hex: 'beaded'},
      release: {tag: 'sorafs-41'},
      manifest: {blake3_hex: 'aaaa'},
    }),
    'utf8',
  );
  const outputPath = join(dir, 'plan.json');
  const descriptor = await generatePlan({
    pinReportPath: reportPath,
    outputPath,
    changeTicket: null,
    cutoverWindow: null,
    dnsHostname: null,
    dnsZone: null,
    opsContact: null,
    cachePurgeEndpoint: null,
    cachePurgeAuthEnv: null,
    previousPlanPath: previousPath,
  });
  assert.ok(descriptor.rollback);
  assert.equal(descriptor.rollback.manifest_digest_hex, 'beaded');
});
