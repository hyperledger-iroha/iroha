import { createHash, randomUUID } from "node:crypto";
import {
  closeSync,
  constants,
  cpSync,
  existsSync,
  fstatSync,
  fsyncSync,
  linkSync,
  lstatSync,
  openSync,
  readFileSync,
  realpathSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = resolve(__filename, "..");
const PROJECT_ROOT = resolve(__dirname, "..");
const ROOT = process.env.IROHA_JS_BUILD_DIST_ROOT
  ? resolve(process.env.IROHA_JS_BUILD_DIST_ROOT)
  : PROJECT_ROOT;
const LOCK_TIMEOUT_MS = 60_000;
const STALE_LOCK_MS = 5 * 60_000;
const REQUIRED_OUTPUTS = [
  "address.js",
  "atomicPrivateSettlement.js",
  "browser.js",
  "curveRegistry.js",
  "ivmArtifact.js",
  "native.js",
  "nativeArtifactHash.js",
  "numericV1.js",
  "strictLosslessJson.js",
  "sorafsOrderbookSubmission.js",
  "sorafsOrderbookSubmission.d.ts",
  "smartContractDeploymentSubmit.js",
  "sumeragiTyped.js",
  "tairaTestnetProfile.js",
  "toriiBrowserClient.js",
  "toriiClient.js",
  "toriiOptional.js",
  "kotodamaCompiler/index.js",
  "kotodamaCompiler/browser.js",
  "kotodamaCompiler/client.js",
  "kotodamaCompiler/nativeBridge.js",
  "kotodamaCompiler/normalize.js",
];
const STAGING_PREFIX = ".dist-stage-";
const BACKUP_PREFIX = ".dist-backup-";
const FAILED_PREFIX = ".dist-failed-";
const TEST_FAILPOINTS = new Set(["after-backup", "after-publish"]);
const RETIRED_LOCK_PREFIX = ".build-dist.lock.retired-";

const delay = (milliseconds) => new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));

function lockPathFor(root) {
  return join(root, ".build-dist.lock");
}

const UNSUPPORTED_DIRECTORY_SYNC_CODES = new Set([
  "EACCES",
  "EBADF",
  "EINVAL",
  "EISDIR",
  "ENOTSUP",
  "EPERM",
]);

/** Flush a directory entry update when the host filesystem supports it. */
export function syncDirectory(directory) {
  let descriptor;
  try {
    descriptor = openSync(directory, "r");
    fsyncSync(descriptor);
  } catch (error) {
    if (!UNSUPPORTED_DIRECTORY_SYNC_CODES.has(error?.code)) throw error;
  } finally {
    if (descriptor !== undefined) closeSync(descriptor);
  }
}

function syncFile(path) {
  const descriptor = openSync(path, constants.O_RDONLY);
  try {
    fsyncSync(descriptor);
  } finally {
    closeSync(descriptor);
  }
}

/** Persist every staged file and directory before its tree is made public. */
function syncTree(directory) {
  const metadata = lstatSync(directory);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error(`build:dist cannot sync non-directory root: ${directory}`);
  }
  const entries = readdirSync(directory, { withFileTypes: true }).sort((left, right) =>
    left.name.localeCompare(right.name),
  );
  for (const entry of entries) {
    const entryPath = join(directory, entry.name);
    const entryMetadata = lstatSync(entryPath);
    if (entryMetadata.isDirectory()) {
      syncTree(entryPath);
    } else if (entryMetadata.isSymbolicLink()) {
      throw new Error(`build:dist cannot sync symbolic link: ${entryPath}`);
    } else if (entryMetadata.isFile()) {
      syncFile(entryPath);
    } else {
      throw new Error(`build:dist cannot sync unsupported entry: ${entryPath}`);
    }
  }
  syncDirectory(directory);
}

function processIsRunning(pid) {
  if (!Number.isSafeInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return error?.code === "EPERM";
  }
}

function parseLockOwner(bytes) {
  try {
    const parsed = JSON.parse(bytes.toString("utf8"));
    return {
      pid: Number(parsed?.pid),
      token: typeof parsed?.token === "string" ? parsed.token : undefined,
      createdAt: typeof parsed?.createdAt === "string" ? parsed.createdAt : undefined,
    };
  } catch {
    return {};
  }
}

