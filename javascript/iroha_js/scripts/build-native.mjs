#!/usr/bin/env node
/** Build the native iroha_js_host library from the authenticated live root. */
import { spawnSync } from "node:child_process";
import { createHash, randomUUID } from "node:crypto";
import {
  accessSync,
  closeSync,
  constants,
  copyFileSync,
  fstatSync,
  fsyncSync,
  lstatSync,
  mkdirSync,
  openSync,
  readSync,
  realpathSync,
  renameSync,
  rmSync,
} from "node:fs";
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
  createNativeBuildProvenance,
  invalidateNativeBuildProvenance,
  NATIVE_BUILD_CARGO_LOCK_ENV,
  readNativeBuildSourceState,
  readStableRegularFile,
  readStableRegularFileDigest,
  writeNativeBuildProvenance,
} from "./native-build-provenance.mjs";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const defaultRepoRoot = join(scriptDir, "..", "..", "..");
const INTENDED_PACKAGE = "iroha_js_host";
const PINNED_RUST_TOOLCHAIN = "1.93.1";
const CARGO_PATH_ENV = "IROHA_JS_CARGO_PATH";
const MAX_CARGO_JSON_BYTES = 64 * 1024 * 1024;
const MAX_CARGO_MANIFEST_BYTES = 4 * 1024 * 1024;
const REQUIRED_BUILD_ENVIRONMENT = Object.freeze({
  CARGO_BUILD_JOBS: "1",
  CARGO_INCREMENTAL: "0",
  CARGO_NET_OFFLINE: "true",
  RUSTC_BOOTSTRAP: "1",
});
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

function isPathInside(parent, child) {
  const pathFromParent = relative(parent, child);
  return (
    pathFromParent === "" ||
    (pathFromParent !== ".." && !pathFromParent.startsWith(".." + sep))
  );
}

function assertDisjointPathAncestry(left, right, label) {
  if (isPathInside(left, right) || isPathInside(right, left)) {
    throw new Error(
      label + " must not contain or be contained by the build source.",
    );
  }
}

function canonicalDirectory(path, label, { create = false } = {}) {
  const resolvedPath = resolve(path);
  if (create) mkdirSync(resolvedPath, { mode: 0o700, recursive: true });
  const metadata = lstatSync(resolvedPath, { bigint: true });
  if (!metadata.isDirectory() || metadata.isSymbolicLink()) {
    throw new Error(label + " must be a real non-symbolic-link directory.");
  }
  const canonicalPath = realpathSync(resolvedPath);
  if (canonicalPath !== resolvedPath) {
    throw new Error(label + " path must be canonical.");
  }
  return Object.freeze({
    canonicalPath,
    identity: Object.freeze({ dev: metadata.dev, ino: metadata.ino }),
  });
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
    throw new Error(label + " changed identity.");
  }
}

function canonicalRegularFile(path, label, { executable = false } = {}) {
  if (
    typeof path !== "string" ||
    path.length === 0 ||
    !isAbsolute(path) ||
    resolve(path) !== path
  ) {
    throw new Error(label + " must be an absolute canonical path.");
  }
  const metadata = lstatSync(path, { bigint: true });
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    realpathSync(path) !== path
  ) {
    throw new Error(label + " must be a canonical regular non-symbolic-link file.");
  }
  if (executable) accessSync(path, constants.X_OK);
  return path;
}

function readUtf8RegularFile(path, label, maximumBytes) {
  return new TextDecoder("utf-8", { fatal: true }).decode(
    readStableRegularFile(path, {
      label,
      maximumBytes,
      requireNonempty: true,
    }).bytes,
  );
}

function readPinnedToolchain(repoRoot) {
  const path = canonicalRegularFile(
    join(repoRoot, "rust-toolchain.toml"),
    "Native build Rust toolchain file",
  );
  const document = readUtf8RegularFile(
    path,
    "Native build Rust toolchain file",
    64 * 1024,
  );
  const channels = [
    ...document.matchAll(
      /^[ \t]*channel[ \t]*=[ \t]*"([^"\r\n]+)"[ \t]*(?:#.*)?$/gmu,
    ),
  ];
  if (channels.length !== 1 || channels[0][1] !== PINNED_RUST_TOOLCHAIN) {
    throw new Error(
      "Native build requires rust-toolchain.toml to pin Rust " +
        PINNED_RUST_TOOLCHAIN +
        ".",
    );
  }
  return channels[0][1];
}

