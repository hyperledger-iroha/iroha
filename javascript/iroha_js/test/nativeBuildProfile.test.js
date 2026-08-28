import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import {
  chmodSync,
  existsSync,
  linkSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { pathToFileURL } from "node:url";

import {
  cargoBuildArgsForNativeProfile,
  NATIVE_BUILD_PROFILE_ENV,
  resolveNativeBuildProfile,
} from "../scripts/native-build-profile.mjs";
import {
  nativeBuildOutputPath,
  runNativeBuild,
} from "../scripts/build-native.mjs";
import { publishNativeBinding } from "../scripts/copy-native.mjs";
import { nativeSourceProvenanceMatches } from "../src/native.js";

const SOURCE_DIGEST = "b".repeat(64);
const SOURCE_REVISION = "a".repeat(40);

function sourceState(overrides = {}) {
  return {
    sourceGitRevision: SOURCE_REVISION,
    sourceTreeClean: false,
    sourceTreeSha256: SOURCE_DIGEST,
    ...overrides,
  };
}

function cargoArtifactProfile(cargoProfile) {
  const optimized = cargoProfile === "release" || cargoProfile === "deploy";
  return {
    opt_level: optimized ? "3" : "0",
    debuginfo: 0,
    debug_assertions: !optimized,
    overflow_checks: !optimized,
    test: false,
  };
}

function createFixture(t, { profile = "debug" } = {}) {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-live-root-")),
  );
  const targetRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-live-target-")),
  );
  const toolchainsRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-toolchains-")),
  );
  t.after(() => {
    rmSync(repoRoot, { recursive: true, force: true });
    rmSync(targetRoot, { recursive: true, force: true });
    rmSync(toolchainsRoot, { recursive: true, force: true });
  });

  const packageRoot = path.join(repoRoot, "crates", "iroha_js_host");
  const sourcePath = path.join(packageRoot, "src", "lib.rs");
  mkdirSync(path.dirname(sourcePath), { recursive: true });
  writeFileSync(
    path.join(repoRoot, "Cargo.toml"),
    [
      "[workspace]",
      "members = [\"crates/iroha_js_host\"]",
      "",
      "[workspace.package]",
      "version = \"0.0.0\"",
      "",
    ].join("\n"),
  );
  writeFileSync(path.join(repoRoot, "Cargo.lock"), "version = 4\n");
  writeFileSync(
    path.join(repoRoot, "rust-toolchain.toml"),
    "[toolchain]\nchannel = \"1.93.1\"\n",
  );
  writeFileSync(
    path.join(packageRoot, "Cargo.toml"),
    [
      "[package]",
      "name = \"iroha_js_host\"",
      "version.workspace = true",
      "",
      "[lib]",
      "crate-type = [\"cdylib\"]",
      "",
    ].join("\n"),
  );
  writeFileSync(sourcePath, "pub fn fixture() {}\n");

  const binDirectory = path.join(
    toolchainsRoot,
    "1.93.1-fixture",
    "bin",
  );
  mkdirSync(binDirectory, { recursive: true });
  const cargoPath = path.join(binDirectory, "cargo");
  const rustcPath = path.join(binDirectory, "rustc");
  const rustdocPath = path.join(binDirectory, "rustdoc");
  for (const executable of [cargoPath, rustcPath, rustdocPath]) {
    writeFileSync(executable, "#!/bin/sh\nexit 99\n");
    chmodSync(executable, 0o700);
  }

  const env = {
    CARGO_BUILD_JOBS: "1",
    CARGO_INCREMENTAL: "0",
    CARGO_NET_OFFLINE: "true",
    CARGO_TARGET_DIR: targetRoot,
    IROHA_JS_CARGO_LOCKFILE_PATH: path.join(repoRoot, "Cargo.lock"),
    IROHA_JS_CARGO_PATH: cargoPath,
    RUSTC: rustcPath,
    RUSTC_BOOTSTRAP: "1",
    RUSTDOC: rustdocPath,
    ...(profile === "debug"
      ? {}
      : { [NATIVE_BUILD_PROFILE_ENV]: profile }),
  };
  const nativePath = nativeBuildOutputPath({
    repoRoot,
    cargoProfile: profile,
    env,
    platform: "linux",
  });
  return {
    cargoPath,
    env,
    nativePath,
    packageRoot,
    profile,
    repoRoot,
    sourcePath,
    targetRoot,
  };
}

