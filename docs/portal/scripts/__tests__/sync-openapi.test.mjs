import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, readFile, access, mkdir, symlink, utimes, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {dirname, join, resolve} from 'node:path';

import {computeOpenApiBlake3Hex} from '../lib/openapi-manifest-v2.mjs';
import {defaultRepoRoot, parseArgs, syncOpenApi} from '../sync-openapi.mjs';
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

test('default repository root contains the Cargo workspace', async () => {
  await access(join(defaultRepoRoot, 'Cargo.toml'));
});

test('parseArgs handles version, latest, and mirrors', () => {
  const options = parseArgs([
    '--version=2025-q4',
    '--latest',
    '--mirror=current',
    '--mirror=2025-q3',
    '--allow-unsigned',
    '--require-signed',
    '--allowed-signers=operator/openapi-signers.json',
  ]);

  assert.equal(options.version, '2025-q4');
  assert.equal(options.latest, true);
  assert.deepEqual(options.mirrors, ['current', '2025-q3']);
  assert.equal(options.requireSigned, true);
  assert.equal(
    options.allowedSignersFile,
    resolve('operator/openapi-signers.json'),
  );

  assert.throws(() => parseArgs(['--mirror=']), /mirror label must not be empty/);
  assert.throws(() => parseArgs(['--version=../../escape']), /version label must/);
  assert.throws(() => parseArgs(['--mirror=latest']), /reserved/);
  assert.throws(() => parseArgs(['--allowed-signers=']), /path must not be empty/);
  assert.throws(() => parseArgs(['--unknown']), /unknown argument/);
});

test('parseArgs can disable signature enforcement explicitly', () => {
  const options = parseArgs(['--allow-unsigned', '--latest']);
  assert.equal(options.requireSigned, false);
  assert.throws(
    () => parseArgs(['--version=current']),
    /requires --latest/,
  );
});

test('syncOpenApi rejects current without latest before generation or tracked writes', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-current-without-latest-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const sentinelPath = join(outputDir, 'sentinel.txt');
  let generated = false;
  await mkdir(outputDir, {recursive: true});
  await writeFile(sentinelPath, 'unchanged', 'utf8');

  await assert.rejects(
    () =>
      syncOpenApi(
        {version: 'current', latest: false, mirrors: [], requireSigned: false},
        {
          repoRoot: tempRoot,
          outputDir,
          versionsDir,
          async generateSpec() {
            generated = true;
          },
        },
      ),
    /requires latest=true/,
  );

  assert.equal(generated, false);
  assert.equal(await readFile(sentinelPath, 'utf8'), 'unchanged');
});

test('syncOpenApi rejects an empty generated stub before tracked writes', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-empty-stub-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const sentinelPath = join(outputDir, 'sentinel.txt');
  await mkdir(outputDir, {recursive: true});
  await writeFile(sentinelPath, 'unchanged', 'utf8');

  await assert.rejects(
    () =>
      syncOpenApi(
        {version: 'current', latest: true, mirrors: [], requireSigned: false},
        {
          repoRoot: tempRoot,
          outputDir,
          versionsDir,
          async generateSpec(_, outputFile) {
            await writeFile(
              outputFile,
              JSON.stringify({
                openapi: '3.1.0',
                info: {title: 'Torii stub', version: '1.0.0'},
                paths: {},
                components: {},
              }),
              'utf8',
            );
          },
        },
      ),
    /empty\/stub specifications are forbidden/i,
  );

  assert.equal(await readFile(sentinelPath, 'utf8'), 'unchanged');
  await assert.rejects(() => access(join(versionsDir, 'current')));
});

test('syncOpenApi rejects unknown fields in the previous versions index', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-unknown-index-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const freshSpec = releaseSpec('fresh');

  await writeCanonicalManifest(outputDir, freshSpec, {signature: null});
  await writeAllowedSigners(outputDir, []);
  await writeFile(
    join(outputDir, 'versions.json'),
    JSON.stringify(
      {
        versions: [],
        generatedAt: '2025-01-01T00:00:00.000Z',
        entries: [],
        legacyEntries: [],
      },
      null,
      2,
    ),
    'utf8',
  );

  await assert.rejects(
    () =>
      syncOpenApi(
        {
          version: 'current',
          latest: true,
          mirrors: [],
          requireSigned: false,
        },
        testContext(tempRoot, outputDir, versionsDir, freshSpec),
      ),
    /unknown field/i,
  );
});

