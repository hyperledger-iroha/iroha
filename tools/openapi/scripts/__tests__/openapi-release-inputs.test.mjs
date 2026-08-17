import assert from 'node:assert/strict';
import {execFile} from 'node:child_process';
import {createHash} from 'node:crypto';
import {
  chmod,
  link,
  mkdir,
  mkdtemp,
  readFile,
  rename,
  rm,
  symlink,
  writeFile,
} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import test from 'node:test';
import {dirname, join, resolve} from 'node:path';
import {promisify} from 'node:util';
import {fileURLToPath} from 'node:url';

import {computeOpenApiBlake3Hex} from '../lib/openapi-manifest-v2.mjs';
import {readOpenApiStableFile} from '../lib/openapi-safe-file.mjs';
import {
  OPENAPI_CARGO_LOCK_PIN_PATH,
  encodeOpenApiCargoLockPin,
  isolateGitRepositoryEnvironment,
} from '../provision-openapi-cargo-lock.mjs';
import {
  OPENAPI_GENERATOR_INPUT_PATHS,
  OPENAPI_GENERATOR_INPUT_INVENTORY_HEADER,
  OPENAPI_TRACKED_GENERATOR_INPUT_MAX_BYTES,
  OPENAPI_TRACKED_GENERATOR_INPUT_MODE,
  OPENAPI_TRACKED_GENERATOR_INPUT_PATH,
  OPENAPI_TRACKED_GENERATOR_INPUT_PATHS,
  computeOpenApiGeneratorInputTreeSha256,
  computeOpenApiGeneratorInputTreeSha256Components,
  parseOpenApiGeneratorInputInventory,
  readOpenApiTrackedCargoLock,
  validateOpenApiCargoLockPinGitEvidence,
  validateOpenApiTrackedCargoLockAgainstPin,
  validatePinnedOpenApiProvenance,
  validateOpenApiTrackedCargoLockGitEvidence,
  verifyOpenApiReleaseInputs,
} from '../verify-openapi-release-inputs.mjs';

const testDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(testDir, '..', '..', '..', '..');
const execFileAsync = promisify(execFile);
const SOURCE_DIGEST = '33'.repeat(32);
const GENERATED_UNIX_MS = 1_700_000_000_000;

function cleanManifest(options = {}) {
  const commit = Object.hasOwn(options, 'commit')
    ? options.commit
    : '11'.repeat(20);
  const source = Object.hasOwn(options, 'source')
    ? options.source
    : SOURCE_DIGEST;
  return {
    version: 2,
    generated_unix_ms: Object.hasOwn(options, 'generatedUnixMs')
      ? options.generatedUnixMs
      : GENERATED_UNIX_MS,
    generator_commit: commit,
    generator_dirty: false,
    ...(source === undefined
      ? {}
      : {generator_source_sha256_hex: source}),
    artifact: {signature: null},
  };
}

function validationEvidence(overrides = {}) {
  const rootManifest = overrides.rootManifest ?? cleanManifest();
  return {
    rootManifest,
    currentManifest:
      overrides.currentManifest ?? structuredClone(rootManifest),
    resolvedGeneratorCommit:
      Object.hasOwn(overrides, 'resolvedGeneratorCommit')
        ? overrides.resolvedGeneratorCommit
        : rootManifest.generator_commit,
    generatorCommitIsAncestor:
      overrides.generatorCommitIsAncestor ?? true,
    expectedGeneratedUnixMs:
      overrides.expectedGeneratedUnixMs ?? GENERATED_UNIX_MS,
    pinnedSourceSha256Hex:
      overrides.pinnedSourceSha256Hex ?? SOURCE_DIGEST,
    headSourceSha256Hex:
      overrides.headSourceSha256Hex ?? SOURCE_DIGEST,
  };
}

function syntheticTree(paths = OPENAPI_TRACKED_GENERATOR_INPUT_PATHS) {
  const objectId = '44'.repeat(20);
  return Buffer.from(
    [...paths]
      .sort()
      .map((path) => `100644 blob ${objectId}\t${path}\0`)
      .join(''),
    'utf8',
  );
}

