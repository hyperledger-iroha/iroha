#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0

/**
 * Verify the tracked root Cargo.lock used by the Torii OpenAPI release gate.
 *
 * The canonical verifier interface is:
 *
 *   node tools/openapi/scripts/provision-openapi-cargo-lock.mjs provision
 *   node tools/openapi/scripts/provision-openapi-cargo-lock.mjs \
 *     provision --source=/absolute/canonical/Cargo.lock
 *   node tools/openapi/scripts/provision-openapi-cargo-lock.mjs \
 *     pin --source=/absolute/canonical/Cargo.lock \
 *     --output=/absolute/external/staging/openapi-cargo-lock-v1.txt
 *
 * The tracked root lock is the sole lock authority. An explicit source may be
 * supplied only as an additional byte-identical comparison input. Provision
 * never writes the checkout, starts Cargo, or generates lock bytes.
 */
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {constants as fsConstants} from 'node:fs';
import {lstat, open, realpath} from 'node:fs/promises';
import path from 'node:path';
import {TextDecoder} from 'node:util';
import {fileURLToPath, pathToFileURL} from 'node:url';

import {writeOpenApiAtomicFile} from './lib/openapi-safe-file.mjs';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

export const OPENAPI_CARGO_LOCK_PIN_SCHEMA =
  'iroha.openapi.cargo-lock.v1';
export const OPENAPI_CARGO_LOCK_PROVISION_SCHEMA =
  'iroha.openapi.cargo-lock.provision.v1';
export const OPENAPI_CARGO_LOCK_PIN_OWNER_SCHEMA =
  'iroha.openapi.cargo-lock-pin.owner.v1';
export const OPENAPI_CARGO_LOCK_PATH = 'Cargo.lock';
export const OPENAPI_CARGO_LOCK_PIN_PATH =
  'release/openapi-cargo-lock-v1.txt';
export const OPENAPI_CARGO_LOCK_MAX_BYTES = 16 * 1024 * 1024;
export const OPENAPI_CARGO_LOCK_PIN_MAX_BYTES = 1024;

const IO_CHUNK_BYTES = 64 * 1024;
const GIT_MAX_BUFFER_BYTES = 1024 * 1024;
const SHA256_HEX = /^[0-9a-f]{64}$/;
const defaultRepoRoot = path.resolve(__dirname, '..', '..', '..');

/**
 * Remove ambient Git routing/configuration before repository-policy reads.
 */
export function isolateGitRepositoryEnvironment(environment = process.env) {
  if (environment === null || typeof environment !== 'object') {
    throw new TypeError('Git environment must be an object');
  }
  const isolated = {...environment};
  for (const name of Object.keys(isolated)) {
    if (name.startsWith('GIT_')) {
      delete isolated[name];
    }
  }
  Object.assign(isolated, {
    GIT_OPTIONAL_LOCKS: '0',
    GIT_NO_LAZY_FETCH: '1',
    GIT_NO_REPLACE_OBJECTS: '1',
    GIT_CONFIG_NOSYSTEM: '1',
    GIT_CONFIG_GLOBAL: '/dev/null',
    GIT_CONFIG_COUNT: '2',
    GIT_CONFIG_KEY_0: 'core.hooksPath',
    GIT_CONFIG_VALUE_0: '/dev/null',
    GIT_CONFIG_KEY_1: 'core.fsmonitor',
    GIT_CONFIG_VALUE_1: 'false',
  });
  return isolated;
}

/**
 * Parse the canonical command-line surface.
 */
export function parseArgs(argv) {
  if (!Array.isArray(argv)) {
    throw new TypeError('provisioner arguments must be an array');
  }
  if (argv.length === 0) {
    throw new Error('OpenAPI Cargo.lock owner requires a provision or pin command');
  }
  const [command, ...arguments_] = argv;
  if (command === 'provision') {
    return parseProvisionArgs(arguments_);
  }
  if (command === 'pin') {
    return parsePinOwnerArgs(arguments_);
  }
  throw new Error(`unknown OpenAPI Cargo.lock owner command: ${String(command)}`);
}

