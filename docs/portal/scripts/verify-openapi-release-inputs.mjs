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
import {verifyOpenApiVersions} from './verify-openapi-versions.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

export const OPENAPI_GENERATOR_INPUT_INVENTORY_HEADER =
  'iroha.openapi.generator-inputs.v1';
export const OPENAPI_GENERATOR_INPUT_TREE_DOMAIN =
  'iroha-openapi-generator-input-tree-v1';
export const OPENAPI_GENERATOR_INPUT_PATHS = Object.freeze([
  '.github/workflows/openapi.yml',
  'Cargo.lock',
  'Cargo.toml',
  'IrohaSwift',
  'Makefile',
  'ci',
  'ci/check_android_codegen.sh',
  'ci/check_openapi_spec.sh',
  'crates',
  'csharp',
  'data_model',
  'docs/i18n',
  'docs/portal/package-lock.json',
  'docs/portal/package.json',
  'docs/portal/scripts',
  'docs/portal/static/openapi/allowed_signers.json',
  'docs/source/sdk/android/generated',
  'docs/source/sdk/android/generated/codegen_hash_tree.json',
  'docs/source/sdk/android/generated/codegen_manifest_metadata.json',
  'fixtures',
  'integration_tests',
  'java/iroha_android',
  'java/norito_java',
  'javascript/iroha_js',
  'kotlin',
  'mochi',
  'python/iroha_python',
  'release/openapi-generator-inputs-v1.txt',
  'release/version-map.toml',
  'rust-toolchain.toml',
  'scripts',
  'scripts/android_codegen_docs.py',
  'scripts/android_codegen_replay_sorafs_fixture.py',
  'scripts/check_android_codegen_parity.py',
  'scripts/check_sorafs_release_version_map.py',
  'tools',
  'vendor',
  'xtask',
]);

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
  inventoryPaths = parseOpenApiGeneratorInputInventory(inventoryBytes),
}) {
  const tree = Buffer.from(treeBytes);
  validateGitTreeBytes(tree, inventoryPaths);
  const hasher = createHash('sha256');
  updateSha256Component(
    hasher,
    'domain',
    OPENAPI_GENERATOR_INPUT_TREE_DOMAIN,
  );
  updateSha256Component(hasher, 'inventory', Buffer.from(inventoryBytes));
  updateSha256Component(hasher, 'tree', tree);
  return hasher.digest('hex');
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
  const openapiDir = join(root, 'docs', 'portal', 'static', 'openapi');
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
  const [pinnedTreeBytes, headTreeBytes, expectedGeneratedUnixMs] = await Promise.all([
    readGeneratorInputTree(root, generatorCommit, inventoryPaths),
    readGeneratorInputTree(root, 'HEAD', inventoryPaths),
    readGitCommitTimestampMs(root, generatorCommit),
  ]);
  const pinnedSourceSha256Hex =
    computeOpenApiGeneratorInputTreeSha256({
      inventoryBytes,
      treeBytes: pinnedTreeBytes,
      inventoryPaths,
    });
  const headSourceSha256Hex =
    computeOpenApiGeneratorInputTreeSha256({
      inventoryBytes,
      treeBytes: headTreeBytes,
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