test('generator input inventory is exact and binds every release surface', async () => {
  const inventoryBytes = await readFile(
    join(repoRoot, 'release', 'openapi-generator-inputs-v1.txt'),
  );
  assert.deepEqual(
    parseOpenApiGeneratorInputInventory(inventoryBytes),
    OPENAPI_GENERATOR_INPUT_PATHS,
  );
  assert.equal(OPENAPI_TRACKED_GENERATOR_INPUT_PATH, 'Cargo.lock');
  assert.equal(OPENAPI_TRACKED_GENERATOR_INPUT_MODE, '100644');
  assert.deepEqual(
    OPENAPI_TRACKED_GENERATOR_INPUT_PATHS,
    OPENAPI_GENERATOR_INPUT_PATHS,
  );
  for (const required of [
    '.cargo/config.toml',
    'crates',
    'vendor',
    'tools',
    'scripts',
    'fixtures',
    'IrohaSwift',
    'javascript/iroha_js',
    'python/iroha_python',
    'python/iroha_torii_client',
    'kotlin',
    'java/iroha_android',
    'java/norito_java',
    'csharp',
    'artifacts/openapi/allowed_signers.json',
    OPENAPI_CARGO_LOCK_PIN_PATH,
    'release/version-map.toml',
    'specs/sdk/android/generated/codegen_hash_tree.json',
    'specs/sdk/android/generated/codegen_manifest_metadata.json',
  ]) {
    assert.ok(
      OPENAPI_GENERATOR_INPUT_PATHS.includes(required),
      `${required} must be release-bound`,
    );
  }
});

test('release verifier closes Git environment and final-state surfaces', async () => {
  const source = await readFile(
    join(testDir, '..', 'verify-openapi-release-inputs.mjs'),
    'utf8',
  );
  assert.match(source, /env: isolateGitRepositoryEnvironment\(\)/);
  assert.match(source, /beforeFinalStateCheck/);
  assert.match(source, /assertOpenApiFinalGitState/);
  assert.ok(source.match(/'status'/g).length >= 2);
  assert.match(source, /pinIndexEntryBytes/);
  await assert.rejects(
    () => verifyOpenApiReleaseInputs({beforeFinalStateCheck: true}),
    /beforeFinalStateCheck must be a function/,
  );
});

test('release verifier rejects real final-state Git mutations', async (context) => {
  const root = await makeReleaseVerifierRepository(context);
  const first = await verifyOpenApiReleaseInputs({repoRoot: root});
  assert.deepEqual(
    await verifyOpenApiReleaseInputs({repoRoot: root}),
    first,
  );
  const [head, parent, cargoLockBlob, pinBlob, otherBlob, cargoToml] =
    await Promise.all([
      fixtureGitText(root, ['rev-parse', 'HEAD']),
      fixtureGitText(root, ['rev-parse', 'HEAD^']),
      fixtureGitText(root, ['rev-parse', 'HEAD:Cargo.lock']),
      fixtureGitText(root, [
        'rev-parse',
        `HEAD:${OPENAPI_CARGO_LOCK_PIN_PATH}`,
      ]),
      fixtureGitText(root, ['rev-parse', 'HEAD:Cargo.toml']),
      readFile(join(root, 'Cargo.toml')),
    ]);
  const cases = [
    {
      name: 'HEAD',
      pattern: /HEAD changed during verification/i,
      mutate: () => fixtureGit(root, ['update-ref', 'HEAD', parent, head]),
      restore: () => fixtureGit(root, ['update-ref', 'HEAD', head, parent]),
    },
    {
      name: 'staged Cargo.lock',
      pattern: /Cargo\.lock.*same blob|same blob.*index.*generator/i,
      mutate: () =>
        fixtureGit(root, [
          'update-index',
          '--cacheinfo',
          `100644,${otherBlob},Cargo.lock`,
        ]),
      restore: () =>
        fixtureGit(root, [
          'update-index',
          '--cacheinfo',
          `100644,${cargoLockBlob},Cargo.lock`,
        ]),
    },
    {
      name: 'staged pin',
      pattern: /pin.*same blob|same blob.*index.*pinned/i,
      mutate: () =>
        fixtureGit(root, [
          'update-index',
          '--cacheinfo',
          `100644,${otherBlob},${OPENAPI_CARGO_LOCK_PIN_PATH}`,
        ]),
      restore: () =>
        fixtureGit(root, [
          'update-index',
          '--cacheinfo',
          `100644,${pinBlob},${OPENAPI_CARGO_LOCK_PIN_PATH}`,
        ]),
    },
    {
      name: 'dirty non-lock input',
      pattern: /checkout changed during verification/i,
      mutate: () => writeFile(join(root, 'Cargo.toml'), 'dirty\n'),
      restore: () => writeFile(join(root, 'Cargo.toml'), cargoToml),
    },
  ];
  for (const mutation of cases) {
    try {
      await assert.rejects(
        () =>
          verifyOpenApiReleaseInputs({
            repoRoot: root,
            beforeFinalStateCheck: mutation.mutate,
          }),
        mutation.pattern,
        mutation.name,
      );
    } finally {
      await mutation.restore();
    }
    assert.equal(
      await fixtureGitText(root, [
        'status',
        '--porcelain=v1',
        '--untracked-files=all',
      ]),
      '',
      mutation.name,
    );
  }
  assert.deepEqual(await verifyOpenApiReleaseInputs({repoRoot: root}), first);
});

