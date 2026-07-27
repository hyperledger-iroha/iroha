import assert from "node:assert/strict";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

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
    path.join(snapshotTargetRoot, ".iroha-js-source-snapshot-fixture-"),
  );
  const sourcePath = path.join(
    snapshotRoot,
    "crates",
    "iroha_js_host",
    "src",
    "lib.rs",
  );
  mkdirSync(path.dirname(sourcePath), { recursive: true });
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
  const artifact = {
    reason: "compiler-artifact",
    package_id: `path+file://${snapshot.snapshotRoot}/crates/iroha_js_host#iroha_js_host@0.0.0`,
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
    filenames: [nativePath],
    fresh: false,
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
  return `${messages.map((message) => JSON.stringify(message)).join("\n")}\n`;
}

function successfulCargoJson(snapshot, nativePath, artifactOverrides = {}) {
  return cargoJson(
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

function finalNativePath(repoRoot, cargoProfile = "debug") {
  return nativeBuildOutputPath({
    repoRoot,
    cargoProfile,
    env: {},
    platform: "linux",
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
  repoRoot,
  state = sourceState(),
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
    platform: "linux",
    createSourceSnapshot: snapshots.createSourceSnapshot,
    verifySourceSnapshot: () => state,
    cleanupSourceSnapshot: snapshots.cleanupSourceSnapshot,
    publicationFailpoint,
    runCargo(args, options) {
      const runNativePath = nativeBuildOutputPath({
        repoRoot,
        cargoProfile: "debug",
        env: options.cargoEnv,
        platform: "linux",
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

function assertNoPublicationTransients(nativePath) {
  const nativeName = path.basename(nativePath);
  const unexpected = readdirSync(path.dirname(nativePath)).filter(
    (name) =>
      name.startsWith(`.${nativeName}.stage-`) ||
      name.startsWith(`.${nativeName}.retired-`) ||
      name === `.${nativeName}.publish-lock`,
  );
  assert.deepEqual(unexpected, []);
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
        assert.deepEqual(args.slice(0, 3), [
          "build",
          "--locked",
          "--manifest-path",
        ]);
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
          stdout: successfulCargoJson(snapshots.snapshot, runNativePath),
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
