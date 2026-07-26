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
    /payload hash: 2519723601cf2e75576c7f7886e32179eb83f624717552e600108db6e4127f65/u,
  );
  assert.match(
    result.stdout,
    /signed transaction hash: 6f39fd5e193f09f750939f0b089188b9a327a9dda0c8fb3de312c953bf2d93bb/u,
  );
  assert.match(result.stdout, /final status: Applied/u);
});
