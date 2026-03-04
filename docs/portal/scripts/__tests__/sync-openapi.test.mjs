import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, readFile, access, mkdir, writeFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {tmpdir} from 'node:os';
import {dirname, join} from 'node:path';

import {parseArgs, syncOpenApi} from '../sync-openapi.mjs';

test('parseArgs handles version, latest, and mirrors', () => {
  const options = parseArgs([
    '--version=2025-q4',
    '--latest',
    '--mirror=current',
    '--mirror=2025-q3',
    '--allow-unsigned',
    '--require-signed',
  ]);

  assert.equal(options.version, '2025-q4');
  assert.equal(options.latest, true);
  assert.deepEqual(options.mirrors, ['current', '2025-q3']);
  assert.equal(options.requireSigned, true);

  assert.throws(() => parseArgs(['--mirror=']), /mirror label must not be empty/);
});

test('parseArgs can disable signature enforcement explicitly', () => {
  const options = parseArgs(['--allow-unsigned']);
  assert.equal(options.requireSigned, false);
});

test('syncOpenApi mirrors specs into multiple version directories', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');

  const fakeSpec = JSON.stringify({generated: true}, null, 2);

  const manifestDir = join(outputDir);
  await mkdir(manifestDir, {recursive: true});
  const specBytes = Buffer.from(fakeSpec, 'utf8');
  const sha256 = createHash('sha256').update(specBytes).digest('hex');
  const canonicalManifest = {
    version: 1,
    generated_unix_ms: 123,
    generator_commit: 'abc',
    artifact: {
      path: 'torii.json',
      bytes: specBytes.length,
      sha256_hex: sha256,
      blake3_hex: 'deadbeef',
      signature: {
        algorithm: 'ed25519',
        public_key_hex: '00',
        signature_hex: '11',
      },
    },
  };
  await writeFile(join(manifestDir, 'manifest.json'), JSON.stringify(canonicalManifest, null, 2), 'utf8');

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
  assert.equal(latestEntry.blake3, 'deadbeef');
  assert.equal(latestEntry.signaturePublicKeyHex, '00');
  assert.equal(latestEntry.signatureHex, '11');

  await access(join(versionsDir, '2025-q4', 'torii.json'));
  await access(join(versionsDir, 'current', 'torii.json'));
  await access(join(versionsDir, '2025-q3', 'torii.json'));

  const versionManifest = JSON.parse(
    await readFile(join(versionsDir, '2025-q4', 'manifest.json'), 'utf8')
  );
  assert.equal(versionManifest.artifact.path, 'versions/2025-q4/torii.json');
  assert.equal(versionManifest.artifact.sha256_hex.toLowerCase(), sha256.toLowerCase());

  const mirrorManifest = JSON.parse(
    await readFile(join(versionsDir, 'current', 'manifest.json'), 'utf8')
  );
  assert.equal(mirrorManifest.artifact.path, 'versions/current/torii.json');
});

test('syncOpenApi rejects unsigned publications by default', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-unsigned-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');

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
            await writeFile(outputFile, JSON.stringify({generated: true}), 'utf8');
          },
        },
      ),
    /manifest .*not found|missing signature/i,
  );
});

test('syncOpenApi allows unsigned manifests only when opted-in', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'sync-openapi-unsigned-allowed-'));
  const outputDir = join(tempRoot, 'static', 'openapi');
  const versionsDir = join(outputDir, 'versions');

  await syncOpenApi(
    {
      version: '2025-q4',
      latest: false,
      mirrors: [],
      requireSigned: false,
    },
    {
      repoRoot: tempRoot,
      outputDir,
      versionsDir,
      async generateSpec(_, outputFile) {
        await mkdir(dirname(outputFile), {recursive: true});
        await writeFile(outputFile, JSON.stringify({generated: true}), 'utf8');
      },
    }
  );

  const manifestPath = join(versionsDir, '2025-q4', 'manifest.json');
  assert.equal(await pathExists(manifestPath), false);
  const versionsManifest = JSON.parse(
    await readFile(join(outputDir, 'versions.json'), 'utf8')
  );
  const entry = versionsManifest.entries.find((candidate) => candidate.label === '2025-q4');
  assert.equal(entry?.signed, false);
});

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
