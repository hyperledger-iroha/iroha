#!/usr/bin/env node
/** Build the native `iroha_js_host` library and bind it to Git provenance. */
import { spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  closeSync,
  constants,
  fchmodSync,
  fstatSync,
  fsyncSync,
  linkSync,
  lstatSync,
  mkdirSync,
  openSync,
  readSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  rmdirSync,
  writeSync,
} from "node:fs";
import { hostname, tmpdir } from "node:os";
import {
  basename,
  dirname,
  isAbsolute,
  join,
  relative,
  resolve,
  sep,
} from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import {
  cargoBuildArgsForNativeProfile,
  resolveNativeBuildProfile,
} from "./native-build-profile.mjs";
import {
  cleanupNativeBuildSourceSnapshot,
  createNativeBuildProvenance,
  createNativeBuildSourceSnapshot,
  invalidateNativeBuildProvenance,
  readStableRegularFile,
  readStableRegularFileDigest,
  verifyNativeBuildSourceSnapshot,
  writeNativeBuildProvenance,
} from "./native-build-provenance.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = join(scriptDir, "..", "..", "..");
const RUN_CONTAINER_PREFIX = ".iroha-js-native-build-run-";
const RUN_INITIALIZER_PREFIX = ".iroha-js-native-build-init-v1-";
const RUN_TRASH_PREFIX = ".iroha-js-native-build-trash-v1-";
const RUN_TRASH_OWNER_SUFFIX = ".owner.json";
const RUN_OWNER_FILENAME = ".iroha-js-native-build-run-owner-v1.json";
const RUN_OWNER_VERSION = 1;
const RUN_CARGO_TARGET_NAME = "cargo-target";
const RUN_CARGO_OUTPUT_SEAL_NAME = ".iroha-js-native-output-seal";
const PUBLISH_LOCK_SUFFIX = ".publish-lock";
const PUBLISH_LOCK_CANDIDATE_SUFFIX = ".publish-lock-candidate-";
const PUBLISH_LOCK_RELEASED_SUFFIX = ".publish-lock-released-";
const PUBLISH_LOCK_STALE_SUFFIX = ".publish-lock-stale-";
const PUBLISH_LOCK_TRASH_SUFFIX = ".publish-lock-trash-";
const PUBLISH_LOCK_OWNER_FILENAME = "owner.json";
const PUBLISH_LOCK_RECOVERED_FILENAME = "recovered.json";
const PUBLISH_LOCK_VERSION = 1;
const MAX_CARGO_JSON_BYTES = 64 * 1024 * 1024;
const MAX_CARGO_MANIFEST_BYTES = 4 * 1024 * 1024;
const MAX_PUBLISH_LOCK_OWNER_BYTES = 4 * 1024;
const MAX_RUN_OWNER_BYTES = 4 * 1024;
const INTENDED_PACKAGE = "iroha_js_host";
const UUID_SOURCE =
  "[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}";
const UUID_PATTERN = new RegExp(`^${UUID_SOURCE}$`, "u");
const RUN_CONTAINER_PATTERN = new RegExp(
  `^${RUN_CONTAINER_PREFIX}(${UUID_SOURCE})$`,
  "u",
);
const RUN_INITIALIZER_PATTERN = new RegExp(
  `^${RUN_INITIALIZER_PREFIX}(${UUID_SOURCE})-(${UUID_SOURCE})$`,
  "u",
);
const RUN_TRASH_PATTERN = new RegExp(
  `^${RUN_TRASH_PREFIX}(${UUID_SOURCE})-(${UUID_SOURCE})$`,
  "u",
);
const RUN_TRASH_OWNER_PATTERN = new RegExp(
  `^(${RUN_TRASH_PREFIX}(${UUID_SOURCE})-(${UUID_SOURCE}))\\.owner\\.json$`,
  "u",
);
const RUN_SNAPSHOT_PATTERN =
  /^\.iroha-js-source-snapshot-[A-Za-z0-9]{6}$/u;
const UNSUPPORTED_DIRECTORY_SYNC_CODES = new Set([
  "EACCES",
  "EBADF",
  "EINVAL",
  "EISDIR",
  "ENOTSUP",
  "EPERM",
]);

function newRunUuid() {
  const uuid = randomUUID();
  if (!UUID_PATTERN.test(uuid)) {
    throw new Error("Native build UUID generator returned an invalid value.");
  }
  return uuid;
}

function lstatOrNull(path) {
  try {
    return lstatSync(path, { bigint: true });
  } catch (error) {
    if (error?.code === "ENOENT" || error?.code === "ENOTDIR") return null;
    throw error;
  }
}

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

function isPathInside(parent, child) {
  const pathFromParent = relative(parent, child);
  return (
    pathFromParent === "" ||
    (pathFromParent !== ".." && !pathFromParent.startsWith(`..${sep}`))
  );
}

function assertDisjointPathAncestry(left, right, label) {
  if (isPathInside(left, right) || isPathInside(right, left)) {
    throw new Error(`${label} must not contain or be contained by the build source.`);
  }
}