function parseProvisionArgs(argv) {
  let sourcePath;
  for (const argument of argv) {
    if (typeof argument !== 'string') {
      throw new TypeError('provisioner arguments must be strings');
    }
    if (argument.startsWith('--source=')) {
      if (sourcePath !== undefined) {
        throw new Error('--source may be supplied only once');
      }
      sourcePath = argument.slice('--source='.length);
      if (sourcePath.length === 0) {
        throw new Error('--source must not be empty');
      }
      if (!path.isAbsolute(sourcePath) || path.resolve(sourcePath) !== sourcePath) {
        throw new Error('--source must be an absolute canonical path');
      }
      continue;
    }
    throw new Error(`unknown provision-openapi-cargo-lock option: ${argument}`);
  }
  return {command: 'provision', sourcePath};
}

function parsePinOwnerArgs(argv) {
  let sourcePath;
  let outputPath;
  let checkPath;
  for (const argument of argv) {
    if (typeof argument !== 'string') {
      throw new TypeError('pin-owner arguments must be strings');
    }
    if (argument.startsWith('--source=')) {
      if (sourcePath !== undefined) {
        throw new Error('pin owner accepts --source only once');
      }
      sourcePath = parseAbsolutePinOwnerPath(
        argument.slice('--source='.length),
        '--source',
      );
      continue;
    }
    if (argument.startsWith('--output=')) {
      if (outputPath !== undefined) {
        throw new Error('pin owner accepts --output only once');
      }
      outputPath = parseAbsolutePinOwnerPath(
        argument.slice('--output='.length),
        '--output',
      );
      continue;
    }
    if (argument.startsWith('--check=')) {
      if (checkPath !== undefined) {
        throw new Error('pin owner accepts --check only once');
      }
      checkPath = parseAbsolutePinOwnerPath(
        argument.slice('--check='.length),
        '--check',
      );
      continue;
    }
    throw new Error(`unknown OpenAPI Cargo.lock pin-owner option: ${argument}`);
  }
  if (sourcePath === undefined) {
    throw new Error('pin owner requires explicit --source=/absolute/Cargo.lock');
  }
  if ((outputPath === undefined) === (checkPath === undefined)) {
    throw new Error('pin owner requires exactly one of --output or --check');
  }
  if ((outputPath ?? checkPath) === sourcePath) {
    throw new Error('pin owner source and destination paths must not alias');
  }
  return {command: 'pin', sourcePath, outputPath, checkPath};
}

function parseAbsolutePinOwnerPath(value, option) {
  if (
    !value ||
    value.trim() !== value ||
    value.startsWith('-') ||
    !path.isAbsolute(value) ||
    path.resolve(value) !== value ||
    path.dirname(value) === value ||
    path.dirname(value) === path.parse(value).root
  ) {
    throw new Error(`${option} requires an absolute normalized file path`);
  }
  return value;
}

/** Parse the tracked, canonical V1 lock pin. */
export function parseOpenApiCargoLockPin(value) {
  const bytes = toBuffer(value, 'OpenAPI Cargo.lock pin');
  if (bytes.length === 0 || bytes.length > OPENAPI_CARGO_LOCK_PIN_MAX_BYTES) {
    throw new Error(
      `OpenAPI Cargo.lock pin must contain 1..${OPENAPI_CARGO_LOCK_PIN_MAX_BYTES} bytes`,
    );
  }
  let text;
  try {
    text = new TextDecoder('utf-8', {fatal: true}).decode(bytes);
  } catch (error) {
    throw new Error(
      `OpenAPI Cargo.lock pin must be UTF-8: ${error?.message ?? error}`,
    );
  }
  if (text.includes('\r') || !text.endsWith('\n')) {
    throw new Error(
      'OpenAPI Cargo.lock pin must use LF endings and one final newline',
    );
  }
  const lines = text.slice(0, -1).split('\n');
  if (lines.length !== 3 || lines[0] !== OPENAPI_CARGO_LOCK_PIN_SCHEMA) {
    throw new Error('OpenAPI Cargo.lock pin has an invalid schema');
  }
  const byteMatch = /^bytes=([1-9][0-9]*)$/.exec(lines[1]);
  const digestMatch = /^sha256_hex=([0-9a-f]{64})$/.exec(lines[2]);
  if (!byteMatch) {
    throw new Error('OpenAPI Cargo.lock pin has an invalid bytes field');
  }
  if (!digestMatch || !SHA256_HEX.test(digestMatch[1])) {
    throw new Error('OpenAPI Cargo.lock pin has an invalid SHA-256 field');
  }
  const expectedBytes = Number(byteMatch[1]);
  if (
    !Number.isSafeInteger(expectedBytes) ||
    expectedBytes <= 0 ||
    expectedBytes > OPENAPI_CARGO_LOCK_MAX_BYTES
  ) {
    throw new Error(
      `OpenAPI Cargo.lock pin bytes must be within 1..${OPENAPI_CARGO_LOCK_MAX_BYTES}`,
    );
  }
  if (digestMatch[1] === '0'.repeat(64)) {
    throw new Error('OpenAPI Cargo.lock pin SHA-256 must be nonzero');
  }
  return {
    schema: lines[0],
    bytes: expectedBytes,
    sha256Hex: digestMatch[1],
  };
}

