// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

import { NetworkId } from "../src/networkId.js";
import { crc64Xz } from "../src/crc64Xz.js";
import { Kagemusha } from "../src/kagemusha.js";
import { requireKagemushaSubmissionResponseV1 } from "../src/kagemushaToriiV1.js";

const octets = (value, length = 32) => new Uint8Array(length).fill(value);
const accountLiteral = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
const devicePublicKey = Uint8Array.from(Buffer.from(
  "046b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"
    + "4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5",
  "hex",
));

function baseContext() {
  const networkId = NetworkId.fromBytes(Uint8Array.from([
    ...Array.from({ length: 31 }, (_, index) => index + 1), 1,
  ]));
  const asset = new Kagemusha.AssetDefinitionId("6TEAJqbb8oEPmLncoNiMRbLEK6tw");
  const assetIncarnation = new Kagemusha.AssetIncarnation(octets(1));
  const recipient = new Kagemusha.AccountId(accountLiteral);
  const key = new Kagemusha.DevicePublicKey(devicePublicKey);
  const hardwareCredential = new Kagemusha.HardwareCredential({
    version: 1,
    credentialId: octets(2),
    networkId,
    hardwareProfileId: octets(3),
    suiteId: octets(4),
    firmwarePolicyDigest: octets(5),
    policyEpoch: 1n,
    laneCommitment: octets(6),
    hardwareEpochId: octets(7),
    hardwareEpochGeneration: 1n,
    devicePublicKey: key,
    deviceKeyReference: Kagemusha.deviceKeyReference(key),
    issuedAtMs: 1n,
    expiresAtMs: 10_000n,
    governanceSignature: new Kagemusha.DeviceSignature(octets(8, 64)),
  });
  return { networkId, asset, assetIncarnation, recipient, hardwareCredential };
}

function requestFor() {
  const value = baseContext();
  return new Kagemusha.PaymentRequest({
    version: 1,
    releaseId: octets(9),
    networkId: value.networkId,
    asset: value.asset,
    assetIncarnation: value.assetIncarnation,
    scale: 2,
    liabilityPoolId: Kagemusha.liabilityPoolId(value.networkId, value.asset, value.assetIncarnation),
    recipient: value.recipient,
    amount: 5n,
    recipientEncryptionKey: octets(10),
    hardwareCredential: value.hardwareCredential,
    requestId: octets(11),
    issuedAtMs: 100n,
    expiresAtMs: 200n,
    signature: new Kagemusha.DeviceSignature(octets(12, 64)),
  });
}

// These are codec/shape fixtures, not cryptographic proof qualification.
const replace = (value, updates) => new value.constructor({ ...value._kagemushaValues(), ...updates });
const digest = (domain, transcript) => {
  const size = Buffer.alloc(8);
  size.writeBigUInt64LE(BigInt(transcript.length));
  return createHash("sha256").update(Buffer.concat([Buffer.from(domain), Buffer.from([0]), size, Buffer.from(transcript)])).digest();
};

function paymentFor(request, proofBytes = 8) {
  const encryptedCredit = Kagemusha.encodeEncryptedCreditEnvelope(new Kagemusha.EncryptedCreditEnvelope({
    version: 1, ephemeralX25519PublicKey: octets(50), nonce: octets(51, 24), ciphertextAndTag: octets(52, 216),
  }), request.recipientEncryptionKey);
  const transitionNullifier = octets(33);
  const requestDigest = Kagemusha.paymentRequestDigest(request);
  const output = new Kagemusha.PaymentOutput({ version: 1, requestDigest, amount: request.amount,
    senderBeforeCommitment: octets(31), senderAfterCommitment: octets(32), transitionNullifier,
    creditId: Kagemusha.creditId(transitionNullifier, requestDigest), ciphertextCommitment: octets(34),
    commitEvidence: new Kagemusha.TrustedCommitTime({ timeEvidenceCommitment: octets(53) }),
    committedAtMs: 150n });
  const provisional = new Kagemusha.CommitCertificate({ version: 1, certificateId: octets(60),
    candidateEnvelopeDigest: octets(61), lifecycleBindingDigest: octets(62), transitionNullifier,
    outboxReservationCommitment: octets(63), commitEvidence: output.commitEvidence, hardwareProfileId: octets(64),
    policyEpoch: 1n, hardwareTerminalCommitment: octets(65) });
  const commitCertificate = replace(provisional, { certificateId: Kagemusha.expectedCommitCertificateId(provisional) });
  const proof = new Kagemusha.PaymentProof({ version: 1, eqProtocolDigest: octets(20), epProtocolDigest: octets(21),
    semanticDigest: Kagemusha.paymentBodyDigest(output, encryptedCredit), candidateEnvelopeDigest: commitCertificate.candidateEnvelopeDigest,
    commitCertificateDigest: Kagemusha.commitCertificateDigest(commitCertificate),
    eqDeferredAudit: octets(24), epDeferredAudit: octets(25), eqProof: octets(26, proofBytes), epProof: octets(27, proofBytes),
    eqHistory: octets(28, 544), epHistory: octets(29, 544) });
  return new Kagemusha.Payment({ version: 1, output, encryptedCredit, commitCertificate, proof });
}

