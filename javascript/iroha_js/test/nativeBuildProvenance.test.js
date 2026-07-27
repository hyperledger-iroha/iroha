import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  createNativeBuildProvenance,
  nativeBuildProvenancePath,
  readNativeBuildProvenance,
  readNativeBuildSourceState,
  validateNativeBuildProvenance,
  writeNativeBuildProvenance,
} from "../scripts/native-build-provenance.mjs";

const REVISION = "a".repeat(40);
const SOURCE_DIGEST = "b".repeat(64);

function sourceState({
  revision = REVISION,
  clean = true,
  digest = SOURCE_DIGEST,
} = {}) {
  return {
    sourceGitRevision: revision,
    sourceTreeClean: clean,
    sourceTreeSha256: digest,
  };
}

function withNativeFixture(run) {
  const directory = mkdtempSync(path.join(os.tmpdir(), "iroha-js-build-provenance-"));
  const nativePath = path.join(directory, "libiroha_js_host.so");
  writeFileSync(nativePath, "compiled-native-fixture");
  try {
    return run({ directory, nativePath });
  } finally {
    rmSync(directory, { recursive: true, force: true });
  }
}

function git(repoRoot, args) {
  const result = spawnSync("git", ["-C", repoRoot, ...args], {
    encoding: "utf8",
    maxBuffer: 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${result.stderr}`);
  }
  return result.stdout.trim();
}

function withSourceRepository(run) {
  const repoRoot = mkdtempSync(path.join(os.tmpdir(), "iroha-js-source-state-"));
  try {
    git(repoRoot, ["init", "--quiet"]);
    writeFileSync(path.join(repoRoot, ".gitignore"), "Cargo.lock\nignored/\n");
    writeFileSync(path.join(repoRoot, "Cargo.lock"), "version = 4\n");
    writeFileSync(path.join(repoRoot, "tracked.txt"), "tracked-v1\n");
    writeFileSync(path.join(repoRoot, "other.txt"), "other\n");
    git(repoRoot, ["add", ".gitignore", "tracked.txt", "other.txt"]);
    git(repoRoot, [
      "-c",
      "user.name=Iroha JS Test",
      "-c",
      "user.email=iroha-js-test@example.invalid",
      "commit",
      "--quiet",
      "-m",
      "fixture",
    ]);
    return run(repoRoot);
  } finally {
    rmSync(repoRoot, { recursive: true, force: true });
  }
}

test("native build provenance V2 binds the exact binary and source-tree digest", () => {
  withNativeFixture(({ nativePath }) => {
    const state = sourceState();
    const provenance = createNativeBuildProvenance({
      cargoProfile: "deploy",
      nativePath,
      sourceBefore: state,
      sourceAfter: state,
    });
    assert.equal(provenance.version, 2);
    assert.equal(provenance.cargo_profile, "deploy");
    assert.match(provenance.native_sha256, /^[0-9a-f]{64}$/u);
    assert.equal(provenance.source_git_revision, REVISION);
    assert.equal(provenance.source_tree_clean, true);
    assert.equal(provenance.source_tree_sha256, SOURCE_DIGEST);

    const written = writeNativeBuildProvenance(nativePath, provenance);
    assert.equal(written, nativeBuildProvenancePath(nativePath));
    assert.deepEqual(readNativeBuildProvenance(nativePath), provenance);
    assert.match(readFileSync(written, "utf8"), /"source_tree_sha256": "[0-9a-f]{64}"/u);
  });
});

test("same-HEAD dirty changes and clean transitions abort native provenance", () => {
  withNativeFixture(({ nativePath }) => {
    const dirtyA = sourceState({ clean: false, digest: "1".repeat(64) });
    const dirtyB = sourceState({ clean: false, digest: "2".repeat(64) });
    const cleanA = sourceState({ clean: true, digest: dirtyA.sourceTreeSha256 });

    const stableDirty = createNativeBuildProvenance({
      cargoProfile: "debug",
      nativePath,
      sourceBefore: dirtyA,
      sourceAfter: dirtyA,
    });
    assert.equal(stableDirty.source_tree_clean, false);
    assert.throws(
      () =>
        createNativeBuildProvenance({
          cargoProfile: "debug",
          nativePath,
          sourceBefore: dirtyA,
          sourceAfter: dirtyB,
        }),
      /source tree changed/u,
    );
    for (const [sourceBefore, sourceAfter] of [
      [cleanA, dirtyA],
      [dirtyA, cleanA],
    ]) {
      assert.throws(
        () =>
          createNativeBuildProvenance({
            cargoProfile: "debug",
            nativePath,
            sourceBefore,
            sourceAfter,
          }),
        /source tree changed/u,
      );
    }
    assert.throws(
      () =>
        createNativeBuildProvenance({
          cargoProfile: "debug",
          nativePath,
          sourceBefore: cleanA,
          sourceAfter: sourceState({
            revision: "c".repeat(40),
            digest: cleanA.sourceTreeSha256,
          }),
        }),
      /revision changed/u,
    );
  });
});

test("source seal covers tracked, untracked, lock, mode, symlink, and deletion state", () => {
  withSourceRepository((repoRoot) => {
    const trackedPath = path.join(repoRoot, "tracked.txt");
    const linkPath = path.join(repoRoot, "tracked-link");
    const lockPath = path.join(repoRoot, "Cargo.lock");
    const untrackedPath = path.join(repoRoot, "loose.txt");
    const base = readNativeBuildSourceState(repoRoot);
    assert.equal(base.sourceTreeClean, true);
    assert.match(base.sourceTreeSha256, /^[0-9a-f]{64}$/u);

    writeFileSync(trackedPath, "tracked-v2\n");
    const trackedChanged = readNativeBuildSourceState(repoRoot);
    assert.equal(trackedChanged.sourceTreeClean, false);
    assert.notEqual(trackedChanged.sourceTreeSha256, base.sourceTreeSha256);
    writeFileSync(trackedPath, "tracked-v1\n");

    writeFileSync(untrackedPath, "loose-v1\n");
    const untrackedA = readNativeBuildSourceState(repoRoot);
    writeFileSync(untrackedPath, "loose-v2\n");
    const untrackedB = readNativeBuildSourceState(repoRoot);
    assert.equal(untrackedA.sourceTreeClean, false);
    assert.equal(untrackedB.sourceTreeClean, false);
    assert.notEqual(untrackedB.sourceTreeSha256, untrackedA.sourceTreeSha256);
    unlinkSync(untrackedPath);

    writeFileSync(lockPath, "version = 4\n# changed ignored lock\n");
    const lockChanged = readNativeBuildSourceState(repoRoot);
    assert.equal(lockChanged.sourceTreeClean, true);
    assert.notEqual(lockChanged.sourceTreeSha256, base.sourceTreeSha256);
    writeFileSync(lockPath, "version = 4\n");

    chmodSync(trackedPath, 0o755);
    const modeChanged = readNativeBuildSourceState(repoRoot);
    assert.notEqual(modeChanged.sourceTreeSha256, base.sourceTreeSha256);
    chmodSync(trackedPath, 0o644);

    if (process.platform !== "win32") {
      symlinkSync("tracked.txt", linkPath);
      const linkA = readNativeBuildSourceState(repoRoot);
      unlinkSync(linkPath);
      symlinkSync("other.txt", linkPath);
      const linkB = readNativeBuildSourceState(repoRoot);
      assert.notEqual(linkB.sourceTreeSha256, linkA.sourceTreeSha256);
      unlinkSync(linkPath);
    }

    unlinkSync(trackedPath);
    const deleted = readNativeBuildSourceState(repoRoot);
    assert.notEqual(deleted.sourceTreeSha256, base.sourceTreeSha256);
  });
});

test("source seal rejects inventory and content mutations during observation", () => {
  withSourceRepository((repoRoot) => {
    let trackedInventoryCalls = 0;
    const mutateBetweenFingerprints = (command, args, options) => {
      if (args.includes("ls-files") && args.includes("--stage")) {
        trackedInventoryCalls += 1;
        if (trackedInventoryCalls === 3) {
          writeFileSync(path.join(repoRoot, "tracked.txt"), "raced-content\n");
        }
      }
      return spawnSync(command, args, options);
    };
    assert.throws(
      () =>
        readNativeBuildSourceState(repoRoot, {
          run: mutateBetweenFingerprints,
        }),
      /changed while its exact fingerprint was captured/u,
    );
  });

  withSourceRepository((repoRoot) => {
    let untrackedInventoryCalls = 0;
    const mutateInventory = (command, args, options) => {
      if (args.includes("ls-files") && args.includes("--others")) {
        untrackedInventoryCalls += 1;
        if (untrackedInventoryCalls === 2) {
          writeFileSync(path.join(repoRoot, "appeared.txt"), "appeared\n");
        }
      }
      return spawnSync(command, args, options);
    };
    assert.throws(
      () => readNativeBuildSourceState(repoRoot, { run: mutateInventory }),
      /source inventory changed/u,
    );
  });
});

test("source seal rejects unresolved index stages before reading source files", () => {
  const run = (_command, args) => {
    if (args.includes("rev-parse")) {
      return { status: 0, stdout: Buffer.from(`${REVISION}\n`) };
    }
    if (args.includes("status")) {
      return { status: 0, stdout: Buffer.alloc(0) };
    }
    if (args.includes("ls-files") && args.includes("--stage")) {
      return {
        status: 0,
        stdout: Buffer.from(`100644 ${"c".repeat(40)} 2\ttracked.txt\0`),
      };
    }
    return { status: 0, stdout: Buffer.alloc(0) };
  };
  assert.throws(
    () => readNativeBuildSourceState("/unused-test-repo", { run }),
    /unsupported mode or unresolved stage/u,
  );
});

test("source seal rejects Git repository, index, and config redirection", () => {
  for (const key of [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_CONFIG_COUNT",
    "GIT_CONFIG_KEY_0",
  ]) {
    assert.throws(
      () =>
        readNativeBuildSourceState("/unused-test-repo", {
          env: { ...process.env, [key]: "/redirected" },
        }),
      new RegExp(`forbids the Git environment override ${key}`, "u"),
    );
  }
});

test(
  "source seal rejects unsafe source filesystem types",
  { skip: process.platform === "win32" },
  () => {
    withSourceRepository((repoRoot) => {
      const fifoPath = path.join(repoRoot, "tracked.txt");
      unlinkSync(fifoPath);
      const created = spawnSync("mkfifo", [fifoPath], { encoding: "utf8" });
      assert.equal(created.status, 0, created.stderr);
      assert.throws(
        () => readNativeBuildSourceState(repoRoot),
        /unsafe file type/u,
      );
    });
  },
);

test("native build provenance rejects stale binaries and malformed V1/V2 fields", () => {
  withNativeFixture(({ nativePath }) => {
    const state = sourceState();
    const provenance = createNativeBuildProvenance({
      cargoProfile: "release",
      nativePath,
      sourceBefore: state,
      sourceAfter: state,
    });
    writeFileSync(nativePath, "different-native-output");
    assert.throws(
      () => validateNativeBuildProvenance(provenance, nativePath),
      /does not match/u,
    );
    writeFileSync(nativePath, "compiled-native-fixture");
    for (const malformed of [
      { ...provenance, unexpected: true },
      { ...provenance, version: 1 },
      { ...provenance, source_tree_sha256: "A".repeat(64) },
      { ...provenance, source_tree_sha256: undefined },
    ]) {
      assert.throws(
        () => validateNativeBuildProvenance(malformed, nativePath),
        /unexpected or missing fields|does not match/u,
      );
    }
  });
});