test('syncOpenApi mirrors specs into multiple version directories', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');

  const fakeSpec = releaseSpec('generated');

  const manifestDir = join(outputDir);
  await mkdir(manifestDir, {recursive: true});
  const specBytes = Buffer.from(fakeSpec, 'utf8');
  const sha256 = createHash('sha256').update(specBytes).digest('hex');
  const canonicalSignature = signatureFor(fakeSpec);
  const canonicalManifest = {
    version: 2,
    generated_unix_ms: 123,
    generator_commit: 'ab'.repeat(20),
    generator_dirty: false,
    generator_source_sha256_hex: 'cd'.repeat(32),
    artifact: {
      path: 'torii.json',
      bytes: specBytes.length,
      sha256_hex: sha256,
      blake3_hex: computeOpenApiBlake3Hex(specBytes),
      signature: canonicalSignature,
    },
  };
  await writeFile(join(manifestDir, 'manifest.json'), JSON.stringify(canonicalManifest, null, 2), 'utf8');
  const canonicalManifestBytes = await readFile(join(manifestDir, 'manifest.json'));
  await writeAllowedSigners(outputDir, [canonicalSignature.public_key_hex]);

  await syncOpenApi(
    {
      version: '2025-q4',
      latest: true,
      mirrors: ['current', '2025-q3', '2025-q4', 'current'],
    },
    {
      repoRoot: tempRoot,
      outputDir,
      versionsDir,
      async generateSpec(_, outputFile) {
        await mkdir(dirname(outputFile), {recursive: true});
        await writeFile(outputFile, fakeSpec, 'utf8');
      },
    }
  );

  const baseContent = await readFile(join(versionsDir, '2025-q4', 'torii.json'), 'utf8');
  const mirrorContent = await readFile(join(versionsDir, 'current', 'torii.json'), 'utf8');
  const otherMirrorContent = await readFile(join(versionsDir, '2025-q3', 'torii.json'), 'utf8');
  const latestContent = await readFile(join(outputDir, 'torii.json'), 'utf8');

  assert.equal(baseContent, fakeSpec);
  assert.equal(mirrorContent, fakeSpec);
  assert.equal(otherMirrorContent, fakeSpec);
  assert.equal(latestContent, fakeSpec);

  const versionsManifest = JSON.parse(
    await readFile(join(outputDir, 'versions.json'), 'utf8')
  );
  assert.deepEqual(versionsManifest.versions, ['2025-q3', '2025-q4', 'current']);
  assert.ok(versionsManifest.generatedAt, 'generatedAt timestamp recorded');
  assert.ok(Array.isArray(versionsManifest.entries), 'entries array recorded');
  const entryLabels = versionsManifest.entries.map((entry) => entry.label);
  assert.deepEqual(entryLabels, ['latest', '2025-q3', '2025-q4', 'current']);
  const latestEntry = versionsManifest.entries.find((entry) => entry.label === 'latest');
  assert.equal(latestEntry.path, 'torii.json');
  assert.equal(latestEntry.sha256.toLowerCase(), sha256.toLowerCase());
  assert.equal(latestEntry.manifestPath, 'manifest.json');
  assert.equal(latestEntry.signatureAlgorithm, 'ed25519');
  assert.equal(latestEntry.signed, true);
  assert.equal(latestEntry.blake3, computeOpenApiBlake3Hex(specBytes));
  assert.equal(latestEntry.signaturePublicKeyHex, canonicalSignature.public_key_hex);
  assert.equal(latestEntry.signatureHex, canonicalSignature.signature_hex);

  await access(join(versionsDir, '2025-q4', 'torii.json'));
  await access(join(versionsDir, 'current', 'torii.json'));
  await access(join(versionsDir, '2025-q3', 'torii.json'));

  const versionManifest = JSON.parse(
    await readFile(join(versionsDir, '2025-q4', 'manifest.json'), 'utf8')
  );
  assert.equal(versionManifest.artifact.path, 'torii.json');
  assert.equal(versionManifest.artifact.sha256_hex.toLowerCase(), sha256.toLowerCase());
  assert.deepEqual(
    await readFile(join(versionsDir, '2025-q4', 'manifest.json')),
    canonicalManifestBytes,
    'version publication must copy the signed manifest byte-for-byte',
  );

  const mirrorManifest = JSON.parse(
    await readFile(join(versionsDir, 'current', 'manifest.json'), 'utf8')
  );
  assert.equal(mirrorManifest.artifact.path, 'torii.json');
});