function completeExchange(proofBytes = 8) {
  const request = requestFor();
  const payment = paymentFor(request, proofBytes);
  const acknowledgement = new Kagemusha.Acknowledgement({ version: 1,
    requestDigest: Kagemusha.paymentRequestDigest(request), paymentDigest: Kagemusha.paymentDigest(payment, request),
    inboxReceipt: new Kagemusha.InboxReceipt({ version: 1, creditId: payment.output.creditId, receiptCommitment: octets(55) }),
    signature: new Kagemusha.DeviceSignature(octets(56, 64)) });
  return { request, payment, acknowledgement };
}

function pairedProof(semanticDigest, seed) {
  return new Kagemusha.PairedProof({
    version: 1, eqProtocolDigest: octets(seed), epProtocolDigest: octets(seed + 1), semanticDigest,
    guardEqCredentialAudit: octets(seed + 2), guardEpCredentialAudit: octets(seed + 3),
    eqDeferredAudit: octets(seed + 4), epDeferredAudit: octets(seed + 5),
    eqProof: octets(seed + 6, 8), epProof: octets(seed + 7, 8),
    eqHistory: octets(seed + 8, 544), epHistory: octets(seed + 9, 544),
  });
}

function mintStagePair() {
  const base = baseContext();
  const encryptedCredit = Kagemusha.encodeEncryptedCreditEnvelope(new Kagemusha.EncryptedCreditEnvelope({
    version: 1, ephemeralX25519PublicKey: octets(90), nonce: octets(91, 24), ciphertextAndTag: octets(92, 216),
  }));
  const context = new Kagemusha.MintAuthorizationContext({
    version: 1, operationId: octets(100), releaseId: octets(101), suiteId: base.hardwareCredential.suiteId,
    vkDigest: octets(102), artifactManifestDigest: octets(103), networkId: base.networkId, asset: base.asset,
    assetIncarnation: base.assetIncarnation, scale: 2,
    liabilityPoolId: Kagemusha.liabilityPoolId(base.networkId, base.asset, base.assetIncarnation),
    amount: 7n, payer: base.recipient, recipient: base.recipient,
    hardwareCredentialId: base.hardwareCredential.credentialId,
    hardwareProfileId: base.hardwareCredential.hardwareProfileId, policyEpoch: 1n,
    recipientCredentialCommitment: octets(104), creditCommitment: octets(105),
    recipientOneTimeKey: octets(106),
  });
  const ciphertextDigest = Kagemusha.ciphertextDigest(encryptedCredit);
  const provisionalLifecycle = new Kagemusha.LifecycleBinding({
    version: 1, networkId: base.networkId, protocolVersion: 1, suiteId: context.suiteId,
    vkDigest: context.vkDigest, releaseId: context.releaseId, asset: base.asset,
    assetIncarnation: base.assetIncarnation, scale: context.scale, liabilityPoolId: context.liabilityPoolId,
    hardwareProfileId: context.hardwareProfileId, policyEpoch: context.policyEpoch, operationKind: "mintFold",
    requestId: octets(0), receiverLaneCommitment: octets(0), creditId: octets(107), ciphertextDigest,
  });
  const provisionalStatement = new Kagemusha.MintCreditStatement({
    version: 1, lifecycle: provisionalLifecycle,
    recipientCredentialCommitment: context.recipientCredentialCommitment,
    authorizationContextDigest: Kagemusha.mintAuthorizationContextDigest(context),
    mintAuthorizationDigest: octets(108), amount: context.amount, issuanceCommitment: octets(109),
    recipient: context.recipient, creditCommitment: context.creditCommitment, mintedAtMs: 123n,
  });
  const creditId = Kagemusha.mintCreditId(provisionalStatement);
  const lifecycle = replace(provisionalLifecycle, { creditId });
  const authorizationStatement = new Kagemusha.MintAuthorizationStatement({
    version: 1, context, issuanceCommitment: provisionalStatement.issuanceCommitment, creditId, ciphertextDigest,
  });
  const authorization = new Kagemusha.MintAuthorization({
    version: 1, statement: authorizationStatement,
    proof: pairedProof(Kagemusha.mintAuthorizationStatementDigest(authorizationStatement), 110),
  });
  const statement = replace(provisionalStatement, {
    lifecycle, mintAuthorizationDigest: Kagemusha.mintAuthorizationDigest(authorization),
  });
  const credit = new Kagemusha.MintCredit({
    version: 1, statement, proof: pairedProof(Kagemusha.mintCreditStatementDigest(statement), 121),
    finalityCertificateBinding: octets(132), finalityAuthorityHead: octets(133),
    finalityGenesisRosterId: octets(134), finalityProofBindingDigest: octets(135),
    encryptedCredit, artifactManifestDigest: context.artifactManifestDigest,
  });
  return { authorization, credit };
}

