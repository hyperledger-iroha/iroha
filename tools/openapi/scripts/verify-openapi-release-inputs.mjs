#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0

/**
 * Verify the clean, ancestor-pinned Torii OpenAPI release-input contract.
 *
 * A release manifest is generated at a clean source commit and then checked in
 * by a later output-only commit. The manifest therefore records a real ancestor
 * commit, while generator_source_sha256_hex binds the exact canonical input
 * inventory at both that commit and the output-bearing HEAD.
 */
import {execFile} from 'node:child_process';
import {createHash} from 'node:crypto';
import {lstat} from 'node:fs/promises';
import {TextDecoder} from 'node:util';
import {dirname, join, resolve} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';

import {
  encodeOpenApiManifestSigningPayload,
  parseOpenApiManifestV2Json,
  verifyOpenApiManifestV2,
} from './lib/openapi-manifest-v2.mjs';
import {
  validateOpenApiGeneratorProvenance,
  validateReleaseOpenApiDocumentBytes,
} from './lib/openapi-provenance.mjs';
import {readOpenApiStableFile} from './lib/openapi-safe-file.mjs';
import {
  OPENAPI_CARGO_LOCK_PIN_MAX_BYTES,
  OPENAPI_CARGO_LOCK_PIN_PATH,
  parseOpenApiCargoLockPin,
  validateOpenApiCargoLockBytes,
} from './provision-openapi-cargo-lock.mjs';
import {verifyOpenApiVersions} from './verify-openapi-versions.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

export const OPENAPI_GENERATOR_INPUT_INVENTORY_HEADER =
  'iroha.openapi.generator-inputs.v1';
export const OPENAPI_GENERATOR_INPUT_TREE_DOMAIN =
  'iroha-openapi-generator-input-tree-v2';
export const OPENAPI_IGNORED_GENERATOR_INPUT_PATH = 'Cargo.lock';
export const OPENAPI_IGNORED_GENERATOR_INPUT_MODE = '100644';
export const OPENAPI_IGNORED_GENERATOR_INPUT_MAX_BYTES =
  16 * 1024 * 1024;
export const OPENAPI_GENERATOR_INPUT_PATHS = Object.freeze([
  '.cargo/config.toml',
  '.github/workflows/openapi.yml',
  OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
  'Cargo.toml',
  'IrohaSwift',
  'Makefile',
  'artifacts/openapi/allowed_signers.json',
  'ci',
  'ci/check_android_codegen.sh',
  'ci/check_openapi_spec.sh',
  'crates',
  'csharp',
  'data_model',
  'fixtures',
  'integration_tests',
  'java/iroha_android',
  'java/norito_java',
  'javascript/iroha_js',
  'kotlin',
  'mochi',
  'python/iroha_python',
  'python/iroha_torii_client',
  OPENAPI_CARGO_LOCK_PIN_PATH,
  'release/openapi-generator-inputs-v1.txt',
  'release/version-map.toml',
  'rust-toolchain.toml',
  'scripts',
  'scripts/android_codegen_docs.py',
  'scripts/android_codegen_replay_sorafs_fixture.py',
  'scripts/check_android_codegen_parity.py',
  'scripts/check_sorafs_release_version_map.py',
  'specs/sdk/android/generated',
  'specs/sdk/android/generated/codegen_hash_tree.json',
  'specs/sdk/android/generated/codegen_manifest_metadata.json',
  'tools',
  'vendor',
  'xtask',
]);
export const OPENAPI_TRACKED_GENERATOR_INPUT_PATHS = Object.freeze(
  OPENAPI_GENERATOR_INPUT_PATHS.filter(
    (path) => path !== OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
  ),
);

const SHA256_HEX = /^[0-9a-f]{64}$/;
const GIT_SHA1_HEX = /^[0-9a-f]{40}$/;
const OPENAPI_SPEC_MAX_BYTES = 64 * 1024 * 1024;
const OPENAPI_MANIFEST_MAX_BYTES = 64 * 1024;
const OPENAPI_VERSIONS_MAX_BYTES = 1024 * 1024;
const RELEASE_VERSION_MAP_MAX_BYTES = 1024 * 1024;
const GENERATOR_INPUT_INVENTORY_MAX_BYTES = 64 * 1024;
const GIT_MAX_BUFFER_BYTES = 128 * 1024 * 1024;

const defaultRepoRoot = resolve(__dirname, '..', '..', '..');

/**
 * Parse the checked-in inventory and require the exact V1 path contract.
 */