/** Validate exact Cargo.lock bytes against one canonical V1 pin. */
export function validateOpenApiCargoLockBytes(value, pin) {
  const bytes = toBuffer(value, 'OpenAPI Cargo.lock');
  if (
    !pin ||
    pin.schema !== OPENAPI_CARGO_LOCK_PIN_SCHEMA ||
    !Number.isSafeInteger(pin.bytes) ||
    pin.bytes <= 0 ||
    pin.bytes > OPENAPI_CARGO_LOCK_MAX_BYTES ||
    !SHA256_HEX.test(pin.sha256Hex) ||
    pin.sha256Hex === '0'.repeat(64)
  ) {
    throw new Error('OpenAPI Cargo.lock validation requires one canonical V1 pin');
  }
  if (bytes.length === 0) {
    throw new Error('OpenAPI Cargo.lock must not be empty');
  }
  if (bytes.length > OPENAPI_CARGO_LOCK_MAX_BYTES) {
    throw new Error(
      `OpenAPI Cargo.lock exceeds the ${OPENAPI_CARGO_LOCK_MAX_BYTES}-byte limit`,
    );
  }
  if (bytes.length !== pin.bytes) {
    throw new Error(
      `OpenAPI Cargo.lock has ${bytes.length} bytes; expected exactly ${pin.bytes}`,
    );
  }
  const digest = sha256Hex(bytes);
  if (digest !== pin.sha256Hex) {
    throw new Error(
      `OpenAPI Cargo.lock SHA-256 ${digest} does not match pinned ${pin.sha256Hex}`,
    );
  }
  return bytes;
}

/** Encode the sole canonical V1 pin for explicit Cargo.lock bytes. */
export function encodeOpenApiCargoLockPin(value) {
  const bytes = toBuffer(value, 'OpenAPI Cargo.lock');
  if (bytes.length === 0) {
    throw new Error('OpenAPI Cargo.lock must not be empty');
  }
  if (bytes.length > OPENAPI_CARGO_LOCK_MAX_BYTES) {
    throw new Error(
      `OpenAPI Cargo.lock exceeds the ${OPENAPI_CARGO_LOCK_MAX_BYTES}-byte limit`,
    );
  }
  return Buffer.from(
    `${OPENAPI_CARGO_LOCK_PIN_SCHEMA}\n` +
      `bytes=${bytes.length}\n` +
      `sha256_hex=${sha256Hex(bytes)}\n`,
    'utf8',
  );
}

/**
 * Read one stable regular non-executable file without following links.
 *
 * `beforeOpen` and `afterRead` are injectable race hooks used by focused tests.
 */
