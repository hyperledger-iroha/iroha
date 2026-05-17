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
      blake3: 'fe'.repeat(32),
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
      blake3: 'c0'.repeat(32),
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
        blake3: 'fe'.repeat(32),
        manifestPath: 'manifest.json',
        signature: latestSignature,
      }),
      buildVersionEntry({
        label: versionLabel,
        path: versionRelative,
        payload: versionSpec,
        sha256: versionSha,
        blake3: 'c0'.repeat(32),
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
  await writeAsset(join(staticDir, 'versions', '2025-q2', 'torii.json'), spec);
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

test('checkOpenApiSignatures still validates unsigned label metadata', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-allow-metadata-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/current"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'versions', 'current', 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'versions', 'current', 'manifest.json'),
    buildManifest({
      path: 'versions/current/torii.json',
      payload: spec,
      sha256: 'deadbeef',
      signature: null,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
        path: 'versions/current/torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'versions/current/manifest.json',
        signed: false,
        signature: null,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
        allowUnsigned: ['current'],
      }),
    /manifest sha256 mismatch/i,
  );
});

test('checkOpenApiSignatures does not let unsigned allowlist bypass signed entries', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-allow-signed-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/current"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'versions', 'current', 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'versions', 'current', 'manifest.json'),
    buildManifest({
      path: 'versions/current/torii.json',
      payload: spec,
      sha256: sha,
      signature: null,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
        path: 'versions/current/torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'versions/current/manifest.json',
        signed: true,
        signature: null,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
        allowUnsigned: ['current'],
      }),
    /manifest missing artifact.signature/i,
  );
});

test('checkOpenApiSignatures rejects signed metadata smuggled into unsigned labels', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-smuggled-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/current"}', 'utf8');
  const sha = sha256Hex(spec);
  const allowedSignature = signPayload(spec);
  const forbiddenSignature = signPayload(spec, {
    privateKeyHex: '22'.repeat(32),
  });
  await writeAllowedSigners(staticDir, [allowedSignature.publicKeyHex]);
  await writeAsset(join(staticDir, 'versions', 'current', 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'versions', 'current', 'manifest.json'),
    buildManifest({
      path: 'versions/current/torii.json',
      payload: spec,
      sha256: sha,
      signature: forbiddenSignature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
        path: 'versions/current/torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'versions/current/manifest.json',
        signed: false,
        signature: null,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
        allowUnsigned: ['current'],
      }),
    /manifest signer not allowed/i,
  );
});

test('checkOpenApiSignatures rejects malformed allowed signer entries', async () => {
  for (const [name, signer, pattern] of [
    [
      'bad-key',
      {algorithm: 'ed25519', public_key_hex: 'not-hex'},
      /invalid public_key_hex/i,
    ],
    [
      'short-key',
      {algorithm: 'ed25519', public_key_hex: '00'},
      /invalid public_key_hex/i,
    ],
    [
      'bad-algorithm',
      {algorithm: 'ed448', public_key_hex: '00'.repeat(32)},
      /unsupported algorithm/i,
    ],
  ]) {
    const tempRoot = await mkdtemp(join(tmpdir(), `openapi-signatures-allowlist-${name}-`));
    const staticDir = join(tempRoot, 'static', 'openapi');
    await mkdir(staticDir, {recursive: true});
    await writeJson(join(staticDir, 'allowed_signers.json'), {
      version: 1,
      allow: [signer],
    });

    await assert.rejects(
      () =>
        checkOpenApiSignatures({
          staticDir,
          versionsFile: join(staticDir, 'versions.json'),
        }),
      pattern,
    );
  }
});

test('checkOpenApiSignatures rejects unsupported allowed signer versions', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-allowlist-version-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});
  await writeJson(join(staticDir, 'allowed_signers.json'), {
    version: 2,
    allow: [
      {
        algorithm: 'ed25519',
        public_key_hex: '00'.repeat(32),
      },
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /unsupported version 2/i,
  );
});

test('checkOpenApiSignatures rejects duplicate allowed signers', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-allowlist-duplicate-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});
  const publicKey = '00'.repeat(32);
  await writeJson(join(staticDir, 'allowed_signers.json'), {
    version: 1,
    allow: [
      {
        algorithm: 'ed25519',
        public_key_hex: publicKey,
      },
      {
        algorithm: 'ED25519',
        public_key_hex: publicKey.toUpperCase(),
      },
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /duplicates allowed signer/i,
  );
});

