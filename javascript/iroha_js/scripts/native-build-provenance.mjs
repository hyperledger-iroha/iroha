import { Buffer } from "node:buffer";
import { createHash, randomUUID } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  closeSync,
  chmodSync,
  copyFileSync,
  constants,
  existsSync,
  fstatSync,
  fsyncSync,
  mkdirSync,
  mkdtempSync,
  lstatSync,
  openSync,
  readlinkSync,
  readdirSync,
  realpathSync,
  readSync,
  renameSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { devNull } from "node:os";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";

export const NATIVE_BUILD_PROVENANCE_FILENAME =
  "iroha_js_host.build-provenance.json";
export const NATIVE_BUILD_EXECUTION_POLICY = "trusted-local-cargo-v1";
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REVISION_PATTERN = /^[0-9a-f]{40}$/u;
const MAX_PROVENANCE_BYTES = 16 * 1024;
const MAX_GIT_INVENTORY_BYTES = 32 * 1024 * 1024;
const MAX_GIT_STATUS_BYTES = 16 * 1024 * 1024;
const MAX_UNTRACKED_FILES = 100_000;
const MAX_UNTRACKED_FILE_BYTES = 64n * 1024n * 1024n;
const MAX_CARGO_LOCK_BYTES = 16n * 1024n * 1024n;
const CARGO_LOCK_PATH = Buffer.from("Cargo.lock", "utf8");
export const NATIVE_BUILD_CARGO_LOCK_ENV =
  "IROHA_JS_CARGO_LOCKFILE_PATH";
const SNAPSHOT_PREFIX = ".iroha-js-source-snapshot-";
const PROVENANCE_PREVIOUS_SUFFIX = ".previous";
const PROVENANCE_RETIRED_PREFIX = `.${NATIVE_BUILD_PROVENANCE_FILENAME}.retired-`;
const SOURCE_TREE_DOMAIN = Buffer.from(
  "iroha.js.native-build-source-tree.v3\0",
  "utf8",
);
const GITLINK_MODE = "160000";
const ALLOWED_TRACKED_MODES = new Set([
  "100644",
  "100755",
  "120000",
  GITLINK_MODE,
]);
const GIT_CONFIGURATION = Object.freeze([
  "-c",
  "core.fsmonitor=false",
  "-c",
  "core.untrackedCache=false",
  "-c",
  "core.excludesFile=",
]);
const FORBIDDEN_GIT_ENVIRONMENT = new Set([
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_COMMON_DIR",
  "GIT_CONFIG",
  "GIT_CONFIG_COUNT",
  "GIT_CONFIG_GLOBAL",
  "GIT_CONFIG_NOSYSTEM",
  "GIT_CONFIG_PARAMETERS",
  "GIT_CONFIG_SYSTEM",
  "GIT_DIR",
  "GIT_GRAFT_FILE",
  "GIT_INDEX_FILE",
  "GIT_OBJECT_DIRECTORY",
  "GIT_REPLACE_REF_BASE",
  "GIT_SHALLOW_FILE",
  "GIT_WORK_TREE",
]);

function assertPlainObject(value, label) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    (Object.getPrototypeOf(value) !== Object.prototype &&
      Object.getPrototypeOf(value) !== null)
  ) {
    throw new TypeError(`${label} must be an object`);
  }
}

function assertExactKeys(value, expected, label) {
  assertPlainObject(value, label);
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (
    actual.length !== wanted.length ||
    actual.some((key, index) => key !== wanted[index])
  ) {
    throw new TypeError(`${label} has unexpected or missing fields`);
  }
}

