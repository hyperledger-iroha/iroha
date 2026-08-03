#!/usr/bin/env node
// SPDX-License-Identifier: Apache-2.0

/**
 * Provision the ignored root Cargo.lock used by the Torii OpenAPI release gate.
 *
 * The canonical operator interface is:
 *
 *   node tools/openapi/scripts/provision-openapi-cargo-lock.mjs provision
 *   node tools/openapi/scripts/provision-openapi-cargo-lock.mjs \
 *     provision --source=/absolute/canonical/Cargo.lock
 *   node tools/openapi/scripts/provision-openapi-cargo-lock.mjs \
 *     pin --source=/absolute/canonical/Cargo.lock \
 *     --output=/absolute/external/staging/openapi-cargo-lock-v1.txt
 *
 * An exact existing root lock is reused. Otherwise an operator source is
 * preferred; without one, Cargo generates a candidate only at an isolated
 * temporary `--lockfile-path`. Every candidate must match the tracked V1 pin
 * before an absent-target atomic installation.
 */
import {spawn} from 'node:child_process';
import {createHash, randomBytes} from 'node:crypto';
import {constants as fsConstants, readFileSync} from 'node:fs';
import {
  link,
  lstat,
  mkdtemp,
  open,
  realpath,
  rm,
  unlink,
} from 'node:fs/promises';
import {tmpdir} from 'node:os';
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
const sourceBoundPin = parseOpenApiCargoLockPin(
  readFileSync(path.join(defaultRepoRoot, OPENAPI_CARGO_LOCK_PIN_PATH)),
);

// Release-gate exports are derived from the tracked pin at module load and are
// not independent size or digest authorities.
export const OPENAPI_CARGO_LOCK_EXPECTED_BYTES = sourceBoundPin.bytes;
export const OPENAPI_CARGO_LOCK_EXPECTED_SHA256_HEX =
  sourceBoundPin.sha256Hex;

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
 * `afterRead` is an injectable race hook used only by focused tests.
 */
