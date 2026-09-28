import assert from "node:assert/strict";
import test from "node:test";
import { makeNativeTest, nativeBinding } from "./helpers/native.js";
import { confidentialChangeToInput, defaultConfidentialDiversifier } from "../src/index.js";

const walletTest = makeNativeTest(test, { require: ["proveConfidentialTransfer", "proveConfidentialRedemption"] });
walletTest("canonical wallet native methods reject invalid network before proof work", () => {
  const common = [new Uint8Array(31), "62Fk4FPcMuLvW5QjDGNF2a4jAmjM", Buffer.alloc(32, 0x42), [], []];
  assert.throws(() => nativeBinding.proveConfidentialTransfer(...common, [], "11".repeat(32)), /network/i);
  assert.throws(() => nativeBinding.proveConfidentialRedemption(...common, "7", "11".repeat(32), undefined), /network/i);
});

walletTest("packaged addon exposes no caller-selected confidential proof routes", () => {
  for (const suffix of ["TransferProofV2", "UnshieldProofV2", "UnshieldProofV3"]) {
    assert.equal(nativeBinding[`buildConfidential${suffix}`], undefined);
  }
});

const rootTest = makeNativeTest(test, { require: ["computeConfidentialRoot"] });
rootTest("native root helper computes empty and populated histories and rejects oversized input", async () => {
  const empty = await nativeBinding.computeConfidentialRoot([]);
  const populated = await nativeBinding.computeConfidentialRoot(["01".repeat(32)]);
  assert.equal(empty.length, 32);
  assert.equal(populated.length, 32);
  assert.notDeepEqual(empty, populated);
  assert.deepEqual(await nativeBinding.computeConfidentialRoot(["01".repeat(32)]), populated);
  assert.throws(() => nativeBinding.computeConfidentialRoot(new Array(65537).fill("")), /capacity/u);
  assert.throws(() => nativeBinding.computeConfidentialRoot(["FF".repeat(32)]), /lowercase hex/u);
});

const changeTest = makeNativeTest(test, { require: ["defaultConfidentialDiversifier", "deriveConfidentialOwnerTagV2"] });
changeTest("native change helper uses the same default and owner derivation as Core", () => {
  const key = Buffer.alloc(32, 0x5b);
  try {
    const diversifier = nativeBinding.defaultConfidentialDiversifier();
    assert.equal(diversifier.length, 32);
    const explicit = nativeBinding.deriveConfidentialOwnerTagV2(key, diversifier.toString("hex"));
    const input = confidentialChangeToInput({ amount: 7n, rhoHex: "5c".repeat(32) }, 65535);
    assert.deepEqual(Buffer.from(input.diversifierHex, "hex"), diversifier);
    assert.deepEqual(nativeBinding.deriveConfidentialOwnerTagV2(key, input.diversifierHex), explicit);
    assert.deepEqual(defaultConfidentialDiversifier(), diversifier);
    diversifier.fill(0);
    assert.notDeepEqual(nativeBinding.defaultConfidentialDiversifier(), diversifier);
  } finally { key.fill(0); }
});
