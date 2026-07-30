import assert from 'node:assert/strict';
import {
  chmod,
  link,
  mkdtemp,
  readFile,
  rename,
  symlink,
  writeFile,
} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import test from 'node:test';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

import {readOpenApiStableFile} from '../lib/openapi-safe-file.mjs';
import {OPENAPI_CARGO_LOCK_PIN_PATH} from '../provision-openapi-cargo-lock.mjs';
import {
  OPENAPI_GENERATOR_INPUT_PATHS,
  OPENAPI_IGNORED_GENERATOR_INPUT_MAX_BYTES,
  OPENAPI_IGNORED_GENERATOR_INPUT_MODE,
  OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
  OPENAPI_TRACKED_GENERATOR_INPUT_PATHS,
  computeOpenApiGeneratorInputTreeSha256,
  computeOpenApiGeneratorInputTreeSha256Components,
  parseOpenApiGeneratorInputInventory,
  readOpenApiIgnoredCargoLock,
  validateOpenApiCargoLockPinGitEvidence,
  validateOpenApiIgnoredCargoLockAgainstPin,
  validatePinnedOpenApiProvenance,
  validateOpenApiIgnoredCargoLockGitEvidence,
} from '../verify-openapi-release-inputs.mjs';

const testDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(testDir, '..', '..', '..', '..');
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
  assert.equal(OPENAPI_IGNORED_GENERATOR_INPUT_PATH, 'Cargo.lock');
  assert.equal(OPENAPI_IGNORED_GENERATOR_INPUT_MODE, '100644');
  assert.deepEqual(
    OPENAPI_GENERATOR_INPUT_PATHS.filter(
      (path) => !OPENAPI_TRACKED_GENERATOR_INPUT_PATHS.includes(path),
    ),
    [OPENAPI_IGNORED_GENERATOR_INPUT_PATH],
  );
  for (const required of [
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
  const ignoredInputBytes = Buffer.from('lock fixture\n');
  const first = computeOpenApiGeneratorInputTreeSha256({
    inventoryBytes,
    treeBytes: tree,
    ignoredInputBytes,
  });
  const second = computeOpenApiGeneratorInputTreeSha256({
    inventoryBytes,
    treeBytes: Buffer.from(tree),
    ignoredInputBytes: Buffer.from(ignoredInputBytes),
  });
  assert.equal(first, second);
  assert.match(first, /^[0-9a-f]{64}$/);

  const substituted = Buffer.from(tree);
  substituted[12] ^= 1;
  assert.notEqual(
    computeOpenApiGeneratorInputTreeSha256({
      inventoryBytes,
      treeBytes: substituted,
      ignoredInputBytes,
    }),
    first,
  );
  assert.notEqual(
    computeOpenApiGeneratorInputTreeSha256({
      inventoryBytes,
      treeBytes: tree,
      ignoredInputBytes: Buffer.from('substituted lock fixture\n'),
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
        ignoredInputBytes,
      }),
    /Cargo\.toml.*missing/i,
  );
  assert.throws(
    () =>
      computeOpenApiGeneratorInputTreeSha256({
        inventoryBytes,
        treeBytes: syntheticTree([
          ...OPENAPI_TRACKED_GENERATOR_INPUT_PATHS,
          OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
        ]),
        ignoredInputBytes,
      }),
    /Cargo\.lock must not resolve from a Git tree/i,
  );
});

test('generator input-tree V2 digest matches the fixed cross-language vector', async () => {
  const inventoryBytes = await readFile(
    join(repoRoot, 'release', 'openapi-generator-inputs-v1.txt'),
  );
  assert.equal(
    computeOpenApiGeneratorInputTreeSha256Components({
      inventoryBytes,
      treeBytes: Buffer.from('tree-fixture-v1\0'),
      ignoredInputBytes: Buffer.from('lock-fixture-v1\n'),
    }),
    'd46a10a45a97a731c8c20330f435f6ca2ab143dfc38b9ca7949c2ab5010fb9f1',
  );
});

