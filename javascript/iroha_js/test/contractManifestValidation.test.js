import assert from "node:assert/strict";
import test from "node:test";
import { buildRegisterSmartContractCodeInstruction } from "../src/instructionBuilders.js";
import { canonicalHashLiteral } from "../src/instructionBuilderPrimitives.js";

const codeHash = "11".repeat(32);
const artifactId = { dataspaceId: "18446744073709551615", codeHash };
const unit = { returnType: "()", returnSchema: { nodes: [{ kind: "Unit", value: null }] } };
const view = { name: "read", kind: "View", ...unit };
const call = { name: "write", kind: "Kotoage", permission: "Write", ...unit };
const trigger = {
  id: "update",
  repeats: { Indefinitely: null },
  filter: "AQ==",
  callback: { entrypoint: "write" },
};

function build(manifest) {
  return buildRegisterSmartContractCodeInstruction({ artifactId, manifest: { codeHash, ...manifest } });
}

for (const [name, manifest, error] of [
  ["unknown fields", { contractName: "Retired" }, /unsupported fields: contractName/u],
  ["conflicting aliases", { seiyakuName: "Ledger", seiyaku_name: "Other" }, /conflicting aliases/u],
  ["null aliases", { seiyakuName: "Ledger", seiyaku_name: null }, /conflicting aliases/u],
  ["unmarked code hash", { codeHash: "aa".repeat(32) }, /Hash marker bit/u],
  ["unmarked ABI hash", { abiHash: "aa".repeat(32) }, /Hash marker bit/u],
  ["wrong lifecycle kind", { entrypoints: [{ name: "hajimari", kind: "Kotoage", permission: "Init", ...unit }] }, /branded lifecycle selector/u],
  ["wrong lifecycle name", { entrypoints: [{ name: "setup", kind: "Hajimari", ...unit }] }, /branded lifecycle selector/u],
  ["lifecycle permission", { entrypoints: [{ name: "kaizen", kind: "Kaizen", permission: "Upgrade", ...unit }] }, /permission must be null/u],
  ["missing call permission", { entrypoints: [{ ...call, permission: null }] }, /permission is required/u],
  ["reserved selector", { entrypoints: [{ ...view, name: "match" }] }, /canonical Kotodama V1 identifier/u],
  ["duplicate names", { entrypoints: [view, view] }, /duplicate name read/u],
  ["duplicate lifecycle", { entrypoints: [{ name: "hajimari", kind: "Hajimari", ...unit }, { name: "始まり", kind: "Hajimari", ...unit }] }, /duplicate Hajimari/u],
  ["duplicate triggers", { entrypoints: [{ ...call, triggers: [trigger, trigger] }] }, /duplicate trigger update/u],
  ["undeclared callback", { entrypoints: [{ ...call, triggers: [{ ...trigger, callback: { entrypoint: "absent" } }] }] }, /undeclared local entrypoint/u],
  ["view callback", { entrypoints: [view, { ...call, triggers: [{ ...trigger, callback: { entrypoint: "read" } }] }] }, /local callback must target kotoage/u],
  ["missing hint reason", { entrypoints: [{ ...view, accessHintsComplete: false }] }, /incomplete without a reason/u],
  ["inconsistent complete hints", { entrypoints: [{ ...view, accessHintsComplete: true, accessHintsSkipped: ["dynamic"] }] }, /complete but records skipped reasons/u],
  ["duplicate states", { states: [{ name: "balance", typeName: "quantity" }, { name: "balance", typeName: "quantity" }] }, /duplicate state name balance/u],
  ["duplicate message IDs", { kotoba: [{ msgId: "title", translations: [] }, { msgId: "title", translations: [] }] }, /duplicate msg_id title/u],
  ["duplicate languages", { kotoba: [{ msgId: "title", translations: [{ lang: "en", text: "Ledger" }, { lang: "en", text: "Other" }] }] }, /duplicate language en/u],
]) {
  test(`manifest instruction rejects ${name} before native encoding`, () => {
    assert.throws(() => build(manifest), error);
  });
}

test("manifest instruction retains valid lifecycle, callback, hints and full-width scope", () => {
  const manifest = {
    entrypoints: [
      { name: "hajimari", kind: "Hajimari", ...unit },
      { name: "改善", kind: "Kaizen", ...unit },
      view,
      { ...call, triggers: [trigger], accessHintsComplete: false, accessHintsSkipped: ["dynamic"] },
    ],
    states: [{ name: "balance", typeName: "quantity" }],
    kotoba: [{ msgId: "title", translations: [{ lang: "en", text: "Ledger" }] }],
  };
  const original = structuredClone(manifest);
  const result = build(manifest).RegisterSmartContractCode;
  assert.equal(result.artifact_id.dataspace_id, artifactId.dataspaceId);
  assert.equal(result.manifest.entrypoints[3].triggers[0].callback.entrypoint, "write");
  assert.deepEqual(manifest, original);
  assert.equal(build({ abiHash: codeHash }).RegisterSmartContractCode.manifest.abi_hash,
    canonicalHashLiteral(Buffer.from(codeHash, "hex")));
  assert.equal(build({ abiHash: canonicalHashLiteral(Buffer.from(codeHash, "hex")) }).RegisterSmartContractCode.manifest.abi_hash,
    canonicalHashLiteral(Buffer.from(codeHash, "hex")));
});