function intendedCargoArtifact(fixture, overrides = {}) {
  const artifact = {
    reason: "compiler-artifact",
    package_id:
      "path+" +
      pathToFileURL(fixture.packageRoot).href +
      "#0.0.0",
    manifest_path: path.join(fixture.packageRoot, "Cargo.toml"),
    target: {
      crate_types: ["cdylib"],
      kind: ["cdylib"],
      name: "iroha_js_host",
      src_path: fixture.sourcePath,
    },
    executable: null,
    features: [],
    filenames: [fixture.nativePath],
    fresh: false,
    profile: cargoArtifactProfile(fixture.profile),
  };
  return {
    ...artifact,
    ...overrides,
    target:
      overrides.target === undefined
        ? artifact.target
        : { ...artifact.target, ...overrides.target },
  };
}

function cargoJson(...messages) {
  return (
    messages.map((message) => JSON.stringify(message)).join("\n") + "\n"
  );
}

function successfulCargoJson(fixture, overrides = {}) {
  return cargoJson(
    intendedCargoArtifact(fixture, overrides),
    { reason: "build-finished", success: true },
  );
}

function writeNativeOutput(fixture, bytes = "native-output") {
  mkdirSync(path.dirname(fixture.nativePath), { recursive: true });
  writeFileSync(fixture.nativePath, bytes);
}

function sha256File(file) {
  return createHash("sha256").update(readFileSync(file)).digest("hex");
}

test("native profiles map to one explicit Cargo profile contract", () => {
  assert.equal(resolveNativeBuildProfile({}), "debug");
  assert.deepEqual(cargoBuildArgsForNativeProfile("debug"), []);
  assert.deepEqual(cargoBuildArgsForNativeProfile("release"), ["--release"]);
  assert.deepEqual(
    cargoBuildArgsForNativeProfile("deploy"),
    ["--profile", "deploy"],
  );
  for (const invalid of ["", "dev", "production", "DEBUG"]) {
    assert.throws(
      () =>
        resolveNativeBuildProfile({
          [NATIVE_BUILD_PROFILE_ENV]: invalid,
        }),
      /must be exactly/u,
    );
  }
});

test("native output requires the caller-provided absolute Cargo target", () => {
  assert.throws(
    () =>
      nativeBuildOutputPath({
        cargoProfile: "debug",
        env: {},
        platform: "linux",
      }),
    /requires CARGO_TARGET_DIR/u,
  );
  assert.throws(
    () =>
      nativeBuildOutputPath({
        cargoProfile: "debug",
        env: { CARGO_TARGET_DIR: "relative-target" },
        platform: "linux",
      }),
    /absolute canonical path/u,
  );
});

test("native build uses the live root, root lock, pinned Cargo, and shared target", (t) => {
  const fixture = createFixture(t);
  const state = sourceState();
  let invalidated;
  let written;
  let sourceReads = 0;
  const status = runNativeBuild({
    repoRoot: fixture.repoRoot,
    env: fixture.env,
    platform: "linux",
    readSourceState(root, options) {
      assert.equal(root, fixture.repoRoot);
      assert.equal(options.env, fixture.env);
      sourceReads += 1;
      return state;
    },
    invalidateProvenance(nativePath) {
      invalidated = nativePath;
    },
    runCargo(cargoPath, args, options) {
      assert.equal(cargoPath, fixture.cargoPath);
      assert.equal(options.cwd, fixture.repoRoot);
      assert.equal(
        options.cargoEnv.CARGO_TARGET_DIR,
        fixture.targetRoot,
      );
      assert.equal(options.cargoEnv.CARGO, fixture.cargoPath);
      assert.equal(options.cargoEnv.RUSTC, fixture.env.RUSTC);
      assert.equal(options.cargoEnv.RUSTDOC, fixture.env.RUSTDOC);
      assert.equal(
        options.cargoEnv.IROHA_GIT_COMMIT_HASH,
        SOURCE_REVISION,
      );
      assert.deepEqual(args, [
        "build",
        "--locked",
        "--offline",
        "--jobs",
        "1",
        "-Z",
        "unstable-options",
        "--lockfile-path",
        path.join(fixture.repoRoot, "Cargo.lock"),
        "--manifest-path",
        path.join(fixture.repoRoot, "Cargo.toml"),
        "--package",
        "iroha_js_host",
        "--lib",
        "--target-dir",
        fixture.targetRoot,
        "--message-format=json-render-diagnostics",
      ]);
      writeNativeOutput(fixture, "authenticated-live-root-output");
      return {
        status: 0,
        stdout: successfulCargoJson(fixture),
      };
    },
    writeProvenance(nativePath, provenance) {
      written = { nativePath, provenance };
    },
  });
  assert.equal(status, 0);
  assert.equal(invalidated, fixture.nativePath);
  assert.equal(written.nativePath, fixture.nativePath);
  assert.equal(written.provenance.source_tree_clean, false);
  assert.equal(written.provenance.source_tree_sha256, SOURCE_DIGEST);
  assert.equal(
    written.provenance.native_sha256,
    sha256File(fixture.nativePath),
  );
  assert.equal(sourceReads, 3);
  assert.equal(
    readFileSync(fixture.nativePath, "utf8"),
    "authenticated-live-root-output",
  );
  assert.equal(
    existsSync(path.join(fixture.targetRoot, "cargo-target")),
    false,
  );
});

