import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import {
  chmodSync,
  existsSync,
  linkSync,
  lstatSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  symlinkSync,
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
import {
  createNativeBuildProvenance,
  readNativeBuildProvenance,
  writeNativeBuildProvenance,
} from "../scripts/native-build-provenance.mjs";

const SOURCE_DIGEST = "b".repeat(64);
const SOURCE_REVISION = "a".repeat(40);
const RUN_CONTAINER_PREFIX = ".iroha-js-native-build-run-";
const RUN_INITIALIZER_PREFIX = ".iroha-js-native-build-init-v1-";
const RUN_TRASH_PREFIX = ".iroha-js-native-build-trash-v1-";
const RUN_OWNER_FILENAME = ".iroha-js-native-build-run-owner-v1.json";
const BUILD_NATIVE_SCRIPT = path.resolve("scripts/build-native.mjs");

function sourceState(overrides = {}) {
  return {
    sourceGitRevision: SOURCE_REVISION,
    sourceTreeClean: true,
    sourceTreeSha256: SOURCE_DIGEST,
    ...overrides,
  };
}

function isPathInside(parent, child) {
  const relative = path.relative(parent, child);
  return (
    relative === "" ||
    (relative !== ".." && !relative.startsWith(`..${path.sep}`))
  );
}

function createFixtureSnapshot(snapshotTargetRoot, state) {
  const snapshotRoot = mkdtempSync(
    path.join(snapshotTargetRoot, ".iroha-js-source-snapshot-"),
  );
  const sourcePath = path.join(
    snapshotRoot,
    "crates",
    "iroha_js_host",
    "src",
    "lib.rs",
  );
  mkdirSync(path.dirname(sourcePath), { recursive: true });
  writeFileSync(
    path.join(snapshotRoot, "Cargo.toml"),
    '[workspace.package]\nversion = "0.0.0"\n',
  );
  writeFileSync(path.join(snapshotRoot, "Cargo.lock"), "version = 4\n");
  writeFileSync(
    path.join(snapshotRoot, "crates", "iroha_js_host", "Cargo.toml"),
    '[package]\nname = "iroha_js_host"\nversion.workspace = true\n',
  );
  writeFileSync(sourcePath, "pub fn fixture() {}\n");
  return {
    snapshotRoot,
    sourceState: state,
    targetRoot: snapshotTargetRoot,
  };
}

function cleanupFixtureSnapshot(snapshot) {
  rmSync(snapshot.snapshotRoot, { force: true, recursive: true });
}

function intendedCargoArtifact(snapshot, nativePath, overrides = {}) {
  const packageRoot = path.join(
    snapshot.snapshotRoot,
    "crates",
    "iroha_js_host",
  );
  const artifact = {
    reason: "compiler-artifact",
    package_id: `path+${pathToFileURL(packageRoot).href}#0.0.0`,
    manifest_path: path.join(packageRoot, "Cargo.toml"),
    target: {
      crate_types: ["cdylib"],
      kind: ["cdylib"],
      name: "iroha_js_host",
      src_path: path.join(
        snapshot.snapshotRoot,
        "crates",
        "iroha_js_host",
        "src",
        "lib.rs",
      ),
    },
    executable: null,
    features: [],
    filenames: [nativePath],
    fresh: false,
    profile: cargoArtifactProfile("debug"),
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

function intendedCargoBuildScriptArtifact(snapshot) {
  const packageRoot = path.join(
    snapshot.snapshotRoot,
    "crates",
    "iroha_js_host",
  );
  return {
    reason: "compiler-artifact",
    package_id: `path+${pathToFileURL(packageRoot).href}#0.0.0`,
    manifest_path: path.join(packageRoot, "Cargo.toml"),
    target: {
      crate_types: ["bin"],
      kind: ["custom-build"],
      name: "build-script-build",
      src_path: path.join(packageRoot, "build.rs"),
    },
    executable: null,
    features: [],
    filenames: [
      path.join(
        snapshot.targetRoot,
        "cargo-target",
        "debug",
        "build",
        "iroha_js_host-fixture",
        "build-script-build",
      ),
    ],
    fresh: false,
    profile: cargoArtifactProfile("debug"),
  };
}

function cargoJson(...messages) {
  return `${messages.map((message) => JSON.stringify(message)).join("\n")}\n`;
}

function successfulCargoJson(snapshot, nativePath, artifactOverrides = {}) {
  return cargoJson(
    intendedCargoBuildScriptArtifact(snapshot),
    intendedCargoArtifact(snapshot, nativePath, artifactOverrides),
    { reason: "build-finished", success: true },
  );
}

function createInjectedSnapshotFactory({
  repoRoot,
  state,
  targetRoot,
  onCreate,
}) {
  let snapshot;
  return {
    cleanupSourceSnapshot(actual) {
      assert.equal(actual, snapshot);
      cleanupFixtureSnapshot(actual);
    },
    createSourceSnapshot(root, snapshotTargetRoot, options) {
      assert.equal(root, realpathSync(repoRoot));
      assert.equal(options.env.CARGO_TARGET_DIR, undefined);
      assert.equal(path.dirname(snapshotTargetRoot), realpathSync(os.tmpdir()));
      assert.equal(isPathInside(repoRoot, snapshotTargetRoot), false);
      assert.equal(isPathInside(targetRoot, snapshotTargetRoot), false);
      assert.equal(isPathInside(snapshotTargetRoot, repoRoot), false);
      assert.equal(isPathInside(snapshotTargetRoot, targetRoot), false);
      snapshot = createFixtureSnapshot(snapshotTargetRoot, state);
      onCreate?.(snapshot);
      return snapshot;
    },
    get snapshot() {
      return snapshot;
    },
  };
}

function finalNativePath(
  repoRoot,
  cargoProfile = "debug",
  platform = "linux",
) {
  return nativeBuildOutputPath({
    repoRoot,
    cargoProfile,
    env: {},
    platform,
  });
}

function writeAuthenticatedPair(repoRoot, bytes, state) {
  const nativePath = finalNativePath(repoRoot);
  mkdirSync(path.dirname(nativePath), { recursive: true });
  writeFileSync(nativePath, bytes);
  const provenance = createNativeBuildProvenance({
    cargoProfile: "debug",
    nativePath,
    sourceAfter: state,
    sourceBefore: state,
  });
  writeNativeBuildProvenance(nativePath, provenance);
  return { nativePath, provenance };
}

function runFixtureNativeBuild({
  bytes = "new-native-output",
  cargoHandler,
  publicationFailpoint,
  publicationOwnerIsAlive,
  platform = "linux",
  repoRoot,
  runContainerFailpoint,
  runContainerOwnerIsAlive,
  state = sourceState(),
  writeProvenance,
}) {
  const targetRoot = path.join(repoRoot, "target");
  const snapshots = createInjectedSnapshotFactory({
    repoRoot,
    state,
    targetRoot,
  });
  return runNativeBuild({
    repoRoot,
    env: {},
    platform,
    createSourceSnapshot: snapshots.createSourceSnapshot,
    verifySourceSnapshot: () => state,
    cleanupSourceSnapshot: snapshots.cleanupSourceSnapshot,
    publicationOwnerIsAlive,
    publicationFailpoint,
    runContainerFailpoint,
    runContainerOwnerIsAlive,
    writeProvenance,
    runCargo(args, options) {
      const runNativePath = nativeBuildOutputPath({
        repoRoot,
        cargoProfile: "debug",
        env: options.cargoEnv,
        platform,
      });
      if (cargoHandler !== undefined) {
        return cargoHandler({
          args,
          options,
          runNativePath,
          snapshot: snapshots.snapshot,
        });
      }
      mkdirSync(path.dirname(runNativePath), { recursive: true });
      writeFileSync(runNativePath, bytes);
      return {
        status: 0,
        stdout: successfulCargoJson(snapshots.snapshot, runNativePath),
      };
    },
  });
}

function assertNoPublicationTransients(
  nativePath,
  { allowedStaleOwners = [] } = {},
) {
  const nativeName = path.basename(nativePath);
  const unexpected = readdirSync(path.dirname(nativePath))
    .filter(
    (name) =>
      name.startsWith(`.${nativeName}.stage-`) ||
      name.startsWith(`.${nativeName}.retired-`) ||
      name.startsWith(`.${nativeName}.publish-lock`),
    )
    .sort();
  const expected = allowedStaleOwners
    .map(
      (ownerId) =>
        `.${nativeName}.publish-lock-stale-${ownerId}`,
    )
    .sort();
  assert.deepEqual(unexpected, expected);
}

function writePublicationLockFixture(
  nativePath,
  {
    host = os.hostname(),
    ownerId = "12345678-1234-4123-8123-123456789abc",
    pid = process.pid,
    suffix = ".publish-lock",
  } = {},
) {
  const finalName = path.basename(nativePath);
  const directory = path.dirname(nativePath);
  const lockPath = path.join(directory, `.${finalName}${suffix}`);
  mkdirSync(directory, { recursive: true });
  mkdirSync(lockPath, { mode: 0o700, recursive: false });
  writeFileSync(
    path.join(lockPath, "owner.json"),
    `${JSON.stringify({
      version: 1,
      final_name: finalName,
      host,
      owner_id: ownerId,
      pid,
      retired_name: `.${finalName}.retired-${ownerId}`,
      stage_name: `.${finalName}.stage-${ownerId}`,
    })}\n`,
  );
  return {
    lockPath,
    ownerId,
    retiredPath: path.join(directory, `.${finalName}.retired-${ownerId}`),
    stagePath: path.join(directory, `.${finalName}.stage-${ownerId}`),
  };
}

function writeRunContainerFixture({
  directChild,
  host = os.hostname(),
  kind = "run",
  malformedOwner,
  omitOwner = false,
  pid = 2_000_000_001,
  uid = typeof process.geteuid === "function" ? process.geteuid() : null,
  withCargoTarget = false,
  withSnapshot = false,
} = {}) {
  const parent = realpathSync(os.tmpdir());
  const runId = randomUUID();
  const artifactId = randomUUID();
  const name =
    kind === "run"
      ? `${RUN_CONTAINER_PREFIX}${runId}`
      : kind === "initializer"
        ? `${RUN_INITIALIZER_PREFIX}${runId}-${artifactId}`
        : `${RUN_TRASH_PREFIX}${runId}-${artifactId}`;
  const artifactPath = path.join(parent, name);
  mkdirSync(artifactPath, { mode: 0o700 });
  const metadata = lstatSync(artifactPath, { bigint: true });
  const ownerPath = path.join(artifactPath, RUN_OWNER_FILENAME);
  if (omitOwner) {
    // Exact-name unowned artifact: recovery must preserve it.
  } else if (malformedOwner !== undefined) {
    writeFileSync(ownerPath, malformedOwner, { mode: 0o600 });
  } else {
    writeFileSync(
      ownerPath,
      `${JSON.stringify({
        version: 1,
        run_id: runId,
        run_name: `${RUN_CONTAINER_PREFIX}${runId}`,
        host,
        pid,
        uid,
        directory: {
          birthtime_ns: String(metadata.birthtimeNs),
          dev: String(metadata.dev),
          ino: String(metadata.ino),
        },
      })}\n`,
      { mode: 0o600 },
    );
  }
  if (withCargoTarget) {
    mkdirSync(path.join(artifactPath, "cargo-target"), { mode: 0o700 });
    writeFileSync(
      path.join(artifactPath, "cargo-target", "partial-output"),
      "partial Cargo output",
    );
  }
  if (withSnapshot) {
    const snapshot = mkdtempSync(
      path.join(artifactPath, ".iroha-js-source-snapshot-"),
    );
    writeFileSync(path.join(snapshot, "source.rs"), "snapshot source");
  }
  if (directChild !== undefined) {
    writeFileSync(path.join(artifactPath, directChild), "must survive");
  }
  return {
    artifactId,
    artifactPath,
    ownerPath,
    parent,
    pid,
    runId,
  };
}

test("run janitor thaws a sealed snapshot before dead-run recovery", (t) => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-frozen-")),
  );
  const dead = writeRunContainerFixture({ withSnapshot: true });
  const snapshotName = readdirSync(dead.artifactPath).find((name) =>
    name.startsWith(".iroha-js-source-snapshot-"),
  );
  assert.ok(snapshotName);
  const snapshot = path.join(dead.artifactPath, snapshotName);
  const nested = path.join(snapshot, "nested");
  mkdirSync(nested);
  writeFileSync(path.join(nested, "source.rs"), "sealed source");
  chmodSync(nested, 0o500);
  chmodSync(snapshot, 0o500);
  t.after(() => {
    cleanupExactTestArtifacts(repoRoot);
    for (const name of readdirSync(dead.parent)) {
      if (name.includes(dead.runId)) {
        cleanupExactTestArtifacts(path.join(dead.parent, name));
      }
    }
  });

  assert.equal(
    runRecoveryFixture({
      repoRoot,
      runContainerOwnerIsAlive(pid) {
        return pid !== dead.pid;
      },
    }),
    7,
  );
  assert.equal(
    readdirSync(dead.parent).some((name) => name.includes(dead.runId)),
    false,
  );
});

function runRecoveryFixture({
  repoRoot,
  runContainerFailpoint,
  runContainerOwnerIsAlive,
}) {
  return runFixtureNativeBuild({
    repoRoot,
    runContainerFailpoint,
    runContainerOwnerIsAlive,
    cargoHandler() {
      return { status: 7, stdout: "" };
    },
  });
}

function cleanupExactTestArtifacts(...paths) {
  for (const artifactPath of paths) {
    if (artifactPath !== undefined) {
      rmSync(artifactPath, { recursive: true, force: true });
    }
  }
}

const delay = (milliseconds) =>
  new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));

