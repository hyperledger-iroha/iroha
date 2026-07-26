import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import test from 'node:test';

import {
  computeOpenApiBlake3Hex,
  encodeOpenApiManifestSigningPayload,
  parseOpenApiManifestV2Json,
  validateOpenApiManifestV2,
  verifyOpenApiManifestV2,
} from '../lib/openapi-manifest-v2.mjs';
import {buildOpenApiManifest} from './helpers/openapi-signing.mjs';

const ARTIFACT = Buffer.from('OpenAPI V2 deterministic fixture\n', 'utf8');

test('V2 signing payload has a fixed cross-language digest', () => {
  const manifest = buildOpenApiManifest({
    artifactBytes: ARTIFACT,
    generatedUnixMs: 1_700_000_000_000,
    generatorCommit: '11'.repeat(20),
    blake3Hex: 'd39d778cc128eafcfb89fbc286690120032a99eff18ffdec08c2a373e8618d41',
    signed: false,
  });
  const payload = encodeOpenApiManifestSigningPayload({
    manifest,
    artifactBytes: ARTIFACT,
  });
  assert.equal(
    createHash('sha256').update(payload).digest('hex'),
    '83148de11a5187d7f000770c29f4f18419163d654ed6aa5ffade0558ce47b5e5',
  );
});

test('V2 signature binds every manifest field and the artifact bytes', () => {
  const manifest = buildOpenApiManifest({artifactBytes: ARTIFACT});
  verifyOpenApiManifestV2({manifest, artifactBytes: ARTIFACT});

  const mutations = [
    ['generated_unix_ms', (value) => {
      value.generated_unix_ms += 1;
    }],
    ['generator_commit', (value) => {
      value.generator_commit = 'ac'.repeat(20);
    }],
    ['generator_dirty', (value) => {
      value.generator_dirty = true;
      value.generator_commit = null;
      value.generator_source_sha256_hex = 'cd'.repeat(32);
    }],
    ['generator_source_sha256_hex', (value) => {
      value.generator_source_sha256_hex = 'cd'.repeat(32);
    }],
    ['artifact.path', (value) => {
      value.artifact.path = 'other.json';
    }],
    ['artifact.bytes', (value) => {
      value.artifact.bytes += 1;
    }],
    ['artifact.sha256_hex', (value) => {
      value.artifact.sha256_hex = '00'.repeat(32);
    }],
    ['artifact.blake3_hex', (value) => {
      value.artifact.blake3_hex = '00'.repeat(32);
    }],
  ];
  for (const [field, mutate] of mutations) {
    const changed = structuredClone(manifest);
    mutate(changed);
    assert.throws(
      () =>
        verifyOpenApiManifestV2({
          manifest: changed,
          artifactBytes: ARTIFACT,
        }),
      undefined,
      field,
    );
  }
  assert.throws(() =>
    verifyOpenApiManifestV2({
      manifest,
      artifactBytes: Buffer.concat([ARTIFACT, Buffer.from('tampered')]),
    }));
});

test('V2 rejects V1, aliases, and unknown fields', () => {
  const base = buildOpenApiManifest({artifactBytes: ARTIFACT});
  for (const [name, mutate] of [
    ['V1', (value) => {
      value.version = 1;
    }],
    ['top-level unknown', (value) => {
      value.generator = 'legacy';
    }],
    ['artifact alias', (value) => {
      value.artifact.sha256Hex = value.artifact.sha256_hex;
      delete value.artifact.sha256_hex;
    }],
    ['signature alias', (value) => {
      value.artifact.signature.publicKeyHex =
        value.artifact.signature.public_key_hex;
      delete value.artifact.signature.public_key_hex;
    }],
  ]) {
    const manifest = structuredClone(base);
    mutate(manifest);
    assert.throws(
      () => validateOpenApiManifestV2({manifest, artifactBytes: ARTIFACT}),
      /version|unknown|missing/i,
      name,
    );
  }
});