test("a fresh Cargo artifact is authenticated without forcing a rebuild", (t) => {
  const fixture = createFixture(t);
  writeNativeOutput(fixture, "already-current-output");
  const originalInode = lstatSync(fixture.nativePath, {
    bigint: true,
  }).ino;
  let written = 0;
  assert.equal(
    runNativeBuild({
      repoRoot: fixture.repoRoot,
      env: fixture.env,
      platform: "linux",
      readSourceState: () => sourceState(),
      invalidateProvenance() {},
      runCargo() {
        return {
          status: 0,
          stdout: successfulCargoJson(fixture, { fresh: true }),
        };
      },
      writeProvenance() {
        written += 1;
      },
    }),
    0,
  );
  assert.equal(written, 1);
  assert.equal(
    readFileSync(fixture.nativePath, "utf8"),
    "already-current-output",
  );
  assert.notEqual(
    lstatSync(fixture.nativePath, { bigint: true }).ino,
    originalInode,
  );
});

test("a non-fresh Cargo artifact must update an existing output", (t) => {
  const fixture = createFixture(t);
  writeNativeOutput(fixture, "stale-output");
  let writes = 0;
  assert.throws(
    () =>
      runNativeBuild({
        repoRoot: fixture.repoRoot,
        env: fixture.env,
        platform: "linux",
        readSourceState: () => sourceState(),
        invalidateProvenance() {},
        runCargo() {
          return {
            status: 0,
            stdout: successfulCargoJson(fixture),
          };
        },
        writeProvenance() {
          writes += 1;
        },
      }),
    /non-fresh Cargo artifact did not update/u,
  );
  assert.equal(writes, 0);
});

test("Cargo hardlink uplift is replaced by one authenticated output link", (t) => {
  const fixture = createFixture(t);
  const dependencyOutput = path.join(
    fixture.targetRoot,
    "debug",
    "deps",
    "libiroha_js_host-fixture.so",
  );
  assert.equal(
    runNativeBuild({
      repoRoot: fixture.repoRoot,
      env: fixture.env,
      platform: "linux",
      readSourceState: () => sourceState(),
      invalidateProvenance() {},
      runCargo() {
        mkdirSync(path.dirname(dependencyOutput), { recursive: true });
        writeFileSync(dependencyOutput, "hardlinked-output");
        linkSync(dependencyOutput, fixture.nativePath);
        assert.equal(
          lstatSync(fixture.nativePath, { bigint: true }).nlink,
          2n,
        );
        return {
          status: 0,
          stdout: successfulCargoJson(fixture),
        };
      },
      writeProvenance() {},
    }),
    0,
  );
  assert.equal(
    lstatSync(fixture.nativePath, { bigint: true }).nlink,
    1n,
  );
  assert.equal(
    lstatSync(dependencyOutput, { bigint: true }).nlink,
    1n,
  );
  assert.equal(readFileSync(fixture.nativePath, "utf8"), "hardlinked-output");
});

test("Cargo JSON must identify the exact live-root cdylib", (t) => {
  const fixture = createFixture(t);
  let writes = 0;
  assert.throws(
    () =>
      runNativeBuild({
        repoRoot: fixture.repoRoot,
        env: fixture.env,
        platform: "linux",
        readSourceState: () => sourceState(),
        invalidateProvenance() {},
        runCargo() {
          writeNativeOutput(fixture);
          return {
            status: 0,
            stdout: successfulCargoJson(fixture, {
              package_id:
                "registry+https://example.invalid/index#iroha_js_host@0.0.0",
            }),
          };
        },
        writeProvenance() {
          writes += 1;
        },
      }),
    /invalid iroha_js_host cdylib/u,
  );
  assert.equal(writes, 0);
});

