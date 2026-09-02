// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

import { NetworkId } from "../src/networkId.js";
import { OfflineCashV1 } from "../src/offlineCashV1.js";

const fixture = JSON.parse(
  readFileSync(new URL("../../../fixtures/offline/offline_cash_v1.json", import.meta.url), "utf8"),
);
const canonicalFixtureKeys = [
  "payment_request",
  "acceptance_intent_authorization",
  "acceptance_ticket",
  "no_commit_closure",
  "payment",
  "acknowledgement",
  "mint_authorization",
  "mint_credit",
  "redemption_voucher",
  "encrypted_credit_envelope",
  "encrypted_credit_aad",
  "credit_opening",
  "pre_ticket_exchange",
  "terminal_trio",
  "complete_five_message",
];
const hasCanonicalFixture = canonicalFixtureKeys.every((key) => Object.hasOwn(fixture, key));
const fixtureTransportKinds = Object.freeze({
  payment_request: "paymentRequest",
  acceptance_intent_authorization: "acceptanceIntentAuthorization",
  acceptance_ticket: "acceptanceTicket",
  payment: "payment",
  acknowledgement: "acknowledgement",
  mint_authorization: "mintAuthorization",
  mint_credit: "mintCredit",
  redemption_voucher: "redemptionVoucher",
});
const fixtureSummaries = Object.freeze({
  pre_ticket_exchange: [8960, 9984, 13326],
  terminal_trio: [8960, 9211, 12288],
  complete_five_message: [16384, 18171, 24244],
});

