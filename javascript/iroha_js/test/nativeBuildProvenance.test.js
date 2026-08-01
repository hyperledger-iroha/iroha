import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  linkSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readlinkSync,
  realpathSync,
  rmSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

import {
  createNativeBuildProvenance,
  cleanupNativeBuildSourceSnapshot,
  createNativeBuildSourceSnapshot,
  invalidateNativeBuildProvenance,
  NATIVE_BUILD_CARGO_LOCK_ENV,
  nativeBuildProvenancePath,
  readNativeBuildProvenance,
  readNativeBuildSourceState,
  readStableRegularFile,
  validateNativeBuildProvenance,
  verifyNativeBuildSourceSnapshot,
  writeNativeBuildProvenance,
} from "../scripts/native-build-provenance.mjs";

const REVISION = "a".repeat(40);
const SOURCE_DIGEST = "b".repeat(64);
const SOURCE_STATE_READER = fileURLToPath(
  new URL("../scripts/read-native-build-source-state.mjs", import.meta.url),
);
const REPOSITORY_ROOT = fileURLToPath(new URL("../../../", import.meta.url));
const INHERITED_CARGO_LOCK_PATH = process.env[NATIVE_BUILD_CARGO_LOCK_ENV];

delete process.env[NATIVE_BUILD_CARGO_LOCK_ENV];