async function waitForCrashMarker(marker, child, timeoutMs = 10_000) {
  const started = Date.now();
  while (!existsSync(marker)) {
    if (child.exitCode !== null || child.signalCode !== null) {
      throw new Error(
        `native build crash worker exited early: ${child.stderrText}`,
      );
    }
    if (Date.now() - started > timeoutMs) {
      throw new Error("timed out waiting for native build crash marker");
    }
    await delay(10);
  }
}

function waitForChildExit(child) {
  return new Promise((resolveExit, rejectExit) => {
    child.once("error", rejectExit);
    child.once("exit", (code, signal) => resolveExit({ code, signal }));
  });
}

test("native build profile defaults to Cargo debug", () => {
  assert.equal(resolveNativeBuildProfile({}), "debug");
  assert.deepEqual(cargoBuildArgsForNativeProfile("debug"), []);
});

test("native build profile selects Cargo release explicitly", () => {
  assert.equal(
    resolveNativeBuildProfile({ [NATIVE_BUILD_PROFILE_ENV]: "release" }),
    "release",
  );
  assert.deepEqual(cargoBuildArgsForNativeProfile("release"), ["--release"]);
});

test("native build profile selects the hardened Cargo deploy profile", () => {
  assert.equal(
    resolveNativeBuildProfile({ [NATIVE_BUILD_PROFILE_ENV]: "deploy" }),
    "deploy",
  );
  assert.deepEqual(cargoBuildArgsForNativeProfile("deploy"), ["--profile", "deploy"]);
});

test("native build profile rejects ambiguous or unsupported values", () => {
  for (const value of ["", "dev", "production", "Release", " release "]) {
    assert.throws(
      () => resolveNativeBuildProfile({ [NATIVE_BUILD_PROFILE_ENV]: value }),
      /must be exactly "debug", "release", or "deploy"/,
    );
  }
  assert.throws(
    () => cargoBuildArgsForNativeProfile("production"),
    /must be exactly "debug", "release", or "deploy"/,
  );
});

test("native build output paths use the exact platform library filename", () => {
  const repoRoot = path.resolve("/fixture-repository");
  assert.equal(
    path.basename(
      nativeBuildOutputPath({
        repoRoot,
        cargoProfile: "debug",
        env: {},
        platform: "linux",
      }),
    ),
    "libiroha_js_host.so",
  );
  assert.equal(
    path.basename(
      nativeBuildOutputPath({
        repoRoot,
        cargoProfile: "release",
        env: {},
        platform: "darwin",
      }),
    ),
    "libiroha_js_host.dylib",
  );
  assert.equal(
    path.basename(
      nativeBuildOutputPath({
        repoRoot,
        cargoProfile: "deploy",
        env: {},
        platform: "win32",
      }),
    ),
    "iroha_js_host.dll",
  );
});

