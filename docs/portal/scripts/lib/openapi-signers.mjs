// SPDX-License-Identifier: Apache-2.0

import {readFile} from 'node:fs/promises';

function normalizeAlgorithm(value) {
  return typeof value === 'string' && value.trim() !== ''
    ? value.trim().toLowerCase()
    : null;
}

function normalizeHex(value) {
  return typeof value === 'string' ? value.toLowerCase() : null;
}

function formatSignerKey(algorithm, publicKey) {
  return `${algorithm}:${publicKey}`;
}

function isEd25519PublicKeyHex(value) {
  return /^[0-9a-f]{64}$/.test(value ?? '');
}

/**
 * Load and validate the operator-maintained OpenAPI signer allowlist.
 *
 * @param {string} allowedSignersFile
 * @returns {Promise<Set<string>>}
 */
export async function loadAllowedSigners(allowedSignersFile) {
  let raw;
  try {
    raw = await readFile(allowedSignersFile, 'utf8');
  } catch (error) {
    throw new Error(
      `failed to read allowed signers file ${allowedSignersFile}: ${error.message ?? error}`,
    );
  }

  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    throw new Error(`failed to parse ${allowedSignersFile}: ${error.message ?? error}`);
  }
  if (!parsed || !Array.isArray(parsed.allow)) {
    throw new Error(`${allowedSignersFile} must contain an allow array`);
  }
  if (parsed.version !== 1) {
    throw new Error(`${allowedSignersFile} unsupported version ${parsed.version ?? '(missing)'}`);
  }

  const entries = [];
  const issues = [];
  const seenSigners = new Set();
  for (const [index, entry] of parsed.allow.entries()) {
    const algorithm = normalizeAlgorithm(entry?.algorithm);
    const publicKey = normalizeHex(entry?.public_key_hex ?? entry?.publicKeyHex);
    if (!algorithm) {
      issues.push(`entry ${index} missing algorithm`);
    } else if (algorithm !== 'ed25519') {
      issues.push(`entry ${index} unsupported algorithm ${algorithm}`);
    }
    if (!publicKey) {
      issues.push(`entry ${index} missing public_key_hex`);
    } else if (!isEd25519PublicKeyHex(publicKey)) {
      issues.push(`entry ${index} invalid public_key_hex`);
    }
    if (algorithm === 'ed25519' && isEd25519PublicKeyHex(publicKey)) {
      const signerKey = formatSignerKey(algorithm, publicKey);
      if (seenSigners.has(signerKey)) {
        issues.push(`entry ${index} duplicates allowed signer`);
      } else {
        seenSigners.add(signerKey);
        entries.push(signerKey);
      }
    }
  }
  if (issues.length > 0) {
    throw new Error(
      `invalid ${allowedSignersFile}:\n${issues.map((issue) => `- ${issue}`).join('\n')}`,
    );
  }
  return new Set(entries);
}

/**
 * Return whether a manifest signature is issued by an approved key.
 *
 * @param {Set<string>} allowedSigners
 * @param {{algorithm?: unknown, publicKey?: unknown}} signature
 */
export function isAllowedSigner(allowedSigners, signature) {
  const algorithm = normalizeAlgorithm(signature.algorithm);
  const publicKey = normalizeHex(signature.publicKey);
  if (!algorithm || !publicKey) {
    return false;
  }
  return allowedSigners.has(formatSignerKey(algorithm, publicKey));
}
