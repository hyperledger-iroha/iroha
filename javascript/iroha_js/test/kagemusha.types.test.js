// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import test from "node:test";

test("KAGEMUSHA declarations expose only request, payment, and acknowledgement", () => {
  const result = spawnSync(process.execPath, [
    "./node_modules/typescript/bin/tsc", "--noEmit", "--strict", "--skipLibCheck",
    "--module", "NodeNext", "--moduleResolution", "NodeNext", "--target", "ES2022",
    "--types", "node", "./fixtures/typescript/kagemusha.types.ts",
  ], { cwd: fileURLToPath(new URL("..", import.meta.url)), encoding: "utf8" });
  assert.equal(result.status, 0, [result.stdout, result.stderr].filter(Boolean).join("\n"));
});
