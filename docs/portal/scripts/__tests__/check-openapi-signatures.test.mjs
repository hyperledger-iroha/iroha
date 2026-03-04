import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp, mkdir, writeFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import {join, dirname} from 'node:path';
import {createHash} from 'node:crypto';

import {checkOpenApiSignatures} from '../check-openapi-signatures.mjs';
import {signPayload} from './helpers/openapi-signing.mjs';

test('checkOpenApiSignatures validates signed entries', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const latestSpec = Buffer.from('{"route":"/v1/status"}', 'utf8');
  const latestSha = sha256Hex(latestSpec);
  const latestSignature = signPayload(latestSpec);
  await writeAsset(join(staticDir, 'torii.json'), latestSpec);
  await writeJson(
    join(staticDir, 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: latestSpec,
      sha256: latestSha,
      blake3: 'feedface',
      signature: latestSignature,
    }),
  );

  const versionLabel = '2025-q3';
  const versionRelative = join('versions', versionLabel, 'torii.json').split('\\').join('/');
  const versionSpec = Buffer.from('{"route":"/v1/blocks"}', 'utf8');
  const versionSha = sha256Hex(versionSpec);
  const versionSignature = signPayload(versionSpec);
  await writeAllowedSigners(staticDir, [
    latestSignature.publicKeyHex,
    versionSignature.publicKeyHex,
  ]);
  await writeAsset(join(staticDir, 'versions', versionLabel, 'torii.json'), versionSpec);
  await writeJson(
    join(staticDir, 'versions', versionLabel, 'manifest.json'),
    buildManifest({
      path: versionRelative,
      payload: versionSpec,
      sha256: versionSha,
      blake3: 'c0ffeebabe',
      signature: versionSignature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: [versionLabel],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'latest',
        path: 'torii.json',
        payload: latestSpec,
        sha256: latestSha,
        blake3: 'feedface',
        manifestPath: 'manifest.json',
        signature: latestSignature,
      }),
      buildVersionEntry({
        label: versionLabel,
        path: versionRelative,
        payload: versionSpec,
        sha256: versionSha,
        blake3: 'c0ffeebabe',
        manifestPath: `versions/${versionLabel}/manifest.json`,
        signature: versionSignature,
      }),
    ],
  });

  const summary = await checkOpenApiSignatures({
    staticDir,
    versionsFile: join(staticDir, 'versions.json'),
  });
  assert.deepEqual(summary.checkedLabels.sort(), ['2025-q3', 'latest']);
  assert.deepEqual(summary.skippedLabels, []);
});

test('checkOpenApiSignatures fails when manifests are missing signatures', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-fail-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/gov"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature,
    }),
  );
  await writeJson(
    join(staticDir, 'versions', '2025-q4', 'manifest.json'),
    buildManifest({
      path: 'versions/2025-q4/torii.json',
      payload: spec,
      sha256: sha,
      signature: null,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['2025-q4'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'latest',
        path: 'torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'manifest.json',
        signature,
      }),
      buildVersionEntry({
        label: '2025-q4',
        path: 'versions/2025-q4/torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: `versions/2025-q4/manifest.json`,
        signature,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /2025-q4/i,
  );
});

test('checkOpenApiSignatures allows opting out specific labels', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-allow-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/metrics"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['2025-q2'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'latest',
        path: 'torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'manifest.json',
        signature,
      }),
      buildVersionEntry({
        label: '2025-q2',
        path: 'versions/2025-q2/torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: null,
        signed: false,
        signature: null,
      }),
    ],
  });

  const summary = await checkOpenApiSignatures({
    staticDir,
    versionsFile: join(staticDir, 'versions.json'),
    allowUnsigned: ['2025-q2'],
  });
  assert.deepEqual(summary.skippedLabels, ['2025-q2']);
  assert.deepEqual(summary.checkedLabels, ['latest']);
});

test('checkOpenApiSignatures rejects duplicate entry labels', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-duplicates-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/duplicate"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['2025-q8'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'latest',
        path: 'torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'manifest.json',
        signature,
      }),
      buildVersionEntry({
        label: '2025-q8',
        path: 'torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'manifest.json',
        signature,
      }),
      buildVersionEntry({
        label: '2025-q8',
        path: 'torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'manifest.json',
        signature,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /duplicate label/i,
  );
});

test('checkOpenApiSignatures fails when versions list lacks entries', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-missing-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/metrics"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['2026-q1'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'latest',
        path: 'torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'manifest.json',
        signature,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /versions list does not have matching entries/,
  );
});