function canonicalDirectory(path, label, { create = false } = {}) {
  const resolvedPath = resolve(path);
  if (create) mkdirSync(resolvedPath, { mode: 0o700, recursive: true });
  const metadata = lstatSync(resolvedPath, { bigint: true });
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a real non-symbolic-link directory.`);
  }
  const canonicalPath = realpathSync(resolvedPath);
  return {
    canonicalPath,
    identity: Object.freeze({ dev: metadata.dev, ino: metadata.ino }),
  };
}

function assertDirectoryIdentity(directory, label) {
  const metadata = lstatSync(directory.canonicalPath, { bigint: true });
  if (
    !metadata.isDirectory() ||
    metadata.isSymbolicLink() ||
    metadata.dev !== directory.identity.dev ||
    metadata.ino !== directory.identity.ino ||
    realpathSync(directory.canonicalPath) !== directory.canonicalPath
  ) {
    throw new Error(`${label} changed identity.`);
  }
}

function currentEffectiveUid() {
  return typeof process.geteuid === "function" ? process.geteuid() : null;
}

function runDirectoryIdentity(metadata) {
  return Object.freeze({
    birthtime_ns: String(metadata.birthtimeNs),
    dev: String(metadata.dev),
    ino: String(metadata.ino),
  });
}

function sameRunDirectoryIdentity(left, right) {
  return (
    left?.birthtime_ns === right?.birthtime_ns &&
    left?.dev === right?.dev &&
    left?.ino === right?.ino
  );
}

function exactRunOwnerKeys(value) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.getPrototypeOf(value) !== Object.prototype
  ) {
    return false;
  }
  const actual = Object.keys(value).sort();
  const expected = [
    "directory",
    "host",
    "pid",
    "run_id",
    "run_name",
    "uid",
    "version",
  ].sort();
  return (
    actual.length === expected.length &&
    actual.every((key, index) => key === expected[index])
  );
}

function validateRunOwner(value, runId) {
  if (!exactRunOwnerKeys(value)) {
    throw new Error("Native build run owner has an invalid schema.");
  }
  const directory = value.directory;
  if (
    value.version !== RUN_OWNER_VERSION ||
    value.run_id !== runId ||
    value.run_name !== `${RUN_CONTAINER_PREFIX}${runId}` ||
    typeof value.host !== "string" ||
    value.host.length === 0 ||
    value.host.length > 255 ||
    value.host.includes("\0") ||
    !Number.isSafeInteger(value.pid) ||
    value.pid <= 0 ||
    (value.uid !== null &&
      (!Number.isSafeInteger(value.uid) || value.uid < 0)) ||
    directory === null ||
    typeof directory !== "object" ||
    Array.isArray(directory) ||
    Object.keys(directory).sort().join("\0") !==
      ["birthtime_ns", "dev", "ino"].join("\0") ||
    Object.values(directory).some(
      (entry) =>
        typeof entry !== "string" ||
        !/^(?:0|[1-9][0-9]*)$/u.test(entry),
    )
  ) {
    throw new Error("Native build run owner is invalid.");
  }
  return Object.freeze({
    directory: Object.freeze({ ...directory }),
    host: value.host,
    pid: value.pid,
    run_id: value.run_id,
    run_name: value.run_name,
    uid: value.uid,
    version: value.version,
  });
}

function assertPrivateRunMetadata(metadata, expectedUid, label, kind) {
  if (expectedUid === null) return;
  if (
    metadata.uid !== BigInt(expectedUid) ||
    (metadata.mode & (kind === "directory" ? 0o077n : 0o177n)) !== 0n
  ) {
    throw new Error(`${label} is not private to its recorded owner.`);
  }
}

function readRunOwnerFile(path, runId, label) {
  const snapshot = readStableRegularFile(path, {
    label,
    maximumBytes: MAX_RUN_OWNER_BYTES,
    requireNonempty: true,
  });
  let parsed;
  try {
    parsed = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(snapshot.bytes),
    );
  } catch {
    throw new Error(`${label} is malformed.`);
  }
  const owner = validateRunOwner(parsed, runId);
  const metadata = lstatSync(path, { bigint: true });
  if (
    !sameOutputIdentity(metadata, snapshot.identity) ||
    realpathSync(path) !== path
  ) {
    throw new Error(`${label} changed identity.`);
  }
  assertPrivateRunMetadata(metadata, owner.uid, label, "file");
  return Object.freeze({ identity: snapshot.identity, owner });
}

function inspectRunPayloadChildren(path, names, label) {
  const payloadNames = names.filter((name) => name !== RUN_OWNER_FILENAME);
  const snapshotNames = payloadNames.filter((name) =>
    RUN_SNAPSHOT_PATTERN.test(name),
  );
  if (
    payloadNames.length > 2 ||
    snapshotNames.length > 1 ||
    payloadNames.some(
      (name) =>
        name !== RUN_CARGO_TARGET_NAME &&
        !RUN_SNAPSHOT_PATTERN.test(name),
    )
  ) {
    throw new Error(`${label} has an invalid direct-child inventory.`);
  }
  const children = new Map();
  for (const name of payloadNames) {
    const childPath = join(path, name);
    const metadata = lstatSync(childPath, { bigint: true });
    if (
      !metadata.isDirectory() ||
      metadata.isSymbolicLink() ||
      realpathSync(childPath) !== childPath
    ) {
      throw new Error(`${label} has an unsafe direct child.`);
    }
    children.set(
      name,
      Object.freeze({ dev: metadata.dev, ino: metadata.ino }),
    );
  }
  return children;
}

function sameRunPayloadChildren(left, right) {
  if (left.size !== right.size) return false;
  for (const [name, identity] of left) {
    if (!sameDirectoryIdentity(identity, right.get(name))) return false;
  }
  return true;
}

function readOwnedRunDirectory(
  path,
  parent,
  runId,
  label,
  { initializer = false } = {},
) {
  assertDirectoryIdentity(
    parent,
    "Native build operating-system temporary directory",
  );
  const before = lstatSync(path, { bigint: true });
  if (
    dirname(path) !== parent.canonicalPath ||
    !before.isDirectory() ||
    before.isSymbolicLink() ||
    realpathSync(path) !== path ||
    realpathSync(dirname(path)) !== parent.canonicalPath
  ) {
    throw new Error(`${label} is unsafe.`);
  }
  const namesBefore = readdirSync(path).sort();
  if (
    !namesBefore.includes(RUN_OWNER_FILENAME) ||
    (initializer &&
      (namesBefore.length !== 1 || namesBefore[0] !== RUN_OWNER_FILENAME))
  ) {
    throw new Error(`${label} has no complete ownership inventory.`);
  }
  const childrenBefore = initializer
    ? new Map()
    : inspectRunPayloadChildren(path, namesBefore, label);
  const ownerSnapshot = readRunOwnerFile(
    join(path, RUN_OWNER_FILENAME),
    runId,
    `${label} owner`,
  );
  const after = lstatSync(path, { bigint: true });
  const namesAfter = readdirSync(path).sort();
  const childrenAfter = initializer
    ? new Map()
    : inspectRunPayloadChildren(path, namesAfter, label);
  if (
    !sameDirectoryIdentity(before, after) ||
    !after.isDirectory() ||
    after.isSymbolicLink() ||
    namesBefore.length !== namesAfter.length ||
    namesBefore.some((name, index) => name !== namesAfter[index]) ||
    !sameRunPayloadChildren(childrenBefore, childrenAfter) ||
    !sameRunDirectoryIdentity(
      runDirectoryIdentity(after),
      ownerSnapshot.owner.directory,
    ) ||
    realpathSync(path) !== path
  ) {
    throw new Error(`${label} changed while it was inspected.`);
  }
  assertPrivateRunMetadata(
    after,
    ownerSnapshot.owner.uid,
    label,
    "directory",
  );
  return Object.freeze({
    children: childrenAfter,
    identity: Object.freeze({ dev: after.dev, ino: after.ino }),
    ownerIdentity: ownerSnapshot.identity,
    path,
    plan: ownerSnapshot.owner,
    runId,
  });
}

function writeRunOwner(path, runId, failpoint) {
  const metadata = lstatSync(path, { bigint: true });
  const owner = Object.freeze({
    version: RUN_OWNER_VERSION,
    run_id: runId,
    run_name: `${RUN_CONTAINER_PREFIX}${runId}`,
    host: hostname(),
    pid: process.pid,
    uid: currentEffectiveUid(),
    directory: runDirectoryIdentity(metadata),
  });
  const ownerPath = join(path, RUN_OWNER_FILENAME);
  const descriptor = openSync(
    ownerPath,
    constants.O_CREAT |
      constants.O_EXCL |
      constants.O_WRONLY |
      (constants.O_CLOEXEC ?? 0) |
      (constants.O_NOFOLLOW ?? 0),
    0o600,
  );
  try {
    const bytes = Buffer.from(`${JSON.stringify(owner)}\n`, "utf8");
    let offset = 0;
    while (offset < bytes.length) {
      const written = writeSync(
        descriptor,
        bytes,
        offset,
        bytes.length - offset,
        null,
      );
      if (written <= 0) {
        throw new Error("Native build run owner write made no progress.");
      }
      offset += written;
    }
    failpoint("run-owner-written", { ownerPath, path, runId });
    fsyncSync(descriptor);
  } finally {
    closeSync(descriptor);
  }
  syncDirectory(path);
  failpoint("run-owner-synced", { ownerPath, path, runId });
}

function createRunContainer(
  repoRoot,
  targetRoot,
  temporaryParent,
  failpoint,
) {
  const runUuid = newRunUuid();
  const initializerUuid = newRunUuid();
  const initializerPath = join(
    temporaryParent.canonicalPath,
    `${RUN_INITIALIZER_PREFIX}${runUuid}-${initializerUuid}`,
  );
  const runContainer = join(
    temporaryParent.canonicalPath,
    `${RUN_CONTAINER_PREFIX}${runUuid}`,
  );
  assertDirectoryIdentity(
    temporaryParent,
    "Native build operating-system temporary directory",
  );
  mkdirSync(initializerPath, { mode: 0o700 });
  failpoint("run-initializer-created", {
    initializerPath,
    runId: runUuid,
  });
  writeRunOwner(initializerPath, runUuid, failpoint);
  const initialized = readOwnedRunDirectory(
    initializerPath,
    temporaryParent,
    runUuid,
    "Native build run initializer",
    { initializer: true },
  );
  assertDirectoryIdentity(
    temporaryParent,
    "Native build operating-system temporary directory",
  );
  if (lstatOrNull(runContainer) !== null) {
    throw new Error("Native build run container path already exists.");
  }
  renameSync(initializerPath, runContainer);
  failpoint("run-container-renamed", {
    path: runContainer,
    runId: runUuid,
  });
  syncDirectory(temporaryParent.canonicalPath);
  failpoint("run-container-created", {
    path: runContainer,
    runId: runUuid,
  });
  const created = readOwnedRunDirectory(
    runContainer,
    temporaryParent,
    runUuid,
    "Native build private run container",
  );
  if (
    !sameDirectoryIdentity(created.identity, initialized.identity) ||
    !sameRenamedOutputIdentity(
      created.ownerIdentity,
      initialized.ownerIdentity,
    )
  ) {
    throw new Error("Native build run container changed during publication.");
  }
  assertDisjointPathAncestry(
    repoRoot,
    runContainer,
    "Native build run container and repository",
  );
  assertDisjointPathAncestry(
    targetRoot,
    runContainer,
    "Native build run container and publication target directory",
  );
  return Object.freeze({
    identity: created.identity,
    ownerIdentity: created.ownerIdentity,
    parent: temporaryParent.canonicalPath,
    parentIdentity: temporaryParent.identity,
    path: runContainer,
    plan: created.plan,
    uuid: runUuid,
  });
}

function assertRunContainerIdentity(container) {
  if (
    typeof container?.path !== "string" ||
    typeof container?.parent !== "string" ||
    typeof container?.uuid !== "string" ||
    !UUID_PATTERN.test(container.uuid) ||
    dirname(container.path) !== container.parent ||
    basename(container.path) !== `${RUN_CONTAINER_PREFIX}${container.uuid}`
  ) {
    throw new Error("Native build run container identity is invalid.");
  }
  const parent = Object.freeze({
    canonicalPath: container.parent,
    identity: container.parentIdentity,
  });
  assertDirectoryIdentity(
    parent,
    "Native build operating-system temporary directory",
  );
  const current = readOwnedRunDirectory(
    container.path,
    parent,
    container.uuid,
    "Native build private run container",
  );
  if (
    !sameDirectoryIdentity(current.identity, container.identity) ||
    !sameOutputIdentity(current.ownerIdentity, container.ownerIdentity) ||
    current.plan.host !== container.plan.host ||
    current.plan.pid !== container.plan.pid ||
    current.plan.uid !== container.plan.uid
  ) {
    throw new Error("Native build private run container changed identity.");
  }
  return current;
}

function runOwnerIsDefinitelyDead(run, ownerIsAlive) {
  if (
    run.plan.host !== hostname() ||
    run.plan.uid !== currentEffectiveUid()
  ) {
    return false;
  }
  try {
    return ownerIsAlive(run.plan.pid) === false;
  } catch {
    return false;
  }
}

function runBelongsToCurrentPrincipal(run) {
  return (
    run.plan.host === hostname() &&
    run.plan.uid === currentEffectiveUid()
  );
}

function exactRunTrashPaths(parent, runId, trashId) {
  if (!UUID_PATTERN.test(runId) || !UUID_PATTERN.test(trashId)) {
    throw new Error("Native build run trash identity is invalid.");
  }
  const baseName = `${RUN_TRASH_PREFIX}${runId}-${trashId}`;
  return Object.freeze({
    markerPath: join(
      parent.canonicalPath,
      `${baseName}${RUN_TRASH_OWNER_SUFFIX}`,
    ),
    path: join(parent.canonicalPath, baseName),
  });
}

function readRunTrashMarker(markerPath, parent, runId, trashId) {
  assertDirectoryIdentity(
    parent,
    "Native build operating-system temporary directory",
  );
  const expected = exactRunTrashPaths(parent, runId, trashId);
  if (markerPath !== expected.markerPath) {
    throw new Error("Native build run trash marker path is invalid.");
  }
  const ownerSnapshot = readRunOwnerFile(
    markerPath,
    runId,
    "Native build run trash marker",
  );
  if (
    ownerSnapshot.owner.host !== hostname() ||
    ownerSnapshot.owner.uid !== currentEffectiveUid()
  ) {
    throw new Error("Native build run trash marker belongs to another owner.");
  }
  const rootMetadata = lstatOrNull(expected.path);
  if (rootMetadata !== null) {
    if (
      !rootMetadata.isDirectory() ||
      rootMetadata.isSymbolicLink() ||
      !sameRunDirectoryIdentity(
        runDirectoryIdentity(rootMetadata),
        ownerSnapshot.owner.directory,
      ) ||
      realpathSync(expected.path) !== expected.path ||
      readdirSync(expected.path).length !== 0
    ) {
      throw new Error("Native build terminal run trash root is unsafe.");
    }
    assertPrivateRunMetadata(
      rootMetadata,
      ownerSnapshot.owner.uid,
      "Native build terminal run trash root",
      "directory",
    );
  }
  return Object.freeze({
    markerIdentity: ownerSnapshot.identity,
    markerPath,
    path: expected.path,
    plan: ownerSnapshot.owner,
    runId,
    trashId,
  });
}

function openRunOwnerWitness(path, identity, label) {
  // Keep the exact owner inode open across retirement. A competing janitor
  // only counts as converged when this descriptor reports that same inode
  // unlinked; pathname ENOENT alone is not ownership evidence.
  const descriptor = openSync(
    path,
    constants.O_RDONLY |
      (constants.O_CLOEXEC ?? 0) |
      (constants.O_NOFOLLOW ?? 0),
  );
  try {
    const opened = fstatSync(descriptor, { bigint: true });
    if (
      !opened.isFile() ||
      opened.isSymbolicLink() ||
      opened.nlink !== 1n ||
      !sameOutputIdentity(opened, identity)
    ) {
      throw new Error(`${label} changed while its retirement was witnessed.`);
    }
  } catch (error) {
    closeSync(descriptor);
    throw error;
  }
  return Object.freeze({ descriptor, identity });
}

function runOwnerWitnessProvesRemoval(witness) {
  const current = fstatSync(witness.descriptor, { bigint: true });
  return (
    current.isFile() &&
    !current.isSymbolicLink() &&
    current.nlink === 0n &&
    current.dev === witness.identity.dev &&
    current.ino === witness.identity.ino &&
    current.mode === witness.identity.mode &&
    current.mtimeNs === witness.identity.mtimeNs &&
    current.size === witness.identity.size
  );
}

function readRunTrashMarkerOrCompleted(
  markerPath,
  parent,
  runId,
  trashId,
  witness,
) {
  try {
    return readRunTrashMarker(
      markerPath,
      parent,
      runId,
      trashId,
    );
  } catch (error) {
    assertDirectoryIdentity(
      parent,
      "Native build operating-system temporary directory",
    );
    const paths = exactRunTrashPaths(parent, runId, trashId);
    if (
      lstatOrNull(paths.markerPath) === null &&
      lstatOrNull(paths.path) === null &&
      witness !== undefined &&
      runOwnerWitnessProvesRemoval(witness)
    ) {
      return null;
    }
    throw error;
  }
}

function completeRunTrashMarker(
  marker,
  parent,
  failpoint,
  suppliedWitness,
) {
  const witness =
    suppliedWitness ??
    openRunOwnerWitness(
      marker.markerPath,
      marker.markerIdentity,
      "Native build run trash marker",
    );
  const closeWitness = suppliedWitness === undefined;
  try {
    if (
      !sameRenamedOutputIdentity(
        marker.markerIdentity,
        witness.identity,
      )
    ) {
      throw new Error(
        "Native build run trash marker does not match its retirement witness.",
      );
    }
    let current = readRunTrashMarkerOrCompleted(
      marker.markerPath,
      parent,
      marker.runId,
      marker.trashId,
      witness,
    );
    if (current === null) return;
    if (lstatOrNull(current.path) !== null) {
      assertDirectoryIdentity(
        parent,
        "Native build operating-system temporary directory",
      );
      try {
        rmdirSync(current.path);
      } catch (error) {
        if (
          (error?.code !== "ENOENT" && error?.code !== "ENOTDIR") ||
          lstatOrNull(current.path) !== null
        ) {
          throw error;
        }
      }
      syncDirectory(parent.canonicalPath);
      failpoint("run-trash-directory-removed", {
        markerPath: current.markerPath,
        path: current.path,
        runId: current.runId,
      });
    }
    current = readRunTrashMarkerOrCompleted(
      marker.markerPath,
      parent,
      marker.runId,
      marker.trashId,
      witness,
    );
    if (current === null) return;
    assertDirectoryIdentity(
      parent,
      "Native build operating-system temporary directory",
    );
    try {
      rmSync(current.markerPath);
    } catch (error) {
      if (
        (error?.code !== "ENOENT" && error?.code !== "ENOTDIR") ||
        lstatOrNull(current.markerPath) !== null ||
        lstatOrNull(current.path) !== null ||
        !runOwnerWitnessProvesRemoval(witness)
      ) {
        throw error;
      }
      return;
    }
    syncDirectory(parent.canonicalPath);
    failpoint("run-trash-marker-removed", {
      markerPath: current.markerPath,
      runId: current.runId,
    });
  } finally {
    if (closeWitness) closeSync(witness.descriptor);
  }
}

function completeConvergedRunTrash(
  paths,
  parent,
  runId,
  trashId,
  failpoint,
  witness,
) {
  assertDirectoryIdentity(
    parent,
    "Native build operating-system temporary directory",
  );
  if (lstatOrNull(paths.markerPath) !== null) {
    const marker = readRunTrashMarkerOrCompleted(
      paths.markerPath,
      parent,
      runId,
      trashId,
      witness,
    );
    if (marker !== null) {
      if (
        !sameRenamedOutputIdentity(
          marker.markerIdentity,
          witness.identity,
        )
      ) {
        throw new Error(
          "Native build run trash marker changed retirement identity.",
        );
      }
      completeRunTrashMarker(marker, parent, failpoint, witness);
    }
    return true;
  }
  return (
    lstatOrNull(paths.path) === null &&
    runOwnerWitnessProvesRemoval(witness)
  );
}

function removeOwnedRunTrash(trash, parent, trashId, failpoint) {
  const paths = exactRunTrashPaths(parent, trash.runId, trashId);
  if (trash.path !== paths.path) {
    throw new Error("Native build run trash ownership is ambiguous.");
  }
  const witness = openRunOwnerWitness(
    join(trash.path, RUN_OWNER_FILENAME),
    trash.ownerIdentity,
    "Native build run trash owner",
  );
  try {
    let current;
    try {
      current = readOwnedRunDirectory(
        trash.path,
        parent,
        trash.runId,
        "Native build owned run trash",
      );
    } catch (error) {
      if (
        completeConvergedRunTrash(
          paths,
          parent,
          trash.runId,
          trashId,
          failpoint,
          witness,
        )
      ) {
        return;
      }
      throw error;
    }
    if (!runBelongsToCurrentPrincipal(current)) {
      throw new Error("Native build run trash belongs to another owner.");
    }
    if (lstatOrNull(paths.markerPath) !== null) {
      throw new Error("Native build run trash ownership is ambiguous.");
    }
    for (const name of [...current.children.keys()].sort()) {
      try {
        current = readOwnedRunDirectory(
          current.path,
          parent,
          current.runId,
          "Native build owned run trash",
        );
      } catch (error) {
        if (
          completeConvergedRunTrash(
            paths,
            parent,
            trash.runId,
            trashId,
            failpoint,
            witness,
          )
        ) {
          return;
        }
        throw error;
      }
      const expectedChild = current.children.get(name);
      if (expectedChild === undefined) continue;
      const childPath = join(current.path, name);
      const child = lstatOrNull(childPath);
      if (child === null) continue;
      if (
        !child.isDirectory() ||
        child.isSymbolicLink() ||
        !sameDirectoryIdentity(child, expectedChild) ||
        realpathSync(childPath) !== childPath
      ) {
        throw new Error(
          "Native build run trash child changed before removal.",
        );
      }
      // The root and exact direct child have just been revalidated. Recursive
      // payload cleanup is confined beneath this already-retired directory.
      // Same-UID peers are trusted because Node has no portable descriptor-
      // relative recursive removal API on POSIX and Windows.
      rmSync(childPath, { recursive: true });
      syncDirectory(current.path);
      failpoint(`run-payload-removed:${name}`, {
        path: current.path,
        runId: current.runId,
      });
    }
    try {
      current = readOwnedRunDirectory(
        current.path,
        parent,
        current.runId,
        "Native build emptied run trash",
      );
    } catch (error) {
      if (
        completeConvergedRunTrash(
          paths,
          parent,
          trash.runId,
          trashId,
          failpoint,
          witness,
        )
      ) {
        return;
      }
      throw error;
    }
    if (current.children.size !== 0) {
      throw new Error(
        "Native build run trash payload cleanup is incomplete.",
      );
    }
    assertDirectoryIdentity(
      parent,
      "Native build operating-system temporary directory",
    );
    try {
      renameSync(join(current.path, RUN_OWNER_FILENAME), paths.markerPath);
    } catch (error) {
      if (
        completeConvergedRunTrash(
          paths,
          parent,
          trash.runId,
          trashId,
          failpoint,
          witness,
        )
      ) {
        return;
      }
      throw error;
    }
    syncDirectory(current.path);
    syncDirectory(parent.canonicalPath);
    const marker = readRunTrashMarkerOrCompleted(
      paths.markerPath,
      parent,
      current.runId,
      trashId,
      witness,
    );
    if (marker === null) return;
    if (
      !sameRenamedOutputIdentity(
        marker.markerIdentity,
        current.ownerIdentity,
      )
    ) {
      throw new Error(
        "Native build run owner changed during terminal retirement.",
      );
    }
    failpoint("run-owner-retired", {
      markerPath: paths.markerPath,
      path: paths.path,
      runId: current.runId,
    });
    completeRunTrashMarker(marker, parent, failpoint, witness);
  } finally {
    closeSync(witness.descriptor);
  }
}

function retireOwnedRun(run, parent, failpoint) {
  let current = readOwnedRunDirectory(
    run.path,
    parent,
    run.runId,
    "Native build owned run container",
    { initializer: basename(run.path).startsWith(RUN_INITIALIZER_PREFIX) },
  );
  const witness = openRunOwnerWitness(
    join(current.path, RUN_OWNER_FILENAME),
    current.ownerIdentity,
    "Native build owned run owner",
  );
  let trash;
  let trashId;
  let trashPaths;
  try {
    failpoint("run-stale-verified", {
      path: current.path,
      runId: current.runId,
    });
    let verified;
    try {
      verified = readOwnedRunDirectory(
        current.path,
        parent,
        current.runId,
        "Native build owned run container",
        {
          initializer: basename(current.path).startsWith(
            RUN_INITIALIZER_PREFIX,
          ),
        },
      );
    } catch (error) {
      assertDirectoryIdentity(
        parent,
        "Native build operating-system temporary directory",
      );
      if (
        lstatOrNull(current.path) === null &&
        runOwnerWitnessProvesRemoval(witness)
      ) {
        return;
      }
      throw error;
    }
    if (
      !sameDirectoryIdentity(current.identity, verified.identity) ||
      !sameOutputIdentity(current.ownerIdentity, verified.ownerIdentity)
    ) {
      throw new Error("Native build owned run changed before retirement.");
    }
    current = verified;
    trashId = newRunUuid();
    trashPaths = exactRunTrashPaths(parent, current.runId, trashId);
    if (
      lstatOrNull(trashPaths.path) !== null ||
      lstatOrNull(trashPaths.markerPath) !== null
    ) {
      throw new Error("Native build run trash target already exists.");
    }
    assertDirectoryIdentity(
      parent,
      "Native build operating-system temporary directory",
    );
    try {
      renameSync(current.path, trashPaths.path);
    } catch (error) {
      assertDirectoryIdentity(
        parent,
        "Native build operating-system temporary directory",
      );
      if (
        lstatOrNull(current.path) === null &&
        runOwnerWitnessProvesRemoval(witness)
      ) {
        return;
      }
      throw error;
    }
    failpoint("run-trash-renamed", {
      path: trashPaths.path,
      runId: current.runId,
    });
    syncDirectory(parent.canonicalPath);
    failpoint("run-trash-synced", {
      path: trashPaths.path,
      runId: current.runId,
    });
    try {
      trash = readOwnedRunDirectory(
        trashPaths.path,
        parent,
        current.runId,
        "Native build retired run trash",
      );
    } catch (error) {
      if (
        completeConvergedRunTrash(
          trashPaths,
          parent,
          current.runId,
          trashId,
          failpoint,
          witness,
        )
      ) {
        return;
      }
      throw error;
    }
    if (
      !sameDirectoryIdentity(trash.identity, current.identity) ||
      !sameRenamedOutputIdentity(
        trash.ownerIdentity,
        current.ownerIdentity,
      )
    ) {
      throw new Error("Native build run changed during trash retirement.");
    }
  } finally {
    closeSync(witness.descriptor);
  }
  removeOwnedRunTrash(trash, parent, trashId, failpoint);
}

function tryReadOwnedRun(path, parent, runId, options) {
  try {
    return readOwnedRunDirectory(
      path,
      parent,
      runId,
      "Native build recoverable run",
      options,
    );
  } catch {
    return undefined;
  }
}

function recoverNativeBuildRunContainers(
  parent,
  ownerIsAlive,
  failpoint,
) {
  assertDirectoryIdentity(
    parent,
    "Native build operating-system temporary directory",
  );
  const names = readdirSync(parent.canonicalPath).sort();
  for (const name of names) {
    const match = RUN_TRASH_OWNER_PATTERN.exec(name);
    if (match === null) continue;
    let marker;
    try {
      marker = readRunTrashMarker(
        join(parent.canonicalPath, name),
        parent,
        match[2],
        match[3],
      );
    } catch {
      // Malformed, foreign, ambiguous, or replaced terminal markers remain
      // untouched. They are never interpreted as active run containers.
      continue;
    }
    completeRunTrashMarker(marker, parent, failpoint);
  }
  for (const name of names) {
    const match = RUN_TRASH_PATTERN.exec(name);
    if (match === null) continue;
    const path = join(parent.canonicalPath, name);
    const trash = tryReadOwnedRun(path, parent, match[1]);
    if (trash === undefined || !runBelongsToCurrentPrincipal(trash)) continue;
    const markerPath = `${path}${RUN_TRASH_OWNER_SUFFIX}`;
    if (lstatOrNull(markerPath) !== null) continue;
    removeOwnedRunTrash(trash, parent, match[2], failpoint);
  }
  for (const name of names) {
    const initializer = RUN_INITIALIZER_PATTERN.exec(name);
    const final = RUN_CONTAINER_PATTERN.exec(name);
    if (initializer === null && final === null) continue;
    const runId = initializer?.[1] ?? final[1];
    const run = tryReadOwnedRun(
      join(parent.canonicalPath, name),
      parent,
      runId,
      { initializer: initializer !== null },
    );
    if (
      run === undefined ||
      !runOwnerIsDefinitelyDead(run, ownerIsAlive)
    ) {
      continue;
    }
    retireOwnedRun(run, parent, failpoint);
  }
}

function cleanupRunContainer(container, failpoint) {
  const current = assertRunContainerIdentity(container);
  const parent = Object.freeze({
    canonicalPath: container.parent,
    identity: container.parentIdentity,
  });
  retireOwnedRun(current, parent, failpoint);
}

function assertSnapshotPlacement(snapshot, container) {
  if (
    typeof snapshot?.snapshotRoot !== "string" ||
    typeof snapshot?.targetRoot !== "string" ||
    resolve(snapshot.targetRoot) !== container.path ||
    dirname(resolve(snapshot.snapshotRoot)) !== container.path ||
    realpathSync(dirname(snapshot.snapshotRoot)) !== container.path
  ) {
    throw new Error(
      "Native build source snapshot must be a direct child of its private temporary container.",
    );
  }
}

function createRunCargoTarget(container) {
  assertRunContainerIdentity(container);
  const targetPath = join(container.path, RUN_CARGO_TARGET_NAME);
  mkdirSync(targetPath, { mode: 0o700 });
  const canonical = canonicalDirectory(
    targetPath,
    "Native build private Cargo target",
  );
  if (
    canonical.canonicalPath !== targetPath ||
    dirname(canonical.canonicalPath) !== container.path ||
    basename(canonical.canonicalPath) !== RUN_CARGO_TARGET_NAME
  ) {
    throw new Error("Native build private Cargo target placement is invalid.");
  }
  syncDirectory(container.path);
  return Object.freeze({
    identity: canonical.identity,
    path: canonical.canonicalPath,
    runContainerPath: container.path,
    runUuid: container.uuid,
  });
}

function assertRunCargoTargetIdentity(target, container) {
  assertRunContainerIdentity(container);
  if (
    typeof target?.path !== "string" ||
    target.runContainerPath !== container.path ||
    target.runUuid !== container.uuid ||
    target.path !== join(container.path, RUN_CARGO_TARGET_NAME)
  ) {
    throw new Error("Native build private Cargo target identity is invalid.");
  }
  const metadata = lstatSync(target.path, { bigint: true });
  if (
    !metadata.isDirectory() ||
    metadata.isSymbolicLink() ||
    metadata.dev !== target.identity.dev ||
    metadata.ino !== target.identity.ino ||
    realpathSync(target.path) !== target.path
  ) {
    throw new Error("Native build private Cargo target changed identity.");
  }
  return metadata;
}

function cleanupRunCargoTarget(target, container) {
  assertRunCargoTargetIdentity(target, container);
  rmSync(target.path, { recursive: true });
  syncDirectory(container.path);
}

function cargoArtifactSourceIdentity(path) {
  const metadata = lstatSync(path, { bigint: true });
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    metadata.nlink < 1n ||
    realpathSync(path) !== resolve(path)
  ) {
    throw new Error(
      "Cargo native build artifact must be a canonical regular file.",
    );
  }
  return Object.freeze({
    ctimeNs: metadata.ctimeNs,
    dev: metadata.dev,
    ino: metadata.ino,
    mode: metadata.mode,
    mtimeNs: metadata.mtimeNs,
    nlink: metadata.nlink,
    size: metadata.size,
  });
}

function digestCargoArtifactSource(sourcePath, expectedIdentity) {
  const before = cargoArtifactSourceIdentity(sourcePath);
  if (
    expectedIdentity !== undefined &&
    !sameOutputIdentity(before, expectedIdentity)
  ) {
    throw new Error(
      "Cargo native build artifact changed before digest verification.",
    );
  }
  const descriptor = openSync(
    sourcePath,
    constants.O_RDONLY |
      (constants.O_NOFOLLOW ?? 0) |
      (constants.O_CLOEXEC ?? 0),
  );
  const digest = createHash("sha256");
  let total = 0n;
  let openedAfter;
  try {
    const openedBefore = fstatSync(descriptor, { bigint: true });
    if (
      !openedBefore.isFile() ||
      openedBefore.isSymbolicLink() ||
      openedBefore.nlink < 1n ||
      !sameOutputIdentity(before, openedBefore)
    ) {
      throw new Error(
        "Cargo native build artifact changed while it was opened for digest verification.",
      );
    }
    const buffer = Buffer.allocUnsafe(64 * 1024);
    let bytesRead;
    do {
      bytesRead = readSync(descriptor, buffer, 0, buffer.length, null);
      if (bytesRead > 0) {
        digest.update(buffer.subarray(0, bytesRead));
        total += BigInt(bytesRead);
      }
    } while (bytesRead > 0);
    openedAfter = fstatSync(descriptor, { bigint: true });
  } finally {
    closeSync(descriptor);
  }
  const after = cargoArtifactSourceIdentity(sourcePath);
  if (
    openedAfter === undefined ||
    total !== before.size ||
    !sameOutputIdentity(before, openedAfter) ||
    !sameOutputIdentity(before, after)
  ) {
    throw new Error(
      "Cargo native build artifact changed during digest verification.",
    );
  }
  return Object.freeze({
    identity: before,
    sha256: digest.digest("hex"),
  });
}

function copyCargoArtifactToOwnedSeal(
  sourcePath,
  target,
  container,
  failpoint,
) {
  assertRunCargoTargetIdentity(target, container);
  const sealPath = join(target.path, RUN_CARGO_OUTPUT_SEAL_NAME);
  if (lstatOrNull(sealPath) !== null) {
    throw new Error("Native build private Cargo output seal already exists.");
  }
  const sourceDigestBefore = digestCargoArtifactSource(sourcePath);
  const sourceBefore = sourceDigestBefore.identity;
  failpoint("cargo-output-source-digested", { sealPath, sourcePath });
  const noFollow = constants.O_NOFOLLOW ?? 0;
  const closeOnExec = constants.O_CLOEXEC ?? 0;
  const sourceDescriptor = openSync(
    sourcePath,
    constants.O_RDONLY | noFollow | closeOnExec,
  );
  let sealDescriptor;
  let sealCreationIdentity;
  let copyError;
  let copied = 0n;
  const copiedDigest = createHash("sha256");
  try {
    const sourceOpened = fstatSync(sourceDescriptor, { bigint: true });
    if (
      !sourceOpened.isFile() ||
      sourceOpened.isSymbolicLink() ||
      sourceOpened.nlink < 1n ||
      !sameOutputIdentity(sourceBefore, sourceOpened)
    ) {
      throw new Error("Cargo native build artifact changed while it was opened.");
    }
    sealDescriptor = openSync(
      sealPath,
      constants.O_CREAT |
        constants.O_EXCL |
        constants.O_WRONLY |
        noFollow |
        closeOnExec,
      0o600,
    );
    const sealOpened = fstatSync(sealDescriptor, { bigint: true });
    if (!sealOpened.isFile() || sealOpened.nlink !== 1n) {
      throw new Error("Native build private output seal must be singly linked.");
    }
    sealCreationIdentity = Object.freeze({
      dev: sealOpened.dev,
      ino: sealOpened.ino,
    });
    const buffer = Buffer.allocUnsafe(64 * 1024);
    let bytesRead;
    do {
      bytesRead = readSync(
        sourceDescriptor,
        buffer,
        0,
        buffer.length,
        null,
      );
      if (bytesRead > 0) {
        copiedDigest.update(buffer.subarray(0, bytesRead));
      }
      let written = 0;
      while (written < bytesRead) {
        const count = writeSync(
          sealDescriptor,
          buffer,
          written,
          bytesRead - written,
          null,
        );
        if (count <= 0) {
          throw new Error("Native build output seal copy made no progress.");
        }
        written += count;
      }
      copied += BigInt(bytesRead);
    } while (bytesRead > 0);
    fchmodSync(sealDescriptor, Number(sourceOpened.mode & 0o777n));
    fsyncSync(sealDescriptor);
    const sourceAfterOpen = fstatSync(sourceDescriptor, { bigint: true });
    const sealAfterOpen = fstatSync(sealDescriptor, { bigint: true });
    if (
      copied !== sourceBefore.size ||
      !sameOutputIdentity(sourceBefore, sourceAfterOpen) ||
      !sealAfterOpen.isFile() ||
      sealAfterOpen.nlink !== 1n ||
      sealAfterOpen.size !== copied
    ) {
      throw new Error(
        "Cargo native build artifact changed while it was privately sealed.",
      );
    }
    if (copiedDigest.digest("hex") !== sourceDigestBefore.sha256) {
      throw new Error(
        "Cargo native build artifact digest changed while it was privately sealed.",
      );
    }
  } catch (error) {
    copyError = error;
  } finally {
    closeSync(sourceDescriptor);
    if (sealDescriptor !== undefined) closeSync(sealDescriptor);
  }
  if (copyError !== undefined) {
    if (sealCreationIdentity !== undefined) {
      const failedSeal = lstatOrNull(sealPath);
      if (
        failedSeal === null ||
        !failedSeal.isFile() ||
        failedSeal.isSymbolicLink() ||
        failedSeal.nlink !== 1n ||
        failedSeal.dev !== sealCreationIdentity.dev ||
        failedSeal.ino !== sealCreationIdentity.ino
      ) {
        throw new AggregateError(
          [copyError],
          "Native build output sealing failed and its partial copy changed identity.",
        );
      }
      rmSync(sealPath);
      syncDirectory(target.path);
    }
    throw copyError;
  }
  let postCopyError;
  let sourceDigestAfter;
  let seal;
  try {
    failpoint("cargo-output-copied", { sealPath, sourcePath });
    sourceDigestAfter = digestCargoArtifactSource(
      sourcePath,
      sourceBefore,
    );
    if (sourceDigestAfter.sha256 !== sourceDigestBefore.sha256) {
      throw new Error(
        "Cargo native build artifact digest changed after private sealing.",
      );
    }
    syncDirectory(target.path);
    seal = sealNativeOutput(sealPath);
    if (
      seal.identity.size !== copied ||
      seal.sha256 !== sourceDigestBefore.sha256
    ) {
      throw new Error(
        "Native build private output seal does not match its Cargo artifact.",
      );
    }
  } catch (error) {
    postCopyError = error;
  }
  if (postCopyError !== undefined) {
    const failedSeal = lstatOrNull(sealPath);
    if (
      failedSeal === null ||
      !failedSeal.isFile() ||
      failedSeal.isSymbolicLink() ||
      failedSeal.nlink !== 1n ||
      failedSeal.dev !== sealCreationIdentity.dev ||
      failedSeal.ino !== sealCreationIdentity.ino
    ) {
      throw new AggregateError(
        [postCopyError],
        "Native build output post-copy validation failed and its private seal changed identity.",
      );
    }
    rmSync(sealPath);
    syncDirectory(target.path);
    throw postCopyError;
  }
  return Object.freeze({ path: sealPath, seal });
}

function outputIdentityOrNull(nativePath) {
  let metadata;
  try {
    metadata = lstatSync(nativePath, { bigint: true });
  } catch (error) {
    if (error?.code === "ENOENT" || error?.code === "ENOTDIR") return null;
    throw error;
  }
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    metadata.nlink !== 1n ||
    realpathSync(nativePath) !== resolve(nativePath)
  ) {
    throw new Error(
      "Native build output must be a canonical singly linked regular file.",
    );
  }
  return Object.freeze({
    ctimeNs: metadata.ctimeNs,
    dev: metadata.dev,
    ino: metadata.ino,
    mode: metadata.mode,
    mtimeNs: metadata.mtimeNs,
    nlink: metadata.nlink,
    size: metadata.size,
  });
}

function sameOutputIdentity(left, right) {
  return (
    left !== null &&
    left !== undefined &&
    right !== null &&
    right !== undefined &&
    left.ctimeNs === right.ctimeNs &&
    left.dev === right.dev &&
    left.ino === right.ino &&
    left.mode === right.mode &&
    left.mtimeNs === right.mtimeNs &&
    left.nlink === right.nlink &&
    left.size === right.size
  );
}

function sameRenamedOutputIdentity(left, right) {
  return (
    left !== null &&
    left !== undefined &&
    right !== null &&
    right !== undefined &&
    left.dev === right.dev &&
    left.ino === right.ino &&
    left.mode === right.mode &&
    left.mtimeNs === right.mtimeNs &&
    left.nlink === right.nlink &&
    left.size === right.size
  );
}

function sealNativeOutput(nativePath) {
  if (realpathSync(nativePath) !== resolve(nativePath)) {
    throw new Error("Native build output path is not canonical.");
  }
  return readStableRegularFileDigest(nativePath, {
    label: "Native build output",
    requireNonempty: true,
  });
}

function exactOwnedPath(directory, prefix, uuid) {
  if (!UUID_PATTERN.test(uuid)) {
    throw new Error("Native build owned path UUID is invalid.");
  }
  return join(directory, `${prefix}${uuid}`);
}

function assertOwnedRegularFile(owned, directory, prefix, label) {
  if (
    typeof owned?.path !== "string" ||
    typeof owned?.uuid !== "string" ||
    owned.path !== exactOwnedPath(directory, prefix, owned.uuid) ||
    dirname(owned.path) !== directory
  ) {
    throw new Error(`${label} path identity is invalid.`);
  }
  const metadata = lstatSync(owned.path, { bigint: true });
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    metadata.nlink !== 1n ||
    !sameOutputIdentity(metadata, owned.identity) ||
    realpathSync(owned.path) !== owned.path
  ) {
    throw new Error(`${label} changed identity.`);
  }
  return metadata;
}

function removeOwnedRegularFile(owned, directory, prefix, label) {
  assertOwnedRegularFile(owned, directory, prefix, label);
  rmSync(owned.path);
  syncDirectory(directory);
}

function createStagedNative(
  sourcePath,
  finalPath,
  sourceSeal,
  stageUuid = newRunUuid(),
) {
  const finalDirectory = dirname(finalPath);
  const stagePrefix = `.${basename(finalPath)}.stage-`;
  const stagePath = exactOwnedPath(finalDirectory, stagePrefix, stageUuid);
  if (lstatOrNull(stagePath) !== null) {
    throw new Error("Native build staging path already exists.");
  }
  const sourceBefore = outputIdentityOrNull(sourcePath);
  if (
    sourceBefore === null ||
    !sameOutputIdentity(sourceBefore, sourceSeal.identity)
  ) {
    throw new Error("Native build artifact changed before staging.");
  }
  const noFollow = constants.O_NOFOLLOW ?? 0;
  const closeOnExec = constants.O_CLOEXEC ?? 0;
  const sourceDescriptor = openSync(
    sourcePath,
    constants.O_RDONLY | noFollow | closeOnExec,
  );
  let stageDescriptor;
  let stageCreationIdentity;
  let copyError;
  let copied = 0n;
  try {
    const sourceOpened = fstatSync(sourceDescriptor, { bigint: true });
    if (
      !sourceOpened.isFile() ||
      sourceOpened.isSymbolicLink() ||
      sourceOpened.nlink !== 1n ||
      !sameOutputIdentity(sourceBefore, sourceOpened)
    ) {
      throw new Error("Native build artifact changed while it was opened.");
    }
    stageDescriptor = openSync(
      stagePath,
      constants.O_CREAT |
        constants.O_EXCL |
        constants.O_WRONLY |
        noFollow |
        closeOnExec,
      0o600,
    );
    const stageOpened = fstatSync(stageDescriptor, { bigint: true });
    if (!stageOpened.isFile() || stageOpened.nlink !== 1n) {
      throw new Error("Native build staging file must be singly linked.");
    }
    stageCreationIdentity = Object.freeze({
      ctimeNs: stageOpened.ctimeNs,
      dev: stageOpened.dev,
      ino: stageOpened.ino,
      mode: stageOpened.mode,
      mtimeNs: stageOpened.mtimeNs,
      nlink: stageOpened.nlink,
      size: stageOpened.size,
    });
    const buffer = Buffer.allocUnsafe(64 * 1024);
    let bytesRead;
    do {
      bytesRead = readSync(
        sourceDescriptor,
        buffer,
        0,
        buffer.length,
        null,
      );
      let written = 0;
      while (written < bytesRead) {
        const count = writeSync(
          stageDescriptor,
          buffer,
          written,
          bytesRead - written,
          null,
        );
        if (count <= 0) {
          throw new Error("Native build staging copy made no progress.");
        }
        written += count;
      }
      copied += BigInt(bytesRead);
    } while (bytesRead > 0);
    fchmodSync(stageDescriptor, Number(sourceOpened.mode & 0o777n));
    fsyncSync(stageDescriptor);
    const sourceAfterOpen = fstatSync(sourceDescriptor, { bigint: true });
    const stageAfterOpen = fstatSync(stageDescriptor, { bigint: true });
    if (
      copied !== sourceBefore.size ||
      !sameOutputIdentity(sourceBefore, sourceAfterOpen) ||
      !stageAfterOpen.isFile() ||
      stageAfterOpen.nlink !== 1n ||
      stageAfterOpen.size !== copied
    ) {
      throw new Error("Native build artifact changed while it was staged.");
    }
  } catch (error) {
    copyError = error;
  } finally {
    closeSync(sourceDescriptor);
    if (stageDescriptor !== undefined) closeSync(stageDescriptor);
  }
  if (copyError !== undefined) {
    if (stageCreationIdentity !== undefined) {
      const failedStage = lstatOrNull(stagePath);
      if (
        failedStage === null ||
        !failedStage.isFile() ||
        failedStage.isSymbolicLink() ||
        failedStage.nlink !== 1n ||
        failedStage.dev !== stageCreationIdentity.dev ||
        failedStage.ino !== stageCreationIdentity.ino
      ) {
        throw new AggregateError(
          [copyError],
          "Native build staging failed and its partial file changed identity.",
        );
      }
      rmSync(stagePath);
      syncDirectory(finalDirectory);
    }
    throw copyError;
  }
  const sourceAfter = outputIdentityOrNull(sourcePath);
  const staged = lstatSync(stagePath, { bigint: true });
  const owned = Object.freeze({
    identity: Object.freeze({
      ctimeNs: staged.ctimeNs,
      dev: staged.dev,
      ino: staged.ino,
      mode: staged.mode,
      mtimeNs: staged.mtimeNs,
      nlink: staged.nlink,
      size: staged.size,
    }),
    path: stagePath,
    uuid: stageUuid,
  });
  try {
    if (
      !sameOutputIdentity(sourceBefore, sourceAfter) ||
      !staged.isFile() ||
      staged.isSymbolicLink() ||
      staged.nlink !== 1n ||
      staged.size !== copied
    ) {
      throw new Error("Native build artifact or staging file changed after copy.");
    }
    const stagedSeal = sealNativeOutput(stagePath);
    if (
      stagedSeal.sha256 !== sourceSeal.sha256 ||
      !sameOutputIdentity(stagedSeal.identity, owned.identity)
    ) {
      throw new Error("Native build staging copy does not match its artifact.");
    }
    syncDirectory(finalDirectory);
    return Object.freeze({
      ...owned,
      prefix: stagePrefix,
      sha256: stagedSeal.sha256,
    });
  } catch (error) {
    try {
      if (lstatOrNull(stagePath) !== null) {
        removeOwnedRegularFile(
          owned,
          finalDirectory,
          stagePrefix,
          "Native build failed staging file",
        );
      }
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        "Native build staging validation and safe cleanup both failed.",
      );
    }
    throw error;
  }
}

function sameDirectoryIdentity(left, right) {
  return (
    left !== null &&
    left !== undefined &&
    right !== null &&
    right !== undefined &&
    left.dev === right.dev &&
    left.ino === right.ino
  );
}

function publicationLockPlan(
  finalPath,
  ownerId,
  ownerHost = hostname(),
  ownerPid = process.pid,
) {
  if (
    !UUID_PATTERN.test(ownerId) ||
    typeof ownerHost !== "string" ||
    ownerHost.length === 0 ||
    ownerHost.length > 255 ||
    ownerHost.includes("\0") ||
    !Number.isSafeInteger(ownerPid) ||
    ownerPid <= 0
  ) {
    throw new Error("Native build publication lock owner is invalid.");
  }
  const finalName = basename(finalPath);
  return Object.freeze({
    version: PUBLISH_LOCK_VERSION,
    final_name: finalName,
    host: ownerHost,
    owner_id: ownerId,
    pid: ownerPid,
    retired_name: `.${finalName}.retired-${ownerId}`,
    stage_name: `.${finalName}.stage-${ownerId}`,
  });
}

function publicationLockPath(finalPath) {
  return join(
    dirname(finalPath),
    `.${basename(finalPath)}${PUBLISH_LOCK_SUFFIX}`,
  );
}

function publicationLockOwnerPath(path) {
  return join(path, PUBLISH_LOCK_OWNER_FILENAME);
}

function exactPublicationLockKeys(value) {
  if (
    value === null ||
    typeof value !== "object" ||
    Array.isArray(value) ||
    Object.getPrototypeOf(value) !== Object.prototype
  ) {
    return false;
  }
  const actual = Object.keys(value).sort();
  const expected = [
    "final_name",
    "host",
    "owner_id",
    "pid",
    "retired_name",
    "stage_name",
    "version",
  ].sort();
  return (
    actual.length === expected.length &&
    actual.every((key, index) => key === expected[index])
  );
}

function parsePublicationRecoveryMarker(bytes, label) {
  let recovery;
  try {
    recovery = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(bytes),
    );
  } catch {
    throw new Error(`${label} is malformed.`);
  }
  if (
    recovery === null ||
    typeof recovery !== "object" ||
    Array.isArray(recovery) ||
    Object.keys(recovery).length !== 2 ||
    recovery.version !== PUBLISH_LOCK_VERSION ||
    !UUID_PATTERN.test(recovery.recovered_by)
  ) {
    throw new Error(`${label} has an invalid schema.`);
  }
  return recovery;
}

function readPublicationLock(
  path,
  finalPath,
  label,
  { allowRecovered = false } = {},
) {
  const directory = dirname(finalPath);
  const before = lstatSync(path, { bigint: true });
  if (
    dirname(path) !== directory ||
    !before.isDirectory() ||
    before.isSymbolicLink() ||
    realpathSync(path) !== path ||
    realpathSync(directory) !== directory
  ) {
    throw new Error(`${label} is unsafe.`);
  }
  const entriesBefore = readdirSync(path);
  const expectedEntries = allowRecovered
    ? [PUBLISH_LOCK_OWNER_FILENAME, PUBLISH_LOCK_RECOVERED_FILENAME]
    : [PUBLISH_LOCK_OWNER_FILENAME];
  if (
    entriesBefore.length !== expectedEntries.length ||
    !expectedEntries.every((entry) => entriesBefore.includes(entry))
  ) {
    throw new Error(`${label} has an invalid inventory.`);
  }
  const owner = readStableRegularFile(publicationLockOwnerPath(path), {
    label: `${label} owner`,
    maximumBytes: MAX_PUBLISH_LOCK_OWNER_BYTES,
    requireNonempty: true,
  });
  let parsed;
  try {
    parsed = JSON.parse(
      new TextDecoder("utf-8", { fatal: true }).decode(owner.bytes),
    );
  } catch {
    throw new Error(`${label} owner is malformed.`);
  }
  if (!exactPublicationLockKeys(parsed)) {
    throw new Error(`${label} owner has an invalid schema.`);
  }
  const expectedPlan = publicationLockPlan(
    finalPath,
    parsed.owner_id,
    parsed.host,
    parsed.pid,
  );
  for (const key of Object.keys(expectedPlan)) {
    if (parsed[key] !== expectedPlan[key]) {
      throw new Error(`${label} owner does not match its path plan.`);
    }
  }
  const after = lstatSync(path, { bigint: true });
  const entriesAfter = readdirSync(path);
  if (
    !sameDirectoryIdentity(before, after) ||
    !after.isDirectory() ||
    after.isSymbolicLink() ||
    entriesAfter.length !== expectedEntries.length ||
    !expectedEntries.every((entry) => entriesAfter.includes(entry)) ||
    realpathSync(path) !== path
  ) {
    throw new Error(`${label} changed while it was read.`);
  }
  let recoveredBy;
  if (allowRecovered) {
    const recovered = readStableRegularFile(
      join(path, PUBLISH_LOCK_RECOVERED_FILENAME),
      {
        label: `${label} recovery marker`,
        maximumBytes: MAX_PUBLISH_LOCK_OWNER_BYTES,
        requireNonempty: true,
      },
    );
    const recovery = parsePublicationRecoveryMarker(
      recovered.bytes,
      `${label} recovery marker`,
    );
    recoveredBy = recovery.recovered_by;
  }
  return Object.freeze({
    directory,
    identity: Object.freeze({ dev: before.dev, ino: before.ino }),
    ownerIdentity: owner.identity,
    path,
    plan: expectedPlan,
    recoveredBy,
  });
}

function assertPublicationLockIdentity(lock, finalPath, label) {
  const current = readPublicationLock(lock.path, finalPath, label);
  if (
    !sameDirectoryIdentity(current.identity, lock.identity) ||
    current.plan.owner_id !== lock.plan.owner_id ||
    current.plan.host !== lock.plan.host ||
    current.plan.pid !== lock.plan.pid
  ) {
    throw new Error(`${label} changed identity.`);
  }
  return current;
}

function removePublicationLockDirectory(lock, finalPath, label) {
  let current = assertPublicationLockIdentity(lock, finalPath, label);
  const trashUuid = newRunUuid();
  const trashPath = exactOwnedPath(
    current.directory,
    `.${basename(finalPath)}${PUBLISH_LOCK_TRASH_SUFFIX}`,
    trashUuid,
  );
  if (lstatOrNull(trashPath) !== null) {
    throw new Error(`${label} cleanup trash path already exists.`);
  }
  renameSync(current.path, trashPath);
  const moved = readPublicationLock(
    trashPath,
    finalPath,
    `${label} cleanup trash`,
  );
  if (
    !sameDirectoryIdentity(moved.identity, current.identity) ||
    moved.plan.owner_id !== current.plan.owner_id
  ) {
    throw new Error(`${label} changed during cleanup retirement.`);
  }
  syncDirectory(current.directory);
  current = moved;
  const ownerMetadata = lstatSync(
    publicationLockOwnerPath(current.path),
    { bigint: true },
  );
  if (!sameOutputIdentity(ownerMetadata, current.ownerIdentity)) {
    throw new Error(`${label} owner changed identity.`);
  }
  rmSync(publicationLockOwnerPath(current.path));
  rmdirSync(current.path);
  syncDirectory(current.directory);
}

function writePublicationLockOwner(path, plan) {
  const ownerPath = publicationLockOwnerPath(path);
  const descriptor = openSync(
    ownerPath,
    constants.O_CREAT |
      constants.O_EXCL |
      constants.O_WRONLY |
      (constants.O_CLOEXEC ?? 0) |
      (constants.O_NOFOLLOW ?? 0),
    0o600,
  );
  try {
    const opened = fstatSync(descriptor, { bigint: true });
    if (!opened.isFile() || opened.nlink !== 1n) {
      throw new Error(
        "Native build publication lock owner must be singly linked.",
      );
    }
    const bytes = Buffer.from(`${JSON.stringify(plan)}\n`, "utf8");
    let offset = 0;
    while (offset < bytes.length) {
      const written = writeSync(
        descriptor,
        bytes,
        offset,
        bytes.length - offset,
        null,
      );
      if (written <= 0) {
        throw new Error(
          "Native build publication lock owner write made no progress.",
        );
      }
      offset += written;
    }
    fsyncSync(descriptor);
  } finally {
    closeSync(descriptor);
  }
  syncDirectory(path);
}

function createPublicationLockCandidate(finalPath) {
  const directory = dirname(finalPath);
  const ownerId = newRunUuid();
  const plan = publicationLockPlan(finalPath, ownerId);
  const candidatePath = exactOwnedPath(
    directory,
    `.${basename(finalPath)}${PUBLISH_LOCK_CANDIDATE_SUFFIX}`,
    ownerId,
  );
  mkdirSync(candidatePath, { mode: 0o700 });
  try {
    writePublicationLockOwner(candidatePath, plan);
    return readPublicationLock(
      candidatePath,
      finalPath,
      "Native build publication lock candidate",
    );
  } catch (error) {
    const entries = readdirSync(candidatePath);
    if (
      entries.length === 1 &&
      entries[0] === PUBLISH_LOCK_OWNER_FILENAME
    ) {
      const owner = lstatSync(publicationLockOwnerPath(candidatePath), {
        bigint: true,
      });
      if (
        !owner.isFile() ||
        owner.isSymbolicLink() ||
        owner.nlink !== 1n
      ) {
        throw new AggregateError(
          [error],
          "Native build publication lock candidate changed during cleanup.",
        );
      }
      rmSync(publicationLockOwnerPath(candidatePath));
    } else if (entries.length !== 0) {
      throw new AggregateError(
        [error],
        "Native build publication lock candidate has unexpected entries.",
      );
    }
    rmdirSync(candidatePath);
    syncDirectory(directory);
    throw error;
  }
}

function defaultPublicationOwnerIsAlive(pid) {
  if (pid === process.pid) return true;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    if (error?.code === "ESRCH") return false;
    if (error?.code === "EPERM") return true;
    throw error;
  }
}

function publicationOwnerIsAlive(lock, ownerIsAlive) {
  if (lock.plan.host !== hostname()) {
    throw new Error(
      "Native build publication lock belongs to another host and cannot be recovered.",
    );
  }
  const alive = ownerIsAlive(lock.plan.pid);
  if (typeof alive !== "boolean") {
    throw new Error(
      "Native build publication owner liveness probe returned an invalid result.",
    );
  }
  return alive;
}

function assertOwnedRecoveryFile(path, label) {
  const metadata = lstatSync(path, { bigint: true });
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    metadata.nlink !== 1n ||
    realpathSync(path) !== path
  ) {
    throw new Error(`${label} must be a canonical singly linked regular file.`);
  }
  return metadata;
}

function publicationRecoveryTemporaryName(ownerId) {
  if (!UUID_PATTERN.test(ownerId)) {
    throw new Error("Native build publication recovery owner is invalid.");
  }
  return `.${PUBLISH_LOCK_RECOVERED_FILENAME}.${ownerId}.tmp`;
}

function writePublicationRecoveryMarker(tombstone, currentLock, finalPath) {
  const temporaryPath = join(
    tombstone.path,
    publicationRecoveryTemporaryName(currentLock.plan.owner_id),
  );
  const recoveredPath = join(
    tombstone.path,
    PUBLISH_LOCK_RECOVERED_FILENAME,
  );
  const bytes = Buffer.from(
    `${JSON.stringify({
      version: PUBLISH_LOCK_VERSION,
      recovered_by: currentLock.plan.owner_id,
    })}\n`,
    "utf8",
  );
  const descriptor = openSync(
    temporaryPath,
    constants.O_CREAT |
      constants.O_EXCL |
      constants.O_WRONLY |
      (constants.O_CLOEXEC ?? 0) |
      (constants.O_NOFOLLOW ?? 0),
    0o600,
  );
  try {
    let offset = 0;
    while (offset < bytes.length) {
      const written = writeSync(
        descriptor,
        bytes,
        offset,
        bytes.length - offset,
        null,
      );
      if (written <= 0) {
        throw new Error(
          "Native build publication recovery marker write made no progress.",
        );
      }
      offset += written;
    }
    fsyncSync(descriptor);
  } finally {
    closeSync(descriptor);
  }
  linkSync(temporaryPath, recoveredPath);
  syncDirectory(tombstone.path);
  rmSync(temporaryPath);
  syncDirectory(tombstone.path);
  return readPublicationLock(
    tombstone.path,
    finalPath,
    "Native build recovered publication guard",
    { allowRecovered: true },
  );
}

function readPublicationTombstone(path, finalPath, currentLock) {
  const initial = lstatSync(path, { bigint: true });
  if (
    !initial.isDirectory() ||
    initial.isSymbolicLink() ||
    dirname(path) !== dirname(finalPath) ||
    realpathSync(path) !== path
  ) {
    throw new Error("Native build stale publication lock is unsafe.");
  }
  const names = readdirSync(path);
  const temporaryNames = names.filter(
    (name) =>
      name.startsWith(`.${PUBLISH_LOCK_RECOVERED_FILENAME}.`) &&
      name.endsWith(".tmp") &&
      UUID_PATTERN.test(
        name.slice(
          `.${PUBLISH_LOCK_RECOVERED_FILENAME}.`.length,
          -".tmp".length,
        ),
      ),
  );
  const unknown = names.filter(
    (name) =>
      name !== PUBLISH_LOCK_OWNER_FILENAME &&
      name !== PUBLISH_LOCK_RECOVERED_FILENAME &&
      !temporaryNames.includes(name),
  );
  if (
    unknown.length !== 0 ||
    temporaryNames.length > 1 ||
    !names.includes(PUBLISH_LOCK_OWNER_FILENAME)
  ) {
    throw new Error(
      "Native build stale publication lock has an invalid inventory.",
    );
  }
  const recoveredPath = join(path, PUBLISH_LOCK_RECOVERED_FILENAME);
  if (temporaryNames.length === 1) {
    const temporaryPath = join(path, temporaryNames[0]);
    const temporaryOwnerId = temporaryNames[0].slice(
      `.${PUBLISH_LOCK_RECOVERED_FILENAME}.`.length,
      -".tmp".length,
    );
    const temporary = lstatSync(temporaryPath, { bigint: true });
    if (
      !temporary.isFile() ||
      temporary.isSymbolicLink() ||
      (temporary.nlink !== 1n && temporary.nlink !== 2n) ||
      realpathSync(temporaryPath) !== temporaryPath
    ) {
      throw new Error(
        "Native build publication recovery marker temporary is unsafe.",
      );
    }
    const recoveredMetadata = lstatOrNull(recoveredPath);
    if (recoveredMetadata === null) {
      if (temporary.nlink !== 1n) {
        throw new Error(
          "Native build publication recovery marker temporary has an invalid link count.",
        );
      }
      let recovery;
      try {
        recovery = parsePublicationRecoveryMarker(
          readStableRegularFile(temporaryPath, {
            label: "Native build publication recovery marker temporary",
            maximumBytes: MAX_PUBLISH_LOCK_OWNER_BYTES,
            requireNonempty: true,
          }).bytes,
          "Native build publication recovery marker temporary",
        );
      } catch {
        assertPublicationLockIdentity(
          currentLock,
          finalPath,
          "Native build current publication lock",
        );
        rmSync(temporaryPath);
        syncDirectory(path);
        return readPublicationLock(
          path,
          finalPath,
          "Native build stale publication lock",
        );
      }
      if (recovery.recovered_by !== temporaryOwnerId) {
        throw new Error(
          "Native build publication recovery marker temporary owner is invalid.",
        );
      }
      linkSync(temporaryPath, recoveredPath);
      syncDirectory(path);
    } else {
      if (
        !recoveredMetadata.isFile() ||
        recoveredMetadata.isSymbolicLink() ||
        temporary.nlink !== 2n ||
        recoveredMetadata.nlink !== 2n ||
        recoveredMetadata.dev !== temporary.dev ||
        recoveredMetadata.ino !== temporary.ino
      ) {
        throw new Error(
          "Native build publication recovery marker changed identity.",
        );
      }
    }
    rmSync(temporaryPath);
    syncDirectory(path);
  }
  if (lstatOrNull(recoveredPath) !== null) {
    return readPublicationLock(
      path,
      finalPath,
      "Native build stale publication lock",
      { allowRecovered: true },
    );
  }
  return readPublicationLock(
    path,
    finalPath,
    "Native build stale publication lock",
  );
}

function recoverPublicationTombstone({
  currentLock,
  finalPath,
  invalidateProvenance,
  ownerIsAlive,
  tombstone,
}) {
  assertPublicationLockIdentity(
    currentLock,
    finalPath,
    "Native build current publication lock",
  );
  if (tombstone.recoveredBy !== undefined) return;
  if (publicationOwnerIsAlive(tombstone, ownerIsAlive)) {
    throw new Error(
      "Native build publication recovery owner is still running.",
    );
  }
  invalidateProvenance(finalPath);
  const stagePath = join(tombstone.directory, tombstone.plan.stage_name);
  const retiredPath = join(
    tombstone.directory,
    tombstone.plan.retired_name,
  );
  if (lstatOrNull(stagePath) !== null) {
    assertOwnedRecoveryFile(
      stagePath,
      "Native build recovered staging file",
    );
    rmSync(stagePath);
    syncDirectory(tombstone.directory);
  }
  if (lstatOrNull(retiredPath) !== null) {
    const retired = assertOwnedRecoveryFile(
      retiredPath,
      "Native build recovered retired file",
    );
    if (outputIdentityOrNull(finalPath) === null) {
      renameSync(retiredPath, finalPath);
      const restored = outputIdentityOrNull(finalPath);
      if (!sameRenamedOutputIdentity(retired, restored)) {
        throw new Error(
          "Native build recovered retired file changed during restoration.",
        );
      }
    } else {
      rmSync(retiredPath);
    }
    syncDirectory(tombstone.directory);
  }
  writePublicationRecoveryMarker(tombstone, currentLock, finalPath);
}

function recoverPublicationTombstones({
  currentLock,
  finalPath,
  invalidateProvenance,
  ownerIsAlive,
}) {
  const prefix =
    `.${basename(finalPath)}${PUBLISH_LOCK_STALE_SUFFIX}`;
  const names = readdirSync(currentLock.directory)
    .filter((name) => name.startsWith(prefix))
    .sort();
  for (const name of names) {
    const ownerId = name.slice(prefix.length);
    if (!UUID_PATTERN.test(ownerId)) continue;
    const path = join(currentLock.directory, name);
    const tombstone = readPublicationTombstone(
      path,
      finalPath,
      currentLock,
    );
    if (tombstone.plan.owner_id !== ownerId) {
      throw new Error(
        "Native build stale publication lock name does not match its owner.",
      );
    }
    recoverPublicationTombstone({
      currentLock,
      finalPath,
      invalidateProvenance,
      ownerIsAlive,
      tombstone,
    });
  }
}

function cleanupInactivePublicationLockDebris({
  currentLock,
  finalPath,
  ownerIsAlive,
}) {
  const prefixes = [
    `.${basename(finalPath)}${PUBLISH_LOCK_CANDIDATE_SUFFIX}`,
    `.${basename(finalPath)}${PUBLISH_LOCK_RELEASED_SUFFIX}`,
  ];
  const names = readdirSync(currentLock.directory).sort();
  for (const name of names) {
    const prefix = prefixes.find((candidatePrefix) =>
      name.startsWith(candidatePrefix),
    );
    if (prefix === undefined) continue;
    const ownerId = name.slice(prefix.length);
    if (!UUID_PATTERN.test(ownerId)) continue;
    assertPublicationLockIdentity(
      currentLock,
      finalPath,
      "Native build current publication lock",
    );
    let debris;
    try {
      debris = readPublicationLock(
        join(currentLock.directory, name),
        finalPath,
        "Native build publication lock debris",
      );
    } catch (error) {
      if (prefix.endsWith(PUBLISH_LOCK_CANDIDATE_SUFFIX)) {
        // A crash can leave an off-name initializer incomplete. It never
        // owned the canonical lock, so preserve it as forensic debris without
        // blocking a later publication.
        continue;
      }
      throw error;
    }
    if (debris.plan.owner_id !== ownerId) {
      throw new Error(
        "Native build publication lock debris name does not match its owner.",
      );
    }
    if (publicationOwnerIsAlive(debris, ownerIsAlive)) continue;
    removePublicationLockDirectory(
      debris,
      finalPath,
      "Native build publication lock debris",
    );
  }
}

function acquirePublicationLock(
  finalPath,
  {
    invalidateProvenance,
    ownerIsAlive = defaultPublicationOwnerIsAlive,
  },
) {
  const candidate = createPublicationLockCandidate(finalPath);
  const lockPath = publicationLockPath(finalPath);
  let acquired;
  let candidatePresent = true;
  let movedStale = false;
  let releasedAcquired = false;
  try {
    try {
      if (lstatOrNull(lockPath) !== null) {
        throw new Error("Native build publication lock path already exists.");
      }
      renameSync(candidate.path, lockPath);
      candidatePresent = false;
    } catch (error) {
      if (lstatOrNull(lockPath) === null) throw error;
      const existing = readPublicationLock(
        lockPath,
        finalPath,
        "Native build publication lock",
      );
      if (publicationOwnerIsAlive(existing, ownerIsAlive)) {
        throw new Error("Another native build publication is in progress.");
      }
      const stalePath = join(
        existing.directory,
        `.${basename(finalPath)}${PUBLISH_LOCK_STALE_SUFFIX}` +
          existing.plan.owner_id,
      );
      if (lstatOrNull(stalePath) !== null) {
        throw new Error(
          "Native build stale publication lock recovery is already pending.",
        );
      }
      renameSync(existing.path, stalePath);
      movedStale = true;
      syncDirectory(existing.directory);
      try {
        renameSync(candidate.path, lockPath);
        candidatePresent = false;
      } catch (acquireError) {
        if (lstatOrNull(lockPath) !== null) {
          throw new Error("Another native build publication is in progress.");
        }
        throw acquireError;
      }
    }
    acquired = readPublicationLock(
      lockPath,
      finalPath,
      "Native build publication lock",
    );
    if (
      !sameDirectoryIdentity(acquired.identity, candidate.identity) ||
      acquired.plan.owner_id !== candidate.plan.owner_id
    ) {
      throw new Error("Native build publication lock acquisition was replaced.");
    }
    syncDirectory(acquired.directory);
    recoverPublicationTombstones({
      currentLock: acquired,
      finalPath,
      invalidateProvenance,
      ownerIsAlive,
    });
    cleanupInactivePublicationLockDebris({
      currentLock: acquired,
      finalPath,
      ownerIsAlive,
    });
    return acquired;
  } catch (error) {
    if (acquired !== undefined && lstatOrNull(acquired.path) !== null) {
      try {
        releasePublicationLock(acquired, finalPath);
        releasedAcquired = true;
      } catch (cleanupError) {
        throw new AggregateError(
          [error, cleanupError],
          "Native build publication recovery and lock release both failed.",
        );
      }
    }
    if (candidatePresent && lstatOrNull(candidate.path) !== null) {
      try {
        removePublicationLockDirectory(
          candidate,
          finalPath,
          "Native build publication lock candidate",
        );
      } catch (cleanupError) {
        throw new AggregateError(
          [error, cleanupError],
          "Native build publication lock acquisition and candidate cleanup both failed.",
        );
      }
    }
    if (
      movedStale &&
      !releasedAcquired &&
      lstatOrNull(lockPath) === null
    ) {
      throw new AggregateError(
        [error],
        "Native build publication lock recovery yielded ownership to another publisher.",
      );
    }
    throw error;
  }
}

function releasePublicationLock(lock, finalPath) {
  assertPublicationLockIdentity(
    lock,
    finalPath,
    "Native build publication lock",
  );
  const releasedPath = join(
    lock.directory,
    `.${basename(finalPath)}${PUBLISH_LOCK_RELEASED_SUFFIX}` +
      lock.plan.owner_id,
  );
  if (lstatOrNull(releasedPath) !== null) {
    throw new Error("Native build released publication lock already exists.");
  }
  renameSync(lock.path, releasedPath);
  const released = readPublicationLock(
    releasedPath,
    finalPath,
    "Native build released publication lock",
  );
  if (
    !sameDirectoryIdentity(released.identity, lock.identity) ||
    released.plan.owner_id !== lock.plan.owner_id
  ) {
    throw new Error("Native build publication lock changed during release.");
  }
  syncDirectory(lock.directory);
  removePublicationLockDirectory(
    released,
    finalPath,
    "Native build released publication lock",
  );
}

function publishStagedNative({
  finalPath,
  invalidateProvenance,
  ownerIsAlive,
  provenance,
  publicationFailpoint,
  sourcePath,
  sourceSeal,
  writeProvenance,
}) {
  const finalDirectory = dirname(finalPath);
  const retiredPrefix = `.${basename(finalPath)}.retired-`;
  const lock = acquirePublicationLock(finalPath, {
    invalidateProvenance,
    ownerIsAlive,
  });
  let stage;
  try {
    stage = createStagedNative(
      sourcePath,
      finalPath,
      sourceSeal,
      lock.plan.owner_id,
    );
  } catch (error) {
    try {
      releasePublicationLock(lock, finalPath);
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        "Native build staging and publication lock cleanup both failed.",
      );
    }
    throw error;
  }
  let binaryPublished = false;
  let publishedOutputIdentity;
  let publicationError;
  let retired;
  let stagePresent = true;
  try {
    assertOwnedRegularFile(
      stage,
      finalDirectory,
      stage.prefix,
      "Native build staging file",
    );
    const priorIdentity = outputIdentityOrNull(finalPath);
    invalidateProvenance(finalPath);
    publicationFailpoint("after-invalidation");
    assertOwnedRegularFile(
      stage,
      finalDirectory,
      stage.prefix,
      "Native build staging file",
    );
    if (priorIdentity === null) {
      if (lstatOrNull(finalPath) !== null) {
        throw new Error(
          "Native build publication target appeared after provenance invalidation.",
        );
      }
    } else {
      const retiredUuid = lock.plan.owner_id;
      const retiredPath = exactOwnedPath(
        finalDirectory,
        retiredPrefix,
        retiredUuid,
      );
      if (lstatOrNull(retiredPath) !== null) {
        throw new Error("Native build retired binary path already exists.");
      }
      renameSync(finalPath, retiredPath);
      const retiredMetadata = lstatSync(retiredPath, { bigint: true });
      if (
        !sameRenamedOutputIdentity(priorIdentity, retiredMetadata) ||
        retiredMetadata.isSymbolicLink() ||
        retiredMetadata.nlink !== 1n
      ) {
        throw new Error(
          "Native build publication target changed while it was retired.",
        );
      }
      retired = Object.freeze({
        identity: Object.freeze({
          ctimeNs: retiredMetadata.ctimeNs,
          dev: retiredMetadata.dev,
          ino: retiredMetadata.ino,
          mode: retiredMetadata.mode,
          mtimeNs: retiredMetadata.mtimeNs,
          nlink: retiredMetadata.nlink,
          size: retiredMetadata.size,
        }),
        path: retiredPath,
        uuid: retiredUuid,
      });
      syncDirectory(finalDirectory);
      publicationFailpoint("after-binary-retire");
    }
    assertOwnedRegularFile(
      stage,
      finalDirectory,
      stage.prefix,
      "Native build staging file",
    );
    renameSync(stage.path, finalPath);
    stagePresent = false;
    const publishedIdentity = outputIdentityOrNull(finalPath);
    if (!sameRenamedOutputIdentity(stage.identity, publishedIdentity)) {
      throw new Error("Native build binary changed during atomic publication.");
    }
    publishedOutputIdentity = publishedIdentity;
    publicationFailpoint("after-binary-rename");
    syncDirectory(finalDirectory);
    const publishedSeal = sealNativeOutput(finalPath);
    if (
      publishedSeal.sha256 !== stage.sha256 ||
      provenance.native_sha256 !== stage.sha256 ||
      !sameOutputIdentity(publishedSeal.identity, publishedIdentity)
    ) {
      throw new Error("Published native build does not match its staged artifact.");
    }
    publishedOutputIdentity = publishedSeal.identity;
    binaryPublished = true;
    publicationFailpoint("after-binary-publish");
    try {
      writeProvenance(finalPath, provenance);
    } catch (error) {
      invalidateProvenance(finalPath);
      throw error;
    }
    let finalIdentity;
    try {
      finalIdentity = outputIdentityOrNull(finalPath);
    } catch (error) {
      invalidateProvenance(finalPath);
      throw error;
    }
    if (!sameOutputIdentity(publishedOutputIdentity, finalIdentity)) {
      invalidateProvenance(finalPath);
      throw new Error(
        "Native build binary changed while final provenance was published.",
      );
    }
  } catch (error) {
    publicationError = error;
  }
  let cleanupError;
  try {
    if (binaryPublished) {
      if (
        publishedOutputIdentity === undefined ||
        lstatOrNull(finalPath) === null
      ) {
        throw new Error(
          "Native build published target disappeared before cleanup.",
        );
      }
      const currentPublished = outputIdentityOrNull(finalPath);
      if (
        !sameOutputIdentity(
          currentPublished,
          publishedOutputIdentity,
        )
      ) {
        throw new Error(
          "Native build published target changed before cleanup.",
        );
      }
    }
    if (
      !binaryPublished &&
      publishedOutputIdentity !== undefined
    ) {
      if (lstatOrNull(finalPath) === null) {
        throw new Error(
          "Native build publication target disappeared before rollback.",
        );
      }
      const currentPublished = outputIdentityOrNull(finalPath);
      if (
        !sameOutputIdentity(
          currentPublished,
          publishedOutputIdentity,
        )
      ) {
        throw new Error(
          "Native build publication target changed before rollback.",
        );
      }
      if (lstatOrNull(stage.path) !== null) {
        throw new Error(
          "Native build rollback staging path unexpectedly appeared.",
        );
      }
      renameSync(finalPath, stage.path);
      const rolledBackStage = outputIdentityOrNull(stage.path);
      if (
        !sameRenamedOutputIdentity(
          currentPublished,
          rolledBackStage,
        )
      ) {
        throw new Error(
          "Native build publication target changed during rollback retirement.",
        );
      }
      stage = Object.freeze({
        ...stage,
        identity: rolledBackStage,
      });
      stagePresent = true;
      syncDirectory(finalDirectory);
      publicationFailpoint("after-binary-rollback-rename");
    }
    if (retired !== undefined) {
      if (lstatOrNull(retired.path) === null) {
        throw new Error(
          "Native build retired binary disappeared before cleanup.",
        );
      }
      if (binaryPublished) {
        removeOwnedRegularFile(
          retired,
          finalDirectory,
          retiredPrefix,
          "Native build retired binary",
        );
      } else {
        assertOwnedRegularFile(
          retired,
          finalDirectory,
          retiredPrefix,
          "Native build retired binary",
        );
        if (outputIdentityOrNull(finalPath) !== null) {
          throw new Error(
            "Native build publication target appeared before rollback.",
          );
        }
        renameSync(retired.path, finalPath);
        const restored = outputIdentityOrNull(finalPath);
        if (!sameRenamedOutputIdentity(retired.identity, restored)) {
          throw new Error(
            "Native build retired binary changed during rollback.",
          );
        }
        syncDirectory(finalDirectory);
        publicationFailpoint("after-binary-rollback-restore");
      }
    }
    if (stagePresent) {
      if (lstatOrNull(stage.path) === null) {
        throw new Error(
          "Native build staging file disappeared before cleanup.",
        );
      }
      removeOwnedRegularFile(
        stage,
        finalDirectory,
        stage.prefix,
        "Native build staging file",
      );
    }
  } catch (error) {
    cleanupError = error;
  }
  if (cleanupError !== undefined) {
    if (publicationError !== undefined) {
      throw new AggregateError(
        [publicationError, cleanupError],
        "Native build publication and owner-bound rollback both failed.",
      );
    }
    throw cleanupError;
  }
  try {
    releasePublicationLock(lock, finalPath);
  } catch (error) {
    if (publicationError !== undefined) {
      throw new AggregateError(
        [publicationError, error],
        "Native build publication and lock release both failed.",
      );
    }
    throw error;
  }
  if (publicationError !== undefined) {
    throw publicationError;
  }
  return finalPath;
}

function parseCargoMessages(stdout) {
  const encoded = Buffer.isBuffer(stdout)
    ? stdout.toString("utf8")
    : typeof stdout === "string"
      ? stdout
      : null;
  if (encoded === null) {
    throw new Error("Successful Cargo execution did not return JSON messages.");
  }
  const messages = [];
  for (const line of encoded.split(/\r?\n/u)) {
    if (line.length === 0) continue;
    let message;
    try {
      message = JSON.parse(line);
    } catch {
      throw new Error("Cargo emitted a malformed JSON build message.");
    }
    if (
      message === null ||
      typeof message !== "object" ||
      Array.isArray(message) ||
      typeof message.reason !== "string"
    ) {
      throw new Error("Cargo emitted a malformed JSON build message.");
    }
    messages.push(message);
  }
  return messages;
}

function readWorkspacePackageVersion(snapshotRoot) {
  const manifestPath = join(snapshotRoot, "Cargo.toml");
  const manifest = new TextDecoder("utf-8", { fatal: true }).decode(
    readStableRegularFile(manifestPath, {
      label: "Native build workspace Cargo manifest",
      maximumBytes: MAX_CARGO_MANIFEST_BYTES,
      requireNonempty: true,
    }).bytes,
  );
  const sectionHeader = /^\[workspace\.package\][ \t]*(?:#.*)?$/mu.exec(
    manifest,
  );
  if (sectionHeader === null) {
    throw new Error(
      "Native build workspace Cargo manifest has no workspace.package section.",
    );
  }
  const sectionTail = manifest.slice(
    sectionHeader.index + sectionHeader[0].length,
  );
  const nextSection = /^\[[^\]\r\n]+\][ \t]*(?:#.*)?$/mu.exec(sectionTail);
  const section =
    nextSection === null
      ? sectionTail
      : sectionTail.slice(0, nextSection.index);
  const versions = [
    ...section.matchAll(
      /^version[ \t]*=[ \t]*"([^"\r\n]+)"[ \t]*(?:#.*)?$/gmu,
    ),
  ];
  if (
    versions.length !== 1 ||
    !/^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/u.test(
      versions[0][1],
    )
  ) {
    throw new Error(
      "Native build workspace Cargo manifest has no exact semantic version.",
    );
  }
  return versions[0][1];
}

function packageIdNamesIntendedPackage(
  packageId,
  expectedPackageRoot,
  expectedVersion,
) {
  return (
    typeof packageId === "string" &&
    packageId ===
      `path+${pathToFileURL(realpathSync(expectedPackageRoot)).href}` +
        `#${expectedVersion}`
  );
}