test('generator input-tree V2 digest rejects missing, empty, and oversized lock bytes', async () => {
  const inventoryBytes = await readFile(
    join(repoRoot, 'release', 'openapi-generator-inputs-v1.txt'),
  );
  const treeBytes = Buffer.from('tree-fixture-v1\0');
  for (const ignoredInputBytes of [undefined, Buffer.alloc(0)]) {
    assert.throws(
      () =>
        computeOpenApiGeneratorInputTreeSha256Components({
          inventoryBytes,
          treeBytes,
          ignoredInputBytes,
        }),
      /ignoredInputBytes must be bytes|Cargo\.lock must not be empty/i,
    );
  }
  assert.throws(
    () =>
      computeOpenApiGeneratorInputTreeSha256Components({
        inventoryBytes,
        treeBytes,
        ignoredInputBytes: Buffer.alloc(
          OPENAPI_IGNORED_GENERATOR_INPUT_MAX_BYTES + 1,
        ),
      }),
    /exceeds the 16777216-byte limit/i,
  );
});

test('ignored Cargo.lock reader accepts one stable nonempty regular file', async () => {
  const root = await mkdtemp(join(tmpdir(), 'openapi-cargo-lock-'));
  const lockPath = join(root, OPENAPI_IGNORED_GENERATOR_INPUT_PATH);
  await writeFile(lockPath, 'lock fixture\n');
  assert.deepEqual(
    await readOpenApiIgnoredCargoLock(lockPath),
    Buffer.from('lock fixture\n'),
  );
});

test('ignored Cargo.lock reader rejects missing, empty, and oversized files', async () => {
  const root = await mkdtemp(join(tmpdir(), 'openapi-cargo-lock-bounds-'));
  const lockPath = join(root, OPENAPI_IGNORED_GENERATOR_INPUT_PATH);
  await assert.rejects(
    () => readOpenApiIgnoredCargoLock(lockPath),
    /failed to inspect.*Cargo\.lock/i,
  );

  await writeFile(lockPath, Buffer.alloc(0));
  await assert.rejects(
    () => readOpenApiIgnoredCargoLock(lockPath),
    /Cargo\.lock.*must not be empty/i,
  );

  await writeFile(
    lockPath,
    Buffer.alloc(OPENAPI_IGNORED_GENERATOR_INPUT_MAX_BYTES + 1),
  );
  await assert.rejects(
    () => readOpenApiIgnoredCargoLock(lockPath),
    /exceeds the 16777216-byte limit/i,
  );
});

test(
  'ignored Cargo.lock reader rejects executable, symbolic, and multiply linked files',
  {skip: process.platform === 'win32'},
  async () => {
    const executableRoot = await mkdtemp(
      join(tmpdir(), 'openapi-cargo-lock-executable-'),
    );
    const executablePath = join(
      executableRoot,
      OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
    );
    await writeFile(executablePath, 'lock fixture\n');
    await chmod(executablePath, 0o744);
    await assert.rejects(
      () => readOpenApiIgnoredCargoLock(executablePath),
      /Cargo\.lock.*must not be executable/i,
    );

    const symlinkRoot = await mkdtemp(
      join(tmpdir(), 'openapi-cargo-lock-symlink-'),
    );
    const symlinkTarget = join(symlinkRoot, 'target.lock');
    const symlinkPath = join(
      symlinkRoot,
      OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
    );
    await writeFile(symlinkTarget, 'lock fixture\n');
    await symlink(symlinkTarget, symlinkPath);
    await assert.rejects(
      () => readOpenApiIgnoredCargoLock(symlinkPath),
      /Cargo\.lock.*must not be a symlink/i,
    );

    const hardlinkRoot = await mkdtemp(
      join(tmpdir(), 'openapi-cargo-lock-hardlink-'),
    );
    const hardlinkTarget = join(hardlinkRoot, 'target.lock');
    const hardlinkPath = join(
      hardlinkRoot,
      OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
    );
    await writeFile(hardlinkTarget, 'lock fixture\n');
    await link(hardlinkTarget, hardlinkPath);
    await assert.rejects(
      () => readOpenApiIgnoredCargoLock(hardlinkPath),
      /Cargo\.lock.*exactly one hard link/i,
    );
  },
);