test('generator input inventory rejects omitted and substituted paths', async () => {
  const inventoryBytes = await readFile(
    join(repoRoot, 'release', 'openapi-generator-inputs-v1.txt'),
  );
  const text = inventoryBytes.toString('utf8');
  for (const changed of [
    text.replace('Cargo.lock\n', ''),
    text.replace('Cargo.lock\n', 'Cargo.lock.alias\n'),
    text.replace('Cargo.lock\n', 'Cargo.lock\r\n'),
    text.replace(`${OPENAPI_CARGO_LOCK_PIN_PATH}\n`, ''),
    text.replace(
      `${OPENAPI_CARGO_LOCK_PIN_PATH}\n`,
      'release/openapi-cargo-lock-v1.alias\n',
    ),
    text.replace('python/iroha_torii_client\n', ''),
    text.replace(
      'python/iroha_torii_client\n',
      'python/iroha_torii_client.alias\n',
    ),
    text.slice(0, -1),
  ]) {
    assert.throws(
      () => parseOpenApiGeneratorInputInventory(Buffer.from(changed)),
      /exact V1 release-input contract|LF endings/i,
    );
  }
});

test('generator input-tree digest is deterministic and rejects missing roots', async () => {
  const inventoryBytes = await readFile(
    join(repoRoot, 'release', 'openapi-generator-inputs-v1.txt'),
  );
  const tree = syntheticTree();
  const trackedInputBytes = Buffer.from('lock fixture\n');
  const first = computeOpenApiGeneratorInputTreeSha256({
    inventoryBytes,
    treeBytes: tree,
    trackedInputBytes,
  });
  const second = computeOpenApiGeneratorInputTreeSha256({
    inventoryBytes,
    treeBytes: Buffer.from(tree),
    trackedInputBytes: Buffer.from(trackedInputBytes),
  });
  assert.equal(first, second);
  assert.match(first, /^[0-9a-f]{64}$/);

  const substituted = Buffer.from(tree);
  substituted[12] ^= 1;
  assert.notEqual(
    computeOpenApiGeneratorInputTreeSha256({
      inventoryBytes,
      treeBytes: substituted,
      trackedInputBytes,
    }),
    first,
  );
  assert.notEqual(
    computeOpenApiGeneratorInputTreeSha256({
      inventoryBytes,
      treeBytes: tree,
      trackedInputBytes: Buffer.from('substituted lock fixture\n'),
    }),
    first,
  );
  assert.throws(
    () =>
      computeOpenApiGeneratorInputTreeSha256({
        inventoryBytes,
        treeBytes: syntheticTree(
          OPENAPI_TRACKED_GENERATOR_INPUT_PATHS.filter(
            (path) => path !== 'Cargo.toml',
          ),
        ),
        trackedInputBytes,
      }),
    /Cargo\.toml.*missing/i,
  );
  assert.throws(
    () =>
      computeOpenApiGeneratorInputTreeSha256({
        inventoryBytes,
        treeBytes: syntheticTree([
          ...OPENAPI_TRACKED_GENERATOR_INPUT_PATHS,
          OPENAPI_TRACKED_GENERATOR_INPUT_PATH,
        ]),
        trackedInputBytes,
      }),
    /duplicated.*Cargo\.lock/i,
  );
});

