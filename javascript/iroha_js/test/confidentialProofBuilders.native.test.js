import assert from "node:assert/strict";
import test from "node:test";
import { makeNativeTest, nativeBinding } from "./helpers/native.js";

const methods = ["buildConfidentialTransferProofV2", "buildConfidentialUnshieldProofV2", "buildConfidentialUnshieldProofV3"];
const nativeTest = makeNativeTest(test, { require: methods });

nativeTest("packaged confidential proof exports reject invalid network before proving", () => {
  // These are direct addon calls, not mocked exports or self-referential source
  // assertions. Rejection confirms dispatch, not successful proof production.
  const common = [new Uint8Array(31), "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", Buffer.alloc(32, 0x42), [], []];
  const key = ["11".repeat(32), "halo2/ipa", "invalid-fixture", new Uint8Array([1])];
  for (const [name, tail] of [
    [methods[0], [[], ...key]],
    [methods[1], ["7", ...key]],
    [methods[2], [[], "7", ...key]],
  ]) {
    assert.throws(() => nativeBinding[name](...common, ...tail), /network/i);
  }
});

const walletTest = makeNativeTest(test, { require: ["proveConfidentialTransfer", "proveConfidentialRedemption"] });
walletTest("canonical wallet native methods reject invalid network before proof work", () => {
  const common = [new Uint8Array(31), "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", Buffer.alloc(32, 0x42), [], []];
  assert.throws(() => nativeBinding.proveConfidentialTransfer(...common, [], "11".repeat(32)), /network/i);
  assert.throws(() => nativeBinding.proveConfidentialRedemption(...common, "7", "11".repeat(32), undefined), /network/i);
});