test("native builds reject inherited Cargo profile environment overrides", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-profile-env-")),
  );
  try {
    for (const key of [
      "CARGO_PROFILE_DEPLOY_LTO",
      "cargo_profile_deploy_lto",
      "Cargo_Profile_Deploy_Lto",
    ]) {
      assert.throws(
        () =>
          runNativeBuild({
            repoRoot,
            env: { [key]: "false" },
          }),
        /forbids Cargo profile environment override/u,
      );
    }
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("successful Cargo execution records provenance for the exact profile output", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-build-")),
  );
  try {
    const env = { IROHA_JS_NATIVE_BUILD_PROFILE: "deploy" };
    const targetRoot = path.join(repoRoot, "target");
    const nativePath = nativeBuildOutputPath({
      repoRoot,
      cargoProfile: "deploy",
      env,
      platform: "linux",
    });
    const state = sourceState();
    const snapshots = createInjectedSnapshotFactory({
      repoRoot,
      state,
      targetRoot,
    });
    let cleaned = 0;
    let written;
    const status = runNativeBuild({
      repoRoot,
      env,
      platform: "linux",
      createSourceSnapshot(root, snapshotTargetRoot, options) {
        assert.equal(options.env, env);
        return snapshots.createSourceSnapshot(
          root,
          snapshotTargetRoot,
          options,
        );
      },
      verifySourceSnapshot(actual) {
        assert.equal(actual, snapshots.snapshot);
        return state;
      },
      cleanupSourceSnapshot(actual) {
        snapshots.cleanupSourceSnapshot(actual);
        cleaned += 1;
      },
      runCargo(args, options) {
        assert.deepEqual(args.slice(0, 7), [
          "build",
          "--locked",
          "--offline",
          "--jobs",
          "1",
          "--manifest-path",
          path.join(snapshots.snapshot.snapshotRoot, "Cargo.toml"),
        ]);
        assert.equal(args.includes("-Z"), false);
        assert.equal(args.includes("--lockfile-path"), false);
        assert.deepEqual(args.slice(-2), ["--profile", "deploy"]);
        assert.equal(
          args[args.indexOf("--package") + 1],
          "iroha_js_host",
        );
        assert.equal(args.includes("--lib"), true);
        const runTargetRoot = options.cargoEnv.CARGO_TARGET_DIR;
        assert.equal(
          args[args.indexOf("--target-dir") + 1],
          runTargetRoot,
        );
        assert.equal(path.basename(runTargetRoot), "cargo-target");
        assert.equal(
          path.dirname(runTargetRoot),
          snapshots.snapshot.targetRoot,
        );
        assert.equal(isPathInside(repoRoot, runTargetRoot), false);
        assert.equal(isPathInside(targetRoot, runTargetRoot), false);
        assert.equal(
          args.includes("--message-format=json-render-diagnostics"),
          true,
        );
        assert.equal(options.cwd, snapshots.snapshot.snapshotRoot);
        assert.equal(options.cargoEnv.IROHA_GIT_COMMIT_HASH, state.sourceGitRevision);
        const runNativePath = nativeBuildOutputPath({
          repoRoot,
          cargoProfile: "deploy",
          env: options.cargoEnv,
          platform: "linux",
        });
        mkdirSync(path.dirname(runNativePath), { recursive: true });
        writeFileSync(runNativePath, "deploy-native-output");
        return {
          status: 0,
          stdout: successfulCargoJson(snapshots.snapshot, runNativePath, {
            profile: cargoArtifactProfile("deploy"),
          }),
        };
      },
      writeProvenance(path_, provenance) {
        written = { path: path_, provenance };
      },
    });
    assert.equal(status, 0);
    assert.equal(written.path, nativePath);
    assert.equal(written.provenance.version, 3);
    assert.equal(
      written.provenance.build_execution_policy,
      "trusted-local-cargo-v1",
    );
    assert.equal(written.provenance.cargo_profile, "deploy");
    assert.equal(written.provenance.source_tree_clean, true);
    assert.equal(written.provenance.source_tree_sha256, SOURCE_DIGEST);
    assert.equal(readFileSync(nativePath, "utf8"), "deploy-native-output");
    assert.equal(cleaned, 1);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("Cargo deps-to-profile hardlink uplift is copied into a singly linked publication source", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-hardlink-uplift-")),
  );
  try {
    assert.equal(
      runFixtureNativeBuild({
        repoRoot,
        cargoHandler({ runNativePath, snapshot }) {
          const depsPath = path.join(
            path.dirname(runNativePath),
            "deps",
            "libiroha_js_host-fixture.so",
          );
          mkdirSync(path.dirname(depsPath), { recursive: true });
          writeFileSync(depsPath, "hardlinked Cargo output");
          linkSync(depsPath, runNativePath);
          assert.equal(lstatSync(depsPath, { bigint: true }).nlink, 2n);
          assert.equal(
            lstatSync(runNativePath, { bigint: true }).nlink,
            2n,
          );
          return {
            status: 0,
            stdout: successfulCargoJson(snapshot, runNativePath),
          };
        },
      }),
      0,
    );
    const nativePath = finalNativePath(repoRoot);
    assert.equal(readFileSync(nativePath, "utf8"), "hardlinked Cargo output");
    assert.equal(lstatSync(nativePath, { bigint: true }).nlink, 1n);
    assert.equal(
      readNativeBuildProvenance(nativePath).source_tree_sha256,
      SOURCE_DIGEST,
    );
    assertNoPublicationTransients(nativePath);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("a Cargo hardlink mutation after private copying is rejected before publication", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-hardlink-race-")),
  );
  let raceInjected = false;
  try {
    assert.throws(
      () =>
        runFixtureNativeBuild({
          repoRoot,
          cargoHandler({ runNativePath, snapshot }) {
            const depsPath = path.join(
              path.dirname(runNativePath),
              "deps",
              "libiroha_js_host-fixture.so",
            );
            mkdirSync(path.dirname(depsPath), { recursive: true });
            writeFileSync(depsPath, "hardlinked Cargo output");
            linkSync(depsPath, runNativePath);
            return {
              status: 0,
              stdout: successfulCargoJson(snapshot, runNativePath),
            };
          },
          runContainerFailpoint(phase, details) {
            if (phase !== "cargo-output-copied") return;
            const size = Number(
              lstatSync(details.sourcePath, { bigint: true }).size,
            );
            writeFileSync(details.sourcePath, Buffer.alloc(size, 0x78));
            raceInjected = true;
          },
        }),
      /changed before digest verification|digest changed after private sealing/u,
    );
    assert.equal(raceInjected, true);
    const nativePath = finalNativePath(repoRoot);
    assert.equal(existsSync(nativePath), false);
    assert.throws(
      () => readNativeBuildProvenance(nativePath),
      /ENOENT|unreadable/u,
    );
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("successful Windows publication selects and authenticates the DLL", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-windows-")),
  );
  try {
    assert.equal(
      runFixtureNativeBuild({
        bytes: "windows-native-output",
        platform: "win32",
        repoRoot,
      }),
      0,
    );
    const nativePath = finalNativePath(repoRoot, "debug", "win32");
    assert.equal(path.basename(nativePath), "iroha_js_host.dll");
    assert.equal(readFileSync(nativePath, "utf8"), "windows-native-output");
    assert.equal(
      readNativeBuildProvenance(nativePath).source_tree_sha256,
      SOURCE_DIGEST,
    );
    assertNoPublicationTransients(nativePath);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("failed Cargo execution does not claim build provenance", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-fail-")),
  );
  try {
    const targetRoot = path.join(repoRoot, "target");
    const state = sourceState();
    const snapshots = createInjectedSnapshotFactory({
      repoRoot,
      state,
      targetRoot,
    });
    let writes = 0;
    let cleaned = 0;
    const status = runNativeBuild({
      repoRoot,
      env: {},
      createSourceSnapshot: snapshots.createSourceSnapshot,
      verifySourceSnapshot: () => state,
      cleanupSourceSnapshot(snapshot) {
        snapshots.cleanupSourceSnapshot(snapshot);
        cleaned += 1;
      },
      runCargo: () => ({ status: 7, stdout: "" }),
      writeProvenance: () => {
        writes += 1;
      },
    });
    assert.equal(status, 7);
    assert.equal(writes, 0);
    assert.equal(cleaned, 1);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("a source-seal mismatch after successful Cargo writes no provenance", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-race-")),
  );
  try {
    const nativePath = nativeBuildOutputPath({
      repoRoot,
      cargoProfile: "debug",
      env: {},
      platform: "linux",
    });
    const states = [
      {
        sourceGitRevision: SOURCE_REVISION,
        sourceTreeClean: false,
        sourceTreeSha256: "1".repeat(64),
      },
      {
        sourceGitRevision: SOURCE_REVISION,
        sourceTreeClean: false,
        sourceTreeSha256: "2".repeat(64),
      },
    ];
    const targetRoot = path.join(repoRoot, "target");
    const snapshots = createInjectedSnapshotFactory({
      repoRoot,
      state: states[0],
      targetRoot,
    });
    let writes = 0;
    let cleaned = 0;
    assert.throws(
      () =>
        runNativeBuild({
          repoRoot,
          env: {},
          platform: "linux",
          createSourceSnapshot: snapshots.createSourceSnapshot,
          verifySourceSnapshot: () => states.shift(),
          cleanupSourceSnapshot(snapshot) {
            snapshots.cleanupSourceSnapshot(snapshot);
            cleaned += 1;
          },
          runCargo(_args, options) {
            const runNativePath = nativeBuildOutputPath({
              repoRoot,
              cargoProfile: "debug",
              env: options.cargoEnv,
              platform: "linux",
            });
            mkdirSync(path.dirname(runNativePath), { recursive: true });
            writeFileSync(runNativePath, "raced-native-output");
            return {
              status: 0,
              stdout: successfulCargoJson(
                snapshots.snapshot,
                runNativePath,
              ),
            };
          },
          writeProvenance() {
            writes += 1;
          },
        }),
      /source tree changed/u,
    );
    assert.equal(writes, 0);
    assert.equal(cleaned, 1);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("a caller-supplied source revision must match the sealed snapshot", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-rev-")),
  );
  try {
    const targetRoot = path.join(repoRoot, "target");
    const state = sourceState();
    const snapshots = createInjectedSnapshotFactory({
      repoRoot,
      state,
      targetRoot,
    });
    let cargoRuns = 0;
    let cleaned = 0;
    assert.throws(
      () =>
        runNativeBuild({
          repoRoot,
          env: { IROHA_GIT_COMMIT_HASH: "c".repeat(40) },
          createSourceSnapshot: snapshots.createSourceSnapshot,
          verifySourceSnapshot: () => state,
          cleanupSourceSnapshot(snapshot) {
            snapshots.cleanupSourceSnapshot(snapshot);
            cleaned += 1;
          },
          runCargo: () => {
            cargoRuns += 1;
            return { status: 0, stdout: "" };
          },
        }),
      /does not match the sealed native build source revision/u,
    );
    assert.equal(cargoRuns, 0);
    assert.equal(cleaned, 1);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

function assertRejectedCargoClaim({
  configureArtifact,
  label,
  output,
  prepareStaleOutput = false,
}) {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-reject-")),
  );
  try {
    const targetRoot = path.join(repoRoot, "target");
    const nativePath = nativeBuildOutputPath({
      repoRoot,
      cargoProfile: "debug",
      env: {},
      platform: "linux",
    });
    if (prepareStaleOutput) {
      mkdirSync(path.dirname(nativePath), { recursive: true });
      writeFileSync(nativePath, "stale-native-output");
    }
    const state = sourceState();
    const snapshots = createInjectedSnapshotFactory({
      repoRoot,
      state,
      targetRoot,
    });
    let writes = 0;
    assert.throws(
      () =>
        runNativeBuild({
          repoRoot,
          env: {},
          platform: "linux",
          createSourceSnapshot: snapshots.createSourceSnapshot,
          verifySourceSnapshot: () => state,
          cleanupSourceSnapshot: snapshots.cleanupSourceSnapshot,
          runCargo(_args, options) {
            const snapshot = snapshots.snapshot;
            const runNativePath = nativeBuildOutputPath({
              repoRoot,
              cargoProfile: "debug",
              env: options.cargoEnv,
              platform: "linux",
            });
            const artifact = intendedCargoArtifact(
              snapshot,
              runNativePath,
            );
            const configured = configureArtifact?.(artifact, {
              nativePath: runNativePath,
              snapshot,
            });
            if (!prepareStaleOutput) {
              mkdirSync(path.dirname(runNativePath), { recursive: true });
              writeFileSync(runNativePath, "new-native-output");
            }
            return {
              status: 0,
              stdout:
                output?.({
                  artifact,
                  configured,
                  nativePath: runNativePath,
                  snapshot,
                }) ??
                cargoJson(
                  configured ?? artifact,
                  { reason: "build-finished", success: true },
                ),
            };
          },
          writeProvenance() {
            writes += 1;
          },
        }),
      label,
    );
    assert.equal(writes, 0);
    if (prepareStaleOutput) {
      assert.equal(readFileSync(nativePath, "utf8"), "stale-native-output");
    }
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
}

test("successful status cannot claim a stale output without a Cargo artifact", () => {
  assertRejectedCargoClaim({
    label: /exactly one iroha_js_host compiler artifact/u,
    output: () => cargoJson({ reason: "build-finished", success: true }),
    prepareStaleOutput: true,
  });
});

test("a non-fresh Cargo event cannot claim an output that Cargo did not update", () => {
  assertRejectedCargoClaim({
    label: /ENOENT|no such file/u,
    prepareStaleOutput: true,
  });
});

test("fresh, wrong-package, wrong-kind, and wrong-path artifacts are rejected", () => {
  const cases = [
    {
      label: "fresh",
      mutate: (artifact) => ({ ...artifact, fresh: true }),
    },
    {
      label: "wrong package",
      mutate: (artifact) => ({
        ...artifact,
        package_id: "registry+https://example.invalid#index@1.0.0",
      }),
    },
    {
      label: "lookalike workspace package",
      mutate: (artifact) => ({
        ...artifact,
        package_id:
          "path+file:///unrelated/iroha_js_host" +
          "#999.0.0",
      }),
    },
    {
      label: "wrong workspace version",
      mutate: (artifact) => ({
        ...artifact,
        package_id: artifact.package_id.replace(/#0\.0\.0$/u, "#999.0.0"),
      }),
    },
    {
      label: "pre-1.77 opaque package ID",
      mutate: (artifact) => ({
        ...artifact,
        package_id:
          "iroha_js_host 0.0.0 " +
          `(path+${pathToFileURL(path.dirname(artifact.manifest_path)).href})`,
      }),
    },
    {
      label: "wrong manifest",
      mutate: (artifact) => ({
        ...artifact,
        manifest_path: path.join(
          path.dirname(artifact.manifest_path),
          "unrelated.toml",
        ),
      }),
    },
    {
      label: "wrong kind",
      mutate: (artifact) => ({
        ...artifact,
        target: {
          ...artifact.target,
          crate_types: ["rlib"],
          kind: ["lib"],
        },
      }),
    },
    {
      label: "enabled feature",
      mutate: (artifact) => ({
        ...artifact,
        features: ["compact-len"],
      }),
    },
    {
      label: "library executable",
      mutate: (artifact) => ({
        ...artifact,
        executable: artifact.filenames[0],
      }),
    },
    {
      label: "wrong compilation profile",
      mutate: (artifact) => ({
        ...artifact,
        profile: {
          ...artifact.profile,
          overflow_checks: false,
        },
      }),
    },
    {
      label: "wrong path",
      mutate: (artifact, { nativePath }) => ({
        ...artifact,
        filenames: [path.join(path.dirname(nativePath), "wrong-output.so")],
      }),
    },
  ];
  for (const scenario of cases) {
    assertRejectedCargoClaim({
      configureArtifact: scenario.mutate,
      label: /invalid iroha_js_host cdylib compiler artifact/u,
    });
  }
});

test("multiple intended artifacts and unsuccessful terminal messages are rejected", () => {
  assertRejectedCargoClaim({
    label: /exactly one iroha_js_host compiler artifact/u,
    output: ({ artifact }) =>
      cargoJson(
        artifact,
        artifact,
        { reason: "build-finished", success: true },
      ),
  });
  assertRejectedCargoClaim({
    label: /successful terminal build-finished/u,
    output: ({ artifact }) =>
      cargoJson(artifact, { reason: "build-finished", success: false }),
  });
  assertRejectedCargoClaim({
    label: /successful terminal build-finished/u,
    output: ({ artifact }) => cargoJson(artifact),
  });
});

test("malformed Cargo JSON is rejected without provenance", () => {
  assertRejectedCargoClaim({
    label: /malformed JSON build message/u,
    output: () => "{not-json}\n",
  });
});

test("run janitor removes only exact dead owned containers", (t) => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-dead-")),
  );
  const dead = writeRunContainerFixture({
    withCargoTarget: true,
    withSnapshot: true,
  });
  t.after(() => {
    cleanupExactTestArtifacts(repoRoot, dead.artifactPath);
  });

  assert.equal(
    runRecoveryFixture({
      repoRoot,
      runContainerOwnerIsAlive(pid) {
        return pid !== dead.pid;
      },
    }),
    7,
  );
  assert.equal(lstatSync(dead.parent).isDirectory(), true);
  assert.equal(
    readdirSync(dead.parent).some((name) => name.includes(dead.runId)),
    false,
  );
});

test("run janitor preserves live, foreign, malformed, and unexpected inventories", (t) => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-preserve-")),
  );
  const live = writeRunContainerFixture({
    pid: process.pid,
    withCargoTarget: true,
  });
  const foreign = writeRunContainerFixture({
    host: "foreign-builder.invalid",
    withCargoTarget: true,
  });
  const malformed = writeRunContainerFixture({
    malformedOwner: '{"version":',
  });
  const unowned = writeRunContainerFixture({ omitOwner: true });
  const unexpected = writeRunContainerFixture({
    directChild: "keep.txt",
  });
  t.after(() => {
    cleanupExactTestArtifacts(
      repoRoot,
      live.artifactPath,
      foreign.artifactPath,
      malformed.artifactPath,
      unowned.artifactPath,
      unexpected.artifactPath,
    );
  });

  assert.equal(
    runRecoveryFixture({
      repoRoot,
      runContainerOwnerIsAlive(pid) {
        if (pid === live.pid) return true;
        if (pid === foreign.pid) {
          throw new Error("foreign owner liveness must not be queried");
        }
        return false;
      },
    }),
    7,
  );
  for (const fixture of [
    live,
    foreign,
    malformed,
    unowned,
    unexpected,
  ]) {
    assert.equal(lstatSync(fixture.artifactPath).isDirectory(), true);
  }
  assert.equal(
    readFileSync(path.join(unexpected.artifactPath, "keep.txt"), "utf8"),
    "must survive",
  );
});

test("partial run initializer and broad prefix user data are forensic-only", (t) => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-init-")),
  );
  const partial = writeRunContainerFixture({
    kind: "initializer",
    malformedOwner: '{"version":',
  });
  const broad = path.join(
    partial.parent,
    `${RUN_CONTAINER_PREFIX}user-data-${randomUUID()}`,
  );
  mkdirSync(broad, { mode: 0o700 });
  writeFileSync(path.join(broad, "keep.txt"), "broad prefix data");
  t.after(() => {
    cleanupExactTestArtifacts(repoRoot, partial.artifactPath, broad);
  });

  assert.equal(
    runRecoveryFixture({
      repoRoot,
      runContainerOwnerIsAlive() {
        return false;
      },
    }),
    7,
  );
  assert.equal(readFileSync(partial.ownerPath, "utf8"), '{"version":');
  assert.equal(
    readFileSync(path.join(broad, "keep.txt"), "utf8"),
    "broad prefix data",
  );
});

test("run initialization crash states never publish an unowned final name", async (t) => {
  await t.test("after initializer mkdir", (subtest) => {
    const repoRoot = realpathSync(
      mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-init-crash-")),
    );
    let initializerPath;
    let runId;
    subtest.after(() => {
      cleanupExactTestArtifacts(repoRoot, initializerPath);
    });

    assert.throws(
      () =>
        runRecoveryFixture({
          repoRoot,
          runContainerFailpoint(phase, details) {
            if (phase !== "run-initializer-created") return;
            initializerPath = details.initializerPath;
            runId = details.runId;
            throw new Error("injected initializer mkdir crash");
          },
        }),
      /injected initializer mkdir crash/u,
    );
    assert.deepEqual(readdirSync(initializerPath), []);
    assert.equal(
      readdirSync(realpathSync(os.tmpdir())).includes(
        `${RUN_CONTAINER_PREFIX}${runId}`,
      ),
      false,
    );
    assert.equal(runRecoveryFixture({ repoRoot }), 7);
    assert.deepEqual(readdirSync(initializerPath), []);
  });

  await t.test("during owner durability", (subtest) => {
    const repoRoot = realpathSync(
      mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-owner-crash-")),
    );
    let initializerPath;
    let ownerPath;
    let runId;
    subtest.after(() => {
      cleanupExactTestArtifacts(repoRoot, initializerPath);
    });

    assert.throws(
      () =>
        runRecoveryFixture({
          repoRoot,
          runContainerFailpoint(phase, details) {
            if (phase !== "run-owner-written") return;
            initializerPath = details.path;
            ownerPath = details.ownerPath;
            runId = details.runId;
            writeFileSync(ownerPath, '{"version":');
            throw new Error("injected partial owner crash");
          },
        }),
      /injected partial owner crash/u,
    );
    assert.equal(readFileSync(ownerPath, "utf8"), '{"version":');
    assert.equal(
      readdirSync(realpathSync(os.tmpdir())).includes(
        `${RUN_CONTAINER_PREFIX}${runId}`,
      ),
      false,
    );
    assert.equal(runRecoveryFixture({ repoRoot }), 7);
    assert.equal(readFileSync(ownerPath, "utf8"), '{"version":');
  });
});

test("fully owned initialization crash phases are recoverable after owner death", async (t) => {
  for (const phase of [
    "run-owner-synced",
    "run-container-renamed",
    "run-container-created",
  ]) {
    await t.test(phase, (subtest) => {
      const repoRoot = realpathSync(
        mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-owned-crash-")),
      );
      let interruptedPath;
      let runId;
      subtest.after(() => {
        cleanupExactTestArtifacts(repoRoot, interruptedPath);
      });

      assert.throws(
        () =>
          runRecoveryFixture({
            repoRoot,
            runContainerFailpoint(actual, details) {
              if (actual !== phase) return;
              interruptedPath = details.path;
              runId = details.runId;
              throw new Error(`injected ${phase}`);
            },
          }),
        new RegExp(`injected ${phase}`, "u"),
      );
      assert.equal(lstatSync(interruptedPath).isDirectory(), true);
      assert.equal(
        readFileSync(
          path.join(interruptedPath, RUN_OWNER_FILENAME),
          "utf8",
        ).includes(runId),
        true,
      );

      assert.equal(
        runRecoveryFixture({
          repoRoot,
          runContainerOwnerIsAlive(pid) {
            return pid !== process.pid;
          },
        }),
        7,
      );
      assert.equal(
        readdirSync(realpathSync(os.tmpdir())).some((name) =>
          name.includes(runId),
        ),
        false,
      );
    });
  }
});

test("run janitor never follows an exact-name symlink root", (t) => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-symlink-")),
  );
  const external = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-victim-")),
  );
  const runId = randomUUID();
  const linkPath = path.join(
    realpathSync(os.tmpdir()),
    `${RUN_CONTAINER_PREFIX}${runId}`,
  );
  writeFileSync(path.join(external, "keep.txt"), "external victim");
  symlinkSync(external, linkPath);
  t.after(() => {
    cleanupExactTestArtifacts(repoRoot, linkPath, external);
  });

  assert.equal(
    runRecoveryFixture({
      repoRoot,
      runContainerOwnerIsAlive() {
        return false;
      },
    }),
    7,
  );
  assert.equal(lstatSync(linkPath).isSymbolicLink(), true);
  assert.equal(
    readFileSync(path.join(external, "keep.txt"), "utf8"),
    "external victim",
  );
});

