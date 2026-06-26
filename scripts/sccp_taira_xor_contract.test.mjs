import { readFile } from "node:fs/promises";
import { test } from "node:test";
import assert from "node:assert/strict";

import { compileKotodamaProgram } from "../javascript/iroha_js/src/index.js";

const CONTRACT_PATH = new URL("../contracts/taira/sccp/TairaXorSccpBurnRecord.ko", import.meta.url);
const INBOUND_SETTLEMENT_CONTRACT_PATH = new URL(
  "../contracts/taira/sccp/TairaXorSccpInboundSettlement.ko",
  import.meta.url,
);

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
  assert.ok(
    compiled.artifactBytes.some((byte, index, bytes) => byte === 0x00 && bytes[index + 1] === 0x09),
    "compiled artifact should contain a NoritoBytes TLV for execute_instruction",
  );

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
      ["amount", "Amount"],
      ["record_instruction", "bytes"],
    ],
  );
  assert.equal(entrypoint.access_hints_complete, true);
});

test("TAIRA XOR SCCP inbound settlement contract exposes finalize_inbound", async () => {
  const source = await readFile(INBOUND_SETTLEMENT_CONTRACT_PATH, "utf8");
  const compiled = compileKotodamaProgram(source, {
    sourceName: "contracts/taira/sccp/TairaXorSccpInboundSettlement.ko",
  });

  assert.deepEqual(compiled.diagnostics, []);
  assert.ok(compiled.artifactBytes.length > 0);

  const entrypoint = compiled.manifest?.entrypoints.find(
    (candidate) => candidate.name === "finalize_inbound",
  );
  assert.ok(entrypoint, "finalize_inbound entrypoint must be present");
  assert.equal(entrypoint.permission, "AssetManager");
  assert.deepEqual(entrypoint.params, []);
  assert.equal(entrypoint.access_hints_complete, null);
});
