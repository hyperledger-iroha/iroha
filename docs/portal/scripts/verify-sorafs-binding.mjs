#!/usr/bin/env node

import assert from 'node:assert/strict';
import {Buffer} from 'node:buffer';
import {writeFile} from 'node:fs/promises';
import {parseArgs} from 'node:util';
import {fileURLToPath} from 'node:url';

const BASE64_PATTERN = /^[A-Za-z0-9+/=_-]+$/;

/**
 * Fetch a gateway endpoint and verify that the stapled Sora-Proof headers match
 * the expected alias + manifest digest.
 *
 * @param {object} options
 * @param {string} options.url - Gateway URL (e.g., https://docs.sora/.well-known/sorafs/manifest)
 * @param {string} [options.expectedAlias] - Alias that must be advertised via Sora-Name
 * @param {string} [options.expectedManifest] - Manifest digest expected inside the proof bundle
 * @param {string} [options.expectedProofStatus] - Required Sora-Proof-Status label (e.g., ok, refresh)
 * @param {string} [options.expectedContentCid] - Optional content CID that must match the Sora-Content-CID header
 * @param {number} [options.expectedHttpStatus=200] - Expected HTTP status code
 * @param {number} [options.timeoutMs=10000] - Request timeout in milliseconds
 * @param {Function} [options.fetchImpl=globalThis.fetch] - Fetch implementation to use (injected for tests)
 * @returns {Promise<object>} Summary payload
 */
export async function verifySorafsBinding({
  url,
  expectedAlias,
  expectedManifest,
  expectedProofStatus,
  expectedContentCid,
  expectedHttpStatus = 200,
  timeoutMs = 10_000,
  fetchImpl = globalThis.fetch,
} = {}) {
  assert(typeof url === 'string' && url.length > 0, 'url is required');
  assert(typeof fetchImpl === 'function', 'fetch implementation must be provided');

  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), timeoutMs);

  const requestHeaders = {
    Accept: 'application/json',
    'Cache-Control': 'no-cache',
    'User-Agent': 'sorafs-binding-verifier/1.0',
  };

  let response;
  try {
    response = await fetchImpl(url, {
      method: 'GET',
      headers: requestHeaders,
      redirect: 'manual',
      signal: controller.signal,
    });
  } catch (error) {
    clearTimeout(timeout);
    if (error?.name === 'AbortError') {
      throw new Error(`request timed out after ${timeoutMs} ms`);
    }
    throw new Error(`failed to fetch ${url}: ${error?.message ?? String(error)}`);
  }
  clearTimeout(timeout);

  if (response.status !== expectedHttpStatus) {
    throw new Error(
      `expected HTTP ${expectedHttpStatus} from ${url}, received ${response.status}`,
    );
  }

  const header = (name) => response.headers.get(name) ?? '';
  const soraName = header('sora-name');
  const proofStatus = header('sora-proof-status');
  const soraProofRaw = header('sora-proof');
  const soraCid = validateContentCid(header('sora-content-cid'));

  if (!soraName) {
    throw new Error('missing Sora-Name header');
  }
  if (expectedAlias && soraName !== expectedAlias) {
    throw new Error(
      `Sora-Name mismatch: expected "${expectedAlias}", received "${soraName}"`,
    );
  }
  if (expectedProofStatus && proofStatus !== expectedProofStatus) {
    throw new Error(
      `Sora-Proof-Status mismatch: expected "${expectedProofStatus}", received "${proofStatus || 'unknown'}"`,
    );
  }
  if (expectedContentCid && soraCid !== expectedContentCid) {
    throw new Error(
      `Sora-Content-CID mismatch: expected "${expectedContentCid}", received "${soraCid}"`,
    );
  }

  const proofPayload = decodeProofBundle(soraProofRaw);
  if (expectedAlias && proofPayload.alias !== expectedAlias) {
    throw new Error(
      `alias in proof bundle (${proofPayload.alias}) does not match expected alias ${expectedAlias}`,
    );
  }
  if (expectedManifest && proofPayload.manifest !== expectedManifest) {
    throw new Error(
      `manifest digest mismatch: expected ${expectedManifest}, received ${proofPayload.manifest}`,
    );
  }

  return {
    url,
    statusCode: response.status,
    headers: {
      'sora-name': soraName,
      'sora-proof-status': proofStatus || null,
      'sora-content-cid': soraCid,
    },
    proof: proofPayload,
  };
}

/**
 * Decode the base64-encoded Sora-Proof header.
 *
 * @param {string} value
 * @returns {{alias?: string, manifest?: string}}
 */
export function decodeProofBundle(value) {
  if (!value) {
    throw new Error('missing Sora-Proof header');
  }
  let decoded;
  try {
    const normalized = value.trim();
    if (!BASE64_PATTERN.test(normalized)) {
      throw new Error('Sora-Proof header is not valid base64: contains invalid characters');
    }
    decoded = Buffer.from(normalized, 'base64').toString('utf8');
  } catch (error) {
    throw new Error(`Sora-Proof header is not valid base64: ${error.message}`);
  }
  try {
    const parsed = JSON.parse(decoded);
    if (typeof parsed !== 'object' || parsed === null) {
      throw new Error('decoded payload is not an object');
    }
    return parsed;
  } catch (error) {
    throw new Error(`Sora-Proof header is not valid JSON: ${error.message}`);
  }
}

function validateContentCid(value) {
  const normalized = value?.trim() ?? '';
  if (normalized === '') {
    throw new Error('missing Sora-Content-CID header');
  }
  if (!/^[a-z0-9]+$/i.test(normalized)) {
    throw new Error('invalid Sora-Content-CID header: expected alphanumeric CID string');
  }
  return normalized;
}

async function runCli() {
  const {values} = parseArgs({
    options: {
      url: {type: 'string', short: 'u', default: 'https://docs.sora/.well-known/sorafs/manifest'},
      alias: {type: 'string', short: 'a'},
      manifest: {type: 'string', short: 'm'},
      status: {type: 'string', short: 's'},
      'content-cid': {type: 'string'},
      'expected-http-status': {type: 'string'},
      timeout: {type: 'string', short: 't'},
      'json-out': {type: 'string'},
    },
  });

  const summary = await verifySorafsBinding({
    url: values.url,
    expectedAlias: values.alias,
    expectedManifest: values.manifest,
    expectedProofStatus: values.status,
    expectedContentCid: values['content-cid'],
    expectedHttpStatus:
      values['expected-http-status'] != null
        ? Number.parseInt(values['expected-http-status'], 10)
        : 200,
    timeoutMs: values.timeout != null ? Number.parseInt(values.timeout, 10) : 10_000,
  });

  console.log(
    `[verify-sorafs-binding] ${summary.headers['sora-name']} → ${summary.proof.manifest}`,
  );
  if (values['json-out']) {
    await writeFile(
      values['json-out'],
      JSON.stringify(
        {
          url: summary.url,
          statusCode: summary.statusCode,
          headers: summary.headers,
          proof: summary.proof,
        },
        null,
        2,
      ) + '\n',
      {encoding: 'utf8'},
    );
    console.log(`[verify-sorafs-binding] wrote summary to ${values['json-out']}`);
  }
}

if (fileURLToPath(import.meta.url) === process.argv[1]) {
  runCli().catch((error) => {
    console.error(`[verify-sorafs-binding] ${error.message}`);
    process.exitCode = 1;
  });
}
