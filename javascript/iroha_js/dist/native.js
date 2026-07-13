import { createHash } from "node:crypto";
import { Buffer } from "node:buffer";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import {
  chmodSync,
  closeSync,
  constants,
  existsSync,
  fchmodSync,
  fsyncSync,
  lstatSync,
  mkdtempSync,
  openSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";

import { machOSigningIndependentSHA256 } from "./nativeArtifactHash.js";

const NATIVE_FILENAME = "iroha_js_host.node";
const CHECKSUM_FILENAME = "iroha_js_host.checksums.json";
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const PLATFORM_KEY_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)+$/u;

let cachedBinding;
let cachedBindingPath;
let cachedSnapshotDir;

function nativeBindingError(reason, status = "unknown") {
  const error = new Error(`Native binding required; ${reason}`);
  Object.defineProperties(error, {
    code: {
      value: "ERR_IROHA_NATIVE_BINDING",
      enumerable: true,
    },
    nativeStatus: {
      value: status,
      enumerable: true,
    },
  });
  return error;
}

function formatForceNativeVerificationError(verification, paths) {
  switch (verification.status) {
    case "missing_file":
      return nativeBindingError(
        `binding missing at ${paths.bindingPath}; run \`npm run build:native\`.`,
        verification.status,
      );
    case "manifest_error":
      return nativeBindingError(
        `checksum manifest at ${paths.checksumPath} is invalid or unreadable: ${
          verification.error?.message ?? verification.error
        }.`,
        verification.status,
      );
    case "missing_manifest":
    case "missing_expected_entry":
      return nativeBindingError(
        `checksum manifest missing entries for ${verification.platform}; run \`npm run build:native\`.`,
        verification.status,
      );
    case "hash_mismatch":
      return nativeBindingError(
        `checksum mismatch for ${paths.bindingPath}; expected ${verification.expectedSha256}, found ${verification.sha256}.`,
        verification.status,
      );
    case "hash_error":
    default:
      return nativeBindingError(
        `verification failed (${verification.status}).`,
        verification.status,
      );
  }
}

/**
 * Load the required native `iroha_js_host` binding.
 */
export function getNativeBinding() {
  const paths = resolveNativePaths();
  if (cachedBindingPath !== paths.bindingPath) {
    cachedBinding = undefined;
    cleanupSnapshotDirectory(cachedSnapshotDir);
    cachedSnapshotDir = undefined;
  }
  if (cachedBinding !== undefined) {
    return cachedBinding;
  }

  const verification = verifyNativeBindingInternal(
    paths.bindingPath,
    { manifestPath: paths.checksumPath },
    true,
  );
  if (!verification.ok) {
    throw formatForceNativeVerificationError(verification, paths);
  }

  let snapshot;
  try {
    snapshot = materializeVerifiedSnapshot(verification);
    const require = createRequire(import.meta.url);
    cachedBinding = require(snapshot.path);
  } catch (error) {
    cleanupSnapshotDirectory(snapshot?.directory);
    throw nativeBindingError(
      `failed to load verified binding from ${paths.bindingPath}: ${error?.message ?? error}.`,
      "load_error",
    );
  }
  cachedBindingPath = paths.bindingPath;
  cachedSnapshotDir = snapshot.directory;
  return cachedBinding;
}

/** Verify a native binding against the checksum manifest. */
export function verifyNativeBinding(
  bindingPath,
  options = {},
) {
  return verifyNativeBindingInternal(bindingPath, options, false);
}