test("KAGEMUSHA submission responses pin status, Location, and Retry-After", () => {
  const operationIdHex = "41".repeat(32), location = `/v1/kagemusha/operations/${operationIdHex}`;
  for (const [statusCode, retryAfter, operationState] of [[202, "1", "pending"], [200, null, "applied"]])
    assert.doesNotThrow(() => requireKagemushaSubmissionResponseV1({ statusCode, location, retryAfter, operationIdHex, operationState }));
});

test("operation-21 mint-stage bodies are canonical, bounded, and credit-bound", () => {
  const { authorization, credit } = mintStagePair();
  const authorizationBytes = Kagemusha.encodeMintAuthorization(authorization);
  const creditBytes = Kagemusha.encodeMintCredit(credit, authorization);
  const command = new Kagemusha.DeviceMintStageCommand({
    version: 1, canonicalAuthorization: authorizationBytes, canonicalMintCredit: creditBytes,
  });
  const commandBytes = Kagemusha.encodeDeviceMintStageCommandShape(command);
  assert.ok(commandBytes.length <= Kagemusha.maximumDeviceMintStageCommandBytes);
  const decoded = Kagemusha.decodeDeviceMintStageCommandShapeExact(commandBytes);
  assert.deepEqual(decoded.canonicalAuthorization, authorizationBytes);
  assert.deepEqual(decoded.canonicalMintCredit, creditBytes);
  assert.deepEqual(Kagemusha.encodeDeviceMintStageCommandShape(authorizationBytes, creditBytes), commandBytes);

  for (const disposition of Object.values(Kagemusha.deviceMintStageDispositions)) {
    const result = new Kagemusha.DeviceMintStageResult({
      version: 1, disposition, creditId: credit.statement.lifecycle.creditId,
    });
    const resultBytes = Kagemusha.encodeDeviceMintStageResultShape(result, command);
    assert.ok(resultBytes.length <= Kagemusha.maximumDeviceMintStageResultBytes);
    assert.deepEqual(Kagemusha.encodeDeviceMintStageResultShape(
      Kagemusha.decodeDeviceMintStageResultShapeExact(resultBytes, decoded), decoded,
    ), resultBytes);
  }

  assert.throws(() => Kagemusha.decodeDeviceMintStageCommandShapeExact(
    Buffer.concat([Buffer.from(commandBytes), Buffer.of(0)]),
  ));
  assert.throws(() => Kagemusha.encodeDeviceMintStageCommandShape(new Kagemusha.DeviceMintStageCommand({
    version: 1, canonicalAuthorization: Buffer.concat([Buffer.from(authorizationBytes), Buffer.of(0)]),
    canonicalMintCredit: creditBytes,
  })));
  const substituted = new Kagemusha.DeviceMintStageResult({ version: 1, disposition: 0, creditId: octets(222) });
  assert.throws(() => Kagemusha.encodeDeviceMintStageResultShape(substituted, command), /credit ID/u);
  assert.throws(() => new Kagemusha.DeviceMintStageResult({ version: 1, disposition: 2, creditId: octets(1) }));
  assert.throws(() => new Kagemusha.DeviceMintStageCommand({
    version: 1, canonicalAuthorization: new Uint8Array(7937), canonicalMintCredit: creditBytes,
  }), /7936/u);

  const badLifecycle = replace(credit.statement.lifecycle, { creditId: octets(223) });
  const badStatement = replace(credit.statement, { lifecycle: badLifecycle });
  assert.throws(() => Kagemusha.mintCreditStatementDigest(badStatement), /credit ID/u);
  assert.throws(() => Kagemusha.validateMintCreditAgainstAuthorization(
    replace(credit, { statement: badStatement }), authorization,
  ), /credit ID/u);
  const substitutedAuthorization = replace(authorization, {
    proof: replace(authorization.proof, { eqProof: octets(224, 9) }),
  });
  assert.throws(() => Kagemusha.encodeDeviceMintStageCommandShape(
    Kagemusha.encodeMintAuthorization(substitutedAuthorization), creditBytes,
  ), /authorization digest/u);
});