function lockIdentity(metadata) {
  return {
    dev: metadata.dev,
    ino: metadata.ino,
    size: metadata.size,
  };
}

function sameLockIdentity(left, right) {
  return left?.dev === right?.dev && left?.ino === right?.ino;
}

function snapshotLock(lockPath) {
  const pathMetadata = lstatSync(lockPath);
  if (!pathMetadata.isFile() || pathMetadata.isSymbolicLink()) {
    throw new Error(`build:dist lock must be a regular non-symbolic-link file: ${lockPath}`);
  }
  const descriptor = openSync(lockPath, constants.O_RDONLY);
  try {
    const metadata = fstatSync(descriptor);
    if (!metadata.isFile() || !sameLockIdentity(lockIdentity(pathMetadata), lockIdentity(metadata))) {
      throw new Error(`build:dist lock changed while it was being examined: ${lockPath}`);
    }
    const bytes = readFileSync(descriptor);
    return {
      digest: createHash("sha256").update(bytes).digest("hex"),
      identity: lockIdentity(metadata),
      mtimeMs: metadata.mtimeMs,
      owner: parseLockOwner(bytes),
    };
  } finally {
    closeSync(descriptor);
  }
}

function sameLockSnapshot(left, right) {
  return (
    sameLockIdentity(left?.identity, right?.identity) &&
    left?.digest === right?.digest &&
    left?.mtimeMs === right?.mtimeMs &&
    left?.owner?.pid === right?.owner?.pid &&
    left?.owner?.token === right?.owner?.token
  );
}

function restoreUnexpectedRetiredLock(retiredPath, lockPath) {
  try {
    // Hard-linking is exclusive at the public pathname and cannot overwrite a
    // lock that another contender acquired while the candidate was examined.
    // It also works when Windows refuses rename-over-existing-file.
    linkSync(retiredPath, lockPath);
    syncDirectory(dirname(lockPath));
    rmSync(retiredPath, { force: true });
    syncDirectory(dirname(lockPath));
    return true;
  } catch (error) {
    if (error?.code === "EEXIST") return false;
    throw error;
  }
}

function retireLockCandidate(root, lockPath, observed) {
  const retiredPath = join(root, `${RETIRED_LOCK_PREFIX}${process.pid}-${randomUUID()}`);
  renameSync(lockPath, retiredPath);
  syncDirectory(root);
  const moved = snapshotLock(retiredPath);
  if (!sameLockSnapshot(observed, moved)) {
    const restored = restoreUnexpectedRetiredLock(retiredPath, lockPath);
    if (!restored) {
      throw new Error(
        `build:dist lock changed during stale takeover; preserved the replacement at ${retiredPath}`,
      );
    }
    return false;
  }
  rmSync(retiredPath, { force: true });
  syncDirectory(root);
  return true;
}

/** Fail closed if the public lock pathname no longer names this transaction. */
export function assertDistLockOwnership(lock) {
  if (!lock || typeof lock !== "object") {
    throw new TypeError("build:dist lock ownership requires a lock handle");
  }
  const current = snapshotLock(lock.lockPath);
  if (
    current.owner.token !== lock.token ||
    current.owner.pid !== lock.pid ||
    current.digest !== lock.digest ||
    !sameLockIdentity(current.identity, lock.identity)
  ) {
    throw new Error(`build:dist lost ownership of ${lock.lockPath}`);
  }
  return current;
}

/**
 * Acquire the distribution publication lock.
 *
 * Release/package readers use this same lock while snapshotting or packing
 * `dist`, so a portable two-rename directory replacement is never observed by
 * those readers halfway through publication.
 */
