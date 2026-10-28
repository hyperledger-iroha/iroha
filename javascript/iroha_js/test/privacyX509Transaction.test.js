import assert from "node:assert/strict";
import test from "node:test";
import {
  PrivacyX509DirectTransactionV1,
  preparePrivacyX509DirectTransactionV1,
  buildPrivacyX509DirectTransactionV1,
} from "../src/privacyX509Transaction.js";
import {
  PrivacyExact12CapabilityManifestV1,
  privacyExact12AuthenticatedConstructionArchiveV1,
} from "../src/privacyCapabilities.js";
import { bindPrivacyExact12CapabilityAdmissionV1 } from "../src/privacyCapabilityAdmission.js";
import { NetworkId } from "../src/networkId.js";

const networkId = NetworkId.fromBytes(Buffer.alloc(32, 1));
function input(manifest = {}) {
  return { manifest, networkId, requestNorito: Buffer.of(1), authority: "not-an-admitted-account",
    feePayment: { payer: "authority", chargeLimits: [] }, creationTimeMs: 17, ttlMs: 600_000 };
}

test("X509 public preparation rejects arbitrary capability objects and caller admission callbacks", () => {
  const manifest = Object.freeze({ canonicalBytes: () => Buffer.of(1), network_id: networkId });
  let called = false;
  bindPrivacyExact12CapabilityAdmissionV1(manifest, () => { called = true; return { readiness: "available" }; });
  assert.throws(() => preparePrivacyX509DirectTransactionV1(input(manifest)), /authenticated Torii-origin/);
  assert.equal(called, false, "a caller-installed callback never authorizes this producer");
  assert.throws(() => privacyExact12AuthenticatedConstructionArchiveV1(manifest, networkId), /authenticated Torii-origin/);
});

test("X509 public preparation rejects prototype-only manifests and all alternate action fields", () => {
  const shell = Object.create(PrivacyExact12CapabilityManifestV1.prototype);
  assert.throws(() => preparePrivacyX509DirectTransactionV1(input(shell)), /authenticated Torii-origin/);
  for (const field of ["instructions", "attachments", "actionStatementDigest", "unsignedPayload", "nativeBinding"]) {
    assert.throws(() => preparePrivacyX509DirectTransactionV1({ ...input(), [field]: Buffer.of(1) }), /unsupported X509 input field/);
  }
  for (const invalid of [null, [], 1]) assert.throws(() => preparePrivacyX509DirectTransactionV1(invalid), /must be an object/);
});

test("X509 prepared owners have no public constructor or transferable receiver", () => {
  assert.throws(() => new PrivacyX509DirectTransactionV1(), /no public constructor/);
  const shell = Object.create(PrivacyX509DirectTransactionV1.prototype);
  for (const property of ["materializedStatementNorito", "orderedIntentDigest", "actionStatementNorito", "actionStatementDigest"]) {
    assert.throws(() => shell[property], /invalid X509 prepared/);
  }
  assert.throws(() => shell.sign(Buffer.of(1), Buffer.alloc(32, 1)), /invalid X509 prepared/);
});

test("X509 one-call construction refuses before invoking a holder prover without authenticated admission", async () => {
  let calls = 0;
  await assert.rejects(buildPrivacyX509DirectTransactionV1(input(), {
    prove: () => { calls += 1; return Buffer.of(1); }, privateKey: Buffer.alloc(32, 1),
  }), /authenticated Torii-origin/);
  assert.equal(calls, 0);
  await assert.rejects(buildPrivacyX509DirectTransactionV1(input(), {}), /holder prover callback/);
});


test("X509 preparation requires exact caller-owned creation time and positive TTL", () => {
  for (const ttlMs of [undefined, null, 0, -1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => preparePrivacyX509DirectTransactionV1({ ...input(), ttlMs }), /ttlMs must be an exact positive safe integer/);
  }
  for (const creationTimeMs of [undefined, null, -1, 1.5, NaN, Infinity, Number.MAX_SAFE_INTEGER + 1]) {
    assert.throws(() => preparePrivacyX509DirectTransactionV1({ ...input(), creationTimeMs }), /creationTimeMs must be an exact nonnegative safe integer/);
  }
  assert.throws(() => preparePrivacyX509DirectTransactionV1({ ...input(), creationTimeMs: 0, ttlMs: 1 }), /authenticated Torii-origin/);
});