test('generator input-tree V3 digest matches the fixed cross-language vector', async () => {
  const inventoryBytes = await readFile(
    join(repoRoot, 'release', 'openapi-generator-inputs-v1.txt'),
  );
  assert.equal(
    computeOpenApiGeneratorInputTreeSha256Components({
      inventoryBytes,
      treeBytes: Buffer.from('tree-fixture-v1\0'),
      trackedInputBytes: Buffer.from('lock-fixture-v1\n'),
    }),
    '5034e34e44a804546302178b2f13468096663997b2074ef0a2fe6916f654e080',
  );
});

test('generator input-tree V3 digest rejects missing, empty, and oversized lock bytes', async () => {
  const inventoryBytes = await readFile(
    join(repoRoot, 'release', 'openapi-generator-inputs-v1.txt'),
  );
  const treeBytes = Buffer.from('tree-fixture-v1\0');
  for (const trackedInputBytes of [undefined, Buffer.alloc(0)]) {
    assert.throws(
      () =>
        computeOpenApiGeneratorInputTreeSha256Components({
          inventoryBytes,
          treeBytes,
          trackedInputBytes,
        }),
      /trackedInputBytes must be bytes|Cargo\.lock must not be empty/i,
    );
  }
  assert.throws(
    () =>
      computeOpenApiGeneratorInputTreeSha256Components({
        inventoryBytes,
        treeBytes,
        trackedInputBytes: Buffer.alloc(
          OPENAPI_TRACKED_GENERATOR_INPUT_MAX_BYTES + 1,
        ),
      }),
    /exceeds the 16777216-byte limit/i,
  );
});

test('tracked Cargo.lock reader accepts one stable nonempty regular file', async () => {
  const root = await mkdtemp(join(tmpdir(), 'openapi-cargo-lock-'));
  const lockPath = join(root, OPENAPI_TRACKED_GENERATOR_INPUT_PATH);
  await writeFile(lockPath, 'lock fixture\n');
  assert.deepEqual(
    await readOpenApiTrackedCargoLock(lockPath),
    Buffer.from('lock fixture\n'),
  );
});

test('tracked Cargo.lock reader rejects missing, empty, and oversized files', async () => {
  const root = await mkdtemp(join(tmpdir(), 'openapi-cargo-lock-bounds-'));
  const lockPath = join(root, OPENAPI_TRACKED_GENERATOR_INPUT_PATH);
  await assert.rejects(
    () => readOpenApiTrackedCargoLock(lockPath),
    /failed to inspect.*Cargo\.lock/i,
  );

  await writeFile(lockPath, Buffer.alloc(0));
  await assert.rejects(
    () => readOpenApiTrackedCargoLock(lockPath),
    /Cargo\.lock.*must not be empty/i,
  );

  await writeFile(
    lockPath,
    Buffer.alloc(OPENAPI_TRACKED_GENERATOR_INPUT_MAX_BYTES + 1),
  );
  await assert.rejects(
    () => readOpenApiTrackedCargoLock(lockPath),
    /exceeds the 16777216-byte limit/i,
  );
});

test(
  'tracked Cargo.lock reader rejects executable, symbolic, and multiply linked files',
  {skip: process.platform === 'win32'},
  async () => {
    const executableRoot = await mkdtemp(
      join(tmpdir(), 'openapi-cargo-lock-executable-'),
    );
    const executablePath = join(
      executableRoot,
      OPENAPI_TRACKED_GENERATOR_INPUT_PATH,
    );
    await writeFile(executablePath, 'lock fixture\n');
    await chmod(executablePath, 0o744);
    await assert.rejects(
      () => readOpenApiTrackedCargoLock(executablePath),
      /Cargo\.lock.*must not be executable/i,
    );

    const symlinkRoot = await mkdtemp(
      join(tmpdir(), 'openapi-cargo-lock-symlink-'),
    );
    const symlinkTarget = join(symlinkRoot, 'target.lock');
    const symlinkPath = join(
      symlinkRoot,
      OPENAPI_TRACKED_GENERATOR_INPUT_PATH,
    );
    await writeFile(symlinkTarget, 'lock fixture\n');
    await symlink(symlinkTarget, symlinkPath);
    await assert.rejects(
      () => readOpenApiTrackedCargoLock(symlinkPath),
      /Cargo\.lock.*must not be a symlink/i,
    );

    const hardlinkRoot = await mkdtemp(
      join(tmpdir(), 'openapi-cargo-lock-hardlink-'),
    );
    const hardlinkTarget = join(hardlinkRoot, 'target.lock');
    const hardlinkPath = join(
      hardlinkRoot,
      OPENAPI_TRACKED_GENERATOR_INPUT_PATH,
    );
    await writeFile(hardlinkTarget, 'lock fixture\n');
    await link(hardlinkTarget, hardlinkPath);
    await assert.rejects(
      () => readOpenApiTrackedCargoLock(hardlinkPath),
      /Cargo\.lock.*exactly one hard link/i,
    );
  },
);

