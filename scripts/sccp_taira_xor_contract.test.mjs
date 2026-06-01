import { readFile } from "node:fs/promises";
import { test } from "node:test";
import assert from "node:assert/strict";

import { compileKotodamaProgram } from "../javascript/iroha_js/src/index.js";

const CONTRACT_PATH = new URL("../contracts/taira/sccp/TairaXorSccpBurnRecord.ko", import.meta.url);

test("TAIRA XOR SCCP burn-record contract compiles as IVM ZK proved artifact", async () => {
  const source = await readFile(CONTRACT_PATH, "utf8");
  const compiled = compileKotodamaProgram(source, {
    sourceName: "contracts/taira/sccp/TairaXorSccpBurnRecord.ko",
    forceZk: true,
  });

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(compiled.artifactBytes.length > 0);
  assert.equal(compiled.artifactBytes[6], 1);
  assert.equal(compiled.manifest?.features_bitmap, 1);

  const entrypoint = compiled.manifest?.entrypoints.find(
    (candidate) => candidate.name === "burn_and_record",
  );
  assert.ok(entrypoint, "burn_and_record entrypoint must be present");
  assert.equal(entrypoint.permission, "AssetTransferRole");
  assert.deepEqual(
    entrypoint.params.map((param) => [param.name, param.type_name]),
    [
      ["sender", "AccountId"],
      ["settlement_asset", "AssetDefinitionId"],
      ["amount", "int"],
      ["record_instruction", "Blob"],
    ],
  );
  assert.equal(entrypoint.access_hints_complete, true);
});