export async function readOpenApiCargoLockStable(
  filePath,
  {
    label = 'OpenAPI Cargo.lock',
    maxBytes = OPENAPI_CARGO_LOCK_MAX_BYTES,
    allowEmpty = false,
    beforeOpen,
    afterRead,
  } = {},
) {
  if (!path.isAbsolute(filePath) || path.resolve(filePath) !== filePath) {
    throw new Error(`${label} path must be absolute and normalized`);
  }
  if (
    !Number.isSafeInteger(maxBytes) ||
    maxBytes <= 0 ||
    maxBytes > OPENAPI_CARGO_LOCK_MAX_BYTES
  ) {
    throw new TypeError(
      `${label} maxBytes must be within 1..${OPENAPI_CARGO_LOCK_MAX_BYTES}`,
    );
  }
  for (const [name, hook] of Object.entries({beforeOpen, afterRead})) {
    if (hook !== undefined && typeof hook !== 'function') {
      throw new TypeError(`${label} ${name} must be a function`);
    }
  }

  await requireCanonicalParent(filePath, label);
  const pathBefore = await inspectStableFile(filePath, {
    label,
    maxBytes,
    allowEmpty,
  });
  if (beforeOpen) {
    await beforeOpen({filePath});
  }
  const flags =
    fsConstants.O_RDONLY |
    (typeof fsConstants.O_NOFOLLOW === 'number'
      ? fsConstants.O_NOFOLLOW
      : 0) |
    (typeof fsConstants.O_NONBLOCK === 'number'
      ? fsConstants.O_NONBLOCK
      : 0);
  let handle;
  try {
    handle = await open(filePath, flags);
  } catch (error) {
    throw withCode(
      new Error(
        `failed to open ${label} ${filePath} without following links: ${error?.message ?? error}`,
        {cause: error},
      ),
      error?.code,
    );
  }

  try {
    const openedBefore = await handle.stat({bigint: true});
    validateStableMetadata(openedBefore, {
      label,
      filePath,
      maxBytes,
      allowEmpty,
    });
    requireSameIdentity(openedBefore, pathBefore, label, filePath);

    const chunks = [];
    let offset = 0;
    while (offset <= maxBytes) {
      const remaining = maxBytes + 1 - offset;
      if (remaining === 0) {
        break;
      }
      const chunk = Buffer.allocUnsafe(
        Math.min(IO_CHUNK_BYTES, remaining),
      );
      const {bytesRead} = await handle.read(
        chunk,
        0,
        chunk.length,
        offset,
      );
      if (bytesRead === 0) {
        break;
      }
      chunks.push(chunk.subarray(0, bytesRead));
      offset += bytesRead;
    }
    if (offset > maxBytes) {
      throw new Error(`${label} ${filePath} exceeds the ${maxBytes}-byte limit`);
    }
    if (afterRead) {
      await afterRead({filePath});
    }

    const openedAfter = await handle.stat({bigint: true});
    const pathAfter = await inspectStableFile(filePath, {
      label,
      maxBytes,
      allowEmpty,
    });
    await requireCanonicalParent(filePath, label);
    if (
      !sameStableState(openedBefore, openedAfter) ||
      !sameStableState(openedAfter, pathAfter)
    ) {
      throw new Error(`${label} ${filePath} was replaced or mutated while read`);
    }
    const bytes = Buffer.concat(chunks, offset);
    if (BigInt(bytes.length) !== openedAfter.size) {
      throw new Error(`${label} ${filePath} changed length while read`);
    }
    return {
      bytes,
      filePath,
      state: openedAfter,
    };
  } finally {
    await handle.close();
  }
}

/**
 * Verify the tracked Git policy for the root lock and its derived pin.
 */