test("source drift after Cargo prevents provenance publication", (t) => {
  const fixture = createFixture(t);
  const before = sourceState();
  const after = sourceState({ sourceTreeSha256: "c".repeat(64) });
  let reads = 0;
  let writes = 0;
  assert.throws(
    () =>
      runNativeBuild({
        repoRoot: fixture.repoRoot,
        env: fixture.env,
        platform: "linux",
        readSourceState() {
          reads += 1;
          return reads === 1 ? before : after;
        },
        invalidateProvenance() {},
        runCargo() {
          writeNativeOutput(fixture);
          return {
            status: 0,
            stdout: successfulCargoJson(fixture),
          };
        },
        writeProvenance() {
          writes += 1;
        },
      }),
    /source tree changed while Cargo was running/u,
  );
  assert.equal(writes, 0);
});

for (const profile of ["release", "deploy"]) {
  test(profile + " native build rejects a dirty source before Cargo", (t) => {
    const fixture = createFixture(t, { profile });
    let cargoRuns = 0;
    assert.throws(
      () =>
        runNativeBuild({
          repoRoot: fixture.repoRoot,
          env: fixture.env,
          platform: "linux",
          readSourceState: () => sourceState(),
          runCargo() {
            cargoRuns += 1;
            return { status: 7, stdout: "" };
          },
        }),
      /require an exactly clean source tree/u,
    );
    assert.equal(cargoRuns, 0);
  });
}

test("the live build rejects incomplete or redirected build envelopes", async (t) => {
  const fixture = createFixture(t);
  const cases = [
    {
      label: /requires CARGO_INCREMENTAL=0/u,
      mutate(env) {
        delete env.CARGO_INCREMENTAL;
      },
    },
    {
      label: /IROHA_JS_CARGO_PATH must be an absolute canonical path/u,
      mutate(env) {
        env.IROHA_JS_CARGO_PATH = "cargo";
      },
    },
    {
      label: /requires CARGO_TARGET_DIR to be an absolute canonical path/u,
      mutate(env) {
        env.CARGO_TARGET_DIR = "target";
      },
    },
    {
      label: /external Cargo.lock must remain outside the source tree/u,
      mutate(env) {
        const nested = path.join(fixture.repoRoot, "private-lock");
        mkdirSync(nested);
        env.IROHA_JS_CARGO_LOCKFILE_PATH = path.join(nested, "Cargo.lock");
        writeFileSync(env.IROHA_JS_CARGO_LOCKFILE_PATH, "version = 4\n");
      },
    },
    {
      label: /forbids Cargo profile environment override/u,
      mutate(env) {
        env.CARGO_PROFILE_DEV_DEBUG = "0";
      },
    },
  ];
  for (const entry of cases) {
    await t.test(String(entry.label), () => {
      const env = { ...fixture.env };
      entry.mutate(env);
      assert.throws(
        () =>
          runNativeBuild({
            repoRoot: fixture.repoRoot,
            env,
            platform: "linux",
            readSourceState: () => sourceState(),
            runCargo() {
              throw new Error("Cargo must not run");
            },
          }),
        entry.label,
      );
    });
  }
});

test("the live build accepts an authenticated external Cargo.lock", (t) => {
  const fixture = createFixture(t);
  const lockDirectory = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-release-lock-")),
  );
  t.after(() => rmSync(lockDirectory, { recursive: true, force: true }));
  const externalLock = path.join(lockDirectory, "Cargo.lock");
  writeFileSync(externalLock, "version = 4\n");
  const env = {
    ...fixture.env,
    IROHA_JS_CARGO_LOCKFILE_PATH: externalLock,
  };
  let cargoRuns = 0;

  const status = runNativeBuild({
    repoRoot: fixture.repoRoot,
    env,
    platform: "linux",
    readSourceState: () => sourceState(),
    runCargo(_cargo, args) {
      cargoRuns += 1;
      assert.deepEqual(
        args.slice(args.indexOf("--lockfile-path"), args.indexOf("--lockfile-path") + 2),
        ["--lockfile-path", externalLock],
      );
      return { status: 7, stdout: "" };
    },
  });

  assert.equal(status, 7);
  assert.equal(cargoRuns, 1);
});

