// SPDX-License-Identifier: Apache-2.0

import {createPublicKey, verify as verifySignatureNode} from 'node:crypto';

const ED25519_PUBLIC_KEY_DER_PREFIX = Buffer.from(
  '302a300506032b6570032100',
  'hex',
);
const ED25519_FIELD_MODULUS = (1n << 255n) - 19n;
const ED25519_WEAK_PUBLIC_KEYS = new Set([
  '0100000000000000000000000000000000000000000000000000000000000000',
  'c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac037a',
  '0000000000000000000000000000000000000000000000000000000000000080',
  '26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc05',
  'ecffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff7f',
  '26e8958fc2b227b045c3f489f2ef98f0d5dfac05d3c63339b13802886d53fc85',
  '0000000000000000000000000000000000000000000000000000000000000000',
  'c7176a703d4dd84fba3c0b760d10670f2a2053fa2c39ccc64ec7fd7792ac03fa',
]);

function toBuffer(payload) {
  if (Buffer.isBuffer(payload)) {
    return payload;
  }
  if (payload instanceof Uint8Array) {
    return Buffer.from(payload);
  }
  if (typeof payload === 'string') {
    return Buffer.from(payload, 'utf8');
  }
  throw new TypeError('payload must be a Buffer, Uint8Array, or string');
}

function decodeHex(hexString, label) {
  if (typeof hexString !== 'string' || hexString === '') {
    throw new Error(`${label} must be a non-empty hex string`);
  }
  if (hexString.length % 2 !== 0) {
    throw new Error(`${label} must have an even number of characters`);
  }
  if (!/^[0-9a-f]*$/.test(hexString)) {
    throw new Error(`${label} is not valid lowercase hex`);
  }
  let decoded;
  try {
    decoded = Buffer.from(hexString, 'hex');
  } catch (error) {
    throw new Error(`${label} is not valid hex: ${error.message ?? error}`);
  }
  return decoded;
}

function buildEd25519PublicKey(publicKeyHex) {
  const raw = validateOpenApiEd25519PublicKeyHex(publicKeyHex);
  const spki = Buffer.concat([ED25519_PUBLIC_KEY_DER_PREFIX, raw]);
  return createPublicKey({
    key: spki,
    format: 'der',
    type: 'spki',
  });
}

export function validateOpenApiEd25519PublicKeyHex(publicKeyHex) {
  const raw = decodeHex(publicKeyHex, 'public key');
  if (raw.length !== 32) {
    throw new Error('public key must be 32 bytes for ed25519');
  }
  validateEd25519PublicKey(raw);
  return raw;
}

function validateEd25519PublicKey(raw) {
  const encoded = raw.toString('hex');
  const sign = raw[31] >>> 7;
  const yBytes = Buffer.from(raw);
  yBytes[31] &= 0x7f;
  let y = 0n;
  for (let index = yBytes.length - 1; index >= 0; index -= 1) {
    y = (y << 8n) | BigInt(yBytes[index]);
  }
  if (y >= ED25519_FIELD_MODULUS) {
    throw new Error('public key is not a canonical Ed25519 point');
  }
  if (
    sign === 1 &&
    (y === 1n || y === ED25519_FIELD_MODULUS - 1n)
  ) {
    throw new Error('public key uses a noncanonical Ed25519 sign encoding');
  }
  if (ED25519_WEAK_PUBLIC_KEYS.has(encoded)) {
    throw new Error('public key is weak or small-order Ed25519 material');
  }
}

export function verifyOpenApiSignature({
  algorithm,
  publicKeyHex,
  signatureHex,
  payload,
}) {
  if (algorithm !== 'ed25519') {
    throw new Error(`unsupported manifest signature algorithm: ${algorithm ?? '(missing)'}`);
  }

  const publicKey = buildEd25519PublicKey(publicKeyHex);
  const signature = decodeHex(signatureHex, 'signature');
  const data = toBuffer(payload);
  const ok = verifySignatureNode(null, data, publicKey, signature);
  if (!ok) {
    throw new Error('signature verification failed');
  }
  return true;
}
