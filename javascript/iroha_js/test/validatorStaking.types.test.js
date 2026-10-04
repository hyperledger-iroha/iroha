import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";
import * as root from "../src/index.js";
import * as norito from "../src/public/norito.js";

test("staking codecs are exported by the root and Norito entrypoints", () => {
  assert.equal(typeof root.encodeValidatorStakingValueV1, "function");
  assert.equal(typeof root.decodeValidatorStakingValueV1, "function");
  assert.equal(root.encodeValidatorStakingValueV1, norito.encodeValidatorStakingValueV1);
  assert.equal(root.decodeValidatorStakingValueV1, norito.decodeValidatorStakingValueV1);
  for (const name of ["encodeValidatorStakingPreparationFrameV1", "decodeValidatorStakingPreparationFrameV1", "validateValidatorStakingPreparationV1"]) {
    assert.equal(typeof root[name], "function", name);
    assert.equal(root[name], norito[name], name);
  }
  assert.equal(typeof root.ToriiClient.prototype.preparePublicLanePlan, "function");
});

test("staking declarations require typed operation bindings and explicit fee claims", () => {
  const result = spawnSync(process.execPath, [
    fileURLToPath(new URL("../node_modules/typescript/bin/tsc", import.meta.url)),
    "--noEmit", "--strict", "--skipLibCheck", "--module", "NodeNext",
    "--moduleResolution", "NodeNext", "--target", "ES2022", "--types", "node",
    fileURLToPath(new URL("./fixtures/typescript/validatorStaking.types.ts", import.meta.url)),
  ], { encoding: "utf8" });
  assert.equal(result.status, 0, `tsc failed:\n${result.stdout}\n${result.stderr}`);
});
