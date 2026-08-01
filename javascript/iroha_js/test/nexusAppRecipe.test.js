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
    /payload hash: 64dd716ea5a440e08a204ff2e7c0587050ce6ab4a129bdcbde725c428a12ad97/u,
  );
  assert.match(
    result.stdout,
    /signed transaction hash: da4476563ac64bf8708de3e56dd69a0baf0e2c1f9b87347d7352a6555d23deb5/u,
  );
  assert.match(result.stdout, /final status: Applied/u);
});