export async function validateOpenApiCargoLockGitPolicy(repoRoot) {
  const root = await requireCanonicalRepoRoot(repoRoot);
  const topLevel = decodeGitLine(
    await gitBytes(root, ['rev-parse', '--show-toplevel']),
    'git repository root',
  );
  let canonicalTopLevel;
  try {
    canonicalTopLevel = await realpath(topLevel);
  } catch (error) {
    throw new Error(
      `failed to resolve Git repository root ${topLevel}: ${error?.message ?? error}`,
    );
  }
  if (canonicalTopLevel !== root) {
    throw new Error(
      `OpenAPI Cargo.lock repository root ${root} does not match Git root ${canonicalTopLevel}`,
    );
  }

  const [indexEntry, headEntry, pinIndexEntry, pinHeadEntry] =
    await Promise.all([
      gitBytes(root, [
        'ls-files',
        '--stage',
        '--',
        OPENAPI_CARGO_LOCK_PATH,
      ]),
      gitBytes(root, [
        'ls-tree',
        'HEAD',
        '--',
        OPENAPI_CARGO_LOCK_PATH,
      ]),
      gitBytes(root, [
        'ls-files',
        '--stage',
        '--',
        OPENAPI_CARGO_LOCK_PIN_PATH,
      ]),
      gitBytes(root, [
        'ls-tree',
        'HEAD',
        '--',
        OPENAPI_CARGO_LOCK_PIN_PATH,
      ]),
    ]);
  const lockIndexOid = parseTrackedIndexEntry(
    indexEntry,
    OPENAPI_CARGO_LOCK_PATH,
    'OpenAPI root Cargo.lock',
  );
  const lockHeadOid = parseTrackedTreeEntry(
    headEntry,
    OPENAPI_CARGO_LOCK_PATH,
    'OpenAPI root Cargo.lock',
  );
  if (lockIndexOid !== lockHeadOid) {
    throw new Error(
      'OpenAPI root Cargo.lock index and HEAD entries must reference the same blob',
    );
  }
  const escapedPin = escapeRegExp(OPENAPI_CARGO_LOCK_PIN_PATH);
  const pinIndexMatch = new RegExp(
    `^100644 ([0-9a-f]{40}) 0\\t${escapedPin}\\n$`,
  ).exec(pinIndexEntry.toString('utf8'));
  const pinHeadMatch = new RegExp(
    `^100644 blob ([0-9a-f]{40})\\t${escapedPin}\\n$`,
  ).exec(pinHeadEntry.toString('utf8'));
  if (
    !pinIndexMatch ||
    !pinHeadMatch ||
    /^0{40}$/.test(pinIndexMatch[1]) ||
    /^0{40}$/.test(pinHeadMatch[1])
  ) {
    throw new Error(
      'OpenAPI Cargo.lock V1 pin must be one tracked non-executable HEAD file',
    );
  }
  if (pinIndexMatch[1] !== pinHeadMatch[1]) {
    throw new Error(
      'OpenAPI Cargo.lock V1 pin index and HEAD entries must reference the same blob',
    );
  }
  const [committedLock, workingLock, committedPin, workingPin] = await Promise.all([
    gitBytes(root, ['cat-file', 'blob', lockHeadOid]),
    readOpenApiCargoLockStable(path.join(root, OPENAPI_CARGO_LOCK_PATH), {
      label: 'tracked root OpenAPI Cargo.lock',
    }),
    gitBytes(root, ['cat-file', 'blob', pinHeadMatch[1]]),
    readOpenApiCargoLockStable(
      path.join(root, OPENAPI_CARGO_LOCK_PIN_PATH),
      {
        label: 'tracked OpenAPI Cargo.lock V1 pin',
        maxBytes: OPENAPI_CARGO_LOCK_PIN_MAX_BYTES,
      },
    ),
  ]);
  if (!committedLock.equals(workingLock.bytes)) {
    throw new Error(
      'OpenAPI root Cargo.lock working file must exactly match its HEAD blob',
    );
  }
  if (!committedPin.equals(workingPin.bytes)) {
    throw new Error(
      'OpenAPI Cargo.lock V1 pin working file must exactly match its HEAD blob',
    );
  }
  const pin = parseOpenApiCargoLockPin(committedPin);
  validateOpenApiCargoLockBytes(committedLock, pin);
  return {root, lockSnapshot: workingLock, pinSnapshot: workingPin, pin};
}

/**
 * Verify the exact tracked root lock and any explicit comparison source.
 */
export async function provisionOpenApiCargoLock({
  repoRoot = defaultRepoRoot,
  sourcePath,
  beforeVerify,
} = {}) {
  if (beforeVerify !== undefined && typeof beforeVerify !== 'function') {
    throw new TypeError('beforeVerify must be a function');
  }
  const policy = await validateOpenApiCargoLockGitPolicy(repoRoot);
  let comparison;
  if (sourcePath !== undefined) {
    const candidatePath = await requireCanonicalSourcePath(sourcePath);
    comparison = await readOpenApiCargoLockStable(candidatePath, {
      label: 'comparison OpenAPI Cargo.lock candidate',
    });
    validateOpenApiCargoLockBytes(comparison.bytes, policy.pin);
    if (!comparison.bytes.equals(policy.lockSnapshot.bytes)) {
      throw new Error(
        'comparison OpenAPI Cargo.lock must be byte-identical to the tracked root authority',
      );
    }
  }
  if (beforeVerify) {
    await beforeVerify({
      sourcePath: comparison?.filePath,
      trackedPath: policy.lockSnapshot.filePath,
    });
  }
  await assertOpenApiCargoLockSnapshotStable(policy.lockSnapshot);
  await assertOpenApiCargoLockSnapshotStable(policy.pinSnapshot);
  if (comparison) {
    await assertOpenApiCargoLockSnapshotStable(comparison);
  }
  await validateOpenApiCargoLockGitPolicy(policy.root);
  return provisionSummary('verified', 'tracked', policy.pin);
}

/**
 * Derive or verify the canonical V1 pin without editing Cargo.lock or the
 * tracked release pin. Output is restricted to external staging paths.
 */