function exactStringArray(value, expected) {
  return (
    Array.isArray(value) &&
    value.length === expected.length &&
    value.every((item, index) => item === expected[index])
  );
}

function cargoArtifactProfileMatches(value, cargoProfile) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const optimized = cargoProfile === "release" || cargoProfile === "deploy";
  return (
    value.opt_level === (optimized ? "3" : "0") &&
    value.debuginfo === 0 &&
    value.debug_assertions === !optimized &&
    value.overflow_checks === !optimized &&
    value.test === false
  );
}

function artifactMightBeIntended(
  message,
  expectedNativePath,
  expectedSourcePath,
) {
  const filenames = Array.isArray(message.filenames) ? message.filenames : [];
  return (
    message.target?.name === INTENDED_PACKAGE ||
    message.target?.src_path === expectedSourcePath ||
    filenames.some(
      (filename) =>
        typeof filename === "string" &&
        isAbsolute(filename) &&
        resolve(filename) === expectedNativePath,
    )
  );
}

function verifyCargoArtifactMessages(
  stdout,
  { cargoProfile, nativePath, snapshotRoot },
) {
  const expectedNativePath = resolve(nativePath);
  const expectedPackageRoot = resolve(
    snapshotRoot,
    "crates",
    INTENDED_PACKAGE,
  );
  const expectedManifestPath = join(expectedPackageRoot, "Cargo.toml");
  const expectedVersion = readWorkspacePackageVersion(snapshotRoot);
  const expectedSourcePath = resolve(
    expectedPackageRoot,
    "src",
    "lib.rs",
  );
  const messages = parseCargoMessages(stdout);
  const buildFinished = messages.filter(
    ({ reason }) => reason === "build-finished",
  );
  if (
    buildFinished.length !== 1 ||
    buildFinished[0].success !== true ||
    messages.at(-1) !== buildFinished[0]
  ) {
    throw new Error(
      "Cargo did not emit exactly one successful terminal build-finished message.",
    );
  }
  const candidates = messages.filter(
    (message) =>
      message.reason === "compiler-artifact" &&
      artifactMightBeIntended(
        message,
        expectedNativePath,
        expectedSourcePath,
      ),
  );
  if (candidates.length !== 1) {
    throw new Error(
      "Cargo did not emit exactly one iroha_js_host compiler artifact.",
    );
  }
  const artifact = candidates[0];
  const filenames = Array.isArray(artifact.filenames)
    ? artifact.filenames
    : [];
  const matchingFilenames = filenames.filter(
    (filename) =>
      typeof filename === "string" &&
      isAbsolute(filename) &&
      resolve(filename) === expectedNativePath,
  );
  if (
    !packageIdNamesIntendedPackage(
      artifact.package_id,
      expectedPackageRoot,
      expectedVersion,
    ) ||
    artifact.manifest_path !== expectedManifestPath ||
    realpathSync(artifact.manifest_path) !== expectedManifestPath ||
    artifact.target?.name !== INTENDED_PACKAGE ||
    !exactStringArray(artifact.target?.kind, ["cdylib"]) ||
    !exactStringArray(artifact.target?.crate_types, ["cdylib"]) ||
    artifact.target?.src_path !== expectedSourcePath ||
    realpathSync(artifact.target.src_path) !== expectedSourcePath ||
    !exactStringArray(artifact.features, []) ||
    artifact.executable !== null ||
    !cargoArtifactProfileMatches(artifact.profile, cargoProfile) ||
    artifact.fresh !== false ||
    matchingFilenames.length !== 1 ||
    realpathSync(matchingFilenames[0]) !== expectedNativePath
  ) {
    throw new Error(
      "Cargo emitted an invalid iroha_js_host cdylib compiler artifact.",
    );
  }
  return artifact;
}

