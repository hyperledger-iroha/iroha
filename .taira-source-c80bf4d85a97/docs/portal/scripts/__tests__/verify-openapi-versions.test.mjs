import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdtemp, mkdir, readFile, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';

import {isIsoTimestamp, verifyOpenApiVersions} from '../verify-openapi-versions.mjs';
import {computeOpenApiBlake3Hex} from '../lib/openapi-manifest-v2.mjs';
import {validateOpenApiGeneratorProvenance} from '../lib/openapi-provenance.mjs';
import {attachOpenApiManifestSignature} from './helpers/openapi-signing.mjs';

function releaseSpec(marker) {
  return JSON.stringify(
    {
      openapi: '3.1.0',
      info: {title: `Torii ${marker}`, version: '1.0.0'},
      paths: {
        [`/${marker}`]: {
          get: {responses: {'200': {description: 'ok'}}},
        },
      },
      components: {schemas: {Fixture: {type: 'object'}}},
    },
    null,
    2,
  );
}

test('OpenAPI provenance accepts explicit V2 dirty and clean state', () => {
  assert.deepEqual(
    validateOpenApiGeneratorProvenance({
      version: 2,
      generator_commit: null,
      generator_dirty: true,
      generator_source_sha256_hex: 'ab'.repeat(32),
    }),
    {
      dirty: true,
      commit: null,
      sourceSha256Hex: 'ab'.repeat(32),
    },
  );
  assert.deepEqual(
    validateOpenApiGeneratorProvenance({
      version: 2,
      generator_commit: 'cd'.repeat(20),
      generator_dirty: false,
    }),
    {
      dirty: false,
      commit: 'cd'.repeat(20),
      sourceSha256Hex: null,
    },
  );
});

test('OpenAPI provenance rejects dirty-state ambiguity and release smuggling', () => {
  for (const [name, manifest, options, pattern] of [
    [
      'dirty commit alias',
      {
        generator_commit: 'pretend-clean',
        generator_dirty: true,
        generator_source_sha256_hex: 'ab'.repeat(32),
      },
      {},
      /generator_commit to null/i,
    ],
    [
      'missing dirty digest',
      {generator_commit: null, generator_dirty: true},
      {},
      /requires generator_source_sha256_hex/i,
    ],
    [
      'uppercase dirty digest',
      {
        generator_commit: null,
        generator_dirty: true,
        generator_source_sha256_hex: 'AB'.repeat(32),
      },
      {},
      /64 lowercase hexadecimal/i,
    ],
    [
      'dirty signed manifest',
      {
        generator_commit: null,
        generator_dirty: true,
        generator_source_sha256_hex: 'ab'.repeat(32),
      },
      {signed: true},
      /must not be signed/i,
    ],
    [
      'dirty release manifest',
      {
        generator_commit: null,
        generator_dirty: true,
        generator_source_sha256_hex: 'ab'.repeat(32),
      },
      {requireClean: true},
      /cannot be release-verified/i,
    ],
    [
      'dirty field type confusion',
      {generator_commit: 'clean', generator_dirty: 'false'},
      {},
      /generator_dirty must be boolean/i,
    ],
    [
      'short clean commit',
      {generator_commit: 'ab'.repeat(19)},
      {},
      /exactly 40 lowercase hexadecimal/i,
    ],
    [
      'uppercase clean commit',
      {generator_commit: 'AB'.repeat(20)},
      {},
      /exactly 40 lowercase hexadecimal/i,
    ],
    [
      'nonhex clean commit',
      {generator_commit: 'gg'.repeat(20)},
      {},
      /exactly 40 lowercase hexadecimal/i,
    ],
    [
      'padded clean commit',
      {generator_commit: ` ${'ab'.repeat(20)} `},
      {},
      /exactly 40 lowercase hexadecimal/i,
    ],
  ]) {
    assert.throws(
      () =>
        validateOpenApiGeneratorProvenance(
          {version: 2, generator_dirty: false, ...manifest},
          options,
        ),
      pattern,
      name,
    );
  }
});