export async function generateOpenApiCargoLockPin({
  sourcePath,
  outputPath,
  checkPath,
  repoRoot = defaultRepoRoot,
  beforePublish,
}) {
  requirePinOwnerFilePath(sourcePath, 'pin-owner source');
  if ((outputPath === undefined) === (checkPath === undefined)) {
    throw new Error('pin owner requires exactly one output or check path');
  }
  if (beforePublish !== undefined && typeof beforePublish !== 'function') {
    throw new TypeError('pin-owner beforePublish must be a function');
  }
  const root = await requireCanonicalRepoRoot(repoRoot);
  const canonicalSource = await requireCanonicalSourcePath(sourcePath);
  const source = await readOpenApiCargoLockStable(canonicalSource, {
    label: 'pin-owner Cargo.lock source',
  });
  const pinBytes = encodeOpenApiCargoLockPin(source.bytes);
  const pin = parseOpenApiCargoLockPin(pinBytes);

  if (beforePublish) {
    await beforePublish({sourcePath, outputPath, checkPath, pinBytes});
  }
  await assertOpenApiCargoLockSnapshotStable(source);

  if (checkPath !== undefined) {
    requirePinOwnerFilePath(checkPath, 'pin-owner check');
    if (checkPath === canonicalSource) {
      throw new Error('pin owner source and check paths must not alias');
    }
    const expected = await readOpenApiCargoLockStable(checkPath, {
      label: 'tracked OpenAPI Cargo.lock V1 pin',
      maxBytes: OPENAPI_CARGO_LOCK_PIN_MAX_BYTES,
    });
    parseOpenApiCargoLockPin(expected.bytes);
    if (!expected.bytes.equals(pinBytes)) {
      throw new Error(
        `OpenAPI Cargo.lock V1 pin ${checkPath} is stale for explicit source ${canonicalSource}`,
      );
    }
    await assertOpenApiCargoLockSnapshotStable(source);
    await assertOpenApiCargoLockSnapshotStable(expected);
    return pinOwnerSummary(
      'verified',
      canonicalSource,
      checkPath,
      pin,
      pinBytes.length,
    );
  }

  requirePinOwnerFilePath(outputPath, 'pin-owner output');
  if (outputPath === canonicalSource) {
    throw new Error('pin owner source and output paths must not alias');
  }
  if (isWithin(root, outputPath)) {
    throw new Error(
      'pin-owner --output must be an external staging path outside the repository',
    );
  }
  await requireCanonicalDirectory(
    path.dirname(outputPath),
    'pin-owner output parent',
  );
  await writeOpenApiAtomicFile(outputPath, pinBytes, {
    label: 'staged OpenAPI Cargo.lock V1 pin',
  });
  const staged = await readOpenApiCargoLockStable(outputPath, {
    label: 'staged OpenAPI Cargo.lock V1 pin',
    maxBytes: OPENAPI_CARGO_LOCK_PIN_MAX_BYTES,
  });
  if (!staged.bytes.equals(pinBytes)) {
    throw new Error('staged OpenAPI Cargo.lock V1 pin changed during publication');
  }
  await assertOpenApiCargoLockSnapshotStable(source);
  await assertOpenApiCargoLockSnapshotStable(staged);
  return pinOwnerSummary(
    'staged',
    canonicalSource,
    outputPath,
    pin,
    pinBytes.length,
  );
}

function requirePinOwnerFilePath(value, label) {
  if (
    typeof value !== 'string' ||
    !path.isAbsolute(value) ||
    path.resolve(value) !== value ||
    path.dirname(value) === value ||
    path.dirname(value) === path.parse(value).root
  ) {
    throw new Error(`${label} must be an absolute normalized file path`);
  }
}

async function requireCanonicalDirectory(directory, label) {
  const canonical = await realpath(directory).catch((error) => {
    throw withCode(
      new Error(
        `failed to resolve ${label} ${directory}: ${error?.message ?? error}`,
        {cause: error},
      ),
      error?.code,
    );
  });
  if (canonical !== directory) {
    throw new Error(`${label} must not contain symbolic links`);
  }
  const metadata = await lstat(canonical, {bigint: true});
  if (metadata.isSymbolicLink() || !metadata.isDirectory()) {
    throw new Error(`${label} must be a real directory`);
  }
  return canonical;
}