test("run janitor detects a replacement after dead-owner validation", (t) => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-replace-")),
  );
  const dead = writeRunContainerFixture({ withCargoTarget: true });
  const preserved = `${dead.artifactPath}.preserved`;
  let replaced = false;
  t.after(() => {
    cleanupExactTestArtifacts(
      repoRoot,
      dead.artifactPath,
      preserved,
    );
  });

  assert.throws(
    () =>
      runRecoveryFixture({
        repoRoot,
        runContainerOwnerIsAlive(pid) {
          return pid !== dead.pid;
        },
        runContainerFailpoint(phase, details) {
          if (
            phase !== "run-stale-verified" ||
            details.runId !== dead.runId
          ) {
            return;
          }
          renameSync(dead.artifactPath, preserved);
          mkdirSync(dead.artifactPath, { mode: 0o700 });
          writeFileSync(
            path.join(dead.artifactPath, "keep.txt"),
            "replacement directory",
          );
          replaced = true;
        },
      }),
    /no complete ownership inventory|unsafe|changed/u,
  );
  assert.equal(replaced, true);
  assert.equal(
    readFileSync(path.join(dead.artifactPath, "keep.txt"), "utf8"),
    "replacement directory",
  );
  assert.equal(
    readFileSync(path.join(preserved, RUN_OWNER_FILENAME), "utf8").includes(
      dead.runId,
    ),
    true,
  );
});

