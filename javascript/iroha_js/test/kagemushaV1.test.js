// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

import { NetworkId } from "../src/networkId.js";
import { KagemushaV1 } from "../src/kagemushaV1.js";

const bytes = (value, length = 32) => new Uint8Array(length).fill(value);
const state = (eq, ep) => new KagemushaV1.PastaStateCommitment({ eq: bytes(eq), ep: bytes(ep) });
const signatureBytes = () => {
  const value = new Uint8Array(64);
  value[31] = 1;
  value[63] = 1;
  return value;
};
const publicKeyBytes = Uint8Array.from(Buffer.from(
  "04"
    + "6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
    + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
  "hex",
));

function context() {
  const networkId = NetworkId.fromBytes(Uint8Array.from([
    ...Array.from({ length: 31 }, (_, index) => index + 1), 1,
  ]));
  const asset = new KagemushaV1.AssetDefinitionId("6TEAJqbb8oEPmLncoNiMRbLEK6tw");
  const assetIncarnation = new KagemushaV1.AssetIncarnation(bytes(1));
  const recipient = new KagemushaV1.AccountId(
    "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
  );
  const devicePublicKey = new KagemushaV1.DevicePublicKey(publicKeyBytes);
  const signature = new KagemushaV1.DeviceSignature(signatureBytes());
  const hardwareCredential = new KagemushaV1.HardwareCredential({
    version: 1,
    credentialId: bytes(2),
    networkId,
    hardwareProfileId: bytes(3),
    suiteId: bytes(4),
    firmwarePolicyDigest: bytes(5),
    policyEpoch: 1n,
    laneCommitment: bytes(6),
    hardwareEpochId: bytes(7),
    hardwareEpochGeneration: 1n,
    devicePublicKey,
    deviceKeyReference: KagemushaV1.deviceKeyReference(devicePublicKey),
    issuedAtMs: 1n,
    expiresAtMs: 10_000n,
    governanceSignature: signature,
  });
  return { networkId, asset, assetIncarnation, recipient, signature, hardwareCredential };
}

function requestFor(amount = 5n) {
  const value = context();
  return new KagemushaV1.PaymentRequest({
    version: 1,
    releaseId: bytes(8),
    networkId: value.networkId,
    asset: value.asset,
    assetIncarnation: value.assetIncarnation,
    scale: 2,
    liabilityPoolId: KagemushaV1.liabilityPoolId(value.networkId, value.asset, value.assetIncarnation),
    recipient: value.recipient,
    recipientLaneId: value.hardwareCredential.laneCommitment,
    recipientEncryptionKey: bytes(20),
    amount,
    hardwareCredential: value.hardwareCredential,
    requestId: bytes(9),
    issuedAtMs: 100n,
    expiresAtMs: 200n,
    signature: value.signature,
  });
}

function proof(semanticDigest) {
  return new KagemushaV1.PairedProof({
    version: 1,
    eqProtocolDigest: bytes(10),
    epProtocolDigest: bytes(11),
    semanticDigest,
    guardEqCredentialAudit: bytes(12),
    guardEpCredentialAudit: bytes(13),
    eqDeferredAudit: bytes(14),
    epDeferredAudit: bytes(15),
    eqProof: bytes(16, 8),
    epProof: bytes(17, 8),
    eqHistory: bytes(18, 544),
    epHistory: bytes(19, 544),
  });
}

