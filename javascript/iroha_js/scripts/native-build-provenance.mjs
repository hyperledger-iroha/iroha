import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  closeSync,
  constants,
  fsyncSync,
  lstatSync,
  openSync,
  readFileSync,
  readSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";

export const NATIVE_BUILD_PROVENANCE_FILENAME =
  "iroha_js_host.build-provenance.json";
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REVISION_PATTERN = /^[0-9a-f]{40}$/u;
const MAX_PROVENANCE_BYTES = 16 * 1024;

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
  { run = spawnSync } = {},
) {
  const revision = run("git", ["-C", repoRoot, "rev-parse", "HEAD"], {
    encoding: "utf8",
    maxBuffer: 1024 * 1024,
  });
  const status = run(
    "git",
    ["-C", repoRoot, "status", "--porcelain=v1", "--untracked-files=all"],
    { encoding: "utf8", maxBuffer: 16 * 1024 * 1024 },
  );
  const sourceGitRevision = String(revision.stdout ?? "").trim().toLowerCase();
  if (
    revision.status !== 0 ||
    status.status !== 0 ||
    !REVISION_PATTERN.test(sourceGitRevision)
  ) {
    throw new Error("Native build source Git provenance could not be determined.");
  }
  return Object.freeze({
    sourceGitRevision,
    sourceTreeClean: String(status.stdout ?? "").trim().length === 0,
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
      ["sourceGitRevision", "sourceTreeClean"],
      label,
    );
    if (
      !REVISION_PATTERN.test(state.sourceGitRevision) ||
      typeof state.sourceTreeClean !== "boolean"
    ) {
      throw new TypeError(`${label} is invalid`);
    }
  }
  if (sourceBefore.sourceGitRevision !== sourceAfter.sourceGitRevision) {
    throw new Error("Native build source revision changed while Cargo was running.");
  }
  return Object.freeze({
    version: 1,
    cargo_profile: cargoProfile,
    native_sha256: sha256NativeFile(nativePath),
    source_git_revision: sourceAfter.sourceGitRevision,
    source_tree_clean:
      sourceBefore.sourceTreeClean && sourceAfter.sourceTreeClean,
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
      "version",
    ],
    "native build provenance",
  );
  if (
    provenance.version !== 1 ||
    (provenance.cargo_profile !== "debug" &&
      provenance.cargo_profile !== "release" &&
      provenance.cargo_profile !== "deploy") ||
    !SHA256_PATTERN.test(provenance.native_sha256) ||
    !REVISION_PATTERN.test(provenance.source_git_revision) ||
    typeof provenance.source_tree_clean !== "boolean" ||
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