function requiredExecutable(env, key, executableName) {
  const path = canonicalRegularFile(
    env[key],
    "Native build " + key,
    { executable: true },
  );
  const actualName = basename(path).toLowerCase();
  if (
    actualName !== executableName &&
    actualName !== executableName + ".exe"
  ) {
    throw new Error(
      "Native build " + key + " must name the " + executableName + " executable.",
    );
  }
  return path;
}

function validatePinnedExecutables(repoRoot, env) {
  const channel = readPinnedToolchain(repoRoot);
  const cargoPath = requiredExecutable(env, CARGO_PATH_ENV, "cargo");
  const rustcPath = requiredExecutable(env, "RUSTC", "rustc");
  const rustdocPath = requiredExecutable(env, "RUSTDOC", "rustdoc");
  const binDirectory = dirname(cargoPath);
  if (
    dirname(rustcPath) !== binDirectory ||
    dirname(rustdocPath) !== binDirectory
  ) {
    throw new Error(
      "Native build Cargo, rustc, and rustdoc must come from one pinned toolchain.",
    );
  }
  const toolchainDirectory = basename(dirname(binDirectory));
  if (
    toolchainDirectory !== channel &&
    !toolchainDirectory.startsWith(channel + "-")
  ) {
    throw new Error(
      "Native build executables do not belong to the pinned Rust " +
        channel +
        " toolchain.",
    );
  }
  return Object.freeze({ cargoPath, rustcPath, rustdocPath });
}

function validateRequiredEnvironment(env) {
  for (const [key, expected] of Object.entries(REQUIRED_BUILD_ENVIRONMENT)) {
    if (env[key] !== expected) {
      throw new Error(
        "Native build requires " + key + "=" + expected + ".",
      );
    }
  }
}

function canonicalRepoRoot(repoRoot) {
  const resolved = resolve(repoRoot);
  return canonicalDirectory(
    resolved,
    "Native build repository root",
  ).canonicalPath;
}

function canonicalBuildInputs(repoRoot, env) {
  const cargoManifest = canonicalRegularFile(
    join(repoRoot, "Cargo.toml"),
    "Native build root Cargo.toml",
  );
  const configuredLock = env[NATIVE_BUILD_CARGO_LOCK_ENV];
  const selectedLock =
    configuredLock === undefined ? join(repoRoot, "Cargo.lock") : configuredLock;
  if (basename(selectedLock) !== "Cargo.lock") {
    throw new Error(
      "Native build requires " +
        NATIVE_BUILD_CARGO_LOCK_ENV +
        " to name a Cargo.lock file.",
    );
  }
  const cargoLock = canonicalRegularFile(
    selectedLock,
    "Native build selected Cargo.lock",
  );
  return Object.freeze({ cargoLock, cargoManifest });
}

function canonicalTargetRoot(repoRoot, env) {
  const configured = env.CARGO_TARGET_DIR;
  if (
    typeof configured !== "string" ||
    configured.length === 0 ||
    !isAbsolute(configured) ||
    resolve(configured) !== configured
  ) {
    throw new Error(
      "Native build requires CARGO_TARGET_DIR to be an absolute canonical path.",
    );
  }
  const target = canonicalDirectory(
    configured,
    "Native build Cargo target directory",
    { create: true },
  );
  assertDisjointPathAncestry(
    repoRoot,
    target.canonicalPath,
    "Native build Cargo target",
  );
  return target;
}

function assertCargoProfileEnvironment(env) {
  const profileOverride = Object.keys(env).find((key) =>
    key.toUpperCase().startsWith("CARGO_PROFILE_"),
  );
  if (profileOverride !== undefined) {
    throw new Error(
      "Native build forbids Cargo profile environment override " +
        profileOverride +
        ".",
    );
  }
}

function nativeFilename(platform) {
  if (platform === "win32") return "iroha_js_host.dll";
  return "libiroha_js_host." + (platform === "darwin" ? "dylib" : "so");
}

