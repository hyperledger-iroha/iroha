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
    /payload hash: f5bc4a4cc1b8df1125f847255995cc8d76f66c0045a0ea875df5b30dda16f14b/u,
  );
  assert.match(
    result.stdout,
    /signed transaction hash: b410d55b960d396c1034221dea22464d08de1237363b02cb1f7c35d4c6eaf0a1/u,
  );
  assert.match(result.stdout, /final status: Applied/u);
});
