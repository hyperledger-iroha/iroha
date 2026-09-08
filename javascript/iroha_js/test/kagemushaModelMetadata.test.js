// SPDX-License-Identifier: Apache-2.0
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { Kagemusha } from "../src/kagemusha.js";

const fixture = JSON.parse(readFileSync(
  new URL("../../../fixtures/offline/kagemusha_v1.json", import.meta.url), "utf8",
));

for (const [section, decode, encode] of [
  ["commit_certificate", Kagemusha.decodeCommitCertificate, Kagemusha.encodeCommitCertificate],
  ["payment_proof", Kagemusha.decodePaymentProof, Kagemusha.encodePaymentProof],
]) {
  test(`private Kagemusha metadata preserves the Rust ${section} archive`, () => {
    const wire = Buffer.from(fixture[section].norito_hex, "hex");
    const model = decode(wire);
    assert.deepEqual(Buffer.from(encode(model)), wire);
    // The public prototype's constructor property cannot select another wire schema.
    const prototype = Object.getPrototypeOf(model);
    const original = Object.getOwnPropertyDescriptor(prototype, "constructor");
    try {
      Object.defineProperty(prototype, "constructor", { value: Kagemusha.CreditOpening });
      assert.deepEqual(Buffer.from(encode(model)), wire);
    } finally {
      Object.defineProperty(prototype, "constructor", original);
    }
    const detached = model.semanticDigest ?? model.candidateEnvelopeDigest;
    detached.fill(0);
    assert.deepEqual(Buffer.from(encode(model)), wire);
    assert.throws(() => encode(Object.create(prototype)), TypeError);
  });
}

test("model-owned alignment and values survive prototype constructor changes", () => {
  const opening = new Kagemusha.CreditOpening({
    version: 1, creditId: new Uint8Array(32).fill(1), amount: 2n,
    creditCommitmentOpening: new Uint8Array(32).fill(3),
    recipientBindingOpening: new Uint8Array(32).fill(4), recoveryNonce: new Uint8Array(32).fill(5),
  });
  const wire = Kagemusha.encodeCreditOpening(opening);
  assert.equal(wire.length, 200);
  assert.deepEqual(Kagemusha.encodeCreditOpening(Kagemusha.decodeCreditOpening(wire)), wire);
  const prototype = Object.getPrototypeOf(opening);
  const original = Object.getOwnPropertyDescriptor(prototype, "constructor");
  try {
    Object.defineProperty(prototype, "constructor", { value: Kagemusha.DeviceMintStageResult });
    assert.deepEqual(Kagemusha.encodeCreditOpening(opening), wire);
  } finally {
    Object.defineProperty(prototype, "constructor", original);
  }
  assert.throws(() => new Kagemusha.DeviceMintStageResult({
    version: 1, disposition: 2, creditId: new Uint8Array(32).fill(1),
  }), { name: "TypeError", message: "KAGEMUSHA V1 mint-stage disposition must be 0 or 1" });
});
