// SPDX-License-Identifier: Apache-2.0

import {
  scanJsonRejectDuplicateKeys,
} from './openapi-manifest-v2.mjs';
import {readOpenApiStableFile} from './openapi-safe-file.mjs';
import {
  validateOpenApiEd25519PublicKeyHex,
} from './openapi-signature.mjs';

const ALLOWED_SIGNERS_MAX_BYTES = 64 * 1024;
const ALLOWED_SIGNERS_MAX_ENTRIES = 256;
const ROOT_FIELDS = Object.freeze(['version', 'allow']);
const ENTRY_FIELDS = Object.freeze(['algorithm', 'public_key_hex']);
const LOWER_ED25519_PUBLIC_KEY_HEX = /^[0-9a-f]{64}$/;

function formatSignerKey(algorithm, publicKey) {
  return `${algorithm}:${publicKey}`;
}

/**
 * Load and validate the operator-maintained OpenAPI signer allowlist.
 *
 * The V1 trust root is a strict, duplicate-free schema. Algorithm labels and
 * public keys are canonical bytes, not case-folded aliases.
 *
 * @param {string} allowedSignersFile
 * @returns {Promise<Set<string>>}
 */
export async function loadAllowedSigners(allowedSignersFile) {
  let raw;
  try {
    raw = await readOpenApiStableFile(allowedSignersFile, {
      label: 'OpenAPI signer allowlist',
      maxBytes: ALLOWED_SIGNERS_MAX_BYTES,
      encoding: 'utf8',
      requireSafePermissions: true,
    });
  } catch (error) {
    throw new Error(
      `failed to read allowed signers file ${allowedSignersFile}: ${error.message ?? error}`,
    );
  }

  scanJsonRejectDuplicateKeys(raw, `allowed signers file ${allowedSignersFile}`);
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch (error) {
    throw new Error(`failed to parse ${allowedSignersFile}: ${error.message ?? error}`);
  }
  assertExactObjectFields(parsed, ROOT_FIELDS, allowedSignersFile);
  if (!Array.isArray(parsed.allow)) {
    throw new Error(`${allowedSignersFile} must contain an allow array`);
  }
  if (parsed.version !== 1) {
    throw new Error(`${allowedSignersFile} unsupported version ${parsed.version ?? '(missing)'}`);
  }
  if (parsed.allow.length > ALLOWED_SIGNERS_MAX_ENTRIES) {
    throw new Error(
      `${allowedSignersFile} contains ${parsed.allow.length} entries; the limit is ${ALLOWED_SIGNERS_MAX_ENTRIES}`,
    );
  }

  const entries = [];
  const issues = [];
  const seenSigners = new Set();
  for (const [index, entry] of parsed.allow.entries()) {
    try {
      assertExactObjectFields(
        entry,
        ENTRY_FIELDS,
        `${allowedSignersFile} entry ${index}`,
      );
    } catch (error) {
      issues.push(error.message ?? String(error));
      continue;
    }
    if (entry.algorithm !== 'ed25519') {
      issues.push(
        `entry ${index} unsupported algorithm; must be exactly ed25519`,
      );
      continue;
    }
    if (
      typeof entry.public_key_hex !== 'string' ||
      !LOWER_ED25519_PUBLIC_KEY_HEX.test(entry.public_key_hex)
    ) {
      issues.push(
        `entry ${index} invalid public_key_hex: must be exactly 64 lowercase hexadecimal characters`,
      );
      continue;
    }
    try {
      validateOpenApiEd25519PublicKeyHex(entry.public_key_hex);
    } catch (error) {
      issues.push(`entry ${index} invalid public_key_hex: ${error.message ?? error}`);
      continue;
    }
    const signerKey = formatSignerKey(entry.algorithm, entry.public_key_hex);
    if (seenSigners.has(signerKey)) {
      issues.push(`entry ${index} duplicates allowed signer`);
    } else {
      seenSigners.add(signerKey);
      entries.push(signerKey);
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
  if (
    signature.algorithm !== 'ed25519' ||
    typeof signature.publicKey !== 'string' ||
    !LOWER_ED25519_PUBLIC_KEY_HEX.test(signature.publicKey)
  ) {
    return false;
  }
  return allowedSigners.has(
    formatSignerKey(signature.algorithm, signature.publicKey),
  );
}

function assertExactObjectFields(value, expected, label) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error(`${label} must be a JSON object`);
  }
  const keys = Object.keys(value);
  const expectedSet = new Set(expected);
  const missing = expected.filter((field) => !Object.hasOwn(value, field));
  if (missing.length > 0) {
    throw new Error(`${label} is missing field(s): ${missing.join(', ')}`);
  }
  const unknown = keys.filter((field) => !expectedSet.has(field));
  if (unknown.length > 0) {
    throw new Error(`${label} contains unknown field(s): ${unknown.join(', ')}`);
  }
}
