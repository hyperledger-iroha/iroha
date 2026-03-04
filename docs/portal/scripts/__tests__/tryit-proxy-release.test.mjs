import test from 'node:test';
import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdtemp, mkdir, readFile, rm, writeFile} from 'node:fs/promises';
import path from 'node:path';
import {tmpdir} from 'node:os';

import {buildTryItRelease} from '../tryit-proxy-release.mjs';
import {signPayload} from './helpers/openapi-signing.mjs';

test('buildTryItRelease copies sources and writes manifest', async () => {
  const tmpRoot = await mkdtemp(path.join(tmpdir(), 'tryit-release-'));
  const outDir = path.join(tmpRoot, 'bundle');
  const manifestDir = path.join(tmpRoot, 'openapi');
  try {
    await mkdir(outDir, {recursive: true});
    await mkdir(manifestDir, {recursive: true});
    const specBuffer = Buffer.from('{"openapi":"3.1.0","info":{"title":"test"}}', 'utf8');
    const digest = createHash('sha256').update(specBuffer).digest('hex');
    const signature = signPayload(specBuffer);
    const specPath = path.join(manifestDir, 'torii.json');
    await writeFile(specPath, specBuffer);
    const manifestPath = path.join(manifestDir, 'manifest.json');
    const manifestPayload = {
      version: 1,
      artifact: {
        path: 'torii.json',
        bytes: specBuffer.length,
        sha256_hex: digest,
        signature: {
          algorithm: 'ed25519',
          public_key_hex: signature.publicKeyHex,
          signature_hex: signature.signatureHex,
        },
      },
    };
    await writeFile(manifestPath, JSON.stringify(manifestPayload, null, 2), 'utf8');

    const {release, releasePath, checksumPath} = await buildTryItRelease({
      outDir,
      label: 'test-release',
      targetHint: 'https://torii.test',
      openapiManifestPath: manifestPath,
    });

    assert.equal(release.label, 'test-release');
    assert.equal(release.entrypoint, 'tryit-proxy.mjs');
    assert.ok(
      Array.isArray(release.files) && release.files.length > 0,
      'release should record copied files',
    );
    const proxyEntry = release.files.find(
      (entry) => entry.name === 'tryit-proxy.mjs',
    );
    assert.ok(proxyEntry, 'release should include tryit-proxy.mjs');
    assert.match(proxyEntry.sha256_hex, /^[0-9a-f]{64}$/);
    assert.ok(
      release.openapi?.sha256_hex,
      'release should include openapi manifest digest',
    );

    const manifest = JSON.parse(await readFile(releasePath, 'utf8'));
    assert.equal(manifest.label, 'test-release');
    const checksums = await readFile(checksumPath, 'utf8');
    assert.match(checksums, /release\.json/);
    assert.match(checksums, /tryit-proxy\.mjs/);
  } finally {
    await rm(tmpRoot, {recursive: true, force: true});
  }
});