test(
  'tracked Cargo.lock reader rejects replacement and mutation across its stable read',
  {skip: process.platform === 'win32'},
  async () => {
    for (const operation of ['replace', 'mutate']) {
      const root = await mkdtemp(
        join(tmpdir(), `openapi-cargo-lock-${operation}-`),
      );
      const lockPath = join(
        root,
        OPENAPI_TRACKED_GENERATOR_INPUT_PATH,
      );
      await writeFile(lockPath, 'original lock fixture\n');
      const replacementPath = join(root, 'replacement.lock');
      await writeFile(replacementPath, 'replacement lock fixture\n');

      await assert.rejects(
        () =>
          readOpenApiTrackedCargoLock(lockPath, {
            stableFileReader: async (path, options) => {
              if (operation === 'replace') {
                await rename(replacementPath, lockPath);
              } else {
                await writeFile(lockPath, 'mutated lock fixture\n');
              }
              return readOpenApiStableFile(path, options);
            },
          }),
        /Cargo\.lock.*changed while it was read/i,
        operation,
      );
    }
  },
);

test('tracked Cargo.lock Git evidence requires one identical 100644 blob', () => {
  const trackedBlobOid = '55'.repeat(20);
  const valid = {
    indexEntryBytes: Buffer.from(
      `100644 ${trackedBlobOid} 0\t${OPENAPI_TRACKED_GENERATOR_INPUT_PATH}\0`,
    ),
    pinnedTreeEntryBytes: Buffer.from(
      `100644 blob ${trackedBlobOid}\t${OPENAPI_TRACKED_GENERATOR_INPUT_PATH}\0`,
    ),
    headTreeEntryBytes: Buffer.from(
      `100644 blob ${trackedBlobOid}\t${OPENAPI_TRACKED_GENERATOR_INPUT_PATH}\0`,
    ),
  };
  assert.equal(
    validateOpenApiTrackedCargoLockGitEvidence(valid),
    trackedBlobOid,
  );

  for (const [field, value, pattern] of [
    ['indexEntryBytes', Buffer.alloc(0), /Git index/i],
    [
      'indexEntryBytes',
      Buffer.from(
        `100755 ${trackedBlobOid} 0\t${OPENAPI_TRACKED_GENERATOR_INPUT_PATH}\0`,
      ),
      /Git index.*stage-zero 100644 blob/i,
    ],
    [
      'pinnedTreeEntryBytes',
      Buffer.from(
        `100644 blob ${'66'.repeat(20)}\t${OPENAPI_TRACKED_GENERATOR_INPUT_PATH}\0`,
      ),
      /same blob.*index.*generator/i,
    ],
    [
      'headTreeEntryBytes',
      Buffer.from(
        `100644 blob ${'66'.repeat(20)}\t${OPENAPI_TRACKED_GENERATOR_INPUT_PATH}\0`,
      ),
      /same blob.*index.*generator.*HEAD/i,
    ],
  ]) {
    assert.throws(
      () =>
        validateOpenApiTrackedCargoLockGitEvidence({
          ...valid,
          [field]: value,
        }),
      pattern,
      field,
    );
  }
});