test('verifyOpenApiVersions requires explicit unsigned opt-in for dirty provenance', async () => {
  const context = await setupFixture();
  for (const manifestPath of [
    join(context.outputDir, 'manifest.json'),
    join(context.outputDir, 'versions', 'current', 'manifest.json'),
  ]) {
    await corruptManifest(manifestPath, (manifest) => {
      manifest.generator_commit = null;
      manifest.generator_dirty = true;
      manifest.generator_source_sha256_hex = 'ab'.repeat(32);
      manifest.artifact.signature = null;
    });
  }
  const versionsPath = join(context.outputDir, 'versions.json');
  const versions = JSON.parse(await readFile(versionsPath, 'utf8'));
  for (const entry of versions.entries) {
    if (entry.label === 'latest' || entry.label === 'current') {
      entry.signed = false;
      entry.signatureAlgorithm = null;
      entry.signaturePublicKeyHex = null;
      entry.signatureHex = null;
    }
  }
  await writeFile(versionsPath, JSON.stringify(versions, null, 2), 'utf8');

  await assert.rejects(
    () => verifyOpenApiVersions(context),
    /dirty provenance cannot be release-verified/i,
  );
  await verifyOpenApiVersions({...context, allowUnsigned: true});
});

test('verifyOpenApiVersions validates recorded metadata', async () => {
  const context = await setupFixture();
  await verifyOpenApiVersions(context);
});

test('verifyOpenApiVersions rejects unknown root and entry fields', async () => {
  for (const mutate of [
    (manifest) => {
      manifest.legacyVersions = [];
    },
    (manifest) => {
      manifest.entries[0].legacyDigest = 'ab'.repeat(32);
    },
  ]) {
    const context = await setupFixture(mutate);
    await assert.rejects(
      () => verifyOpenApiVersions(context),
      /unknown field/i,
    );
  }
});

test('verifyOpenApiVersions requires explicit nullable entry metadata', async () => {
  for (const field of [
    'blake3',
    'manifestPath',
    'signatureAlgorithm',
    'signaturePublicKeyHex',
    'signatureHex',
  ]) {
    const context = await setupFixture((manifest) => {
      const historical = manifest.entries.find((entry) => entry.label === '2025-q4');
      delete historical[field];
    });
    await assert.rejects(
      () => verifyOpenApiVersions(context),
      new RegExp(`missing field.*${field}`, 'i'),
    );
  }
});

test('verifyOpenApiVersions rejects malformed manifest timestamps through its returned promise', async () => {
  const context = await setupFixture((manifest) => {
    manifest.generatedAt = 'not-an-iso-timestamp';
  });

  await assert.rejects(
    () => verifyOpenApiVersions(context),
    /versions\.json generatedAt must be an ISO-8601 timestamp/i,
  );
});

test('verifyOpenApiVersions fails when the recorded digest is stale', async () => {
  const context = await setupFixture((manifest) => {
    for (const entry of manifest.entries) {
      if (entry.label === 'latest' || entry.label === 'current') {
        entry.sha256 = 'deadbeef';
      }
    }
  });
  await assert.rejects(
    () => verifyOpenApiVersions(context),
    /sha256/i,
  );
});

test('verifyOpenApiVersions fails when manifest metadata is stale', async () => {
  const context = await setupFixture();
  await corruptManifest(join(context.outputDir, 'manifest.json'), (manifest) => {
    manifest.artifact.bytes += 1;
  });
  await corruptManifest(
    join(context.outputDir, 'versions', 'current', 'manifest.json'),
    (manifest) => {
      manifest.artifact.sha256_hex = 'cafebabe';
    },
  );

  await assert.rejects(
    () => verifyOpenApiVersions(context),
    /manifest .*bytes|manifest .*sha256/i,
  );
});