test('checkOpenApiSignatures rejects malformed versions lists', async () => {
  for (const [name, versions, pattern] of [
    ['missing', undefined, /missing a versions array/i],
    ['not-array', 'current', /versions must be an array/i],
    ['empty-string', [''], /versions\[0\] must be a non-empty string/i],
    ['non-string', [42], /versions\[0\] must be a non-empty string/i],
    ['whitespace', [' current'], /versions\[0\] must not have surrounding whitespace/i],
    ['duplicate', ['current', 'current'], /versions\[1\] duplicates current/i],
  ]) {
    const tempRoot = await mkdtemp(join(tmpdir(), `openapi-signatures-versions-${name}-`));
    const staticDir = join(tempRoot, 'static', 'openapi');
    await mkdir(staticDir, {recursive: true});
    await writeAllowedSigners(staticDir, ['00'.repeat(32)]);
    const manifest = {
      generatedAt: new Date().toISOString(),
      entries: [],
    };
    if (versions !== undefined) {
      manifest.versions = versions;
    }
    await writeJson(join(staticDir, 'versions.json'), manifest);

    await assert.rejects(
      () =>
        checkOpenApiSignatures({
          staticDir,
          versionsFile: join(staticDir, 'versions.json'),
        }),
      pattern,
    );
  }
});

test('checkOpenApiSignatures rejects non-object version entries', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-non-object-entry-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});
  await writeAllowedSigners(staticDir, ['00'.repeat(32)]);
  await writeJson(join(staticDir, 'versions.json'), {
    versions: [],
    generatedAt: new Date().toISOString(),
    entries: [null],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /entry is not an object/i,
  );
});

test('checkOpenApiSignatures rejects missing and whitespace version entry labels', async () => {
  for (const [name, label, pattern] of [
    ['missing', undefined, /versions entry missing label/i],
    ['blank', '', /versions entry missing label/i],
    ['whitespace', ' current', /versions entry label must not have surrounding whitespace/i],
  ]) {
    const tempRoot = await mkdtemp(join(tmpdir(), `openapi-signatures-entry-label-${name}-`));
    const staticDir = join(tempRoot, 'static', 'openapi');
    await mkdir(staticDir, {recursive: true});

    const spec = Buffer.from('{"route":"/v1/entry-label"}', 'utf8');
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
    const entry = buildVersionEntry({
      label: 'current',
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      manifestPath: 'manifest.json',
      signature,
    });
    if (label === undefined) {
      delete entry.label;
    } else {
      entry.label = label;
    }
    await writeJson(join(staticDir, 'versions.json'), {
      versions: [],
      generatedAt: new Date().toISOString(),
      entries: [entry],
    });

    await assert.rejects(
      () =>
        checkOpenApiSignatures({
          staticDir,
          versionsFile: join(staticDir, 'versions.json'),
        }),
      pattern,
    );
  }
});

test('checkOpenApiSignatures rejects signed flag type confusion', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-signed-type-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/signed-type"}', 'utf8');
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
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      {
        ...buildVersionEntry({
          label: 'current',
          path: 'torii.json',
          payload: spec,
          sha256: sha,
          manifestPath: 'manifest.json',
          signature,
        }),
        signed: 'true',
      },
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /versions entry signed must be boolean/i,
  );
});

test('checkOpenApiSignatures rejects versions signature metadata on unsigned labels', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-entry-smuggled-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/current"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'versions', 'current', 'torii.json'), spec);

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      {
        ...buildVersionEntry({
          label: 'current',
          path: 'versions/current/torii.json',
          payload: spec,
          sha256: sha,
          manifestPath: null,
          signed: false,
          signature: null,
        }),
        signatureAlgorithm: 'ed25519',
        signaturePublicKeyHex: signature.publicKeyHex,
        signatureHex: signature.signatureHex,
      },
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
        allowUnsigned: ['current'],
      }),
    /unsigned versions entry must not include signature metadata/i,
  );
});

