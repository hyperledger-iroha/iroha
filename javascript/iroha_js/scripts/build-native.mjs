#!/usr/bin/env node
/**
 * Build the native `iroha_js_host` library. Attempts an offline build first
 * to take advantage of local caches, then falls back to an online build when
 * dependencies are missing.
 */
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(scriptDir, "..", "..", "..");
const cargoManifest = join(repoRoot, "Cargo.toml");

const baseArgs = ["build", "--manifest-path", cargoManifest, "-p", "iroha_js_host"];
const skipOffline = process.env.IROHA_JS_SKIP_OFFLINE === "1";

if (process.env.IROHA_JS_DISABLE_NATIVE === "1") {
  console.warn("[iroha-js] IROHA_JS_DISABLE_NATIVE=1; skipping native build.");
  process.exit(0);
}

function runCargo(args) {
  const result = spawnSync("cargo", args, {
    cwd: repoRoot,
    stdio: "inherit",
    env: process.env,
  });
  return result;
}

let build;
if (!skipOffline) {
  build = runCargo(["build", "--offline", "--manifest-path", cargoManifest, "-p", "iroha_js_host"]);
  if (build.status === 0) {
    process.exit(0);
  }
  console.warn(
    "[iroha-js] Offline build failed; retrying without --offline (dependency fetch may be required).",
  );
}

build = runCargo(baseArgs);
if (build.status !== 0) {
  process.exit(build.status ?? 1);
}