test("run janitor does not treat an off-name move as completed deletion", (t) => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-move-")),
  );
  const dead = writeRunContainerFixture({ withCargoTarget: true });
  const preserved = `${dead.artifactPath}.preserved`;
  t.after(() => {
    cleanupExactTestArtifacts(repoRoot, dead.artifactPath, preserved);
  });

  assert.throws(
    () =>
      runRecoveryFixture({
        repoRoot,
        runContainerOwnerIsAlive(pid) {
          return pid !== dead.pid;
        },
        runContainerFailpoint(phase, details) {
          if (
            phase === "run-stale-verified" &&
            details.runId === dead.runId
          ) {
            renameSync(dead.artifactPath, preserved);
          }
        },
      }),
    /ENOENT|no such file/u,
  );
  assert.equal(lstatSync(preserved).isDirectory(), true);
  assert.equal(
    readFileSync(path.join(preserved, RUN_OWNER_FILENAME), "utf8").includes(
      dead.runId,
    ),
    true,
  );
});

test("competing janitors converge only after the witnessed owner is unlinked", async (t) => {
  const phases = [
    "run-stale-verified",
    "run-trash-renamed",
    "run-trash-synced",
    "run-payload-removed:cargo-target",
    "run-owner-retired",
    "run-trash-directory-removed",
  ];
  for (const phase of phases) {
    await t.test(phase, (subtest) => {
      const repoRoot = realpathSync(
        mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-race-")),
      );
      const dead = writeRunContainerFixture({ withCargoTarget: true });
      let contenderCompleted = false;
      subtest.after(() => {
        cleanupExactTestArtifacts(repoRoot);
        for (const name of readdirSync(dead.parent)) {
          if (name.includes(dead.runId)) {
            cleanupExactTestArtifacts(path.join(dead.parent, name));
          }
        }
      });

      assert.equal(
        runRecoveryFixture({
          repoRoot,
          runContainerOwnerIsAlive(pid) {
            return pid !== dead.pid;
          },
          runContainerFailpoint(actual, details) {
            if (
              contenderCompleted ||
              actual !== phase ||
              details.runId !== dead.runId
            ) {
              return;
            }
            assert.equal(
              runRecoveryFixture({
                repoRoot,
                runContainerOwnerIsAlive(pid) {
                  return pid !== dead.pid;
                },
              }),
              7,
            );
            contenderCompleted = true;
          },
        }),
        7,
      );
      assert.equal(contenderCompleted, true);
      assert.equal(
        readdirSync(dead.parent).some((name) =>
          name.includes(dead.runId),
        ),
        false,
      );
    });
  }
});

test("run trash recovery resumes every deletion crash phase", async (t) => {
  const phases = [
    "run-trash-renamed",
    "run-trash-synced",
    "run-payload-removed:cargo-target",
    "run-owner-retired",
    "run-trash-directory-removed",
  ];
  for (const phase of phases) {
    await t.test(phase, (subtest) => {
      const repoRoot = realpathSync(
        mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-crash-")),
      );
      const dead = writeRunContainerFixture({ withCargoTarget: true });
      subtest.after(() => {
        cleanupExactTestArtifacts(repoRoot);
        for (const name of readdirSync(dead.parent)) {
          if (name.includes(dead.runId)) {
            cleanupExactTestArtifacts(path.join(dead.parent, name));
          }
        }
      });
      let interruptedPath;

      assert.throws(
        () =>
          runRecoveryFixture({
            repoRoot,
            runContainerOwnerIsAlive(pid) {
              return pid !== dead.pid;
            },
            runContainerFailpoint(actual, details) {
              if (actual !== phase || details.runId !== dead.runId) return;
              interruptedPath = details.path;
              throw new Error(`injected ${phase}`);
            },
          }),
        new RegExp(`injected ${phase}`, "u"),
      );
      assert.ok(interruptedPath);
      assert.equal(
        readdirSync(dead.parent).some((name) => name.includes(dead.runId)),
        true,
      );

      assert.equal(
        runRecoveryFixture({
          repoRoot,
          runContainerOwnerIsAlive() {
            return true;
          },
        }),
        7,
      );
      assert.equal(
        readdirSync(dead.parent).some((name) => name.includes(dead.runId)),
        false,
      );
    });
  }
});

test("a crash during normal run retirement is reaped by the next build", (t) => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-normal-crash-")),
  );
  let interrupted;
  t.after(() => {
    cleanupExactTestArtifacts(repoRoot, interrupted);
  });

  assert.throws(
    () =>
      runRecoveryFixture({
        repoRoot,
        runContainerFailpoint(phase, details) {
          if (phase !== "run-trash-renamed") return;
          interrupted = details.path;
          throw new Error("injected normal run retirement crash");
        },
      }),
    /injected normal run retirement crash/u,
  );
  assert.ok(interrupted);
  assert.equal(lstatSync(interrupted).isDirectory(), true);

  assert.equal(
    runRecoveryFixture({
      repoRoot,
      runContainerOwnerIsAlive() {
        return true;
      },
    }),
    7,
  );
  assert.equal(
    readdirSync(realpathSync(os.tmpdir())).some((name) =>
      name === path.basename(interrupted),
    ),
    false,
  );
});

test("SIGKILL after run retirement is recovered without Cargo", async (t) => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-run-sigkill-")),
  );
  const marker = path.join(repoRoot, "run-retired.marker.json");
  let interruptedPath;
  t.after(() => {
    cleanupExactTestArtifacts(repoRoot, interruptedPath);
  });
  const workerSource = String.raw`
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { runNativeBuild } from ${JSON.stringify(pathToFileURL(BUILD_NATIVE_SCRIPT).href)};
const blocker = new Int32Array(new SharedArrayBuffer(4));
const state = {
  sourceGitRevision: ${JSON.stringify(SOURCE_REVISION)},
  sourceTreeClean: true,
  sourceTreeSha256: ${JSON.stringify(SOURCE_DIGEST)},
};
await runNativeBuild({
  repoRoot: process.env.REPO_ROOT,
  env: {},
  createSourceSnapshot(_root, targetRoot) {
    const snapshotRoot = mkdtempSync(
      join(targetRoot, ".iroha-js-source-snapshot-"),
    );
    mkdirSync(join(snapshotRoot, "fixture"), { recursive: true });
    return { snapshotRoot, targetRoot };
  },
  verifySourceSnapshot() {
    return state;
  },
  cleanupSourceSnapshot(snapshot) {
    rmSync(snapshot.snapshotRoot, { recursive: true, force: true });
  },
  runCargo() {
    return { status: 7, stdout: "" };
  },
  runContainerFailpoint(phase, details) {
    if (phase !== "run-trash-renamed") return;
    const owner = JSON.parse(
      readFileSync(
        join(
          details.path,
          ".iroha-js-native-build-run-owner-v1.json",
        ),
        "utf8",
      ),
    );
    if (owner.pid !== process.pid) return;
    writeFileSync(
      process.env.MARKER,
      JSON.stringify(details),
      { flag: "wx" },
    );
    Atomics.wait(blocker, 0, 0);
  },
});
`;
  const child = spawn(
    process.execPath,
    ["--input-type=module", "--eval", workerSource],
    {
      env: {
        ...process.env,
        MARKER: marker,
        REPO_ROOT: repoRoot,
      },
      stdio: ["ignore", "ignore", "pipe"],
    },
  );
  child.stderrText = "";
  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    child.stderrText += chunk;
  });

  await waitForCrashMarker(marker, child);
  interruptedPath = JSON.parse(readFileSync(marker, "utf8")).path;
  assert.equal(lstatSync(interruptedPath).isDirectory(), true);
  const exited = waitForChildExit(child);
  assert.equal(child.kill("SIGKILL"), true);
  const exit = await exited;
  assert.equal(exit.code, null);
  assert.equal(exit.signal, "SIGKILL");

  assert.equal(runRecoveryFixture({ repoRoot }), 7);
  assert.equal(existsSync(interruptedPath), false);
});