test('syncOpenApi rejects unsigned publications by default', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-unsigned-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const versionDir = join(versionsDir, '2025-q4');
  const existingSpec = releaseSpec('existing');
  await mkdir(versionDir, {recursive: true});
  await writeFile(join(versionDir, 'torii.json'), existingSpec, 'utf8');

  await assert.rejects(
    () =>
      syncOpenApi(
        {
          version: '2025-q4',
          latest: false,
          mirrors: [],
        },
        {
          repoRoot: tempRoot,
          outputDir,
          versionsDir,
          async generateSpec(_, outputFile) {
            await mkdir(dirname(outputFile), {recursive: true});
            await writeFile(outputFile, releaseSpec('generated'), 'utf8');
          },
        },
    ),
    /manifest .*not found|missing signature/i,
  );

  assert.equal(
    await readFile(join(versionDir, 'torii.json'), 'utf8'),
    existingSpec,
    'manifest validation failure must not partially overwrite the tracked snapshot',
  );
});

test('syncOpenApi allows unsigned manifests only when opted-in', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-unsigned-allowed-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const fakeSpec = releaseSpec('generated');
  await writeCanonicalManifest(outputDir, fakeSpec, {signature: null});
  await writeAllowedSigners(outputDir, []);

  await syncOpenApi(
    {
      version: '2025-q4',
      latest: true,
      mirrors: ['current'],
      requireSigned: false,
    },
    {
      repoRoot: tempRoot,
      outputDir,
      versionsDir,
      async generateSpec(_, outputFile) {
        await mkdir(dirname(outputFile), {recursive: true});
        await writeFile(outputFile, fakeSpec, 'utf8');
      },
    }
  );

  const manifestPath = join(versionsDir, '2025-q4', 'manifest.json');
  assert.equal(await pathExists(manifestPath), true);
  const versionManifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  assert.equal(versionManifest.artifact.signature, null);
  assert.equal(versionManifest.artifact.path, 'torii.json');
  const versionsManifest = JSON.parse(
    await readFile(join(outputDir, 'versions.json'), 'utf8')
  );
  const entry = versionsManifest.entries.find((candidate) => candidate.label === '2025-q4');
  assert.equal(entry?.signed, false);
  assert.equal(entry?.manifestPath, 'versions/2025-q4/manifest.json');
});

test('syncOpenApi rejects a forged signature before changing tracked snapshots', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-forged-signature-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const versionDir = join(versionsDir, '2025-q4');
  const existingSpec = releaseSpec('existing');
  const existingManifest = JSON.stringify({sentinel: true});
  const freshSpec = releaseSpec('fresh');

  await mkdir(versionDir, {recursive: true});
  await writeFile(join(versionDir, 'torii.json'), existingSpec, 'utf8');
  await writeFile(join(versionDir, 'manifest.json'), existingManifest, 'utf8');
  await writeCanonicalManifest(outputDir, freshSpec, {
    signature: signatureFor(releaseSpec('different')),
  });

  await assert.rejects(
    () =>
      syncOpenApi(
        {version: '2025-q4', latest: false, mirrors: []},
        testContext(tempRoot, outputDir, versionsDir, freshSpec),
      ),
    /signature verification failed/i,
  );

  assert.equal(await readFile(join(versionDir, 'torii.json'), 'utf8'), existingSpec);
  assert.equal(await readFile(join(versionDir, 'manifest.json'), 'utf8'), existingManifest);
});

test('syncOpenApi rejects an artifact byte-count mismatch before changing tracked snapshots', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-byte-mismatch-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const versionDir = join(versionsDir, '2025-q4');
  const existingSpec = releaseSpec('existing');
  const freshSpec = releaseSpec('fresh');

  await mkdir(versionDir, {recursive: true});
  await writeFile(join(versionDir, 'torii.json'), existingSpec, 'utf8');
  await writeCanonicalManifest(outputDir, freshSpec, {
    signature: signatureFor(freshSpec),
  });
  const manifestPath = join(outputDir, 'manifest.json');
  const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
  manifest.artifact.bytes += 1;
  await writeFile(manifestPath, JSON.stringify(manifest, null, 2), 'utf8');

  await assert.rejects(
    () =>
      syncOpenApi(
        {version: '2025-q4', latest: false, mirrors: []},
        testContext(tempRoot, outputDir, versionsDir, freshSpec),
      ),
    /artifact\.bytes .* does not match/i,
  );

  assert.equal(await readFile(join(versionDir, 'torii.json'), 'utf8'), existingSpec);
});

