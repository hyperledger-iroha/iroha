// SPDX-License-Identifier: Apache-2.0

import {scanJsonRejectDuplicateKeys} from './openapi-json.mjs';

const SHA256_HEX = /^[0-9a-f]{64}$/;
const GIT_SHA1_HEX = /^[0-9a-f]{40}$/;

/**
 * Parse and validate a release OpenAPI document.
 *
 * Release publication requires actual route and schema inventories. A
 * syntactically valid skeleton with empty `paths` or `components.schemas`
 * must never be signed, versioned, or accepted by verification tooling.
 */
export function validateReleaseOpenApiDocumentBytes(
  bytes,
  {label = 'OpenAPI document'} = {},
) {
  let document;
  try {
    const text =
      typeof bytes === 'string' ? bytes : Buffer.from(bytes).toString('utf8');
    scanJsonRejectDuplicateKeys(text, label);
    document = JSON.parse(text);
  } catch (error) {
    throw new Error(`${label} is not valid JSON: ${error?.message ?? error}`);
  }
  return validateReleaseOpenApiDocument(document, {label});
}


export function validateReleaseOpenApiDocument(
  document,
  {label = 'OpenAPI document'} = {},
) {
  if (!isObject(document)) {
    throw new Error(`${label} must be a JSON object`);
  }
  if (
    typeof document.openapi !== 'string' ||
    !document.openapi.startsWith('3.')
  ) {
    throw new Error(`${label} must declare an OpenAPI 3.x version`);
  }
  if (!isObject(document.info)) {
    throw new Error(`${label} is missing the info object`);
  }
  for (const field of ['title', 'version']) {
    if (
      typeof document.info[field] !== 'string' ||
      document.info[field].trim().length === 0
    ) {
      throw new Error(`${label} info.${field} must be a non-empty string`);
    }
  }
  if (!isObject(document.paths)) {
    throw new Error(`${label} is missing the paths object`);
  }
  if (Object.keys(document.paths).length === 0) {
    throw new Error(
      `${label} must define at least one path; empty/stub specifications are forbidden`,
    );
  }
  if (!isObject(document.components?.schemas)) {
    throw new Error(`${label} is missing components.schemas`);
  }
  if (Object.keys(document.components.schemas).length === 0) {
    throw new Error(
      `${label} must define at least one component schema; empty/stub specifications are forbidden`,
    );
  }
  return document;
}

/**
 * Validate the V2 OpenAPI generator provenance contract.
 *
 * Every manifest binds the canonical generator-input inventory with a
 * lowercase, nonzero SHA-256 digest. Dirty development manifests must also be
 * unsigned and cannot be release-verified.
 */
export function validateOpenApiGeneratorProvenance(
  manifest,
  {label = 'OpenAPI manifest', signed = false, requireClean = false} = {},
) {
  if (manifest?.version !== 2) {
    throw new Error(`${label} provenance requires manifest version exactly 2`);
  }
  const dirtyField = manifest?.generator_dirty;
  if (typeof dirtyField !== 'boolean') {
    throw new Error(`${label} generator_dirty must be boolean and is required`);
  }
  const dirty = dirtyField;
  const commit = manifest?.generator_commit;
  const sourceDigest = manifest?.generator_source_sha256_hex;
  if (
    typeof sourceDigest !== 'string' ||
    !SHA256_HEX.test(sourceDigest) ||
    /^0{64}$/.test(sourceDigest)
  ) {
    throw new Error(
      `${label} provenance requires generator_source_sha256_hex as 64 lowercase hexadecimal characters and it must be nonzero`,
    );
  }

  if (dirty) {
    if (commit !== null) {
      throw new Error(`${label} dirty provenance must set generator_commit to null`);
    }
    if (signed) {
      throw new Error(`${label} dirty provenance must not be signed`);
    }
    if (requireClean) {
      throw new Error(`${label} dirty provenance cannot be release-verified`);
    }
    return {dirty: true, commit: null, sourceSha256Hex: sourceDigest};
  }

  if (typeof commit !== 'string' || !GIT_SHA1_HEX.test(commit)) {
    throw new Error(
      `${label} clean provenance requires generator_commit as exactly 40 lowercase hexadecimal characters`,
    );
  }
  if (/^0{40}$/.test(commit)) {
    throw new Error(`${label} generator_commit must identify a nonzero Git commit`);
  }
  return {dirty: false, commit, sourceSha256Hex: sourceDigest};
}

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
