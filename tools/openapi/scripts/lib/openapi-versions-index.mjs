// SPDX-License-Identifier: Apache-2.0

const ROOT_FIELDS = Object.freeze([
  'versions',
  'generatedAt',
  'entries',
]);
const ENTRY_FIELDS = Object.freeze([
  'label',
  'path',
  'bytes',
  'sha256',
  'blake3',
  'updatedAt',
  'signed',
  'manifestPath',
  'signatureAlgorithm',
  'signaturePublicKeyHex',
  'signatureHex',
]);

/**
 * Reject fields outside the canonical V1 versions-index schema.
 *
 * The individual consumers retain their context-specific validation and error
 * reporting for required fields and values. This common guard prevents a
 * misspelled field or pre-release alias from being silently ignored by one
 * consumer while another consumer interprets it.
 */
export function rejectUnknownOpenApiVersionsIndexFields(
  index,
  {label = 'OpenAPI versions index'} = {},
) {
  if (!isObject(index)) {
    return;
  }
  rejectUnknownFields(index, ROOT_FIELDS, label);
  if (!Array.isArray(index.entries)) {
    return;
  }
  for (const [position, entry] of index.entries.entries()) {
    if (!isObject(entry)) {
      continue;
    }
    rejectUnknownFields(
      entry,
      ENTRY_FIELDS,
      `${label} entry ${position}`,
    );
  }
}

/**
 * Require every canonical root and entry field, including explicit nulls.
 *
 * Keeping nullable metadata present makes the serialized index unambiguous
 * across Rust, JavaScript, and generated-client consumers.
 */
export function requireOpenApiVersionsIndexFields(
  index,
  {label = 'OpenAPI versions index'} = {},
) {
  if (!isObject(index)) {
    return;
  }
  requireFields(index, ROOT_FIELDS, label);
  if (!Array.isArray(index.entries)) {
    return;
  }
  for (const [position, entry] of index.entries.entries()) {
    if (!isObject(entry)) {
      continue;
    }
    requireFields(entry, ENTRY_FIELDS, `${label} entry ${position}`);
  }
}

function rejectUnknownFields(value, expectedFields, label) {
  const expected = new Set(expectedFields);
  const unknown = Object.keys(value).filter((field) => !expected.has(field));
  if (unknown.length > 0) {
    throw new Error(`${label} contains unknown field(s): ${unknown.join(', ')}`);
  }
}

function requireFields(value, expectedFields, label) {
  const missing = expectedFields.filter((field) => !Object.hasOwn(value, field));
  if (missing.length > 0) {
    throw new Error(`${label} is missing field(s): ${missing.join(', ')}`);
  }
}

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}