export function parseOpenApiGeneratorInputInventory(bytes) {
  const payload = Buffer.from(bytes);
  let text;
  try {
    text = new TextDecoder('utf-8', {fatal: true}).decode(payload);
  } catch (error) {
    throw new Error(
      `OpenAPI generator input inventory must be UTF-8: ${error?.message ?? error}`,
    );
  }
  if (text.includes('\r') || !text.endsWith('\n')) {
    throw new Error(
      'OpenAPI generator input inventory must use LF endings and one final newline',
    );
  }
  const lines = text.slice(0, -1).split('\n');
  if (lines.shift() !== OPENAPI_GENERATOR_INPUT_INVENTORY_HEADER) {
    throw new Error(
      'OpenAPI generator input inventory has an invalid schema header',
    );
  }
  for (const path of lines) {
    if (
      path.length === 0 ||
      path.startsWith('/') ||
      path.endsWith('/') ||
      path.includes('\\') ||
      path.split('/').some((component) =>
        component === '' || component === '.' || component === '..')
    ) {
      throw new Error(
        `OpenAPI generator input inventory contains a noncanonical path ${JSON.stringify(path)}`,
      );
    }
  }
  if (
    new Set(lines).size !== lines.length ||
    lines.some((path, index) =>
      index > 0 && lines[index - 1] >= path)
  ) {
    throw new Error(
      'OpenAPI generator input inventory paths must be strictly sorted and unique',
    );
  }
  if (
    lines.length !== OPENAPI_GENERATOR_INPUT_PATHS.length ||
    lines.some((path, index) => path !== OPENAPI_GENERATOR_INPUT_PATHS[index])
  ) {
    throw new Error(
      'OpenAPI generator input inventory does not match the exact V1 release-input contract',
    );
  }
  return [...lines];
}

/**
 * Compute the cross-language SHA-256 over an inventory and canonical ls-tree.
 */
export function computeOpenApiGeneratorInputTreeSha256({
  inventoryBytes,
  treeBytes,
  ignoredInputBytes,
  inventoryPaths = parseOpenApiGeneratorInputInventory(inventoryBytes),
}) {
  const tree = Buffer.from(treeBytes);
  const trackedInventoryPaths =
    trackedOpenApiGeneratorInputPaths(inventoryPaths);
  validateGitTreeBytes(tree, trackedInventoryPaths);
  return computeOpenApiGeneratorInputTreeSha256Components({
    inventoryBytes,
    treeBytes: tree,
    ignoredInputBytes,
  });
}

/**
 * Hash the exact V2 cross-language component contract.
 *
 * Tree validation belongs to computeOpenApiGeneratorInputTreeSha256 so this
 * helper can also carry a compact fixed cross-language digest vector.
 */
export function computeOpenApiGeneratorInputTreeSha256Components({
  inventoryBytes,
  treeBytes,
  ignoredInputBytes,
}) {
  const inventory = toDigestBytes(inventoryBytes, 'inventoryBytes');
  const tree = toDigestBytes(treeBytes, 'treeBytes');
  const ignoredInput = validateIgnoredInputBytes(ignoredInputBytes);
  const hasher = createHash('sha256');
  updateSha256Component(
    hasher,
    'domain',
    OPENAPI_GENERATOR_INPUT_TREE_DOMAIN,
  );
  updateSha256Component(hasher, 'inventory', inventory);
  updateSha256Component(hasher, 'tree', tree);
  updateSha256Component(
    hasher,
    'ignored-input-path',
    OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
  );
  updateSha256Component(
    hasher,
    'ignored-input-mode',
    OPENAPI_IGNORED_GENERATOR_INPUT_MODE,
  );
  updateSha256Component(
    hasher,
    'ignored-input-bytes',
    ignoredInput,
  );
  return hasher.digest('hex');
}

/**
 * Read the separately provisioned ignored Cargo.lock without following links.
 */
export async function readOpenApiIgnoredCargoLock(
  filePath,
  {stableFileReader = readOpenApiStableFile} = {},
) {
  const snapshot = await captureOpenApiIgnoredCargoLock(filePath, {
    stableFileReader,
  });
  return snapshot.bytes;
}

/**
 * Validate the Git evidence that Cargo.lock is ignored and never tree-backed.
 */
export function validateOpenApiIgnoredCargoLockGitEvidence({
  ignoredPathBytes,
  indexEntryBytes,
  pinnedTreeEntryBytes,
  headTreeEntryBytes,
}) {
  const expectedIgnoredPath = Buffer.from(
    `${OPENAPI_IGNORED_GENERATOR_INPUT_PATH}\n`,
    'utf8',
  );
  if (
    !Buffer.isBuffer(ignoredPathBytes) ||
    !ignoredPathBytes.equals(expectedIgnoredPath)
  ) {
    throw new Error(
      'OpenAPI generator input Cargo.lock must be the exact ignored root path',
    );
  }
  for (const [label, bytes] of [
    ['Git index', indexEntryBytes],
    ['pinned Git tree', pinnedTreeEntryBytes],
    ['HEAD Git tree', headTreeEntryBytes],
  ]) {
    if (!Buffer.isBuffer(bytes) || bytes.length !== 0) {
      throw new Error(
        `OpenAPI generator input Cargo.lock must be untracked in the ${label}`,
      );
    }
  }
}