test('isIsoTimestamp requires timezone and parseable value', () => {
  assert.ok(isIsoTimestamp('2025-11-10T04:39:40.260Z'));
  assert.ok(isIsoTimestamp('2025-11-10T04:39:40+00:00'));
  assert.ok(!isIsoTimestamp('2025-11-10'));
  assert.ok(!isIsoTimestamp('2025-11-10T04:39:40'));
  assert.ok(!isIsoTimestamp(''));
});

test('verifyOpenApiVersions rejects signed entries without blake3', async () => {
  const context = await setupFixture((manifest) => {
    for (const entry of manifest.entries) {
      if (entry.signed) {
        entry.blake3 = null;
      }
    }
  });
  await assert.rejects(
    () => verifyOpenApiVersions(context),
    /blake3/i,
  );
});

test('verifyOpenApiVersions rejects diverging latest/current aliases', async () => {
  const context = await setupFixture();
  const currentSpecPath = join(context.outputDir, 'versions', 'current', 'torii.json');
  const divergentContent = releaseSpec('current-only');
  const divergentBuffer = Buffer.from(divergentContent, 'utf8');
  const divergentSha = createHash('sha256').update(divergentBuffer).digest('hex');
  const divergentBlake3 = computeOpenApiBlake3Hex(divergentBuffer);

  await writeFile(currentSpecPath, divergentContent, 'utf8');
  await writeManifest(
    join(context.outputDir, 'versions', 'current', 'manifest.json'),
    'torii.json',
    {
      sha256: divergentSha,
      blake3: divergentBlake3,
      signature: {algorithm: 'ed25519', public_key_hex: '00', signature_hex: '11'},
      bytes: divergentBuffer.length,
    },
  );

  const versionsManifest = JSON.parse(await readFile(context.versionsFile, 'utf8'));
  const currentEntry = versionsManifest.entries.find((entry) => entry.label === 'current');
  currentEntry.sha256 = divergentSha;
  currentEntry.bytes = divergentBuffer.length;
  currentEntry.blake3 = divergentBlake3;
  await writeFile(context.versionsFile, JSON.stringify(versionsManifest, null, 2), 'utf8');

  await assert.rejects(
    () => verifyOpenApiVersions(context),
    /latest .*current .*digest/i,
  );
});

test('verifyOpenApiVersions rejects an empty OpenAPI stub', async () => {
  const context = await setupFixture();
  await writeFile(
    join(context.outputDir, 'torii.json'),
    JSON.stringify({
      openapi: '3.1.0',
      info: {title: 'Torii stub', version: '1.0.0'},
      paths: {},
      components: {},
    }),
    'utf8',
  );

  await assert.rejects(
    () => verifyOpenApiVersions(context),
    /empty\/stub specifications are forbidden/i,
  );
});

test('verifyOpenApiVersions rejects entries missing updatedAt', async () => {
  const context = await setupFixture((manifest) => {
    for (const entry of manifest.entries) {
      delete entry.updatedAt;
    }
  });

  await assert.rejects(verifyOpenApiVersions(context), /updatedat/i);
});

test('verifyOpenApiVersions rejects absolute spec paths', async () => {
  const absolutePath = resolve(tmpdir(), 'outside', 'spec.json');
  const context = await setupFixture((manifest) => {
    const latest = manifest.entries.find((entry) => entry.label === 'latest');
    if (latest) {
      latest.path = absolutePath;
    }
  });

  await assert.rejects(
    () => verifyOpenApiVersions(context),
    /must be relative/i,
  );
});

test('verifyOpenApiVersions rejects manifests escaping the output directory', async () => {
  const context = await setupFixture((manifest) => {
    const latest = manifest.entries.find((entry) => entry.label === 'latest');
    if (latest) {
      latest.manifestPath = '../manifest.json';
    }
  });

  await assert.rejects(
    () => verifyOpenApiVersions(context),
    /escapes the OpenAPI output directory/i,
  );
});

