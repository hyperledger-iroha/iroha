#!/usr/bin/env node
// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
// Fixed local package/runtime check before pack, publish or native execution.
// Reads this physical script's sibling contract and parent package/lock only;
// accepts no environment policy, arguments or runtime-version override.
import assert from "node:assert/strict";
import { readFileSync, realpathSync } from "node:fs";
import { fileURLToPath, pathToFileURL } from "node:url";

try {
  assert.equal(process.argv.length, 2, "Node engine guard takes no options");
  const moduleUrl = pathToFileURL(realpathSync(fileURLToPath(import.meta.url)));
  const { validateNodeEngineContract } = await import(new URL("./node-engine-contract.mjs", moduleUrl));
  const pkg = JSON.parse(readFileSync(new URL("../package.json", moduleUrl), "utf8"));
  const lock = JSON.parse(readFileSync(new URL("../package-lock.json", moduleUrl), "utf8"));
  const result = validateNodeEngineContract(pkg, lock, process.versions.node);
  console.log(`Node >=${result.minimum} covers ${result.productionDependencies} locked production dependencies; current ${process.versions.node}`);
} catch (error) {
  console.error(`[node-engine] ${error.message}`);
  process.exitCode = 1;
}
