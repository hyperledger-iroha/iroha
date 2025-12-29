import {test} from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdtemp, mkdir, readFile, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, resolve} from 'node:path';

import {isIsoTimestamp, verifyOpenApiVersions} from '../verify-openapi-versions.mjs';

test('verifyOpenApiVersions validates recorded metadata', async () => {
  const context = await setupFixture();
  await verifyOpenApiVersions(context);
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
  const divergentContent = JSON.stringify({generated: 'current-only'}, null, 2);
  const divergentBuffer = Buffer.from(divergentContent, 'utf8');
  const divergentSha = createHash('sha256').update(divergentBuffer).digest('hex');
  const divergentBlake3 = 'bb278ba70b4eeb85dc30fa2d0ef67d47';

  await writeFile(currentSpecPath, divergentContent, 'utf8');
  await writeManifest(
    join(context.outputDir, 'versions', 'current', 'manifest.json'),
    'versions/current/torii.json',
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

  const specContent = JSON.stringify({generated: true}, null, 2);
  const specBytes = Buffer.from(specContent, 'utf8');
  const sha256 = createHash('sha256').update(specBytes).digest('hex');
  const timestamp = '2025-11-10T04:39:40.260Z';
  const signature = {
    algorithm: 'ed25519',
    public_key_hex: '00',
    signature_hex: '11',
  };
  const blake3Hex = '66278ba70b4eeb85dc30fa2d0ef67d47';

  await writeFile(join(outputDir, 'torii.json'), specContent, 'utf8');
  await writeFile(join(currentDir, 'torii.json'), specContent, 'utf8');
  await writeFile(join(archivedDir, 'torii.json'), specContent, 'utf8');

  await writeManifest(join(outputDir, 'manifest.json'), 'torii.json', {
    sha256,
    blake3: blake3Hex,
    signature,
    bytes: specBytes.length,
  });
  await writeManifest(join(currentDir, 'manifest.json'), 'versions/current/torii.json', {
    sha256,
    blake3: blake3Hex,
    signature,
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
        signaturePublicKeyHex: '00',
        signatureHex: '11',
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
        signaturePublicKeyHex: '00',
        signatureHex: '11',
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
    version: 1,
    generated_unix_ms: 123,
    generator_commit: options.generatorCommit ?? 'fixture-commit',
    artifact: {
      path: artifactPath,
      bytes: options.bytes ?? 0,
      sha256_hex: options.sha256,
      blake3_hex: options.blake3,
      signature: {
        algorithm: options.signature.algorithm,
        public_key_hex: options.signature.public_key_hex,
        signature_hex: options.signature.signature_hex,
      },
    },
  };
  await writeFile(manifestPath, JSON.stringify(payload, null, 2), 'utf8');
}

async function corruptManifest(manifestPath, mutator) {
  const raw = await readFile(manifestPath, 'utf8');
  const manifest = JSON.parse(raw);
  mutator(manifest);
  await writeFile(manifestPath, JSON.stringify(manifest, null, 2), 'utf8');
}
