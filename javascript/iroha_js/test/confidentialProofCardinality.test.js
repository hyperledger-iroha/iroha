import assert from "node:assert/strict";
import test from "node:test";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createConfidentialProofBuilders } from "../src/confidentialProofBuilders.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import { NetworkId } from "../src/networkId.js";

const hex = "11".repeat(32);
const input = { amount: "7", rhoHex: hex, diversifierHex: hex, leafIndex: 0 };
const output = { amount: "7", rhoHex: hex, ownerTagHex: hex };
const backend = "halo2/ipa";
const request = {
  networkId: NetworkId.fromBytes(Buffer.alloc(32, 0x13)),
  assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
  spendKey: Buffer.alloc(32, 0x42),
  treeCommitments: [hex], inputs: [input], outputs: [output],
  publicAmount: "7", rootHintHex: hex,
  verifyingKey: {
    id: { backend },
    record: { circuit_id: "fixture-only", backend, inline_key: { backend, bytes_b64: "AQID" } },
  },
};
const names = ["buildConfidentialTransferProofV2", "buildConfidentialUnshieldProofV2", "buildConfidentialUnshieldProofV3"];

test("confidential shape limits reject before native dispatch or array entry reads", () => {
  let called = false;
  const api = createConfidentialProofBuilders(createNativeRuntime(Object.fromEntries(
    names.map((name) => [name, () => { called = true; throw new Error("unexpected dispatch"); }]),
  )));
  const oversized = new Array(3);
  Object.defineProperty(oversized, 0, { get() { throw new Error("unexpected entry read"); } });
  for (const name of names) {
    for (const inputs of [[], oversized]) {
      assert.throws(() => api[name]({ ...request, inputs }), /inputs must contain between 1 and 2/u);
    }
    assert.throws(() => api[name]({ ...request, inputs: new Array(1) }), /inputs\[0\] must be an object/u);
    assert.throws(() => api[name]({ ...request, inputs: [{ ...input, leafIndex: 65536 }] }), /tree capacity/u);
    assert.throws(() => api[name]({ ...request, treeCommitments: new Array(65537) }), /treeCommitments must contain between 0 and 65536/u);
  }
  for (const outputs of [[], oversized]) {
    assert.throws(() => api.buildConfidentialTransferProofV2({ ...request, outputs }), /outputs must contain between 1 and 2/u);
  }
  assert.throws(() => api.buildConfidentialUnshieldProofV3({ ...request, outputs: [output, output] }), /outputs must contain between 0 and 1/u);
  assert.equal(called, false);
});

test("single actual input at full tree capacity forwards without caller dummy notes", () => {
  // This checks argument forwarding only; these synthetic commitments are not
  // a cryptographically valid tree and the injected binding does not prove.
  const calls = [];
  const api = createConfidentialProofBuilders(createNativeRuntime(Object.fromEntries(
    names.map((name) => [name, (...args) => {
      calls.push([name, args]);
      return {
        nullifiers: [Buffer.alloc(32, 1)], root: Buffer.alloc(32, 2), proof: Buffer.from([3]),
        ...(name === "buildConfidentialUnshieldProofV2" ? {} : { outputCommitments: [] }),
      };
    }]),
  )));
  const fullTree = { ...request, treeCommitments: Array(65536).fill(hex), inputs: [{ ...input, leafIndex: 65535 }] };
  api.buildConfidentialTransferProofV2(fullTree);
  api.buildConfidentialUnshieldProofV2(fullTree);
  api.buildConfidentialUnshieldProofV3({ ...fullTree, outputs: undefined });
  for (const [, args] of calls) {
    assert.equal(args[3].length, 65536);
    assert.deepEqual(args[4], [{ ...input, leafIndex: 65535 }]);
  }
  assert.deepEqual(calls[2][1][5], []);
  api.buildConfidentialTransferProofV2({ ...request, inputs: [input, { ...input, leafIndex: 1 }], outputs: [output, output] });
  assert.equal(calls[3][1][4].length, 2);
  assert.equal(calls[3][1][5].length, 2);
});

test("confidential TypeScript declarations enforce actual input and output counts", () => {
  const result = spawnSync(process.execPath, [
    fileURLToPath(new URL("../node_modules/typescript/bin/tsc", import.meta.url)),
    "--noEmit", "--strict", "--skipLibCheck", "--module", "NodeNext",
    "--moduleResolution", "NodeNext", "--target", "ES2022", "--types", "node",
    fileURLToPath(new URL("./fixtures/typescript/confidentialProofCardinality.types.ts", import.meta.url)),
  ], { encoding: "utf8", cwd: fileURLToPath(new URL("..", import.meta.url)) });
  assert.equal(result.status, 0, `tsc failed:\n${result.stdout}\n${result.stderr}`);
});