test('syncOpenApi rejects a valid signature from an unapproved key before tracked writes', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-unapproved-signer-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const versionDir = join(versionsDir, '2025-q4');
  const existingSpec = releaseSpec('existing');
  const freshSpec = releaseSpec('fresh');
  const manifestSignature = signatureFor(freshSpec);
  const otherSignature = signatureFor(freshSpec, {privateKeyHex: '01'.repeat(32)});

  await mkdir(versionDir, {recursive: true});
  await writeFile(join(versionDir, 'torii.json'), existingSpec, 'utf8');
  await writeCanonicalManifest(outputDir, freshSpec, {signature: manifestSignature});
  await writeAllowedSigners(outputDir, [otherSignature.public_key_hex]);

  await assert.rejects(
    () =>
      syncOpenApi(
        {version: '2025-q4', latest: false, mirrors: []},
        testContext(tempRoot, outputDir, versionsDir, freshSpec),
      ),
    /signer is not present/i,
  );

  assert.equal(await readFile(join(versionDir, 'torii.json'), 'utf8'), existingSpec);
});

test('syncOpenApi rejects signed publication when no signer is provisioned', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-empty-allowlist-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const versionDir = join(versionsDir, '2025-q4');
  const existingSpec = releaseSpec('existing');
  const freshSpec = releaseSpec('fresh');

  await mkdir(versionDir, {recursive: true});
  await writeFile(join(versionDir, 'torii.json'), existingSpec, 'utf8');
  await writeCanonicalManifest(outputDir, freshSpec, {
    signature: signatureFor(freshSpec),
  });
  await writeAllowedSigners(outputDir, []);

  await assert.rejects(
    () =>
      syncOpenApi(
        {version: '2025-q4', latest: false, mirrors: []},
        testContext(tempRoot, outputDir, versionsDir, freshSpec),
      ),
    /signer is not present/i,
  );

  assert.equal(await readFile(join(versionDir, 'torii.json'), 'utf8'), existingSpec);
});

test('syncOpenApi accepts an operator-provided signer allowlist override', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-operator-allowlist-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const operatorDir = join(tempRoot, 'operator');
  const operatorAllowlist = join(operatorDir, 'allowed_signers.json');
  const freshSpec = releaseSpec('fresh');
  const manifestSignature = signatureFor(freshSpec);

  await writeCanonicalManifest(outputDir, freshSpec, {signature: manifestSignature});
  await writeAllowedSigners(outputDir, []);
  await mkdir(operatorDir, {recursive: true});
  await writeAllowedSigners(operatorDir, [manifestSignature.public_key_hex]);

  const options = parseArgs([
    '--version=2025-q4',
    '--latest',
    '--mirror=current',
    `--allowed-signers=${operatorAllowlist}`,
  ]);
  await syncOpenApi(
    options,
    testContext(tempRoot, outputDir, versionsDir, freshSpec),
  );

  assert.equal(
    await readFile(join(versionsDir, '2025-q4', 'torii.json'), 'utf8'),
    freshSpec,
  );
});

test('syncOpenApi refuses to overwrite or mirror an existing historical version', async () => {
  for (const mode of ['version', 'mirror']) {
    const tempRoot = await mkdtemp(join(tmpdir(), `sync-openapi-immutable-${mode}-`));
    const outputDir = join(tempRoot, 'static', 'openapi');
    const versionsDir = join(outputDir, 'versions');
    const historicalDir = join(versionsDir, '2025-q2');
    const historicalSpec = releaseSpec('historical');
    const freshSpec = releaseSpec('fresh');
    await mkdir(historicalDir, {recursive: true});
    await writeFile(join(historicalDir, 'torii.json'), historicalSpec, 'utf8');
    await writeCanonicalManifest(outputDir, freshSpec, {signature: null});

    const options = {
      version: mode === 'version' ? '2025-q2' : 'current',
      latest: true,
      mirrors: mode === 'mirror' ? ['2025-q2'] : [],
      requireSigned: false,
    };
    await assert.rejects(
      () =>
        syncOpenApi(
          options,
          testContext(tempRoot, outputDir, versionsDir, freshSpec),
        ),
      /already exists and is immutable/i,
    );
    assert.equal(await readFile(join(historicalDir, 'torii.json'), 'utf8'), historicalSpec);
  }
});