test("operation-21 result decoding rejects semantic and canonical mutations", () => {
  const raw = Kagemusha.encodeDeviceMintStageResultShape(new Kagemusha.DeviceMintStageResult({
    version: 1, disposition: 0, creditId: octets(1),
  }));
  assert.equal(raw.length, 78);
  assert.deepEqual(Array.from(raw.subarray(40, 46)), [2, 1, 0, 1, 0, 32]);
  for (const mutate of [
    (bytes) => { bytes[41] = 2; },
    (bytes) => { bytes[44] = 2; },
    (bytes) => { bytes.fill(0, 46); },
    (bytes) => { bytes[43] = 2; },
    (bytes) => { bytes[39] = 0; },
  ]) {
    const mutated = Uint8Array.from(raw);
    mutate(mutated);
    new DataView(mutated.buffer).setBigUint64(31, crc64Xz(mutated.subarray(40)), true);
    assert.throws(() => Kagemusha.decodeDeviceMintStageResultShapeExact(mutated));
  }
  for (const invalid of [raw.subarray(0, -1), Buffer.concat([Buffer.from(raw), Buffer.of(0)]), new Uint8Array(129)]) {
    assert.throws(() => Kagemusha.decodeDeviceMintStageResultShapeExact(invalid));
  }
  assert.throws(() => Kagemusha.decodeDeviceMintStageCommandShapeExact(new Uint8Array(65537)), /65536/u);
  assert.throws(() => Kagemusha.encodeDeviceMintStageCommandShape(new Kagemusha.DeviceMintStageCommand({
    version: 1, canonicalAuthorization: new Uint8Array(), canonicalMintCredit: new Uint8Array(),
  })));
});

test("operation-21 command archive bytes are defensively copied", () => {
  const { authorization, credit } = mintStagePair();
  const authorizationBytes = Kagemusha.encodeMintAuthorization(authorization);
  const creditBytes = Kagemusha.encodeMintCredit(credit, authorization);
  const command = new Kagemusha.DeviceMintStageCommand({
    version: 1, canonicalAuthorization: authorizationBytes, canonicalMintCredit: creditBytes,
  });
  const before = Kagemusha.encodeDeviceMintStageCommandShape(command);
  authorizationBytes.fill(0);
  creditBytes.fill(0);
  command.canonicalAuthorization.fill(0);
  command.canonicalMintCredit.fill(0);
  assert.deepEqual(Kagemusha.encodeDeviceMintStageCommandShape(command), before);
});

test("three-message tags and caps are the sole peer transport", () => {
  assert.deepEqual(Object.fromEntries(Object.entries(Kagemusha.ipm1PayloadKinds).map(([kind, value]) => [kind, value.tag])),
    { request: 1, payment: 2, acknowledgement: 3 });
  assert.deepEqual(Kagemusha.operationKinds,
    { bootstrap: 0, mintFold: 1, sendSplit: 2, receiveFold: 3, redeemSplit: 4, rotate: 5 });
  assert.equal(Kagemusha.maximumCompleteExchangeRawBytes, 9211);
  assert.equal(Kagemusha.maximumCompleteExchangeTextBytes, 12288);
  assert.equal(Kagemusha.paymentOutboxMinimumBytes, 25728);
  assert.equal(Kagemusha.maximumPaymentProofBytes, 6528);
  for (const name of ["AmountPolicy", "SingleExact", "PartialUntilTotal", "BoundedMultiPayment", "OpenReceive",
    "AcceptanceIntent", "AcceptanceTicket", "NoCommitClosure", "encodeAcceptanceIntent",
    "decodeAcceptanceTicket", "validatePreTicketExchange"]) assert.equal(name in Kagemusha, false);
  assert.throws(() => Kagemusha.ipm1PayloadKindFromTag(4));
  assert.throws(() => Kagemusha.ipm1PayloadKindFromTag(5));
});