function forwardCargoRenderedDiagnostics(stdout) {
  const encoded = Buffer.isBuffer(stdout)
    ? stdout.toString("utf8")
    : typeof stdout === "string"
      ? stdout
      : "";
  for (const line of encoded.split(/\r?\n/u)) {
    if (line.length === 0) continue;
    try {
      const rendered = JSON.parse(line)?.message?.rendered;
      if (typeof rendered === "string") process.stderr.write(rendered);
    } catch {
      // Strict validation below reports malformed output after Cargo exits.
    }
  }
}

export function nativeBuildOutputPath({
  repoRoot = defaultRepoRoot,
  cargoProfile,
  env = process.env,
  platform = process.platform,
}) {
  const configuredTarget = env.CARGO_TARGET_DIR;
  const targetRoot = configuredTarget
    ? isAbsolute(configuredTarget)
      ? configuredTarget
      : resolve(repoRoot, configuredTarget)
    : join(repoRoot, "target");
  const filename =
    platform === "win32"
      ? "iroha_js_host.dll"
      : `libiroha_js_host.${platform === "darwin" ? "dylib" : "so"}`;
  return join(targetRoot, cargoProfile, filename);
}

function nativeBuildTargetRoot(repoRoot, env) {
  const configuredTarget = env.CARGO_TARGET_DIR;
  return configuredTarget
    ? isAbsolute(configuredTarget)
      ? configuredTarget
      : resolve(repoRoot, configuredTarget)
    : join(repoRoot, "target");
}