test('source-bound Cargo.lock pin requires one identical 100644 Git blob', () => {
  const pinnedBlobOid = '55'.repeat(20);
  const valid = {
    indexEntryBytes: Buffer.from(
      `100644 ${pinnedBlobOid} 0\t${OPENAPI_CARGO_LOCK_PIN_PATH}\0`,
    ),
    pinnedTreeEntryBytes: Buffer.from(
      `100644 blob ${pinnedBlobOid}\t${OPENAPI_CARGO_LOCK_PIN_PATH}\0`,
    ),
    headTreeEntryBytes: Buffer.from(
      `100644 blob ${pinnedBlobOid}\t${OPENAPI_CARGO_LOCK_PIN_PATH}\0`,
    ),
  };
  assert.equal(
    validateOpenApiCargoLockPinGitEvidence(valid),
    pinnedBlobOid,
  );

  for (const [field, value, pattern] of [
    ['indexEntryBytes', Buffer.alloc(0), /Git index/i],
    [
      'indexEntryBytes',
      Buffer.from(
        `100755 ${pinnedBlobOid} 0\t${OPENAPI_CARGO_LOCK_PIN_PATH}\0`,
      ),
      /Git index.*stage-zero 100644 blob/i,
    ],
    ['pinnedTreeEntryBytes', Buffer.alloc(0), /pinned Git tree/i],
    [
      'pinnedTreeEntryBytes',
      Buffer.from(
        `100755 blob ${pinnedBlobOid}\t${OPENAPI_CARGO_LOCK_PIN_PATH}\0`,
      ),
      /pinned Git tree.*canonical 100644 blob/i,
    ],
    [
      'pinnedTreeEntryBytes',
      Buffer.from(
        `120000 blob ${pinnedBlobOid}\t${OPENAPI_CARGO_LOCK_PIN_PATH}\0`,
      ),
      /pinned Git tree.*canonical 100644 blob/i,
    ],
    [
      'headTreeEntryBytes',
      Buffer.from(
        `100644 blob ${'66'.repeat(20)}\t${OPENAPI_CARGO_LOCK_PIN_PATH}\0`,
      ),
      /same blob.*index.*pinned.*HEAD/i,
    ],
  ]) {
    assert.throws(
      () =>
        validateOpenApiCargoLockPinGitEvidence({
          ...valid,
          [field]: value,
        }),
      pattern,
      field,
    );
  }
});

test('source-bound Cargo.lock pin validates exact root lock bytes', async () => {
  const [pinBytes, trackedInputBytes] = await Promise.all([
    readFile(join(repoRoot, OPENAPI_CARGO_LOCK_PIN_PATH)),
    readFile(join(repoRoot, OPENAPI_TRACKED_GENERATOR_INPUT_PATH)),
  ]);
  const pin = validateOpenApiTrackedCargoLockAgainstPin({
    trackedInputBytes,
    pinBytes,
  });
  assert.equal(pin.bytes, trackedInputBytes.length);

  assert.throws(
    () =>
      validateOpenApiTrackedCargoLockAgainstPin({
        trackedInputBytes: trackedInputBytes.subarray(
          0,
          trackedInputBytes.length - 1,
        ),
        pinBytes,
      }),
    /expected exactly/i,
  );
  const substituted = Buffer.from(trackedInputBytes);
  substituted[0] ^= 0xff;
  assert.throws(
    () =>
      validateOpenApiTrackedCargoLockAgainstPin({
        trackedInputBytes: substituted,
        pinBytes,
      }),
    /does not match pinned/i,
  );
  assert.throws(
    () =>
      validateOpenApiTrackedCargoLockAgainstPin({
        trackedInputBytes,
        pinBytes: Buffer.from(
          pinBytes.toString('utf8').replace(
            /sha256_hex=[0-9a-f]{64}/,
            `sha256_hex=${'0'.repeat(64)}`,
          ),
        ),
      }),
    /pin SHA-256 must be nonzero/i,
  );
});

test('ancestor-pinned provenance accepts a real matching source pin', () => {
  assert.deepEqual(
    validatePinnedOpenApiProvenance(validationEvidence()),
    {
      generatorCommit: '11'.repeat(20),
      generatorSourceSha256Hex: SOURCE_DIGEST,
    },
  );
});

test('ancestor-pinned provenance binds the exact source commit timestamp', () => {
  assert.throws(
    () =>
      validatePinnedOpenApiProvenance(
        validationEvidence({
          rootManifest: cleanManifest({
            generatedUnixMs: GENERATED_UNIX_MS + 1,
          }),
          currentManifest: cleanManifest({
            generatedUnixMs: GENERATED_UNIX_MS + 1,
          }),
        }),
      ),
    /generated_unix_ms must equal pinned generator commit time/i,
  );
  for (const expectedGeneratedUnixMs of [
    undefined,
    null,
    0,
    -1,
    Number.MAX_SAFE_INTEGER + 1,
  ]) {
    assert.throws(
      () =>
        validatePinnedOpenApiProvenance({
          ...validationEvidence(),
          expectedGeneratedUnixMs,
        }),
      /positive JavaScript-safe integer/i,
    );
  }
});

