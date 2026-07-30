// SPDX-License-Identifier: Apache-2.0

import {createHash} from 'node:crypto';

import {blake3} from '@noble/hashes/blake3';

import {
  validateOpenApiEd25519PublicKeyHex,
  verifyOpenApiSignature,
} from './openapi-signature.mjs';
import {validateOpenApiGeneratorProvenance} from './openapi-provenance.mjs';

export const OPENAPI_MANIFEST_VERSION = 2;
export const OPENAPI_MANIFEST_SIGNATURE_DOMAIN_V2 =
  'iroha.openapi.manifest.signature.v2';

const LOWER_SHA256_HEX = /^[0-9a-f]{64}$/;
const LOWER_BLAKE3_HEX = /^[0-9a-f]{64}$/;
const LOWER_ED25519_PUBLIC_KEY_HEX = /^[0-9a-f]{64}$/;
const LOWER_ED25519_SIGNATURE_HEX = /^[0-9a-f]{128}$/;
const COMMON_MANIFEST_FIELDS = Object.freeze([
  'version',
  'generated_unix_ms',
  'generator_commit',
  'generator_dirty',
  'generator_source_sha256_hex',
  'artifact',
]);
const ARTIFACT_FIELDS = Object.freeze([
  'path',
  'bytes',
  'sha256_hex',
  'blake3_hex',
  'signature',
]);
const SIGNATURE_FIELDS = Object.freeze([
  'algorithm',
  'public_key_hex',
  'signature_hex',
]);

/**
 * Parse a V2 manifest while rejecting duplicate JSON member names before
 * `JSON.parse` can collapse them.
 */
export function parseOpenApiManifestV2Json(
  source,
  {label = 'OpenAPI manifest'} = {},
) {
  const text = Buffer.isBuffer(source)
    ? source.toString('utf8')
    : source;
  if (typeof text !== 'string') {
    throw new TypeError(`${label} JSON must be a string or Buffer`);
  }
  scanJsonRejectDuplicateKeys(text, label);
  let manifest;
  try {
    manifest = JSON.parse(text);
  } catch (error) {
    throw new Error(`failed to parse ${label}: ${error.message ?? error}`);
  }
  if (!isObject(manifest)) {
    throw new Error(`${label} must be a JSON object`);
  }
  if (manifest.version !== OPENAPI_MANIFEST_VERSION) {
    throw new Error(
      `${label} has unsupported version ${String(manifest.version)}; expected exactly ${OPENAPI_MANIFEST_VERSION}`,
    );
  }
  return manifest;
}

export function computeOpenApiBlake3Hex(artifactBytes) {
  return Buffer.from(blake3(toBuffer(artifactBytes, 'artifactBytes'))).toString(
    'hex',
  );
}

/**
 * Validate the hard-cut OpenAPI manifest V2 contract.
 *
 * The artifact path is the immutable V1 name `torii.json`. Callers may supply
 * `expectedArtifactPath` as an additional binding to the resolved file.
 */
export function validateOpenApiManifestV2({
  manifest,
  artifactBytes,
  label = 'OpenAPI manifest',
  expectedArtifactPath,
  requireSignature = true,
  requireClean = requireSignature,
} = {}) {
  if (!isObject(manifest)) {
    throw new Error(`${label} must be a JSON object`);
  }
  assertExactFields(manifest, COMMON_MANIFEST_FIELDS, label);
  if (manifest.version !== OPENAPI_MANIFEST_VERSION) {
    throw new Error(
      `${label} has unsupported version ${String(manifest.version)}; expected exactly ${OPENAPI_MANIFEST_VERSION}`,
    );
  }
  if (
    !Number.isSafeInteger(manifest.generated_unix_ms) ||
    manifest.generated_unix_ms <= 0
  ) {
    throw new Error(`${label} generated_unix_ms must be a positive safe integer`);
  }

  const artifact = manifest.artifact;
  if (!isObject(artifact)) {
    throw new Error(`${label} artifact must be an object`);
  }
  assertExactFields(artifact, ARTIFACT_FIELDS, `${label} artifact`);
  validateArtifactPath(artifact.path, `${label} artifact.path`);
  if (
    expectedArtifactPath !== undefined &&
    artifact.path !== expectedArtifactPath
  ) {
    throw new Error(
      `${label} artifact.path (${artifact.path}) does not match ${expectedArtifactPath}`,
    );
  }

  const bytes = toBuffer(artifactBytes, 'artifactBytes');
  if (!Number.isSafeInteger(artifact.bytes) || artifact.bytes < 0) {
    throw new Error(`${label} artifact.bytes must be a non-negative safe integer`);
  }
  if (artifact.bytes !== bytes.length) {
    throw new Error(
      `${label} artifact.bytes (${artifact.bytes}) does not match the artifact (${bytes.length})`,
    );
  }
  if (
    typeof artifact.sha256_hex !== 'string' ||
    !LOWER_SHA256_HEX.test(artifact.sha256_hex)
  ) {
    throw new Error(
      `${label} artifact.sha256_hex must be exactly 64 lowercase hexadecimal characters`,
    );
  }
  const expectedSha256 = createHash('sha256').update(bytes).digest('hex');
  if (artifact.sha256_hex !== expectedSha256) {
    throw new Error(
      `${label} artifact.sha256_hex (${artifact.sha256_hex}) does not match the artifact (${expectedSha256})`,
    );
  }
  if (
    typeof artifact.blake3_hex !== 'string' ||
    !LOWER_BLAKE3_HEX.test(artifact.blake3_hex)
  ) {
    throw new Error(
      `${label} artifact.blake3_hex must be exactly 64 lowercase hexadecimal characters`,
    );
  }
  const expectedBlake3 = computeOpenApiBlake3Hex(bytes);
  if (artifact.blake3_hex !== expectedBlake3) {
    throw new Error(
      `${label} artifact.blake3_hex (${artifact.blake3_hex}) does not match the artifact (${expectedBlake3})`,
    );
  }

  const signature = artifact.signature;
  if (signature === null) {
    if (requireSignature) {
      throw new Error(`${label} is missing artifact.signature`);
    }
  } else {
    validateSignatureEnvelope(signature, `${label} artifact.signature`);
  }
  validateOpenApiGeneratorProvenance(manifest, {
    label,
    signed: signature !== null,
    requireClean,
  });
  return {artifactBytes: bytes, signature};
}

