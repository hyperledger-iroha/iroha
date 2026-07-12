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
import { fileURLToPath, pathToFileURL } from "node:url";
import test from "node:test";

import {
  acquireDistLock,
  directoryDigest,
  releaseDistLock,
} from "../scripts/build-dist.mjs";

const TEST_DIR = resolve(fileURLToPath(import.meta.url), "..");
const ROOT = resolve(TEST_DIR, "..");
const SCRIPT = join(ROOT, "scripts", "build-dist.mjs");
const SCRIPT_URL = pathToFileURL(SCRIPT).href;
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
        entry.startsWith(".dist-failed-") ||
        entry.startsWith(".build-dist.lock.retired-"),
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

test("release refuses to unlink a replacement lock after ownership is lost", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-release-race-"));
  try {
    const lock = await acquireDistLock({ root: fixtureRoot });
    const lockPath = join(fixtureRoot, ".build-dist.lock");
    const displaced = join(fixtureRoot, ".displaced-owned-lock");
    renameSync(lockPath, displaced);
    const replacement = `${JSON.stringify({
      pid: process.pid,
      token: "replacement-owner",
      createdAt: new Date().toISOString(),
    })}\n`;
    writeFileSync(lockPath, replacement, { flag: "wx", mode: 0o600 });

    assert.throws(() => releaseDistLock(lock), /lost ownership/u);
    assert.equal(readFileSync(lockPath, "utf8"), replacement);
    assert.equal(existsSync(displaced), true);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("lock ownership includes the exact fsynced owner record, not only its inode", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-lock-digest-"));
  try {
    const lock = await acquireDistLock({ root: fixtureRoot });
    const lockPath = join(fixtureRoot, ".build-dist.lock");
    const modified = JSON.parse(readFileSync(lockPath, "utf8"));
    modified.attacker = "same-inode-content-rewrite";
    writeFileSync(lockPath, `${JSON.stringify(modified)}\n`);

    assert.throws(() => releaseDistLock(lock), /lost ownership/u);
    assert.equal(existsSync(lockPath), true);
    assert.equal(JSON.parse(readFileSync(lockPath, "utf8")).attacker, modified.attacker);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("post-close lock replacement reports the ownership race without masking or deletion", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-post-close-race-"));
  try {
    const lockPath = join(fixtureRoot, ".build-dist.lock");
    const displaced = join(fixtureRoot, ".post-close-displaced-lock");
    const replacement = `${JSON.stringify({
      pid: process.pid,
      token: "post-close-replacement",
      createdAt: new Date().toISOString(),
    })}\n`;
    await assert.rejects(
      acquireDistLock({
        root: fixtureRoot,
        onLockCreated() {
          renameSync(lockPath, displaced);
          writeFileSync(lockPath, replacement, { flag: "wx", mode: 0o600 });
        },
      }),
      /lost ownership/u,
    );
    assert.equal(readFileSync(lockPath, "utf8"), replacement);
    assert.equal(existsSync(displaced), true);
    assert.deepEqual(
      readdirSync(fixtureRoot).filter((entry) => entry.startsWith(".build-dist.lock.retired-")),
      [],
    );
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("post-close same-inode mutation cannot become the accepted lock record", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-post-close-rewrite-"));
  try {
    const lockPath = join(fixtureRoot, ".build-dist.lock");
    await assert.rejects(
      acquireDistLock({
        root: fixtureRoot,
        onLockCreated() {
          const modified = JSON.parse(readFileSync(lockPath, "utf8"));
          modified.attacker = "same-inode-post-close-rewrite";
          writeFileSync(lockPath, `${JSON.stringify(modified)}\n`);
        },
      }),
      /lost ownership/u,
    );
    assertNoPublicationArtifacts(fixtureRoot);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("stale malformed locks are quarantined while live stale-looking locks are preserved", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-stale-owner-"));
  try {
    const lockPath = join(fixtureRoot, ".build-dist.lock");
    writeFileSync(lockPath, "not-json\n", { mode: 0o600 });
    const old = new Date(Date.now() - 10 * 60_000);
    utimesSync(lockPath, old, old);
    const recovered = await acquireDistLock({
      root: fixtureRoot,
      staleLockMs: 1,
      timeoutMs: 500,
    });
    releaseDistLock(recovered);
    assertNoPublicationArtifacts(fixtureRoot);

    const live = `${JSON.stringify({
      pid: process.pid,
      token: "live-owner",
      createdAt: "1970-01-01T00:00:00.000Z",
    })}\n`;
    writeFileSync(lockPath, live, { flag: "wx", mode: 0o600 });
    utimesSync(lockPath, old, old);
    await assert.rejects(
      acquireDistLock({ root: fixtureRoot, staleLockMs: 1, timeoutMs: 125 }),
      /timed out waiting/u,
    );
    assert.equal(readFileSync(lockPath, "utf8"), live);
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("stale takeover atomically preserves a lock replaced during its decision window", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-stale-race-"));
  try {
    const lockPath = join(fixtureRoot, ".build-dist.lock");
    const displaced = join(fixtureRoot, ".observed-stale-lock");
    const marker = join(fixtureRoot, ".stale-candidate-observed");
    const resume = join(fixtureRoot, ".resume-stale-candidate");
    writeFileSync(
      lockPath,
      `${JSON.stringify({
        pid: 2_147_483_647,
        token: "dead-owner",
        createdAt: "1970-01-01T00:00:00.000Z",
      })}\n`,
      { mode: 0o600 },
    );
    const old = new Date(Date.now() - 10 * 60_000);
    utimesSync(lockPath, old, old);

    const source = String.raw`
import { existsSync, writeFileSync } from "node:fs";
import { acquireDistLock } from ${JSON.stringify(SCRIPT_URL)};
const blocker = new Int32Array(new SharedArrayBuffer(4));
try {
  await acquireDistLock({
    root: process.env.ROOT,
    staleLockMs: 1,
    timeoutMs: 750,
    onStaleCandidate() {
      writeFileSync(process.env.MARKER, "observed", { flag: "wx" });
      while (!existsSync(process.env.RESUME)) Atomics.wait(blocker, 0, 0, 5);
    },
  });
  throw new Error("stale race worker unexpectedly acquired the replacement lock");
} catch (error) {
  if (!String(error?.message).includes("timed out waiting")) throw error;
}
`;
    const child = spawn(process.execPath, ["--input-type=module", "--eval", source], {
      env: {
        ...process.env,
        MARKER: marker,
        RESUME: resume,
        ROOT: fixtureRoot,
      },
      stdio: ["ignore", "ignore", "pipe"],
    });
    let stderr = "";
    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });
    const started = Date.now();
    while (!existsSync(marker)) {
      if (child.exitCode !== null || child.signalCode !== null) {
        throw new Error(`stale race worker exited before synchronization: ${stderr}`);
      }
      if (Date.now() - started > 5_000) {
        throw new Error("timed out waiting for stale race synchronization marker");
      }
      await delay(10);
    }

    renameSync(lockPath, displaced);
    const replacement = `${JSON.stringify({
      pid: process.pid,
      token: "live-replacement-owner",
      createdAt: new Date().toISOString(),
    })}\n`;
    writeFileSync(lockPath, replacement, { flag: "wx", mode: 0o600 });
    writeFileSync(resume, "resume", { flag: "wx" });
    const exit = await new Promise((resolveExit, rejectExit) => {
      child.once("error", rejectExit);
      child.once("exit", (code, signal) => resolveExit({ code, signal }));
    });
    assert.deepEqual(exit, { code: 0, signal: null }, stderr);
    assert.equal(readFileSync(lockPath, "utf8"), replacement);
    assert.equal(readFileSync(displaced, "utf8").includes("dead-owner"), true);
    assert.deepEqual(
      readdirSync(fixtureRoot).filter((entry) => entry.startsWith(".build-dist.lock.retired-")),
      [],
    );
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});

test("stale takeover preserves a same-inode lock whose lease mtime is refreshed", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-stale-refresh-"));
  try {
    const lockPath = join(fixtureRoot, ".build-dist.lock");
    const staleRecord = `${JSON.stringify({
      pid: 2_147_483_647,
      token: "dead-owner-refreshed-before-takeover",
      createdAt: "1970-01-01T00:00:00.000Z",
    })}\n`;
    writeFileSync(lockPath, staleRecord, { mode: 0o600 });
    const old = new Date(Date.now() - 10 * 60_000);
    utimesSync(lockPath, old, old);

    let refreshed = false;
    await assert.rejects(
      acquireDistLock({
        root: fixtureRoot,
        staleLockMs: 60_000,
        timeoutMs: 125,
        onStaleCandidate() {
          if (refreshed) return;
          refreshed = true;
          const fresh = new Date();
          utimesSync(lockPath, fresh, fresh);
        },
      }),
      /timed out waiting/u,
    );
    assert.equal(refreshed, true);
    assert.equal(readFileSync(lockPath, "utf8"), staleRecord);
    assert.deepEqual(
      readdirSync(fixtureRoot).filter((entry) => entry.startsWith(".build-dist.lock.retired-")),
      [],
    );
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

test("ambiguous valid crash backups are preserved instead of choosing by mtime", async () => {
  const fixtureRoot = mkdtempSync(join(tmpdir(), "iroha-js-build-dist-backup-ambiguity-"));
  try {
    cpSync(join(ROOT, "src"), join(fixtureRoot, "src"), { recursive: true });
    cpSync(join(fixtureRoot, "src"), join(fixtureRoot, ".dist-backup-first"), {
      recursive: true,
    });
    cpSync(join(fixtureRoot, "src"), join(fixtureRoot, ".dist-backup-second"), {
      recursive: true,
    });

    await assert.rejects(
      runBuild(fixtureRoot),
      /multiple valid crash backups.*cannot prove their generation order/u,
    );
    assert.equal(existsSync(join(fixtureRoot, "dist")), false);
    assert.equal(existsSync(join(fixtureRoot, ".dist-backup-first")), true);
    assert.equal(existsSync(join(fixtureRoot, ".dist-backup-second")), true);
    assert.equal(existsSync(join(fixtureRoot, ".build-dist.lock")), false);
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
