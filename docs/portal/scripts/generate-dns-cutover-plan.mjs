#!/usr/bin/env node
/**
 * Generate a DNS cutover descriptor from the SoraFS pin report.
 *
 * Usage:
 *   node scripts/generate-dns-cutover-plan.mjs \
 *     --pin-report artifacts/sorafs/portal.pin.report.json \
 *     --out artifacts/sorafs/portal.dns-cutover.json \
 *     [--change-ticket=OPS-1234] \
 *     [--cutover-window=2026-03-21T15:00Z/2026-03-21T15:30Z] \
 *     [--dns-hostname=docs.sora.link] \
 *     [--dns-zone=sora.link] \
 *     [--ops-contact=oncall@sora.link]
 */

import {readFile, writeFile} from 'node:fs/promises';
import {dirname, join, resolve} from 'node:path';
import {pathToFileURL} from 'node:url';
import process from 'node:process';

export function parseArgs(argv, env = process.env) {
  const options = {
    pinReportPath: env.DNS_PIN_REPORT || '',
    outputPath: env.DNS_CUTOVER_PLAN || '',
    changeTicket: env.DNS_CHANGE_TICKET || '',
    cutoverWindow: env.DNS_CUTOVER_WINDOW || '',
    dnsHostname: env.DNS_HOSTNAME || '',
    dnsZone: env.DNS_ZONE || '',
    opsContact: env.DNS_OPS_CONTACT || '',
    cachePurgeEndpoint: env.DNS_CACHE_PURGE_ENDPOINT || '',
    cachePurgeAuthEnv: env.DNS_CACHE_PURGE_AUTH_ENV || 'CACHE_PURGE_TOKEN',
    previousPlanPath: env.DNS_PREVIOUS_PLAN || '',
  };

  for (const arg of argv) {
    if (arg.startsWith('--pin-report=')) {
      options.pinReportPath = arg.slice('--pin-report='.length);
    } else if (arg.startsWith('--out=')) {
      options.outputPath = arg.slice('--out='.length);
    } else if (arg.startsWith('--change-ticket=')) {
      options.changeTicket = arg.slice('--change-ticket='.length);
    } else if (arg.startsWith('--cutover-window=')) {
      options.cutoverWindow = arg.slice('--cutover-window='.length);
    } else if (arg.startsWith('--dns-hostname=')) {
      options.dnsHostname = arg.slice('--dns-hostname='.length);
    } else if (arg.startsWith('--dns-zone=')) {
      options.dnsZone = arg.slice('--dns-zone='.length);
    } else if (arg.startsWith('--ops-contact=')) {
      options.opsContact = arg.slice('--ops-contact='.length);
    } else if (arg.startsWith('--cache-purge-endpoint=')) {
      options.cachePurgeEndpoint = arg.slice('--cache-purge-endpoint='.length);
    } else if (arg.startsWith('--cache-purge-auth-env=')) {
      options.cachePurgeAuthEnv = arg.slice('--cache-purge-auth-env='.length);
    } else if (arg.startsWith('--previous-plan=')) {
      options.previousPlanPath = arg.slice('--previous-plan='.length);
    } else {
      throw new Error(`unknown argument: ${arg}`);
    }
  }

  const resolvedReport = resolve(
    options.pinReportPath || 'artifacts/sorafs/portal.pin.report.json',
  );
  const resolvedOut = resolve(
    options.outputPath || join(dirname(resolvedReport), 'portal.dns-cutover.json'),
  );
  const resolvedPrevious = options.previousPlanPath
    ? resolve(options.previousPlanPath)
    : null;

  return {
    pinReportPath: resolvedReport,
    outputPath: resolvedOut,
    changeTicket: options.changeTicket || null,
    cutoverWindow: options.cutoverWindow || null,
    dnsHostname: options.dnsHostname || null,
    dnsZone: options.dnsZone || null,
    opsContact: options.opsContact || null,
    cachePurgeEndpoint: options.cachePurgeEndpoint || null,
    cachePurgeAuthEnv: options.cachePurgeAuthEnv || null,
    previousPlanPath: resolvedPrevious,
  };
}