/**
 * Encode the byte-exact V2 Ed25519 signing payload.
 *
 * Each component is `u64_le(label length) || label || u64_le(value length) ||
 * value`, in the fixed order below. The final value is the raw artifact.
 */
export function encodeOpenApiManifestSigningPayload({
  manifest,
  artifactBytes,
  label = 'OpenAPI manifest',
  expectedArtifactPath,
} = {}) {
  const validated = validateOpenApiManifestV2({
    manifest,
    artifactBytes,
    label,
    expectedArtifactPath,
    requireSignature: false,
    requireClean: false,
  });
  const components = [
    ['domain', OPENAPI_MANIFEST_SIGNATURE_DOMAIN_V2],
    ['version', String(manifest.version)],
    ['generated_unix_ms', String(manifest.generated_unix_ms)],
    ['generator_commit', manifest.generator_commit ?? 'null'],
    ['generator_dirty', manifest.generator_dirty ? 'true' : 'false'],
    [
      'generator_source_sha256_hex',
      manifest.generator_source_sha256_hex ?? 'null',
    ],
    ['artifact.path', manifest.artifact.path],
    ['artifact.bytes', String(manifest.artifact.bytes)],
    ['artifact.sha256_hex', manifest.artifact.sha256_hex],
    ['artifact.blake3_hex', manifest.artifact.blake3_hex],
    ['artifact.content', validated.artifactBytes],
  ];
  return Buffer.concat(
    components.map(([componentLabel, value]) =>
      encodeComponent(componentLabel, value),
    ),
  );
}

export function verifyOpenApiManifestV2(options = {}) {
  const validated = validateOpenApiManifestV2(options);
  if (validated.signature === null) {
    return {
      signed: false,
      signingPayload: encodeOpenApiManifestSigningPayload(options),
    };
  }
  const signingPayload = encodeOpenApiManifestSigningPayload(options);
  verifyOpenApiSignature({
    algorithm: validated.signature.algorithm,
    publicKeyHex: validated.signature.public_key_hex,
    signatureHex: validated.signature.signature_hex,
    payload: signingPayload,
  });
  return {signed: true, signingPayload};
}

function validateSignatureEnvelope(signature, label) {
  if (!isObject(signature)) {
    throw new Error(`${label} must be an object or null`);
  }
  assertExactFields(signature, SIGNATURE_FIELDS, label);
  if (signature.algorithm !== 'ed25519') {
    throw new Error(`${label}.algorithm must be exactly ed25519`);
  }
  if (
    typeof signature.public_key_hex !== 'string' ||
    !LOWER_ED25519_PUBLIC_KEY_HEX.test(signature.public_key_hex)
  ) {
    throw new Error(
      `${label}.public_key_hex must be exactly 64 lowercase hexadecimal characters`,
    );
  }
  validateOpenApiEd25519PublicKeyHex(signature.public_key_hex);
  if (
    typeof signature.signature_hex !== 'string' ||
    !LOWER_ED25519_SIGNATURE_HEX.test(signature.signature_hex)
  ) {
    throw new Error(
      `${label}.signature_hex must be exactly 128 lowercase hexadecimal characters`,
    );
  }
}

function validateArtifactPath(value, label) {
  if (value !== 'torii.json') {
    throw new Error(`${label} must be exactly torii.json`);
  }
}