test.after(() => {
  if (INHERITED_CARGO_LOCK_PATH === undefined) {
    delete process.env[NATIVE_BUILD_CARGO_LOCK_ENV];
  } else {
    process.env[NATIVE_BUILD_CARGO_LOCK_ENV] = INHERITED_CARGO_LOCK_PATH;
  }
});

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
  const repoRoot = realpathSync(
    mkdtempSync(path.join(os.tmpdir(), "iroha-js-source-state-")),
  );
  try {
    git(repoRoot, ["init", "--quiet"]);
    writeFileSync(
      path.join(repoRoot, ".gitignore"),
      "Cargo.lock\nignored/\ntarget/\n",
    );
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

test("repository ignores every native publication artifact", () => {
  const generatedPaths = [
    "javascript/iroha_js/native/.build-dist.lock",
    "javascript/iroha_js/native/iroha_js_host.node",
    "javascript/iroha_js/native/iroha_js_host.checksums.json",
    "javascript/iroha_js/native/.iroha-js-host-txn-00000000-0000-4000-8000-000000000000/iroha_js_host.node.next",
    "javascript/iroha_js/native/.iroha-js-host-init-txn-v1-00000000-0000-4000-8000-000000000000-00000000-0000-4000-8000-000000000001/iroha_js_host.node.next",
    "javascript/iroha_js/native/.iroha-js-host-cleanup-v1-00000000-0000-4000-8000-000000000000-00000000-0000-4000-8000-000000000001",
    "javascript/iroha_js/native/.iroha-js-host-cleanup-v1-00000000-0000-4000-8000-000000000000-00000000-0000-4000-8000-000000000001.owner.json",
  ];
  for (const generatedPath of generatedPaths) {
    git(REPOSITORY_ROOT, [
      "check-ignore",
      "--quiet",
      "--no-index",
      "--",
      generatedPath,
    ]);
  }
});

test("native build provenance V3 binds the exact binary, source, and execution policy", () => {
  withNativeFixture(({ nativePath }) => {
    const state = sourceState();
    const provenance = createNativeBuildProvenance({
      cargoProfile: "deploy",
      nativePath,
      sourceBefore: state,
      sourceAfter: state,
    });
    assert.equal(provenance.version, 3);
    assert.equal(provenance.build_execution_policy, "trusted-local-cargo-v1");
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

test("source seal binds tracked gitlinks without snapshotting optional submodule contents", () => {
  withSourceRepository((repoRoot) => {
    const gitlinkPath = path.join(repoRoot, "iroha-docs");
    const gitlinkObject = git(repoRoot, ["rev-parse", "HEAD"]);
    git(repoRoot, [
      "update-index",
      "--add",
      "--cacheinfo",
      "160000",
      gitlinkObject,
      "iroha-docs",
    ]);

    const absentCheckout = readNativeBuildSourceState(repoRoot);
    assert.match(absentCheckout.sourceTreeSha256, /^[0-9a-f]{64}$/u);

    mkdirSync(gitlinkPath);
    writeFileSync(path.join(gitlinkPath, "optional-guide.md"), "not a build input\n");
    const snapshot = createNativeBuildSourceSnapshot(
      repoRoot,
      path.join(repoRoot, "target", "gitlink-snapshot"),
    );
    try {
      assert.equal(
        existsSync(path.join(snapshot.snapshotRoot, "iroha-docs")),
        false,
      );
      assert.deepEqual(
        verifyNativeBuildSourceSnapshot(snapshot),
        snapshot.sourceState,
      );
    } finally {
      cleanupNativeBuildSourceSnapshot(snapshot);
    }

    rmSync(gitlinkPath, { recursive: true });
    writeFileSync(gitlinkPath, "unsafe gitlink substitution\n");
    assert.throws(
      () => readNativeBuildSourceState(repoRoot),
      /gitlink worktree entry has an unsafe file type/u,
    );
  });
});

test("synchronous source-state reader accepts only the exact dirty debug provenance", () => {
  withSourceRepository((repoRoot) => {
    writeFileSync(path.join(repoRoot, "tracked.txt"), "tracked-v2\n");
    const state = readNativeBuildSourceState(repoRoot);
    const verification = {
      cargoProfile: "debug",
      ...state,
    };
    const runReader = (expected) =>
      spawnSync(
        process.execPath,
        [
          SOURCE_STATE_READER,
          "--verify",
          repoRoot,
          JSON.stringify(expected),
        ],
        {
          encoding: "utf8",
          env: { ...process.env, NODE_OPTIONS: "" },
          maxBuffer: 16 * 1024,
        },
      );

    const accepted = runReader(verification);
    assert.equal(accepted.status, 0, accepted.stderr);
    assert.equal(accepted.stdout, "");

    const stale = runReader({
      ...verification,
      sourceTreeSha256: "f".repeat(64),
    });
    assert.notEqual(stale.status, 0);
    const release = runReader({ ...verification, cargoProfile: "release" });
    assert.notEqual(release.status, 0);
  });
});

test("selected Cargo lock is snapshotted, fingerprinted, and monitored", () => {
  withSourceRepository((repoRoot) => {
    const privateDirectory = path.join(repoRoot, "target", "private-lock");
    const privateLock = path.join(privateDirectory, "Cargo.lock");
    const rootLock = path.join(repoRoot, "Cargo.lock");
    mkdirSync(privateDirectory, { recursive: true });
    writeFileSync(privateLock, "version = 4\n# selected private lock\n");
    const env = {
      ...process.env,
      [NATIVE_BUILD_CARGO_LOCK_ENV]: privateLock,
    };

    const selected = readNativeBuildSourceState(repoRoot, { env });
    writeFileSync(rootLock, "version = 4\n# unrelated root lock change\n");
    assert.deepEqual(
      readNativeBuildSourceState(repoRoot, { env }),
      selected,
    );
    writeFileSync(privateLock, "version = 4\n# changed selected lock\n");
    assert.notEqual(
      readNativeBuildSourceState(repoRoot, { env }).sourceTreeSha256,
      selected.sourceTreeSha256,
    );

    writeFileSync(rootLock, "version = 4\n");
    writeFileSync(privateLock, "version = 4\n# selected private lock\n");
    const snapshot = createNativeBuildSourceSnapshot(
      repoRoot,
      path.join(repoRoot, "target", "selected-lock-snapshot"),
      { env },
    );
    try {
      assert.equal(
        readFileSync(path.join(snapshot.snapshotRoot, "Cargo.lock"), "utf8"),
        "version = 4\n# selected private lock\n",
      );
      assert.deepEqual(
        verifyNativeBuildSourceSnapshot(snapshot),
        snapshot.sourceState,
      );
      writeFileSync(privateLock, "version = 4\n# drifted selected lock\n");
      assert.throws(
        () => verifyNativeBuildSourceSnapshot(snapshot),
        /Cargo\.lock changed while it was in use/u,
      );
    } finally {
      cleanupNativeBuildSourceSnapshot(snapshot);
    }
  });
});

test(
  "selected Cargo lock rejects relative and symbolic-link paths",
  { skip: process.platform === "win32" },
  () => {
    withSourceRepository((repoRoot) => {
      assert.throws(
        () =>
          readNativeBuildSourceState(repoRoot, {
            env: {
              ...process.env,
              [NATIVE_BUILD_CARGO_LOCK_ENV]: "Cargo.lock",
            },
          }),
        /must name an absolute Cargo\.lock path/u,
      );
      const linkPath = path.join(repoRoot, "target", "linked-Cargo.lock");
      mkdirSync(path.dirname(linkPath), { recursive: true });
      symlinkSync(path.join(repoRoot, "Cargo.lock"), linkPath);
      assert.throws(
        () =>
          readNativeBuildSourceState(repoRoot, {
            env: {
              ...process.env,
              [NATIVE_BUILD_CARGO_LOCK_ENV]: linkPath,
            },
          }),
        /canonical and contain no symbolic-link components/u,
      );
    });
  },
);

test("source seal binds exact stage-0 index bytes even when the dirty worktree is unchanged", () => {
  withSourceRepository((repoRoot) => {
    const trackedPath = path.join(repoRoot, "tracked.txt");
    writeFileSync(trackedPath, "index-a\n");
    git(repoRoot, ["add", "tracked.txt"]);
    writeFileSync(trackedPath, "unchanged-worktree\n");
    const indexA = readNativeBuildSourceState(repoRoot);

    const blobInput = path.join(repoRoot, "ignored", "index-b");
    mkdirSync(path.dirname(blobInput), { recursive: true });
    writeFileSync(blobInput, "index-b\n");
    const object = git(repoRoot, ["hash-object", "-w", blobInput]);
    git(repoRoot, [
      "update-index",
      "--cacheinfo",
      "100644",
      object,
      "tracked.txt",
    ]);
    const indexB = readNativeBuildSourceState(repoRoot);

    assert.equal(indexA.sourceGitRevision, indexB.sourceGitRevision);
    assert.equal(indexA.sourceTreeClean, false);
    assert.equal(indexB.sourceTreeClean, false);
    assert.notEqual(indexA.sourceTreeSha256, indexB.sourceTreeSha256);
  });
});

test(
  "private source snapshot remains sealed across original A-to-B-to-A changes",
  { skip: process.platform === "win32" },
  () => {
    withSourceRepository((repoRoot) => {
      const trackedPath = path.join(repoRoot, "tracked.txt");
      const linkPath = path.join(repoRoot, "snapshot-link");
      chmodSync(trackedPath, 0o755);
      symlinkSync("tracked.txt", linkPath);
      const targetRoot = path.join(repoRoot, "target", "native-test");
      const snapshot = createNativeBuildSourceSnapshot(repoRoot, targetRoot);
      try {
        const snapshotPath = path.join(snapshot.snapshotRoot, "tracked.txt");
        assert.equal(readFileSync(snapshotPath, "utf8"), "tracked-v1\n");
        assert.notEqual(
          lstatSync(snapshotPath).mode & 0o111,
          0,
          "tracked executable mode must survive snapshotting",
        );
        assert.equal(
          readlinkSync(path.join(snapshot.snapshotRoot, "snapshot-link")),
          "tracked.txt",
        );
        writeFileSync(trackedPath, "transient-cargo-race\n");
        assert.equal(readFileSync(snapshotPath, "utf8"), "tracked-v1\n");
        writeFileSync(trackedPath, "tracked-v1\n");
        assert.deepEqual(
          verifyNativeBuildSourceSnapshot(snapshot),
          snapshot.sourceState,
        );
      } finally {
        cleanupNativeBuildSourceSnapshot(snapshot);
      }
      assert.equal(existsSync(snapshot.snapshotRoot), false);
    });
  },
);

test(
  "private source snapshot rejects symlinks that resolve outside its sealed root",
  { skip: process.platform === "win32" },
  () => {
    withSourceRepository((repoRoot) => {
      symlinkSync(os.tmpdir(), path.join(repoRoot, "external-link"));
      assert.throws(
        () =>
          createNativeBuildSourceSnapshot(
            repoRoot,
            path.join(repoRoot, "target", "external-link-test"),
          ),
        /symlink must resolve strictly within/u,
      );
    });
  },
);

test(
  "private source snapshot permits a lexically contained dangling symlink",
  { skip: process.platform === "win32" },
  () => {
    withSourceRepository((repoRoot) => {
      symlinkSync(
        "missing/generated-artifact",
        path.join(repoRoot, "internal-dangling-link"),
      );
      const snapshot = createNativeBuildSourceSnapshot(
        repoRoot,
        path.join(repoRoot, "target", "internal-dangling-test"),
      );
      try {
        assert.equal(
          readlinkSync(
            path.join(snapshot.snapshotRoot, "internal-dangling-link"),
          ),
          "missing/generated-artifact",
        );
        verifyNativeBuildSourceSnapshot(snapshot);
      } finally {
        cleanupNativeBuildSourceSnapshot(snapshot);
      }
    });
  },
);

test(
  "private source snapshot follows an existing symlink before applying later dot-dot components",
  { skip: process.platform === "win32" },
  () => {
    withSourceRepository((repoRoot) => {
      symlinkSync(os.tmpdir(), path.join(repoRoot, "external-hop"));
      symlinkSync(
        "external-hop/../missing",
        path.join(repoRoot, "dotdot-after-hop"),
      );
      assert.throws(
        () =>
          createNativeBuildSourceSnapshot(
            repoRoot,
            path.join(repoRoot, "target", "dotdot-after-hop-test"),
          ),
        /symlink must resolve strictly within/u,
      );
    });
  },
);

test("private source snapshot rejects the repository root as its target", () => {
  withSourceRepository((repoRoot) => {
    assert.throws(
      () => createNativeBuildSourceSnapshot(repoRoot, repoRoot),
      /must not be the repository root/u,
    );
  });
});

test("private source snapshot verification rejects extra transient filesystem entries", () => {
  withSourceRepository((repoRoot) => {
    const snapshot = createNativeBuildSourceSnapshot(
      repoRoot,
      path.join(repoRoot, "target", "inventory-test"),
    );
    try {
      chmodSync(snapshot.snapshotRoot, 0o700);
      writeFileSync(path.join(snapshot.snapshotRoot, "unexpected.txt"), "extra\n");
      chmodSync(snapshot.snapshotRoot, 0o500);
      assert.throws(
        () => verifyNativeBuildSourceSnapshot(snapshot),
        /filesystem inventory changed/u,
      );
    } finally {
      cleanupNativeBuildSourceSnapshot(snapshot);
    }
  });
});

test(
  "private source snapshot rejects a target with symbolic-link components",
  { skip: process.platform === "win32" },
  () => {
    withSourceRepository((repoRoot) => {
      const externalTarget = mkdtempSync(
        path.join(os.tmpdir(), "iroha-js-snapshot-target-"),
      );
      const linkedTarget = path.join(repoRoot, "target");
      symlinkSync(externalTarget, linkedTarget);
      try {
        assert.throws(
          () => createNativeBuildSourceSnapshot(repoRoot, linkedTarget),
          /must not contain symbolic-link components/u,
        );
      } finally {
        unlinkSync(linkedTarget);
        rmSync(externalTarget, { recursive: true, force: true });
      }
    });
  },
);

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

test("native build provenance rejects stale binaries and malformed V1/V2/V3 fields", () => {
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
      { ...provenance, version: 2 },
      { ...provenance, version: 1 },
      { ...provenance, build_execution_policy: "hermetic-build-v1" },
      { ...provenance, build_execution_policy: undefined },
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

test("invalidating provenance fails closed across a byte-identical rebuild", () => {
  withNativeFixture(({ nativePath }) => {
    const oldState = sourceState({ digest: "1".repeat(64) });
    const oldProvenance = createNativeBuildProvenance({
      cargoProfile: "release",
      nativePath,
      sourceBefore: oldState,
      sourceAfter: oldState,
    });
    const provenancePath = writeNativeBuildProvenance(nativePath, oldProvenance);
    assert.equal(invalidateNativeBuildProvenance(nativePath), true);
    assert.equal(existsSync(provenancePath), false);

    // Cargo is allowed to produce exactly the same executable bytes from a
    // different sealed source. Until the new sidecar is durable, the output
    // must not retain the earlier source claim.
    writeFileSync(nativePath, "compiled-native-fixture");
    assert.throws(
      () => readNativeBuildProvenance(nativePath),
      /ENOENT|no such file/u,
    );

    const newState = sourceState({ digest: "2".repeat(64) });
    const newProvenance = createNativeBuildProvenance({
      cargoProfile: "release",
      nativePath,
      sourceBefore: newState,
      sourceAfter: newState,
    });
    writeNativeBuildProvenance(nativePath, newProvenance);
    assert.deepEqual(readNativeBuildProvenance(nativePath), newProvenance);
    assert.notEqual(
      readNativeBuildProvenance(nativePath).source_tree_sha256,
      oldProvenance.source_tree_sha256,
    );
  });
});

test(
  "provenance publication rejects a hostile hardlink without touching its victim",
  { skip: process.platform === "win32" },
  () => {
    withNativeFixture(({ directory, nativePath }) => {
      const victimPath = path.join(directory, "victim.txt");
      const provenancePath = nativeBuildProvenancePath(nativePath);
      writeFileSync(victimPath, "must remain unchanged\n");
      linkSync(victimPath, provenancePath);
      const state = sourceState();
      const provenance = createNativeBuildProvenance({
        cargoProfile: "debug",
        nativePath,
        sourceBefore: state,
        sourceAfter: state,
      });

      assert.throws(
        () => writeNativeBuildProvenance(nativePath, provenance),
        /must be a singly linked regular/u,
      );

      assert.equal(readFileSync(victimPath, "utf8"), "must remain unchanged\n");
      assert.equal(readFileSync(provenancePath, "utf8"), "must remain unchanged\n");
    });
  },
);

test(
  "provenance invalidation rejects a hostile hardlink without touching its victim",
  { skip: process.platform === "win32" },
  () => {
    withNativeFixture(({ directory, nativePath }) => {
      const victimPath = path.join(directory, "victim.txt");
      const provenancePath = nativeBuildProvenancePath(nativePath);
      writeFileSync(victimPath, "must remain unchanged\n");
      linkSync(victimPath, provenancePath);

      assert.throws(
        () => invalidateNativeBuildProvenance(nativePath),
        /must be a singly linked regular/u,
      );
      assert.equal(readFileSync(victimPath, "utf8"), "must remain unchanged\n");
      assert.equal(readFileSync(provenancePath, "utf8"), "must remain unchanged\n");
    });
  },
);

test(
  "stable native and provenance reads reject symbolic links and hardlinks",
  { skip: process.platform === "win32" },
  () => {
    withNativeFixture(({ directory, nativePath }) => {
      const linkedNative = path.join(directory, "linked-native.so");
      linkSync(nativePath, linkedNative);
      assert.throws(
        () => readStableRegularFile(nativePath, { label: "native fixture" }),
        /singly linked regular file/u,
      );
      unlinkSync(linkedNative);

      const symlinkedNative = path.join(directory, "symlinked-native.so");
      symlinkSync(nativePath, symlinkedNative);
      assert.throws(
        () => readStableRegularFile(symlinkedNative, { label: "native fixture" }),
        /singly linked regular file/u,
      );

      const state = sourceState();
      const provenance = createNativeBuildProvenance({
        cargoProfile: "debug",
        nativePath,
        sourceBefore: state,
        sourceAfter: state,
      });
      const provenancePath = writeNativeBuildProvenance(nativePath, provenance);
      const linkedProvenance = path.join(directory, "linked-provenance.json");
      linkSync(provenancePath, linkedProvenance);
      assert.throws(
        () => readNativeBuildProvenance(nativePath),
        /singly linked regular file/u,
      );
      unlinkSync(linkedProvenance);
      assert.deepEqual(readNativeBuildProvenance(nativePath), provenance);
    });
  },
);