export function runNativeBuild({
  repoRoot = defaultRepoRoot,
  env = process.env,
  platform = process.platform,
  runCargo = (args, { cwd = repoRoot, cargoEnv = env } = {}) =>
    spawnSync("cargo", args, {
      cwd,
      encoding: "utf8",
      env: cargoEnv,
      maxBuffer: MAX_CARGO_JSON_BYTES,
      stdio: ["inherit", "pipe", "inherit"],
    }),
  createSourceSnapshot = createNativeBuildSourceSnapshot,
  verifySourceSnapshot = verifyNativeBuildSourceSnapshot,
  cleanupSourceSnapshot = cleanupNativeBuildSourceSnapshot,
  invalidateProvenance = invalidateNativeBuildProvenance,
  publicationOwnerIsAlive = defaultPublicationOwnerIsAlive,
  publicationFailpoint = () => {},
  runContainerFailpoint = () => {},
  runContainerOwnerIsAlive = defaultPublicationOwnerIsAlive,
  writeProvenance = writeNativeBuildProvenance,
} = {}) {
  const cargoProfile = resolveNativeBuildProfile(env);
  const profileOverride = Object.keys(env).find((key) =>
    key.toUpperCase().startsWith("CARGO_PROFILE_"),
  );
  if (profileOverride !== undefined) {
    throw new Error(
      `Native build forbids Cargo profile environment override ${profileOverride}.`,
    );
  }
  const canonicalRepoRoot = canonicalDirectory(
    repoRoot,
    "Native build repository root",
  ).canonicalPath;
  const publicationTarget = canonicalDirectory(
    nativeBuildTargetRoot(canonicalRepoRoot, env),
    "Native build publication target directory",
    { create: true },
  );
  const targetRoot = publicationTarget.canonicalPath;
  const temporaryParent = canonicalDirectory(
    tmpdir(),
    "Native build operating-system temporary directory",
  );
  recoverNativeBuildRunContainers(
    temporaryParent,
    runContainerOwnerIsAlive,
    runContainerFailpoint,
  );
  const runContainer = createRunContainer(
    canonicalRepoRoot,
    targetRoot,
    temporaryParent,
    runContainerFailpoint,
  );
  let runCargoTarget;
  let snapshot;
  let snapshotPlacementVerified = false;
  try {
    runCargoTarget = createRunCargoTarget(runContainer);
    snapshot = createSourceSnapshot(canonicalRepoRoot, runContainer.path, {
      env,
    });
    assertSnapshotPlacement(snapshot, runContainer);
    snapshotPlacementVerified = true;
    const sourceBefore = verifySourceSnapshot(snapshot);
    const suppliedRevision = env.IROHA_GIT_COMMIT_HASH;
    if (
      suppliedRevision !== undefined &&
      suppliedRevision !== sourceBefore.sourceGitRevision
    ) {
      throw new Error(
        "IROHA_GIT_COMMIT_HASH does not match the sealed native build source revision.",
      );
    }
    const cargoManifest = join(snapshot.snapshotRoot, "Cargo.toml");
    const buildArgs = [
      "build",
      "--locked",
      "--offline",
      "--jobs",
      "1",
      "--manifest-path",
      cargoManifest,
      "--package",
      INTENDED_PACKAGE,
      "--lib",
      "--target-dir",
      runCargoTarget.path,
      "--message-format=json-render-diagnostics",
      ...cargoBuildArgsForNativeProfile(cargoProfile),
    ];
    const cargoEnv = {
      ...env,
      CARGO_TARGET_DIR: runCargoTarget.path,
      IROHA_GIT_COMMIT_HASH: sourceBefore.sourceGitRevision,
    };
    const runNativePath = nativeBuildOutputPath({
      repoRoot: canonicalRepoRoot,
      cargoProfile,
      env: cargoEnv,
      platform,
    });
    if (outputIdentityOrNull(runNativePath) !== null) {
      throw new Error("Native build private Cargo output already exists.");
    }
    const build = runCargo(buildArgs, {
      cargoEnv,
      cwd: snapshot.snapshotRoot,
    });
    forwardCargoRenderedDiagnostics(build.stdout);
    if (build.status !== 0) return build.status ?? 1;
    if (build.error !== undefined) {
      throw new Error("Cargo reported an execution error despite a successful status.");
    }
    verifyCargoArtifactMessages(build.stdout, {
      cargoProfile,
      nativePath: runNativePath,
      snapshotRoot: snapshot.snapshotRoot,
    });
    const privateOutput = copyCargoArtifactToOwnedSeal(
      runNativePath,
      runCargoTarget,
      runContainer,
      runContainerFailpoint,
    );
    const outputAfterCargo = privateOutput.seal;
    const sourceAfter = verifySourceSnapshot(snapshot);
    const provenance = createNativeBuildProvenance({
      cargoProfile,
      nativePath: privateOutput.path,
      sourceBefore,
      sourceAfter,
    });
    if (
      provenance.native_sha256 !== outputAfterCargo.sha256 ||
      !sameOutputIdentity(
        outputAfterCargo.identity,
        outputIdentityOrNull(privateOutput.path),
      )
    ) {
      throw new Error(
        "Native build output changed while its provenance was being created.",
      );
    }
    assertDirectoryIdentity(
      publicationTarget,
      "Native build publication target directory",
    );
    const finalNativePath = nativeBuildOutputPath({
      repoRoot: canonicalRepoRoot,
      cargoProfile,
      env: { ...env, CARGO_TARGET_DIR: targetRoot },
      platform,
    });
    const finalDirectory = dirname(finalNativePath);
    const canonicalFinalDirectory = canonicalDirectory(
      finalDirectory,
      "Native build final profile directory",
      { create: true },
    );
    if (canonicalFinalDirectory.canonicalPath !== resolve(finalDirectory)) {
      throw new Error(
        "Native build final profile directory must be canonical.",
      );
    }
    publishStagedNative({
      finalPath: finalNativePath,
      invalidateProvenance,
      ownerIsAlive: publicationOwnerIsAlive,
      provenance,
      publicationFailpoint,
      sourcePath: privateOutput.path,
      sourceSeal: outputAfterCargo,
      writeProvenance,
    });
    return 0;
  } finally {
    try {
      if (snapshotPlacementVerified) {
        cleanupSourceSnapshot(snapshot);
        syncDirectory(runContainer.path);
      }
    } finally {
      try {
        if (runCargoTarget !== undefined) {
          cleanupRunCargoTarget(runCargoTarget, runContainer);
        }
      } finally {
        cleanupRunContainer(runContainer, runContainerFailpoint);
      }
    }
  }
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  process.exitCode = runNativeBuild();
}
