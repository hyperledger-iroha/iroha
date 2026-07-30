import {
  createHash,
  createPrivateKey,
  createPublicKey,
  sign as signNode,
} from 'node:crypto';

import {
  computeOpenApiBlake3Hex,
  encodeOpenApiManifestSigningPayload,
} from '../../lib/openapi-manifest-v2.mjs';

const ED25519_PRIVATE_KEY_DER_PREFIX = Buffer.from(
  '302e020100300506032b657004220420',
  'hex',
);
const ED25519_PUBLIC_KEY_DER_PREFIX = Buffer.from(
  '302a300506032b6570032100',
  'hex',
);

export const TEST_ED25519_PRIVATE_KEY_HEX =
  '8f2195b4c53a6d7e1f0cbd93a4e8f7650a1b2c3d4e5f60718293a4b5c6d7e8f1';

export function signPayload(payloadInput, {privateKeyHex = TEST_ED25519_PRIVATE_KEY_HEX} = {}) {
  const payload = Buffer.isBuffer(payloadInput)
    ? payloadInput
    : Buffer.from(payloadInput, 'utf8');
  const keyBytes = Buffer.from(privateKeyHex, 'hex');
  if (keyBytes.length !== 32) {
    throw new Error('ed25519 private key must be 32 bytes');
  }
  const privateKey = createPrivateKey({
    key: Buffer.concat([ED25519_PRIVATE_KEY_DER_PREFIX, keyBytes]),
    format: 'der',
    type: 'pkcs8',
  });
  const signature = signNode(null, payload, privateKey);
  const publicKeyDer = createPublicKey(privateKey).export({format: 'der', type: 'spki'});
  const publicKeyHex = publicKeyDer
    .slice(ED25519_PUBLIC_KEY_DER_PREFIX.length)
    .toString('hex');
  return {
    signatureHex: signature.toString('hex'),
    publicKeyHex,
    privateKeyHex,
  };
}

export function buildOpenApiManifest({
  artifactBytes,
  path = 'torii.json',
  generatedUnixMs = 1_700_000_000_000,
  generatorCommit = 'ab'.repeat(20),
  generatorDirty = false,
  generatorSourceSha256Hex = 'cd'.repeat(32),
  sha256Hex,
  blake3Hex,
  privateKeyHex = TEST_ED25519_PRIVATE_KEY_HEX,
  signed = true,
} = {}) {
  const bytes = Buffer.isBuffer(artifactBytes)
    ? artifactBytes
    : Buffer.from(artifactBytes, 'utf8');
  const manifest = {
    version: 2,
    generated_unix_ms: generatedUnixMs,
    generator_commit: generatorDirty ? null : generatorCommit,
    generator_dirty: generatorDirty,
    generator_source_sha256_hex: generatorSourceSha256Hex,
    artifact: {
      path,
      bytes: bytes.length,
      sha256_hex:
        sha256Hex ?? createHash('sha256').update(bytes).digest('hex'),
      blake3_hex: blake3Hex ?? computeOpenApiBlake3Hex(bytes),
      signature: null,
    },
  };
  if (signed) {
    attachOpenApiManifestSignature(manifest, bytes, {privateKeyHex});
  }
  return manifest;
}

export function attachOpenApiManifestSignature(
  manifest,
  artifactBytes,
  {privateKeyHex = TEST_ED25519_PRIVATE_KEY_HEX} = {},
) {
  manifest.artifact.signature = null;
  const payload = encodeOpenApiManifestSigningPayload({
    manifest,
    artifactBytes,
  });
  const signature = signPayload(payload, {privateKeyHex});
  manifest.artifact.signature = {
    algorithm: 'ed25519',
    public_key_hex: signature.publicKeyHex,
    signature_hex: signature.signatureHex,
  };
  return signature;
}
