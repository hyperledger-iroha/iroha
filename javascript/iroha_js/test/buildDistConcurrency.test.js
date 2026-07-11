import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import {
  appendFileSync,
  cpSync,
  existsSync,
  lstatSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

import {
  acquireDistLock,
  directoryDigest,
  releaseDistLock,
} from "../scripts/build-dist.mjs";

const TEST_DIR = resolve(fileURLToPath(import.meta.url), "..");
const ROOT = resolve(TEST_DIR, "..");
const SCRIPT = join(ROOT, "scripts", "build-dist.mjs");
const REQUIRED_OUTPUTS = [
  "address.js",
  "curveRegistry.js",
  "ivmArtifact.js",
  "toriiClient.js",
  "kotodamaCompiler/index.js",
];

function runBuild(root, env = {}) {
  return new Promise((resolveRun, rejectRun) => {
    const child = spawn(process.execPath, [SCRIPT], {
      env: { ...process.env, IROHA_JS_BUILD_DIST_ROOT: root, ...env },
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stderr = "";
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    child.once("error", rejectRun);
    child.once("exit", (code, signal) => {
      if (code === 0 && signal === null) resolveRun();
      else rejectRun(new Error(`build:dist exited with code=${code} signal=${signal}: ${stderr}`));
    });
  });
}

const delay = (milliseconds) => new Promise((resolveDelay) => setTimeout(resolveDelay, milliseconds));

function assertNoPublicationArtifacts(root) {
  assert.equal(existsSync(join(root, ".build-dist.lock")), false);
  assert.deepEqual(
    readdirSync(root).filter(
      (entry) =>
        entry.startsWith(".dist-stage-") ||
        entry.startsWith(".dist-backup-") ||
        entry.startsWith(".dist-failed-"),
    ),
    [],
  );
}

function markSourceGeneration(root, generation) {
  for (const fileName of REQUIRED_OUTPUTS) {
    appendFileSync(join(root, "src", fileName), `\n// build-dist-generation:${generation}\n`, "utf8");
  }
}

function assertPublishedGeneration(root, generation) {
  for (const fileName of REQUIRED_OUTPUTS) {
    assert.match(
      readFileSync(join(root, "dist", fileName), "utf8"),
      new RegExp(`build-dist-generation:${generation}`, "u"),
      fileName,
    );
  }
}

test("parallel build:dist processes publish one complete distribution", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-"));
  try {
    cpSync(join(ROOT, "src"), join(fixtureRoot, "src"), { recursive: true });
    await Promise.all([runBuild(fixtureRoot), runBuild(fixtureRoot), runBuild(fixtureRoot)]);

    for (const fileName of REQUIRED_OUTPUTS) {
      assert.equal(existsSync(join(fixtureRoot, "dist", fileName)), true, fileName);
    }
    const publishedAt = statSync(join(fixtureRoot, "dist")).mtimeMs;
    await Promise.all([runBuild(fixtureRoot), runBuild(fixtureRoot)]);
    assert.equal(
      statSync(join(fixtureRoot, "dist")).mtimeMs,
      publishedAt,
      "an unchanged distribution must not be replaced while another pack operation may read it",
    );
    assertNoPublicationArtifacts(fixtureRoot);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("a lock-aware reader holds a complete old snapshot until changed source is published", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-reader-"));
  try {
    cpSync(join(ROOT, "src"), join(fixtureRoot, "src"), { recursive: true });
    markSourceGeneration(fixtureRoot, "old");
    await runBuild(fixtureRoot);
    assertPublishedGeneration(fixtureRoot, "old");

    markSourceGeneration(fixtureRoot, "new");
    const readerLock = await acquireDistLock({ root: fixtureRoot });
    let buildSettled = false;
    const changedBuild = runBuild(fixtureRoot).finally(() => {
      buildSettled = true;
    });
    try {
      await delay(175);
      assert.equal(buildSettled, false, "the writer must wait for the active snapshot reader");
      for (let index = 0; index < 100; index += 1) {
        assertPublishedGeneration(fixtureRoot, "old");
      }
    } finally {
      releaseDistLock(readerLock);
    }

    await changedBuild;
    assertPublishedGeneration(fixtureRoot, "new");
    assert.equal(directoryDigest(join(fixtureRoot, "src")), directoryDigest(join(fixtureRoot, "dist")));
    assertNoPublicationArtifacts(fixtureRoot);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("forced publication failures restore the last-good dist and clean transaction artifacts", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-rollback-"));
  try {
    cpSync(join(ROOT, "src"), join(fixtureRoot, "src"), { recursive: true });
    markSourceGeneration(fixtureRoot, "last-good");
    await runBuild(fixtureRoot);
    const lastGoodDigest = directoryDigest(join(fixtureRoot, "dist"));

    for (const failpoint of ["after-backup", "after-publish"]) {
      appendFileSync(
        join(fixtureRoot, "src", "address.js"),
        `\n// rejected-generation:${failpoint}\n`,
        "utf8",
      );
      await assert.rejects(
        runBuild(fixtureRoot, {
          IROHA_JS_BUILD_DIST_TEST_MODE: "1",
          IROHA_JS_BUILD_DIST_TEST_FAILPOINT: failpoint,
        }),
        new RegExp(`injected test failure at ${failpoint}`, "u"),
      );
      assert.equal(directoryDigest(join(fixtureRoot, "dist")), lastGoodDigest);
      assertPublishedGeneration(fixtureRoot, "last-good");
      assertNoPublicationArtifacts(fixtureRoot);
    }

    await runBuild(fixtureRoot, {
      IROHA_JS_BUILD_DIST_TEST_FAILPOINT: "after-backup",
    });
    assert.equal(directoryDigest(join(fixtureRoot, "src")), directoryDigest(join(fixtureRoot, "dist")));
    assertNoPublicationArtifacts(fixtureRoot);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("a stale crashed transaction restores its backup before rebuilding", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-recovery-"));
  try {
    cpSync(join(ROOT, "src"), join(fixtureRoot, "src"), { recursive: true });
    markSourceGeneration(fixtureRoot, "recovered");
    await runBuild(fixtureRoot);
    const expectedDigest = directoryDigest(join(fixtureRoot, "dist"));

    const backup = join(fixtureRoot, ".dist-backup-crashed-process");
    renameSync(join(fixtureRoot, "dist"), backup);
    cpSync(join(fixtureRoot, "src"), join(fixtureRoot, ".dist-stage-crashed-process"), {
      recursive: true,
    });
    const lock = join(fixtureRoot, ".build-dist.lock");
    writeFileSync(
      lock,
      `${JSON.stringify({ pid: 2_147_483_647, token: "crashed", createdAt: "1970-01-01T00:00:00.000Z" })}\n`,
      { encoding: "utf8", mode: 0o600 },
    );
    const old = new Date(Date.now() - 10 * 60_000);
    utimesSync(lock, old, old);

    await runBuild(fixtureRoot);
    assert.equal(directoryDigest(join(fixtureRoot, "dist")), expectedDigest);
    assertPublishedGeneration(fixtureRoot, "recovered");
    assertNoPublicationArtifacts(fixtureRoot);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("a staged symbolic link is rejected without replacing the last-good dist", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-symlink-"));
  try {
    cpSync(join(ROOT, "src"), join(fixtureRoot, "src"), { recursive: true });
    markSourceGeneration(fixtureRoot, "last-good");
    await runBuild(fixtureRoot);
    const lastGoodDigest = directoryDigest(join(fixtureRoot, "dist"));

    symlinkSync("address.js", join(fixtureRoot, "src", "linked-address.js"));
    await assert.rejects(runBuild(fixtureRoot), /cannot publish symbolic link/u);
    assert.equal(directoryDigest(join(fixtureRoot, "dist")), lastGoodDigest);
    assertPublishedGeneration(fixtureRoot, "last-good");
    assertNoPublicationArtifacts(fixtureRoot);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("an invalid existing dist is repaired from the validated source tree", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-repair-"));
  try {
    cpSync(join(ROOT, "src"), join(fixtureRoot, "src"), { recursive: true });
    markSourceGeneration(fixtureRoot, "repaired");
    await runBuild(fixtureRoot);
    symlinkSync("address.js", join(fixtureRoot, "dist", "invalid-link.js"));

    await runBuild(fixtureRoot);
    assert.equal(directoryDigest(join(fixtureRoot, "src")), directoryDigest(join(fixtureRoot, "dist")));
    assertPublishedGeneration(fixtureRoot, "repaired");
    assertNoPublicationArtifacts(fixtureRoot);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("a symlinked crash backup is never promoted to the dist root", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-backup-link-"));
  try {
    cpSync(join(ROOT, "src"), join(fixtureRoot, "src"), { recursive: true });
    markSourceGeneration(fixtureRoot, "real-directory");
    symlinkSync("src", join(fixtureRoot, ".dist-backup-malicious"), "dir");

    await runBuild(fixtureRoot);
    assert.equal(lstatSync(join(fixtureRoot, "dist")).isDirectory(), true);
    assert.equal(directoryDigest(join(fixtureRoot, "src")), directoryDigest(join(fixtureRoot, "dist")));
    assertNoPublicationArtifacts(fixtureRoot);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