test('checkOpenApiSignatures rejects malformed signed version entry metadata', async () => {
  for (const [name, patch, pattern] of [
    [
      'bad-algorithm',
      {signatureAlgorithm: 'ed448'},
      /versions entry unsupported signatureAlgorithm/i,
    ],
    [
      'bad-public-key',
      {signaturePublicKeyHex: 'zz'.repeat(32)},
      /versions entry invalid signaturePublicKeyHex/i,
    ],
    [
      'bad-signature',
      {signatureHex: 'zz'.repeat(64)},
      /versions entry invalid signatureHex/i,
    ],
  ]) {
    const tempRoot = await mkdtemp(join(tmpdir(), `openapi-signatures-entry-${name}-`));
    const staticDir = join(tempRoot, 'static', 'openapi');
    await mkdir(staticDir, {recursive: true});

    const spec = Buffer.from('{"route":"/v1/entry-metadata"}', 'utf8');
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
      versions: ['current'],
      generatedAt: new Date().toISOString(),
      entries: [
        {
          ...buildVersionEntry({
            label: 'current',
            path: 'torii.json',
            payload: spec,
            sha256: sha,
            manifestPath: 'manifest.json',
            signature,
          }),
          ...patch,
        },
      ],
    });

    await assert.rejects(
      () =>
        checkOpenApiSignatures({
          staticDir,
          versionsFile: join(staticDir, 'versions.json'),
        }),
      pattern,
    );
  }
});