function assertRegularFile(path, label) {
  const metadata = lstatSync(path);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a regular non-symbolic-link file`);
  }
  return metadata;
}

function assertNoGitSourceOverrides(env) {
  for (const key of Object.keys(env)) {
    if (
      FORBIDDEN_GIT_ENVIRONMENT.has(key) ||
      key.startsWith("GIT_CONFIG_KEY_") ||
      key.startsWith("GIT_CONFIG_VALUE_")
    ) {
      throw new Error(
        `Native build source sealing forbids the Git environment override ${key}.`,
      );
    }
  }
}

function canonicalGitEnvironment(env) {
  const canonical = {};
  for (const [key, value] of Object.entries(env)) {
    if (!key.startsWith("GIT_")) canonical[key] = value;
  }
  return {
    ...canonical,
    GIT_CONFIG_COUNT: "0",
    GIT_CONFIG_GLOBAL: devNull,
    GIT_CONFIG_NOSYSTEM: "1",
    GIT_LITERAL_PATHSPECS: "1",
    GIT_NO_REPLACE_OBJECTS: "1",
    GIT_OPTIONAL_LOCKS: "0",
    GIT_PAGER: "cat",
    GIT_TERMINAL_PROMPT: "0",
    LANG: "C",
    LC_ALL: "C",
  };
}

function runGit(repoRoot, args, { env, run, maxBuffer }) {
  const result = run(
    "git",
    [...GIT_CONFIGURATION, "-C", repoRoot, ...args],
    {
      encoding: null,
      env: canonicalGitEnvironment(env),
      maxBuffer,
    },
  );
  if (result.error || result.status !== 0) {
    throw new Error("Native build source Git provenance could not be determined.");
  }
  return Buffer.isBuffer(result.stdout)
    ? result.stdout
    : Buffer.from(result.stdout ?? "");
}

function readGitRevision(repoRoot, run, env) {
  const stdout = runGit(repoRoot, ["rev-parse", "--verify", "HEAD"], {
    env,
    run,
    maxBuffer: 1024 * 1024,
  });
  const revision = stdout.toString("ascii").trim();
  if (!REVISION_PATTERN.test(revision)) {
    throw new Error("Native build source Git provenance could not be determined.");
  }
  return revision;
}

function readGitStatus(repoRoot, run, env) {
  return runGit(
    repoRoot,
    ["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    { env, run, maxBuffer: MAX_GIT_STATUS_BYTES },
  );
}

function splitNulRecords(stdout, label) {
  if (stdout.length === 0) return [];
  if (stdout.at(-1) !== 0) {
    throw new Error(`${label} is not canonically NUL terminated`);
  }
  const records = [];
  let start = 0;
  for (let index = 0; index < stdout.length; index += 1) {
    if (stdout[index] !== 0) continue;
    const record = Buffer.from(stdout.subarray(start, index));
    if (record.length === 0) {
      throw new Error(`${label} contains an empty record`);
    }
    records.push(record);
    start = index + 1;
  }
  return records;
}

function assertSafeRelativePath(path, label) {
  if (
    path.length === 0 ||
    path[0] === 0x2f ||
    path.at(-1) === 0x2f ||
    path.includes(0) ||
    (process.platform === "win32" && path.includes(0x5c))
  ) {
    throw new Error(`${label} contains an unsafe source path`);
  }
  const components = [];
  let start = 0;
  for (let index = 0; index <= path.length; index += 1) {
    if (index !== path.length && path[index] !== 0x2f) continue;
    components.push(path.subarray(start, index));
    start = index + 1;
  }
  if (
    components.some(
      (component) =>
        component.length === 0 ||
        component.equals(Buffer.from(".")) ||
        component.equals(Buffer.from("..")),
    ) ||
    components[0].equals(Buffer.from(".git"))
  ) {
    throw new Error(`${label} contains an unsafe source path`);
  }
}

function canonicalizeEntries(entries, label) {
  entries.sort((left, right) => Buffer.compare(left.path, right.path));
  for (let index = 0; index < entries.length; index += 1) {
    assertSafeRelativePath(entries[index].path, label);
    if (index > 0 && entries[index - 1].path.equals(entries[index].path)) {
      throw new Error(`${label} contains a duplicate source path`);
    }
  }
  return entries;
}

function readTrackedInventory(repoRoot, run, env) {
  const raw = runGit(repoRoot, ["ls-files", "--stage", "-z", "--"], {
    env,
    run,
    maxBuffer: MAX_GIT_INVENTORY_BYTES,
  });
  const entries = splitNulRecords(raw, "Native build tracked inventory").map(
    (record) => {
      const tab = record.indexOf(0x09);
      if (tab <= 0 || tab === record.length - 1) {
        throw new Error("Native build tracked inventory is malformed");
      }
      const metadata = record.subarray(0, tab).toString("ascii");
      const match = /^(?<mode>[0-9]{6}) (?<object>[0-9a-f]{40}) (?<stage>[0-3])$/u.exec(
        metadata,
      );
      if (
        !match?.groups ||
        !ALLOWED_TRACKED_MODES.has(match.groups.mode) ||
        match.groups.stage !== "0"
      ) {
        throw new Error(
          "Native build tracked inventory has an unsupported mode or unresolved stage",
        );
      }
      return {
        indexMode: match.groups.mode,
        path: Buffer.from(record.subarray(tab + 1)),
      };
    },
  );
  return {
    entries: canonicalizeEntries(entries, "Native build tracked inventory"),
    raw,
  };
}

function readUntrackedInventory(repoRoot, run, env) {
  const raw = runGit(
    repoRoot,
    ["ls-files", "--others", "--exclude-standard", "-z", "--"],
    { env, run, maxBuffer: MAX_GIT_INVENTORY_BYTES },
  );
  const records = splitNulRecords(raw, "Native build untracked inventory");
  if (records.length > MAX_UNTRACKED_FILES) {
    throw new Error("Native build untracked source inventory is too large");
  }
  return {
    entries: canonicalizeEntries(
      records.map((path) => ({ path })),
      "Native build untracked inventory",
    ),
    raw,
  };
}

function appendField(hash, value) {
  const bytes = Buffer.isBuffer(value) ? value : Buffer.from(value, "utf8");
  const length = Buffer.alloc(8);
  length.writeBigUInt64BE(BigInt(bytes.length));
  hash.update(length);
  hash.update(bytes);
}

function appendLength(hash, lengthValue) {
  if (lengthValue < 0n || lengthValue > 0xffff_ffff_ffff_ffffn) {
    throw new Error("Native build source file is outside the supported size bound");
  }
  const length = Buffer.alloc(8);
  length.writeBigUInt64BE(lengthValue);
  hash.update(length);
}

function sameStableIdentity(left, right) {
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

function sameRenamedFileIdentity(left, right) {
  return (
    left.dev === right.dev &&
    left.ino === right.ino &&
    left.mode === right.mode &&
    left.nlink === right.nlink &&
    left.size === right.size &&
    left.mtimeNs === right.mtimeNs
  );
}

function consumeStableRegularFile(
  path,
  label,
  {
    collectBytes = false,
    maximumBytes,
    requireNonempty = false,
  } = {},
) {
  const before = lstatSync(path, { bigint: true });
  if (!before.isFile() || before.isSymbolicLink() || before.nlink !== 1n) {
    throw new Error(`${label} must be a singly linked regular file`);
  }
  if (
    (requireNonempty && before.size === 0n) ||
    (maximumBytes !== undefined && before.size > BigInt(maximumBytes))
  ) {
    throw new Error(`${label} is outside the supported size bound`);
  }
  const descriptor = openSync(
    path,
    constants.O_RDONLY |
      (constants.O_CLOEXEC ?? 0) |
      (constants.O_NOFOLLOW ?? 0),
  );
  const hash = createHash("sha256");
  const chunks = collectBytes ? [] : undefined;
  let openedBefore;
  let openedAfter;
  let total = 0n;
  try {
    openedBefore = fstatSync(descriptor, { bigint: true });
    if (
      !openedBefore.isFile() ||
      openedBefore.isSymbolicLink() ||
      openedBefore.nlink !== 1n ||
      !sameStableIdentity(before, openedBefore)
    ) {
      throw new Error(`${label} changed while it was opened`);
    }
    const buffer = Buffer.allocUnsafe(64 * 1024);
    let bytesRead;
    do {
      bytesRead = readSync(descriptor, buffer, 0, buffer.length, null);
      if (bytesRead > 0) {
        const bytes = buffer.subarray(0, bytesRead);
        hash.update(bytes);
        if (chunks !== undefined) chunks.push(Buffer.from(bytes));
        total += BigInt(bytesRead);
      }
    } while (bytesRead > 0);
    openedAfter = fstatSync(descriptor, { bigint: true });
  } finally {
    closeSync(descriptor);
  }
  const after = lstatSync(path, { bigint: true });
  if (
    !sameStableIdentity(before, openedAfter) ||
    !sameStableIdentity(before, after) ||
    total !== before.size
  ) {
    throw new Error(`${label} changed while it was read`);
  }
  return Object.freeze({
    bytes: chunks === undefined ? undefined : Buffer.concat(chunks),
    identity: Object.freeze({
      ctimeNs: before.ctimeNs,
      dev: before.dev,
      ino: before.ino,
      mode: before.mode,
      mtimeNs: before.mtimeNs,
      nlink: before.nlink,
      size: before.size,
    }),
    sha256: hash.digest("hex"),
  });
}

export function readStableRegularFile(
  path,
  {
    label = "File",
    maximumBytes,
    requireNonempty = false,
  } = {},
) {
  if (
    maximumBytes !== undefined &&
    (!Number.isSafeInteger(maximumBytes) || maximumBytes < 0)
  ) {
    throw new TypeError("maximumBytes must be a non-negative safe integer");
  }
  return consumeStableRegularFile(path, label, {
    collectBytes: true,
    maximumBytes,
    requireNonempty,
  });
}

export function readStableRegularFileDigest(
  path,
  {
    label = "File",
    maximumBytes,
    requireNonempty = false,
  } = {},
) {
  if (
    maximumBytes !== undefined &&
    (!Number.isSafeInteger(maximumBytes) || maximumBytes < 0)
  ) {
    throw new TypeError("maximumBytes must be a non-negative safe integer");
  }
  return consumeStableRegularFile(path, label, {
    maximumBytes,
    requireNonempty,
  });
}

function lstatOrNull(path) {
  try {
    return lstatSync(path, { bigint: true });
  } catch (error) {
    if (error?.code === "ENOENT" || error?.code === "ENOTDIR") return null;
    throw error;
  }
}

function selectedCargoLock(repoRoot, env) {
  const configured = env[NATIVE_BUILD_CARGO_LOCK_ENV];
  if (
    configured !== undefined &&
    (typeof configured !== "string" ||
      configured.length === 0 ||
      !isAbsolute(configured))
  ) {
    throw new Error(
      `${NATIVE_BUILD_CARGO_LOCK_ENV} must name an absolute Cargo.lock path`,
    );
  }
  const cargoLockPath =
    configured === undefined
      ? join(resolve(repoRoot), "Cargo.lock")
      : configured;
  if (
    resolve(cargoLockPath) !== cargoLockPath ||
    realpathSync(cargoLockPath) !== cargoLockPath
  ) {
    throw new Error(
      "Native build Cargo.lock path must be canonical and contain no symbolic-link components",
    );
  }
  const metadata = lstatSync(cargoLockPath, { bigint: true });
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    canonicalRegularMode(metadata) !== "100644"
  ) {
    throw new Error(
      "Native build Cargo.lock must be a non-executable regular non-symbolic-link file",
    );
  }
  const seal = readStableRegularFileDigest(cargoLockPath, {
    label: "Native build Cargo.lock",
    maximumBytes: Number(MAX_CARGO_LOCK_BYTES),
    requireNonempty: true,
  });
  return Object.freeze({
    identity: seal.identity,
    path: cargoLockPath,
    sha256: seal.sha256,
  });
}

function assertSelectedCargoLockUnchanged(cargoLock) {
  if (
    realpathSync(cargoLock.path) !== cargoLock.path ||
    resolve(cargoLock.path) !== cargoLock.path
  ) {
    throw new Error("Native build Cargo.lock path changed identity");
  }
  const current = readStableRegularFileDigest(cargoLock.path, {
    label: "Native build Cargo.lock",
    maximumBytes: Number(MAX_CARGO_LOCK_BYTES),
    requireNonempty: true,
  });
  if (
    current.sha256 !== cargoLock.sha256 ||
    !sameStableIdentity(current.identity, cargoLock.identity)
  ) {
    throw new Error("Native build Cargo.lock changed while it was in use");
  }
}

/** Capture the exact selected Cargo.lock path, digest, and stable inode. */
export function readSelectedCargoLockSeal(
  repoRoot,
  { env = process.env } = {},
) {
  return selectedCargoLock(repoRoot, env);
}

/** Reject any path, byte, metadata, or inode drift from a captured lock seal. */
export function assertSelectedCargoLockSeal(cargoLock) {
  assertSelectedCargoLockUnchanged(cargoLock);
  return cargoLock;
}

const UNSUPPORTED_DIRECTORY_SYNC_CODES = new Set([
  "EACCES",
  "EBADF",
  "EINVAL",
  "EISDIR",
  "ENOTSUP",
  "EPERM",
]);

function syncDirectory(directory) {
  let descriptor;
  try {
    descriptor = openSync(directory, constants.O_RDONLY);
    fsyncSync(descriptor);
  } catch (error) {
    if (!UNSUPPORTED_DIRECTORY_SYNC_CODES.has(error?.code)) throw error;
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
}

function retireRegularFile(path, label) {
  const before = lstatOrNull(path);
  if (before === null) return false;
  if (!before.isFile() || before.isSymbolicLink() || before.nlink !== 1n) {
    throw new Error(`${label} must be a singly linked regular file`);
  }
  const directory = dirname(path);
  const retiredPath = join(
    directory,
    `${PROVENANCE_RETIRED_PREFIX}${process.pid}-${randomUUID()}`,
  );
  if (lstatOrNull(retiredPath) !== null) {
    throw new Error("Native build provenance retirement path already exists");
  }
  renameSync(path, retiredPath);
  const retired = lstatSync(retiredPath, { bigint: true });
  if (!sameRenamedFileIdentity(before, retired)) {
    throw new Error("Native build provenance changed while it was retired");
  }
  syncDirectory(directory);
  rmSync(retiredPath);
  syncDirectory(directory);
  return true;
}

function canonicalRegularMode(metadata) {
  return (metadata.mode & 0o111n) === 0n ? "100644" : "100755";
}

function appendStableRegularFile(
  hash,
  path,
  label,
  { maximumBytes, requireNonempty = false } = {},
) {
  const before = lstatSync(path, { bigint: true });
  if (!before.isFile() || before.isSymbolicLink() || before.nlink !== 1n) {
    throw new Error(`${label} must be a singly linked regular file`);
  }
  if (
    (requireNonempty && before.size === 0n) ||
    (maximumBytes !== undefined && before.size > maximumBytes)
  ) {
    throw new Error(`${label} is outside the supported size bound`);
  }
  const descriptor = openSync(
    path,
    constants.O_RDONLY |
      (constants.O_CLOEXEC ?? 0) |
      (constants.O_NOFOLLOW ?? 0),
  );
  let openedBefore;
  let openedAfter;
  let total = 0n;
  try {
    openedBefore = fstatSync(descriptor, { bigint: true });
    if (!sameStableIdentity(before, openedBefore)) {
      throw new Error(`${label} changed while it was opened`);
    }
    appendLength(hash, openedBefore.size);
    const buffer = Buffer.allocUnsafe(64 * 1024);
    let bytesRead;
    do {
      bytesRead = readSync(descriptor, buffer, 0, buffer.length, null);
      if (bytesRead > 0) {
        hash.update(buffer.subarray(0, bytesRead));
        total += BigInt(bytesRead);
      }
    } while (bytesRead > 0);
    openedAfter = fstatSync(descriptor, { bigint: true });
  } finally {
    closeSync(descriptor);
  }
  const after = lstatSync(path, { bigint: true });
  if (
    !sameStableIdentity(before, openedAfter) ||
    !sameStableIdentity(before, after) ||
    total !== before.size
  ) {
    throw new Error(`${label} changed while it was read`);
  }
}

function readStableSymlink(path, label) {
  const before = lstatSync(path, { bigint: true });
  if (!before.isSymbolicLink() || before.nlink !== 1n) {
    throw new Error(`${label} must be a singly linked symbolic link`);
  }
  const payload = readlinkSync(path, { encoding: "buffer" });
  const after = lstatSync(path, { bigint: true });
  if (!sameStableIdentity(before, after)) {
    throw new Error(`${label} changed while it was read`);
  }
  return payload;
}

function absoluteSourcePath(repoRoot, relativePath) {
  return Buffer.concat([
    Buffer.from(resolve(repoRoot), "utf8"),
    Buffer.from(sep, "utf8"),
    relativePath,
  ]);
}

function appendSourceEntry(hash, repoRoot, entry, kind) {
  appendField(hash, kind);
  appendField(hash, entry.path);
  const absolutePath = absoluteSourcePath(repoRoot, entry.path);
  const metadata = lstatOrNull(absolutePath);
  if (entry.indexMode === GITLINK_MODE) {
    if (kind !== "tracked-source-v1") {
      throw new Error("Native build untracked source cannot be a gitlink");
    }
    if (
      metadata !== null &&
      (!metadata.isDirectory() || metadata.isSymbolicLink())
    ) {
      throw new Error("Native build gitlink worktree entry has an unsafe file type");
    }
    // The exact gitlink object is already bound by trackedInventory.raw, and
    // Git status binds an absent, dirty, or substituted checkout. Optional
    // submodule contents are deliberately not native build inputs.
    appendField(hash, "gitlink");
    appendField(hash, GITLINK_MODE);
    return;
  }
  if (metadata === null) {
    if (kind !== "tracked-source-v1") {
      // Git paths are arbitrary bytes. Hex keeps this fail-closed diagnostic
      // exact and single-line even for hostile filenames.
      throw new Error(
        `Native build untracked source disappeared while sealing (path_hex=${entry.path.toString("hex")})`,
      );
    }
    appendField(hash, "absent");
    appendField(hash, entry.indexMode);
    return;
  }
  if (metadata.isFile() && !metadata.isSymbolicLink()) {
    appendField(hash, "regular");
    appendField(hash, canonicalRegularMode(metadata));
    appendStableRegularFile(hash, absolutePath, "Native build source", {
      ...(kind === "untracked-source-v1"
        ? { maximumBytes: MAX_UNTRACKED_FILE_BYTES }
        : {}),
    });
    return;
  }
  if (metadata.isSymbolicLink()) {
    appendField(hash, "symlink");
    appendField(hash, "120000");
    appendField(hash, readStableSymlink(absolutePath, "Native build source"));
    return;
  }
  throw new Error("Native build source has an unsafe file type");
}

function fingerprintSourceEntries(
  sourceRoot,
  trackedInventory,
  untrackedInventory,
  status,
  cargoLockPath = absoluteSourcePath(sourceRoot, CARGO_LOCK_PATH),
) {
  const hash = createHash("sha256");
  hash.update(SOURCE_TREE_DOMAIN);
  appendField(hash, "stage0-index-inventory-v1");
  appendField(hash, trackedInventory.raw);
  appendField(hash, "git-status-porcelain-v1-v1");
  appendField(hash, status);
  for (const entry of trackedInventory.entries) {
    if (entry.path.equals(CARGO_LOCK_PATH)) continue;
    appendSourceEntry(hash, sourceRoot, entry, "tracked-source-v1");
  }
  for (const entry of untrackedInventory.entries) {
    if (entry.path.equals(CARGO_LOCK_PATH)) continue;
    appendSourceEntry(hash, sourceRoot, entry, "untracked-source-v1");
  }

  const cargoLockMetadata = lstatOrNull(cargoLockPath);
  if (
    cargoLockMetadata === null ||
    !cargoLockMetadata.isFile() ||
    cargoLockMetadata.isSymbolicLink() ||
    canonicalRegularMode(cargoLockMetadata) !== "100644"
  ) {
    throw new Error(
      "Native build requires a non-executable regular Cargo.lock source input",
    );
  }
  appendField(hash, "required-ignored-build-input-v1");
  appendField(hash, CARGO_LOCK_PATH);
  appendField(hash, "regular");
  appendField(hash, "100644");
  appendStableRegularFile(
    hash,
    cargoLockPath,
    "Native build Cargo.lock",
    {
      maximumBytes: MAX_CARGO_LOCK_BYTES,
      requireNonempty: true,
    },
  );
  return hash.digest("hex");
}

function captureSourceTreeFingerprint(
  repoRoot,
  run,
  env,
  status,
  selectedLock,
) {
  const trackedBefore = readTrackedInventory(repoRoot, run, env);
  const untrackedBefore = readUntrackedInventory(repoRoot, run, env);
  const trackedPaths = new Set(
    trackedBefore.entries.map(({ path }) => path.toString("base64")),
  );
  for (const { path } of untrackedBefore.entries) {
    if (trackedPaths.has(path.toString("base64"))) {
      throw new Error("Native build source inventories overlap");
    }
  }
  const cargoLock = selectedLock ?? selectedCargoLock(repoRoot, env);
  assertSelectedCargoLockUnchanged(cargoLock);

  const sourceTreeSha256 = fingerprintSourceEntries(
    repoRoot,
    trackedBefore,
    untrackedBefore,
    status,
    cargoLock.path,
  );
  assertSelectedCargoLockUnchanged(cargoLock);

  const trackedAfter = readTrackedInventory(repoRoot, run, env);
  const untrackedAfter = readUntrackedInventory(repoRoot, run, env);
  if (
    !trackedAfter.raw.equals(trackedBefore.raw) ||
    !untrackedAfter.raw.equals(untrackedBefore.raw)
  ) {
    throw new Error("Native build source inventory changed while it was sealed");
  }
  return {
    cargoLock,
    sourceTreeSha256,
    trackedInventory: trackedBefore,
    untrackedInventory: untrackedBefore,
  };
}

export function sha256NativeFile(path) {
  return consumeStableRegularFile(path, "Native build output").sha256;
}

function captureNativeBuildSourceState(
  repoRoot,
  { env = process.env, run = spawnSync } = {},
) {
  assertNoGitSourceOverrides(env);
  const sourceGitRevision = readGitRevision(repoRoot, run, env);
  const statusBefore = readGitStatus(repoRoot, run, env);
  const captured = captureSourceTreeFingerprint(
    repoRoot,
    run,
    env,
    statusBefore,
  );
  const cargoLock = captured.cargoLock;
  const statusMiddle = readGitStatus(repoRoot, run, env);
  const rechecked = captureSourceTreeFingerprint(
    repoRoot,
    run,
    env,
    statusMiddle,
    cargoLock,
  );
  assertSelectedCargoLockUnchanged(cargoLock);
  const statusAfter = readGitStatus(repoRoot, run, env);
  const revisionAfter = readGitRevision(repoRoot, run, env);
  if (
    revisionAfter !== sourceGitRevision ||
    !statusMiddle.equals(statusBefore) ||
    !statusAfter.equals(statusBefore) ||
    rechecked.sourceTreeSha256 !== captured.sourceTreeSha256
  ) {
    throw new Error(
      "Native build source changed while its exact fingerprint was captured.",
    );
  }
  return {
    cargoLock,
    sourceState: Object.freeze({
      sourceGitRevision,
      sourceTreeClean: statusBefore.length === 0,
      sourceTreeSha256: captured.sourceTreeSha256,
    }),
    status: Buffer.from(statusBefore),
    trackedInventory: captured.trackedInventory,
    untrackedInventory: captured.untrackedInventory,
  };
}

export function readNativeBuildSourceState(repoRoot, options = {}) {
  return captureNativeBuildSourceState(repoRoot, options).sourceState;
}

function assertSnapshotTarget(repoRoot, targetRoot, run, env) {
  const relativeTarget = relative(resolve(repoRoot), resolve(targetRoot));
  if (relativeTarget === "") {
    throw new Error("Native build source snapshot target must not be the repository root.");
  }
  if (relativeTarget === ".." || relativeTarget.startsWith(`..${sep}`)) {
    return;
  }
  const probe = join(relativeTarget, `${SNAPSHOT_PREFIX}probe`);
  const checkIgnoreEnvironment = canonicalGitEnvironment(env);
  delete checkIgnoreEnvironment.GIT_LITERAL_PATHSPECS;
  const result = run(
    "git",
    [
      ...GIT_CONFIGURATION,
      "-C",
      repoRoot,
      "check-ignore",
      "--quiet",
      "--stdin",
      "-z",
    ],
    {
      encoding: null,
      env: checkIgnoreEnvironment,
      input: Buffer.concat([Buffer.from(probe, "utf8"), Buffer.alloc(1)]),
      maxBuffer: 1024 * 1024,
    },
  );
  if (result.error || result.status !== 0) {
    throw new Error(
      "Native build source snapshot target must be outside the repository or ignored by Git.",
    );
  }
}

function assertCanonicalTargetRoot(targetRoot) {
  const resolvedTarget = resolve(targetRoot);
  if (dirname(resolvedTarget) === resolvedTarget) {
    throw new Error("Native build source snapshot target must not be a filesystem root");
  }
  const metadata = lstatSync(resolvedTarget, { bigint: true });
  if (metadata.isSymbolicLink()) {
    throw new Error(
      "Native build source snapshot target must not contain symbolic-link components",
    );
  }
  if (!metadata.isDirectory()) {
    throw new Error("Native build source snapshot target must be a real directory");
  }
  if (realpathSync(resolvedTarget) !== resolvedTarget) {
    throw new Error(
      "Native build source snapshot target must not contain symbolic-link components",
    );
  }
  return {
    dev: metadata.dev,
    ino: metadata.ino,
    targetRoot: resolvedTarget,
  };
}

function assertTargetIdentity(snapshot) {
  const metadata = lstatSync(snapshot.targetRoot, { bigint: true });
  if (
    !metadata.isDirectory() ||
    metadata.isSymbolicLink() ||
    metadata.dev !== snapshot.targetIdentity.dev ||
    metadata.ino !== snapshot.targetIdentity.ino ||
    realpathSync(snapshot.targetRoot) !== snapshot.targetRoot
  ) {
    throw new Error("Native build source snapshot target changed identity");
  }
}

function assertSnapshotRootIdentity(snapshot) {
  const metadata = lstatSync(snapshot.snapshotRoot, { bigint: true });
  if (
    !metadata.isDirectory() ||
    metadata.isSymbolicLink() ||
    metadata.dev !== snapshot.snapshotIdentity.dev ||
    metadata.ino !== snapshot.snapshotIdentity.ino ||
    realpathSync(dirname(snapshot.snapshotRoot)) !== snapshot.targetRoot
  ) {
    throw new Error("Native build source snapshot root changed identity");
  }
}

function ensureSnapshotParents(snapshotRoot, relativePath) {
  for (let index = 0; index < relativePath.length; index += 1) {
    if (relativePath[index] !== 0x2f) continue;
    const componentPath = relativePath.subarray(0, index);
    if (componentPath.length === 0) continue;
    const directory = absoluteSourcePath(snapshotRoot, componentPath);
    try {
      mkdirSync(directory, { mode: 0o700 });
    } catch (error) {
      if (error?.code !== "EEXIST") throw error;
      const metadata = lstatSync(directory);
      if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
        throw new Error("Native build snapshot parent is not a safe directory");
      }
    }
  }
}

function copySnapshotEntry(repoRoot, snapshotRoot, entry, kind) {
  if (entry.indexMode === GITLINK_MODE) {
    if (kind !== "tracked-source-v1") {
      throw new Error("Native build untracked source cannot be a gitlink");
    }
    return;
  }
  const sourcePath = absoluteSourcePath(repoRoot, entry.path);
  const destinationPath = absoluteSourcePath(snapshotRoot, entry.path);
  const before = lstatOrNull(sourcePath);
  if (before === null) {
    if (kind !== "tracked-source-v1") {
      throw new Error("Native build untracked source disappeared during snapshot");
    }
    return;
  }
  ensureSnapshotParents(snapshotRoot, entry.path);
  if (before.isFile() && !before.isSymbolicLink()) {
    if (before.nlink !== 1n) {
      throw new Error("Native build source must be a singly linked regular file");
    }
    copyFileSync(
      sourcePath,
      destinationPath,
      constants.COPYFILE_EXCL | (constants.COPYFILE_FICLONE ?? 0),
    );
    chmodSync(destinationPath, canonicalRegularMode(before) === "100755" ? 0o500 : 0o400);
  } else if (before.isSymbolicLink()) {
    const payload = readStableSymlink(sourcePath, "Native build source");
    symlinkSync(payload, destinationPath);
  } else {
    throw new Error("Native build source has an unsafe file type");
  }
  const after = lstatSync(sourcePath, { bigint: true });
  if (!sameStableIdentity(before, after)) {
    throw new Error("Native build source changed while it was snapshotted");
  }
}

function copySelectedCargoLock(snapshotRoot, cargoLock) {
  assertSelectedCargoLockUnchanged(cargoLock);
  const destinationPath = absoluteSourcePath(
    snapshotRoot,
    CARGO_LOCK_PATH,
  );
  copyFileSync(
    cargoLock.path,
    destinationPath,
    constants.COPYFILE_EXCL | (constants.COPYFILE_FICLONE ?? 0),
  );
  chmodSync(destinationPath, 0o400);
  const copied = readStableRegularFileDigest(destinationPath, {
    label: "Native build snapshotted Cargo.lock",
    maximumBytes: Number(MAX_CARGO_LOCK_BYTES),
    requireNonempty: true,
  });
  if (copied.sha256 !== cargoLock.sha256) {
    throw new Error(
      "Native build snapshotted Cargo.lock does not match its selected input",
    );
  }
  assertSelectedCargoLockUnchanged(cargoLock);
}

function snapshotRelativePath(parent, name) {
  return parent.length === 0
    ? Buffer.from(name)
    : Buffer.concat([parent, Buffer.from("/"), name]);
}

function snapshotInventory(snapshotRoot) {
  const entries = [];
  const visit = (relativeDirectory) => {
    const absoluteDirectory =
      relativeDirectory.length === 0
        ? Buffer.from(snapshotRoot, "utf8")
        : absoluteSourcePath(snapshotRoot, relativeDirectory);
    const names = readdirSync(absoluteDirectory, { encoding: "buffer" }).sort(
      Buffer.compare,
    );
    for (const name of names) {
      if (name.length === 0 || name.includes(0) || name.includes(0x2f)) {
        throw new Error("Native build snapshot contains an unsafe filesystem name");
      }
      const relativePath = snapshotRelativePath(relativeDirectory, name);
      const absolutePath = absoluteSourcePath(snapshotRoot, relativePath);
      const metadata = lstatSync(absolutePath, { bigint: true });
      let kind;
      if (metadata.isDirectory() && !metadata.isSymbolicLink()) kind = "directory";
      else if (metadata.isFile() && !metadata.isSymbolicLink()) kind = "regular";
      else if (metadata.isSymbolicLink()) kind = "symlink";
      else throw new Error("Native build snapshot contains an unsafe file type");
      entries.push({ absolutePath, kind, metadata, path: relativePath });
      if (kind === "directory") visit(relativePath);
    }
  };
  visit(Buffer.alloc(0));
  return entries;
}

function expectedSnapshotInventory(snapshotRoot, trackedInventory, untrackedInventory) {
  const expected = new Map();
  const add = (path, kind) => {
    const key = path.toString("base64");
    const prior = expected.get(key);
    if (prior !== undefined && prior.kind !== kind) {
      throw new Error("Native build snapshot has conflicting expected paths");
    }
    expected.set(key, { kind, path });
    let slash = path.lastIndexOf(0x2f);
    while (slash > 0) {
      const parent = Buffer.from(path.subarray(0, slash));
      expected.set(parent.toString("base64"), {
        kind: "directory",
        path: parent,
      });
      slash = parent.lastIndexOf(0x2f);
    }
  };
  for (const entry of [...trackedInventory.entries, ...untrackedInventory.entries]) {
    if (entry.path.equals(CARGO_LOCK_PATH)) continue;
    if (entry.indexMode === GITLINK_MODE) continue;
    const metadata = lstatOrNull(absoluteSourcePath(snapshotRoot, entry.path));
    if (metadata === null) continue;
    add(entry.path, metadata.isSymbolicLink() ? "symlink" : "regular");
  }
  add(CARGO_LOCK_PATH, "regular");
  return expected;
}

function assertExactSnapshotInventory(snapshot) {
  const rootMetadata = lstatSync(snapshot.snapshotRoot, { bigint: true });
  if (
    !rootMetadata.isDirectory() ||
    rootMetadata.isSymbolicLink() ||
    (rootMetadata.mode & 0o222n) !== 0n
  ) {
    throw new Error("Native build snapshot root is not owner-read-only");
  }
  const expected = expectedSnapshotInventory(
    snapshot.snapshotRoot,
    snapshot.trackedInventory,
    snapshot.untrackedInventory,
  );
  const actual = snapshotInventory(snapshot.snapshotRoot);
  if (actual.length !== expected.size) {
    throw new Error("Native build snapshot filesystem inventory changed");
  }
  for (const entry of actual) {
    const wanted = expected.get(entry.path.toString("base64"));
    if (wanted?.kind !== entry.kind) {
      throw new Error("Native build snapshot filesystem inventory changed");
    }
    if (
      (entry.kind === "regular" || entry.kind === "directory") &&
      (entry.metadata.mode & 0o222n) !== 0n
    ) {
      throw new Error("Native build snapshot is not owner-read-only");
    }
    if (
      (entry.kind === "regular" || entry.kind === "symlink") &&
      entry.metadata.nlink !== 1n
    ) {
      throw new Error("Native build snapshot contains a multiply linked source");
    }
  }
  return actual;
}

function assertSnapshotSymlinksContained(snapshot) {
  const root = Buffer.from(realpathSync(snapshot.snapshotRoot), "utf8");
  const separator = Buffer.from(sep, "utf8");
  const rootPrefix = Buffer.concat([root, separator]);
  const isInside = (resolved, { allowRoot = false } = {}) =>
    (allowRoot && resolved.equals(root)) ||
    (resolved.length > rootPrefix.length &&
      resolved.subarray(0, rootPrefix.length).equals(rootPrefix));
  const assertResolvedInside = (resolved, options) => {
    if (!isInside(resolved, options)) {
      throw new Error(
        "Native build snapshot symlink must resolve strictly within the snapshot",
      );
    }
  };
  for (const entry of snapshotInventory(snapshot.snapshotRoot)) {
    if (entry.kind !== "symlink") continue;
    const payload = readlinkSync(entry.absolutePath, { encoding: "buffer" });
    if (
      payload.length === 0 ||
      payload[0] === 0x2f ||
      payload.includes(0) ||
      (process.platform === "win32" &&
        (payload.includes(0x5c) ||
          /^[A-Za-z]:/u.test(payload.toString("latin1"))))
    ) {
      throw new Error(
        "Native build snapshot symlink must resolve strictly within the snapshot",
      );
    }
    const linkSlash = entry.path.lastIndexOf(0x2f);
    const linkParent =
      linkSlash < 0
        ? Buffer.from(snapshot.snapshotRoot, "utf8")
        : absoluteSourcePath(
            snapshot.snapshotRoot,
            entry.path.subarray(0, linkSlash),
          );
    let current = realpathSync(linkParent, { encoding: "buffer" });
    assertResolvedInside(current, { allowRoot: true });
    let start = 0;
    for (let index = 0; index <= payload.length; index += 1) {
      if (index !== payload.length && payload[index] !== 0x2f) continue;
      const component = Buffer.from(payload.subarray(start, index));
      start = index + 1;
      if (component.length === 0 || component.equals(Buffer.from("."))) continue;
      if (component.equals(Buffer.from(".."))) {
        const slash = current.lastIndexOf(separator);
        if (slash <= 0) {
          throw new Error(
            "Native build snapshot symlink must resolve strictly within the snapshot",
          );
        }
        current = Buffer.from(current.subarray(0, slash));
        assertResolvedInside(current, { allowRoot: true });
      } else {
        const candidate = Buffer.concat([current, separator, component]);
        const metadata = lstatOrNull(candidate);
        if (metadata?.isSymbolicLink()) {
          try {
            current = realpathSync(candidate, { encoding: "buffer" });
          } catch {
            throw new Error(
              "Native build snapshot contains a dangling chained symbolic link",
            );
          }
        } else {
          current = candidate;
        }
        assertResolvedInside(current);
      }
    }
    assertResolvedInside(current);
  }
}

function freezeSnapshotDirectories(snapshotRoot) {
  const directories = snapshotInventory(snapshotRoot)
    .filter(({ kind }) => kind === "directory")
    .sort((left, right) => right.path.length - left.path.length);
  for (const { absolutePath } of directories) chmodSync(absolutePath, 0o500);
  chmodSync(snapshotRoot, 0o500);
}

function thawSnapshotDirectories(snapshotRoot) {
  const rootMetadata = lstatSync(snapshotRoot);
  if (!rootMetadata.isDirectory() || rootMetadata.isSymbolicLink()) {
    throw new Error("Native build snapshot root is not a safe directory");
  }
  chmodSync(snapshotRoot, 0o700);
  for (const { absolutePath, kind } of snapshotInventory(snapshotRoot)) {
    if (kind === "directory") chmodSync(absolutePath, 0o700);
  }
}

function removeNativeBuildSourceSnapshot(snapshot) {
  if (
    typeof snapshot?.snapshotRoot !== "string" ||
    !snapshot.snapshotRoot.startsWith(
      `${resolve(snapshot.targetRoot)}${sep}${SNAPSHOT_PREFIX}`,
    ) ||
    dirname(snapshot.snapshotRoot) !== resolve(snapshot.targetRoot)
  ) {
    throw new Error("Refusing to remove an invalid native build snapshot path");
  }
  assertTargetIdentity(snapshot);
  assertSnapshotRootIdentity(snapshot);
  thawSnapshotDirectories(snapshot.snapshotRoot);
  rmSync(snapshot.snapshotRoot, { recursive: true, force: true });
}

export function createNativeBuildSourceSnapshot(
  repoRoot,
  targetRoot,
  { env = process.env, run = spawnSync } = {},
) {
  assertNoGitSourceOverrides(env);
  if (existsSync(targetRoot)) assertCanonicalTargetRoot(targetRoot);
  assertSnapshotTarget(repoRoot, targetRoot, run, env);
  const captured = captureNativeBuildSourceState(repoRoot, { env, run });
  mkdirSync(targetRoot, { recursive: true });
  const canonicalTarget = assertCanonicalTargetRoot(targetRoot);
  const snapshotRoot = mkdtempSync(
    join(canonicalTarget.targetRoot, SNAPSHOT_PREFIX),
  );
  const snapshotMetadata = lstatSync(snapshotRoot, { bigint: true });
  const snapshot = {
    cargoLockSource: captured.cargoLock,
    snapshotRoot,
    snapshotIdentity: Object.freeze({
      dev: snapshotMetadata.dev,
      ino: snapshotMetadata.ino,
    }),
    sourceState: captured.sourceState,
    status: captured.status,
    targetIdentity: Object.freeze({
      dev: canonicalTarget.dev,
      ino: canonicalTarget.ino,
    }),
    targetRoot: canonicalTarget.targetRoot,
    trackedInventory: captured.trackedInventory,
    untrackedInventory: captured.untrackedInventory,
  };
  try {
    for (const entry of captured.trackedInventory.entries) {
      if (entry.path.equals(CARGO_LOCK_PATH)) continue;
      copySnapshotEntry(repoRoot, snapshotRoot, entry, "tracked-source-v1");
    }
    for (const entry of captured.untrackedInventory.entries) {
      if (entry.path.equals(CARGO_LOCK_PATH)) continue;
      copySnapshotEntry(repoRoot, snapshotRoot, entry, "untracked-source-v1");
    }
    copySelectedCargoLock(snapshotRoot, captured.cargoLock);
    assertTargetIdentity(snapshot);
    assertSnapshotRootIdentity(snapshot);
    assertSnapshotSymlinksContained(snapshot);
    if (
      fingerprintSourceEntries(
        snapshotRoot,
        captured.trackedInventory,
        captured.untrackedInventory,
        captured.status,
      ) !== captured.sourceState.sourceTreeSha256
    ) {
      throw new Error("Native build source snapshot does not match its captured seal");
    }
    freezeSnapshotDirectories(snapshotRoot);
    assertExactSnapshotInventory(snapshot);
    const sourceAfterCopy = readNativeBuildSourceState(repoRoot, { env, run });
    if (
      sourceAfterCopy.sourceGitRevision !== captured.sourceState.sourceGitRevision ||
      sourceAfterCopy.sourceTreeClean !== captured.sourceState.sourceTreeClean ||
      sourceAfterCopy.sourceTreeSha256 !== captured.sourceState.sourceTreeSha256
    ) {
      throw new Error("Native build source changed while its snapshot was created");
    }
    return Object.freeze(snapshot);
  } catch (error) {
    removeNativeBuildSourceSnapshot(snapshot);
    throw error;
  }
}

export function verifyNativeBuildSourceSnapshot(snapshot) {
  assertSelectedCargoLockUnchanged(snapshot.cargoLockSource);
  assertTargetIdentity(snapshot);
  assertSnapshotRootIdentity(snapshot);
  assertExactSnapshotInventory(snapshot);
  assertSnapshotSymlinksContained(snapshot);
  const digest = fingerprintSourceEntries(
    snapshot.snapshotRoot,
    snapshot.trackedInventory,
    snapshot.untrackedInventory,
    snapshot.status,
  );
  if (digest !== snapshot.sourceState.sourceTreeSha256) {
    throw new Error("Native build source snapshot changed while Cargo was running");
  }
  assertSelectedCargoLockUnchanged(snapshot.cargoLockSource);
  return snapshot.sourceState;
}

export function cleanupNativeBuildSourceSnapshot(snapshot) {
  removeNativeBuildSourceSnapshot(snapshot);
}

export function createNativeBuildProvenance({
  cargoProfile,
  nativePath,
  sourceBefore,
  sourceAfter,
}) {
  if (
    cargoProfile !== "debug" &&
    cargoProfile !== "release" &&
    cargoProfile !== "deploy"
  ) {
    throw new TypeError("native build provenance has an invalid Cargo profile");
  }
  for (const [label, state] of [
    ["pre-build source", sourceBefore],
    ["post-build source", sourceAfter],
  ]) {
    assertExactKeys(
      state,
      ["sourceGitRevision", "sourceTreeClean", "sourceTreeSha256"],
      label,
    );
    if (
      !REVISION_PATTERN.test(state.sourceGitRevision) ||
      typeof state.sourceTreeClean !== "boolean" ||
      !SHA256_PATTERN.test(state.sourceTreeSha256)
    ) {
      throw new TypeError(`${label} is invalid`);
    }
  }
  if (sourceBefore.sourceGitRevision !== sourceAfter.sourceGitRevision) {
    throw new Error("Native build source revision changed while Cargo was running.");
  }
  if (
    sourceBefore.sourceTreeClean !== sourceAfter.sourceTreeClean ||
    sourceBefore.sourceTreeSha256 !== sourceAfter.sourceTreeSha256
  ) {
    throw new Error(
      "Native build source tree changed while Cargo was running.",
    );
  }
  return Object.freeze({
    version: 3,
    build_execution_policy: NATIVE_BUILD_EXECUTION_POLICY,
    cargo_profile: cargoProfile,
    native_sha256: sha256NativeFile(nativePath),
    source_git_revision: sourceAfter.sourceGitRevision,
    source_tree_clean: sourceAfter.sourceTreeClean,
    source_tree_sha256: sourceAfter.sourceTreeSha256,
  });
}

export function nativeBuildProvenancePath(nativePath) {
  return join(dirname(nativePath), NATIVE_BUILD_PROVENANCE_FILENAME);
}

/**
 * Durably remove every sidecar name that could authenticate an earlier build.
 * Call this before any in-place Cargo invocation. It deliberately never
 * restores retired provenance after a failed or interrupted build.
 */
export function invalidateNativeBuildProvenance(nativePath) {
  const path = nativeBuildProvenancePath(nativePath);
  const invalidated = retireRegularFile(path, "Native build provenance");
  const previousInvalidated = retireRegularFile(
    `${path}${PROVENANCE_PREVIOUS_SUFFIX}`,
    "Native build provenance recovery backup",
  );
  return invalidated || previousInvalidated;
}

export function writeNativeBuildProvenance(nativePath, provenance) {
  validateNativeBuildProvenance(provenance, nativePath);
  const path = nativeBuildProvenancePath(nativePath);
  const previousPath = `${path}${PROVENANCE_PREVIOUS_SUFFIX}`;
  const bytes = `${JSON.stringify(provenance, null, 2)}\n`;
  const noFollow = constants.O_NOFOLLOW ?? 0;
  const temporaryPath = join(
    dirname(path),
    `.${NATIVE_BUILD_PROVENANCE_FILENAME}.${process.pid}.${randomUUID()}.tmp`,
  );
  const descriptor = openSync(
    temporaryPath,
    constants.O_CREAT |
      constants.O_EXCL |
      constants.O_WRONLY |
      noFollow,
    0o600,
  );
  try {
    const metadata = fstatSync(descriptor, { bigint: true });
    if (!metadata.isFile() || metadata.nlink !== 1n) {
      throw new Error("Native build provenance temporary must be singly linked");
    }
    writeFileSync(descriptor, bytes, "utf8");
    fsyncSync(descriptor);
  } finally {
    closeSync(descriptor);
  }
  try {
    if (existsSync(previousPath)) {
      const previous = lstatSync(previousPath, { bigint: true });
      if (
        !previous.isFile() ||
        previous.isSymbolicLink() ||
        previous.nlink !== 1n
      ) {
        throw new Error("Native build provenance recovery backup is unsafe");
      }
      retireRegularFile(
        previousPath,
        "Native build provenance recovery backup",
      );
    }
    if (existsSync(path)) {
      const current = lstatSync(path, { bigint: true });
      if (
        !current.isFile() ||
        current.isSymbolicLink() ||
        current.nlink !== 1n
      ) {
        throw new Error(
          "Existing native build provenance must be a singly linked regular non-symbolic-link file",
        );
      }
    }
    if (process.platform === "win32" && existsSync(path)) {
      renameSync(path, previousPath);
      syncDirectory(dirname(path));
      try {
        renameSync(temporaryPath, path);
      } catch (error) {
        throw error;
      }
      syncDirectory(dirname(path));
      retireRegularFile(
        previousPath,
        "Native build provenance recovery backup",
      );
    } else {
      renameSync(temporaryPath, path);
      syncDirectory(dirname(path));
    }
  } catch (error) {
    rmSync(temporaryPath, { force: true });
    throw error;
  }
  const metadata = lstatSync(path, { bigint: true });
  if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.nlink !== 1n) {
    throw new Error(
      "Native build provenance must be a singly linked regular non-symbolic-link file",
    );
  }
  const published = readStableRegularFile(path, {
    label: "Native build provenance",
    maximumBytes: MAX_PROVENANCE_BYTES,
    requireNonempty: true,
  });
  if (published.bytes.toString("utf8") !== bytes) {
    throw new Error("Native build provenance changed during atomic publication");
  }
  return path;
}

export function validateNativeBuildProvenance(provenance, nativePath) {
  assertExactKeys(
    provenance,
    [
      "build_execution_policy",
      "cargo_profile",
      "native_sha256",
      "source_git_revision",
      "source_tree_clean",
      "source_tree_sha256",
      "version",
    ],
    "native build provenance",
  );
  if (
    provenance.version !== 3 ||
    provenance.build_execution_policy !== NATIVE_BUILD_EXECUTION_POLICY ||
    (provenance.cargo_profile !== "debug" &&
      provenance.cargo_profile !== "release" &&
      provenance.cargo_profile !== "deploy") ||
    !SHA256_PATTERN.test(provenance.native_sha256) ||
    !REVISION_PATTERN.test(provenance.source_git_revision) ||
    typeof provenance.source_tree_clean !== "boolean" ||
    !SHA256_PATTERN.test(provenance.source_tree_sha256) ||
    provenance.native_sha256 !== sha256NativeFile(nativePath)
  ) {
    throw new Error("Native build provenance does not match the compiled binary.");
  }
  return Object.freeze({ ...provenance });
}

export function readNativeBuildProvenance(nativePath) {
  const path = nativeBuildProvenancePath(nativePath);
  const before = readStableRegularFile(path, {
    label: "Native build provenance",
    maximumBytes: MAX_PROVENANCE_BYTES,
    requireNonempty: true,
  });
  let parsed;
  try {
    parsed = JSON.parse(before.bytes.toString("utf8"));
  } catch {
    throw new Error("Native build provenance is invalid or unreadable.");
  }
  const validated = validateNativeBuildProvenance(parsed, nativePath);
  const after = readStableRegularFile(path, {
    label: "Native build provenance",
    maximumBytes: MAX_PROVENANCE_BYTES,
    requireNonempty: true,
  });
  if (
    !sameStableIdentity(before.identity, after.identity) ||
    !before.bytes.equals(after.bytes)
  ) {
    throw new Error(
      "Native build provenance changed while the compiled binary was validated.",
    );
  }
  return validated;
}