function buildRoutePromotion({
  routePlan,
  gatewayBinding,
  aliasLiteral,
  manifestPath,
  dnsHostname,
}) {
  if (!routePlan || typeof routePlan !== 'object') {
    return null;
  }
  const host =
    typeof routePlan.hostname === 'string' && routePlan.hostname.trim() !== ''
      ? routePlan.hostname.trim()
      : dnsHostname || null;
  const cid =
    typeof routePlan.content_cid === 'string' && routePlan.content_cid.trim() !== ''
      ? routePlan.content_cid.trim()
      : null;
  const generatedAt =
    typeof routePlan.generated_at === 'string' ? routePlan.generated_at : null;

  if (!host && !cid) {
    return null;
  }

  const commands = [];
  const verifyUrl = host ? `https://${host}/.well-known/sorafs/manifest` : null;
  if (gatewayBinding?.json_path) {
    let xtaskCmd = `cargo xtask soradns-verify-binding --binding ${gatewayBinding.json_path}`;
    if (aliasLiteral) xtaskCmd += ` --alias ${aliasLiteral}`;
    if (cid) xtaskCmd += ` --content-cid ${cid}`;
    if (host) xtaskCmd += ` --hostname ${host}`;
    if (gatewayBinding.proof_status) {
      xtaskCmd += ` --proof-status ${gatewayBinding.proof_status}`;
    }
    if (manifestPath) {
      xtaskCmd += ` --manifest-json ${manifestPath}`;
    }
    commands.push(xtaskCmd);
  }

  return {
    host,
    content_cid: cid,
    generated_at: generatedAt,
    headers_path: routePlan.headers_path || null,
    headers_template: routePlan.headers_template || null,
    binding_json_path: gatewayBinding?.json_path || null,
    verify_url: verifyUrl,
    commands,
  };
}