test('checkOpenApiSignatures rejects invalid versions byte counts', async () => {
  for (const [name, bytes] of [
    ['negative', -1],
    ['fractional', 1.5],
  ]) {
    const tempRoot = await mkdtemp(join(tmpdir(), `openapi-signatures-entry-bytes-${name}-`));
    const staticDir = join(tempRoot, 'static', 'openapi');
    await mkdir(staticDir, {recursive: true});

    const spec = Buffer.from('{"route":"/v1/entry-bytes"}', 'utf8');
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
      versions: ['current'],
      generatedAt: new Date().toISOString(),
      entries: [
        buildVersionEntry({
          label: 'current',
          path: 'torii.json',
          payload: spec,
          bytes,
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
      /versions entry missing bytes/i,
    );
  }
});

test('checkOpenApiSignatures rejects malformed blake3 metadata', async () => {
  for (const [name, blake3, pattern] of [
    ['non-string', 42, /versions entry invalid blake3/i],
    ['bad-hex', 'zz'.repeat(32), /versions entry invalid blake3/i],
    ['short', '00', /versions entry invalid blake3/i],
  ]) {
    const tempRoot = await mkdtemp(join(tmpdir(), `openapi-signatures-blake3-${name}-`));
    const staticDir = join(tempRoot, 'static', 'openapi');
    await mkdir(staticDir, {recursive: true});

    const spec = Buffer.from('{"route":"/v1/blake3"}', 'utf8');
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
        blake3: '00'.repeat(32),
        signature,
      }),
    );

    await writeJson(join(staticDir, 'versions.json'), {
      versions: ['current'],
      generatedAt: new Date().toISOString(),
      entries: [
        buildVersionEntry({
          label: 'current',
          path: 'torii.json',
          payload: spec,
          sha256: sha,
          blake3,
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
      pattern,
    );
  }
});

test('checkOpenApiSignatures rejects malformed manifest signature metadata', async () => {
  for (const [name, patch, pattern] of [
    [
      'bad-algorithm',
      {algorithm: 'ed448'},
      /unsupported manifest signature algorithm/i,
    ],
    [
      'bad-public-key',
      {public_key_hex: 'zz'.repeat(32)},
      /signature invalid public key/i,
    ],
    [
      'bad-signature',
      {signature_hex: 'zz'.repeat(64)},
      /signature invalid value/i,
    ],
  ]) {
    const tempRoot = await mkdtemp(join(tmpdir(), `openapi-signatures-manifest-sig-${name}-`));
    const staticDir = join(tempRoot, 'static', 'openapi');
    await mkdir(staticDir, {recursive: true});

    const spec = Buffer.from('{"route":"/v1/manifest-signature"}', 'utf8');
    const sha = sha256Hex(spec);
    const signature = signPayload(spec);
    await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
    await writeAsset(join(staticDir, 'torii.json'), spec);
    const manifest = buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature,
    });
    Object.assign(manifest.artifact.signature, patch);
    await writeJson(join(staticDir, 'manifest.json'), manifest);

    await writeJson(join(staticDir, 'versions.json'), {
      versions: ['current'],
      generatedAt: new Date().toISOString(),
      entries: [
        buildVersionEntry({
          label: 'current',
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
      pattern,
    );
  }
});

test('checkOpenApiSignatures rejects non-object manifest signature metadata', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-manifest-sig-type-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/manifest-signature-type"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);
  const manifest = buildManifest({
    path: 'torii.json',
    payload: spec,
    sha256: sha,
    signature,
  });
  manifest.artifact.signature = 'ed25519';
  await writeJson(join(staticDir, 'manifest.json'), manifest);

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
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
    /manifest signature must be an object/i,
  );
});

test('checkOpenApiSignatures rejects allowed signed manifests on unsigned labels', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-allowed-smuggled-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/current"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'versions', 'current', 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'versions', 'current', 'manifest.json'),
    buildManifest({
      path: 'versions/current/torii.json',
      payload: spec,
      sha256: sha,
      signature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
        path: 'versions/current/torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'versions/current/manifest.json',
        signed: false,
        signature: null,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
        allowUnsigned: ['current'],
      }),
    /unsigned manifest must not include artifact.signature/i,
  );
});

test('checkOpenApiSignatures rejects incomplete manifest generator metadata', async () => {
  for (const [name, patch, pattern] of [
    ['version', {version: 2}, /manifest unsupported version/i],
    ['generated', {generated_unix_ms: null}, /manifest missing generated_unix_ms/i],
    ['negative-generated', {generated_unix_ms: -1}, /manifest missing generated_unix_ms/i],
    ['fractional-generated', {generated_unix_ms: 1.5}, /manifest missing generated_unix_ms/i],
    ['commit', {generator_commit: ''}, /manifest missing generator_commit/i],
  ]) {
    const tempRoot = await mkdtemp(join(tmpdir(), `openapi-signatures-manifest-meta-${name}-`));
    const staticDir = join(tempRoot, 'static', 'openapi');
    await mkdir(staticDir, {recursive: true});

    const spec = Buffer.from('{"route":"/v1/manifest-metadata"}', 'utf8');
    const sha = sha256Hex(spec);
    const signature = signPayload(spec);
    await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
    await writeAsset(join(staticDir, 'torii.json'), spec);
    await writeJson(
      join(staticDir, 'manifest.json'),
      {
        ...buildManifest({
          path: 'torii.json',
          payload: spec,
          sha256: sha,
          signature,
        }),
        ...patch,
      },
    );

    await writeJson(join(staticDir, 'versions.json'), {
      versions: ['current'],
      generatedAt: new Date().toISOString(),
      entries: [
        buildVersionEntry({
          label: 'current',
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
      pattern,
    );
  }
});

test('checkOpenApiSignatures rejects unsafe manifest artifact paths', async () => {
  for (const [name, artifactPath] of [
    ['backslash', 'versions\\current\\torii.json'],
    ['drive', 'C:/outside/torii.json'],
    ['traversal', '../outside/torii.json'],
    ['empty', ''],
  ]) {
    const tempRoot = await mkdtemp(join(tmpdir(), `openapi-signatures-artifact-${name}-`));
    const staticDir = join(tempRoot, 'static', 'openapi');
    await mkdir(staticDir, {recursive: true});

    const spec = Buffer.from('{"route":"/v1/artifact-path"}', 'utf8');
    const sha = sha256Hex(spec);
    const signature = signPayload(spec);
    await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
    await writeAsset(join(staticDir, 'versions', 'current', 'torii.json'), spec);
    const manifest = buildManifest({
      path: 'versions/current/torii.json',
      payload: spec,
      sha256: sha,
      signature,
    });
    manifest.artifact.path = artifactPath;
    await writeJson(join(staticDir, 'versions', 'current', 'manifest.json'), manifest);

    await writeJson(join(staticDir, 'versions.json'), {
      versions: ['current'],
      generatedAt: new Date().toISOString(),
      entries: [
        buildVersionEntry({
          label: 'current',
          path: 'versions/current/torii.json',
          payload: spec,
          sha256: sha,
          manifestPath: 'versions/current/manifest.json',
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
      /manifest missing or invalid artifact.path/i,
    );
  }
});

test('checkOpenApiSignatures rejects malformed manifest artifact metadata', async () => {
  for (const [name, patch, pattern] of [
    ['non-object', {artifact: 'torii.json'}, /manifest missing artifact metadata/i],
    ['negative-bytes', {artifact: {bytes: -1}}, /manifest missing artifact.bytes/i],
    ['fractional-bytes', {artifact: {bytes: 1.5}}, /manifest missing artifact.bytes/i],
  ]) {
    const tempRoot = await mkdtemp(join(tmpdir(), `openapi-signatures-artifact-meta-${name}-`));
    const staticDir = join(tempRoot, 'static', 'openapi');
    await mkdir(staticDir, {recursive: true});

    const spec = Buffer.from('{"route":"/v1/artifact-metadata"}', 'utf8');
    const sha = sha256Hex(spec);
    const signature = signPayload(spec);
    await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
    await writeAsset(join(staticDir, 'torii.json'), spec);
    const manifest = buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature,
    });
    if (typeof patch.artifact === 'object' && patch.artifact !== null) {
      Object.assign(manifest.artifact, patch.artifact);
    } else {
      manifest.artifact = patch.artifact;
    }
    await writeJson(join(staticDir, 'manifest.json'), manifest);

    await writeJson(join(staticDir, 'versions.json'), {
      versions: ['current'],
      generatedAt: new Date().toISOString(),
      entries: [
        buildVersionEntry({
          label: 'current',
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
      pattern,
    );
  }
});

test('checkOpenApiSignatures rejects spec paths escaping static openapi directory', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-spec-escape-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/escape"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(tempRoot, 'static', 'outside', 'torii.json'), spec);

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
        path: '../outside/torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: null,
        signed: false,
        signature: null,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
        allowUnsigned: ['current'],
      }),
    /missing spec path/i,
  );
});

test('checkOpenApiSignatures rejects manifest paths escaping static openapi directory', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-manifest-escape-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/escape-manifest"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);
  await writeJson(
    join(tempRoot, 'static', 'outside', 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
        path: 'torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: '../outside/manifest.json',
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
    /missing manifest path/i,
  );
});

test('checkOpenApiSignatures rejects absolute spec paths', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-absolute-spec-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/absolute"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  const absoluteSpecPath = join(tempRoot, 'outside', 'torii.json');
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(absoluteSpecPath, spec);

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
        path: absoluteSpecPath,
        payload: spec,
        sha256: sha,
        manifestPath: null,
        signed: false,
        signature: null,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
        allowUnsigned: ['current'],
      }),
    /missing spec path/i,
  );
});

test('checkOpenApiSignatures rejects windows drive manifest paths', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-windows-manifest-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/windows"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
        path: 'torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'C:\\outside\\manifest.json',
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
    /missing manifest path/i,
  );
});

test('checkOpenApiSignatures rejects drive-relative spec paths', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-drive-relative-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/drive-relative"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
        path: 'C:outside\\torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: null,
        signed: false,
        signature: null,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
        allowUnsigned: ['current'],
      }),
    /missing spec path/i,
  );
});