test('ancestor-pinned provenance rejects malformed or nonexistent commits', () => {
  for (const [name, commit, pattern] of [
    ['missing', undefined, /generator_commit/i],
    ['empty', '', /exactly 40 lowercase hexadecimal/i],
    ['zero', '00'.repeat(20), /nonzero Git commit/i],
    ['short', '11'.repeat(19), /exactly 40 lowercase hexadecimal/i],
    ['uppercase', 'AA'.repeat(20), /exactly 40 lowercase hexadecimal/i],
    ['nonhex', 'gg'.repeat(20), /exactly 40 lowercase hexadecimal/i],
  ]) {
    assert.throws(
      () =>
        validatePinnedOpenApiProvenance(
          validationEvidence({
            rootManifest: cleanManifest({commit}),
            currentManifest: cleanManifest({commit}),
            resolvedGeneratorCommit: commit,
          }),
        ),
      pattern,
      name,
    );
  }

  assert.throws(
    () =>
      validatePinnedOpenApiProvenance(
        validationEvidence({resolvedGeneratorCommit: null}),
      ),
    /does not resolve to a full Git commit object/i,
  );
  assert.throws(
    () =>
      validatePinnedOpenApiProvenance(
        validationEvidence({
          resolvedGeneratorCommit: '22'.repeat(20),
        }),
      ),
    /resolves to substituted commit/i,
  );
  assert.throws(
    () =>
      validatePinnedOpenApiProvenance(
        validationEvidence({generatorCommitIsAncestor: false}),
      ),
    /not an ancestor/i,
  );
});

test('ancestor-pinned provenance rejects omitted, malformed, and substituted source digests', () => {
  for (const [name, source, pattern] of [
    ['missing', undefined, /generator_source_sha256_hex/i],
    ['empty', '', /64 lowercase hexadecimal/i],
    ['zero', '00'.repeat(32), /must be nonzero/i],
    ['short', '11'.repeat(31), /64 lowercase hexadecimal/i],
    ['uppercase', 'AA'.repeat(32), /64 lowercase hexadecimal/i],
    ['nonhex', 'gg'.repeat(32), /64 lowercase hexadecimal/i],
  ]) {
    assert.throws(
      () =>
        validatePinnedOpenApiProvenance(
          validationEvidence({
            rootManifest: cleanManifest({source}),
            currentManifest: cleanManifest({source}),
          }),
        ),
      pattern,
      name,
    );
  }

  assert.throws(
    () =>
      validatePinnedOpenApiProvenance(
        validationEvidence({
          pinnedSourceSha256Hex: '55'.repeat(32),
        }),
      ),
    /does not match pinned generator inputs/i,
  );
  assert.throws(
    () =>
      validatePinnedOpenApiProvenance(
        validationEvidence({
          headSourceSha256Hex: '66'.repeat(32),
        }),
      ),
    /does not match current generator inputs/i,
  );
  assert.throws(
    () =>
      validatePinnedOpenApiProvenance(
        validationEvidence({
          currentManifest: cleanManifest({source: '77'.repeat(32)}),
        }),
      ),
    /must bind identical generator provenance/i,
  );
});