function verifyNativeBindingInternal(
  bindingPath,
  { manifestPath, expectedChecksums, platformKey } = {},
  retainBytes,
) {
  if (!existsSync(bindingPath)) {
    return {
      ok: false,
      status: "missing_file",
      path: bindingPath,
      manifestPath,
    };
  }

  const hash = hashFile(bindingPath, retainBytes);
  if (!hash.ok) {
    return {
      ok: false,
      status: "hash_error",
      path: bindingPath,
      manifestPath,
      error: hash.error,
    };
  }

  const platform = platformKey ?? `${process.platform}-${process.arch}`;
  const checksumEntries =
    expectedChecksums === undefined
      ? loadChecksumEntries(manifestPath ?? resolveNativePaths().checksumPath)
      : normalizeExpectedChecksums(expectedChecksums);

  if (checksumEntries.error) {
    return {
      ok: false,
      status: "manifest_error",
      path: bindingPath,
      manifestPath,
      platform,
      sha256: hash.sha256,
      error: checksumEntries.error,
    };
  }

  if (!checksumEntries.entries) {
    return {
      ok: false,
      status: "missing_manifest",
      path: bindingPath,
      manifestPath,
      platform,
      sha256: hash.sha256,
    };
  }

  if (
    typeof platform !== "string" ||
    !PLATFORM_KEY_PATTERN.test(platform) ||
    platform !== platform.toLowerCase()
  ) {
    return {
      ok: false,
      status: "manifest_error",
      path: bindingPath,
      manifestPath,
      platform,
      sha256: hash.sha256,
      error: new TypeError(
        "native platform key must be a canonical lowercase platform-architecture pair",
      ),
    };
  }
  const entries = checksumEntries.entries;
  if (!Object.hasOwn(entries, platform)) {
    return {
      ok: false,
      status: "missing_expected_entry",
      path: bindingPath,
      manifestPath,
      platform,
      sha256: hash.sha256,
    };
  }
  const expectedEntry = entries[platform];
  const expectedSha256 = expectedEntry?.sha256;
  const expectedMachOSigningIndependentSha256 =
    expectedEntry?.mach_o_signing_independent_sha256;
  const entryKeys = isPlainObject(expectedEntry)
    ? Object.keys(expectedEntry).sort()
    : [];
  const exactChecksumKeys = entryKeys.length === 1 && entryKeys[0] === "sha256";
  const resignableMachOKeys =
    platform.startsWith("darwin-") &&
    entryKeys.length === 2 &&
    entryKeys[0] === "mach_o_signing_independent_sha256" &&
    entryKeys[1] === "sha256";
  if (
    !isPlainObject(expectedEntry) ||
    (!exactChecksumKeys && !resignableMachOKeys) ||
    typeof expectedSha256 !== "string" ||
    !SHA256_PATTERN.test(expectedSha256) ||
    (resignableMachOKeys &&
      (typeof expectedMachOSigningIndependentSha256 !== "string" ||
        !SHA256_PATTERN.test(expectedMachOSigningIndependentSha256)))
  ) {
    return {
      ok: false,
      status: "manifest_error",
      path: bindingPath,
      manifestPath,
      platform,
      sha256: hash.sha256,
      error: new TypeError(
        `checksum entry for ${platform} has an invalid platform checksum profile`,
      ),
    };
  }

  let verificationStatus = "verified";
  let machOSigningIndependentSha256;
  if (expectedSha256 !== hash.sha256 && platform.startsWith("darwin-")) {
    try {
      machOSigningIndependentSha256 = machOSigningIndependentSHA256(hash.fileBytes);
    } catch (error) {
      return {
        ok: false,
        status: "hash_error",
        path: bindingPath,
        manifestPath,
        platform,
        sha256: hash.sha256,
        expectedSha256,
        error,
      };
    }
    if (machOSigningIndependentSha256 === expectedMachOSigningIndependentSha256) {
      verificationStatus = "verified_resigned_macho";
    }
  }

  if (expectedSha256 !== hash.sha256 && verificationStatus !== "verified_resigned_macho") {
    return {
      ok: false,
      status: "hash_mismatch",
      path: bindingPath,
      manifestPath,
      platform,
      sha256: hash.sha256,
      expectedSha256,
    };
  }

  const result = {
    ok: true,
    status: verificationStatus,
    path: bindingPath,
    manifestPath,
    platform,
    sha256: hash.sha256,
    expectedSha256,
    ...(expectedMachOSigningIndependentSha256 === undefined
      ? {}
      : { expectedMachOSigningIndependentSha256 }),
    ...(machOSigningIndependentSha256 === undefined
      ? {}
      : {
          machOSigningIndependentSha256,
          expectedMachOSigningIndependentSha256,
        }),
  };
  if (retainBytes) {
    result.verifiedBytes = hash.bytes;
  }
  return result;
}

/**
 * Materialize a verified snapshot while a test replaces the original path.
 * @internal
 */
export function __snapshotNativeBindingForTests(
  bindingPath,
  options = {},
  afterVerification,
) {
  const verification = verifyNativeBindingInternal(bindingPath, options, true);
  if (!verification.ok) {
    return verification;
  }
  if (afterVerification !== undefined) {
    if (typeof afterVerification !== "function") {
      throw new TypeError("afterVerification must be a function");
    }
    afterVerification();
  }
  const snapshot = materializeVerifiedSnapshot(verification);
  return {
    ok: true,
    status: "snapshotted",
    sha256: verification.sha256,
    path: snapshot.path,
    directory: snapshot.directory,
  };
}

/**
 * Reset cached native state (test helper).
 * @internal
 */
export function __resetNativeStateForTests() {
  cachedBinding = undefined;
  cachedBindingPath = undefined;
  cleanupSnapshotDirectory(cachedSnapshotDir);
  cachedSnapshotDir = undefined;
}