async function requireCanonicalRepoRoot(repoRoot) {
  if (typeof repoRoot !== 'string') {
    throw new TypeError('repoRoot must be a string');
  }
  const resolved = path.resolve(repoRoot);
  let canonical;
  try {
    canonical = await realpath(resolved);
  } catch (error) {
    throw new Error(
      `failed to resolve OpenAPI repository root ${resolved}: ${error?.message ?? error}`,
    );
  }
  if (canonical !== resolved) {
    throw new Error(
      `OpenAPI repository root must be canonical: ${resolved} resolves to ${canonical}`,
    );
  }
  const metadata = await lstat(canonical, {bigint: true});
  if (metadata.isSymbolicLink() || !metadata.isDirectory()) {
    throw new Error('OpenAPI repository root must be a real directory');
  }
  return canonical;
}

async function requireCanonicalSourcePath(sourcePath) {
  if (
    typeof sourcePath !== 'string' ||
    !path.isAbsolute(sourcePath) ||
    path.resolve(sourcePath) !== sourcePath
  ) {
    throw new Error(
      'comparison OpenAPI Cargo.lock source must be an absolute canonical path',
    );
  }
  let canonical;
  try {
    canonical = await realpath(sourcePath);
  } catch (error) {
    throw withCode(
      new Error(
        `failed to resolve comparison OpenAPI Cargo.lock source ${sourcePath}: ${error?.message ?? error}`,
        {cause: error},
      ),
      error?.code,
    );
  }
  if (canonical !== sourcePath) {
    throw new Error(
      `comparison OpenAPI Cargo.lock source must be canonical: ${sourcePath} resolves to ${canonical}`,
    );
  }
  return canonical;
}

async function inspectStableFile(
  filePath,
  {label, maxBytes, allowEmpty},
) {
  let metadata;
  try {
    metadata = await lstat(filePath, {bigint: true});
  } catch (error) {
    throw withCode(
      new Error(
        `failed to inspect ${label} ${filePath}: ${error?.message ?? error}`,
        {cause: error},
      ),
      error?.code,
    );
  }
  validateStableMetadata(metadata, {
    label,
    filePath,
    maxBytes,
    allowEmpty,
  });
  return metadata;
}

function validateStableMetadata(
  metadata,
  {label, filePath, maxBytes, allowEmpty},
) {
  if (metadata.isSymbolicLink()) {
    throw new Error(`${label} ${filePath} must not be a symbolic link`);
  }
  if (!metadata.isFile()) {
    throw new Error(`${label} ${filePath} must be a regular file`);
  }
  if (metadata.nlink !== 1n) {
    throw new Error(`${label} ${filePath} must have exactly one hard link`);
  }
  if (
    process.platform !== 'win32' &&
    (metadata.mode & 0o111n) !== 0n
  ) {
    throw new Error(`${label} ${filePath} must not be executable`);
  }
  if (!allowEmpty && metadata.size === 0n) {
    throw new Error(`${label} ${filePath} must not be empty`);
  }
  if (metadata.size > BigInt(maxBytes)) {
    throw new Error(`${label} ${filePath} exceeds the ${maxBytes}-byte limit`);
  }
}

async function requireCanonicalParent(filePath, label) {
  const parent = path.dirname(filePath);
  let canonicalParent;
  try {
    canonicalParent = await realpath(parent);
  } catch (error) {
    throw withCode(
      new Error(
        `failed to resolve ${label} parent ${parent}: ${error?.message ?? error}`,
        {cause: error},
      ),
      error?.code,
    );
  }
  if (canonicalParent !== parent) {
    throw new Error(
      `${label} parent must not contain symbolic links: ${parent} resolves to ${canonicalParent}`,
    );
  }
}

export async function assertOpenApiCargoLockSnapshotStable(snapshot) {
  await requireCanonicalParent(snapshot.filePath, 'OpenAPI Cargo.lock');
  const current = await lstat(snapshot.filePath, {bigint: true});
  if (!sameStableState(snapshot.state, current)) {
    throw new Error(
      `OpenAPI Cargo.lock ${snapshot.filePath} was replaced or mutated after read`,
    );
  }
}

function requireSameIdentity(left, right, label, filePath) {
  if (left.dev !== right.dev || left.ino !== right.ino) {
    throw new Error(`${label} ${filePath} was replaced while opened`);
  }
}