test('syncOpenApi validates every historical entry before any tracked write', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-preflight-history-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const currentDir = join(versionsDir, 'current');
  const invalidHistoricalSpec = join(versionsDir, '2025-q2', 'torii.json');
  const oldSpec = releaseSpec('old');
  const freshSpec = releaseSpec('fresh');

  await writeCanonicalManifest(outputDir, freshSpec, {signature: null});
  await writeFile(join(outputDir, 'torii.json'), oldSpec, 'utf8');
  await mkdir(currentDir, {recursive: true});
  await writeFile(join(currentDir, 'torii.json'), oldSpec, 'utf8');
  await writeManifestForSpec(join(currentDir, 'manifest.json'), oldSpec, 'torii.json');
  await mkdir(invalidHistoricalSpec, {recursive: true});
  const oldSpecBytes = Buffer.from(oldSpec, 'utf8');
  await writeFile(
    join(outputDir, 'versions.json'),
    JSON.stringify({
      versions: ['current'],
      generatedAt: '2025-01-01T00:00:00.000Z',
      entries: [{
        label: 'current',
        path: 'versions/current/torii.json',
        bytes: oldSpecBytes.length,
        sha256: createHash('sha256').update(oldSpecBytes).digest('hex'),
        blake3: computeOpenApiBlake3Hex(oldSpecBytes),
        updatedAt: '2025-01-01T00:00:00.000Z',
        signed: false,
        manifestPath: 'versions/current/manifest.json',
        signatureAlgorithm: null,
        signaturePublicKeyHex: null,
        signatureHex: null,
      }],
    }, null, 2),
    'utf8',
  );

  const trackedPaths = [
    join(outputDir, 'torii.json'),
    join(outputDir, 'manifest.json'),
    join(currentDir, 'torii.json'),
    join(currentDir, 'manifest.json'),
    join(outputDir, 'versions.json'),
  ];
  const before = await Promise.all(trackedPaths.map((path) => readFile(path)));

  await assert.rejects(
    () =>
      syncOpenApi(
        {version: 'current', latest: true, mirrors: [], requireSigned: false},
        testContext(tempRoot, outputDir, versionsDir, freshSpec),
      ),
    /EISDIR|illegal operation on a directory|must be a regular file/i,
  );

  const after = await Promise.all(trackedPaths.map((path) => readFile(path)));
  assert.deepEqual(after, before);
});

test('syncOpenApi rejects symlinks in the artifact tree before tracked writes', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-symlink-tree-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const historicalDir = join(versionsDir, '2025-q2');
  const externalSpec = join(tempRoot, 'external-torii.json');
  const freshSpec = releaseSpec('fresh');

  await writeCanonicalManifest(outputDir, freshSpec, {signature: null});
  await writeAllowedSigners(outputDir, []);
  await mkdir(historicalDir, {recursive: true});
  await writeFile(externalSpec, releaseSpec('external'), 'utf8');
  await symlink(externalSpec, join(historicalDir, 'torii.json'));
  const manifestBefore = await readFile(join(outputDir, 'manifest.json'));

  await assert.rejects(
    () =>
      syncOpenApi(
        {version: 'current', latest: true, mirrors: [], requireSigned: false},
        testContext(tempRoot, outputDir, versionsDir, freshSpec),
      ),
    /non-regular path/i,
  );

  assert.deepEqual(await readFile(join(outputDir, 'manifest.json')), manifestBefore);
  assert.equal(await pathExists(join(versionsDir, 'current')), false);
});

