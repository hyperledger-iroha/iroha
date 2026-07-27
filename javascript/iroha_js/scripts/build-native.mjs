#!/usr/bin/env node
/** Build the native `iroha_js_host` library and bind it to Git provenance. */
import { spawnSync } from "node:child_process";
import { randomUUID } from "node:crypto";
import {
  closeSync,
  constants,
  fchmodSync,
  fstatSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  readSync,
  realpathSync,
  renameSync,
  rmSync,
  rmdirSync,
  writeSync,
} from "node:fs";
import { tmpdir } from "node:os";
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
  readStableRegularFileDigest,
  verifyNativeBuildSourceSnapshot,
  writeNativeBuildProvenance,
} from "./native-build-provenance.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = join(scriptDir, "..", "..", "..");
const RUN_CONTAINER_PREFIX = ".iroha-js-native-build-run-";
const RUN_CARGO_TARGET_NAME = "cargo-target";
const PUBLISH_LOCK_SUFFIX = ".publish-lock";
const MAX_CARGO_JSON_BYTES = 64 * 1024 * 1024;
const INTENDED_PACKAGE = "iroha_js_host";
const UUID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
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

function createRunContainer(repoRoot, targetRoot) {
  const temporaryParent = canonicalDirectory(
    tmpdir(),
    "Native build operating-system temporary directory",
  ).canonicalPath;
  const runUuid = newRunUuid();
  const runContainer = join(
    temporaryParent,
    `${RUN_CONTAINER_PREFIX}${runUuid}`,
  );
  mkdirSync(runContainer, { mode: 0o700 });
  try {
    const canonical = canonicalDirectory(
      runContainer,
      "Native build private run container",
    );
    if (
      dirname(canonical.canonicalPath) !== temporaryParent ||
      basename(canonical.canonicalPath) !== `${RUN_CONTAINER_PREFIX}${runUuid}`
    ) {
      throw new Error(
        "Native build run container is not the exact direct operating-system temporary child.",
      );
    }
    assertDisjointPathAncestry(
      repoRoot,
      canonical.canonicalPath,
      "Native build run container and repository",
    );
    assertDisjointPathAncestry(
      targetRoot,
      canonical.canonicalPath,
      "Native build run container and publication target directory",
    );
    return Object.freeze({
      identity: canonical.identity,
      parent: temporaryParent,
      path: canonical.canonicalPath,
      uuid: runUuid,
    });
  } catch (error) {
    rmdirSync(runContainer);
    throw error;
  }
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
  const metadata = lstatSync(container.path, { bigint: true });
  if (
    !metadata.isDirectory() ||
    metadata.isSymbolicLink() ||
    metadata.dev !== container.identity.dev ||
    metadata.ino !== container.identity.ino ||
    realpathSync(container.path) !== container.path ||
    realpathSync(dirname(container.path)) !== container.parent
  ) {
    throw new Error("Native build private run container changed identity.");
  }
}

function cleanupRunContainer(container) {
  assertRunContainerIdentity(container);
  // This is deliberately non-recursive: an unexpected entry is preserved and
  // makes cleanup fail closed instead of allowing an unsafe broad deletion.
  rmdirSync(container.path);
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
  return Object.freeze({
    identity: canonical.identity,
    path: canonical.canonicalPath,
    runContainerPath: container.path,
    runUuid: container.uuid,
  });
}