function sameStableState(left, right) {
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

async function gitBytes(repoRoot, arguments_, {allowedExitCodes = [0]} = {}) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn('git', arguments_, {
      cwd: repoRoot,
      env: isolateGitRepositoryEnvironment(),
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    const stdout = [];
    const stderr = [];
    let stdoutBytes = 0;
    let stderrBytes = 0;
    child.stdout.on('data', (chunk) => {
      stdoutBytes += chunk.length;
      if (stdoutBytes > GIT_MAX_BUFFER_BYTES) {
        child.stdout.destroy(
          new Error('Git stdout exceeded the provisioning limit'),
        );
        return;
      }
      stdout.push(chunk);
    });
    child.stderr.on('data', (chunk) => {
      stderrBytes += chunk.length;
      if (stderrBytes > GIT_MAX_BUFFER_BYTES) {
        child.stderr.destroy(
          new Error('Git stderr exceeded the provisioning limit'),
        );
        return;
      }
      stderr.push(chunk);
    });
    child.once('error', (error) => {
      rejectPromise(
        new Error(`failed to execute git: ${error?.message ?? error}`, {
          cause: error,
        }),
      );
    });
    child.once('close', (code) => {
      if (!allowedExitCodes.includes(code)) {
        rejectPromise(
          new Error(
            `git ${arguments_[0]} failed with status ${code}: ${Buffer.concat(stderr).toString('utf8').trim()}`,
          ),
        );
        return;
      }
      resolvePromise(Buffer.concat(stdout));
    });
  });
}

function decodeGitLine(bytes, label) {
  let text;
  try {
    text = new TextDecoder('utf-8', {fatal: true}).decode(bytes);
  } catch (error) {
    throw new Error(`${label} must be UTF-8: ${error?.message ?? error}`);
  }
  if (text.includes('\r') || !text.endsWith('\n')) {
    throw new Error(`${label} must contain one LF-terminated path`);
  }
  const value = text.slice(0, -1);
  if (value.length === 0 || value.includes('\n')) {
    throw new Error(`${label} must contain exactly one path`);
  }
  return value;
}

function parseTrackedIndexEntry(bytes, expectedPath, label) {
  const escapedPath = escapeRegExp(expectedPath);
  const match = new RegExp(
    `^100644 ([0-9a-f]{40}) 0\\t${escapedPath}\\n$`,
  ).exec(bytes.toString('utf8'));
  if (!match || /^0{40}$/.test(match[1])) {
    throw new Error(`${label} must be one stage-zero 100644 Git blob`);
  }
  return match[1];
}

function parseTrackedTreeEntry(bytes, expectedPath, label) {
  const escapedPath = escapeRegExp(expectedPath);
  const match = new RegExp(
    `^100644 blob ([0-9a-f]{40})\\t${escapedPath}\\n$`,
  ).exec(bytes.toString('utf8'));
  if (!match || /^0{40}$/.test(match[1])) {
    throw new Error(`${label} must be one canonical 100644 HEAD blob`);
  }
  return match[1];
}

function provisionSummary(status, source, pin) {
  return {
    schema: OPENAPI_CARGO_LOCK_PROVISION_SCHEMA,
    status,
    source,
    path: OPENAPI_CARGO_LOCK_PATH,
    bytes: pin.bytes,
    sha256_hex: pin.sha256Hex,
  };
}

function pinOwnerSummary(status, source, path_, pin, pinBytes) {
  return {
    schema: OPENAPI_CARGO_LOCK_PIN_OWNER_SCHEMA,
    status,
    source,
    path: path_,
    cargo_lock_bytes: pin.bytes,
    cargo_lock_sha256_hex: pin.sha256Hex,
    pin_bytes: pinBytes,
  };
}

function sha256Hex(bytes) {
  return createHash('sha256').update(bytes).digest('hex');
}

function toBuffer(value, label) {
  if (Buffer.isBuffer(value)) {
    return value;
  }
  if (value instanceof Uint8Array) {
    return Buffer.from(value);
  }
  throw new TypeError(`${label} must be bytes`);
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

function isWithin(parent, child) {
  const relative = path.relative(parent, child);
  return relative === '' || (
    relative !== '..' &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative)
  );
}

function withCode(error, code) {
  if (code !== undefined) {
    error.code = code;
  }
  return error;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const summary = options.command === 'provision'
    ? await provisionOpenApiCargoLock({sourcePath: options.sourcePath})
    : await generateOpenApiCargoLockPin(options);
  process.stdout.write(`${JSON.stringify(summary)}\n`);
}

if (
  process.argv[1] &&
  pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url
) {
  main().catch((error) => {
    console.error(`error: ${error?.message ?? error}`);
    process.exitCode = 1;
  });
}