async function setupFixture(manifestMutator) {
  const root = await mkdtemp(join(tmpdir(), 'verify-openapi-versions-'));
  const outputDir = join(root, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const currentDir = join(versionsDir, 'current');
  const archivedDir = join(versionsDir, '2025-q4');

  await mkdir(currentDir, {recursive: true});
  await mkdir(archivedDir, {recursive: true});

  const specContent = releaseSpec('generated');
  const specBytes = Buffer.from(specContent, 'utf8');
  const sha256 = createHash('sha256').update(specBytes).digest('hex');
  const timestamp = '2025-11-10T04:39:40.260Z';
  const blake3Hex = computeOpenApiBlake3Hex(specBytes);

  await writeFile(join(outputDir, 'torii.json'), specContent, 'utf8');
  await writeFile(join(currentDir, 'torii.json'), specContent, 'utf8');
  await writeFile(join(archivedDir, 'torii.json'), specContent, 'utf8');

  const signature = await writeManifest(join(outputDir, 'manifest.json'), 'torii.json', {
    sha256,
    blake3: blake3Hex,
    artifactBytes: specBytes,
    bytes: specBytes.length,
  });
  await writeManifest(join(currentDir, 'manifest.json'), 'torii.json', {
    sha256,
    blake3: blake3Hex,
    artifactBytes: specBytes,
    bytes: specBytes.length,
  });

  const versionsManifest = {
    versions: ['2025-q4', 'current'],
    generatedAt: timestamp,
    entries: [
      {
        label: 'latest',
        path: 'torii.json',
        bytes: specBytes.length,
        sha256,
        blake3: blake3Hex,
        updatedAt: timestamp,
        signed: true,
        manifestPath: 'manifest.json',
        signatureAlgorithm: 'ed25519',
        signaturePublicKeyHex: signature.public_key_hex,
        signatureHex: signature.signature_hex,
      },
      {
        label: '2025-q4',
        path: 'versions/2025-q4/torii.json',
        bytes: specBytes.length,
        sha256,
        blake3: null,
        updatedAt: timestamp,
        signed: false,
        manifestPath: null,
        signatureAlgorithm: null,
        signaturePublicKeyHex: null,
        signatureHex: null,
      },
      {
        label: 'current',
        path: 'versions/current/torii.json',
        bytes: specBytes.length,
        sha256,
        blake3: blake3Hex,
        updatedAt: timestamp,
        signed: true,
        manifestPath: 'versions/current/manifest.json',
        signatureAlgorithm: 'ed25519',
        signaturePublicKeyHex: signature.public_key_hex,
        signatureHex: signature.signature_hex,
      },
    ],
  };

  if (manifestMutator) {
    manifestMutator(versionsManifest);
  }

  await writeFile(
    join(outputDir, 'versions.json'),
    JSON.stringify(versionsManifest, null, 2),
    'utf8',
  );

  return {
    outputDir,
    versionsDir,
    versionsFile: join(outputDir, 'versions.json'),
  };
}

async function writeManifest(manifestPath, artifactPath, options) {
  const payload = {
    version: 2,
    generated_unix_ms: 123,
    generator_commit: options.generatorCommit ?? 'ab'.repeat(20),
    generator_dirty: false,
    artifact: {
      path: artifactPath,
      bytes: options.bytes ?? 0,
      sha256_hex: options.sha256,
      blake3_hex: options.blake3,
      signature: options.signature ?? null,
    },
  };
  if (options.artifactBytes) {
    attachOpenApiManifestSignature(payload, options.artifactBytes);
  }
  await writeFile(manifestPath, JSON.stringify(payload, null, 2), 'utf8');
  return payload.artifact.signature;
}

async function corruptManifest(manifestPath, mutator) {
  const raw = await readFile(manifestPath, 'utf8');
  const manifest = JSON.parse(raw);
  mutator(manifest);
  await writeFile(manifestPath, JSON.stringify(manifest, null, 2), 'utf8');
}
