import assert from "node:assert/strict";
import test from "node:test";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { createConfidentialProverClass } from "../src/confidentialProofBuilders.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import { NetworkId } from "../src/networkId.js";
const hex = "11".repeat(32);
const input = { amount: "7", rhoHex: hex, diversifierHex: hex, leafIndex: 0 };
const output = { amount: "7", rhoHex: hex, ownerTagHex: hex };
const request = { treeCommitments: [hex], inputs: [input], rootHex: hex };
function fixture() {
  const calls = [];
  const capture = (kind, args, count) => {
    calls.push([kind, args]);
    return { nullifiers: args[4].map(() => Buffer.alloc(32, 1)), outputCommitments: Array.from({ length: count }, () => Buffer.alloc(32, 2)), root: Buffer.from(hex, "hex"), proof: Buffer.from([3]) };
  };
  const Prover = createConfidentialProverClass(createNativeRuntime({
    proveConfidentialTransfer: (...args) => capture("transfer", args, args[5].length),
    proveConfidentialRedemption: (...args) => capture("redemption", args, args[7] === undefined ? 0 : 1),
  }));
  return { calls, prover: new Prover({ networkId: NetworkId.fromBytes(Buffer.alloc(32, 0x13)), assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", spendKey: Buffer.alloc(32, 0x42) }) };
}
test("confidential shape limits reject before native dispatch or array entry reads", async () => {
  const { prover, calls } = fixture();
  const oversized = new Array(3);
  Object.defineProperty(oversized, 0, { get() { throw new Error("unexpected entry read"); } });
  try {
    for (const prove of [patch => prover.proveTransfer({ ...request, outputs: [output], ...patch }), patch => prover.proveRedemption({ ...request, publicAmount: 7, ...patch })]) {
      for (const inputs of [[], oversized]) await assert.rejects(() => prove({ inputs }), /inputs must contain between 1 and 2/u);
      await assert.rejects(() => prove({ inputs: new Array(1) }), /inputs\[0\] must be an object/u);
      await assert.rejects(() => prove({ inputs: [{ ...input, leafIndex: 65536 }] }), /tree capacity/u);
      await assert.rejects(() => prove({ treeCommitments: new Array(65537) }), /treeCommitments must contain between 1 and 65536/u);
    }
    for (const outputs of [[], oversized]) await assert.rejects(() => prover.proveTransfer({ ...request, outputs }), /outputs must contain between 1 and 2/u);
    await assert.rejects(() => prover.proveRedemption({ ...request, publicAmount: 5, change: [output, output] }), /change amount/u);
    assert.equal(calls.length, 0);
  } finally { prover.dispose(); }
});
test("one actual input at full capacity forwards without dummy notes and two-note shapes remain exact", async () => {
  const { prover, calls } = fixture();
  // Synthetic public leaves exercise forwarding, not cryptographic validity.
  const fullTree = { ...request, treeCommitments: Array(65536).fill(hex), inputs: [{ ...input, leafIndex: 65535 }] };
  try {
    await prover.proveTransfer({ ...fullTree, outputs: [output] });
    await prover.proveRedemption({ ...fullTree, publicAmount: 7 });
    await prover.proveRedemption({ ...fullTree, publicAmount: 6, change: { amount: 1, rhoHex: hex } });
    for (const [, args] of calls) {
      assert.equal(args[3].length, 65536);
      assert.deepEqual(args[4], [{ ...input, leafIndex: 65535 }]);
    }
    assert.equal(calls[1][1][7], undefined);
    assert.deepEqual(calls[2][1][7], { amount: "1", rhoHex: hex });
    await prover.proveTransfer({ ...request, treeCommitments: [hex, hex], inputs: [input, { ...input, leafIndex: 1 }], outputs: [output, output] });
    assert.equal(calls[3][1][4].length, 2);
    assert.equal(calls[3][1][5].length, 2);
  } finally { prover.dispose(); }
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