/**
 * Require one identical, non-executable pin blob in the pinned and HEAD trees.
 */
export function validateOpenApiCargoLockPinGitEvidence({
  pinnedTreeEntryBytes,
  headTreeEntryBytes,
}) {
  const pinnedBlobOid = parseOpenApiCargoLockPinTreeEntry(
    pinnedTreeEntryBytes,
    'pinned Git tree',
  );
  const headBlobOid = parseOpenApiCargoLockPinTreeEntry(
    headTreeEntryBytes,
    'HEAD Git tree',
  );
  if (pinnedBlobOid !== headBlobOid) {
    throw new Error(
      'OpenAPI Cargo.lock pin must reference the same blob in the pinned and HEAD Git trees',
    );
  }
  return pinnedBlobOid;
}

/**
 * Parse source-bound pin bytes and validate the exact ignored root lock.
 */
export function validateOpenApiIgnoredCargoLockAgainstPin({
  ignoredInputBytes,
  pinBytes,
}) {
  const sourceBoundPinBytes = toDigestBytes(pinBytes, 'pinBytes');
  if (sourceBoundPinBytes.length > OPENAPI_CARGO_LOCK_PIN_MAX_BYTES) {
    throw new Error(
      `source-bound OpenAPI Cargo.lock pin exceeds the ${OPENAPI_CARGO_LOCK_PIN_MAX_BYTES}-byte limit`,
    );
  }
  const pin = parseOpenApiCargoLockPin(sourceBoundPinBytes);
  validateOpenApiCargoLockBytes(ignoredInputBytes, pin);
  return pin;
}

/**
 * Validate manifests against resolved Git and input-tree evidence.
 *
 * Git operations remain outside this pure function so every malformed,
 * nonexistent, non-ancestor, omitted, zero, and substituted case can be
 * exercised without trusting a test repository fixture.
 */
export function validatePinnedOpenApiProvenance({
  rootManifest,
  currentManifest,
  resolvedGeneratorCommit,
  generatorCommitIsAncestor,
  expectedGeneratedUnixMs,
  pinnedSourceSha256Hex,
  headSourceSha256Hex,
}) {
  const root = validateOpenApiGeneratorProvenance(rootManifest, {
    label: 'root OpenAPI manifest',
    signed: rootManifest?.artifact?.signature !== null,
    requireClean: true,
  });
  const current = validateOpenApiGeneratorProvenance(currentManifest, {
    label: 'current OpenAPI manifest',
    signed: currentManifest?.artifact?.signature !== null,
    requireClean: true,
  });
  if (
    root.commit !== current.commit ||
    root.sourceSha256Hex !== current.sourceSha256Hex
  ) {
    throw new Error(
      'root and current OpenAPI manifests must bind identical generator provenance',
    );
  }
  if (
    typeof resolvedGeneratorCommit !== 'string' ||
    !GIT_SHA1_HEX.test(resolvedGeneratorCommit)
  ) {
    throw new Error(
      `generator_commit ${String(root.commit)} does not resolve to a full Git commit object`,
    );
  }
  if (resolvedGeneratorCommit !== root.commit) {
    throw new Error(
      `generator_commit ${root.commit} resolves to substituted commit ${resolvedGeneratorCommit}`,
    );
  }
  if (generatorCommitIsAncestor !== true) {
    throw new Error(
      `generator_commit ${root.commit} is not an ancestor of the output-bearing HEAD`,
    );
  }
  if (
    !Number.isSafeInteger(expectedGeneratedUnixMs) ||
    expectedGeneratedUnixMs <= 0
  ) {
    throw new Error(
      'pinned generator commit timestamp must be a positive JavaScript-safe integer',
    );
  }
  if (
    rootManifest.generated_unix_ms !== expectedGeneratedUnixMs ||
    currentManifest.generated_unix_ms !== expectedGeneratedUnixMs
  ) {
    throw new Error(
      `OpenAPI manifests generated_unix_ms must equal pinned generator commit time ${expectedGeneratedUnixMs}`,
    );
  }
  requireNonzeroSha256(
    pinnedSourceSha256Hex,
    'pinned generator input-tree digest',
  );
  requireNonzeroSha256(
    headSourceSha256Hex,
    'HEAD generator input-tree digest',
  );
  if (pinnedSourceSha256Hex !== root.sourceSha256Hex) {
    throw new Error(
      `manifest generator_source_sha256_hex ${root.sourceSha256Hex} does not match pinned generator inputs ${pinnedSourceSha256Hex}`,
    );
  }
  if (headSourceSha256Hex !== root.sourceSha256Hex) {
    throw new Error(
      `manifest generator_source_sha256_hex ${root.sourceSha256Hex} does not match current generator inputs ${headSourceSha256Hex}`,
    );
  }
  return {
    generatorCommit: root.commit,
    generatorSourceSha256Hex: root.sourceSha256Hex,
  };
}