function paymentFor(request, seed = 30) {
  const encryptedCredit = KagemushaV1.encodeEncryptedCreditEnvelope(
    new KagemushaV1.EncryptedCreditEnvelope({
      version: 1,
      ephemeralX25519PublicKey: bytes(seed),
      nonce: bytes(seed + 1, 24),
      ciphertextAndTag: bytes(seed + 2, 216),
    }),
    request.recipientEncryptionKey,
  );
  const requestDigest = KagemushaV1.paymentRequestDigest(request);
  const transitionNullifier = bytes(seed + 3);
  const senderBeforeCommitment = state(seed + 4, seed + 5);
  const senderAfterCommitment = state(seed + 6, seed + 7);
  const ciphertextCommitment = bytes(seed + 8);
  const creditId = KagemushaV1.creditId(
    transitionNullifier,
    requestDigest,
    senderBeforeCommitment,
    senderAfterCommitment,
    request.recipientLaneId,
    request.recipientEncryptionKey,
    request.amount,
    ciphertextCommitment,
  );
  const lifecycle = new KagemushaV1.LifecycleBinding({
    version: 1,
    networkId: request.networkId,
    protocolVersion: 1,
    suiteId: request.hardwareCredential.suiteId,
    vkDigest: bytes(seed + 9),
    releaseId: request.releaseId,
    asset: request.asset,
    assetIncarnation: request.assetIncarnation,
    scale: request.scale,
    liabilityPoolId: request.liabilityPoolId,
    hardwareProfileId: request.hardwareCredential.hardwareProfileId,
    policyEpoch: request.hardwareCredential.policyEpoch,
    operationKind: "sendSplit",
    requestId: request.requestId,
    creditId,
    ciphertextDigest: KagemushaV1.ciphertextDigest(encryptedCredit),
  });
  const statement = new KagemushaV1.TransferStatement({
    version: 1,
    lifecycle,
    amount: request.amount,
    transitionNullifier,
    senderBeforeCommitment,
    senderAfterCommitment,
    requestDigest,
    recipientLaneId: request.recipientLaneId,
    recipientEncryptionKey: request.recipientEncryptionKey,
    ciphertextCommitment,
    committedAtMs: 150n,
    hardwareTransitionCommitment: bytes(seed + 10),
  });
  return new KagemushaV1.Payment({
    version: 1,
    statement,
    proof: proof(KagemushaV1.transferStatementDigest(statement)),
    encryptedCredit,
  });
}

function acknowledgementFor(request, payment) {
  return new KagemushaV1.Acknowledgement({
    version: 1,
    requestDigest: KagemushaV1.paymentRequestDigest(request),
    paymentDigest: KagemushaV1.paymentDigest(payment, request),
    inboxReceipt: new KagemushaV1.InboxReceipt({
      version: 1,
      creditId: payment.statement.lifecycle.creditId,
      receiptCommitment: bytes(60),
    }),
    signature: context().signature,
  });
}

test("Kagemusha V1 exposes only the three-message peer protocol", () => {
  for (const retired of [
    "AcceptanceIntent", "AcceptanceIntentAuthorization", "AcceptanceTicket", "NoCommitClosure",
    "CommitCertificate", "CommitWrapperProof", "validatePreTicketExchange", "validateCompleteExchange",
  ]) assert.equal(Object.hasOwn(KagemushaV1, retired), false);
  assert.equal(KagemushaV1.textPrefix, "kgm1:");
  assert.deepEqual(Object.keys(KagemushaV1.payloadKinds).filter((kind) => kind.includes("acceptance")), []);
});

test("request, payment, and durable acknowledgement round-trip as one compact session", () => {
  const request = requestFor();
  const payment = paymentFor(request);
  const acknowledgement = acknowledgementFor(request, payment);
  const requestRaw = KagemushaV1.encodePaymentRequest(request);
  const paymentRaw = KagemushaV1.encodePayment(payment, request);
  const acknowledgementRaw = KagemushaV1.encodeAcknowledgement(acknowledgement, request, payment);

  assert.deepEqual(KagemushaV1.encodePaymentRequest(KagemushaV1.decodePaymentRequest(requestRaw)), requestRaw);
  assert.deepEqual(KagemushaV1.encodePayment(KagemushaV1.decodePayment(paymentRaw, request), request), paymentRaw);
  assert.deepEqual(KagemushaV1.encodeAcknowledgement(
    KagemushaV1.decodeAcknowledgement(acknowledgementRaw, request, payment), request, payment,
  ), acknowledgementRaw);
  assert.ok(KagemushaV1.validateSession(request, payment, acknowledgement) <= 9211);
  for (const [kind, raw] of [["paymentRequest", requestRaw], ["payment", paymentRaw], ["acknowledgement", acknowledgementRaw]]) {
    const text = KagemushaV1.encodeText(kind, raw);
    assert.match(text, /^kgm1:/u);
    assert.deepEqual(KagemushaV1.decodeText(kind, text), raw);
  }
});

