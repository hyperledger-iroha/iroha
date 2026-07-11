import { createHash, randomUUID } from "node:crypto";
import {
  closeSync,
  cpSync,
  existsSync,
  lstatSync,
  openSync,
  readFileSync,
  realpathSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
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
  "curveRegistry.js",
  "ivmArtifact.js",
  "toriiClient.js",
  "kotodamaCompiler/index.js",
];
const STAGING_PREFIX = ".dist-stage-";
const BACKUP_PREFIX = ".dist-backup-";
const FAILED_PREFIX = ".dist-failed-";
const TEST_FAILPOINTS = new Set(["after-backup", "after-publish"]);

const delay = (milliseconds) => new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));

function lockPathFor(root) {
  return join(root, ".build-dist.lock");
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

function readLockOwner(lockPath) {
  try {
    const parsed = JSON.parse(readFileSync(lockPath, "utf8"));
    return {
      pid: Number(parsed?.pid),
      token: typeof parsed?.token === "string" ? parsed.token : undefined,
    };
  } catch {
    return {};
  }
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
} = {}) {
  const resolvedRoot = resolve(root);
  const lockPath = lockPathFor(resolvedRoot);
  const startedAt = Date.now();
  while (true) {
    const token = randomUUID();
    try {
      const descriptor = openSync(lockPath, "wx", 0o600);
      try {
        writeFileSync(
          descriptor,
          `${JSON.stringify({ pid: process.pid, token, createdAt: new Date().toISOString() })}\n`,
          { encoding: "utf8" },
        );
        return { descriptor, lockPath, root: resolvedRoot, token };
      } catch (error) {
        closeSync(descriptor);
        rmSync(lockPath, { force: true });
        throw error;
      }
    } catch (error) {
      if (error?.code !== "EEXIST") throw error;
    }

    try {
      const stale = Date.now() - statSync(lockPath).mtimeMs > staleLockMs;
      if (stale) {
        const owner = readLockOwner(lockPath);
        if (!processIsRunning(owner.pid)) {
          unlinkSync(lockPath);
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
  let releaseError;
  try {
    const owner = readLockOwner(lock.lockPath);
    if (owner.token !== lock.token || owner.pid !== process.pid) {
      throw new Error(`build:dist lost ownership of ${lock.lockPath}`);
    }
    unlinkSync(lock.lockPath);
  } catch (error) {
    if (error?.code !== "ENOENT") releaseError = error;
  } finally {
    closeSync(lock.descriptor);
  }
  if (releaseError) throw releaseError;
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

function recoverInterruptedPublication(root) {
  const dist = join(root, "dist");
  const backups = publicationArtifacts(root, BACKUP_PREFIX);

  if (!existsSync(dist)) {
    const recoverable = backups.find(isValidDistribution);
    if (recoverable) renameSync(recoverable, dist);
  } else if (!isValidDistribution(dist)) {
    const recoverable = backups.find(isValidDistribution);
    if (recoverable) {
      const failed = join(root, `${FAILED_PREFIX}${process.pid}-${randomUUID()}`);
      renameSync(dist, failed);
      try {
        renameSync(recoverable, dist);
      } catch (error) {
        renameSync(failed, dist);
        throw error;
      }
      rmSync(failed, { recursive: true, force: true });
    }
  }

  for (const backup of backups) {
    if (existsSync(backup)) rmSync(backup, { recursive: true, force: true });
  }
  for (const staging of publicationArtifacts(root, STAGING_PREFIX)) {
    rmSync(staging, { recursive: true, force: true });
  }
  for (const failed of publicationArtifacts(root, FAILED_PREFIX)) {
    rmSync(failed, { recursive: true, force: true });
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

function publishStagingTree({ root, dist, staging, failpoint }) {
  const backup = join(root, `${BACKUP_PREFIX}${process.pid}-${randomUUID()}`);
  const failed = join(root, `${FAILED_PREFIX}${process.pid}-${randomUUID()}`);
  let previousMoved = false;
  let stagingPublished = false;
  try {
    if (existsSync(dist)) {
      renameSync(dist, backup);
      previousMoved = true;
    }
    triggerTestFailpoint(failpoint, "after-backup");

    renameSync(staging, dist);
    stagingPublished = true;
    triggerTestFailpoint(failpoint, "after-publish");
    validateDistOutputs(dist);
  } catch (error) {
    let rollbackError;
    try {
      if (stagingPublished && existsSync(dist)) renameSync(dist, failed);
      if (previousMoved && existsSync(backup)) renameSync(backup, dist);
      if (existsSync(failed)) rmSync(failed, { recursive: true, force: true });
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
  if (previousMoved) rmSync(backup, { recursive: true, force: true });
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
    recoverInterruptedPublication(resolvedRoot);
    cpSync(src, staging, { recursive: true, errorOnExist: true });
    validateDistOutputs(staging);
    const stagingDigest = directoryDigest(staging);
    let distDigest;
    try {
      if (existsSync(dist)) distDigest = directoryDigest(dist);
    } catch {
      // An invalid existing tree must be replaced by the validated staging tree.
    }
    if (distDigest === stagingDigest) {
      return { changed: false, digest: distDigest };
    }
    publishStagingTree({ root: resolvedRoot, dist, staging, failpoint });
    return { changed: true, digest: stagingDigest };
  } catch (error) {
    resultError = error;
    throw error;
  } finally {
    const cleanupErrors = [];
    try {
      rmSync(staging, { recursive: true, force: true });
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