/**
 * Run the complete release-input check and return a deterministic summary.
 */
export async function verifyOpenApiReleaseInputs({
  repoRoot = defaultRepoRoot,
  requireCleanWorkingTree = true,
} = {}) {
  const root = resolve(repoRoot);
  const openapiDir = join(root, 'artifacts', 'openapi');
  const inventoryPath = join(
    root,
    'release',
    'openapi-generator-inputs-v1.txt',
  );
  const paths = {
    rootSpec: join(openapiDir, 'torii.json'),
    currentSpec: join(openapiDir, 'versions', 'current', 'torii.json'),
    rootManifest: join(openapiDir, 'manifest.json'),
    currentManifest: join(
      openapiDir,
      'versions',
      'current',
      'manifest.json',
    ),
    versions: join(openapiDir, 'versions.json'),
    releaseVersionMap: join(root, 'release', 'version-map.toml'),
    ignoredCargoLock: join(
      root,
      OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
    ),
  };

  if (requireCleanWorkingTree) {
    const status = await gitBytes(root, [
      'status',
      '--porcelain=v1',
      '-z',
      '--untracked-files=all',
    ]);
    if (status.length !== 0) {
      throw new Error(
        'OpenAPI release-input verification requires a clean checkout',
      );
    }
  }

  const [
    inventoryBytes,
    rootSpecBytes,
    currentSpecBytes,
    rootManifestBytes,
    currentManifestBytes,
    versionsBytes,
    releaseVersionMapBytes,
  ] = await Promise.all([
    readOpenApiStableFile(inventoryPath, {
      label: 'OpenAPI generator input inventory',
      maxBytes: GENERATOR_INPUT_INVENTORY_MAX_BYTES,
    }),
    readOpenApiStableFile(paths.rootSpec, {
      label: 'root Torii OpenAPI specification',
      maxBytes: OPENAPI_SPEC_MAX_BYTES,
    }),
    readOpenApiStableFile(paths.currentSpec, {
      label: 'current Torii OpenAPI specification',
      maxBytes: OPENAPI_SPEC_MAX_BYTES,
    }),
    readOpenApiStableFile(paths.rootManifest, {
      label: 'root Torii OpenAPI manifest',
      maxBytes: OPENAPI_MANIFEST_MAX_BYTES,
    }),
    readOpenApiStableFile(paths.currentManifest, {
      label: 'current Torii OpenAPI manifest',
      maxBytes: OPENAPI_MANIFEST_MAX_BYTES,
    }),
    readOpenApiStableFile(paths.versions, {
      label: 'Torii OpenAPI versions index',
      maxBytes: OPENAPI_VERSIONS_MAX_BYTES,
    }),
    readOpenApiStableFile(paths.releaseVersionMap, {
      label: 'SoraFS release version map',
      maxBytes: RELEASE_VERSION_MAP_MAX_BYTES,
    }),
  ]);

  const inventoryPaths = parseOpenApiGeneratorInputInventory(inventoryBytes);
  const trackedInventoryPaths =
    trackedOpenApiGeneratorInputPaths(inventoryPaths);
  if (!rootSpecBytes.equals(currentSpecBytes)) {
    throw new Error(
      'root and current Torii OpenAPI specifications must be byte-identical',
    );
  }
  if (!rootManifestBytes.equals(currentManifestBytes)) {
    throw new Error(
      'root and current Torii OpenAPI manifests must be byte-identical',
    );
  }
  validateReleaseOpenApiDocumentBytes(rootSpecBytes, {
    label: 'root Torii OpenAPI specification',
  });
  const rootManifest = parseOpenApiManifestV2Json(rootManifestBytes, {
    label: 'root Torii OpenAPI manifest',
  });
  const currentManifest = parseOpenApiManifestV2Json(currentManifestBytes, {
    label: 'current Torii OpenAPI manifest',
  });
  verifyOpenApiManifestV2({
    manifest: rootManifest,
    artifactBytes: rootSpecBytes,
    label: 'root Torii OpenAPI manifest',
    expectedArtifactPath: 'torii.json',
    requireSignature: false,
    requireClean: true,
  });
  verifyOpenApiManifestV2({
    manifest: currentManifest,
    artifactBytes: currentSpecBytes,
    label: 'current Torii OpenAPI manifest',
    expectedArtifactPath: 'torii.json',
    requireSignature: false,
    requireClean: true,
  });

  const generatorCommit = rootManifest.generator_commit;
  const resolvedGeneratorCommit = await resolveGitCommit(
    root,
    generatorCommit,
  );
  if (resolvedGeneratorCommit === null) {
    validatePinnedOpenApiProvenance({
      rootManifest,
      currentManifest,
      resolvedGeneratorCommit,
      generatorCommitIsAncestor: false,
      expectedGeneratedUnixMs: rootManifest.generated_unix_ms,
      pinnedSourceSha256Hex:
        rootManifest.generator_source_sha256_hex,
      headSourceSha256Hex:
        rootManifest.generator_source_sha256_hex,
    });
  }
  const generatorCommitIsAncestor = await gitIsAncestor(
    root,
    generatorCommit,
    'HEAD',
  );
  if (!generatorCommitIsAncestor) {
    validatePinnedOpenApiProvenance({
      rootManifest,
      currentManifest,
      resolvedGeneratorCommit,
      generatorCommitIsAncestor,
      expectedGeneratedUnixMs: rootManifest.generated_unix_ms,
      pinnedSourceSha256Hex:
        rootManifest.generator_source_sha256_hex,
      headSourceSha256Hex:
        rootManifest.generator_source_sha256_hex,
    });
  }
  const [
    pinnedTreeBytes,
    headTreeBytes,
    expectedGeneratedUnixMs,
    ignoredCargoLockGitState,
  ] = await Promise.all([
    readGeneratorInputTree(
      root,
      generatorCommit,
      trackedInventoryPaths,
    ),
    readGeneratorInputTree(root, 'HEAD', trackedInventoryPaths),
    readGitCommitTimestampMs(root, generatorCommit),
    validateOpenApiIgnoredCargoLockGitState(root, generatorCommit),
  ]);
  const pinBytes = await readOpenApiCargoLockPinBlob(
    root,
    ignoredCargoLockGitState.pinBlobOid,
  );
  const ignoredCargoLockSnapshot =
    await captureOpenApiIgnoredCargoLock(paths.ignoredCargoLock);
  validateOpenApiIgnoredCargoLockAgainstPin({
    ignoredInputBytes: ignoredCargoLockSnapshot.bytes,
    pinBytes,
  });
  const pinnedSourceSha256Hex =
    computeOpenApiGeneratorInputTreeSha256({
      inventoryBytes,
      treeBytes: pinnedTreeBytes,
      ignoredInputBytes: ignoredCargoLockSnapshot.bytes,
      inventoryPaths,
    });
  const headSourceSha256Hex =
    computeOpenApiGeneratorInputTreeSha256({
      inventoryBytes,
      treeBytes: headTreeBytes,
      ignoredInputBytes: ignoredCargoLockSnapshot.bytes,
      inventoryPaths,
    });
  const provenance = validatePinnedOpenApiProvenance({
    rootManifest,
    currentManifest,
    resolvedGeneratorCommit,
    generatorCommitIsAncestor,
    expectedGeneratedUnixMs,
    pinnedSourceSha256Hex,
    headSourceSha256Hex,
  });

  await verifyOpenApiVersions({
    outputDir: openapiDir,
    versionsDir: join(openapiDir, 'versions'),
    versionsFile: paths.versions,
  });

  const signingPayload = encodeOpenApiManifestSigningPayload({
    manifest: rootManifest,
    artifactBytes: rootSpecBytes,
    label: 'root Torii OpenAPI manifest',
    expectedArtifactPath: 'torii.json',
  });
  await assertOpenApiIgnoredCargoLockUnchanged(
    paths.ignoredCargoLock,
    ignoredCargoLockSnapshot,
  );
  return {
    schema: 'iroha.openapi.release_inputs.v1',
    generator_commit: provenance.generatorCommit,
    generated_unix_ms: expectedGeneratedUnixMs,
    generator_source_sha256_hex:
      provenance.generatorSourceSha256Hex,
    generator_input_inventory_sha256_hex: sha256Hex(inventoryBytes),
    openapi_spec_bytes: rootSpecBytes.length,
    openapi_spec_sha256_hex: sha256Hex(rootSpecBytes),
    openapi_manifest_sha256_hex: sha256Hex(rootManifestBytes),
    openapi_signing_payload_sha256_hex: sha256Hex(signingPayload),
    openapi_versions_sha256_hex: sha256Hex(versionsBytes),
    release_version_map_sha256_hex: sha256Hex(releaseVersionMapBytes),
  };
}