export async function readOpenApiCargoLockStable(
  filePath,
  {
    label = 'OpenAPI Cargo.lock',
    maxBytes = OPENAPI_CARGO_LOCK_MAX_BYTES,
    allowEmpty = false,
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
  if (afterRead !== undefined && typeof afterRead !== 'function') {
    throw new TypeError(`${label} afterRead must be a function`);
  }

  await requireCanonicalParent(filePath, label);
  const pathBefore = await inspectStableFile(filePath, {
    label,
    maxBytes,
    allowEmpty,
  });
  const flags =
    fsConstants.O_RDONLY |
    (typeof fsConstants.O_NOFOLLOW === 'number'
      ? fsConstants.O_NOFOLLOW
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
 * Verify the ignored/untracked Git policy for the root lock and the tracked pin.
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

  let ignored;
  try {
    ignored = await gitBytes(
      root,
      ['check-ignore', '--no-index', '--', OPENAPI_CARGO_LOCK_PATH],
      {allowedExitCodes: [0, 1]},
    );
  } catch (error) {
    throw new Error(
      `failed to verify Cargo.lock ignore policy: ${error?.message ?? error}`,
    );
  }
  if (!ignored.equals(Buffer.from(`${OPENAPI_CARGO_LOCK_PATH}\n`, 'utf8'))) {
    throw new Error(
      'OpenAPI root Cargo.lock must be ignored by repository policy',
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
  if (indexEntry.length !== 0 || headEntry.length !== 0) {
    throw new Error(
      'OpenAPI root Cargo.lock must remain untracked in the index and HEAD',
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
  const [committedPin, workingPin] = await Promise.all([
    gitBytes(root, ['cat-file', 'blob', pinHeadMatch[1]]),
    readOpenApiCargoLockStable(
      path.join(root, OPENAPI_CARGO_LOCK_PIN_PATH),
      {
        label: 'tracked OpenAPI Cargo.lock V1 pin',
        maxBytes: OPENAPI_CARGO_LOCK_PIN_MAX_BYTES,
      },
    ),
  ]);
  if (!committedPin.equals(workingPin.bytes)) {
    throw new Error(
      'OpenAPI Cargo.lock V1 pin working file must exactly match its HEAD blob',
    );
  }
  parseOpenApiCargoLockPin(committedPin);
  return root;
}

/**
 * Generate a candidate using Cargo's unstable external lockfile path.
 */
export async function generateOpenApiCargoLockCandidate({
  repoRoot,
  candidatePath,
  cargoExecutable = 'cargo',
}) {
  if (typeof cargoExecutable !== 'string' || cargoExecutable.length === 0) {
    throw new TypeError('cargoExecutable must be a nonempty string');
  }
  const arguments_ = [
    '-Z',
    'unstable-options',
    'generate-lockfile',
    '--manifest-path',
    path.join(repoRoot, 'Cargo.toml'),
    '--lockfile-path',
    candidatePath,
  ];
  await spawnChecked(cargoExecutable, arguments_, {
    cwd: repoRoot,
    env: {
      ...process.env,
      RUSTC_BOOTSTRAP: '1',
    },
  });
}

/**
 * Provision or reuse the exact ignored root lock.
 */
export async function provisionOpenApiCargoLock({
  repoRoot = defaultRepoRoot,
  sourcePath,
  generateCandidate = generateOpenApiCargoLockCandidate,
  beforeInstall,
} = {}) {
  if (typeof generateCandidate !== 'function') {
    throw new TypeError('generateCandidate must be a function');
  }
  if (beforeInstall !== undefined && typeof beforeInstall !== 'function') {
    throw new TypeError('beforeInstall must be a function');
  }
  const root = await validateOpenApiCargoLockGitPolicy(repoRoot);
  const pinPath = path.join(root, OPENAPI_CARGO_LOCK_PIN_PATH);
  const pinSnapshot = await readOpenApiCargoLockStable(pinPath, {
    label: 'tracked OpenAPI Cargo.lock V1 pin',
    maxBytes: OPENAPI_CARGO_LOCK_PIN_MAX_BYTES,
  });
  const pin = parseOpenApiCargoLockPin(pinSnapshot.bytes);
  const targetPath = path.join(root, OPENAPI_CARGO_LOCK_PATH);
  const existing = await readOptionalStable(targetPath, {
    label: 'ignored root OpenAPI Cargo.lock',
  });
  if (existing) {
    validateOpenApiCargoLockBytes(existing.bytes, pin);
    await validateOpenApiCargoLockGitPolicy(root);
    await assertOpenApiCargoLockSnapshotStable(pinSnapshot);
    await assertOpenApiCargoLockSnapshotStable(existing);
    return provisionSummary('reused', 'existing', pin);
  }

  let isolatedDirectory;
  let candidatePath;
  let sourceKind;
  try {
    if (sourcePath !== undefined) {
      candidatePath = await requireCanonicalSourcePath(sourcePath);
      sourceKind = 'operator';
    } else {
      const canonicalTempRoot = await realpath(tmpdir());
      isolatedDirectory = await mkdtemp(
        path.join(canonicalTempRoot, 'iroha-openapi-cargo-lock-'),
      );
      if (isWithin(root, isolatedDirectory)) {
        throw new Error(
          'generated OpenAPI Cargo.lock directory must be outside the repository',
        );
      }
      candidatePath = path.join(
        isolatedDirectory,
        OPENAPI_CARGO_LOCK_PATH,
      );
      await generateCandidate({
        repoRoot: root,
        candidatePath,
      });
      await assertTargetAbsent(targetPath);
      sourceKind = 'generated';
    }

    const candidate = await readOpenApiCargoLockStable(candidatePath, {
      label: `${sourceKind} OpenAPI Cargo.lock candidate`,
    });
    validateOpenApiCargoLockBytes(candidate.bytes, pin);
    if (beforeInstall) {
      await beforeInstall({
        candidatePath,
        targetPath,
        sourceKind,
      });
    }
    await assertOpenApiCargoLockSnapshotStable(candidate);
    await assertTargetAbsent(targetPath);
    await installAbsentAtomic(targetPath, candidate.bytes);

    const installed = await readOpenApiCargoLockStable(targetPath, {
      label: 'installed ignored root OpenAPI Cargo.lock',
    });
    validateOpenApiCargoLockBytes(installed.bytes, pin);
    await validateOpenApiCargoLockGitPolicy(root);
    await assertOpenApiCargoLockSnapshotStable(pinSnapshot);
    await assertOpenApiCargoLockSnapshotStable(candidate);
    await assertOpenApiCargoLockSnapshotStable(installed);
    return provisionSummary('installed', sourceKind, pin);
  } finally {
    if (isolatedDirectory) {
      await rm(isolatedDirectory, {recursive: true, force: true});
    }
  }
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
      'operator OpenAPI Cargo.lock source must be an absolute canonical path',
    );
  }
  let canonical;
  try {
    canonical = await realpath(sourcePath);
  } catch (error) {
    throw withCode(
      new Error(
        `failed to resolve operator OpenAPI Cargo.lock source ${sourcePath}: ${error?.message ?? error}`,
        {cause: error},
      ),
      error?.code,
    );
  }
  if (canonical !== sourcePath) {
    throw new Error(
      `operator OpenAPI Cargo.lock source must be canonical: ${sourcePath} resolves to ${canonical}`,
    );
  }
  return canonical;
}

async function readOptionalStable(filePath, options) {
  try {
    return await readOpenApiCargoLockStable(filePath, options);
  } catch (error) {
    if (error?.code === 'ENOENT') {
      return null;
    }
    throw error;
  }
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

async function assertTargetAbsent(targetPath) {
  try {
    await lstat(targetPath);
  } catch (error) {
    if (error?.code === 'ENOENT') {
      return;
    }
    throw error;
  }
  throw new Error(
    `ignored root OpenAPI Cargo.lock ${targetPath} appeared or was replaced during provisioning`,
  );
}

async function installAbsentAtomic(targetPath, bytes) {
  await requireCanonicalParent(targetPath, 'OpenAPI Cargo.lock installation');
  await assertTargetAbsent(targetPath);
  const parent = path.dirname(targetPath);
  const temporaryPath = path.join(
    parent,
    `.openapi-cargo-lock-${process.pid}-${randomBytes(12).toString('hex')}.tmp`,
  );
  const flags =
    fsConstants.O_WRONLY |
    fsConstants.O_CREAT |
    fsConstants.O_EXCL |
    (typeof fsConstants.O_NOFOLLOW === 'number'
      ? fsConstants.O_NOFOLLOW
      : 0);
  let handle;
  let temporaryExists = false;
  let targetInstalled = false;
  let staged;
  let succeeded = false;
  try {
    handle = await open(temporaryPath, flags, 0o600);
    temporaryExists = true;
    let offset = 0;
    while (offset < bytes.length) {
      const {bytesWritten} = await handle.write(
        bytes,
        offset,
        bytes.length - offset,
        offset,
      );
      if (bytesWritten <= 0) {
        throw new Error('OpenAPI Cargo.lock atomic write made no progress');
      }
      offset += bytesWritten;
    }
    await handle.sync();
    await handle.chmod(0o644);
    staged = await handle.stat({bigint: true});
    validateStableMetadata(staged, {
      label: 'staged OpenAPI Cargo.lock',
      filePath: temporaryPath,
      maxBytes: OPENAPI_CARGO_LOCK_MAX_BYTES,
      allowEmpty: false,
    });
    if (staged.size !== BigInt(bytes.length)) {
      throw new Error('staged OpenAPI Cargo.lock has an unexpected size');
    }
    await handle.close();
    handle = null;

    await assertTargetAbsent(targetPath);
    try {
      await link(temporaryPath, targetPath);
    } catch (error) {
      if (error?.code === 'EEXIST') {
        throw new Error(
          `ignored root OpenAPI Cargo.lock ${targetPath} appeared during atomic installation`,
          {cause: error},
        );
      }
      throw error;
    }
    targetInstalled = true;
    await unlink(temporaryPath);
    temporaryExists = false;

    const installed = await lstat(targetPath, {bigint: true});
    validateStableMetadata(installed, {
      label: 'installed OpenAPI Cargo.lock',
      filePath: targetPath,
      maxBytes: OPENAPI_CARGO_LOCK_MAX_BYTES,
      allowEmpty: false,
    });
    if (
      installed.dev !== staged.dev ||
      installed.ino !== staged.ino ||
      installed.size !== staged.size ||
      (process.platform !== 'win32' &&
        (installed.mode & 0o777n) !== 0o644n)
    ) {
      throw new Error(
        'installed OpenAPI Cargo.lock changed during atomic installation',
      );
    }
    await syncDirectory(parent);
    succeeded = true;
  } finally {
    if (handle) {
      await handle.close().catch(() => {});
    }
    if (temporaryExists) {
      await unlink(temporaryPath).catch(() => {});
    }
    if (!succeeded && targetInstalled && staged) {
      await unlinkIfSameIdentity(targetPath, staged);
    }
  }
}

async function unlinkIfSameIdentity(filePath, expected) {
  try {
    const current = await lstat(filePath, {bigint: true});
    if (current.dev === expected.dev && current.ino === expected.ino) {
      await unlink(filePath);
    }
  } catch (error) {
    if (error?.code !== 'ENOENT') {
      throw error;
    }
  }
}

async function syncDirectory(directory) {
  let handle;
  try {
    handle = await open(directory, fsConstants.O_RDONLY);
    await handle.sync();
  } catch (error) {
    if (
      process.platform === 'win32' &&
      ['EACCES', 'EINVAL', 'ENOTSUP', 'EPERM'].includes(error?.code)
    ) {
      return;
    }
    throw error;
  } finally {
    if (handle) {
      await handle.close().catch(() => {});
    }
  }
}

async function gitBytes(repoRoot, arguments_, {allowedExitCodes = [0]} = {}) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn('git', arguments_, {
      cwd: repoRoot,
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

async function spawnChecked(executable, arguments_, options) {
  return new Promise((resolvePromise, rejectPromise) => {
    const child = spawn(executable, arguments_, {
      ...options,
      stdio: 'inherit',
    });
    child.once('error', (error) => {
      rejectPromise(
        new Error(
          `failed to execute ${executable}: ${error?.message ?? error}`,
          {cause: error},
        ),
      );
    });
    child.once('close', (code, signal) => {
      if (code !== 0) {
        rejectPromise(
          new Error(
            `${executable} OpenAPI Cargo.lock generation failed with status ${String(code)}${signal ? ` and signal ${signal}` : ''}`,
          ),
        );
        return;
      }
      resolvePromise();
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
