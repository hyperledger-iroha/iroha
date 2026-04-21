import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { existsSync, readFileSync } from "node:fs";

const NATIVE_FILENAME = "iroha_js_host.node";
const CHECKSUM_FILENAME = "iroha_js_host.checksums.json";
const VERIFICATION_CACHE = new Map();
const CHECKSUM_CACHE = new Map();

let cachedBinding;
let cachedBindingPath;

function nativeBindingError(reason) {
  return new Error(`Native binding required; ${reason}`);
}

function formatForceNativeVerificationError(verification, paths) {
  switch (verification.status) {
    case "missing_file":
      return nativeBindingError(
        `binding missing at ${paths.bindingPath}; run \`npm run build:native\`.`,
      );
    case "manifest_error":
      return nativeBindingError(
        `checksum manifest at ${paths.checksumPath} is unreadable: ${
          verification.error?.message ?? verification.error
        }.`,
      );
    case "missing_manifest":
    case "missing_expected_entry":
      return nativeBindingError(
        `checksum manifest missing entries for ${verification.platform}; run \`npm run build:native\`.`,
      );
    case "hash_mismatch":
      return nativeBindingError(
        `checksum mismatch for ${paths.bindingPath}; expected ${verification.expectedSha256}, found ${verification.sha256}.`,
      );
    case "hash_error":
    default:
      return nativeBindingError(
        `verification failed (${verification.status}).`,
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
  }
  if (cachedBinding !== undefined) {
    return cachedBinding;
  }

  const verification = verifyNativeBinding(paths.bindingPath, {
    manifestPath: paths.checksumPath,
  });
  if (!verification.ok) {
    throw formatForceNativeVerificationError(verification, paths);
  }

  try {
    const require = createRequire(import.meta.url);
    cachedBinding = require(paths.bindingPath);
  } catch (error) {
    throw nativeBindingError(
      `failed to load binding from ${paths.bindingPath}: ${error?.message ?? error}.`,
    );
  }
  cachedBindingPath = paths.bindingPath;
  return cachedBinding;
}

/** Verify a native binding against the checksum manifest. */
export function verifyNativeBinding(
  bindingPath,
  { manifestPath, expectedChecksums, platformKey, allowMissingExpected = false } = {},
) {
  const cacheKey = [
    bindingPath,
    manifestPath ?? "",
    platformKey ?? "",
    allowMissingExpected ? "allow-missing" : "strict",
  ].join("|");
  const cached = VERIFICATION_CACHE.get(cacheKey);
  if (cached) {
    return cached;
  }

  if (!existsSync(bindingPath)) {
    const result = {
      ok: false,
      status: "missing_file",
      path: bindingPath,
      manifestPath,
    };
    VERIFICATION_CACHE.set(cacheKey, result);
    return result;
  }

  const hash = hashFile(bindingPath);
  if (!hash.ok) {
    const result = {
      ok: false,
      status: "hash_error",
      path: bindingPath,
      manifestPath,
      error: hash.error,
    };
    VERIFICATION_CACHE.set(cacheKey, result);
    return result;
  }

  const platform = platformKey ?? `${process.platform}-${process.arch}`;
  const checksumEntries =
    expectedChecksums ?? loadChecksumEntries(manifestPath ?? resolveNativePaths().checksumPath);

  if (checksumEntries.error) {
    const result = {
      ok: allowMissingExpected,
      status: "manifest_error",
      path: bindingPath,
      manifestPath,
      platform,
      sha256: hash.sha256,
      error: checksumEntries.error,
    };
    VERIFICATION_CACHE.set(cacheKey, result);
    return result;
  }

  if (!checksumEntries.entries) {
    const result = {
      ok: allowMissingExpected,
      status: "missing_manifest",
      path: bindingPath,
      manifestPath,
      platform,
      sha256: hash.sha256,
    };
    VERIFICATION_CACHE.set(cacheKey, result);
    return result;
  }

  const lowerPlatform = typeof platform === "string" ? platform.toLowerCase() : platform;
  const expectedEntry =
    checksumEntries.entries[platform] ?? checksumEntries.entries[lowerPlatform];
  if (!expectedEntry?.sha256) {
    const result = {
      ok: allowMissingExpected,
      status: "missing_expected_entry",
      path: bindingPath,
      manifestPath,
      platform,
      sha256: hash.sha256,
    };
    VERIFICATION_CACHE.set(cacheKey, result);
    return result;
  }

  if (expectedEntry.sha256 !== hash.sha256) {
    const result = {
      ok: false,
      status: "hash_mismatch",
      path: bindingPath,
      manifestPath,
      platform,
      sha256: hash.sha256,
      expectedSha256: expectedEntry.sha256,
    };
    VERIFICATION_CACHE.set(cacheKey, result);
    return result;
  }

  const result = {
    ok: true,
    status: "verified",
    path: bindingPath,
    manifestPath,
    platform,
    sha256: hash.sha256,
    expectedSha256: expectedEntry.sha256,
  };
  VERIFICATION_CACHE.set(cacheKey, result);
  return result;
}

/**
 * Reset cached native state (test helper).
 * @internal
 */
export function __resetNativeStateForTests() {
  cachedBinding = undefined;
  cachedBindingPath = undefined;
  VERIFICATION_CACHE.clear();
  CHECKSUM_CACHE.clear();
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
  const cacheKey = manifestPath ?? "<default>";
  const cached = CHECKSUM_CACHE.get(cacheKey);
  if (cached) {
    return cached;
  }

  if (!manifestPath || !existsSync(manifestPath)) {
    const result = { entries: null, error: null };
    CHECKSUM_CACHE.set(cacheKey, result);
    return result;
  }

  try {
    const raw = readFileSync(manifestPath, "utf8");
    const parsed = JSON.parse(raw);
    const entries = parsed.entries ?? parsed;
    const result = { entries, error: null };
    CHECKSUM_CACHE.set(cacheKey, result);
    return result;
  } catch (error) {
    const result = { entries: null, error };
    CHECKSUM_CACHE.set(cacheKey, result);
    return result;
  }
}

function hashFile(filePath) {
  try {
    const file = readFileSync(filePath);
    const sha256 = createHash("sha256").update(file).digest("hex");
    return { ok: true, sha256 };
  } catch (error) {
    return { ok: false, error };
  }
}