const bytes = (value, length = 32) => new Uint8Array(length).fill(value);
const concatenate = (...values) => Uint8Array.from(Buffer.concat(values.map((value) => Buffer.from(value))));
const littleEndian = (value, width) => {
  const result = new Uint8Array(width);
  let remaining = BigInt(value);
  for (let index = 0; index < width; index += 1) {
    result[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return result;
};
const semanticDigest = (domain, transcript) => Uint8Array.from(createHash("sha256")
  .update(Buffer.from(domain, "ascii"))
  .update(Buffer.from([0]))
  .update(littleEndian(transcript.length, 8))
  .update(transcript)
  .digest());
const intentSemanticTranscript = (intent) => concatenate(
  littleEndian(intent.version, 2),
  intent.requestDigest,
  intent.intentId,
  littleEndian(intent.exactAmount, 16),
  intent.senderOneTimeCommitment,
);
const authorizationStatementSemanticTranscript = (statement) => concatenate(
  littleEndian(statement.version, 2),
  intentSemanticTranscript(statement.intent),
  statement.releaseId,
  statement.suiteId,
  statement.vkDigest,
  statement.artifactManifestDigest,
);
const noCommitStatementSemanticTranscript = (statement) => concatenate(
  littleEndian(statement.version, 2),
  statement.releaseId,
  statement.suiteId,
  statement.vkDigest,
  statement.artifactManifestDigest,
  statement.senderHardwareBindingCommitment,
  statement.requestId,
  statement.requestDigest,
  statement.acceptanceTicketId,
  statement.ticketDigest,
  statement.intentAuthorizationDigest,
  statement.intentDigest,
  littleEndian(statement.exactAmount, 16),
  statement.senderOneTimeCommitment,
  statement.recoveryId,
  statement.cancellationNullifier,
  statement.equivalentDeliverySlotCommitment,
);
const outboxReservationSemanticTranscript = (reservation) => concatenate(
  reservation.reservationId,
  littleEndian(reservation.operationKind === "sendSplit" ? 2 : 4, 4),
  littleEndian(reservation.reservedOutboxBytes, 4),
  littleEndian(reservation.issuedAtMs, 8),
  littleEndian(reservation.expiresAtMs, 8),
);
const commitEvidenceSemanticTranscript = (evidence) => concatenate(
  littleEndian(evidence.source === "trustedTime" ? 0 : 1, 4),
  evidence.source === "trustedTime"
    ? evidence.evidence.timeEvidenceCommitment
    : evidence.evidence.leaseEvidenceCommitment,
);
const commitCertificateSemanticTranscript = (certificate, includeId) => concatenate(
  littleEndian(certificate.version, 2),
  ...(includeId ? [certificate.certificateId] : []),
  certificate.candidateEnvelopeDigest,
  certificate.lifecycleBindingDigest,
  certificate.transitionNullifier,
  certificate.outboxReservationCommitment,
  commitEvidenceSemanticTranscript(certificate.commitEvidence),
  certificate.hardwareProfileId,
  littleEndian(certificate.policyEpoch, 8),
  certificate.hardwareTerminalCommitment,
);
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

function baseContext() {
  const networkId = NetworkId.fromBytes(Uint8Array.from([
    ...Array.from({ length: 31 }, (_, index) => index + 1),
    1,
  ]));
  const asset = new OfflineCashV1.AssetDefinitionId("6TEAJqbb8oEPmLncoNiMRbLEK6tw");
  const assetIncarnation = new OfflineCashV1.AssetIncarnation(bytes(1));
  const recipient = new OfflineCashV1.AccountId(
    "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV",
  );
  const devicePublicKey = new OfflineCashV1.DevicePublicKey(publicKeyBytes);
  const signature = new OfflineCashV1.DeviceSignature(signatureBytes());
  const hardwareCredential = new OfflineCashV1.HardwareCredential({
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
    deviceKeyReference: OfflineCashV1.deviceKeyReference(devicePublicKey),
    issuedAtMs: 1n,
    expiresAtMs: 10_000n,
    governanceSignature: signature,
  });
  return { networkId, asset, assetIncarnation, recipient, signature, hardwareCredential };
}

function requestForAmount(amount = 5n) {
  const context = baseContext();
  return new OfflineCashV1.PaymentRequest({
    version: 1,
    releaseId: bytes(8),
    networkId: context.networkId,
    asset: context.asset,
    assetIncarnation: context.assetIncarnation,
    scale: 2,
    liabilityPoolId: OfflineCashV1.liabilityPoolId(
      context.networkId,
      context.asset,
      context.assetIncarnation,
    ),
    recipient: context.recipient,
    amount,
    hardwareCredential: context.hardwareCredential,
    requestId: bytes(9),
    issuedAtMs: 100n,
    expiresAtMs: 200n,
    signature: context.signature,
  });
}

function proof(semanticDigest, proofLength = 8) {
  return new OfflineCashV1.PairedProof({
    version: 1,
    eqProtocolDigest: bytes(10),
    epProtocolDigest: bytes(11),
    semanticDigest,
    guardEqCredentialAudit: bytes(12),
    guardEpCredentialAudit: bytes(13),
    eqDeferredAudit: bytes(14),
    epDeferredAudit: bytes(15),
    eqProof: bytes(16, proofLength),
    epProof: bytes(17, proofLength),
    eqHistory: bytes(18, 544),
    epHistory: bytes(19, 544),
  });
}

function preTicketExchange(request, exactAmount = request.amount, ticketSeed = 24) {
  const intent = new OfflineCashV1.AcceptanceIntent({
    version: 1,
    requestDigest: OfflineCashV1.paymentRequestDigest(request),
    intentId: bytes(20),
    exactAmount,
    senderOneTimeCommitment: bytes(21),
  });
  const statement = new OfflineCashV1.AcceptanceIntentAuthorizationStatement({
    version: 1,
    intent,
    releaseId: request.releaseId,
    suiteId: request.hardwareCredential.suiteId,
    vkDigest: bytes(22),
    artifactManifestDigest: bytes(23),
  });
  const authorization = new OfflineCashV1.AcceptanceIntentAuthorization({
    version: 1,
    statement,
    proof: proof(OfflineCashV1.acceptanceAuthorizationStatementDigest(statement)),
  });
  const ticket = new OfflineCashV1.AcceptanceTicket({
    version: 1,
    networkId: request.networkId,
    requestId: request.requestId,
    requestDigest: OfflineCashV1.paymentRequestDigest(request),
    acceptanceTicketId: bytes(ticketSeed),
    asset: request.asset,
    assetIncarnation: request.assetIncarnation,
    scale: request.scale,
    intentDigest: OfflineCashV1.acceptanceIntentDigest(intent),
    exactAmount,
    reservedInboxBytes: 8960,
    recipientOneTimeKey: bytes(25),
    hardwareProfileId: request.hardwareCredential.hardwareProfileId,
    policyEpoch: request.hardwareCredential.policyEpoch,
    issuedAtMs: 110n,
    expiresAtMs: 190n,
    signature: request.signature,
  });
  return { intent, authorization, ticket };
}

function noCommitClosureStatement(request, authorization, ticket) {
  const intent = authorization.statement.intent;
  return new OfflineCashV1.NoCommitClosureStatement({
    version: 1,
    releaseId: authorization.statement.releaseId,
    suiteId: authorization.statement.suiteId,
    vkDigest: authorization.statement.vkDigest,
    artifactManifestDigest: authorization.statement.artifactManifestDigest,
    senderHardwareBindingCommitment: bytes(26),
    requestId: request.requestId,
    requestDigest: OfflineCashV1.paymentRequestDigest(request),
    acceptanceTicketId: ticket.acceptanceTicketId,
    ticketDigest: OfflineCashV1.acceptanceTicketDigest(ticket),
    intentAuthorizationDigest: OfflineCashV1.acceptanceAuthorizationDigest(authorization),
    intentDigest: OfflineCashV1.acceptanceIntentDigest(intent),
    exactAmount: intent.exactAmount,
    senderOneTimeCommitment: intent.senderOneTimeCommitment,
    recoveryId: bytes(27),
    cancellationNullifier: bytes(28),
    equivalentDeliverySlotCommitment: bytes(29),
  });
}

function noCommitClosure(request, authorization, ticket, proofLength = 8) {
  const statement = noCommitClosureStatement(request, authorization, ticket);
  return new OfflineCashV1.NoCommitClosure({
    version: 1,
    statement,
    request,
    intentAuthorization: authorization,
    acceptanceTicket: ticket,
    proof: proof(OfflineCashV1.noCommitClosureStatementDigest(statement), proofLength),
  });
}

test("strict oc1 transport enforces the current request caps", () => {
  const raw = Uint8Array.from([0xfb, 0xff, 0x00, 0x01]);
  assert.equal(OfflineCashV1.encodeText("paymentRequest", raw), "oc1:-_8AAQ");
  assert.deepEqual(OfflineCashV1.decodeText("paymentRequest", "oc1:-_8AAQ"), raw);
  assert.equal(OfflineCashV1.maximumRequestRawBytes, 1024);
  assert.equal(OfflineCashV1.maximumRequestTextBytes, 1370);
  assert.throws(() => OfflineCashV1.encodeText("paymentRequest", bytes(1, 1025)));
  for (const invalid of ["OC1:-_8AAQ", "oc1:", "oc1:-_8AAQ==", "oc1:-_8A AQ", "oc1:+_8AAQ", "oc1:A"]) {
    assert.throws(() => OfflineCashV1.decodeText("paymentRequest", invalid));
  }
});

test("positive exact request amount canonically round-trips", () => {
  const request = requestForAmount(5n);
  const raw = OfflineCashV1.encodePaymentRequest(request);
  assert.ok(raw.length <= 1024);
  const decoded = OfflineCashV1.decodePaymentRequest(raw);
  assert.equal(decoded.amount, 5n);
  assert.deepEqual(OfflineCashV1.encodePaymentRequest(decoded), raw);
  assert.throws(() => requestForAmount(0n));
  assert.throws(() => OfflineCashV1.decodePaymentRequest(bytes(1, 1025)));
  assert.throws(() => OfflineCashV1.decodePaymentRequest(Uint8Array.from([...raw, 0])));
});

test("proof-bearing authorization and exact one-use ticket form the pre-ticket exchange", () => {
  const request = requestForAmount(5n);
  const { intent, authorization, ticket } = preTicketExchange(request);
  const authorizationRaw = OfflineCashV1.encodeAcceptanceIntentAuthorization(authorization, request);
  const ticketRaw = OfflineCashV1.encodeAcceptanceTicket(ticket, request, intent);
  assert.deepEqual(
    OfflineCashV1.encodeAcceptanceIntentAuthorization(
      OfflineCashV1.decodeAcceptanceIntentAuthorization(authorizationRaw, request),
      request,
    ),
    authorizationRaw,
  );
  assert.deepEqual(
    OfflineCashV1.encodeAcceptanceTicket(
      OfflineCashV1.decodeAcceptanceTicket(ticketRaw, request, intent),
      request,
      intent,
    ),
    ticketRaw,
  );
  assert.ok(OfflineCashV1.validatePreTicketExchange(request, authorization, ticket) <= 9984);
  assert.equal(OfflineCashV1.maximumPreTicketTextBytes, 13326);
});

test("no-commit recovery closure is canonical, fully cross-bound, and exactly bounded", () => {
  const request = requestForAmount(5n);
  const { authorization, ticket } = preTicketExchange(request);
  const closure = noCommitClosure(request, authorization, ticket);
  const raw = OfflineCashV1.encodeNoCommitClosure(closure);
  assert.ok(raw.length <= OfflineCashV1.maximumNoCommitClosureBytes);
  const decoded = OfflineCashV1.decodeNoCommitClosure(raw);
  assert.deepEqual(OfflineCashV1.encodeNoCommitClosure(decoded), raw);
  assert.equal(OfflineCashV1.noCommitClosureDigest(decoded).length, 32);
  assert.equal("predecessorState" in decoded.statement, false);
  assert.equal("successorState" in decoded.statement, false);

  const { ticket: substitutedTicket } = preTicketExchange(request, 5n, 30);
  assert.throws(() => OfflineCashV1.encodeNoCommitClosure(new OfflineCashV1.NoCommitClosure({
    version: 1,
    statement: closure.statement,
    request,
    intentAuthorization: authorization,
    acceptanceTicket: substitutedTicket,
    proof: closure.proof,
  })));
  const substitutedAuthorizationStatement = new OfflineCashV1.AcceptanceIntentAuthorizationStatement({
    version: 1,
    intent: authorization.statement.intent,
    releaseId: authorization.statement.releaseId,
    suiteId: authorization.statement.suiteId,
    vkDigest: bytes(31),
    artifactManifestDigest: authorization.statement.artifactManifestDigest,
  });
  const substitutedAuthorization = new OfflineCashV1.AcceptanceIntentAuthorization({
    version: 1,
    statement: substitutedAuthorizationStatement,
    proof: proof(OfflineCashV1.acceptanceAuthorizationStatementDigest(
      substitutedAuthorizationStatement,
    )),
  });
  assert.throws(() => OfflineCashV1.encodeNoCommitClosure(new OfflineCashV1.NoCommitClosure({
    version: 1,
    statement: closure.statement,
    request,
    intentAuthorization: substitutedAuthorization,
    acceptanceTicket: ticket,
    proof: closure.proof,
  })));
  assert.throws(() => new OfflineCashV1.NoCommitClosure({
    version: 1,
    statement: closure.statement,
    request,
    intentAuthorization: authorization,
    acceptanceTicket: ticket,
    proof: closure.proof,
    predecessorState: bytes(32),
  }));
  assert.throws(() => OfflineCashV1.decodeNoCommitClosure(bytes(1, 16385)), /16384/);
});

test("circuit-bound semantic hashes use exact fixed transcripts, not Norito archives", () => {
  const request = requestForAmount(5n);
  const { intent, authorization, ticket } = preTicketExchange(request);
  const closure = noCommitClosure(request, authorization, ticket);
  const intentTranscript = intentSemanticTranscript(intent);
  const authorizationTranscript = authorizationStatementSemanticTranscript(
    authorization.statement,
  );
  const closureTranscript = noCommitStatementSemanticTranscript(closure.statement);
  const reservation = new OfflineCashV1.OutboxReservation({
    reservationId: bytes(42),
    operationKind: "sendSplit",
    reservedOutboxBytes: OfflineCashV1.paymentOutboxMinimumBytes,
    issuedAtMs: 100n,
    expiresAtMs: 200n,
  });
  const reservationTranscript = outboxReservationSemanticTranscript(reservation);

  assert.equal(intentTranscript.length, 114);
  assert.equal(authorizationTranscript.length, 244);
  assert.equal(closureTranscript.length, 498);
  assert.equal(reservationTranscript.length, 56);
  assert.deepEqual(
    OfflineCashV1.acceptanceIntentDigest(intent),
    semanticDigest("iroha:offline-cash:v1:acceptance-intent", intentTranscript),
  );
  assert.deepEqual(
    OfflineCashV1.acceptanceAuthorizationStatementDigest(authorization.statement),
    semanticDigest(
      "iroha:offline-cash:v1:acceptance-intent-authorization-statement",
      authorizationTranscript,
    ),
  );
  assert.deepEqual(
    OfflineCashV1.noCommitClosureStatementDigest(closure.statement),
    semanticDigest(
      "iroha:offline-cash:v1:no-commit-closure-statement",
      closureTranscript,
    ),
  );
  assert.deepEqual(
    OfflineCashV1.outboxReservationCommitment(reservation),
    semanticDigest(
      "iroha:offline-cash:v1:outbox-reservation",
      reservationTranscript,
    ),
  );
  assert.throws(() => new OfflineCashV1.OutboxReservation({
    reservationId: bytes(42),
    operationKind: "sendSplit",
    reservedOutboxBytes: OfflineCashV1.paymentOutboxMinimumBytes - 1,
    issuedAtMs: 100n,
    expiresAtMs: 200n,
  }));

  const rawIntent = OfflineCashV1.encodeAcceptanceIntent(intent);
  assert.notEqual(Buffer.compare(Buffer.from(intentTranscript), Buffer.from(rawIntent)), 0);
});

test("typed credit opening, AAD, and envelope codecs are exact and bounded", () => {
  const opening = new OfflineCashV1.CreditOpening({
    version: 1,
    creditId: bytes(30),
    amount: 7n,
    creditCommitmentOpening: bytes(31),
    recipientBindingOpening: bytes(32),
    recoveryNonce: bytes(33),
  });
  const openingRaw = OfflineCashV1.encodeCreditOpening(opening);
  assert.equal(openingRaw.length, 200);
  assert.equal(OfflineCashV1.decodeCreditOpening(openingRaw, bytes(30), 7n).amount, 7n);
  assert.throws(() => OfflineCashV1.decodeCreditOpening(openingRaw, bytes(29), 7n));

  const aad = new OfflineCashV1.EncryptedCreditAad({
    version: 1,
    purpose: "peer",
    contextDigest: bytes(34),
    issuanceOrTransitionCommitment: bytes(35),
    creditId: bytes(30),
    amount: 7n,
  });
  const aadRaw = OfflineCashV1.encodeEncryptedCreditAad(aad);
  assert.deepEqual(OfflineCashV1.encodeEncryptedCreditAad(OfflineCashV1.decodeEncryptedCreditAad(aadRaw)), aadRaw);

  const envelope = new OfflineCashV1.EncryptedCreditEnvelope({
    version: 1,
    ephemeralX25519PublicKey: bytes(36),
    nonce: bytes(37, 24),
    ciphertextAndTag: bytes(38, 216),
  });
  const envelopeRaw = OfflineCashV1.encodeEncryptedCreditEnvelope(envelope, bytes(39));
  assert.ok(envelopeRaw.length <= 384);
  assert.deepEqual(
    OfflineCashV1.encodeEncryptedCreditEnvelope(OfflineCashV1.decodeEncryptedCreditEnvelope(envelopeRaw)),
    envelopeRaw,
  );
});

test("retired public state-link shapes and software money-crypto fallbacks are absent", () => {
  assert.throws(() => new OfflineCashV1.PairedProof({
    version: 1,
    eqProtocolDigest: bytes(1),
    epProtocolDigest: bytes(2),
    semanticDigest: bytes(3),
    guardEqCredentialAudit: bytes(4),
    guardEpCredentialAudit: bytes(5),
    eqDeferredAudit: bytes(6),
    epDeferredAudit: bytes(7),
    predecessorState: new OfflineCashV1.PastaStateCommitment({ eq: bytes(8), ep: bytes(9) }),
    successorState: new OfflineCashV1.PastaStateCommitment({ eq: bytes(10), ep: bytes(11) }),
    eqProof: bytes(12, 8),
    epProof: bytes(13, 8),
    eqHistory: bytes(14, 544),
    epHistory: bytes(15, 544),
  }));
  for (const name of ["encryptCredit", "decryptCredit", "provePayment", "signPayment", "softwareFallback", "drainStagedCredits"]) {
    assert.equal(Object.hasOwn(OfflineCashV1, name), false);
  }
  assert.equal(OfflineCashV1.completeExchangeTargetBytes, 16384);
  assert.equal(OfflineCashV1.maximumCompleteExchangeRawBytes, 18171);
  assert.equal(OfflineCashV1.maximumSessionRawBytes, 9211);
  assert.equal(OfflineCashV1.maximumSessionTextBytes, 12288);
});

test("native-generated canonical V1 fixture round-trips every required transported value", () => {
  assert.equal(fixture.fixture_version, 1);
  assert.equal(hasCanonicalFixture, true, "fixture_version 1 must contain the complete canonical key set");
  const raw = (name) => Uint8Array.from(Buffer.from(fixture[name].norito_hex, "hex"));
  for (const [name, kind] of Object.entries(fixtureTransportKinds)) {
    assert.deepEqual(Object.keys(fixture[name]).sort(), ["norito_hex", "oc1", "raw_bytes"]);
    assert.equal(raw(name).length, fixture[name].raw_bytes);
    assert.equal(OfflineCashV1.encodeText(kind, raw(name)), fixture[name].oc1);
  }
  assert.deepEqual(
    Object.keys(fixture.encrypted_credit_envelope).sort(),
    ["norito_hex", "raw_bytes", "recipient_x25519_public_key_hex"],
  );
  for (const name of ["encrypted_credit_aad", "credit_opening"]) {
    assert.deepEqual(Object.keys(fixture[name]).sort(), ["norito_hex", "raw_bytes"]);
  }
  for (const [name, [target, rawCap, textCap]] of Object.entries(fixtureSummaries)) {
    const summary = fixture[name];
    assert.deepEqual(Object.keys(summary).sort(), [
      "raw_bytes", "raw_hard_cap_bytes", "raw_target_bytes", "text_bytes",
      "text_hard_cap_bytes", "within_raw_hard_cap", "within_raw_target",
      "within_text_hard_cap",
    ]);
    assert.equal(summary.raw_target_bytes, target);
    assert.equal(summary.raw_hard_cap_bytes, rawCap);
    assert.equal(summary.text_hard_cap_bytes, textCap);
    assert.equal(summary.within_raw_target, summary.raw_bytes <= target);
    assert.equal(summary.within_raw_hard_cap, summary.raw_bytes <= rawCap);
    assert.equal(summary.within_text_hard_cap, summary.text_bytes <= textCap);
  }
  const request = OfflineCashV1.decodePaymentRequest(raw("payment_request"));
  const authorization = OfflineCashV1.decodeAcceptanceIntentAuthorization(
    raw("acceptance_intent_authorization"),
    request,
  );
  const ticket = OfflineCashV1.decodeAcceptanceTicket(
    raw("acceptance_ticket"),
    request,
    authorization.statement.intent,
  );
  const intentTranscript = intentSemanticTranscript(authorization.statement.intent);
  const authorizationTranscript = authorizationStatementSemanticTranscript(
    authorization.statement,
  );
  assert.equal(intentTranscript.length, 114);
  assert.equal(authorizationTranscript.length, 244);
  assert.deepEqual(
    ticket.intentDigest,
    semanticDigest("iroha:offline-cash:v1:acceptance-intent", intentTranscript),
  );
  assert.deepEqual(
    authorization.proof.semanticDigest,
    semanticDigest(
      "iroha:offline-cash:v1:acceptance-intent-authorization-statement",
      authorizationTranscript,
    ),
  );
  assert.deepEqual(
    Object.keys(fixture.no_commit_closure).sort(),
    ["norito_hex", "oc1", "raw_bytes"],
  );
  assert.equal(
    `oc1:${Buffer.from(raw("no_commit_closure")).toString("base64url")}`,
    fixture.no_commit_closure.oc1,
  );
  const decodedClosure = OfflineCashV1.decodeNoCommitClosure(raw("no_commit_closure"));
  assert.deepEqual(
    OfflineCashV1.encodeNoCommitClosure(decodedClosure),
    raw("no_commit_closure"),
  );
  const closureTranscript = noCommitStatementSemanticTranscript(decodedClosure.statement);
  assert.equal(closureTranscript.length, 498);
  assert.deepEqual(
    decodedClosure.proof.semanticDigest,
    semanticDigest(
      "iroha:offline-cash:v1:no-commit-closure-statement",
      closureTranscript,
    ),
  );
  const payment = OfflineCashV1.decodePayment(raw("payment"), request);
  const certificateIdTranscript = commitCertificateSemanticTranscript(
    payment.commitCertificate,
    false,
  );
  const certificateTranscript = commitCertificateSemanticTranscript(
    payment.commitCertificate,
    true,
  );
  assert.equal(commitEvidenceSemanticTranscript(payment.commitCertificate.commitEvidence).length, 36);
  assert.equal(certificateIdTranscript.length, 238);
  assert.equal(certificateTranscript.length, 270);
  assert.deepEqual(
    payment.commitCertificate.certificateId,
    semanticDigest(
      "iroha:offline-cash:v1:commit-certificate-id",
      certificateIdTranscript,
    ),
  );
  assert.deepEqual(
    payment.proof.commitCertificateDigest,
    semanticDigest(
      "iroha:offline-cash:v1:commit-certificate",
      certificateTranscript,
    ),
  );
  const acknowledgement = OfflineCashV1.decodeAcknowledgement(raw("acknowledgement"), request, payment);
  const mintAuthorization = OfflineCashV1.decodeMintAuthorization(raw("mint_authorization"));
  const mintCredit = OfflineCashV1.decodeMintCredit(raw("mint_credit"), mintAuthorization);
  const redemptionVoucher = OfflineCashV1.decodeRedemptionVoucher(raw("redemption_voucher"));
  assert.deepEqual(
    OfflineCashV1.encodeRedemptionVoucher(redemptionVoucher),
    raw("redemption_voucher"),
  );
  assert.deepEqual(
    OfflineCashV1.encodeEncryptedCreditEnvelope(OfflineCashV1.decodeEncryptedCreditEnvelope(
      raw("encrypted_credit_envelope"),
      Uint8Array.from(Buffer.from(fixture.encrypted_credit_envelope.recipient_x25519_public_key_hex, "hex")),
    )),
    raw("encrypted_credit_envelope"),
  );
  assert.deepEqual(
    OfflineCashV1.encodeEncryptedCreditAad(OfflineCashV1.decodeEncryptedCreditAad(raw("encrypted_credit_aad"))),
    raw("encrypted_credit_aad"),
  );
  assert.deepEqual(
    OfflineCashV1.encodeCreditOpening(OfflineCashV1.decodeCreditOpening(raw("credit_opening"))),
    raw("credit_opening"),
  );
  assert.equal(
    OfflineCashV1.validatePreTicketExchange(request, authorization, ticket),
    fixture.pre_ticket_exchange.raw_bytes,
  );
  assert.equal(
    OfflineCashV1.validateSession(request, payment, acknowledgement),
    fixture.terminal_trio.raw_bytes,
  );
  assert.equal(
    OfflineCashV1.validateCompleteExchange(request, authorization, ticket, payment, acknowledgement),
    fixture.complete_five_message.raw_bytes,
  );
  assert.equal(OfflineCashV1.validateMintCreditAgainstAuthorization(mintCredit, mintAuthorization), true);

  const mutatedLifecycleDigest = Uint8Array.from(
    payment.commitCertificate.lifecycleBindingDigest,
  );
  mutatedLifecycleDigest[0] ^= 1;
  const substitutedCertificate = new OfflineCashV1.CommitCertificate({
    version: payment.commitCertificate.version,
    certificateId: payment.commitCertificate.certificateId,
    candidateEnvelopeDigest: payment.commitCertificate.candidateEnvelopeDigest,
    lifecycleBindingDigest: mutatedLifecycleDigest,
    transitionNullifier: payment.commitCertificate.transitionNullifier,
    outboxReservationCommitment: payment.commitCertificate.outboxReservationCommitment,
    commitEvidence: payment.commitCertificate.commitEvidence,
    hardwareProfileId: payment.commitCertificate.hardwareProfileId,
    policyEpoch: payment.commitCertificate.policyEpoch,
    hardwareTerminalCommitment: payment.commitCertificate.hardwareTerminalCommitment,
  });
  const substitutedPayment = new OfflineCashV1.Payment({
    version: payment.version,
    statement: payment.statement,
    acceptanceIntent: payment.acceptanceIntent,
    acceptanceTicket: payment.acceptanceTicket,
    commitCertificate: substitutedCertificate,
    proof: payment.proof,
    encryptedCredit: payment.encryptedCredit,
    artifactManifestDigest: payment.artifactManifestDigest,
  });
  assert.throws(
    () => OfflineCashV1.encodePayment(substitutedPayment, request),
    /commit certificate lifecycle digest/,
  );

  const mutatedCertificateId = Uint8Array.from(payment.commitCertificate.certificateId);
  mutatedCertificateId[0] ^= 1;
  const certificateIdSubstitution = new OfflineCashV1.CommitCertificate({
    version: payment.commitCertificate.version,
    certificateId: mutatedCertificateId,
    candidateEnvelopeDigest: payment.commitCertificate.candidateEnvelopeDigest,
    lifecycleBindingDigest: payment.commitCertificate.lifecycleBindingDigest,
    transitionNullifier: payment.commitCertificate.transitionNullifier,
    outboxReservationCommitment: payment.commitCertificate.outboxReservationCommitment,
    commitEvidence: payment.commitCertificate.commitEvidence,
    hardwareProfileId: payment.commitCertificate.hardwareProfileId,
    policyEpoch: payment.commitCertificate.policyEpoch,
    hardwareTerminalCommitment: payment.commitCertificate.hardwareTerminalCommitment,
  });
  assert.throws(
    () => OfflineCashV1.encodePayment(new OfflineCashV1.Payment({
      version: payment.version,
      statement: payment.statement,
      acceptanceIntent: payment.acceptanceIntent,
      acceptanceTicket: payment.acceptanceTicket,
      commitCertificate: certificateIdSubstitution,
      proof: payment.proof,
      encryptedCredit: payment.encryptedCredit,
      artifactManifestDigest: payment.artifactManifestDigest,
    }), request),
    /commit certificate ID/,
  );

  const mutatedCertificateDigest = Uint8Array.from(payment.proof.commitCertificateDigest);
  mutatedCertificateDigest[0] ^= 1;
  const proofDigestSubstitution = new OfflineCashV1.CommitWrapperProof({
    version: payment.proof.version,
    eqProtocolDigest: payment.proof.eqProtocolDigest,
    epProtocolDigest: payment.proof.epProtocolDigest,
    semanticDigest: payment.proof.semanticDigest,
    candidateEnvelopeDigest: payment.proof.candidateEnvelopeDigest,
    commitCertificateDigest: mutatedCertificateDigest,
    eqDeferredAudit: payment.proof.eqDeferredAudit,
    epDeferredAudit: payment.proof.epDeferredAudit,
    eqProof: payment.proof.eqProof,
    epProof: payment.proof.epProof,
    eqHistory: payment.proof.eqHistory,
    epHistory: payment.proof.epHistory,
  });
  assert.throws(
    () => OfflineCashV1.encodePayment(new OfflineCashV1.Payment({
      version: payment.version,
      statement: payment.statement,
      acceptanceIntent: payment.acceptanceIntent,
      acceptanceTicket: payment.acceptanceTicket,
      commitCertificate: payment.commitCertificate,
      proof: proofDigestSubstitution,
      encryptedCredit: payment.encryptedCredit,
      artifactManifestDigest: payment.artifactManifestDigest,
    }), request),
    /commit wrapper certificate digest/,
  );

  const statement = redemptionVoucher.statement;
  const mutatedRedemptionId = Uint8Array.from(statement.redemptionId);
  mutatedRedemptionId[0] ^= 1;
  const substitutedRedemptionStatement = new OfflineCashV1.RedemptionStatement({
    version: statement.version,
    lifecycle: statement.lifecycle,
    amount: statement.amount,
    beneficiary: statement.beneficiary,
    terminalNullifier: statement.terminalNullifier,
    redemptionCommitment: statement.redemptionCommitment,
    redemptionId: mutatedRedemptionId,
    commitEvidence: statement.commitEvidence,
  });
  const substitutedVoucher = new OfflineCashV1.RedemptionVoucher({
    version: redemptionVoucher.version,
    statement: substitutedRedemptionStatement,
    commitCertificate: redemptionVoucher.commitCertificate,
    proof: redemptionVoucher.proof,
    artifactManifestDigest: redemptionVoucher.artifactManifestDigest,
  });
  assert.throws(
    () => OfflineCashV1.encodeRedemptionVoucher(substitutedVoucher),
    /redemption ID/,
  );
});