async function makeReleaseVerifierRepository(context) {
  const root = await mkdtemp(join(tmpdir(), 'openapi-release-final-state-'));
  context.after(() => rm(root, {recursive: true, force: true}));
  await fixtureGit(root, ['init', '--quiet']);
  await fixtureGit(root, [
    'config',
    'user.email',
    'openapi-test@example.invalid',
  ]);
  await fixtureGit(root, ['config', 'user.name', 'OpenAPI Test']);

  const inventoryBytes = Buffer.from(
    `${OPENAPI_GENERATOR_INPUT_INVENTORY_HEADER}\n${OPENAPI_GENERATOR_INPUT_PATHS.join('\n')}\n`,
  );
  const lockBytes = Buffer.from('# fixture Cargo.lock\nversion = 4\n');
  const leafPaths = OPENAPI_GENERATOR_INPUT_PATHS.filter(
    (candidate) =>
      !OPENAPI_GENERATOR_INPUT_PATHS.some((other) =>
        other.startsWith(`${candidate}/`),
      ),
  );
  for (const relativePath of leafPaths) {
    const target = join(root, relativePath);
    await mkdir(dirname(target), {recursive: true});
    let bytes = Buffer.from(`fixture ${relativePath}\n`);
    if (relativePath === 'Cargo.lock') {
      bytes = lockBytes;
    } else if (relativePath === OPENAPI_CARGO_LOCK_PIN_PATH) {
      bytes = encodeOpenApiCargoLockPin(lockBytes);
    } else if (relativePath === 'release/openapi-generator-inputs-v1.txt') {
      bytes = inventoryBytes;
    }
    await writeFile(target, bytes);
  }
  await fixtureGit(root, ['add', '-A']);
  await fixtureGit(root, ['commit', '--quiet', '-m', 'generator inputs']);
  const generatorCommit = await fixtureGitText(root, ['rev-parse', 'HEAD']);
  const generatedUnixMs = Number(
    await fixtureGitText(root, ['show', '-s', '--format=%ct', 'HEAD']),
  ) * 1_000;
  const treeBytes = await fixtureGit(root, [
    'ls-tree',
    '-r',
    '-z',
    '--full-tree',
    generatorCommit,
    '--',
    ...OPENAPI_GENERATOR_INPUT_PATHS,
  ]);
  const generatorSourceSha256Hex =
    computeOpenApiGeneratorInputTreeSha256({
      inventoryBytes,
      treeBytes,
      trackedInputBytes: lockBytes,
      inventoryPaths: OPENAPI_GENERATOR_INPUT_PATHS,
    });
  const specBytes = Buffer.from(
    `${JSON.stringify({
      openapi: '3.1.0',
      info: {title: 'fixture', version: '1'},
      paths: {'/health': {get: {responses: {'200': {description: 'ok'}}}}},
      components: {schemas: {Health: {type: 'object'}}},
    })}\n`,
  );
  const sha256 = createHash('sha256').update(specBytes).digest('hex');
  const blake3 = computeOpenApiBlake3Hex(specBytes);
  const manifestBytes = Buffer.from(
    `${JSON.stringify({
      version: 2,
      generated_unix_ms: generatedUnixMs,
      generator_commit: generatorCommit,
      generator_dirty: false,
      generator_source_sha256_hex: generatorSourceSha256Hex,
      artifact: {
        path: 'torii.json',
        bytes: specBytes.length,
        sha256_hex: sha256,
        blake3_hex: blake3,
        signature: null,
      },
    }, null, 2)}\n`,
  );
  const updatedAt = new Date(generatedUnixMs).toISOString();
  const entry = (label, artifactPath, manifestPath) => ({
    label,
    path: artifactPath,
    bytes: specBytes.length,
    sha256,
    blake3,
    updatedAt,
    signed: false,
    manifestPath,
    signatureAlgorithm: null,
    signaturePublicKeyHex: null,
    signatureHex: null,
  });
  const versionsBytes = Buffer.from(
    `${JSON.stringify({
      versions: ['current'],
      generatedAt: updatedAt,
      entries: [
        entry('latest', 'torii.json', 'manifest.json'),
        entry(
          'current',
          'versions/current/torii.json',
          'versions/current/manifest.json',
        ),
      ],
    }, null, 2)}\n`,
  );
  const openapiRoot = join(root, 'artifacts', 'openapi');
  const currentRoot = join(openapiRoot, 'versions', 'current');
  await mkdir(currentRoot, {recursive: true});
  await Promise.all([
    writeFile(join(openapiRoot, 'torii.json'), specBytes),
    writeFile(join(openapiRoot, 'manifest.json'), manifestBytes),
    writeFile(join(currentRoot, 'torii.json'), specBytes),
    writeFile(join(currentRoot, 'manifest.json'), manifestBytes),
    writeFile(join(openapiRoot, 'versions.json'), versionsBytes),
  ]);
  await fixtureGit(root, ['add', '-A']);
  await fixtureGit(root, ['commit', '--quiet', '-m', 'release artifacts']);
  return root;
}

async function fixtureGit(root, args) {
  const {stdout} = await execFileAsync('git', ['-C', root, ...args], {
    encoding: 'buffer',
    env: isolateGitRepositoryEnvironment(),
    maxBuffer: 128 * 1024 * 1024,
  });
  return Buffer.from(stdout);
}

async function fixtureGitText(root, args) {
  return (await fixtureGit(root, args)).toString('utf8').trim();
}