export async function acquireDistLock({
  root = ROOT,
  timeoutMs = LOCK_TIMEOUT_MS,
  staleLockMs = STALE_LOCK_MS,
  onLockCreated,
  onStaleCandidate,
} = {}) {
  if (!Number.isFinite(timeoutMs) || timeoutMs < 0) {
    throw new TypeError("build:dist lock timeoutMs must be a non-negative finite number");
  }
  if (!Number.isFinite(staleLockMs) || staleLockMs < 0) {
    throw new TypeError("build:dist lock staleLockMs must be a non-negative finite number");
  }
  if (onStaleCandidate !== undefined && typeof onStaleCandidate !== "function") {
    throw new TypeError("build:dist onStaleCandidate must be a function");
  }
  if (onLockCreated !== undefined && typeof onLockCreated !== "function") {
    throw new TypeError("build:dist onLockCreated must be a function");
  }
  const resolvedRoot = resolve(root);
  const lockPath = lockPathFor(resolvedRoot);
  const startedAt = Date.now();
  while (true) {
    const token = randomUUID();
    let descriptor;
    let createdDigest;
    let createdIdentity;
    try {
      descriptor = openSync(lockPath, "wx", 0o600);
      const ownerRecord = `${JSON.stringify({
        pid: process.pid,
        token,
        createdAt: new Date().toISOString(),
      })}\n`;
      writeFileSync(descriptor, ownerRecord, { encoding: "utf8" });
      fsyncSync(descriptor);
      createdDigest = createHash("sha256").update(ownerRecord, "utf8").digest("hex");
      createdIdentity = lockIdentity(fstatSync(descriptor));
      closeSync(descriptor);
      descriptor = undefined;
      onLockCreated?.({ lockPath });
      syncDirectory(resolvedRoot);
      const lock = {
        digest: createdDigest,
        identity: createdIdentity,
        lockPath,
        pid: process.pid,
        root: resolvedRoot,
        token,
      };
      assertDistLockOwnership(lock);
      return lock;
    } catch (error) {
      if (descriptor !== undefined) {
        try {
          createdIdentity ??= lockIdentity(fstatSync(descriptor));
        } finally {
          closeSync(descriptor);
          descriptor = undefined;
        }
      }
      if (createdIdentity !== undefined) {
        try {
          const observed = snapshotLock(lockPath);
          if (sameLockIdentity(createdIdentity, observed.identity)) {
            retireLockCandidate(resolvedRoot, lockPath, observed);
          }
        } catch (cleanupError) {
          if (cleanupError?.code !== "ENOENT") {
            throw new AggregateError(
              [error, cleanupError],
              "build:dist failed to initialize its lock and could not clean it safely",
            );
          }
        }
      }
      if (error?.code !== "EEXIST") throw error;
    }

    try {
      const observed = snapshotLock(lockPath);
      const stale = Date.now() - observed.mtimeMs > staleLockMs;
      if (stale) {
        if (!processIsRunning(observed.owner.pid)) {
          onStaleCandidate?.({
            lockPath,
            owner: Object.freeze({ ...observed.owner }),
          });
          if (retireLockCandidate(resolvedRoot, lockPath, observed)) {
            continue;
          }
          continue;
        }
      }
    } catch (error) {
      if (error?.code === "ENOENT") continue;
      throw error;
    }

    if (Date.now() - startedAt >= timeoutMs) {
      throw new Error(`build:dist timed out waiting for ${lockPath}`);
    }
    await delay(50);
  }
}

export function releaseDistLock(lock) {
  if (!lock) return;
  const observed = assertDistLockOwnership(lock);
  if (!retireLockCandidate(lock.root, lock.lockPath, observed)) {
    throw new Error(`build:dist lost ownership of ${lock.lockPath} during release`);
  }
}

export function validateDistOutputs(directory) {
  if (!lstatSync(directory).isDirectory()) {
    throw new Error(`build:dist output root must be a directory: ${directory}`);
  }
  for (const fileName of REQUIRED_OUTPUTS) {
    const output = join(directory, fileName);
    if (!existsSync(output) || !lstatSync(output).isFile()) {
      throw new Error(`build:dist missing expected output: ${fileName}`);
    }
  }
}