function cleanupRunCargoTarget(target, container) {
  assertRunContainerIdentity(container);
  if (
    typeof target?.path !== "string" ||
    target.runContainerPath !== container.path ||
    target.runUuid !== container.uuid ||
    target.path !== join(container.path, RUN_CARGO_TARGET_NAME)
  ) {
    throw new Error("Refusing to remove an invalid private Cargo target.");
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
  rmSync(target.path, { recursive: true });
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

function createStagedNative(sourcePath, finalPath, sourceSeal) {
  const finalDirectory = dirname(finalPath);
  const stagePrefix = `.${basename(finalPath)}.stage-`;
  const stageUuid = newRunUuid();
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

function acquirePublicationLock(finalPath) {
  const directory = dirname(finalPath);
  const lockPath = join(
    directory,
    `.${basename(finalPath)}${PUBLISH_LOCK_SUFFIX}`,
  );
  try {
    mkdirSync(lockPath, { mode: 0o700 });
  } catch (error) {
    if (error?.code === "EEXIST") {
      throw new Error("Another native build publication is in progress.");
    }
    throw error;
  }
  const metadata = lstatSync(lockPath, { bigint: true });
  if (
    !metadata.isDirectory() ||
    metadata.isSymbolicLink() ||
    realpathSync(lockPath) !== lockPath
  ) {
    throw new Error("Native build publication lock is unsafe.");
  }
  syncDirectory(directory);
  return Object.freeze({
    directory,
    identity: Object.freeze({ dev: metadata.dev, ino: metadata.ino }),
    path: lockPath,
  });
}

function releasePublicationLock(lock) {
  const metadata = lstatSync(lock.path, { bigint: true });
  if (
    dirname(lock.path) !== lock.directory ||
    !metadata.isDirectory() ||
    metadata.isSymbolicLink() ||
    metadata.dev !== lock.identity.dev ||
    metadata.ino !== lock.identity.ino ||
    realpathSync(lock.path) !== lock.path
  ) {
    throw new Error("Native build publication lock changed identity.");
  }
  rmdirSync(lock.path);
  syncDirectory(lock.directory);
}

function publishStagedNative({
  finalPath,
  invalidateProvenance,
  provenance,
  publicationFailpoint,
  stage,
  writeProvenance,
}) {
  const finalDirectory = dirname(finalPath);
  const retiredPrefix = `.${basename(finalPath)}.retired-`;
  let lock;
  try {
    lock = acquirePublicationLock(finalPath);
  } catch (error) {
    try {
      if (lstatOrNull(stage.path) !== null) {
        removeOwnedRegularFile(
          stage,
          finalDirectory,
          stage.prefix,
          "Native build unpublished staging file",
        );
      }
    } catch (cleanupError) {
      throw new AggregateError(
        [error, cleanupError],
        "Native build publication lock and safe staging cleanup both failed.",
      );
    }
    throw error;
  }
  let binaryPublished = false;
  let publishedOutputIdentity;
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
      const retiredUuid = newRunUuid();
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
    return finalPath;
  } finally {
    try {
      if (stagePresent && lstatOrNull(stage.path) !== null) {
        removeOwnedRegularFile(
          stage,
          finalDirectory,
          stage.prefix,
          "Native build staging file",
        );
      }
      if (
        binaryPublished &&
        retired !== undefined &&
        lstatOrNull(retired.path) !== null
      ) {
        removeOwnedRegularFile(
          retired,
          finalDirectory,
          retiredPrefix,
          "Native build retired binary",
        );
      }
    } finally {
      releasePublicationLock(lock);
    }
  }
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

function packageIdNamesIntendedPackage(packageId) {
  return (
    typeof packageId === "string" &&
    /(?:^|[\/# ])iroha_js_host(?:$|[@# ])/u.test(packageId)
  );
}

function exactStringArray(value, expected) {
  return (
    Array.isArray(value) &&
    value.length === expected.length &&
    value.every((item, index) => item === expected[index])
  );
}

function artifactMightBeIntended(message, expectedNativePath, expectedSourcePath) {
  const filenames = Array.isArray(message.filenames) ? message.filenames : [];
  return (
    message.target?.name === INTENDED_PACKAGE ||
    packageIdNamesIntendedPackage(message.package_id) ||
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
  { nativePath, snapshotRoot },
) {
  const expectedNativePath = resolve(nativePath);
  const expectedSourcePath = resolve(
    snapshotRoot,
    "crates",
    INTENDED_PACKAGE,
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
    !packageIdNamesIntendedPackage(artifact.package_id) ||
    artifact.target?.name !== INTENDED_PACKAGE ||
    !exactStringArray(artifact.target?.kind, ["cdylib"]) ||
    !exactStringArray(artifact.target?.crate_types, ["cdylib"]) ||
    artifact.target?.src_path !== expectedSourcePath ||
    realpathSync(artifact.target.src_path) !== expectedSourcePath ||
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
  publicationFailpoint = () => {},
  writeProvenance = writeNativeBuildProvenance,
} = {}) {
  const cargoProfile = resolveNativeBuildProfile(env);
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
  const runContainer = createRunContainer(
    canonicalRepoRoot,
    targetRoot,
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
      nativePath: runNativePath,
      snapshotRoot: snapshot.snapshotRoot,
    });
    const outputAfterCargo = sealNativeOutput(runNativePath);
    const sourceAfter = verifySourceSnapshot(snapshot);
    const provenance = createNativeBuildProvenance({
      cargoProfile,
      nativePath: runNativePath,
      sourceBefore,
      sourceAfter,
    });
    if (
      provenance.native_sha256 !== outputAfterCargo.sha256 ||
      !sameOutputIdentity(
        outputAfterCargo.identity,
        outputIdentityOrNull(runNativePath),
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
    const stage = createStagedNative(
      runNativePath,
      finalNativePath,
      outputAfterCargo,
    );
    publishStagedNative({
      finalPath: finalNativePath,
      invalidateProvenance,
      provenance,
      publicationFailpoint,
      stage,
      writeProvenance,
    });
    return 0;
  } finally {
    try {
      if (snapshotPlacementVerified) cleanupSourceSnapshot(snapshot);
    } finally {
      try {
        if (runCargoTarget !== undefined) {
          cleanupRunCargoTarget(runCargoTarget, runContainer);
        }
      } finally {
        cleanupRunContainer(runContainer);
      }
    }
  }
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  process.exitCode = runNativeBuild();
}
