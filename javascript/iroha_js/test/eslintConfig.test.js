// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { ESLint } from "eslint";

const cwd = fileURLToPath(new URL("..", import.meta.url));

test("source lint rejects unknown names and unused bindings", async () => {
  const lint = new ESLint({ cwd });
  const [result] = await lint.lintText("const unused = 1; missingDependency();", {
    filePath: "src/dependencyConfigProbe.js",
  });
  assert.deepEqual(result.messages.map(({ ruleId }) => ruleId).sort(), [
    "no-undef",
    "no-unused-vars",
  ]);
});

test("source lint retains Node and browser globals and underscore exemptions", async () => {
  const lint = new ESLint({ cwd });
  const [result] = await lint.lintText(
    "const _unused = 1; export const context = [document, process, Buffer];",
    { filePath: "src/dependencyConfigProbe.js" },
  );
  assert.deepEqual(result.messages, []);
});

test("tool lint and generated-directory exclusions retain their existing scope", async () => {
  const lint = new ESLint({ cwd });
  const [result] = await lint.lintText("const unused = 1; missingDependency();", {
    filePath: "scripts/dependencyConfigProbe.mjs",
  });
  assert.deepEqual(result.messages, []);
  for (const directory of ["dist", "native", "target-cargo"]) {
    assert.equal(await lint.isPathIgnored(`${directory}/generated.js`), true);
  }
});
