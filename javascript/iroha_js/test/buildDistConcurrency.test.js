import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { cpSync, existsSync, mkdtempSync, readdirSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const TEST_DIR = resolve(fileURLToPath(import.meta.url), "..");
const ROOT = resolve(TEST_DIR, "..");
const SCRIPT = join(ROOT, "scripts", "build-dist.mjs");
const REQUIRED_OUTPUTS = ["address.js", "curveRegistry.js", "toriiClient.js", "kotodamaCompiler/index.js"];

function runBuild(root) {
  return new Promise((resolveRun, rejectRun) => {
    const child = spawn(process.execPath, [SCRIPT], {
      env: { ...process.env, IROHA_JS_BUILD_DIST_ROOT: root },
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
    assert.equal(existsSync(join(fixtureRoot, ".build-dist.lock")), false);
    assert.deepEqual(
      readdirSync(fixtureRoot).filter((entry) => entry.startsWith(".dist-stage-")),
      [],
    );
  } finally {
    rmSync(fixtureRoot, { recursive: true, force: true });
  }
});
