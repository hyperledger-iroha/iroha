import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  closeSync,
  constants,
  fstatSync,
  fsyncSync,
  lstatSync,
  openSync,
  readFileSync,
  readlinkSync,
  readSync,
  writeFileSync,
} from "node:fs";
import { devNull } from "node:os";
import { dirname, join, resolve, sep } from "node:path";

export const NATIVE_BUILD_PROVENANCE_FILENAME =
  "iroha_js_host.build-provenance.json";
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REVISION_PATTERN = /^[0-9a-f]{40}$/u;
const MAX_PROVENANCE_BYTES = 16 * 1024;
const MAX_GIT_INVENTORY_BYTES = 32 * 1024 * 1024;
const MAX_GIT_STATUS_BYTES = 16 * 1024 * 1024;
const MAX_UNTRACKED_FILES = 100_000;
const MAX_UNTRACKED_FILE_BYTES = 64n * 1024n * 1024n;
const MAX_CARGO_LOCK_BYTES = 16n * 1024n * 1024n;
const CARGO_LOCK_PATH = Buffer.from("Cargo.lock", "utf8");
const SOURCE_TREE_DOMAIN = Buffer.from(
  "iroha.js.native-build-source-tree.v2\0",
  "utf8",
);
const ALLOWED_TRACKED_MODES = new Set(["100644", "100755", "120000"]);
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
    path.includes(0)
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

function lstatOrNull(path) {
  try {
    return lstatSync(path, { bigint: true });
  } catch (error) {
    if (error?.code === "ENOENT" || error?.code === "ENOTDIR") return null;
    throw error;
  }
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
  if (metadata === null) {
    if (kind !== "tracked-source-v1") {
      throw new Error("Native build untracked source disappeared while sealing");
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

function captureSourceTreeFingerprint(repoRoot, run, env) {
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

  const hash = createHash("sha256");
  hash.update(SOURCE_TREE_DOMAIN);
  for (const entry of trackedBefore.entries) {
    if (entry.path.equals(CARGO_LOCK_PATH)) continue;
    appendSourceEntry(hash, repoRoot, entry, "tracked-source-v1");
  }
  for (const entry of untrackedBefore.entries) {
    if (entry.path.equals(CARGO_LOCK_PATH)) continue;
    appendSourceEntry(hash, repoRoot, entry, "untracked-source-v1");
  }

  const cargoLockPath = absoluteSourcePath(repoRoot, CARGO_LOCK_PATH);
  const cargoLockMetadata = lstatOrNull(cargoLockPath);
  if (
    cargoLockMetadata === null ||
    !cargoLockMetadata.isFile() ||
    cargoLockMetadata.isSymbolicLink() ||
    canonicalRegularMode(cargoLockMetadata) !== "100644"
  ) {
    throw new Error(
      "Native build requires a non-executable regular root Cargo.lock source input",
    );
  }
  appendField(hash, "required-ignored-build-input-v1");
  appendField(hash, CARGO_LOCK_PATH);
  appendField(hash, "regular");
  appendField(hash, "100644");
  appendStableRegularFile(hash, cargoLockPath, "Native build root Cargo.lock", {
    maximumBytes: MAX_CARGO_LOCK_BYTES,
    requireNonempty: true,
  });

  const trackedAfter = readTrackedInventory(repoRoot, run, env);
  const untrackedAfter = readUntrackedInventory(repoRoot, run, env);
  if (
    !trackedAfter.raw.equals(trackedBefore.raw) ||
    !untrackedAfter.raw.equals(untrackedBefore.raw)
  ) {
    throw new Error("Native build source inventory changed while it was sealed");
  }
  return hash.digest("hex");
}

export function sha256NativeFile(path) {
  assertRegularFile(path, "Native build output");
  const hash = createHash("sha256");
  const descriptor = openSync(path, "r");
  const buffer = Buffer.allocUnsafe(64 * 1024);
  try {
    let bytesRead;
    do {
      bytesRead = readSync(descriptor, buffer, 0, buffer.length, null);
      if (bytesRead > 0) hash.update(buffer.subarray(0, bytesRead));
    } while (bytesRead > 0);
  } finally {
    closeSync(descriptor);
  }
  return hash.digest("hex");
}

export function readNativeBuildSourceState(
  repoRoot,
  { env = process.env, run = spawnSync } = {},
) {
  assertNoGitSourceOverrides(env);
  const sourceGitRevision = readGitRevision(repoRoot, run, env);
  const statusBefore = readGitStatus(repoRoot, run, env);
  const sourceTreeSha256 = captureSourceTreeFingerprint(repoRoot, run, env);
  const sourceTreeRecheckSha256 = captureSourceTreeFingerprint(
    repoRoot,
    run,
    env,
  );
  const statusAfter = readGitStatus(repoRoot, run, env);
  const revisionAfter = readGitRevision(repoRoot, run, env);
  if (
    revisionAfter !== sourceGitRevision ||
    !statusAfter.equals(statusBefore) ||
    sourceTreeRecheckSha256 !== sourceTreeSha256
  ) {
    throw new Error(
      "Native build source changed while its exact fingerprint was captured.",
    );
  }
  return Object.freeze({
    sourceGitRevision,
    sourceTreeClean: statusBefore.length === 0,
    sourceTreeSha256,
  });
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
    version: 2,
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

export function writeNativeBuildProvenance(nativePath, provenance) {
  validateNativeBuildProvenance(provenance, nativePath);
  const path = nativeBuildProvenancePath(nativePath);
  const bytes = `${JSON.stringify(provenance, null, 2)}\n`;
  const noFollow = constants.O_NOFOLLOW ?? 0;
  const descriptor = openSync(
    path,
    constants.O_CREAT | constants.O_TRUNC | constants.O_WRONLY | noFollow,
    0o600,
  );
  try {
    writeFileSync(descriptor, bytes, "utf8");
    fsyncSync(descriptor);
  } finally {
    closeSync(descriptor);
  }
  assertRegularFile(path, "Native build provenance");
  return path;
}

export function validateNativeBuildProvenance(provenance, nativePath) {
  assertExactKeys(
    provenance,
    [
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
    provenance.version !== 2 ||
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
  const metadata = assertRegularFile(path, "Native build provenance");
  if (metadata.size <= 0 || metadata.size > MAX_PROVENANCE_BYTES) {
    throw new Error("Native build provenance is outside the supported size bound.");
  }
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(path, "utf8"));
  } catch {
    throw new Error("Native build provenance is invalid or unreadable.");
  }
  return validateNativeBuildProvenance(parsed, nativePath);
}