test("shared fixture is the exact three-message KAGEMUSHA protocol", () => {
  const fixture = JSON.parse(readFileSync(
    new URL("../../../fixtures/offline/kagemusha_v1.json", import.meta.url), "utf8",
  ));
  assert.equal(fixture.protocol, "KAGEMUSHA V1");
  assert.equal(fixture.text_prefix, "kgm1:");
  for (const retired of [
    "acceptance_intent_authorization", "acceptance_ticket", "no_commit_closure", "complete_five_message",
  ]) assert.equal(Object.hasOwn(fixture, retired), false);

  const request = requestFor();
  const payment = paymentFor(request);
  const acknowledgement = acknowledgementFor(request, payment);
  const entries = [
    ["payment_request", "paymentRequest", KagemushaV1.encodePaymentRequest(request)],
    ["payment", "payment", KagemushaV1.encodePayment(payment, request)],
    ["acknowledgement", "acknowledgement", KagemushaV1.encodeAcknowledgement(acknowledgement, request, payment)],
  ];
  for (const [name, kind, raw] of entries) {
    assert.deepEqual(Uint8Array.from(Buffer.from(fixture[name].norito_hex, "hex")), raw);
    assert.equal(fixture[name].raw_bytes, raw.length);
    assert.equal(fixture[name].sha256, createHash("sha256").update(raw).digest("hex"));
    assert.equal(fixture[name].kgm1, KagemushaV1.encodeText(kind, raw));
  }
});

test("peer AAD binds state transition, receiver lane/key, and trusted commit time", () => {
  const request = requestFor();
  const payment = paymentFor(request);
  const aad = KagemushaV1.encryptedCreditAadForPeer(payment.statement, request);
  assert.equal(aad.purpose, "peer");
  assert.deepEqual(aad.creditId, payment.statement.lifecycle.creditId);
  assert.deepEqual(aad.issuanceOrTransitionCommitment, payment.statement.ciphertextCommitment);
  const contextValue = KagemushaV1.peerCreditContext(payment.statement, request);
  assert.deepEqual(contextValue.recipientLaneId, request.recipientLaneId);
  assert.equal(contextValue.committedAtMs, 150n);

  const fields = [
    "version", "lifecycle", "amount", "transitionNullifier", "senderBeforeCommitment",
    "senderAfterCommitment", "requestDigest", "recipientLaneId", "recipientEncryptionKey",
    "ciphertextCommitment", "committedAtMs", "hardwareTransitionCommitment",
  ];
  const substituted = new KagemushaV1.TransferStatement({
    ...Object.fromEntries(fields.map((field) => [field, payment.statement[field]])),
    committedAtMs: request.expiresAtMs,
  });
  assert.throws(() => KagemushaV1.encryptedCreditAadForPeer(substituted, request));
});

test("one request authorizes distinct valid credits without a receiver balance-head binding", () => {
  const request = requestFor();
  const first = paymentFor(request, 30);
  const second = paymentFor(request, 70);
  assert.notDeepEqual(first.statement.lifecycle.creditId, second.statement.lifecycle.creditId);
  assert.doesNotThrow(() => KagemushaV1.encodePayment(first, request));
  assert.doesNotThrow(() => KagemushaV1.encodePayment(second, request));

  const oldRequestFields = [
    "version", "releaseId", "networkId", "asset", "assetIncarnation", "scale", "liabilityPoolId",
    "recipient", "amount", "hardwareCredential", "requestId", "issuedAtMs", "expiresAtMs", "signature",
  ];
  assert.throws(() => new KagemushaV1.PaymentRequest(
    Object.fromEntries(oldRequestFields.map((field) => [field, request[field]])),
  ), /missing or unknown/u);
});