function materializeVerifiedSnapshot(verification) {
  if (!Buffer.isBuffer(verification.verifiedBytes)) {
    throw new TypeError("verified native bytes are unavailable");
  }
  const directory = mkdtempSync(join(tmpdir(), "iroha-js-host-"));
  let descriptor;
  try {
    if (process.platform !== "win32") {
      chmodSync(directory, 0o700);
    }
    const path = join(directory, `${verification.sha256}.node`);
    const noFollow = constants.O_NOFOLLOW ?? 0;
    descriptor = openSync(
      path,
      constants.O_CREAT | constants.O_EXCL | constants.O_WRONLY | noFollow,
      0o500,
    );
    writeFileSync(descriptor, verification.verifiedBytes);
    fsyncSync(descriptor);
    if (process.platform !== "win32") {
      fchmodSync(descriptor, 0o500);
    }
    closeSync(descriptor);
    descriptor = undefined;

    const metadata = lstatSync(path);
    if (
      !metadata.isFile() ||
      metadata.size !== verification.verifiedBytes.byteLength
    ) {
      throw new Error("verified native snapshot is not the expected regular file");
    }
    const finalHash = hashFile(path, false);
    if (!finalHash.ok || finalHash.sha256 !== verification.sha256) {
      throw new Error("verified native snapshot changed before module loading");
    }
    return { path, directory };
  } catch (error) {
    if (descriptor !== undefined) {
      try {
        closeSync(descriptor);
      } catch {
        // The primary snapshot error remains authoritative.
      }
    }
    cleanupSnapshotDirectory(directory);
    throw error;
  }
}

function cleanupSnapshotDirectory(directory) {
  if (!directory) {
    return;
  }
  try {
    rmSync(directory, { recursive: true, force: true });
  } catch {
    // A loaded Windows addon can remain locked until process exit. The random,
    // owner-only directory is safe to leave for operating-system cleanup.
  }
}

function resolveNativePaths() {
  const baseDir = dirname(fileURLToPath(import.meta.url));
  const jsRoot = resolve(baseDir, "..");
  const nativeDirOverride = process.env.IROHA_JS_NATIVE_DIR;
  const nativeDir = nativeDirOverride ? resolve(nativeDirOverride) : join(jsRoot, "native");
  return {
    bindingPath: join(nativeDir, NATIVE_FILENAME),
    checksumPath: join(nativeDir, CHECKSUM_FILENAME),
    jsRoot,
    nativeDir,
    hasOverride: Boolean(nativeDirOverride),
  };
}

function loadChecksumEntries(manifestPath) {
  if (!manifestPath || !existsSync(manifestPath)) {
    return { entries: null, error: null };
  }

  try {
    const raw = readFileSync(manifestPath, "utf8");
    const parsed = JSON.parse(raw);
    if (
      !isPlainObject(parsed) ||
      Object.keys(parsed).length !== 1 ||
      !isPlainObject(parsed.entries)
    ) {
      throw new TypeError(
        "checksum manifest must be an object containing only an entries object",
      );
    }
    return validateChecksumEntries(parsed.entries);
  } catch (error) {
    return { entries: null, error };
  }
}

function normalizeExpectedChecksums(expectedChecksums) {
  return validateChecksumEntries(expectedChecksums);
}

function validateChecksumEntries(entries) {
  if (!isPlainObject(entries) || Object.keys(entries).length === 0) {
    return {
      entries: null,
      error: new TypeError(
        "checksum entries must be a non-empty platform-entry object",
      ),
    };
  }
  const normalizedKeys = new Set();
  for (const [platform, entry] of Object.entries(entries)) {
    const normalized = platform.toLowerCase();
    if (normalizedKeys.has(normalized)) {
      return {
        entries: null,
        error: new TypeError(
          `checksum entries contain a case-colliding platform key: ${platform}`,
        ),
      };
    }
    normalizedKeys.add(normalized);
    if (platform !== normalized || !PLATFORM_KEY_PATTERN.test(platform)) {
      return {
        entries: null,
        error: new TypeError(
          `checksum platform key must be canonical lowercase: ${platform}`,
        ),
      };
    }
    if (
      !isPlainObject(entry) ||
      typeof entry.sha256 !== "string" ||
      !SHA256_PATTERN.test(entry.sha256)
    ) {
      return {
        entries: null,
        error: new TypeError(
          `checksum entry for ${platform} must contain a lowercase SHA-256`,
        ),
      };
    }
    const keys = Object.keys(entry).sort();
    const exactChecksumKeys = keys.length === 1 && keys[0] === "sha256";
    const resignableMachOKeys =
      platform.startsWith("darwin-") &&
      keys.length === 2 &&
      keys[0] === "mach_o_signing_independent_sha256" &&
      keys[1] === "sha256";
    if (
      (!exactChecksumKeys && !resignableMachOKeys) ||
      (resignableMachOKeys &&
        (typeof entry.mach_o_signing_independent_sha256 !== "string" ||
          !SHA256_PATTERN.test(entry.mach_o_signing_independent_sha256)))
    ) {
      return {
        entries: null,
        error: new TypeError(
          `checksum entry for ${platform} has an invalid platform checksum profile`,
        ),
      };
    }
  }
  return { entries, error: null };
}

function isPlainObject(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    return false;
  }
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function hashFile(filePath, retainBytes = false) {
  try {
    const file = readFileSync(filePath);
    const sha256 = createHash("sha256").update(file).digest("hex");
    return {
      ok: true,
      sha256,
      fileBytes: file,
      bytes: retainBytes ? file : undefined,
    };
  } catch (error) {
    return { ok: false, error };
  }
}