test("request binds one exact amount and receiver encryption key", () => {
  const request = requestFor();
  const decoded = Kagemusha.decodePaymentRequest(Kagemusha.encodePaymentRequest(request));
  assert.equal(decoded.amount, 5n);
  assert.deepEqual(decoded.recipientEncryptionKey, octets(10));
  assert.equal(Kagemusha.paymentRequestTranscript(request).length, 390);
  assert.throws(() => replace(request, { amount: 0n }), /positive/u);
  assert.throws(() => replace(request, { recipientEncryptionKey: octets(0) }), /nonzero/u);
  assert.throws(() => new Kagemusha.PaymentRequest({ ...request._kagemushaValues(), requestMode: {} }));
});

test("semantic digests use fixed transcripts rather than canonical Norito frames", () => {
  const { request, payment } = completeExchange();
  const requestTranscript = Kagemusha.paymentRequestTranscript(request);
  assert.equal(requestTranscript.length, 390);
  assert.equal(Kagemusha.paymentOutputTranscript(payment.output).length, 254);
  assert.deepEqual(Buffer.from(Kagemusha.paymentRequestDigest(request)), digest("iroha:kagemusha:v1:payment-request", requestTranscript));
  assert.deepEqual(Buffer.from(Kagemusha.paymentRequestSigningBytes(request)).subarray(-326), Buffer.from(requestTranscript).subarray(0, 326));
  const expected = digest("iroha:kagemusha:v1:payment-body", Buffer.concat([
    Buffer.from(Kagemusha.paymentOutputDigest(payment.output)), Buffer.from(Kagemusha.ciphertextDigest(payment.encryptedCredit))]));
  assert.deepEqual(Buffer.from(payment.proof.semanticDigest), expected);
});

test("all three messages and the payment proof round-trip at the maximum proof size", () => {
  const { request, payment, acknowledgement } = completeExchange(Kagemusha.maximumParityProofBytes);
  const messages = [
    ["paymentRequest", request, Kagemusha.encodePaymentRequest, Kagemusha.decodePaymentRequest, []],
    ["payment", payment, Kagemusha.encodePayment, Kagemusha.decodePayment, [request]],
    ["acknowledgement", acknowledgement, Kagemusha.encodeAcknowledgement, Kagemusha.decodeAcknowledgement, [request, payment]],
  ];
  let rawSize = 0, textSize = 0;
  for (const [kind, value, encode, decode, bindings] of messages) {
    const raw = encode(value, ...bindings);
    assert.deepEqual(encode(decode(raw, ...bindings), ...bindings), raw);
    const text = Kagemusha.encodeText(kind, raw);
    assert.deepEqual(Kagemusha.decodeText(kind, text), raw);
    rawSize += raw.length; textSize += text.length;
  }
  assert.equal(Kagemusha.validateCompleteExchange(request, payment, acknowledgement), rawSize);
  assert.ok(rawSize <= 9211); assert.ok(textSize <= 12288);
  const proofRaw = Kagemusha.encodePaymentProof(payment.proof);
  assert.ok(proofRaw.length <= 6528);
  assert.deepEqual(Kagemusha.encodePaymentProof(Kagemusha.decodePaymentProof(proofRaw)), proofRaw);
});

test("request-owned key and sender states bind preparation and peer AAD", () => {
  const { request, payment } = completeExchange();
  const context = Kagemusha.peerCreditContext(payment.output, request);
  assert.deepEqual(context.recipientEncryptionKey, request.recipientEncryptionKey);
  assert.equal(context.amount, request.amount);
  assert.deepEqual(context.senderBeforeCommitment, payment.output.senderBeforeCommitment);
  assert.deepEqual(context.preparedTransferDigest, Kagemusha.preparedTransferDigest(request,
    payment.output.senderBeforeCommitment, payment.output.senderAfterCommitment,
    payment.output.transitionNullifier, payment.output.ciphertextCommitment));
  const changedRequest = replace(request, { recipientEncryptionKey: octets(77) });
  assert.throws(() => Kagemusha.encodePayment(payment, changedRequest), /request digest/u);
});