export function createDescriptor(pinReport, meta, previousPlan = null, now = new Date()) {
  if (!pinReport || typeof pinReport !== 'object') {
    throw new Error('pin report payload is missing or malformed');
  }
  const {
    manifest,
    car,
    alias_binding: aliasBinding,
    submission,
    gateway_binding: gatewayBinding,
    route_plan: routePlan,
  } = pinReport;
  if (!manifest || !manifest.blake3_hex) {
    throw new Error('pin report missing manifest metadata');
  }
  if (!car || !car.car_digest_hex) {
    throw new Error('pin report missing CAR metadata');
  }
  if (!submission) {
    throw new Error('pin report lacks submission summary (was --skip-submit used?)');
  }
  if (!aliasBinding || !aliasBinding.namespace || !aliasBinding.name) {
    throw new Error('pin report missing alias binding details');
  }

  const aliasQualified = `${aliasBinding.namespace}:${aliasBinding.name}`;
  const aliasLiteral =
    gatewayBinding?.alias || `${aliasBinding.namespace}.${aliasBinding.name}`;
  const baseUrl = meta.dnsHostname
    ? `https://${meta.dnsHostname}`
    : 'https://<portal-hostname>';
  const commands = [];
  const expected = pinReport.release_tag || '<release-tag>';
  commands.push(
    `npm run probe:portal -- --base-url=${baseUrl} --expect-release=${expected}`,
  );

  let cacheInvalidation = null;
  if (meta.cachePurgeEndpoint) {
    const payload = {
      aliases: [aliasQualified],
      manifest_digest_hex: submission.manifest_digest_hex || manifest.blake3_hex,
      car_digest_hex: car.car_digest_hex,
      release_tag: pinReport.release_tag || null,
    };
    const authEnv = meta.cachePurgeAuthEnv || 'CACHE_PURGE_TOKEN';
    const payloadJson = JSON.stringify(payload);
    const curlLines = [
      `curl -X POST ${meta.cachePurgeEndpoint}`,
      "  -H 'Content-Type: application/json'",
    ];
    if (authEnv) {
      curlLines.push(`  -H "Authorization: Bearer $${authEnv}"`);
    }
    curlLines.push(`  --data '${payloadJson}'`);
    cacheInvalidation = {
      endpoint: meta.cachePurgeEndpoint,
      auth_env: authEnv,
      payload,
      command: curlLines.join(' \\\n'),
    };
  }

  let rollback = null;
  if (previousPlan && meta.previousPlanPath) {
    const rollbackCommands = [];
    rollbackCommands.push(
      `npm run probe:portal -- --base-url=${baseUrl} --expect-release=${
        previousPlan?.release?.tag ?? '<previous-release>'
      }`,
    );
    rollbackCommands.push(
      `jq -r '.alias.manifest_digest_hex // .manifest.blake3_hex' ${meta.previousPlanPath}`,
    );
    rollback = {
      plan_path: meta.previousPlanPath,
      release: {
        tag: previousPlan?.release?.tag ?? null,
        source: previousPlan?.release?.source ?? null,
      },
      manifest_digest_hex:
        previousPlan?.alias?.manifest_digest_hex ??
        previousPlan?.manifest?.blake3_hex ??
        null,
      commands: rollbackCommands,
    };
  }

  const gatewayBindingDescriptor = gatewayBinding
    ? {
        alias: gatewayBinding.alias || null,
        hostname: gatewayBinding.hostname || null,
        content_cid: gatewayBinding.content_cid || null,
        proof_status: gatewayBinding.proof_status || null,
        json_path: gatewayBinding.json_path || null,
        headers_path: gatewayBinding.headers_path || null,
        headers: gatewayBinding.headers || null,
        headers_template: gatewayBinding.headers_text || null,
      }
    : null;
  const routePromotion = buildRoutePromotion({
    routePlan,
    gatewayBinding,
    aliasLiteral,
    manifestPath: manifest.path,
    dnsHostname: meta.dnsHostname,
  });
  const routePlanDescriptor = routePlan
    ? {
        path: routePlan.path || null,
        headers_path: routePlan.headers_path || null,
        rollback_headers_path: routePlan.rollback_headers_path || null,
        alias: routePlan.alias || null,
        hostname: routePlan.hostname || null,
        content_cid: routePlan.content_cid || null,
        route_binding: routePlan.route_binding || null,
        headers_template: routePlan.headers_template || null,
        generated_at: routePlan.generated_at || null,
      }
    : null;

  return {
    version: 1,
    generated_at: now.toISOString(),
    change_ticket: meta.changeTicket,
    cutover_window: meta.cutoverWindow,
    ops_contact: meta.opsContact,
    dns: {
      hostname: meta.dnsHostname,
      zone: meta.dnsZone,
    },
    release: {
      tag: pinReport.release_tag || null,
      source: pinReport.release_source || null,
      alias_label: pinReport.alias_label || null,
    },
    alias: {
      namespace: aliasBinding.namespace,
      name: aliasBinding.name,
      proof_path: aliasBinding.proof_path || null,
      torii_url: submission.torii_url || null,
      submitted_epoch: submission.submitted_epoch || null,
      authority: submission.authority || null,
      manifest_digest_hex: submission.manifest_digest_hex || null,
    },
    manifest: {
      path: manifest.path,
      blake3_hex: manifest.blake3_hex,
      chunk_digest_sha3_hex: manifest.chunk_digest_sha3_hex || null,
    },
    car: {
      path: car.path,
      digest_hex: car.car_digest_hex,
      size_bytes: car.size_bytes || null,
    },
    chunk_plan_path: pinReport.plan_path || null,
    signing_bundle_path: pinReport.signing?.bundle_path || null,
    signature_path: pinReport.signing?.signature_path || null,
    submission_summary_path: submission.summary_path || null,
    report_path: meta.pinReportPath,
    gateway_binding: gatewayBindingDescriptor,
    route_promotion: routePromotion,
    route_plan: routePlanDescriptor,
    cache_invalidation: cacheInvalidation,
    rollback,
    verification: {
      alias: aliasQualified,
      torii_url: submission.torii_url || null,
      commands,
    },
  };
}

export async function generatePlan(options) {
  const data = await readFile(options.pinReportPath, 'utf8');
  const report = JSON.parse(data);
  let previousPlan = null;
  if (options.previousPlanPath) {
    const previousRaw = await readFile(options.previousPlanPath, 'utf8');
    previousPlan = JSON.parse(previousRaw);
  }
  const descriptor = createDescriptor(report, options, previousPlan);
  await writeFile(
    options.outputPath,
    `${JSON.stringify(descriptor, null, 2)}\n`,
    'utf8',
  );
  return descriptor;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  await generatePlan(options);
}

if (import.meta.url === pathToFileURL(process.argv[1] ?? '').href) {
  main().catch((error) => {
    console.error(`[dns-cutover] ${error.message}`);
    process.exitCode = 1;
  });
}
