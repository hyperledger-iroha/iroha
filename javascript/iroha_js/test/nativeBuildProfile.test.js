import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
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
  const repoRoot = mkdtempSync(path.join(os.tmpdir(), "iroha-js-native-build-"));
  try {
    const env = { IROHA_JS_NATIVE_BUILD_PROFILE: "deploy" };
    const nativePath = nativeBuildOutputPath({
      repoRoot,
      cargoProfile: "deploy",
      env,
      platform: "linux",
    });
    const states = [
      { sourceGitRevision: "a".repeat(40), sourceTreeClean: true },
      { sourceGitRevision: "a".repeat(40), sourceTreeClean: true },
    ];
    let written;
    const status = runNativeBuild({
      repoRoot,
      env,
      platform: "linux",
      readSourceState: () => states.shift(),
      runCargo(args) {
        assert.deepEqual(args.slice(-2), ["--profile", "deploy"]);
        mkdirSync(path.dirname(nativePath), { recursive: true });
        writeFileSync(nativePath, "deploy-native-output");
        return { status: 0 };
      },
      writeProvenance(path_, provenance) {
        written = { path: path_, provenance };
      },
    });
    assert.equal(status, 0);
    assert.equal(written.path, nativePath);
    assert.equal(written.provenance.cargo_profile, "deploy");
    assert.equal(written.provenance.source_tree_clean, true);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
});

test("failed Cargo execution does not claim build provenance", () => {
  let writes = 0;
  const status = runNativeBuild({
    repoRoot: "/nonexistent-test-repo",
    env: {},
    readSourceState: () => ({
      sourceGitRevision: "a".repeat(40),
      sourceTreeClean: true,
    }),
    runCargo: () => ({ status: 7 }),
    writeProvenance: () => {
      writes += 1;
    },
  });
  assert.equal(status, 7);
  assert.equal(writes, 0);
});