test('syncOpenApi index is independent of source mtimes and preserves historical metadata', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-deterministic-index-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const currentDir = join(versionsDir, 'current');
  const historicalDir = join(versionsDir, '2025-q2');
  const spec = releaseSpec('stable');
  const historicalSpec = releaseSpec('historical');
  const generatedAt = '2025-01-01T00:00:00.123Z';
  const historicalUpdatedAt = '2024-06-01T00:00:00.000Z';

  await writeCanonicalManifest(outputDir, spec, {signature: null});
  await writeAllowedSigners(outputDir, []);
  await writeFile(join(outputDir, 'torii.json'), spec, 'utf8');
  await mkdir(currentDir, {recursive: true});
  await writeFile(join(currentDir, 'torii.json'), spec, 'utf8');
  await writeManifestForSpec(join(currentDir, 'manifest.json'), spec, 'torii.json');
  await mkdir(historicalDir, {recursive: true});
  await writeFile(join(historicalDir, 'torii.json'), historicalSpec, 'utf8');

  const specSha = createHash('sha256').update(spec).digest('hex');
  const historicalSha = createHash('sha256').update(historicalSpec).digest('hex');
  const unsignedFields = {
    signed: false,
    signatureAlgorithm: null,
    signaturePublicKeyHex: null,
    signatureHex: null,
  };
  await writeFile(
    join(outputDir, 'versions.json'),
    JSON.stringify({
      versions: ['2025-q2', 'current'],
      generatedAt,
      entries: [
        {
          label: 'latest', path: 'torii.json', bytes: Buffer.byteLength(spec),
          sha256: specSha, blake3: computeOpenApiBlake3Hex(spec), updatedAt: generatedAt,
          ...unsignedFields, manifestPath: 'manifest.json',
        },
        {
          label: '2025-q2', path: 'versions/2025-q2/torii.json',
          bytes: Buffer.byteLength(historicalSpec), sha256: historicalSha,
          blake3: null, updatedAt: historicalUpdatedAt,
          ...unsignedFields, manifestPath: null,
        },
        {
          label: 'current', path: 'versions/current/torii.json', bytes: Buffer.byteLength(spec),
          sha256: specSha, blake3: computeOpenApiBlake3Hex(spec), updatedAt: generatedAt,
          ...unsignedFields, manifestPath: 'versions/current/manifest.json',
        },
      ],
    }, null, 2),
    'utf8',
  );

  const options = {version: 'current', latest: true, mirrors: [], requireSigned: false};
  const context = testContext(tempRoot, outputDir, versionsDir, spec);
  await syncOpenApi(options, context);
  const mutableOutputPaths = [
    join(outputDir, 'torii.json'),
    join(outputDir, 'manifest.json'),
    join(currentDir, 'torii.json'),
    join(currentDir, 'manifest.json'),
    join(outputDir, 'versions.json'),
  ];
  const firstOutputs = await Promise.all(
    mutableOutputPaths.map((path) => readFile(path)),
  );
  const firstIndex = await readFile(join(outputDir, 'versions.json'), 'utf8');

  await utimes(join(outputDir, 'torii.json'), new Date(0), new Date(1_000));
  await utimes(join(currentDir, 'torii.json'), new Date(0), new Date(2_000));
  await utimes(join(historicalDir, 'torii.json'), new Date(0), new Date(3_000));
  await syncOpenApi(options, context);
  const secondOutputs = await Promise.all(
    mutableOutputPaths.map((path) => readFile(path)),
  );
  const secondIndex = await readFile(join(outputDir, 'versions.json'), 'utf8');

  assert.deepEqual(
    secondOutputs,
    firstOutputs,
    'a second sync pass must leave every mutable tracked output byte-identical',
  );
  assert.equal(secondIndex, firstIndex);
  const parsed = JSON.parse(secondIndex);
  assert.equal(parsed.generatedAt, generatedAt);
  assert.equal(
    parsed.entries.find((entry) => entry.label === '2025-q2')?.updatedAt,
    historicalUpdatedAt,
  );
});

