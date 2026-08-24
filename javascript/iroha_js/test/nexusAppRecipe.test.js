// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const PACKAGE_ROOT = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);

test("Nexus App transfer recipe executes the canonical browser codec end to end", () => {
  const result = spawnSync(
    process.execPath,
    [path.join(PACKAGE_ROOT, "recipes/nexus_app_transfer.mjs")],
    {
      cwd: PACKAGE_ROOT,
      encoding: "utf8",
      timeout: 15_000,
      env: { ...process.env, NO_COLOR: "1" },
    },
  );
  assert.equal(
    result.status,
    0,
    `recipe failed\nstdout:\n${result.stdout}\nstderr:\n${result.stderr}`,
  );
  assert.match(
    result.stdout,
    /payload hash: 2b1553daadf14385d797279fe662b01812e4bf37b7d62df8144a2f0bd60b6297/u,
  );
  assert.match(
    result.stdout,
    /signed transaction hash: d338123041fd61a734f21577b92cbe4b2c177541983ddc96e9e63f9fd878bde9/u,
  );
  assert.match(result.stdout, /final status: Applied/u);
});