function assertExactFields(value, expected, label) {
  const expectedSet = new Set(expected);
  const actual = Object.keys(value);
  for (const field of expected) {
    if (!Object.hasOwn(value, field)) {
      throw new Error(`${label} is missing required field ${field}`);
    }
  }
  const unknown = actual.filter((field) => !expectedSet.has(field));
  if (unknown.length > 0) {
    throw new Error(`${label} contains unknown field(s): ${unknown.join(', ')}`);
  }
}

function encodeComponent(label, value) {
  const labelBytes = Buffer.from(label, 'utf8');
  const valueBytes = Buffer.isBuffer(value)
    ? value
    : Buffer.from(value, 'utf8');
  const prefix = Buffer.allocUnsafe(16);
  prefix.writeBigUInt64LE(BigInt(labelBytes.length), 0);
  prefix.writeBigUInt64LE(BigInt(valueBytes.length), 8);
  return Buffer.concat([
    prefix.subarray(0, 8),
    labelBytes,
    prefix.subarray(8),
    valueBytes,
  ]);
}

function toBuffer(value, label) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (value instanceof Uint8Array) {
    return Buffer.from(value);
  }
  if (typeof value === 'string') {
    return Buffer.from(value, 'utf8');
  }
  throw new TypeError(`${label} must be a Buffer, Uint8Array, or string`);
}

function isObject(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

export function scanJsonRejectDuplicateKeys(text, label) {
  let index = 0;

  function fail(message) {
    throw new Error(`${label} contains invalid JSON at offset ${index}: ${message}`);
  }

  function skipWhitespace() {
    while (
      index < text.length &&
      (text[index] === ' ' ||
        text[index] === '\n' ||
        text[index] === '\r' ||
        text[index] === '\t')
    ) {
      index += 1;
    }
  }

  function parseString() {
    const start = index;
    if (text[index] !== '"') {
      fail('expected a string');
    }
    index += 1;
    while (index < text.length) {
      const code = text.charCodeAt(index);
      if (code === 0x22) {
        index += 1;
        try {
          return JSON.parse(text.slice(start, index));
        } catch (error) {
          fail(error.message ?? String(error));
        }
      }
      if (code < 0x20) {
        fail('unescaped control character in string');
      }
      if (code === 0x5c) {
        index += 1;
        if (index >= text.length) {
          fail('unterminated escape sequence');
        }
        const escape = text[index];
        if (escape === 'u') {
          const unicode = text.slice(index + 1, index + 5);
          if (!/^[0-9a-fA-F]{4}$/.test(unicode)) {
            fail('invalid Unicode escape');
          }
          index += 5;
          continue;
        }
        if (!'"\\/bfnrt'.includes(escape)) {
          fail('invalid string escape');
        }
      }
      index += 1;
    }
    fail('unterminated string');
  }

  function parseNumber() {
    const remainder = text.slice(index);
    const match = remainder.match(
      /^-?(?:0|[1-9][0-9]*)(?:\.[0-9]+)?(?:[eE][+-]?[0-9]+)?/,
    );
    if (!match) {
      fail('invalid number');
    }
    index += match[0].length;
  }

  function parseArray() {
    index += 1;
    skipWhitespace();
    if (text[index] === ']') {
      index += 1;
      return;
    }
    while (index < text.length) {
      parseValue();
      skipWhitespace();
      if (text[index] === ']') {
        index += 1;
        return;
      }
      if (text[index] !== ',') {
        fail('expected comma or closing bracket');
      }
      index += 1;
      skipWhitespace();
    }
    fail('unterminated array');
  }

  function parseObject() {
    index += 1;
    skipWhitespace();
    const keys = new Set();
    if (text[index] === '}') {
      index += 1;
      return;
    }
    while (index < text.length) {
      const key = parseString();
      if (keys.has(key)) {
        throw new Error(`${label} contains duplicate JSON member ${JSON.stringify(key)}`);
      }
      keys.add(key);
      skipWhitespace();
      if (text[index] !== ':') {
        fail('expected colon after object member name');
      }
      index += 1;
      parseValue();
      skipWhitespace();
      if (text[index] === '}') {
        index += 1;
        return;
      }
      if (text[index] !== ',') {
        fail('expected comma or closing brace');
      }
      index += 1;
      skipWhitespace();
    }
    fail('unterminated object');
  }

  function parseValue() {
    skipWhitespace();
    const token = text[index];
    if (token === '{') {
      parseObject();
    } else if (token === '[') {
      parseArray();
    } else if (token === '"') {
      parseString();
    } else if (token === '-' || (token >= '0' && token <= '9')) {
      parseNumber();
    } else if (text.startsWith('true', index)) {
      index += 4;
    } else if (text.startsWith('false', index)) {
      index += 5;
    } else if (text.startsWith('null', index)) {
      index += 4;
    } else {
      fail('unexpected token');
    }
  }

  skipWhitespace();
  parseValue();
  skipWhitespace();
  if (index !== text.length) {
    fail('trailing content');
  }
}