test('checkOpenApiSignatures rejects byte mismatches', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-bytes-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/consensus"}', 'utf8');
  const specBytes = spec.length;
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['2025-q5'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: '2025-q5',
        path: 'torii.json',
        payload: spec,
        bytes: specBytes + 4,
        sha256: sha,
        manifestPath: 'manifest.json',
        signature,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /bytes mismatch/i,
  );
});

test('checkOpenApiSignatures rejects signature metadata mismatches', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-sigmeta-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/peers"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['2025-q6'],
    generatedAt: new Date().toISOString(),
    entries: [
      {
        ...buildVersionEntry({
          label: '2025-q6',
          path: 'torii.json',
          payload: spec,
          sha256: sha,
          manifestPath: 'manifest.json',
          signature,
        }),
        signaturePublicKeyHex: 'ff',
      },
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /signature public key mismatch/i,
  );
});

test('checkOpenApiSignatures enforces allowed signer list', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-allowlist-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/peers"}', 'utf8');
  const sha = sha256Hex(spec);
  const allowedSignature = signPayload(spec);
  await writeAllowedSigners(staticDir, [allowedSignature.publicKeyHex]);
  const forbiddenSignature = signPayload(spec, {
    privateKeyHex: '11'.repeat(32),
  });
  await writeAsset(join(staticDir, 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature: forbiddenSignature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['2025-q6'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: '2025-q6',
        path: 'torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'manifest.json',
        signature: forbiddenSignature,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /not allowed/i,
  );
});

test('checkOpenApiSignatures rejects invalid signatures', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-invalid-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/blocks"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  const invalidSignature = {
    publicKeyHex: signature.publicKeyHex,
    signatureHex: '00'.repeat(signature.signatureHex.length / 2),
  };
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature: invalidSignature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['2025-q7'],
    generatedAt: new Date().toISOString(),
    entries: [
      {
        ...buildVersionEntry({
          label: '2025-q7',
          path: 'torii.json',
          payload: spec,
          sha256: sha,
          manifestPath: 'manifest.json',
          signature: invalidSignature,
        }),
      },
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /signature verification failed/i,
  );
});

function sha256Hex(data) {
  const buffer = Buffer.isBuffer(data) ? data : Buffer.from(data, 'utf8');
  return createHash('sha256').update(buffer).digest('hex');
}

async function writeAsset(target, data) {
  await mkdir(dirname(target), {recursive: true});
  if (Buffer.isBuffer(data)) {
    await writeFile(target, data);
  } else {
    await writeFile(target, data, 'utf8');
  }
}

async function writeJson(target, data) {
  await writeAsset(target, `${JSON.stringify(data, null, 2)}\n`);
}

function buildManifest({path: specPath, payload, sha256, blake3 = null, signature = signPayload(payload)}) {
  const buffer = Buffer.isBuffer(payload) ? payload : Buffer.from(payload, 'utf8');
  const artifact = {
    path: specPath,
    bytes: buffer.length,
    sha256_hex: sha256,
    blake3_hex: blake3,
  };
  if (signature) {
    artifact.signature = {
      algorithm: 'ed25519',
      public_key_hex: signature.publicKeyHex,
      signature_hex: signature.signatureHex,
    };
  }
  return {
    version: 1,
    generated_unix_ms: Date.now(),
    generator_commit: 'abcdef',
    artifact,
  };
}

function buildVersionEntry({
  label,
  path,
  payload,
  bytes = Buffer.isBuffer(payload) ? payload.length : Buffer.byteLength(payload),
  sha256,
  blake3 = null,
  manifestPath,
  signed = true,
  signature = signed ? signPayload(payload) : null,
}) {
  return {
    label,
    path,
    bytes,
    sha256,
    blake3,
    updatedAt: new Date().toISOString(),
    signed,
    manifestPath,
    signatureAlgorithm: signature ? 'ed25519' : null,
    signaturePublicKeyHex: signature ? signature.publicKeyHex : null,
    signatureHex: signature ? signature.signatureHex : null,
  };
}

async function writeAllowedSigners(staticDir, publicKeys) {
  const allow = Array.from(new Set(publicKeys)).map((publicKeyHex) => ({
    algorithm: 'ed25519',
    public_key_hex: publicKeyHex,
  }));
  await writeJson(join(staticDir, 'allowed_signers.json'), {
    version: 1,
    allow,
  });
}
