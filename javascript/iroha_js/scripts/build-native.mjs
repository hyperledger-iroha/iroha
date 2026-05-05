#!/usr/bin/env node
/**
 * Build the native `iroha_js_host` library.
 */
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(scriptDir, "..", "..", "..");
const cargoManifest = join(repoRoot, "Cargo.toml");

const buildArgs = ["build", "--manifest-path", cargoManifest, "-p", "iroha_js_host"];

function runCargo(args) {
  const result = spawnSync("cargo", args, {
    cwd: repoRoot,
    stdio: "inherit",
    env: process.env,
  });
  return result;
}

const build = runCargo(buildArgs);
if (build.status !== 0) {
  process.exit(build.status ?? 1);
}