test('checkOpenApiSignatures rejects backslash traversal spec paths', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-backslash-traversal-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/backslash-traversal"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['current'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: 'current',
        path: 'versions\\current\\..\\..\\outside\\torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: null,
        signed: false,
        signature: null,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
        allowUnsigned: ['current'],
      }),
    /missing spec path/i,
  );
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

test('checkOpenApiSignatures rejects malformed signature hex before verification', async () => {
  const tempRoot = await mkdtemp(join(tmpdir(), 'openapi-signatures-malformed-hex-'));
  const staticDir = join(tempRoot, 'static', 'openapi');
  await mkdir(staticDir, {recursive: true});

  const spec = Buffer.from('{"route":"/v1/blocks"}', 'utf8');
  const sha = sha256Hex(spec);
  const signature = signPayload(spec);
  const malformedSignature = {
    publicKeyHex: signature.publicKeyHex,
    signatureHex: 'zz'.repeat(signature.signatureHex.length / 2),
  };
  await writeAllowedSigners(staticDir, [signature.publicKeyHex]);
  await writeAsset(join(staticDir, 'torii.json'), spec);
  await writeJson(
    join(staticDir, 'manifest.json'),
    buildManifest({
      path: 'torii.json',
      payload: spec,
      sha256: sha,
      signature: malformedSignature,
    }),
  );

  await writeJson(join(staticDir, 'versions.json'), {
    versions: ['2025-q7'],
    generatedAt: new Date().toISOString(),
    entries: [
      buildVersionEntry({
        label: '2025-q7',
        path: 'torii.json',
        payload: spec,
        sha256: sha,
        manifestPath: 'manifest.json',
        signature: malformedSignature,
      }),
    ],
  });

  await assert.rejects(
    () =>
      checkOpenApiSignatures({
        staticDir,
        versionsFile: join(staticDir, 'versions.json'),
      }),
    /signature is not valid hex/i,
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
