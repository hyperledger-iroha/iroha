import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

const PACKAGE_ROOT = fileURLToPath(new URL("..", import.meta.url));

test("SCCP imports without a Node Buffer global", () => {
  const script = [
    "delete globalThis.Buffer;",
    'const module = await import("./src/sccp.js");',
    "if (module.SCCP_SOLANA_TESTNET_GENESIS_HASH !==",
    '  "4uhcVJyU9pJkvQyS88uRDiswHXSCkY3zQawwpjk2NsNY") {',
    '  throw new Error("unexpected Solana testnet genesis profile");',
    "}",
  ].join("\n");
  const child = spawnSync(
    process.execPath,
    ["--input-type=module", "--eval", script],
    { cwd: PACKAGE_ROOT, encoding: "utf8" },
  );

  assert.equal(
    child.status,
    0,
    `browser-like SCCP import failed\nstdout:\n${child.stdout}\nstderr:\n${child.stderr}`,
  );
});