export function nativeBuildOutputPath({
  repoRoot = defaultRepoRoot,
  cargoProfile,
  env = process.env,
  platform = process.platform,
}) {
  if (
    cargoProfile !== "debug" &&
    cargoProfile !== "release" &&
    cargoProfile !== "deploy"
  ) {
    throw new TypeError(
      "Native build Cargo profile must be debug, release, or deploy.",
    );
  }
  const targetRoot = env.CARGO_TARGET_DIR;
  if (
    typeof targetRoot !== "string" ||
    targetRoot.length === 0 ||
    !isAbsolute(targetRoot) ||
    resolve(targetRoot) !== targetRoot
  ) {
    throw new Error(
      "Native build requires CARGO_TARGET_DIR to be an absolute canonical path.",
    );
  }
  void repoRoot;
  return join(targetRoot, cargoProfile, nativeFilename(platform));
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

function readWorkspacePackageVersion(repoRoot) {
  const manifestPath = join(repoRoot, "Cargo.toml");
  const manifest = readUtf8RegularFile(
    manifestPath,
    "Native build workspace Cargo manifest",
    MAX_CARGO_MANIFEST_BYTES,
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
      "path+" +
        pathToFileURL(realpathSync(expectedPackageRoot)).href +
        "#" +
        expectedVersion
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
  { cargoProfile, nativePath, repoRoot },
) {
  const expectedNativePath = resolve(nativePath);
  const expectedPackageRoot = resolve(
    repoRoot,
    "crates",
    INTENDED_PACKAGE,
  );
  const expectedManifestPath = join(expectedPackageRoot, "Cargo.toml");
  const expectedVersion = readWorkspacePackageVersion(repoRoot);
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
    typeof artifact.fresh !== "boolean" ||
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
      // Strict validation reports malformed output after Cargo exits.
    }
  }
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

function cargoArtifactSourceIdentity(path) {
  const metadata = lstatSync(path, { bigint: true });
  if (
    !metadata.isFile() ||
    metadata.isSymbolicLink() ||
    metadata.nlink < 1n ||
    metadata.size === 0n ||
    realpathSync(path) !== resolve(path)
  ) {
    throw new Error(
      "Cargo native build artifact must be a nonempty canonical regular file.",
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

function cargoArtifactIdentityOrNull(path) {
  try {
    return cargoArtifactSourceIdentity(path);
  } catch (error) {
    if (error?.code === "ENOENT" || error?.code === "ENOTDIR") return null;
    throw error;
  }
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
        "Cargo native build artifact changed while it was opened.",
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

function sealCargoArtifactInPlace(nativePath) {
  const sourceBefore = digestCargoArtifactSource(nativePath);
  const temporaryPath = join(
    dirname(nativePath),
    "." +
      basename(nativePath) +
      ".authenticated-" +
      process.pid +
      "-" +
      randomUUID(),
  );
  let renamed = false;
  try {
    copyFileSync(nativePath, temporaryPath, constants.COPYFILE_EXCL);
    const sourceAfter = digestCargoArtifactSource(
      nativePath,
      sourceBefore.identity,
    );
    const temporarySeal = readStableRegularFileDigest(temporaryPath, {
      label: "Native build authenticated output",
      requireNonempty: true,
    });
    if (
      sourceAfter.sha256 !== sourceBefore.sha256 ||
      temporarySeal.sha256 !== sourceBefore.sha256
    ) {
      throw new Error(
        "Cargo native build artifact changed while it was authenticated.",
      );
    }
    renameSync(temporaryPath, nativePath);
    renamed = true;
    syncDirectory(dirname(nativePath));
    const finalSeal = readStableRegularFileDigest(nativePath, {
      label: "Native build authenticated output",
      requireNonempty: true,
    });
    if (finalSeal.sha256 !== sourceBefore.sha256) {
      throw new Error(
        "Native build authenticated output changed during publication.",
      );
    }
    return finalSeal;
  } finally {
    if (!renamed) rmSync(temporaryPath, { force: true });
  }
}

function sameSourceState(left, right) {
  return (
    left?.sourceGitRevision === right?.sourceGitRevision &&
    left?.sourceTreeClean === right?.sourceTreeClean &&
    left?.sourceTreeSha256 === right?.sourceTreeSha256
  );
}

function releaseProfileRequiresCleanSource(cargoProfile, sourceState) {
  if (cargoProfile !== "debug" && sourceState.sourceTreeClean !== true) {
    throw new Error(
      "Native release and deploy builds require an exactly clean source tree.",
    );
  }
}

export function runNativeBuild({
  repoRoot = defaultRepoRoot,
  env = process.env,
  platform = process.platform,
  runCargo = (
    cargoPath,
    args,
    { cwd, cargoEnv },
  ) =>
    spawnSync(cargoPath, args, {
      cwd,
      encoding: "utf8",
      env: cargoEnv,
      maxBuffer: MAX_CARGO_JSON_BYTES,
      stdio: ["inherit", "pipe", "inherit"],
    }),
  readSourceState = readNativeBuildSourceState,
  createProvenance = createNativeBuildProvenance,
  invalidateProvenance = invalidateNativeBuildProvenance,
  writeProvenance = writeNativeBuildProvenance,
} = {}) {
  assertCargoProfileEnvironment(env);
  validateRequiredEnvironment(env);
  const cargoProfile = resolveNativeBuildProfile(env);
  const root = canonicalRepoRoot(repoRoot);
  const inputs = canonicalBuildInputs(root, env);
  const executables = validatePinnedExecutables(root, env);
  const target = canonicalTargetRoot(root, env);
  const nativePath = nativeBuildOutputPath({
    repoRoot: root,
    cargoProfile,
    env: { ...env, CARGO_TARGET_DIR: target.canonicalPath },
    platform,
  });
  const profileDirectory = canonicalDirectory(
    dirname(nativePath),
    "Native build Cargo profile directory",
    { create: true },
  );
  if (profileDirectory.canonicalPath !== dirname(nativePath)) {
    throw new Error("Native build Cargo profile directory is not canonical.");
  }

  const sourceBefore = readSourceState(root, { env });
  const suppliedRevision = env.IROHA_GIT_COMMIT_HASH;
  if (
    suppliedRevision !== undefined &&
    suppliedRevision !== sourceBefore.sourceGitRevision
  ) {
    throw new Error(
      "IROHA_GIT_COMMIT_HASH does not match the authenticated build source.",
    );
  }
  releaseProfileRequiresCleanSource(cargoProfile, sourceBefore);

  const outputBefore = cargoArtifactIdentityOrNull(nativePath);
  invalidateProvenance(nativePath);
  const buildArgs = [
    "build",
    "--locked",
    "--offline",
    "--jobs",
    "1",
    "-Z",
    "unstable-options",
    "--lockfile-path",
    inputs.cargoLock,
    "--manifest-path",
    inputs.cargoManifest,
    "--package",
    INTENDED_PACKAGE,
    "--lib",
    "--target-dir",
    target.canonicalPath,
    "--message-format=json-render-diagnostics",
    ...cargoBuildArgsForNativeProfile(cargoProfile),
  ];
  const cargoEnv = {
    ...env,
    CARGO: executables.cargoPath,
    CARGO_TARGET_DIR: target.canonicalPath,
    [NATIVE_BUILD_CARGO_LOCK_ENV]: inputs.cargoLock,
    IROHA_GIT_COMMIT_HASH: sourceBefore.sourceGitRevision,
    RUSTC: executables.rustcPath,
    RUSTDOC: executables.rustdocPath,
  };
  const build = runCargo(executables.cargoPath, buildArgs, {
    cargoEnv,
    cwd: root,
  });
  forwardCargoRenderedDiagnostics(build?.stdout);
  if (build?.error !== undefined) {
    throw new Error(
      "Cargo could not be executed: " +
        (build.error?.message ?? String(build.error)),
    );
  }
  if (build?.status !== 0) return build?.status ?? 1;

  const artifact = verifyCargoArtifactMessages(build.stdout, {
    cargoProfile,
    nativePath,
    repoRoot: root,
  });
  const outputAfterCargo = cargoArtifactSourceIdentity(nativePath);
  if (
    artifact.fresh === false &&
    outputBefore !== null &&
    sameOutputIdentity(outputBefore, outputAfterCargo)
  ) {
    throw new Error(
      "A non-fresh Cargo artifact did not update the native output.",
    );
  }

  const sealedOutput = sealCargoArtifactInPlace(nativePath);
  const sourceAfter = readSourceState(root, { env });
  const provenance = createProvenance({
    cargoProfile,
    nativePath,
    sourceBefore,
    sourceAfter,
  });
  if (provenance.native_sha256 !== sealedOutput.sha256) {
    throw new Error(
      "Native build provenance does not match the authenticated output.",
    );
  }
  assertDirectoryIdentity(target, "Native build Cargo target directory");
  writeProvenance(nativePath, provenance);
  try {
    const sourceAfterPublication = readSourceState(root, { env });
    if (!sameSourceState(sourceAfter, sourceAfterPublication)) {
      throw new Error(
        "Native build source changed while provenance was published.",
      );
    }
  } catch (error) {
    invalidateProvenance(nativePath);
    throw error;
  }
  return 0;
}

if (
  process.argv[1] &&
  pathToFileURL(resolve(process.argv[1])).href === import.meta.url
) {
  process.exitCode = runNativeBuild();
}
