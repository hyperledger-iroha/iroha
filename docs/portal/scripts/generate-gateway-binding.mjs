#!/usr/bin/env node
/**
 * Emit gateway binding metadata + HTTP header templates for SoraFS-hosted sites.
 *
 * This helper reads a `ManifestV1` JSON dump (produced by
 * `sorafs_cli manifest build --manifest-json-out`) and derives the canonical
 * `Sora-Content-CID` plus a ready-to-use header block containing
 * `Sora-Name`, `Sora-Proof`, CSP/HSTS templates, and the optional
 * `Sora-Route-Binding` descriptor used by DG-3 rollout procedures.
 */
import {mkdir, readFile, writeFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {dirname, resolve} from 'node:path';
import {parseArgs} from 'node:util';

const HEADER_ORDER = [
  'Sora-Name',
  'Sora-Content-CID',
  'Sora-Proof',
  'Sora-Proof-Status',
  'Sora-Route-Binding',
  'Content-Security-Policy',
  'Strict-Transport-Security',
  'Permissions-Policy',
];

const DEFAULT_CSP =
  "default-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; object-src 'none'; frame-ancestors 'none'; base-uri 'self'";
const DEFAULT_PERMISSIONS_POLICY =
  'accelerometer=(), ambient-light-sensor=(), autoplay=(), camera=(), clipboard-read=(self), clipboard-write=(self), encrypted-media=(), fullscreen=(self), geolocation=(), gyroscope=(), hid=(), magnetometer=(), microphone=(), midi=(), payment=(), picture-in-picture=(), speaker-selection=(), usb=(), xr-spatial-tracking=()';
const DEFAULT_HSTS_MAX_AGE = 63_072_000; // 2 years.

const DEFAULT_PROOF_STATUS = 'ok';

export function encodeBase32Lower(bytes) {
  const alphabet = 'abcdefghijklmnopqrstuvwxyz234567';
  if (!bytes || bytes.length === 0) {
    return '';
  }
  let acc = 0;
  let bits = 0;
  let output = '';
  for (const byte of bytes) {
    acc = (acc << 8) | (byte & 0xff);
    bits += 8;
    while (bits >= 5) {
      const index = (acc >> (bits - 5)) & 0x1f;
      output += alphabet[index];
      bits -= 5;
    }
  }
  if (bits > 0) {
    const index = (acc << (5 - bits)) & 0x1f;
    output += alphabet[index];
  }
  return output;
}

function hexToBytes(hex) {
  if (typeof hex !== 'string' || hex.length === 0 || hex.length % 2 !== 0) {
    throw new Error('hex value must contain an even number of characters');
  }
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < hex.length; i += 2) {
    const value = Number.parseInt(hex.slice(i, i + 2), 16);
    if (Number.isNaN(value)) {
      throw new Error(`invalid hex byte at offset ${i}`);
    }
    bytes[i / 2] = value;
  }
  return bytes;
}

function extractRootCidBytes(manifest) {
  if (manifest && Array.isArray(manifest.root_cid)) {
    return Uint8Array.from(manifest.root_cid.map((value) => value & 0xff));
  }
  if (
    manifest &&
    Array.isArray(manifest.root_cids_hex) &&
    typeof manifest.root_cids_hex[0] === 'string'
  ) {
    return hexToBytes(manifest.root_cids_hex[0]);
  }
  throw new Error('manifest is missing `root_cid` payload');
}

export function formatHeaders(headers) {
  const lines = [];
  for (const key of HEADER_ORDER) {
    if (headers[key]) {
      lines.push(`${key}: ${headers[key]}`);
    }
  }
  for (const [key, value] of Object.entries(headers)) {
    if (HEADER_ORDER.includes(key)) {
      continue;
    }
    lines.push(`${key}: ${value}`);
  }
  return `${lines.join('\n')}\n`;
}

export async function generateGatewayBinding({
  manifestJsonPath,
  alias,
  hostname,
  proofStatus = DEFAULT_PROOF_STATUS,
  routeLabel,
  includeCsp = true,
  includePermissions = true,
  includeHsts = true,
  hstsMaxAge = DEFAULT_HSTS_MAX_AGE,
  now = new Date(),
}) {
  if (!manifestJsonPath) {
    throw new Error('manifestJsonPath is required');
  }
  const manifestRaw = await readFile(manifestJsonPath, 'utf8');
  const manifest = JSON.parse(manifestRaw);
  const rootCidBytes = extractRootCidBytes(manifest);
  const contentCid = `b${encodeBase32Lower(rootCidBytes)}`;
  const generatedAt = now.toISOString();

  /** @type {Record<string, string>} */
  const headers = {
    'Sora-Content-CID': contentCid,
  };

  if (alias) {
    headers['Sora-Name'] = alias;
    const proofPayload = {
      alias,
      manifest: contentCid,
    };
    headers['Sora-Proof'] = Buffer.from(JSON.stringify(proofPayload), 'utf8').toString('base64');
    headers['Sora-Proof-Status'] = proofStatus;
  }

  if (hostname) {
    const routeParts = [`host=${hostname}`, `cid=${contentCid}`, `generated_at=${generatedAt}`];
    if (routeLabel) {
      routeParts.push(`label=${routeLabel}`);
    }
    headers['Sora-Route-Binding'] = routeParts.join(';');
  }

  if (includeCsp) {
    headers['Content-Security-Policy'] = DEFAULT_CSP;
  }
  if (includeHsts) {
    headers['Strict-Transport-Security'] = `max-age=${hstsMaxAge}; includeSubDomains; preload`;
  }
  if (includePermissions) {
    headers['Permissions-Policy'] = DEFAULT_PERMISSIONS_POLICY;
  }

  return {
    alias: alias ?? null,
    hostname: hostname ?? null,
    contentCid,
    proofStatus: alias ? proofStatus : null,
    generatedAt,
    headers,
  };
}

async function writeFileIfNeeded(path, contents) {
  if (!path) {
    return;
  }
  await mkdir(dirname(path), {recursive: true});
  await writeFile(path, contents, 'utf8');
}

async function runCli() {
  const {values} = parseArgs({
    options: {
      'manifest-json': {type: 'string'},
      alias: {type: 'string'},
      hostname: {type: 'string'},
      'proof-status': {type: 'string', default: DEFAULT_PROOF_STATUS},
      'route-label': {type: 'string'},
      'headers-out': {type: 'string'},
      'json-out': {type: 'string'},
    },
    allowPositionals: false,
  });

  if (!values['manifest-json']) {
    throw new Error('--manifest-json is required');
  }

  const binding = await generateGatewayBinding({
    manifestJsonPath: resolve(values['manifest-json']),
    alias: values.alias,
    hostname: values.hostname,
    proofStatus: values['proof-status'],
    routeLabel: values['route-label'],
  });

  if (values['json-out']) {
    await writeFileIfNeeded(
      resolve(values['json-out']),
      `${JSON.stringify(binding, null, 2)}\n`,
    );
  }
  if (values['headers-out']) {
    await writeFileIfNeeded(resolve(values['headers-out']), formatHeaders(binding.headers));
  }

  const summaryAlias = binding.alias ?? 'no-alias';
  // eslint-disable-next-line no-console
  console.log(
    `[gateway-binding] ${summaryAlias} -> ${binding.contentCid} (${binding.generatedAt})`,
  );
}

if (fileURLToPath(import.meta.url) === process.argv[1]) {
  runCli().catch((error) => {
    // eslint-disable-next-line no-console
    console.error(`[gateway-binding] ${error.message}`);
    process.exitCode = 1;
  });
}
