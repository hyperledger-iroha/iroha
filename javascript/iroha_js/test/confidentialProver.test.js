import assert from "node:assert/strict";
import test from "node:test";
import { inspect } from "node:util";
import { ConfidentialProverError, createConfidentialProverClass } from "../src/confidentialProofBuilders.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import { NetworkId } from "../src/networkId.js";

const hex = "11".repeat(32);
const input = { amount: "7", rhoHex: hex, diversifierHex: hex, leafIndex: 0 };
const output = { amount: "7", rhoHex: hex, ownerTagHex: hex };
const spend = { inputs: [input], treeCommitments: [hex], rootHex: hex };
const options = () => ({ networkId: NetworkId.fromBytes(Buffer.alloc(32, 1)), assetDefinitionId: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", spendKey: Buffer.alloc(32, 0x42) });

function fixture(fail = false) {
  const calls = [];
  const result = (kind, args, count) => {
    calls.push({ kind, args, keyAtCall: Buffer.from(args[2]) });
    if (fail) throw new Error("synthetic native failure");
    return { proof: Buffer.from([3]), root: Buffer.from(hex, "hex"),
      nullifiers: args[4].map(() => Buffer.alloc(32, 1)),
      outputCommitments: Array.from({ length: count }, () => Buffer.alloc(32, 2)) };
  };
  const Prover = createConfidentialProverClass(createNativeRuntime({
    proveConfidentialTransfer: (...args) => result("transfer", args, args[5].length),
    proveConfidentialRedemption: (...args) => result("redemption", args, args[7] === undefined ? 0 : 1),
  }));
  return { Prover, calls };
}
const code = (expected) => (error) => error instanceof ConfidentialProverError && error.code === expected;

test("wallet transfer and redemption delegate canonical selection without caller circuit keys", () => {
  const { Prover, calls } = fixture();
  const prover = new Prover(options());
  assert.equal(prover.proveTransfer({ ...spend, outputs: [output] }).relation, "confidential-transfer");
  assert.equal(prover.proveRedemption({ ...spend, publicAmount: 7n }).relation, "confidential-redemption");
  assert.equal(prover.proveRedemption({ ...spend, publicAmount: 5n, change: { amount: 2n, rhoHex: hex } }).relation, "confidential-redemption-with-change");
  assert.equal(calls[0].args.length, 7);
  assert.equal(calls[1].args.length, 8);
  assert.equal(calls[1].args[7], undefined);
  assert.deepEqual(calls[2].args[7], { amount: "2", rhoHex: hex });
  for (const call of calls) {
    assert.deepEqual(call.keyAtCall, Buffer.alloc(32, 0x42));
    assert.deepEqual(call.args[2], Buffer.alloc(32));
  }
  prover.dispose();
});

test("wallet rejects conservation, duplicate indices, unsafe numbers and noncanonical change before dispatch", () => {
  const { Prover, calls } = fixture();
  const prover = new Prover(options());
  const invalid = [
    () => prover.proveTransfer({ ...spend, outputs: [{ ...output, amount: "6" }] }),
    () => prover.proveTransfer({ ...spend, inputs: [input, input], outputs: [{ ...output, amount: "14" }] }),
    () => prover.proveTransfer({ ...spend, inputs: [{ ...input, leafIndex: 1 }], outputs: [output] }),
    () => prover.proveTransfer({ ...spend, inputs: [{ ...input, amount: 9007199254740992 }], outputs: [output] }),
    () => prover.proveTransfer({ ...spend, outputs: [{ ...output, amount: 9007199254740992 }] }),
    () => prover.proveRedemption({ ...spend, publicAmount: 0n }),
    () => prover.proveRedemption({ ...spend, publicAmount: 8n }),
    () => prover.proveRedemption({ ...spend, publicAmount: 5n }),
    () => prover.proveRedemption({ ...spend, publicAmount: 5n, change: { amount: 3n, rhoHex: hex } }),
    () => prover.proveRedemption({ ...spend, publicAmount: 7n, change: { amount: 1n, rhoHex: hex } }),
    () => prover.proveRedemption({ ...spend, inputs: [{ ...input, amount: 1n << 128n }], publicAmount: 7n }),
  ];
  for (const attempt of invalid) assert.throws(attempt, code("INVALID_INPUT"));
  assert.equal(calls.length, 0);
  prover.dispose();
});

test("wallet key copy is isolated, redacted and FFI scratch clears after native failure", () => {
  const { Prover, calls } = fixture(true);
  const configuration = options();
  const prover = new Prover(configuration);
  configuration.spendKey.fill(0);
  assert.equal(JSON.stringify(prover), "{}");
  assert.equal(inspect(prover), "ConfidentialProver {}");
  assert.throws(() => prover.proveTransfer({ ...spend, outputs: [output] }), code("PROVING_FAILED"));
  assert.deepEqual(calls[0].keyAtCall, Buffer.alloc(32, 0x42));
  assert.deepEqual(calls[0].args[2], Buffer.alloc(32));
  prover.dispose();
  prover.dispose();
  assert.throws(() => prover.proveTransfer({ ...spend, outputs: [output] }), code("DISPOSED"));
  assert.throws(() => prover.proveRedemption({ ...spend, publicAmount: 7 }), code("DISPOSED"));
});

test("unavailable native capability rejects before touching wallet secrets", () => {
  const Prover = createConfidentialProverClass(createNativeRuntime({}));
  // Destructuring reads the reference, but does not enumerate/copy secret bytes.
  const key = new Proxy(new Uint8Array(32), { get() { throw new Error("secret byte read"); } });
  assert.throws(() => new Prover({ ...options(), spendKey: key }), code("NATIVE_UNAVAILABLE"));
});

test("native output root and cardinality are checked before returning a local proof", () => {
  for (const patch of [{ root: Buffer.alloc(32) }, { nullifiers: [] }, { outputCommitments: [] }]) {
    const response = () => ({ proof: Buffer.from([1]), root: Buffer.from(hex, "hex"),
      nullifiers: [Buffer.alloc(32)], outputCommitments: [Buffer.alloc(32)], ...patch });
    const Prover = createConfidentialProverClass(createNativeRuntime({
      proveConfidentialTransfer: response, proveConfidentialRedemption: response,
    }));
    const prover = new Prover(options());
    assert.throws(() => prover.proveTransfer({ ...spend, outputs: [output] }), code("PROVING_FAILED"));
    prover.dispose();
  }
});

test("constructor reports typed failures for absent options and invalid spend keys", () => {
  const { Prover } = fixture();
  for (const configuration of [undefined, null, {}, { ...options(), spendKey: Buffer.alloc(32) }, { ...options(), spendKey: "42".repeat(32) }]) {
    assert.throws(() => new Prover(configuration), code("INVALID_INPUT"));
  }
});