test("overlapping native builds use distinct private Cargo targets", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-overlap-")),
  );
  try {
    let innerTarget;
    let outerTarget;
    const outerStatus = runFixtureNativeBuild({
      repoRoot,
      cargoHandler({ options }) {
        outerTarget = options.cargoEnv.CARGO_TARGET_DIR;
        const innerStatus = runFixtureNativeBuild({
          repoRoot,
          cargoHandler({ options: innerOptions }) {
            innerTarget = innerOptions.cargoEnv.CARGO_TARGET_DIR;
            return { status: 7, stdout: "" };
          },
        });
        assert.equal(innerStatus, 7);
        return { status: 7, stdout: "" };
      },
    });
    assert.equal(outerStatus, 7);
    assert.notEqual(outerTarget, innerTarget);
    assert.notEqual(path.dirname(outerTarget), path.dirname(innerTarget));
    assert.equal(path.basename(outerTarget), "cargo-target");
    assert.equal(path.basename(innerTarget), "cargo-target");
    const runDirectoryPattern =
      /^\.iroha-js-native-build-run-[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;
    assert.match(path.basename(path.dirname(outerTarget)), runDirectoryPattern);
    assert.match(path.basename(path.dirname(innerTarget)), runDirectoryPattern);
    assert.equal(isPathInside(repoRoot, outerTarget), false);
    assert.equal(isPathInside(repoRoot, innerTarget), false);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("the publication lock rejects an overlapping publisher and removes its stage", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-publish-lock-")),
  );
  try {
    let innerAttempted = false;
    assert.equal(
      runFixtureNativeBuild({
        bytes: "outer-native-output",
        repoRoot,
        publicationFailpoint(stage) {
          if (stage !== "after-invalidation") return;
          innerAttempted = true;
          assert.throws(
            () =>
              runFixtureNativeBuild({
                bytes: "inner-native-output",
                repoRoot,
              }),
            /publication is in progress/u,
          );
          const nativePath = finalNativePath(repoRoot);
          const staged = readdirSync(path.dirname(nativePath)).filter((name) =>
            name.startsWith(`.${path.basename(nativePath)}.stage-`),
          );
          assert.equal(staged.length, 1);
          assert.match(
            staged[0],
            /^\.libiroha_js_host\.so\.stage-[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u,
          );
        },
      }),
      0,
    );
    assert.equal(innerAttempted, true);
    const nativePath = finalNativePath(repoRoot);
    assert.equal(readFileSync(nativePath, "utf8"), "outer-native-output");
    assert.equal(
      readNativeBuildProvenance(nativePath).source_tree_sha256,
      SOURCE_DIGEST,
    );
    assertNoPublicationTransients(nativePath);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("a dead publication owner is recovered with only its durable ABA guard retained", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-stale-lock-")),
  );
  try {
    const nativePath = finalNativePath(repoRoot);
    const stale = writePublicationLockFixture(nativePath, { pid: 424242 });
    const candidateOwner = "23456781-2345-4234-8234-23456789abcd";
    writePublicationLockFixture(nativePath, {
      ownerId: candidateOwner,
      pid: 424242,
      suffix: `.publish-lock-candidate-${candidateOwner}`,
    });
    const releasedOwner = "34567812-3456-4345-8345-3456789abcde";
    writePublicationLockFixture(nativePath, {
      ownerId: releasedOwner,
      pid: 424242,
      suffix: `.publish-lock-released-${releasedOwner}`,
    });
    writeFileSync(stale.stagePath, "partial-staged-output");
    writeFileSync(stale.retiredPath, "retired-prior-output");
    assert.equal(
      runFixtureNativeBuild({
        bytes: "recovered-new-output",
        publicationOwnerIsAlive(pid) {
          assert.equal(pid, 424242);
          return false;
        },
        repoRoot,
      }),
      0,
    );
    assert.equal(readFileSync(nativePath, "utf8"), "recovered-new-output");
    assert.equal(
      readNativeBuildProvenance(nativePath).source_tree_sha256,
      SOURCE_DIGEST,
    );
    assertNoPublicationTransients(nativePath, {
      allowedStaleOwners: [stale.ownerId],
    });
    assert.deepEqual(
      readdirSync(`${stale.lockPath}-stale-${stale.ownerId}`).sort(),
      ["owner.json", "recovered.json"],
    );
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("a delayed stale-owner recovery cannot displace the newer publisher", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-lock-aba-")),
  );
  try {
    const nativePath = finalNativePath(repoRoot);
    const stale = writePublicationLockFixture(nativePath, { pid: 424242 });
    let newerStatus;
    assert.throws(
      () =>
        runFixtureNativeBuild({
          bytes: "delayed-publisher-output",
          publicationOwnerIsAlive(pid) {
            assert.equal(pid, 424242);
            newerStatus = runFixtureNativeBuild({
              bytes: "newer-publisher-output",
              publicationOwnerIsAlive(candidatePid) {
                return candidatePid !== 424242;
              },
              repoRoot,
            });
            return false;
          },
          repoRoot,
        }),
      /stale publication lock recovery is already pending/u,
    );
    assert.equal(newerStatus, 0);
    assert.equal(readFileSync(nativePath, "utf8"), "newer-publisher-output");
    assert.equal(
      readNativeBuildProvenance(nativePath).source_tree_sha256,
      SOURCE_DIGEST,
    );
    assertNoPublicationTransients(nativePath, {
      allowedStaleOwners: [stale.ownerId],
    });
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("a live publication owner is never displaced", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-live-lock-")),
  );
  try {
    const nativePath = finalNativePath(repoRoot);
    const live = writePublicationLockFixture(nativePath);
    assert.throws(
      () => runFixtureNativeBuild({ repoRoot }),
      /publication is in progress/u,
    );
    assert.deepEqual(readdirSync(live.lockPath), ["owner.json"]);
    const names = readdirSync(path.dirname(nativePath)).filter((name) =>
      name.startsWith(`.${path.basename(nativePath)}.publish-lock`),
    );
    assert.deepEqual(names, [path.basename(live.lockPath)]);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("recovery marker link and partial-write crash states converge safely", () => {
  for (const state of ["linked", "partial"]) {
    const repoRoot = realpathSync(
      mkdtempSync(
        path.join(os.tmpdir(), `iroha-js-native-recovery-${state}-`),
      ),
    );
    try {
      const nativePath = finalNativePath(repoRoot);
      const ownerId =
        state === "linked"
          ? "45678123-4567-4456-8456-456789abcdef"
          : "56781234-5678-4567-8567-56789abcdef0";
      const tombstone = writePublicationLockFixture(nativePath, {
        ownerId,
        pid: 424242,
        suffix: `.publish-lock-stale-${ownerId}`,
      });
      const recoveredBy = "67812345-6789-4678-8678-6789abcdef01";
      const temporaryPath = path.join(
        tombstone.lockPath,
        `.recovered.json.${recoveredBy}.tmp`,
      );
      if (state === "linked") {
        writeFileSync(
          temporaryPath,
          `${JSON.stringify({ version: 1, recovered_by: recoveredBy })}\n`,
        );
        linkSync(
          temporaryPath,
          path.join(tombstone.lockPath, "recovered.json"),
        );
      } else {
        writeFileSync(temporaryPath, '{"version":');
      }
      assert.equal(
        runFixtureNativeBuild({
          repoRoot,
          publicationOwnerIsAlive(pid) {
            if (state === "linked") {
              assert.fail(`recovered guard unexpectedly probed PID ${pid}`);
            }
            assert.equal(pid, 424242);
            return false;
          },
        }),
        0,
      );
      assert.deepEqual(readdirSync(tombstone.lockPath).sort(), [
        "owner.json",
        "recovered.json",
      ]);
      assertNoPublicationTransients(nativePath, {
        allowedStaleOwners: [ownerId],
      });
    } finally {
      rmSync(repoRoot, { recursive: true, force: true });
    }
  }
});

test("an incomplete off-name lock initializer is forensic-only and never self-deadlocks", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-lock-init-")),
  );
  try {
    const nativePath = finalNativePath(repoRoot);
    const ownerId = "78123456-789a-4789-8789-789abcdef012";
    const initializerPath = path.join(
      path.dirname(nativePath),
      `.${path.basename(nativePath)}.publish-lock-candidate-${ownerId}`,
    );
    mkdirSync(initializerPath, { mode: 0o700, recursive: true });
    writeFileSync(path.join(initializerPath, "owner.json"), '{"version":');
    assert.equal(
      runFixtureNativeBuild({
        bytes: "first-output",
        repoRoot,
      }),
      0,
    );
    assert.equal(
      runFixtureNativeBuild({
        bytes: "second-output",
        repoRoot,
      }),
      0,
    );
    assert.equal(readFileSync(nativePath, "utf8"), "second-output");
    assert.equal(
      readFileSync(path.join(initializerPath, "owner.json"), "utf8"),
      '{"version":',
    );
    const canonicalLock = path.join(
      path.dirname(nativePath),
      `.${path.basename(nativePath)}.publish-lock`,
    );
    assert.equal(readdirSync(path.dirname(nativePath)).includes(
      path.basename(canonicalLock),
    ), false);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("an uninitialized legacy publication lock is preserved and rejected", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-unsafe-lock-")),
  );
  try {
    const nativePath = finalNativePath(repoRoot);
    const lockPath = path.join(
      path.dirname(nativePath),
      `.${path.basename(nativePath)}.publish-lock`,
    );
    mkdirSync(lockPath, { mode: 0o700, recursive: true });
    assert.throws(
      () => runFixtureNativeBuild({ repoRoot }),
      /invalid inventory/u,
    );
    assert.deepEqual(readdirSync(lockPath), []);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("Cargo failure or missing artifact leaves the prior authenticated pair valid", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-prepublish-")),
  );
  try {
    const oldState = sourceState({
      sourceGitRevision: "c".repeat(40),
      sourceTreeSha256: "d".repeat(64),
    });
    const { nativePath, provenance } = writeAuthenticatedPair(
      repoRoot,
      "prior-native-output",
      oldState,
    );
    const failedStatus = runFixtureNativeBuild({
      repoRoot,
      cargoHandler: () => ({ status: 7, stdout: "" }),
    });
    assert.equal(failedStatus, 7);
    assert.deepEqual(readNativeBuildProvenance(nativePath), provenance);
    assert.equal(readFileSync(nativePath, "utf8"), "prior-native-output");

    assert.throws(
      () =>
        runFixtureNativeBuild({
          repoRoot,
          cargoHandler: () => ({
            status: 0,
            stdout: cargoJson({ reason: "build-finished", success: true }),
          }),
        }),
      /exactly one iroha_js_host compiler artifact/u,
    );
    assert.deepEqual(readNativeBuildProvenance(nativePath), provenance);
    assert.equal(readFileSync(nativePath, "utf8"), "prior-native-output");
    assertNoPublicationTransients(nativePath);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("failure after invalidation leaves the old binary unauthenticated", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-invalidate-")),
  );
  try {
    const oldState = sourceState({
      sourceGitRevision: "c".repeat(40),
      sourceTreeSha256: "d".repeat(64),
    });
    const { nativePath } = writeAuthenticatedPair(
      repoRoot,
      "prior-native-output",
      oldState,
    );
    assert.throws(
      () =>
        runFixtureNativeBuild({
          repoRoot,
          publicationFailpoint(stage) {
            if (stage === "after-invalidation") {
              throw new Error("injected failure after invalidation");
            }
          },
        }),
      /injected failure after invalidation/u,
    );
    assert.equal(readFileSync(nativePath, "utf8"), "prior-native-output");
    assert.throws(
      () => readNativeBuildProvenance(nativePath),
      /ENOENT|unreadable/u,
    );
    assertNoPublicationTransients(nativePath);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("failure after retiring the old binary restores it before releasing ownership", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-retire-fail-")),
  );
  try {
    const oldState = sourceState({
      sourceGitRevision: "c".repeat(40),
      sourceTreeSha256: "d".repeat(64),
    });
    const { nativePath } = writeAuthenticatedPair(
      repoRoot,
      "prior-native-output",
      oldState,
    );
    assert.throws(
      () =>
        runFixtureNativeBuild({
          bytes: "replacement-native-output",
          repoRoot,
          publicationFailpoint(stage) {
            if (stage === "after-binary-retire") {
              throw new Error("injected failure after binary retirement");
            }
          },
        }),
      /injected failure after binary retirement/u,
    );
    assert.equal(readFileSync(nativePath, "utf8"), "prior-native-output");
    assert.throws(
      () => readNativeBuildProvenance(nativePath),
      /ENOENT|unreadable/u,
    );
    assertNoPublicationTransients(nativePath);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("failure immediately after stage rename rolls back only the exact owned binary", async (t) => {
  await t.test("replacement restores the prior binary", () => {
    const repoRoot = realpathSync(
      mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-rename-rollback-")),
    );
    try {
      const { nativePath } = writeAuthenticatedPair(
        repoRoot,
        "prior-native-output",
        sourceState({
          sourceGitRevision: "c".repeat(40),
          sourceTreeSha256: "d".repeat(64),
        }),
      );
      let rollbackRenameObserved = false;
      let rollbackRestoreObserved = false;
      assert.throws(
        () =>
          runFixtureNativeBuild({
            bytes: "replacement-native-output",
            repoRoot,
            publicationFailpoint(stage) {
              if (stage === "after-binary-rename") {
                throw new Error("injected failure after binary rename");
              }
              const names = readdirSync(path.dirname(nativePath));
              const staged = names.filter((name) =>
                name.startsWith(
                  `.${path.basename(nativePath)}.stage-`,
                ),
              );
              const retired = names.filter((name) =>
                name.startsWith(
                  `.${path.basename(nativePath)}.retired-`,
                ),
              );
              if (stage === "after-binary-rollback-rename") {
                assert.equal(existsSync(nativePath), false);
                assert.equal(staged.length, 1);
                assert.equal(retired.length, 1);
                assert.equal(
                  readFileSync(
                    path.join(path.dirname(nativePath), staged[0]),
                    "utf8",
                  ),
                  "replacement-native-output",
                );
                rollbackRenameObserved = true;
              }
              if (stage === "after-binary-rollback-restore") {
                assert.equal(
                  readFileSync(nativePath, "utf8"),
                  "prior-native-output",
                );
                assert.equal(staged.length, 1);
                assert.equal(retired.length, 0);
                rollbackRestoreObserved = true;
              }
            },
          }),
        /injected failure after binary rename/u,
      );
      assert.equal(rollbackRenameObserved, true);
      assert.equal(rollbackRestoreObserved, true);
      assert.equal(readFileSync(nativePath, "utf8"), "prior-native-output");
      assert.throws(
        () => readNativeBuildProvenance(nativePath),
        /ENOENT|unreadable/u,
      );
      assertNoPublicationTransients(nativePath);
    } finally {
      rmSync(repoRoot, { recursive: true, force: true });
    }
  });

  await t.test("first publication removes its unpublished binary", () => {
    const repoRoot = realpathSync(
      mkdtempSync(
        path.join(os.tmpdir(), "iroha-js-native-rename-first-rollback-"),
      ),
    );
    try {
      const nativePath = finalNativePath(repoRoot);
      let rollbackRenameObserved = false;
      assert.throws(
        () =>
          runFixtureNativeBuild({
            bytes: "first-native-output",
            repoRoot,
            publicationFailpoint(stage) {
              if (stage === "after-binary-rename") {
                throw new Error("injected first failure after binary rename");
              }
              if (stage === "after-binary-rollback-rename") {
                const names = readdirSync(path.dirname(nativePath));
                const staged = names.filter((name) =>
                  name.startsWith(
                    `.${path.basename(nativePath)}.stage-`,
                  ),
                );
                assert.equal(existsSync(nativePath), false);
                assert.equal(staged.length, 1);
                assert.equal(
                  readFileSync(
                    path.join(path.dirname(nativePath), staged[0]),
                    "utf8",
                  ),
                  "first-native-output",
                );
                rollbackRenameObserved = true;
              }
            },
          }),
        /injected first failure after binary rename/u,
      );
      assert.equal(rollbackRenameObserved, true);
      assert.throws(() => readFileSync(nativePath), /ENOENT/u);
      assert.throws(
        () => readNativeBuildProvenance(nativePath),
        /ENOENT|unreadable/u,
      );
      assertNoPublicationTransients(nativePath);
    } finally {
      rmSync(repoRoot, { recursive: true, force: true });
    }
  });
});

test("a crash after rollback retirement is recovered from owner-bound paths", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-rollback-crash-")),
  );
  try {
    const { nativePath } = writeAuthenticatedPair(
      repoRoot,
      "prior-native-output",
      sourceState({
        sourceGitRevision: "c".repeat(40),
        sourceTreeSha256: "d".repeat(64),
      }),
    );
    assert.throws(
      () =>
        runFixtureNativeBuild({
          bytes: "replacement-native-output",
          repoRoot,
          publicationFailpoint(stage) {
            if (stage === "after-binary-rename") {
              throw new Error("injected publication failure");
            }
            if (stage === "after-binary-rollback-rename") {
              throw new Error("injected rollback crash");
            }
          },
        }),
      /owner-bound rollback both failed/u,
    );
    const directory = path.dirname(nativePath);
    const nativeName = path.basename(nativePath);
    const names = readdirSync(directory);
    const lockName = `.${nativeName}.publish-lock`;
    const owner = JSON.parse(
      readFileSync(path.join(directory, lockName, "owner.json"), "utf8"),
    );
    assert.equal(existsSync(nativePath), false);
    assert.equal(
      names.filter((name) =>
        name.startsWith(`.${nativeName}.stage-`),
      ).length,
      1,
    );
    assert.equal(
      names.filter((name) =>
        name.startsWith(`.${nativeName}.retired-`),
      ).length,
      1,
    );

    let restoredBeforeRepublish = false;
    assert.throws(
      () =>
        runFixtureNativeBuild({
          bytes: "unused-new-output",
          publicationOwnerIsAlive(pid) {
            assert.equal(pid, process.pid);
            return false;
          },
          publicationFailpoint(stage) {
            if (stage !== "after-invalidation") return;
            assert.equal(
              readFileSync(nativePath, "utf8"),
              "prior-native-output",
            );
            restoredBeforeRepublish = true;
            throw new Error("stop after stale-owner recovery");
          },
          repoRoot,
        }),
      /stop after stale-owner recovery/u,
    );
    assert.equal(restoredBeforeRepublish, true);
    assert.equal(readFileSync(nativePath, "utf8"), "prior-native-output");
    assertNoPublicationTransients(nativePath, {
      allowedStaleOwners: [owner.owner_id],
    });
    assert.throws(
      () => readNativeBuildProvenance(nativePath),
      /ENOENT|unreadable/u,
    );
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("post-rename rollback retains ownership when its final is missing or replaced", async (t) => {
  for (const scenario of ["missing", "replaced"]) {
    await t.test(scenario, (subtest) => {
      const repoRoot = realpathSync(
        mkdtempSync(
          path.join(os.tmpdir(), "iroha-js-native-rollback-ambiguous-"),
        ),
      );
      subtest.after(() => {
        rmSync(repoRoot, { recursive: true, force: true });
      });
      const { nativePath } = writeAuthenticatedPair(
        repoRoot,
        "prior-native-output",
        sourceState({
          sourceGitRevision: "c".repeat(40),
          sourceTreeSha256: "d".repeat(64),
        }),
      );
      assert.throws(
        () =>
          runFixtureNativeBuild({
            bytes: "replacement-native-output",
            repoRoot,
            publicationFailpoint(stage) {
              if (stage !== "after-binary-rename") return;
              rmSync(nativePath);
              if (scenario === "replaced") {
                writeFileSync(nativePath, "unowned-racing-output");
              }
              throw new Error("injected ambiguous rollback");
            },
          }),
        /owner-bound rollback both failed/u,
      );
      if (scenario === "missing") {
        assert.equal(existsSync(nativePath), false);
      } else {
        assert.equal(
          readFileSync(nativePath, "utf8"),
          "unowned-racing-output",
        );
      }
      const directory = path.dirname(nativePath);
      const nativeName = path.basename(nativePath);
      const names = readdirSync(directory);
      assert.equal(
        names.includes(`.${nativeName}.publish-lock`),
        true,
      );
      assert.equal(
        names.filter((name) =>
          name.startsWith(`.${nativeName}.retired-`),
        ).length,
        1,
      );
      assert.throws(
        () => readNativeBuildProvenance(nativePath),
        /ENOENT|unreadable/u,
      );
    });
  }
});

test("publication cleanup retains ownership when an owned stage or retired file disappears", async (t) => {
  for (const scenario of ["stage", "retired"]) {
    await t.test(scenario, (subtest) => {
      const repoRoot = realpathSync(
        mkdtempSync(
          path.join(os.tmpdir(), "iroha-js-native-owned-disappear-"),
        ),
      );
      subtest.after(() => {
        rmSync(repoRoot, { recursive: true, force: true });
      });
      const { nativePath } = writeAuthenticatedPair(
        repoRoot,
        "prior-native-output",
        sourceState({
          sourceGitRevision: "c".repeat(40),
          sourceTreeSha256: "d".repeat(64),
        }),
      );
      const nativeName = path.basename(nativePath);
      const directory = path.dirname(nativePath);
      assert.throws(
        () =>
          runFixtureNativeBuild({
            bytes: "replacement-native-output",
            repoRoot,
            publicationFailpoint(stage) {
              const targetPhase =
                scenario === "stage"
                  ? "after-invalidation"
                  : "after-binary-retire";
              if (stage !== targetPhase) return;
              const ownedPrefix =
                scenario === "stage"
                  ? `.${nativeName}.stage-`
                  : `.${nativeName}.retired-`;
              const owned = readdirSync(directory).filter((name) =>
                name.startsWith(ownedPrefix),
              );
              assert.equal(owned.length, 1);
              rmSync(path.join(directory, owned[0]));
              throw new Error(`injected missing ${scenario}`);
            },
          }),
        /owner-bound rollback both failed/u,
      );
      const names = readdirSync(directory);
      assert.equal(
        names.includes(`.${nativeName}.publish-lock`),
        true,
      );
      if (scenario === "stage") {
        assert.equal(
          readFileSync(nativePath, "utf8"),
          "prior-native-output",
        );
      } else {
        assert.equal(existsSync(nativePath), false);
        assert.equal(
          names.filter((name) =>
            name.startsWith(`.${nativeName}.stage-`),
          ).length,
          1,
        );
      }
      assert.throws(
        () => readNativeBuildProvenance(nativePath),
        /ENOENT|unreadable/u,
      );
    });
  }
});

test("failure after binary publication cannot leave stale provenance valid", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-published-")),
  );
  try {
    const oldState = sourceState({
      sourceGitRevision: "c".repeat(40),
      sourceTreeSha256: "d".repeat(64),
    });
    const { nativePath } = writeAuthenticatedPair(
      repoRoot,
      "prior-native-output",
      oldState,
    );
    assert.throws(
      () =>
        runFixtureNativeBuild({
          bytes: "replacement-native-output",
          repoRoot,
          publicationFailpoint(stage) {
            if (stage === "after-binary-publish") {
              throw new Error("injected failure after binary publication");
            }
          },
        }),
      /injected failure after binary publication/u,
    );
    assert.equal(
      readFileSync(nativePath, "utf8"),
      "replacement-native-output",
    );
    assert.throws(
      () => readNativeBuildProvenance(nativePath),
      /ENOENT|unreadable/u,
    );
    assertNoPublicationTransients(nativePath);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("a provenance writer failure invalidates any sidecar it partially published", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-writer-fail-")),
  );
  try {
    const oldState = sourceState({
      sourceGitRevision: "c".repeat(40),
      sourceTreeSha256: "d".repeat(64),
    });
    const { nativePath } = writeAuthenticatedPair(
      repoRoot,
      "prior-native-output",
      oldState,
    );
    assert.throws(
      () =>
        runFixtureNativeBuild({
          bytes: "replacement-native-output",
          repoRoot,
          writeProvenance(path_, provenance) {
            writeNativeBuildProvenance(path_, provenance);
            throw new Error("injected failure after sidecar publication");
          },
        }),
      /injected failure after sidecar publication/u,
    );
    assert.equal(
      readFileSync(nativePath, "utf8"),
      "replacement-native-output",
    );
    assert.throws(
      () => readNativeBuildProvenance(nativePath),
      /ENOENT|unreadable/u,
    );
    assertNoPublicationTransients(nativePath);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("a final-binary switch after sidecar publication invalidates that sidecar", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-final-switch-")),
  );
  try {
    const { nativePath } = writeAuthenticatedPair(
      repoRoot,
      "prior-native-output",
      sourceState({
        sourceGitRevision: "c".repeat(40),
        sourceTreeSha256: "d".repeat(64),
      }),
    );
    const victimPath = path.join(repoRoot, "switch-victim");
    writeFileSync(victimPath, "switch-victim-bytes");
    assert.throws(
      () =>
        runFixtureNativeBuild({
          bytes: "replacement-native-output",
          repoRoot,
          writeProvenance(path_, provenance) {
            writeNativeBuildProvenance(path_, provenance);
            rmSync(path_);
            symlinkSync(victimPath, path_);
          },
        }),
      /owner-bound rollback both failed/u,
    );
    assert.equal(readFileSync(victimPath, "utf8"), "switch-victim-bytes");
    assert.throws(
      () => readNativeBuildProvenance(nativePath),
      /ENOENT|unreadable/u,
    );
    const names = readdirSync(path.dirname(nativePath));
    const nativeName = path.basename(nativePath);
    assert.equal(
      names.includes(`.${nativeName}.publish-lock`),
      true,
    );
    assert.equal(
      names.filter((name) =>
        name.startsWith(`.${nativeName}.retired-`),
      ).length,
      1,
    );
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("byte-identical replacement publishes the new source provenance", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-identical-")),
  );
  try {
    const oldState = sourceState({
      sourceGitRevision: "c".repeat(40),
      sourceTreeSha256: "d".repeat(64),
    });
    const newState = sourceState({
      sourceGitRevision: "e".repeat(40),
      sourceTreeSha256: "f".repeat(64),
    });
    const { nativePath } = writeAuthenticatedPair(
      repoRoot,
      "identical-native-output",
      oldState,
    );
    assert.equal(
      runFixtureNativeBuild({
        bytes: "identical-native-output",
        repoRoot,
        state: newState,
      }),
      0,
    );
    const provenance = readNativeBuildProvenance(nativePath);
    assert.equal(provenance.source_git_revision, newState.sourceGitRevision);
    assert.equal(provenance.source_tree_sha256, newState.sourceTreeSha256);
    assert.notEqual(
      provenance.source_tree_sha256,
      oldState.sourceTreeSha256,
    );
    assert.equal(readFileSync(nativePath, "utf8"), "identical-native-output");
    assertNoPublicationTransients(nativePath);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("Cargo artifact outside the private run target is rejected before publication", () => {
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-wrong-target-")),
  );
  try {
    const oldState = sourceState({
      sourceGitRevision: "c".repeat(40),
      sourceTreeSha256: "d".repeat(64),
    });
    const { nativePath, provenance } = writeAuthenticatedPair(
      repoRoot,
      "prior-native-output",
      oldState,
    );
    assert.throws(
      () =>
        runFixtureNativeBuild({
          repoRoot,
          cargoHandler({ runNativePath, snapshot }) {
            mkdirSync(path.dirname(runNativePath), { recursive: true });
            writeFileSync(runNativePath, "new-native-output");
            return {
              status: 0,
              stdout: successfulCargoJson(snapshot, nativePath),
            };
          },
        }),
      /invalid iroha_js_host cdylib compiler artifact/u,
    );
    assert.equal(readFileSync(nativePath, "utf8"), "prior-native-output");
    assert.deepEqual(readNativeBuildProvenance(nativePath), provenance);
    assertNoPublicationTransients(nativePath);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("publication rejects symbolic-link and hardlink final targets without touching victims", () => {
  for (const kind of ["symlink", "hardlink"]) {
    const repoRoot = realpathSync(
      mkdtempSync(path.join(os.tmpdir(), `iroha-js-native-${kind}-`)),
    );
    try {
      const nativePath = finalNativePath(repoRoot);
      const victimPath = path.join(repoRoot, `${kind}-victim`);
      mkdirSync(path.dirname(nativePath), { recursive: true });
      writeFileSync(victimPath, `${kind}-victim-bytes`);
      if (kind === "symlink") symlinkSync(victimPath, nativePath);
      else linkSync(victimPath, nativePath);
      assert.throws(
        () => runFixtureNativeBuild({ repoRoot }),
        /canonical singly linked regular file/u,
      );
      assert.equal(
        readFileSync(victimPath, "utf8"),
        `${kind}-victim-bytes`,
      );
      assertNoPublicationTransients(nativePath);
    } finally {
      rmSync(repoRoot, { recursive: true, force: true });
    }
  }
});
