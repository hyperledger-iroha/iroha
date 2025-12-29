// SPDX-License-Identifier: Apache-2.0

import {createPublicKey, verify as verifySignatureNode} from 'node:crypto';

const ED25519_PUBLIC_KEY_DER_PREFIX = Buffer.from(
  '302a300506032b6570032100',
  'hex',
);

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
  if (typeof hexString !== 'string' || hexString.trim() === '') {
    throw new Error(`${label} must be a non-empty hex string`);
  }
  const normalized = hexString.trim().toLowerCase();
  if (normalized.length % 2 !== 0) {
    throw new Error(`${label} must have an even number of characters`);
  }
  let decoded;
  try {
    decoded = Buffer.from(normalized, 'hex');
  } catch (error) {
    throw new Error(`${label} is not valid hex: ${error.message ?? error}`);
  }
  return decoded;
}

function buildEd25519PublicKey(publicKeyHex) {
  const raw = decodeHex(publicKeyHex, 'public key');
  if (raw.length !== 32) {
    throw new Error('public key must be 32 bytes for ed25519');
  }
  const spki = Buffer.concat([ED25519_PUBLIC_KEY_DER_PREFIX, raw]);
  return createPublicKey({
    key: spki,
    format: 'der',
    type: 'spki',
  });
}

export function verifyOpenApiSignature({
  algorithm,
  publicKeyHex,
  signatureHex,
  payload,
}) {
  if (typeof algorithm !== 'string' || algorithm.trim().toLowerCase() !== 'ed25519') {
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