export function directoryDigest(directory) {
  if (!lstatSync(directory).isDirectory()) {
    throw new Error(`build:dist cannot publish non-directory root: ${directory}`);
  }
  const hash = createHash("sha256");
  const visit = (current, relative) => {
    const entries = readdirSync(current, { withFileTypes: true }).sort((left, right) =>
      left.name.localeCompare(right.name),
    );
    for (const entry of entries) {
      const entryRelative = relative ? `${relative}/${entry.name}` : entry.name;
      const entryPath = join(current, entry.name);
      const metadata = lstatSync(entryPath);
      if (metadata.isDirectory()) {
        hash.update(`d:${entryRelative}\0`);
        visit(entryPath, entryRelative);
      } else if (metadata.isSymbolicLink()) {
        throw new Error(`build:dist cannot publish symbolic link: ${entryPath}`);
      } else if (metadata.isFile()) {
        hash.update(`f:${entryRelative}:${metadata.mode & 0o777}\0`);
        hash.update(readFileSync(entryPath));
        hash.update("\0");
      } else {
        throw new Error(`build:dist cannot publish unsupported entry: ${entryPath}`);
      }
    }
  };
  visit(directory, "");
  return hash.digest("hex");
}

function publicationArtifacts(root, prefix) {
  return readdirSync(root)
    .filter((entry) => entry.startsWith(prefix))
    .map((entry) => join(root, entry))
    .sort((left, right) => lstatSync(right).mtimeMs - lstatSync(left).mtimeMs);
}

function isValidDistribution(directory) {
  try {
    validateDistOutputs(directory);
    directoryDigest(directory);
    return true;
  } catch {
    return false;
  }
}

function recoverInterruptedPublication(root, lock) {
  const dist = join(root, "dist");
  const backups = publicationArtifacts(root, BACKUP_PREFIX);

  if (!existsSync(dist)) {
    const recoverableBackups = backups.filter(isValidDistribution);
    if (recoverableBackups.length > 1) {
      throw new Error(
        "build:dist found multiple valid crash backups and cannot prove their generation order",
      );
    }
    const recoverable = recoverableBackups[0];
    if (recoverable) {
      assertDistLockOwnership(lock);
      renameSync(recoverable, dist);
      syncDirectory(root);
    }
  } else if (!isValidDistribution(dist)) {
    const recoverableBackups = backups.filter(isValidDistribution);
    if (recoverableBackups.length > 1) {
      throw new Error(
        "build:dist found multiple valid crash backups and cannot prove their generation order",
      );
    }
    const recoverable = recoverableBackups[0];
    if (recoverable) {
      const failed = join(root, `${FAILED_PREFIX}${process.pid}-${randomUUID()}`);
      assertDistLockOwnership(lock);
      renameSync(dist, failed);
      try {
        assertDistLockOwnership(lock);
        renameSync(recoverable, dist);
      } catch (error) {
        assertDistLockOwnership(lock);
        renameSync(failed, dist);
        throw error;
      }
      syncDirectory(root);
      assertDistLockOwnership(lock);
      rmSync(failed, { recursive: true, force: true });
      syncDirectory(root);
    }
  }

  for (const backup of backups) {
    if (existsSync(backup)) {
      assertDistLockOwnership(lock);
      rmSync(backup, { recursive: true, force: true });
      syncDirectory(root);
    }
  }
  for (const staging of publicationArtifacts(root, STAGING_PREFIX)) {
    assertDistLockOwnership(lock);
    rmSync(staging, { recursive: true, force: true });
    syncDirectory(root);
  }
  for (const failed of publicationArtifacts(root, FAILED_PREFIX)) {
    assertDistLockOwnership(lock);
    rmSync(failed, { recursive: true, force: true });
    syncDirectory(root);
  }
}

function activeTestFailpoint(root) {
  const requested = process.env.IROHA_JS_BUILD_DIST_TEST_FAILPOINT;
  if (!requested) return undefined;

  const explicitRoot = process.env.IROHA_JS_BUILD_DIST_ROOT;
  const actualRoot = realpathSync(root);
  const actualProjectRoot = realpathSync(PROJECT_ROOT);
  const enabled =
    process.env.IROHA_JS_BUILD_DIST_TEST_MODE === "1" &&
    explicitRoot !== undefined &&
    realpathSync(resolve(explicitRoot)) === actualRoot &&
    actualRoot !== actualProjectRoot;
  if (!enabled) return undefined;
  if (!TEST_FAILPOINTS.has(requested)) {
    throw new Error(`build:dist unknown test failpoint: ${requested}`);
  }
  return requested;
}

function triggerTestFailpoint(failpoint, point) {
  if (failpoint === point) {
    throw new Error(`build:dist injected test failure at ${point}`);
  }
}