test(
  'ignored Cargo.lock reader rejects replacement and mutation across its stable read',
  {skip: process.platform === 'win32'},
  async () => {
    for (const operation of ['replace', 'mutate']) {
      const root = await mkdtemp(
        join(tmpdir(), `openapi-cargo-lock-${operation}-`),
      );
      const lockPath = join(
        root,
        OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
      );
      await writeFile(lockPath, 'original lock fixture\n');
      const replacementPath = join(root, 'replacement.lock');
      await writeFile(replacementPath, 'replacement lock fixture\n');

      await assert.rejects(
        () =>
          readOpenApiIgnoredCargoLock(lockPath, {
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

test('ignored Cargo.lock Git evidence requires one ignored untracked root input', () => {
  const valid = {
    ignoredPathBytes: Buffer.from('Cargo.lock\n'),
    indexEntryBytes: Buffer.alloc(0),
    pinnedTreeEntryBytes: Buffer.alloc(0),
    headTreeEntryBytes: Buffer.alloc(0),
  };
  assert.doesNotThrow(() =>
    validateOpenApiIgnoredCargoLockGitEvidence(valid));

  for (const [field, value, pattern] of [
    ['ignoredPathBytes', Buffer.alloc(0), /exact ignored root path/i],
    [
      'ignoredPathBytes',
      Buffer.from('nested/Cargo.lock\n'),
      /exact ignored root path/i,
    ],
    ['indexEntryBytes', Buffer.from('tracked\0'), /Git index/i],
    [
      'pinnedTreeEntryBytes',
      Buffer.from('tracked\0'),
      /pinned Git tree/i,
    ],
    ['headTreeEntryBytes', Buffer.from('tracked\0'), /HEAD Git tree/i],
  ]) {
    assert.throws(
      () =>
        validateOpenApiIgnoredCargoLockGitEvidence({
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
    ['pinnedTreeEntryBytes', Buffer.alloc(0), /pinned Git tree/i],
    [
      'pinnedTreeEntryBytes',
      Buffer.from(
        `100755 blob ${pinnedBlobOid}\t${OPENAPI_CARGO_LOCK_PIN_PATH}\0`,
      ),
      /canonical 100644 blob.*pinned Git tree/i,
    ],
    [
      'pinnedTreeEntryBytes',
      Buffer.from(
        `120000 blob ${pinnedBlobOid}\t${OPENAPI_CARGO_LOCK_PIN_PATH}\0`,
      ),
      /canonical 100644 blob.*pinned Git tree/i,
    ],
    [
      'headTreeEntryBytes',
      Buffer.from(
        `100644 blob ${'66'.repeat(20)}\t${OPENAPI_CARGO_LOCK_PIN_PATH}\0`,
      ),
      /same blob.*pinned and HEAD/i,
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
  const [pinBytes, ignoredInputBytes] = await Promise.all([
    readFile(join(repoRoot, OPENAPI_CARGO_LOCK_PIN_PATH)),
    readFile(join(repoRoot, OPENAPI_IGNORED_GENERATOR_INPUT_PATH)),
  ]);
  const pin = validateOpenApiIgnoredCargoLockAgainstPin({
    ignoredInputBytes,
    pinBytes,
  });
  assert.equal(pin.bytes, ignoredInputBytes.length);

  assert.throws(
    () =>
      validateOpenApiIgnoredCargoLockAgainstPin({
        ignoredInputBytes: ignoredInputBytes.subarray(
          0,
          ignoredInputBytes.length - 1,
        ),
        pinBytes,
      }),
    /expected exactly/i,
  );
  const substituted = Buffer.from(ignoredInputBytes);
  substituted[0] ^= 0xff;
  assert.throws(
    () =>
      validateOpenApiIgnoredCargoLockAgainstPin({
        ignoredInputBytes: substituted,
        pinBytes,
      }),
    /does not match pinned/i,
  );
  assert.throws(
    () =>
      validateOpenApiIgnoredCargoLockAgainstPin({
        ignoredInputBytes,
        pinBytes: Buffer.from(
          pinBytes.toString('utf8').replace(
            /sha256_hex=[0-9a-f]{64}/,
            `sha256_hex=${'0'.repeat(64)}`,
          ),
        ),
      }),
    /pin does not match the exact V1 size and SHA-256/i,
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