test('syncOpenApi replaces the mutable current manifest from the validated canonical manifest', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-stale-manifest-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');
  const versionDir = join(versionsDir, 'current');
  await mkdir(versionDir, {recursive: true});

  const staleSpec = releaseSpec('old');
  const freshSpec = releaseSpec('new');
  const staleSha = createHash('sha256').update(Buffer.from(staleSpec, 'utf8')).digest('hex');
  await writeFile(
    join(versionDir, 'manifest.json'),
    JSON.stringify(
      {
        version: 1,
        generated_unix_ms: 123,
        generator_commit: 'ab'.repeat(20),
        artifact: {
          path: 'versions/current/torii.json',
          bytes: Buffer.byteLength(staleSpec),
          sha256_hex: staleSha,
          blake3_hex: 'de'.repeat(32),
          signature: {
            algorithm: 'ed25519',
            public_key_hex: '00',
            signature_hex: '11',
          },
        },
      },
      null,
      2,
    ),
    'utf8',
  );
  await writeCanonicalManifest(outputDir, freshSpec, {signature: null});
  await writeAllowedSigners(outputDir, []);

  await syncOpenApi(
    {
      version: 'current',
      latest: true,
      mirrors: [],
      requireSigned: false,
    },
    {
      repoRoot: tempRoot,
      outputDir,
      versionsDir,
      async generateSpec(_, outputFile) {
        await mkdir(dirname(outputFile), {recursive: true});
        await writeFile(outputFile, freshSpec, 'utf8');
      },
    },
  );

  const versionsManifest = JSON.parse(await readFile(join(outputDir, 'versions.json'), 'utf8'));
  const entry = versionsManifest.entries.find((candidate) => candidate.label === 'current');
  assert.equal(entry?.signed, false);
  assert.equal(entry?.manifestPath, 'versions/current/manifest.json');
  assert.equal(entry?.signatureAlgorithm, null);
  assert.equal(entry?.signaturePublicKeyHex, null);
  assert.equal(entry?.signatureHex, null);
  assert.equal(entry?.blake3, computeOpenApiBlake3Hex(freshSpec));
  const copiedManifest = JSON.parse(
    await readFile(join(versionDir, 'manifest.json'), 'utf8'),
  );
  assert.equal(
    copiedManifest.artifact.sha256_hex,
    createHash('sha256').update(Buffer.from(freshSpec, 'utf8')).digest('hex'),
  );
});

async function writeCanonicalManifest(outputDir, spec, {signature}) {
  await mkdir(outputDir, {recursive: true});
  const specBytes = Buffer.from(spec, 'utf8');
  const manifest = {
    version: 2,
    generated_unix_ms: 123,
    generator_commit: 'ab'.repeat(20),
    generator_dirty: false,
    generator_source_sha256_hex: 'cd'.repeat(32),
    artifact: {
      path: 'torii.json',
      bytes: specBytes.length,
      sha256_hex: createHash('sha256').update(specBytes).digest('hex'),
      blake3_hex: computeOpenApiBlake3Hex(specBytes),
      signature,
    },
  };
  await writeFile(join(outputDir, 'manifest.json'), JSON.stringify(manifest, null, 2), 'utf8');
  if (signature) {
    await writeAllowedSigners(outputDir, [signature.public_key_hex]);
  }
}

async function writeManifestForSpec(target, spec, artifactPath) {
  const specBytes = Buffer.from(spec, 'utf8');
  await writeFile(
    target,
    JSON.stringify({
      version: 2,
      generated_unix_ms: 123,
      generator_commit: 'ab'.repeat(20),
      generator_dirty: false,
      generator_source_sha256_hex: 'cd'.repeat(32),
      artifact: {
        path: artifactPath,
        bytes: specBytes.length,
        sha256_hex: createHash('sha256').update(specBytes).digest('hex'),
        blake3_hex: computeOpenApiBlake3Hex(specBytes),
        signature: null,
      },
    }, null, 2),
    'utf8',
  );
}

function testContext(tempRoot, outputDir, versionsDir, generatedSpec) {
  return {
    repoRoot: tempRoot,
    outputDir,
    versionsDir,
    async generateSpec(_, outputFile) {
      await mkdir(dirname(outputFile), {recursive: true});
      await writeFile(outputFile, generatedSpec, 'utf8');
    },
  };
}

function signatureFor(spec, options) {
  const specBytes = Buffer.from(spec, 'utf8');
  const manifest = {
    version: 2,
    generated_unix_ms: 123,
    generator_commit: 'ab'.repeat(20),
    generator_dirty: false,
    generator_source_sha256_hex: 'cd'.repeat(32),
    artifact: {
      path: 'torii.json',
      bytes: specBytes.length,
      sha256_hex: createHash('sha256').update(specBytes).digest('hex'),
      blake3_hex: computeOpenApiBlake3Hex(specBytes),
      signature: null,
    },
  };
  attachOpenApiManifestSignature(manifest, specBytes, options);
  return manifest.artifact.signature;
}

async function writeAllowedSigners(outputDir, publicKeys) {
  const allowlist = {
    version: 1,
    allow: publicKeys.map((publicKey) => ({
      algorithm: 'ed25519',
      public_key_hex: publicKey,
    })),
  };
  await writeFile(
    join(outputDir, 'allowed_signers.json'),
    JSON.stringify(allowlist, null, 2),
    'utf8',
  );
}

async function pathExists(path) {
  try {
    await access(path);
    return true;
  } catch (error) {
    if (error && error.code === 'ENOENT') {
      return false;
    }
    throw error;
  }
}