async function readGeneratorInputTree(repoRoot, commit, inventoryPaths) {
  return gitBytes(repoRoot, [
    'ls-tree',
    '-r',
    '-z',
    '--full-tree',
    commit,
    '--',
    ...inventoryPaths,
  ]);
}

async function validateOpenApiIgnoredCargoLockGitState(
  repoRoot,
  generatorCommit,
) {
  let ignoredPathBytes;
  try {
    ignoredPathBytes = await gitBytes(repoRoot, [
      'check-ignore',
      '--no-index',
      '--',
      OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
    ]);
  } catch (error) {
    if (error?.gitExitCode === 1) {
      throw new Error(
        'OpenAPI generator input Cargo.lock must be ignored by repository policy',
        {cause: error},
      );
    }
    throw error;
  }
  const [
    indexEntryBytes,
    pinnedTreeEntryBytes,
    headTreeEntryBytes,
    pinnedPinTreeEntryBytes,
    headPinTreeEntryBytes,
  ] = await Promise.all([
    gitBytes(repoRoot, [
      'ls-files',
      '--stage',
      '-z',
      '--',
      OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
    ]),
    gitBytes(repoRoot, [
      'ls-tree',
      '-z',
      '--full-tree',
      generatorCommit,
      '--',
      OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
    ]),
    gitBytes(repoRoot, [
      'ls-tree',
      '-z',
      '--full-tree',
      'HEAD',
      '--',
      OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
    ]),
    gitBytes(repoRoot, [
      'ls-tree',
      '-z',
      '--full-tree',
      generatorCommit,
      '--',
      OPENAPI_CARGO_LOCK_PIN_PATH,
    ]),
    gitBytes(repoRoot, [
      'ls-tree',
      '-z',
      '--full-tree',
      'HEAD',
      '--',
      OPENAPI_CARGO_LOCK_PIN_PATH,
    ]),
  ]);
  validateOpenApiIgnoredCargoLockGitEvidence({
    ignoredPathBytes,
    indexEntryBytes,
    pinnedTreeEntryBytes,
    headTreeEntryBytes,
  });
  return {
    pinBlobOid: validateOpenApiCargoLockPinGitEvidence({
      pinnedTreeEntryBytes: pinnedPinTreeEntryBytes,
      headTreeEntryBytes: headPinTreeEntryBytes,
    }),
  };
}