test("failed Cargo leaves the output unauthenticated", (t) => {
  const fixture = createFixture(t);
  let invalidations = 0;
  let writes = 0;
  const status = runNativeBuild({
    repoRoot: fixture.repoRoot,
    env: fixture.env,
    platform: "linux",
    readSourceState: () => sourceState(),
    invalidateProvenance() {
      invalidations += 1;
    },
    runCargo() {
      return { status: 7, stdout: "" };
    },
    writeProvenance() {
      writes += 1;
    },
  });
  assert.equal(status, 7);
  assert.equal(invalidations, 1);
  assert.equal(writes, 0);
});

function buildProvenance(source, cargoProfile, state) {
  return {
    version: 3,
    build_execution_policy: "trusted-local-cargo-v1",
    cargo_profile: cargoProfile,
    native_sha256: sha256File(source),
    source_git_revision: state.sourceGitRevision,
    source_tree_clean: state.sourceTreeClean,
    source_tree_sha256: state.sourceTreeSha256,
  };
}

function publicationVerifier(file) {
  return {
    ok: true,
    sha256: sha256File(file),
  };
}

test("debug publication accepts only the exact current dirty tree", async (t) => {
  const root = mkdtempSync(
    path.join(os.tmpdir(), "iroha-js-dirty-publication-"),
  );
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const source = path.join(root, "libiroha_js_host.so");
  const destDir = path.join(root, "native");
  const state = sourceState();
  writeFileSync(source, "dirty-tree-native");

  const result = await publishNativeBinding({
    source,
    destDir,
    platform: "linux",
    arch: "x64",
    cargoProfile: "debug",
    signNative() {},
    verifyBinding: publicationVerifier,
    probeBinding() {},
    readBuildProvenance: () => buildProvenance(source, "debug", state),
    readSourceState: () => state,
    log() {},
  });
  const manifest = JSON.parse(readFileSync(result.manifestPath, "utf8"));
  assert.equal(
    manifest.entries["linux-x64"].source_tree_clean,
    false,
  );
  assert.equal(
    manifest.entries["linux-x64"].source_tree_sha256,
    SOURCE_DIGEST,
  );
});

test("dirty publication rejects a different current tree hash", async (t) => {
  const root = mkdtempSync(
    path.join(os.tmpdir(), "iroha-js-dirty-mismatch-"),
  );
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const source = path.join(root, "libiroha_js_host.so");
  const destDir = path.join(root, "native");
  const built = sourceState();
  const current = sourceState({ sourceTreeSha256: "c".repeat(64) });
  writeFileSync(source, "dirty-tree-native");

  await assert.rejects(
    publishNativeBinding({
      source,
      destDir,
      platform: "linux",
      arch: "x64",
      cargoProfile: "debug",
      signNative() {},
      verifyBinding: publicationVerifier,
      probeBinding() {},
      readBuildProvenance: () =>
        buildProvenance(source, "debug", built),
      readSourceState: () => current,
      log() {},
    }),
    /does not match the exact dirty source tree/u,
  );
  assert.equal(existsSync(destDir), false);
});

test("release publication remains clean-only", async (t) => {
  const root = mkdtempSync(
    path.join(os.tmpdir(), "iroha-js-dirty-release-"),
  );
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const source = path.join(root, "libiroha_js_host.so");
  const state = sourceState();
  writeFileSync(source, "dirty-tree-native");

  await assert.rejects(
    publishNativeBinding({
      source,
      destDir: path.join(root, "native"),
      platform: "linux",
      arch: "x64",
      cargoProfile: "release",
      signNative() {},
      verifyBinding: publicationVerifier,
      probeBinding() {},
      readBuildProvenance: () =>
        buildProvenance(source, "release", state),
      readSourceState: () => state,
      log() {},
    }),
    /release publication requires build provenance and current source to be clean/u,
  );
});

test("native loading policy binds dirty debug artifacts to the exact tree", () => {
  const current = sourceState();
  const verification = {
    cargoProfile: "debug",
    sourceGitRevision: SOURCE_REVISION,
    sourceTreeClean: false,
    sourceTreeSha256: SOURCE_DIGEST,
  };
  assert.equal(
    nativeSourceProvenanceMatches(verification, current),
    true,
  );
  assert.equal(
    nativeSourceProvenanceMatches(
      verification,
      sourceState({ sourceTreeSha256: "c".repeat(64) }),
    ),
    false,
  );
  assert.equal(
    nativeSourceProvenanceMatches(
      { ...verification, cargoProfile: "release" },
      current,
    ),
    false,
  );
  assert.equal(
    nativeSourceProvenanceMatches(
      { ...verification, sourceTreeClean: true },
      undefined,
    ),
    true,
  );
});