test('V2 parser rejects duplicate member names before JSON normalization', () => {
  const manifest = buildOpenApiManifest({artifactBytes: ARTIFACT});
  const serialized = JSON.stringify(manifest);
  const duplicateTopLevel = serialized.replace(
    '"version":2',
    '"version":1,"version":2',
  );
  assert.throws(
    () => parseOpenApiManifestV2Json(duplicateTopLevel),
    /duplicate JSON member "version"/i,
  );

  const duplicateEscapedArtifactField = serialized.replace(
    '"path":"torii.json"',
    '"p\\u0061th":"other.json","path":"torii.json"',
  );
  assert.throws(
    () => parseOpenApiManifestV2Json(duplicateEscapedArtifactField),
    /duplicate JSON member "path"/i,
  );
});

test('V2 recomputes BLAKE3 from the exact artifact bytes', () => {
  assert.equal(
    computeOpenApiBlake3Hex(ARTIFACT),
    'd39d778cc128eafcfb89fbc286690120032a99eff18ffdec08c2a373e8618d41',
  );
  const manifest = buildOpenApiManifest({
    artifactBytes: ARTIFACT,
    signed: false,
  });
  manifest.artifact.blake3_hex = '00'.repeat(32);
  assert.throws(
    () =>
      validateOpenApiManifestV2({
        manifest,
        artifactBytes: ARTIFACT,
        requireSignature: false,
      }),
    /blake3_hex .* does not match the artifact/i,
  );
});

test('V2 rejects unsafe paths, malformed digests, keys, and signatures', () => {
  const base = buildOpenApiManifest({artifactBytes: ARTIFACT});
  for (const artifactPath of [
    '',
    '../torii.json',
    './torii.json',
    '/torii.json',
    'C:/torii.json',
    'versions\\torii.json',
    'versions//torii.json',
  ]) {
    const manifest = structuredClone(base);
    manifest.artifact.path = artifactPath;
    assert.throws(
      () => validateOpenApiManifestV2({manifest, artifactBytes: ARTIFACT}),
      /exactly torii\.json/i,
      artifactPath,
    );
  }
  for (const [field, invalid] of [
    ['sha256_hex', 'AA'.repeat(32)],
    ['sha256_hex', '00'],
    ['blake3_hex', 'BB'.repeat(32)],
    ['blake3_hex', '11'],
  ]) {
    const manifest = structuredClone(base);
    manifest.artifact[field] = invalid;
    assert.throws(
      () => validateOpenApiManifestV2({manifest, artifactBytes: ARTIFACT}),
      /lowercase hexadecimal/i,
      `${field}=${invalid}`,
    );
  }
  for (const [field, invalid] of [
    ['public_key_hex', 'AA'.repeat(32)],
    ['public_key_hex', '00'],
    ['signature_hex', 'BB'.repeat(64)],
    ['signature_hex', '11'],
  ]) {
    const manifest = structuredClone(base);
    manifest.artifact.signature[field] = invalid;
    assert.throws(
      () => validateOpenApiManifestV2({manifest, artifactBytes: ARTIFACT}),
      /lowercase hexadecimal/i,
      `${field}=${invalid}`,
    );
  }
  const algorithm = structuredClone(base);
  algorithm.artifact.signature.algorithm = 'Ed25519';
  assert.throws(
    () => validateOpenApiManifestV2({manifest: algorithm, artifactBytes: ARTIFACT}),
    /exactly ed25519/i,
  );
  const forged = structuredClone(base);
  forged.artifact.signature.signature_hex = '00'.repeat(64);
  assert.throws(
    () => verifyOpenApiManifestV2({manifest: forged, artifactBytes: ARTIFACT}),
    /signature verification failed/i,
  );

  for (const publicKeyHex of [
    `01${'00'.repeat(31)}`,
    '00'.repeat(32),
    `ed${'ff'.repeat(30)}7f`,
  ]) {
    const weak = structuredClone(base);
    weak.artifact.signature.public_key_hex = publicKeyHex;
    assert.throws(
      () => validateOpenApiManifestV2({manifest: weak, artifactBytes: ARTIFACT}),
      /weak|small-order|canonical/i,
    );
  }
});
