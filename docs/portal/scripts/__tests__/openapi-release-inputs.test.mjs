import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
import test from 'node:test';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

import {
  OPENAPI_GENERATOR_INPUT_PATHS,
  computeOpenApiGeneratorInputTreeSha256,
  parseOpenApiGeneratorInputInventory,
  validatePinnedOpenApiProvenance,
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

function syntheticTree(paths = OPENAPI_GENERATOR_INPUT_PATHS) {
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
  for (const required of [
    'crates',
    'vendor',
    'tools',
    'scripts',
    'fixtures',
    'IrohaSwift',
    'javascript/iroha_js',
    'python/iroha_python',
    'kotlin',
    'java/iroha_android',
    'java/norito_java',
    'csharp',
    'release/version-map.toml',
    'docs/source/sdk/android/generated/codegen_hash_tree.json',
    'docs/source/sdk/android/generated/codegen_manifest_metadata.json',
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
  const first = computeOpenApiGeneratorInputTreeSha256({
    inventoryBytes,
    treeBytes: tree,
  });
  const second = computeOpenApiGeneratorInputTreeSha256({
    inventoryBytes,
    treeBytes: Buffer.from(tree),
  });
  assert.equal(first, second);
  assert.match(first, /^[0-9a-f]{64}$/);

  const substituted = Buffer.from(tree);
  substituted[12] ^= 1;
  assert.notEqual(
    computeOpenApiGeneratorInputTreeSha256({
      inventoryBytes,
      treeBytes: substituted,
    }),
    first,
  );
  assert.throws(
    () =>
      computeOpenApiGeneratorInputTreeSha256({
        inventoryBytes,
        treeBytes: syntheticTree(
          OPENAPI_GENERATOR_INPUT_PATHS.filter(
            (path) => path !== 'Cargo.lock',
          ),
        ),
      }),
    /Cargo\.lock.*missing/i,
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
