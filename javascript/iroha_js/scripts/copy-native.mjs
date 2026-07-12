#!/usr/bin/env node
/**
 * Publish the compiled `iroha_js_host` dynamic library as a verified `.node`
 * artifact. Publication is transactional and repeatable on platforms where a
 * rename cannot replace an existing file (notably Windows).
 */
import { createHash, randomUUID } from "node:crypto";
import {
  chmodSync,
  closeSync,
  constants,
  copyFileSync,
  existsSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  readFileSync,
  readSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, isAbsolute, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import { verifyNativeBinding } from "../src/native.js";
import {
  acquireDistLock,
  assertDistLockOwnership,
  releaseDistLock,
  syncDirectory,
} from "./build-dist.mjs";

const __filename = fileURLToPath(import.meta.url);
const scriptDir = dirname(__filename);
const repoRoot = join(scriptDir, "..", "..", "..");
const NATIVE_FILENAME = "iroha_js_host.node";
const CHECKSUM_FILENAME = "iroha_js_host.checksums.json";
const TRANSACTION_PREFIX = ".iroha-js-host-txn-";
const CLEANUP_PREFIX = ".iroha-js-host-cleanup-";
const JOURNAL_VERSION = 1;
const MAX_JOURNAL_BYTES = 16 * 1024;
const MAX_CHECKSUM_MANIFEST_BYTES = 1024 * 1024;
const JOURNAL_PATTERN = /^journal-(\d{6})\.json$/u;
const JOURNAL_TEMP_PATTERN = /^\.journal-(\d{6})-[0-9a-f-]+\.tmp$/u;
const NEXT_NATIVE_FILENAME = `${NATIVE_FILENAME}.next`;
const NEXT_MANIFEST_FILENAME = `${CHECKSUM_FILENAME}.next`;
const PREVIOUS_NATIVE_FILENAME = `${NATIVE_FILENAME}.previous`;
const PREVIOUS_MANIFEST_FILENAME = `${CHECKSUM_FILENAME}.previous`;
const DISCARDED_NATIVE_FILENAME = `${NATIVE_FILENAME}.discarded-next`;
const DISCARDED_MANIFEST_FILENAME = `${CHECKSUM_FILENAME}.discarded-next`;
const TEST_FAILPOINTS = new Set(["after-backup", "after-native", "after-manifest"]);
const JOURNAL_PHASES = Object.freeze([
  "prepared",
  "previous-manifest-moved",
  "previous-native-moved",
  "next-native-moved",
  "next-manifest-moved",
  "published-verified",
  "committed",
]);
const JOURNAL_PHASE_SET = new Set(JOURNAL_PHASES);

export const REQUIRED_NATIVE_EXPORTS = Object.freeze([
  "noritoEncodeInstruction",
  "noritoDecodeInstruction",
  "compileKotodama",
]);

function assertRegularFile(path, label) {
  if (!existsSync(path)) {
    throw new Error(`${label} is missing at ${path}`);
  }
  const metadata = lstatSync(path);
  if (!metadata.isFile() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a regular non-symbolic-link file: ${path}`);
  }
  return metadata;
}

function assertChecksumManifest(path, label) {
  const metadata = assertRegularFile(path, label);
  if (metadata.size > MAX_CHECKSUM_MANIFEST_BYTES) {
    throw new Error(`${label} exceeds the native checksum manifest size limit`);
  }
  return metadata;
}

function assertDirectory(path, label) {
  const metadata = lstatSync(path);
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error(`${label} must be a regular non-symbolic-link directory: ${path}`);
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

function defaultSignNative(path, { platform, cwd }) {
  if (platform !== "darwin") return;
  const sign = spawnSync("codesign", ["--force", "--sign", "-", path], {
    cwd,
    stdio: "inherit",
    env: process.env,
  });
  if (sign.status !== 0) {
    throw new Error(
      `Failed to ad-hoc sign ${path}; macOS requires a valid signature for Node.js native addons.`,
    );
  }
}

/** Load a staged addon in a short-lived process and require its public methods. */
export function probeNativeBindingExports(
  bindingPath,
  requiredExports = REQUIRED_NATIVE_EXPORTS,
) {
  if (
    !Array.isArray(requiredExports) ||
    requiredExports.length === 0 ||
    requiredExports.some(
      (name) => typeof name !== "string" || !/^[A-Za-z][A-Za-z0-9]*$/u.test(name),
    )
  ) {
    throw new TypeError("required native exports must be a non-empty identifier array");
  }
  const probeSource = String.raw`
const bindingPath = process.argv[1];
let binding;
if (/\.(?:cjs|js)$/iu.test(bindingPath)) {
  // Test-only fixture support; production stages a native library.
  binding = require(bindingPath);
} else {
  // Publication deliberately retains the legacy .node.next transaction name
  // for crash-recovery compatibility. require() dispatches by the last
  // extension and would parse that Mach-O/ELF/PE file as JavaScript, whereas
  // process.dlopen loads the addon independent of its private filename.
  const nativeModule = { exports: {} };
  process.dlopen(nativeModule, bindingPath);
  binding = nativeModule.exports;
}
const required = JSON.parse(process.argv[2]);
const missing = required.filter((name) => typeof binding[name] !== "function");
if (missing.length > 0) {
  process.stderr.write("missing required native exports: " + missing.join(", "));
  process.exitCode = 1;
}
`;
  const probe = spawnSync(
    process.execPath,
    ["--eval", probeSource, bindingPath, JSON.stringify(requiredExports)],
    {
      cwd: repoRoot,
      encoding: "utf8",
      env: process.env,
      timeout: 30_000,
      windowsHide: true,
      maxBuffer: 64 * 1024,
    },
  );
  if (probe.error) {
    throw new Error(`Failed to probe staged native binding ${bindingPath}`, {
      cause: probe.error,
    });
  }
  if (probe.status !== 0) {
    const detail = String(probe.stderr ?? "").trim().slice(0, 4096);
    throw new Error(
      `Staged native binding failed required-export verification${
        detail.length === 0 ? "" : `: ${detail}`
      }`,
    );
  }
}

function triggerFailpoint(failpoint, point) {
  if (failpoint === point) {
    throw new Error(`copy-native injected test failure at ${point}`);
  }
}

function verificationError(label, verification) {
  return new Error(
    `${label} failed checksum verification (${verification?.status ?? "unknown"})`,
  );
}

function sha256File(path) {
  const hash = createHash("sha256");
  const descriptor = openSync(path, constants.O_RDONLY);
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

function assertSha256(value, label) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    throw new Error(`${label} must be a lowercase SHA-256 digest`);
  }
}

function pairIdentity(nativePath, manifestPath) {
  return Object.freeze({
    nativeSha256: sha256File(nativePath),
    manifestSha256: sha256File(manifestPath),
  });
}

function samePairIdentity(left, right) {
  return (
    left?.nativeSha256 === right?.nativeSha256 &&
    left?.manifestSha256 === right?.manifestSha256
  );
}

function verifyExactPair({
  nativePath,
  manifestPath,
  expected,
  verifyBinding,
  platformKey,
  label,
}) {
  assertRegularFile(nativePath, `${label} native binding`);
  assertChecksumManifest(manifestPath, `${label} checksum manifest`);
  const actual = pairIdentity(nativePath, manifestPath);
  if (!samePairIdentity(actual, expected)) {
    throw new Error(`${label} does not match its journaled binary/checksum identity`);
  }
  const verification = verifyBinding(nativePath, { manifestPath, platformKey });
  if (!verification?.ok) throw verificationError(label, verification);
  if (verification.sha256 !== undefined && verification.sha256 !== expected.nativeSha256) {
    throw new Error(`${label} verifier returned a checksum inconsistent with the journal`);
  }
  return verification;
}

function assertExactKeys(value, expected, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new Error(`${label} must be an object`);
  }
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.length !== wanted.length || actual.some((key, index) => key !== wanted[index])) {
    throw new Error(`${label} has unexpected or missing fields`);
  }
}

function validateJournalIdentity(identity, label) {
  assertExactKeys(identity, ["manifestSha256", "nativeSha256"], label);
  assertSha256(identity.nativeSha256, `${label}.nativeSha256`);
  assertSha256(identity.manifestSha256, `${label}.manifestSha256`);
  return Object.freeze({ ...identity });
}

function journalEntry(transaction, sequence, phase) {
  return {
    version: JOURNAL_VERSION,
    transactionId: transaction.transactionId,
    sequence,
    phase,
    platformKey: transaction.platformKey,
    previous: transaction.previous,
    next: transaction.next,
  };
}

function validateJournalEntry(value, transactionId, expectedSequence) {
  assertExactKeys(
    value,
    ["next", "phase", "platformKey", "previous", "sequence", "transactionId", "version"],
    `native publication journal ${expectedSequence}`,
  );
  if (value.version !== JOURNAL_VERSION) {
    throw new Error(`unsupported native publication journal version: ${value.version}`);
  }
  if (value.transactionId !== transactionId) {
    throw new Error("native publication journal transaction id does not match its directory");
  }
  if (value.sequence !== expectedSequence) {
    throw new Error("native publication journal sequence is non-canonical");
  }
  if (!JOURNAL_PHASE_SET.has(value.phase)) {
    throw new Error(`native publication journal has unknown phase: ${value.phase}`);
  }
  if (typeof value.platformKey !== "string" || !/^[a-z0-9_-]+-[a-z0-9_-]+$/u.test(value.platformKey)) {
    throw new Error("native publication journal has an invalid platform key");
  }
  if (value.previous !== null) validateJournalIdentity(value.previous, "journal.previous");
  validateJournalIdentity(value.next, "journal.next");
  return value;
}

function appendJournalPhase(transaction, phase, lock) {
  if (!JOURNAL_PHASE_SET.has(phase)) {
    throw new Error(`unknown native publication journal phase: ${phase}`);
  }
  const sequence = transaction.sequence + 1;
  const sequenceText = String(sequence).padStart(6, "0");
  const finalPath = join(transaction.directory, `journal-${sequenceText}.json`);
  const temporaryPath = join(
    transaction.directory,
    `.journal-${sequenceText}-${randomUUID()}.tmp`,
  );
  writeFileSync(
    temporaryPath,
    `${JSON.stringify(journalEntry(transaction, sequence, phase), null, 2)}\n`,
    { flag: "wx", mode: 0o600 },
  );
  syncFile(temporaryPath);
  assertDistLockOwnership(lock);
  renameSync(temporaryPath, finalPath);
  syncDirectory(transaction.directory);
  transaction.sequence = sequence;
  transaction.phase = phase;
}

function readTransaction(directory) {
  assertDirectory(directory, "Native publication transaction");
  const transactionId = directory.slice(directory.lastIndexOf(TRANSACTION_PREFIX) + TRANSACTION_PREFIX.length);
  if (!/^[0-9a-f-]{36}$/u.test(transactionId)) {
    throw new Error(`native publication transaction directory has an invalid id: ${directory}`);
  }
  const journalNames = readdirSync(directory)
    .filter((name) => JOURNAL_PATTERN.test(name))
    .sort();
  if (journalNames.length === 0) return null;
  let baseline;
  let expectedPhases;
  let last;
  for (let index = 0; index < journalNames.length; index += 1) {
    const expectedName = `journal-${String(index).padStart(6, "0")}.json`;
    if (journalNames[index] !== expectedName) {
      throw new Error("native publication journal contains a gap or duplicate sequence");
    }
    const journalPath = join(directory, expectedName);
    const journalMetadata = assertRegularFile(
      journalPath,
      "Native publication journal entry",
    );
    if (journalMetadata.size > MAX_JOURNAL_BYTES) {
      throw new Error(`native publication journal entry exceeds ${MAX_JOURNAL_BYTES} bytes`);
    }
    let parsed;
    try {
      parsed = JSON.parse(readFileSync(journalPath, "utf8"));
    } catch (error) {
      throw new Error(`native publication journal is not valid JSON: ${journalPath}`, {
        cause: error,
      });
    }
    const entry = validateJournalEntry(parsed, transactionId, index);
    expectedPhases ??= entry.previous === null
      ? JOURNAL_PHASES.filter(
          (phase) =>
            phase !== "previous-manifest-moved" && phase !== "previous-native-moved",
        )
      : JOURNAL_PHASES;
    if (entry.phase !== expectedPhases[index]) {
      throw new Error("native publication journal phases are non-canonical");
    }
    const invariant = JSON.stringify({
      next: entry.next,
      platformKey: entry.platformKey,
      previous: entry.previous,
      transactionId: entry.transactionId,
      version: entry.version,
    });
    if (baseline === undefined) baseline = invariant;
    else if (baseline !== invariant) {
      throw new Error("native publication journal invariants changed between phases");
    }
    last = entry;
  }
  return {
    directory,
    next: Object.freeze({ ...last.next }),
    phase: last.phase,
    platformKey: last.platformKey,
    previous: last.previous === null ? null : Object.freeze({ ...last.previous }),
    sequence: last.sequence,
    transactionId,
  };
}

function classifyFile(path, previousDigest, nextDigest, label) {
  if (!existsSync(path)) return "absent";
  assertRegularFile(path, label);
  const digest = sha256File(path);
  if (previousDigest !== undefined && digest === previousDigest) return "previous";
  if (digest === nextDigest) return "next";
  throw new Error(`${label} is tampered or does not match the journal`);
}

function requireFileKind(actual, expected, label) {
  if (actual !== "absent" && actual !== expected) {
    throw new Error(`${label} has the wrong journaled generation`);
  }
}

function syncRename(source, destination, lock) {
  assertDistLockOwnership(lock);
  renameSync(source, destination);
  syncDirectory(dirname(source));
  if (dirname(destination) !== dirname(source)) syncDirectory(dirname(destination));
}

function cleanupTransactionDirectory(transactionDirectory, destinationDirectory, lock) {
  if (!existsSync(transactionDirectory)) return;
  assertDistLockOwnership(lock);
  const cleanup = join(
    destinationDirectory,
    `${CLEANUP_PREFIX}${process.pid}-${randomUUID()}`,
  );
  renameSync(transactionDirectory, cleanup);
  syncDirectory(destinationDirectory);
  assertDistLockOwnership(lock);
  rmSync(cleanup, { recursive: true, force: true });
  syncDirectory(destinationDirectory);
}

function assertTransactionEntries(transaction) {
  const allowed = new Set([
    NEXT_NATIVE_FILENAME,
    NEXT_MANIFEST_FILENAME,
    PREVIOUS_NATIVE_FILENAME,
    PREVIOUS_MANIFEST_FILENAME,
    DISCARDED_NATIVE_FILENAME,
    DISCARDED_MANIFEST_FILENAME,
  ]);
  for (const name of readdirSync(transaction.directory)) {
    if (JOURNAL_PATTERN.test(name) || JOURNAL_TEMP_PATTERN.test(name) || allowed.has(name)) {
      const path = join(transaction.directory, name);
      if (
        name === NEXT_MANIFEST_FILENAME ||
        name === PREVIOUS_MANIFEST_FILENAME ||
        name === DISCARDED_MANIFEST_FILENAME
      ) {
        assertChecksumManifest(path, "Native publication transaction checksum manifest");
      } else {
        assertRegularFile(path, "Native publication transaction artifact");
      }
      continue;
    }
    throw new Error(`native publication transaction contains an unexpected artifact: ${name}`);
  }
}

function verifyTransactionArtifacts(transaction) {
  assertTransactionEntries(transaction);
  const paths = {
    discardedManifest: join(transaction.directory, DISCARDED_MANIFEST_FILENAME),
    discardedNative: join(transaction.directory, DISCARDED_NATIVE_FILENAME),
    nextManifest: join(transaction.directory, NEXT_MANIFEST_FILENAME),
    nextNative: join(transaction.directory, NEXT_NATIVE_FILENAME),
    previousManifest: join(transaction.directory, PREVIOUS_MANIFEST_FILENAME),
    previousNative: join(transaction.directory, PREVIOUS_NATIVE_FILENAME),
  };
  requireFileKind(
    classifyFile(
      paths.nextNative,
      undefined,
      transaction.next.nativeSha256,
      "Staged next native binding",
    ),
    "next",
    "Staged next native binding",
  );
  requireFileKind(
    classifyFile(
      paths.nextManifest,
      undefined,
      transaction.next.manifestSha256,
      "Staged next checksum manifest",
    ),
    "next",
    "Staged next checksum manifest",
  );
  requireFileKind(
    classifyFile(
      paths.discardedNative,
      undefined,
      transaction.next.nativeSha256,
      "Discarded next native binding",
    ),
    "next",
    "Discarded next native binding",
  );
  requireFileKind(
    classifyFile(
      paths.discardedManifest,
      undefined,
      transaction.next.manifestSha256,
      "Discarded next checksum manifest",
    ),
    "next",
    "Discarded next checksum manifest",
  );
  if (transaction.previous === null) {
    if (existsSync(paths.previousNative) || existsSync(paths.previousManifest)) {
      throw new Error("first native publication unexpectedly contains previous-pair backups");
    }
  } else {
    requireFileKind(
      classifyFile(
        paths.previousNative,
        transaction.previous.nativeSha256,
        transaction.next.nativeSha256,
        "Previous native binding backup",
      ),
      "previous",
      "Previous native binding backup",
    );
    requireFileKind(
      classifyFile(
        paths.previousManifest,
        transaction.previous.manifestSha256,
        transaction.next.manifestSha256,
        "Previous checksum manifest backup",
      ),
      "previous",
      "Previous checksum manifest backup",
    );
  }
  return paths;
}

function recoverTransaction({
  transaction,
  destinationDirectory,
  verifyBinding,
  lock,
  prefer = "auto",
}) {
  const dest = join(destinationDirectory, NATIVE_FILENAME);
  const checksumManifestPath = join(destinationDirectory, CHECKSUM_FILENAME);
  const paths = verifyTransactionArtifacts(transaction);
  const nativeKind = classifyFile(
    dest,
    transaction.previous?.nativeSha256,
    transaction.next.nativeSha256,
    "Published native binding",
  );
  const manifestKind = classifyFile(
    checksumManifestPath,
    transaction.previous?.manifestSha256,
    transaction.next.manifestSha256,
    "Published checksum manifest",
  );
  const nextNativeCopies =
    Number(nativeKind === "next") +
    Number(existsSync(paths.nextNative)) +
    Number(existsSync(paths.discardedNative));
  const nextManifestCopies =
    Number(manifestKind === "next") +
    Number(existsSync(paths.nextManifest)) +
    Number(existsSync(paths.discardedManifest));
  if (nextNativeCopies !== 1 || nextManifestCopies !== 1) {
    throw new Error("native publication transaction has missing or duplicate next-pair components");
  }
  if (transaction.previous !== null) {
    const previousNativeCopies =
      Number(nativeKind === "previous") + Number(existsSync(paths.previousNative));
    const previousManifestCopies =
      Number(manifestKind === "previous") + Number(existsSync(paths.previousManifest));
    if (previousNativeCopies !== 1 || previousManifestCopies !== 1) {
      throw new Error(
        "native publication transaction has missing or duplicate previous-pair components",
      );
    }
  }
  const completeNext = nativeKind === "next" && manifestKind === "next";
  const completePrevious = nativeKind === "previous" && manifestKind === "previous";
  const commitNext = prefer === "next" || (prefer === "auto" && completeNext);

  if (commitNext) {
    if (!completeNext) {
      throw new Error("native publication cannot commit an incomplete next pair");
    }
    if (transaction.previous !== null) {
      if (!existsSync(paths.previousNative) || !existsSync(paths.previousManifest)) {
        throw new Error("native publication cannot verify the exact previous pair before commit");
      }
    }
    const verification = verifyExactPair({
      nativePath: dest,
      manifestPath: checksumManifestPath,
      expected: transaction.next,
      verifyBinding,
      platformKey: transaction.platformKey,
      label: "Recovered next native binding",
    });
    cleanupTransactionDirectory(transaction.directory, destinationDirectory, lock);
    return { outcome: "next", verification };
  }

  if (completePrevious) {
    const verification = verifyExactPair({
      nativePath: dest,
      manifestPath: checksumManifestPath,
      expected: transaction.previous,
      verifyBinding,
      platformKey: transaction.platformKey,
      label: "Recovered previous native binding",
    });
    cleanupTransactionDirectory(transaction.directory, destinationDirectory, lock);
    return { outcome: "previous", verification };
  }

  // Remove only journal-authenticated next components from the public names.
  // Renames target absent transaction paths and are repeatable on Windows.
  if (manifestKind === "next") {
    if (existsSync(paths.discardedManifest)) {
      throw new Error("native publication recovery found duplicate next manifests");
    }
    syncRename(checksumManifestPath, paths.discardedManifest, lock);
  }
  if (nativeKind === "next") {
    if (existsSync(paths.discardedNative)) {
      throw new Error("native publication recovery found duplicate next binaries");
    }
    syncRename(dest, paths.discardedNative, lock);
  }

  if (transaction.previous === null) {
    if (existsSync(dest) || existsSync(checksumManifestPath)) {
      throw new Error("first native publication recovery could not restore the exact absent state");
    }
    cleanupTransactionDirectory(transaction.directory, destinationDirectory, lock);
    return { outcome: "absent" };
  }

  const currentNativeKind = classifyFile(
    dest,
    transaction.previous.nativeSha256,
    transaction.next.nativeSha256,
    "Native binding during rollback",
  );
  const currentManifestKind = classifyFile(
    checksumManifestPath,
    transaction.previous.manifestSha256,
    transaction.next.manifestSha256,
    "Checksum manifest during rollback",
  );
  if (currentNativeKind === "absent") {
    if (!existsSync(paths.previousNative)) {
      throw new Error("native publication recovery is missing the exact previous binary");
    }
    syncRename(paths.previousNative, dest, lock);
  } else if (currentNativeKind !== "previous") {
    throw new Error("native publication recovery could not isolate the next binary");
  }
  if (currentManifestKind === "absent") {
    if (!existsSync(paths.previousManifest)) {
      throw new Error("native publication recovery is missing the exact previous manifest");
    }
    syncRename(paths.previousManifest, checksumManifestPath, lock);
  } else if (currentManifestKind !== "previous") {
    throw new Error("native publication recovery could not isolate the next manifest");
  }
  const verification = verifyExactPair({
    nativePath: dest,
    manifestPath: checksumManifestPath,
    expected: transaction.previous,
    verifyBinding,
    platformKey: transaction.platformKey,
    label: "Rolled-back previous native binding",
  });
  cleanupTransactionDirectory(transaction.directory, destinationDirectory, lock);
  return { outcome: "previous", verification };
}

function transactionDirectories(destinationDirectory) {
  return readdirSync(destinationDirectory)
    .filter((name) => name.startsWith(TRANSACTION_PREFIX))
    .map((name) => join(destinationDirectory, name));
}

function cleanupDirectories(destinationDirectory) {
  return readdirSync(destinationDirectory)
    .filter((name) => name.startsWith(CLEANUP_PREFIX))
    .map((name) => join(destinationDirectory, name));
}

function recoverInterruptedPublications({
  destinationDirectory,
  platformKey,
  verifyBinding,
  lock,
}) {
  for (const cleanup of cleanupDirectories(destinationDirectory)) {
    assertDirectory(cleanup, "Native publication cleanup artifact");
    assertDistLockOwnership(lock);
    rmSync(cleanup, { recursive: true, force: true });
    syncDirectory(destinationDirectory);
  }

  const journaled = [];
  for (const directory of transactionDirectories(destinationDirectory)) {
    assertDirectory(directory, "Native publication transaction artifact");
    const transaction = readTransaction(directory);
    if (transaction === null) {
      const allowed = new Set([NEXT_NATIVE_FILENAME, NEXT_MANIFEST_FILENAME]);
      for (const name of readdirSync(directory)) {
        if (!allowed.has(name) && !JOURNAL_TEMP_PATTERN.test(name)) {
          throw new Error(
            `native publication found an ambiguous unjournaled artifact: ${name}`,
          );
        }
        assertRegularFile(
          join(directory, name),
          "Unjournaled native publication staging artifact",
        );
      }
      cleanupTransactionDirectory(directory, destinationDirectory, lock);
      continue;
    }
    if (transaction.platformKey !== platformKey) {
      throw new Error(
        `native publication journal platform ${transaction.platformKey} does not match ${platformKey}`,
      );
    }
    journaled.push(transaction);
  }
  if (journaled.length > 1) {
    throw new Error("native publication found multiple journaled transactions; refusing ambiguity");
  }
  if (journaled.length === 1) {
    return recoverTransaction({
      transaction: journaled[0],
      destinationDirectory,
      verifyBinding,
      lock,
      prefer: "auto",
    });
  }
  const dest = join(destinationDirectory, NATIVE_FILENAME);
  const manifest = join(destinationDirectory, CHECKSUM_FILENAME);
  const nativeExists = existsSync(dest);
  const manifestExists = existsSync(manifest);
  if (nativeExists !== manifestExists) {
    throw new Error(
      "native destination contains a partial binary/checksum pair without a recovery journal",
    );
  }
  if (!nativeExists) return { outcome: "none" };
  assertRegularFile(dest, "Existing native binding after recovery");
  assertChecksumManifest(manifest, "Existing native checksum manifest after recovery");
  const verification = verifyBinding(dest, { manifestPath: manifest, platformKey });
  if (!verification?.ok) {
    throw verificationError("Existing native binding after recovery", verification);
  }
  return { outcome: "existing", verification };
}

function createTransaction(destinationDirectory, platformKey, previous, next, lock) {
  const transactionId = randomUUID();
  const directory = join(destinationDirectory, `${TRANSACTION_PREFIX}${transactionId}`);
  assertDistLockOwnership(lock);
  mkdirSync(directory, { mode: 0o700 });
  syncDirectory(destinationDirectory);
  return {
    directory,
    next,
    phase: undefined,
    platformKey,
    previous,
    sequence: -1,
    transactionId,
  };
}

function publishStagedPair({
  transaction,
  destinationDirectory,
  verifyBinding,
  platformKey,
  failpoint,
  phaseHook,
  lock,
}) {
  const stagedNative = join(transaction.directory, NEXT_NATIVE_FILENAME);
  const stagedManifest = join(transaction.directory, NEXT_MANIFEST_FILENAME);
  const backupNative = join(transaction.directory, PREVIOUS_NATIVE_FILENAME);
  const backupManifest = join(transaction.directory, PREVIOUS_MANIFEST_FILENAME);
  const dest = join(destinationDirectory, NATIVE_FILENAME);
  const checksumManifestPath = join(destinationDirectory, CHECKSUM_FILENAME);

  appendJournalPhase(transaction, "prepared", lock);
  phaseHook?.("journal-prepared");
  if (transaction.previous !== null) {
    syncRename(checksumManifestPath, backupManifest, lock);
    phaseHook?.("previous-manifest-renamed");
    appendJournalPhase(transaction, "previous-manifest-moved", lock);
    phaseHook?.("previous-manifest-moved");
    syncRename(dest, backupNative, lock);
    phaseHook?.("previous-native-renamed");
    appendJournalPhase(transaction, "previous-native-moved", lock);
    phaseHook?.("previous-native-moved");
  }
  triggerFailpoint(failpoint, "after-backup");

  syncRename(stagedNative, dest, lock);
  phaseHook?.("next-native-renamed");
  appendJournalPhase(transaction, "next-native-moved", lock);
  phaseHook?.("next-native-moved");
  triggerFailpoint(failpoint, "after-native");

  // Publish the authenticating manifest last. Until this rename completes,
  // the loader rejects the new binary as missing its checksum contract.
  syncRename(stagedManifest, checksumManifestPath, lock);
  phaseHook?.("next-manifest-renamed");
  appendJournalPhase(transaction, "next-manifest-moved", lock);
  phaseHook?.("next-manifest-moved");
  triggerFailpoint(failpoint, "after-manifest");

  const published = verifyExactPair({
    nativePath: dest,
    manifestPath: checksumManifestPath,
    expected: transaction.next,
    verifyBinding,
    platformKey,
    label: "Published native binding",
  });
  phaseHook?.("published-pair-verified");
  appendJournalPhase(transaction, "published-verified", lock);
  phaseHook?.("published-verified");
  appendJournalPhase(transaction, "committed", lock);
  phaseHook?.("journal-committed");
  return published;
}

/**
 * Stage, authenticate, probe, and transactionally publish one native binding.
 * The source and destination must be on the same machine; staging occurs in
 * the destination directory so all publication renames stay on one volume.
 */
export async function publishNativeBinding({
  source,
  destDir,
  platform = process.platform,
  arch = process.arch,
  signNative = defaultSignNative,
  verifyBinding = verifyNativeBinding,
  probeBinding = probeNativeBindingExports,
  requiredExports = REQUIRED_NATIVE_EXPORTS,
  failpoint,
  phaseHook,
  log = console.log,
} = {}) {
  if (typeof source !== "string" || source.length === 0) {
    throw new TypeError("native source must be a non-empty path");
  }
  if (typeof destDir !== "string" || destDir.length === 0) {
    throw new TypeError("native destination must be a non-empty directory path");
  }
  if (failpoint !== undefined && !TEST_FAILPOINTS.has(failpoint)) {
    throw new TypeError(`unknown copy-native failpoint: ${failpoint}`);
  }
  if (
    typeof signNative !== "function" ||
    typeof verifyBinding !== "function" ||
    typeof probeBinding !== "function" ||
    (phaseHook !== undefined && typeof phaseHook !== "function") ||
    typeof log !== "function"
  ) {
    throw new TypeError("native publication hooks must be functions");
  }

  const sourcePath = resolve(source);
  const destinationDirectory = resolve(destDir);
  const dest = join(destinationDirectory, NATIVE_FILENAME);
  const checksumManifestPath = join(destinationDirectory, CHECKSUM_FILENAME);
  if (sourcePath === dest) {
    throw new TypeError("native source and published destination must differ");
  }
  assertRegularFile(sourcePath, "Compiled native module");
  mkdirSync(destinationDirectory, { recursive: true });
  assertDirectory(destinationDirectory, "Native destination");

  const platformKey = `${platform}-${arch}`.toLowerCase();
  let lock;
  let transaction;
  let primaryError;
  try {
    lock = await acquireDistLock({ root: destinationDirectory });
    recoverInterruptedPublications({
      destinationDirectory,
      platformKey,
      verifyBinding,
      lock,
    });
    phaseHook?.("recovery-complete");
    const nativeExists = existsSync(dest);
    const manifestExists = existsSync(checksumManifestPath);
    if (nativeExists !== manifestExists) {
      throw new Error(
        "native destination contains a partial binary/checksum pair; refusing to replace ambiguous state",
      );
    }
    if (nativeExists) {
      assertRegularFile(dest, "Existing native binding");
      assertChecksumManifest(checksumManifestPath, "Existing native checksum manifest");
    }

    let previous = null;
    if (nativeExists) {
      const verification = verifyBinding(dest, {
        manifestPath: checksumManifestPath,
        platformKey,
      });
      if (!verification?.ok) {
        throw verificationError("Existing native binding", verification);
      }
      previous = pairIdentity(dest, checksumManifestPath);
      if (verification.sha256 !== undefined && verification.sha256 !== previous.nativeSha256) {
        throw new Error("Existing native verifier returned an inconsistent checksum");
      }
    }

    // Create the transaction before staging, but do not write a journal until
    // the complete next pair has been synced, authenticated, and probed. A
    // crash before that point therefore leaves a provably non-destructive
    // directory that startup recovery can discard.
    transaction = createTransaction(
      destinationDirectory,
      platformKey,
      previous,
      Object.freeze({ nativeSha256: "0".repeat(64), manifestSha256: "0".repeat(64) }),
      lock,
    );
    if (platform !== "win32") chmodSync(transaction.directory, 0o700);
    const stagedNative = join(transaction.directory, NEXT_NATIVE_FILENAME);
    const stagedManifest = join(transaction.directory, NEXT_MANIFEST_FILENAME);

    copyFileSync(sourcePath, stagedNative, constants.COPYFILE_EXCL);
    signNative(stagedNative, { platform, cwd: repoRoot });
    if (platform !== "win32") chmodSync(stagedNative, 0o500);
    syncFile(stagedNative);
    syncDirectory(transaction.directory);
    phaseHook?.("staged-native-synced");

    const sha256 = sha256File(stagedNative);
    writeFileSync(
      stagedManifest,
      `${JSON.stringify(
        {
          entries: {
            [platformKey]: { sha256 },
          },
        },
        null,
        2,
      )}\n`,
      { flag: "wx", mode: 0o600 },
    );
    syncFile(stagedManifest);
    syncDirectory(transaction.directory);
    phaseHook?.("staged-manifest-synced");

    const staged = verifyBinding(stagedNative, {
      manifestPath: stagedManifest,
      platformKey,
    });
    if (!staged?.ok) throw verificationError("Staged native binding", staged);
    probeBinding(stagedNative, requiredExports);

    transaction.next = pairIdentity(stagedNative, stagedManifest);
    if (staged.sha256 !== undefined && staged.sha256 !== transaction.next.nativeSha256) {
      throw new Error("Staged native verifier returned an inconsistent checksum");
    }
    // A checksum manifest is an authentication sidecar, not part of the
    // executable generation. Existing valid manifests may differ in harmless
    // formatting or contain additional platform entries. Treat an identical
    // probed binary as idempotent so recovery never has to distinguish two
    // byte-identical native generations by their filesystem position alone.
    if (
      transaction.previous !== null &&
      transaction.previous.nativeSha256 === transaction.next.nativeSha256
    ) {
      const matchingSha256 = transaction.next.nativeSha256;
      cleanupTransactionDirectory(transaction.directory, destinationDirectory, lock);
      transaction = undefined;
      log(`Native module at ${dest} already matches the verified build output`);
      return {
        bindingPath: dest,
        manifestPath: checksumManifestPath,
        platformKey,
        sha256: matchingSha256,
      };
    }

    if (transaction.previous !== null) {
      verifyExactPair({
        nativePath: dest,
        manifestPath: checksumManifestPath,
        expected: transaction.previous,
        verifyBinding,
        platformKey,
        label: "Pre-publication existing native binding",
      });
    }

    const published = publishStagedPair({
      transaction,
      destinationDirectory,
      verifyBinding,
      platformKey,
      failpoint,
      phaseHook,
      lock,
    });
    cleanupTransactionDirectory(transaction.directory, destinationDirectory, lock);
    transaction = undefined;
    log(`Published verified native module to ${dest}`);
    log(`Wrote checksum manifest to ${checksumManifestPath}`);
    return {
      bindingPath: dest,
      manifestPath: checksumManifestPath,
      platformKey,
      sha256: published.sha256,
    };
  } catch (error) {
    primaryError = error;
    if (lock && transaction && existsSync(transaction.directory)) {
      try {
        const journaled = readTransaction(transaction.directory);
        if (journaled !== null) {
          recoverTransaction({
            transaction: journaled,
            destinationDirectory,
            verifyBinding,
            lock,
            prefer: "previous",
          });
        } else {
          cleanupTransactionDirectory(transaction.directory, destinationDirectory, lock);
        }
        transaction = undefined;
      } catch (recoveryError) {
        const aggregate = new AggregateError(
          [error, recoveryError],
          "native publication failed and durable recovery could not restore the exact prior pair",
        );
        primaryError = aggregate;
        throw aggregate;
      }
    }
    throw error;
  } finally {
    const cleanupErrors = [];
    if (transaction && existsSync(transaction.directory)) {
      try {
        if (lock) assertDistLockOwnership(lock);
        const journaled = readTransaction(transaction.directory);
        if (journaled === null) {
          cleanupTransactionDirectory(transaction.directory, destinationDirectory, lock);
        }
      } catch (error) {
        cleanupErrors.push(error);
      }
    }
    if (lock) {
      try {
        releaseDistLock(lock);
      } catch (error) {
        cleanupErrors.push(error);
      }
    }
    if (cleanupErrors.length > 0) {
      if (primaryError) {
        throw new AggregateError(
          [primaryError, ...cleanupErrors],
          `native publication failed (${primaryError.message}) and cleanup was incomplete`,
        );
      }
      if (cleanupErrors.length === 1) throw cleanupErrors[0];
      throw new AggregateError(cleanupErrors, "native publication cleanup was incomplete");
    }
  }
}

/** Recover a killed native publication without beginning a new one. */
export async function recoverNativeBindingPublication({
  destDir,
  platform = process.platform,
  arch = process.arch,
  verifyBinding = verifyNativeBinding,
} = {}) {
  if (typeof destDir !== "string" || destDir.length === 0) {
    throw new TypeError("native destination must be a non-empty directory path");
  }
  if (typeof verifyBinding !== "function") {
    throw new TypeError("native recovery verifyBinding must be a function");
  }
  const destinationDirectory = resolve(destDir);
  assertDirectory(destinationDirectory, "Native destination");
  const platformKey = `${platform}-${arch}`.toLowerCase();
  const lock = await acquireDistLock({ root: destinationDirectory });
  let primaryError;
  try {
    return recoverInterruptedPublications({
      destinationDirectory,
      platformKey,
      verifyBinding,
      lock,
    });
  } catch (error) {
    primaryError = error;
    throw error;
  } finally {
    try {
      releaseDistLock(lock);
    } catch (releaseError) {
      if (primaryError) {
        throw new AggregateError(
          [primaryError, releaseError],
          "native recovery failed and its publication lock could not be released",
        );
      }
      throw releaseError;
    }
  }
}

function defaultNativePaths() {
  const configuredTarget = process.env.CARGO_TARGET_DIR;
  const targetRoot = configuredTarget
    ? isAbsolute(configuredTarget)
      ? configuredTarget
      : join(repoRoot, configuredTarget)
    : join(repoRoot, "target");
  const libName = process.platform === "win32"
    ? "iroha_js_host.dll"
    : `libiroha_js_host.${process.platform === "darwin" ? "dylib" : "so"}`;
  const configuredDestDir = process.env.IROHA_JS_NATIVE_DIR;
  const destDir = configuredDestDir
    ? isAbsolute(configuredDestDir)
      ? configuredDestDir
      : join(repoRoot, configuredDestDir)
    : join(repoRoot, "javascript", "iroha_js", "native");
  return {
    source: join(targetRoot, "debug", libName),
    destDir,
  };
}

if (process.argv[1] && resolve(process.argv[1]) === __filename) {
  publishNativeBinding(defaultNativePaths()).catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