async function readOpenApiCargoLockPinBlob(repoRoot, blobOid) {
  const bytes = await gitBytes(repoRoot, [
    'cat-file',
    'blob',
    blobOid,
  ]);
  if (bytes.length > OPENAPI_CARGO_LOCK_PIN_MAX_BYTES) {
    throw new Error(
      `source-bound OpenAPI Cargo.lock pin exceeds the ${OPENAPI_CARGO_LOCK_PIN_MAX_BYTES}-byte limit`,
    );
  }
  return bytes;
}

async function resolveGitCommit(repoRoot, commit) {
  if (typeof commit !== 'string' || !GIT_SHA1_HEX.test(commit)) {
    return null;
  }
  try {
    const resolved = await gitBytes(repoRoot, [
      'rev-parse',
      '--verify',
      `${commit}^{commit}`,
    ]);
    const text = resolved.toString('ascii').trim();
    return GIT_SHA1_HEX.test(text) ? text : null;
  } catch {
    return null;
  }
}

async function gitIsAncestor(repoRoot, ancestor, descendant) {
  try {
    await gitBytes(repoRoot, [
      'merge-base',
      '--is-ancestor',
      ancestor,
      descendant,
    ]);
    return true;
  } catch (error) {
    if (error?.gitExitCode === 1) {
      return false;
    }
    throw error;
  }
}

async function readGitCommitTimestampMs(repoRoot, commit) {
  const bytes = await gitBytes(repoRoot, [
    'show',
    '-s',
    '--format=%ct',
    commit,
  ]);
  const text = bytes.toString('ascii');
  if (!/^[1-9][0-9]*\n$/.test(text)) {
    throw new Error(
      `generator_commit ${commit} has a noncanonical Git commit timestamp`,
    );
  }
  const milliseconds = BigInt(text.slice(0, -1)) * 1_000n;
  if (milliseconds > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error(
      `generator_commit ${commit} timestamp exceeds the JavaScript-safe integer range`,
    );
  }
  return Number(milliseconds);
}

function gitBytes(repoRoot, args) {
  return new Promise((resolvePromise, rejectPromise) => {
    execFile(
      'git',
      ['-C', repoRoot, ...args],
      {
        encoding: 'buffer',
        maxBuffer: GIT_MAX_BUFFER_BYTES,
        windowsHide: true,
      },
      (error, stdout, stderr) => {
        if (error) {
          const wrapped = new Error(
            `git ${args[0] ?? ''} failed (${error.code ?? 'unknown'}): ${Buffer.from(stderr ?? '').toString('utf8').trim()}`,
            {cause: error},
          );
          wrapped.gitExitCode = error.code;
          rejectPromise(wrapped);
          return;
        }
        resolvePromise(Buffer.from(stdout));
      },
    );
  });
}