function publishStagingTree({ root, dist, staging, stagingDigest, failpoint, lock }) {
  const backup = join(root, `${BACKUP_PREFIX}${process.pid}-${randomUUID()}`);
  const failed = join(root, `${FAILED_PREFIX}${process.pid}-${randomUUID()}`);
  let previousMoved = false;
  let stagingPublished = false;
  try {
    if (existsSync(dist)) {
      assertDistLockOwnership(lock);
      renameSync(dist, backup);
      syncDirectory(root);
      previousMoved = true;
    }
    triggerTestFailpoint(failpoint, "after-backup");

    assertDistLockOwnership(lock);
    renameSync(staging, dist);
    syncDirectory(root);
    stagingPublished = true;
    triggerTestFailpoint(failpoint, "after-publish");
    validateDistOutputs(dist);
    const publishedDigest = directoryDigest(dist);
    if (publishedDigest !== stagingDigest) {
      throw new Error("build:dist published tree changed after staged verification");
    }
  } catch (error) {
    let rollbackError;
    try {
      if (stagingPublished && existsSync(dist)) {
        assertDistLockOwnership(lock);
        renameSync(dist, failed);
      }
      if (previousMoved && existsSync(backup)) {
        assertDistLockOwnership(lock);
        renameSync(backup, dist);
      }
      syncDirectory(root);
      if (existsSync(failed)) {
        assertDistLockOwnership(lock);
        rmSync(failed, { recursive: true, force: true });
        syncDirectory(root);
      }
    } catch (candidate) {
      rollbackError = candidate;
    }
    if (rollbackError) {
      throw new AggregateError(
        [error, rollbackError],
        "build:dist publication failed and the last-good distribution could not be restored",
      );
    }
    throw error;
  }
  if (previousMoved) {
    assertDistLockOwnership(lock);
    rmSync(backup, { recursive: true, force: true });
    syncDirectory(root);
  }
}

export async function buildDistribution({ root = ROOT } = {}) {
  const resolvedRoot = resolve(root);
  const dist = join(resolvedRoot, "dist");
  const src = join(resolvedRoot, "src");
  const staging = join(resolvedRoot, `${STAGING_PREFIX}${process.pid}-${randomUUID()}`);
  const failpoint = activeTestFailpoint(resolvedRoot);
  let lock;
  let resultError;
  try {
    lock = await acquireDistLock({ root: resolvedRoot });
    recoverInterruptedPublication(resolvedRoot, lock);
    cpSync(src, staging, { recursive: true, errorOnExist: true });
    validateDistOutputs(staging);
    const stagingDigest = directoryDigest(staging);
    syncTree(staging);
    if (directoryDigest(staging) !== stagingDigest) {
      throw new Error("build:dist staging tree changed while it was being persisted");
    }
    let distDigest;
    try {
      if (existsSync(dist)) distDigest = directoryDigest(dist);
    } catch {
      // An invalid existing tree must be replaced by the validated staging tree.
    }
    if (distDigest === stagingDigest) {
      return { changed: false, digest: distDigest };
    }
    publishStagingTree({
      root: resolvedRoot,
      dist,
      staging,
      stagingDigest,
      failpoint,
      lock,
    });
    return { changed: true, digest: stagingDigest };
  } catch (error) {
    resultError = error;
    throw error;
  } finally {
    const cleanupErrors = [];
    try {
      if (lock) assertDistLockOwnership(lock);
      rmSync(staging, { recursive: true, force: true });
      if (lock) syncDirectory(resolvedRoot);
    } catch (error) {
      cleanupErrors.push(error);
    }
    if (lock) {
      try {
        releaseDistLock(lock);
      } catch (error) {
        cleanupErrors.push(error);
      }
    }
    if (cleanupErrors.length > 0) {
      if (resultError) {
        throw new AggregateError(
          [resultError, ...cleanupErrors],
          `build:dist failed (${resultError.message}) and cleanup was incomplete`,
        );
      }
      if (cleanupErrors.length === 1) throw cleanupErrors[0];
      throw new AggregateError(cleanupErrors, "build:dist cleanup was incomplete");
    }
  }
}

if (process.argv[1] && resolve(process.argv[1]) === __filename) {
  buildDistribution().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
