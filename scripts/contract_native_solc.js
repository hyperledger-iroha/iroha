"use strict";

// Native SCCP compiler adapter for Node test harnesses. Requires Python 3 and
// SCCP_NATIVE_SOLC_PATH pointing to an executable admitted by the pinned corridor.
const assert = require("node:assert/strict");
const path = require("node:path");
const { spawnSync } = require("node:child_process");
const REPO = path.resolve(__dirname, "..");

function compileNativeSolidity(input) {
  assert.equal(typeof input, "string", "standard-json compiler input must be a string");
  const compiler = process.env.SCCP_NATIVE_SOLC_PATH;
  assert(compiler && path.isAbsolute(compiler), "authenticated native compiler path is required");
  const result = spawnSync(process.env.SCCP_CORRIDOR_PYTHON_BIN || "python3", [
    path.join(__dirname, "contract_artifact_corridor.py"), "compile-input",
    "--target", "evm", "--compiler", compiler,
  ], { input, encoding: "utf8", maxBuffer: 128 * 1024 * 1024, cwd: REPO });
  assert.equal(result.status, 0, result.stderr || result.error?.message || "native compiler failed");
  return JSON.parse(result.stdout);
}

module.exports = { compileNativeSolidity };