function validateGitTreeBytes(tree, inventoryPaths) {
  if (tree.length === 0) {
    throw new Error(
      'OpenAPI generator input inventory resolved to an empty Git tree',
    );
  }
  const decoder = new TextDecoder('utf-8', {fatal: true});
  const resolvedPaths = [];
  const seen = new Set();
  for (const entry of splitNul(tree)) {
    const tab = entry.indexOf(0x09);
    if (tab < 0) {
      throw new Error(
        'git ls-tree returned an OpenAPI generator input without a path separator',
      );
    }
    const metadata = entry.subarray(0, tab).toString('ascii').split(' ');
    if (
      metadata.length !== 3 ||
      !/^(?:100644|100755|120000)$/.test(metadata[0]) ||
      metadata[1] !== 'blob' ||
      !/^[0-9a-f]{40}$/.test(metadata[2])
    ) {
      throw new Error(
        'OpenAPI generator inputs must resolve only to canonical Git blobs',
      );
    }
    let path;
    try {
      path = decoder.decode(entry.subarray(tab + 1));
    } catch (error) {
      throw new Error(
        `git ls-tree returned a non-UTF-8 OpenAPI generator input path: ${error?.message ?? error}`,
      );
    }
    if (seen.has(path)) {
      throw new Error(`git ls-tree duplicated OpenAPI generator input ${path}`);
    }
    if (
      path === OPENAPI_IGNORED_GENERATOR_INPUT_PATH ||
      path.startsWith(`${OPENAPI_IGNORED_GENERATOR_INPUT_PATH}/`)
    ) {
      throw new Error(
        'ignored OpenAPI generator input Cargo.lock must not resolve from a Git tree',
      );
    }
    seen.add(path);
    resolvedPaths.push(path);
  }
  for (const required of inventoryPaths) {
    const prefix = `${required}/`;
    if (
      !resolvedPaths.some((path) =>
        path === required || path.startsWith(prefix))
    ) {
      throw new Error(
        `OpenAPI generator input inventory path ${required} is missing from the pinned Git tree`,
      );
    }
  }
}

function trackedOpenApiGeneratorInputPaths(inventoryPaths) {
  if (!Array.isArray(inventoryPaths)) {
    throw new TypeError(
      'OpenAPI generator input inventory paths must be an array',
    );
  }
  const ignoredPaths = inventoryPaths.filter(
    (path) => path === OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
  );
  if (
    ignoredPaths.length !== 1 ||
    inventoryPaths.length !==
      OPENAPI_TRACKED_GENERATOR_INPUT_PATHS.length + 1
  ) {
    throw new Error(
      'Cargo.lock must be the sole separately bound ignored OpenAPI generator input',
    );
  }
  const trackedPaths = inventoryPaths.filter(
    (path) => path !== OPENAPI_IGNORED_GENERATOR_INPUT_PATH,
  );
  if (
    trackedPaths.length !== OPENAPI_TRACKED_GENERATOR_INPUT_PATHS.length ||
    trackedPaths.some(
      (path, index) =>
        path !== OPENAPI_TRACKED_GENERATOR_INPUT_PATHS[index],
    )
  ) {
    throw new Error(
      'OpenAPI tracked generator inputs do not match the exact release-input contract',
    );
  }
  return trackedPaths;
}

function parseOpenApiCargoLockPinTreeEntry(bytes, label) {
  if (!Buffer.isBuffer(bytes)) {
    throw new TypeError(
      `OpenAPI Cargo.lock pin ${label} evidence must be bytes`,
    );
  }
  const escapedPath = OPENAPI_CARGO_LOCK_PIN_PATH.replace(
    /[.*+?^${}()|[\]\\]/g,
    '\\$&',
  );
  const match = new RegExp(
    `^100644 blob ([0-9a-f]{40})\\t${escapedPath}\\0$`,
  ).exec(bytes.toString('utf8'));
  if (!match || /^0{40}$/.test(match[1])) {
    throw new Error(
      `OpenAPI Cargo.lock pin must be one canonical 100644 blob in the ${label}`,
    );
  }
  return match[1];
}

async function captureOpenApiIgnoredCargoLock(
  filePath,
  {stableFileReader = readOpenApiStableFile} = {},
) {
  if (typeof stableFileReader !== 'function') {
    throw new TypeError('stableFileReader must be a function');
  }
  const resolved = resolve(filePath);
  const before = await inspectOpenApiIgnoredCargoLock(resolved);
  const readBytes = await stableFileReader(resolved, {
    label: 'ignored OpenAPI generator input Cargo.lock',
    maxBytes: OPENAPI_IGNORED_GENERATOR_INPUT_MAX_BYTES,
  });
  const bytes = validateIgnoredInputBytes(readBytes);
  const after = await inspectOpenApiIgnoredCargoLock(resolved);
  if (
    !sameOpenApiIgnoredCargoLockState(before, after) ||
    after.size !== BigInt(bytes.length)
  ) {
    throw new Error(
      `ignored OpenAPI generator input Cargo.lock ${resolved} changed while it was read`,
    );
  }
  return {bytes, state: after};
}

async function assertOpenApiIgnoredCargoLockUnchanged(filePath, snapshot) {
  const resolved = resolve(filePath);
  const current = await inspectOpenApiIgnoredCargoLock(resolved);
  if (!sameOpenApiIgnoredCargoLockState(snapshot.state, current)) {
    throw new Error(
      `ignored OpenAPI generator input Cargo.lock ${resolved} changed during release-input verification`,
    );
  }
}