test("output, ciphertext, certificate, and post-commit proof substitutions fail", () => {
  const { request, payment } = completeExchange();
  for (const field of ["requestDigest", "senderBeforeCommitment", "senderAfterCommitment", "transitionNullifier", "creditId", "ciphertextCommitment"]) {
    const bad = replace(payment, { output: replace(payment.output, { [field]: octets(99) }) });
    assert.throws(() => Kagemusha.encodePayment(bad, request));
  }
  assert.throws(() => Kagemusha.encodePayment(replace(payment, { output: replace(payment.output, { amount: 6n }) }), request), /amount/u);
  assert.throws(() => Kagemusha.encodePayment(replace(payment, { output: replace(payment.output, { committedAtMs: 200n }) }), request), /commit time/u);
  for (const field of ["semanticDigest", "candidateEnvelopeDigest", "commitCertificateDigest"])
    assert.throws(() => Kagemusha.encodePayment(replace(payment, { proof: replace(payment.proof, { [field]: octets(98) }) }), request), field);
  const differentEnvelope = Kagemusha.encodeEncryptedCreditEnvelope(new Kagemusha.EncryptedCreditEnvelope({
    version: 1, ephemeralX25519PublicKey: octets(80), nonce: octets(81, 24), ciphertextAndTag: octets(82, 216) }));
  assert.throws(() => Kagemusha.encodePayment(replace(payment, { encryptedCredit: differentEnvelope }), request), /semantic/u);
  assert.throws(() => Kagemusha.encodePayment(replace(payment, { commitCertificate: replace(payment.commitCertificate,
    { hardwareTerminalCommitment: octets(96) }) }), request), /certificate/u);
  const rerandomized = replace(payment, { proof: replace(payment.proof, { eqProof: octets(90, 17), epProof: octets(91, 17) }) });
  assert.deepEqual(Kagemusha.paymentBodyDigest(rerandomized.output, rerandomized.encryptedCredit), payment.proof.semanticDigest);
  assert.doesNotThrow(() => Kagemusha.encodePayment(rerandomized, request));
});

test("credit ID and opening commitment have exact acyclic preimages", () => {
  const transition = octets(71), intentDigest = octets(72);
  assert.deepEqual(Buffer.from(Kagemusha.creditId(transition, intentDigest)), createHash("sha256").update(Buffer.concat([
    Buffer.from("iroha:kagemusha:v1:credit-id\0"), Buffer.from(transition), Buffer.from(intentDigest)])).digest());
  const amount = Buffer.alloc(16); amount.writeBigUInt64LE(5n);
  const expected = createHash("sha256").update(Buffer.concat([
    Buffer.from("iroha:kagemusha:v1:peer-credit-opening-commitment\0"), Buffer.from([1, 0]),
    Buffer.from(octets(81)), Buffer.from(octets(82)), amount,
    Buffer.from(octets(83)), Buffer.from(octets(84)), Buffer.from(octets(85))])).digest();
  assert.deepEqual(Buffer.from(Kagemusha.peerCreditOpeningCommitment(octets(81), octets(82), 5n, octets(83), octets(84), octets(85))), expected);
  assert.throws(() => Kagemusha.peerCreditOpeningCommitment(octets(81), octets(82), 0n, octets(83), octets(84), octets(85)), /positive/u);
});

test("outbox reservations cover the post-commit payment", () => {
  const { payment } = completeExchange();
  assert.throws(() => replace(payment.commitCertificate, { policyEpoch: 0n }), /positive/u);
  const fields = { reservationId: octets(90), operationKind: "sendSplit", issuedAtMs: 1n, expiresAtMs: 2n };
  assert.doesNotThrow(() => new Kagemusha.OutboxReservation({ ...fields, reservedOutboxBytes: 25728 }));
  assert.throws(() => new Kagemusha.OutboxReservation({ ...fields, reservedOutboxBytes: 25727 }), /outbox/u);
});

test("public peer schemas do not expose ancestry or accept retired transports", () => {
  const source = readFileSync(new URL("../src/kagemusha.js", import.meta.url), "utf8");
  for (const fragments of [["recipient", "LaneId"], ["max", "Hops"], ["Acceptance", "Ticket"], ["request", "Mode"]])
    assert.equal(source.includes(fragments.join("")), false);
  const text = Kagemusha.encodeTypedText("paymentRequest", requestFor());
  assert.throws(() => Kagemusha.decodeTypedText("paymentRequest", ["oc", "1:"].join("") + text.slice(5)), /prefix/u);
  assert.throws(() => Kagemusha.encodeTypedText("acceptanceIntentAuthorization", {}));
});