async function inspectOpenApiIgnoredCargoLock(resolved) {
  let metadata;
  try {
    metadata = await lstat(resolved, {bigint: true});
  } catch (error) {
    const wrapped = new Error(
      `failed to inspect ignored OpenAPI generator input Cargo.lock ${resolved}: ${error?.message ?? error}`,
      {cause: error},
    );
    wrapped.code = error?.code;
    throw wrapped;
  }
  if (metadata.isSymbolicLink()) {
    throw new Error(
      `ignored OpenAPI generator input Cargo.lock ${resolved} must not be a symlink`,
    );
  }
  if (!metadata.isFile()) {
    throw new Error(
      `ignored OpenAPI generator input Cargo.lock ${resolved} must be a regular file`,
    );
  }
  if (metadata.nlink !== 1n) {
    throw new Error(
      `ignored OpenAPI generator input Cargo.lock ${resolved} must have exactly one hard link`,
    );
  }
  if (
    process.platform !== 'win32' &&
    (metadata.mode & 0o111n) !== 0n
  ) {
    throw new Error(
      `ignored OpenAPI generator input Cargo.lock ${resolved} must not be executable`,
    );
  }
  if (metadata.size === 0n) {
    throw new Error(
      `ignored OpenAPI generator input Cargo.lock ${resolved} must not be empty`,
    );
  }
  if (
    metadata.size >
    BigInt(OPENAPI_IGNORED_GENERATOR_INPUT_MAX_BYTES)
  ) {
    throw new Error(
      `ignored OpenAPI generator input Cargo.lock ${resolved} exceeds the ${OPENAPI_IGNORED_GENERATOR_INPUT_MAX_BYTES}-byte limit`,
    );
  }
  return metadata;
}

function sameOpenApiIgnoredCargoLockState(left, right) {
  return (
    left.dev === right.dev &&
    left.ino === right.ino &&
    left.mode === right.mode &&
    left.nlink === right.nlink &&
    left.size === right.size &&
    left.mtimeNs === right.mtimeNs &&
    left.ctimeNs === right.ctimeNs
  );
}

function splitNul(bytes) {
  const entries = [];
  let start = 0;
  for (let index = 0; index < bytes.length; index += 1) {
    if (bytes[index] === 0) {
      if (index > start) {
        entries.push(bytes.subarray(start, index));
      }
      start = index + 1;
    }
  }
  if (start !== bytes.length) {
    throw new Error('git ls-tree output must end with a NUL byte');
  }
  return entries;
}

function toDigestBytes(value, label) {
  if (!Buffer.isBuffer(value) && !(value instanceof Uint8Array)) {
    throw new TypeError(`${label} must be bytes`);
  }
  return Buffer.from(value);
}

function validateIgnoredInputBytes(value) {
  const bytes = toDigestBytes(value, 'ignoredInputBytes');
  if (bytes.length === 0) {
    throw new Error(
      'ignored OpenAPI generator input Cargo.lock must not be empty',
    );
  }
  if (bytes.length > OPENAPI_IGNORED_GENERATOR_INPUT_MAX_BYTES) {
    throw new Error(
      `ignored OpenAPI generator input Cargo.lock exceeds the ${OPENAPI_IGNORED_GENERATOR_INPUT_MAX_BYTES}-byte limit`,
    );
  }
  return bytes;
}

function updateSha256Component(hasher, label, value) {
  const labelBytes = Buffer.from(label, 'utf8');
  const valueBytes = Buffer.isBuffer(value)
    ? value
    : Buffer.from(value, 'utf8');
  const labelLength = Buffer.alloc(8);
  const valueLength = Buffer.alloc(8);
  labelLength.writeBigUInt64LE(BigInt(labelBytes.length));
  valueLength.writeBigUInt64LE(BigInt(valueBytes.length));
  hasher.update(labelLength);
  hasher.update(labelBytes);
  hasher.update(valueLength);
  hasher.update(valueBytes);
}

function requireNonzeroSha256(value, label) {
  if (
    typeof value !== 'string' ||
    !SHA256_HEX.test(value) ||
    /^0{64}$/.test(value)
  ) {
    throw new Error(
      `${label} must be 64 lowercase hexadecimal characters and nonzero`,
    );
  }
}

function sha256Hex(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

async function main() {
  if (process.argv.length !== 2) {
    throw new Error(
      'verify-openapi-release-inputs.mjs does not accept command-line arguments',
    );
  }
  const summary = await verifyOpenApiReleaseInputs();
  process.stdout.write(`${JSON.stringify(summary)}\n`);
}

if (
  process.argv[1] &&
  pathToFileURL(resolve(process.argv[1])).href === import.meta.url
) {
  main().catch((error) => {
    console.error(`error: ${error?.message ?? error}`);
    process.exitCode = 1;
  });
}
