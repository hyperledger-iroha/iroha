import CryptoKit
import Foundation
import XCTest

@testable import IrohaSwift

final class OfflineCashWireV1Tests: XCTestCase {
  func testCircuitBoundDigestsUseExactFixedSemanticTranscripts() throws {
    let closure = try noCommitClosure()
    let intent = closure.intentAuthorization.statement.intent
    var intentTranscript = littleEndian(intent.version)
    intentTranscript.append(intent.requestDigest)
    intentTranscript.append(intent.intentID)
    intentTranscript.append(intent.exactAmount.littleEndianBytes)
    intentTranscript.append(intent.senderOneTimeCommitment)
    XCTAssertEqual(intentTranscript.count, 114)
    XCTAssertEqual(
      semanticDigest("iroha:offline-cash:v1:acceptance-intent", intentTranscript),
      try OfflineCashNoritoV1.acceptanceIntentDigestShape(intent, against: closure.request))
    XCTAssertNotEqual(
      intentTranscript, try OfflineCashNoritoV1.encodeAcceptanceIntentShape(intent))

    let authorizationStatement = closure.intentAuthorization.statement
    var authorizationTranscript = littleEndian(authorizationStatement.version)
    authorizationTranscript.append(intentTranscript)
    authorizationTranscript.append(authorizationStatement.releaseID)
    authorizationTranscript.append(authorizationStatement.suiteID)
    authorizationTranscript.append(authorizationStatement.vkDigest)
    authorizationTranscript.append(authorizationStatement.artifactManifestDigest)
    XCTAssertEqual(authorizationTranscript.count, 244)
    XCTAssertEqual(
      semanticDigest(
        "iroha:offline-cash:v1:acceptance-intent-authorization-statement",
        authorizationTranscript),
      try OfflineCashNoritoV1.acceptanceIntentAuthorizationStatementDigestShape(
        authorizationStatement, against: closure.request))

    let statement = closure.statement
    var noCommitTranscript = littleEndian(statement.version)
    for value in [
      statement.releaseID, statement.suiteID, statement.vkDigest,
      statement.artifactManifestDigest, statement.senderHardwareBindingCommitment,
      statement.requestID, statement.requestDigest, statement.acceptanceTicketID,
      statement.ticketDigest, statement.intentAuthorizationDigest, statement.intentDigest,
    ] {
      noCommitTranscript.append(value)
    }
    noCommitTranscript.append(statement.exactAmount.littleEndianBytes)
    for value in [
      statement.senderOneTimeCommitment, statement.recoveryID,
      statement.cancellationNullifier, statement.equivalentDeliverySlotCommitment,
    ] {
      noCommitTranscript.append(value)
    }
    XCTAssertEqual(noCommitTranscript.count, 498)
    XCTAssertEqual(
      semanticDigest("iroha:offline-cash:v1:no-commit-closure-statement", noCommitTranscript),
      OfflineCashNoritoV1.noCommitClosureStatementDigestShape(statement))

    let reservation = try OfflineCashOutboxReservationV1(
      reservationID: digest(0xd0), operationKind: .sendSplit,
      reservedOutboxBytes: UInt32(OfflineCashWireV1.minimumPaymentOutboxBytes),
      issuedAtMS: 100, expiresAtMS: 200)
    var reservationTranscript = reservation.reservationID
    reservationTranscript.append(littleEndian(reservation.operationKind.rawValue))
    reservationTranscript.append(littleEndian(reservation.reservedOutboxBytes))
    reservationTranscript.append(littleEndian(reservation.issuedAtMS))
    reservationTranscript.append(littleEndian(reservation.expiresAtMS))
    XCTAssertEqual(reservationTranscript.count, 56)
    XCTAssertEqual(
      semanticDigest("iroha:offline-cash:v1:outbox-reservation", reservationTranscript),
      OfflineCashNoritoV1.outboxReservationCommitmentShape(reservation))
  }

  func testTerminalCertificateUsesExactFixedSemanticTranscripts() throws {
    let (request, payment) = try fixturePayment()
    let certificate = payment.commitCertificate
    let evidenceTranscript = commitEvidenceTranscript(certificate.commitEvidence)
    XCTAssertEqual(evidenceTranscript.count, 36)
    var certificateIDTranscript = littleEndian(certificate.version)
    certificateIDTranscript.append(certificate.candidateEnvelopeDigest)
    certificateIDTranscript.append(certificate.lifecycleBindingDigest)
    certificateIDTranscript.append(certificate.transitionNullifier)
    certificateIDTranscript.append(certificate.outboxReservationCommitment)
    certificateIDTranscript.append(evidenceTranscript)
    certificateIDTranscript.append(certificate.hardwareProfileID)
    certificateIDTranscript.append(littleEndian(certificate.policyEpoch))
    certificateIDTranscript.append(certificate.hardwareTerminalCommitment)
    XCTAssertEqual(certificateIDTranscript.count, 238)
    XCTAssertEqual(
      semanticDigest("iroha:offline-cash:v1:commit-certificate-id", certificateIDTranscript),
      certificate.certificateID)

    var certificateTranscript = littleEndian(certificate.version)
    certificateTranscript.append(certificate.certificateID)
    certificateTranscript.append(certificate.candidateEnvelopeDigest)
    certificateTranscript.append(certificate.lifecycleBindingDigest)
    certificateTranscript.append(certificate.transitionNullifier)
    certificateTranscript.append(certificate.outboxReservationCommitment)
    certificateTranscript.append(evidenceTranscript)
    certificateTranscript.append(certificate.hardwareProfileID)
    certificateTranscript.append(littleEndian(certificate.policyEpoch))
    certificateTranscript.append(certificate.hardwareTerminalCommitment)
    XCTAssertEqual(certificateTranscript.count, 270)
    XCTAssertEqual(
      semanticDigest("iroha:offline-cash:v1:commit-certificate", certificateTranscript),
      payment.proof.commitCertificateDigest)
    _ = try OfflineCashNoritoV1.encodePaymentShape(payment, against: request)
  }

  func testExactUnpaddedBase64URLRoundTrip() throws {
    let raw = Data([0xfb, 0xff, 0x00, 0x01])
    let text = try OfflineCashWireV1.encodeText(raw, kind: .payment)
    XCTAssertEqual(text, "oc1:-_8AAQ")
    XCTAssertEqual(try OfflineCashWireV1.decodeText(text, kind: .payment), raw)
  }

  func testNonCanonicalTextFailsClosed() {
    for text in ["OC1:-_8AAQ", "oc1:", "oc1:-_8AAQ==", "oc1:-_8A AQ", "oc1:A"] {
      XCTAssertThrowsError(try OfflineCashWireV1.decodeText(text, kind: .payment))
    }
  }

  func testEveryTransportedValueHasTheRustV1ExactBound() throws {
    for kind in OfflineCashWirePayloadKindV1.allCases {
      let exact = Data(repeating: 0xa5, count: kind.maximumRawBytes)
      let text = try OfflineCashWireV1.encodeText(exact, kind: kind)
      XCTAssertEqual(text.utf8.count, kind.maximumTextBytes)
      XCTAssertEqual(try OfflineCashWireV1.decodeText(text, kind: kind), exact)
      XCTAssertThrowsError(
        try OfflineCashWireV1.encodeText(
          Data(repeating: 0xa5, count: kind.maximumRawBytes + 1), kind: kind))
    }
  }

  func testFiveMessageAndRecoverableCapacityConstantsMatchV1() {
    XCTAssertEqual(OfflineCashWireV1.maximumPaymentRequestBytes, 1_024)
    XCTAssertEqual(OfflineCashWireV1.maximumPaymentRequestTextBytes, 1_370)
    XCTAssertEqual(OfflineCashWireV1.maximumAcceptanceIntentBytes, 256)
    XCTAssertEqual(OfflineCashWireV1.maximumAcceptanceIntentAuthorizationBytes, 7_936)
    XCTAssertEqual(OfflineCashWireV1.maximumAcceptanceTicketBytes, 1_024)
    XCTAssertEqual(OfflineCashWireV1.maximumPreTicketExchangeBytes, 9_984)
    XCTAssertEqual(OfflineCashWireV1.maximumPreTicketTextExchangeBytes, 13_326)
    XCTAssertEqual(OfflineCashWireV1.maximumCompleteExchangeBytes, 18_171)
    XCTAssertEqual(OfflineCashWireV1.minimumReservedInboxBytes, 8_960)
    XCTAssertEqual(OfflineCashWireV1.minimumPaymentOutboxBytes, 26_112)
    XCTAssertEqual(OfflineCashWireV1.minimumRedemptionOutboxBytes, 26_112)
    XCTAssertEqual(OfflineCashWireV1.requiredHardwareCapabilityMask, 0xffff)
  }

  func testAcceptanceIntentCanonicalRoundTripHasNoSenderIdentityOrStateLink() throws {
    let value = try OfflineCashAcceptanceIntentV1(
      requestDigest: digest(1), intentID: digest(2), exactAmount: OfflineCashUInt128V1(42),
      senderOneTimeCommitment: digest(3))
    let encoded = try OfflineCashNoritoV1.encodeAcceptanceIntentShape(value)
    XCTAssertEqual(try OfflineCashNoritoV1.decodeAcceptanceIntentShapeExact(encoded), value)
    XCTAssertLessThanOrEqual(encoded.count, OfflineCashWireV1.maximumAcceptanceIntentBytes)
  }

  func testTypedEncryptedCreditComponentsRoundTripWithoutSwiftCryptography() throws {
    let opening = try OfflineCashCreditOpeningV1(
      creditID: digest(4), amount: OfflineCashUInt128V1(99),
      creditCommitmentOpening: digest(5), recipientBindingOpening: digest(6),
      recoveryNonce: digest(7))
    let openingBytes = try OfflineCashNoritoV1.encodeCreditOpeningShape(opening)
    XCTAssertEqual(try OfflineCashNoritoV1.decodeCreditOpeningShapeExact(openingBytes), opening)

    let aad = try OfflineCashEncryptedCreditAADV1(
      purpose: .peer, contextDigest: digest(8),
      issuanceOrTransitionCommitment: digest(9), creditID: opening.creditID,
      amount: opening.amount)
    let aadBytes = try OfflineCashNoritoV1.encodeEncryptedCreditAADShape(aad)
    XCTAssertEqual(try OfflineCashNoritoV1.decodeEncryptedCreditAADShapeExact(aadBytes), aad)

    let envelope = try OfflineCashEncryptedCreditEnvelopeV1(
      ephemeralX25519PublicKey: OfflineCashX25519PublicKeyV1(rawBytes: digest(10)),
      nonce: Data(repeating: 0x42, count: 24),
      ciphertextAndTag: Data(repeating: 0x43, count: openingBytes.count + 16))
    let envelopeBytes = try OfflineCashNoritoV1.encodeEncryptedCreditEnvelopeShape(envelope)
    XCTAssertEqual(
      try OfflineCashNoritoV1.decodeEncryptedCreditEnvelopeShapeExact(envelopeBytes), envelope)
    XCTAssertLessThanOrEqual(envelopeBytes.count, OfflineCashWireV1.maximumEncryptedCreditBytes)
  }

  func testNoCommitClosureRoundTripsWithExactRequestAuthorizationAndTicketBindings() throws {
    let closure = try noCommitClosure()
    let encoded = try OfflineCashNoritoV1.encodeNoCommitClosureShape(closure)
    let decoded = try OfflineCashNoritoV1.decodeNoCommitClosureShapeExact(encoded)
    XCTAssertEqual(try OfflineCashNoritoV1.encodeNoCommitClosureShape(decoded), encoded)
    XCTAssertEqual(try OfflineCashNoritoV1.noCommitClosureDigestShape(decoded).count, 32)
    XCTAssertLessThanOrEqual(encoded.count, OfflineCashWireV1.maximumNoCommitClosureBytes)
    XCTAssertThrowsError(
      try OfflineCashNoritoV1.decodeNoCommitClosureShapeExact(
        Data(repeating: 1, count: OfflineCashWireV1.maximumNoCommitClosureBytes + 1)))

    let names = Mirror(reflecting: closure.statement).children.compactMap(\.label)
      .map { $0.lowercased() }
    for forbidden in [
      "predecessor", "successor", "statecommitment", "beforesequence", "aftersequence",
    ] {
      XCTAssertFalse(names.contains(where: { $0.contains(forbidden) }))
    }

    let statement = closure.statement
    let substituted = try OfflineCashNoCommitClosureStatementV1(
      releaseID: statement.releaseID, suiteID: statement.suiteID,
      vkDigest: statement.vkDigest,
      artifactManifestDigest: statement.artifactManifestDigest,
      senderHardwareBindingCommitment: statement.senderHardwareBindingCommitment,
      requestID: digest(0xee), requestDigest: statement.requestDigest,
      acceptanceTicketID: statement.acceptanceTicketID,
      ticketDigest: statement.ticketDigest,
      intentAuthorizationDigest: statement.intentAuthorizationDigest,
      intentDigest: statement.intentDigest, exactAmount: statement.exactAmount,
      senderOneTimeCommitment: statement.senderOneTimeCommitment,
      recoveryID: statement.recoveryID,
      cancellationNullifier: statement.cancellationNullifier,
      equivalentDeliverySlotCommitment: statement.equivalentDeliverySlotCommitment)
    XCTAssertThrowsError(
      try OfflineCashNoritoV1.encodeNoCommitClosureShape(
        OfflineCashNoCommitClosureV1(
          statement: substituted, request: closure.request,
          intentAuthorization: closure.intentAuthorization,
          acceptanceTicket: closure.acceptanceTicket, proof: closure.proof)))
  }

  func testGeneratedFixtureV1RoundTripsEveryCanonicalPayloadExactly() throws {
    let fixtureURL = URL(fileURLWithPath: #filePath)
      .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
      .deletingLastPathComponent()
      .appendingPathComponent("fixtures/offline/offline_cash_v1.json")
    let root = try XCTUnwrap(
      JSONSerialization.jsonObject(with: Data(contentsOf: fixtureURL)) as? [String: Any])
    XCTAssertEqual(root["fixture_version"] as? Int, 1)

    let payloadNames: Set<String> = [
      "payment_request", "acceptance_intent_authorization", "acceptance_ticket", "payment",
      "acknowledgement", "mint_authorization", "mint_credit", "redemption_voucher",
      "credit_opening", "encrypted_credit_aad", "encrypted_credit_envelope",
      "no_commit_closure",
    ]
    let fixturePayloadNames = Set(
      root.compactMap { name, value -> String? in
        guard let entry = value as? [String: Any], entry["norito_hex"] != nil else { return nil }
        return name
      })
    XCTAssertEqual(fixturePayloadNames, payloadNames)

    let request: OfflineCashPaymentRequestV1 = try assertFixtureRoundTrip(
      "payment_request", in: root,
      decoder: OfflineCashNoritoV1.decodePaymentRequestShapeExact,
      encoder: OfflineCashNoritoV1.encodePaymentRequestShape)
    let _: OfflineCashAcceptanceIntentAuthorizationV1 = try assertFixtureRoundTrip(
      "acceptance_intent_authorization", in: root,
      decoder: OfflineCashNoritoV1.decodeAcceptanceIntentAuthorizationShapeExact,
      encoder: OfflineCashNoritoV1.encodeAcceptanceIntentAuthorizationShape)
    let _: OfflineCashAcceptanceTicketV1 = try assertFixtureRoundTrip(
      "acceptance_ticket", in: root,
      decoder: OfflineCashNoritoV1.decodeAcceptanceTicketShapeExact,
      encoder: OfflineCashNoritoV1.encodeAcceptanceTicketShape)
    let payment: OfflineCashPaymentV1 = try assertFixtureRoundTrip(
      "payment", in: root,
      decoder: { try OfflineCashNoritoV1.decodePaymentShapeExact($0, against: request) },
      encoder: { try OfflineCashNoritoV1.encodePaymentShape($0, against: request) })
    let acknowledgement: OfflineCashAcknowledgementV1 = try assertFixtureRoundTrip(
      "acknowledgement", in: root,
      decoder: {
        try OfflineCashNoritoV1.decodeAcknowledgementShapeExact(
          $0, against: request, payment: payment)
      },
      encoder: {
        try OfflineCashNoritoV1.encodeAcknowledgementShape(
          $0, against: request, payment: payment)
      })
    let _: OfflineCashMintAuthorizationV1 = try assertFixtureRoundTrip(
      "mint_authorization", in: root,
      decoder: OfflineCashNoritoV1.decodeMintAuthorizationShapeExact,
      encoder: OfflineCashNoritoV1.encodeMintAuthorizationShape)
    let _: OfflineCashMintCreditV1 = try assertFixtureRoundTrip(
      "mint_credit", in: root,
      decoder: OfflineCashNoritoV1.decodeMintCreditShapeExact,
      encoder: OfflineCashNoritoV1.encodeMintCreditShape)
    let redemption: OfflineCashRedemptionVoucherV1 = try assertFixtureRoundTrip(
      "redemption_voucher", in: root,
      decoder: OfflineCashNoritoV1.decodeRedemptionVoucherShapeExact,
      encoder: OfflineCashNoritoV1.encodeRedemptionVoucherShape)
    let _: OfflineCashCreditOpeningV1 = try assertFixtureRoundTrip(
      "credit_opening", in: root,
      decoder: OfflineCashNoritoV1.decodeCreditOpeningShapeExact,
      encoder: OfflineCashNoritoV1.encodeCreditOpeningShape)
    let _: OfflineCashEncryptedCreditAADV1 = try assertFixtureRoundTrip(
      "encrypted_credit_aad", in: root,
      decoder: OfflineCashNoritoV1.decodeEncryptedCreditAADShapeExact,
      encoder: OfflineCashNoritoV1.encodeEncryptedCreditAADShape)
    let _: OfflineCashEncryptedCreditEnvelopeV1 = try assertFixtureRoundTrip(
      "encrypted_credit_envelope", in: root,
      decoder: OfflineCashNoritoV1.decodeEncryptedCreditEnvelopeShapeExact,
      encoder: OfflineCashNoritoV1.encodeEncryptedCreditEnvelopeShape)
    let _: OfflineCashNoCommitClosureV1 = try assertFixtureRoundTrip(
      "no_commit_closure", in: root,
      decoder: OfflineCashNoritoV1.decodeNoCommitClosureShapeExact,
      encoder: OfflineCashNoritoV1.encodeNoCommitClosureShape)

    var substitutedPaymentDigest = acknowledgement.paymentDigest
    substitutedPaymentDigest[0] ^= 1
    let substitutedAcknowledgement = try OfflineCashAcknowledgementV1(
      requestDigest: acknowledgement.requestDigest,
      paymentDigest: substitutedPaymentDigest,
      inboxReceipt: acknowledgement.inboxReceipt,
      signature: acknowledgement.signature)
    XCTAssertThrowsError(
      try OfflineCashNoritoV1.encodeAcknowledgementShape(
        substitutedAcknowledgement, against: request, payment: payment))

    var substitutedCertificateID = payment.commitCertificate.certificateID
    substitutedCertificateID[0] ^= 1
    let substitutedCertificate = try OfflineCashCommitCertificateV1(
      certificateID: substitutedCertificateID,
      candidateEnvelopeDigest: payment.commitCertificate.candidateEnvelopeDigest,
      lifecycleBindingDigest: payment.commitCertificate.lifecycleBindingDigest,
      transitionNullifier: payment.commitCertificate.transitionNullifier,
      outboxReservationCommitment: payment.commitCertificate.outboxReservationCommitment,
      commitEvidence: payment.commitCertificate.commitEvidence,
      hardwareProfileID: payment.commitCertificate.hardwareProfileID,
      policyEpoch: payment.commitCertificate.policyEpoch,
      hardwareTerminalCommitment: payment.commitCertificate.hardwareTerminalCommitment)
    let certificateSubstitutedPayment = try OfflineCashPaymentV1(
      statement: payment.statement, acceptanceIntent: payment.acceptanceIntent,
      acceptanceTicket: payment.acceptanceTicket, commitCertificate: substitutedCertificate,
      proof: payment.proof, encryptedCredit: payment.encryptedCredit,
      artifactManifestDigest: payment.artifactManifestDigest)
    XCTAssertThrowsError(
      try OfflineCashNoritoV1.encodePaymentShape(certificateSubstitutedPayment, against: request))

    var substitutedCertificateDigest = payment.proof.commitCertificateDigest
    substitutedCertificateDigest[0] ^= 1
    let substitutedProof = try OfflineCashCommitWrapperProofV1(
      eqProtocolDigest: payment.proof.eqProtocolDigest,
      epProtocolDigest: payment.proof.epProtocolDigest,
      semanticDigest: payment.proof.semanticDigest,
      candidateEnvelopeDigest: payment.proof.candidateEnvelopeDigest,
      commitCertificateDigest: substitutedCertificateDigest,
      eqDeferredAudit: payment.proof.eqDeferredAudit,
      epDeferredAudit: payment.proof.epDeferredAudit,
      eqProof: payment.proof.eqProof, epProof: payment.proof.epProof,
      eqHistory: payment.proof.eqHistory, epHistory: payment.proof.epHistory)
    let proofSubstitutedPayment = try OfflineCashPaymentV1(
      statement: payment.statement, acceptanceIntent: payment.acceptanceIntent,
      acceptanceTicket: payment.acceptanceTicket, commitCertificate: payment.commitCertificate,
      proof: substitutedProof, encryptedCredit: payment.encryptedCredit,
      artifactManifestDigest: payment.artifactManifestDigest)
    XCTAssertThrowsError(
      try OfflineCashNoritoV1.encodePaymentShape(proofSubstitutedPayment, against: request))

    var substitutedRedemptionID = redemption.statement.redemptionID
    substitutedRedemptionID[0] ^= 1
    let substitutedRedemptionStatement = try OfflineCashRedemptionStatementV1(
      lifecycle: redemption.statement.lifecycle, amount: redemption.statement.amount,
      beneficiary: redemption.statement.beneficiary,
      terminalNullifier: redemption.statement.terminalNullifier,
      redemptionCommitment: redemption.statement.redemptionCommitment,
      redemptionID: substitutedRedemptionID,
      commitEvidence: redemption.statement.commitEvidence)
    let substitutedRedemption = try OfflineCashRedemptionVoucherV1(
      statement: substitutedRedemptionStatement,
      commitCertificate: redemption.commitCertificate, proof: redemption.proof,
      artifactManifestDigest: redemption.artifactManifestDigest)
    XCTAssertThrowsError(
      try OfflineCashNoritoV1.encodeRedemptionVoucherShape(substitutedRedemption))
  }

  private func assertFixtureRoundTrip<Value>(
    _ name: String, in root: [String: Any],
    decoder: (Data) throws -> Value, encoder: (Value) throws -> Data
  ) throws -> Value {
    let entry = try XCTUnwrap(root[name] as? [String: Any], "missing fixture payload \(name)")
    let bytes = try XCTUnwrap(
      Data(hexString: try XCTUnwrap(entry["norito_hex"] as? String)),
      "invalid fixture hex for \(name)")
    XCTAssertEqual(entry["raw_bytes"] as? Int, bytes.count, name)
    if let fixtureText = entry["oc1"] as? String {
      let canonicalText =
        "oc1:"
        + bytes.base64EncodedString()
        .replacingOccurrences(of: "+", with: "-")
        .replacingOccurrences(of: "/", with: "_")
        .replacingOccurrences(of: "=", with: "")
      XCTAssertEqual(fixtureText, canonicalText, name)
    }
    let value = try decoder(bytes)
    XCTAssertEqual(try encoder(value), bytes, name)
    return value
  }

  private func fixturePayment() throws -> (OfflineCashPaymentRequestV1, OfflineCashPaymentV1) {
    let fixtureURL = URL(fileURLWithPath: #filePath)
      .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
      .deletingLastPathComponent()
      .appendingPathComponent("fixtures/offline/offline_cash_v1.json")
    let root = try XCTUnwrap(
      JSONSerialization.jsonObject(with: Data(contentsOf: fixtureURL)) as? [String: Any])
    let requestEntry = try XCTUnwrap(root["payment_request"] as? [String: Any])
    let paymentEntry = try XCTUnwrap(root["payment"] as? [String: Any])
    let requestBytes = try XCTUnwrap(
      Data(hexString: try XCTUnwrap(requestEntry["norito_hex"] as? String)))
    let paymentBytes = try XCTUnwrap(
      Data(hexString: try XCTUnwrap(paymentEntry["norito_hex"] as? String)))
    let request = try OfflineCashNoritoV1.decodePaymentRequestShapeExact(requestBytes)
    let payment = try OfflineCashNoritoV1.decodePaymentShapeExact(
      paymentBytes, against: request)
    return (request, payment)
  }

  private func semanticDigest(_ domain: String, _ transcript: Data) -> Data {
    var preimage = Data(domain.utf8)
    preimage.append(0)
    preimage.append(littleEndian(UInt64(transcript.count)))
    preimage.append(transcript)
    return Data(SHA256.hash(data: preimage))
  }

  private func commitEvidenceTranscript(_ evidence: OfflineCashCommitEvidenceV1) -> Data {
    var transcript = Data()
    switch evidence {
    case .trustedTime(let commitment):
      transcript.append(littleEndian(UInt32(0)))
      transcript.append(commitment)
    case .monotonicLease(let commitment):
      transcript.append(littleEndian(UInt32(1)))
      transcript.append(commitment)
    }
    return transcript
  }

  private func littleEndian(_ value: UInt16) -> Data {
    var value = value.littleEndian
    return withUnsafeBytes(of: &value) { Data($0) }
  }

  private func littleEndian(_ value: UInt32) -> Data {
    var value = value.littleEndian
    return withUnsafeBytes(of: &value) { Data($0) }
  }

  private func littleEndian(_ value: UInt64) -> Data {
    var value = value.littleEndian
    return withUnsafeBytes(of: &value) { Data($0) }
  }

  private func digest(_ byte: UInt8) -> Data {
    var value = Data(repeating: byte, count: 32)
    value[31] |= 1
    return value
  }

  private func noCommitClosure() throws -> OfflineCashNoCommitClosureV1 {
    let networkID = digest(0x11)
    let releaseID = digest(0x12)
    let profileID = digest(0x13)
    let suiteID = digest(0x14)
    let publicKey = try OfflineCashDevicePublicKeyV1(
      sec1Bytes: Data([4]) + Data(repeating: 1, count: 64))
    var signatureBytes = Data(repeating: 0, count: 64)
    signatureBytes[31] = 1
    signatureBytes[63] = 1
    let signature = try OfflineCashDeviceSignatureV1(rawBytes: signatureBytes)
    let credential = try OfflineCashHardwareCredentialV1(
      credentialID: digest(0x15), networkID: networkID,
      hardwareProfileID: profileID, suiteID: suiteID,
      firmwarePolicyDigest: digest(0x16), policyEpoch: 1,
      laneCommitment: digest(0x17), hardwareEpochID: digest(0x18),
      hardwareEpochGeneration: 1, devicePublicKey: publicKey,
      deviceKeyReference: digest(0x19), issuedAtMS: 10, expiresAtMS: 9_000,
      governanceSignature: signature)
    let asset = try OfflineCashAssetDefinitionIDV1("6TEAJqbb8oEPmLncoNiMRbLEK6tw")
    let incarnation = try OfflineCashAssetIncarnationV1(bytes: digest(0x21))
    let account = try OfflineCashAccountIDV1(
      "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV")
    let amount = OfflineCashUInt128V1(42)
    let request = try OfflineCashPaymentRequestV1(
      releaseID: releaseID, networkID: networkID, asset: asset,
      assetIncarnation: incarnation, scale: 4, liabilityPoolID: digest(0x22),
      recipient: account, amount: amount, hardwareCredential: credential,
      requestID: digest(0x23), issuedAtMS: 100, expiresAtMS: 200,
      signature: signature)
    let requestDigest = try OfflineCashNoritoV1.paymentRequestDigestShape(request)
    let intent = try OfflineCashAcceptanceIntentV1(
      requestDigest: requestDigest, intentID: digest(0x24),
      exactAmount: OfflineCashUInt128V1(42), senderOneTimeCommitment: digest(0x25))
    let authorizationStatement = try OfflineCashAcceptanceIntentAuthorizationStatementV1(
      intent: intent, releaseID: releaseID, suiteID: suiteID,
      vkDigest: digest(0x26), artifactManifestDigest: digest(0x27))
    let authorization = try OfflineCashAcceptanceIntentAuthorizationV1(
      statement: authorizationStatement,
      proof: pairedProof(
        semanticDigest: OfflineCashNoritoV1.acceptanceIntentAuthorizationStatementDigestShape(
          authorizationStatement, against: request), tag: 0x30))
    let ticket = try OfflineCashAcceptanceTicketV1(
      networkID: networkID, requestID: request.requestID, requestDigest: requestDigest,
      acceptanceTicketID: digest(0x28), asset: asset, assetIncarnation: incarnation,
      scale: 4,
      intentDigest: OfflineCashNoritoV1.acceptanceIntentDigestShape(intent, against: request),
      exactAmount: intent.exactAmount, reservedInboxBytes: 8_960,
      recipientOneTimeKey: OfflineCashX25519PublicKeyV1(rawBytes: digest(0x29)),
      hardwareProfileID: profileID, policyEpoch: 1, issuedAtMS: 110,
      expiresAtMS: 190, signature: signature)
    let statement = try OfflineCashNoCommitClosureStatementV1(
      releaseID: releaseID, suiteID: suiteID, vkDigest: authorizationStatement.vkDigest,
      artifactManifestDigest: authorizationStatement.artifactManifestDigest,
      senderHardwareBindingCommitment: digest(0x2a), requestID: request.requestID,
      requestDigest: requestDigest, acceptanceTicketID: ticket.acceptanceTicketID,
      ticketDigest: OfflineCashNoritoV1.acceptanceTicketDigestShape(
        ticket, against: request, authorization: authorization),
      intentAuthorizationDigest: OfflineCashNoritoV1.acceptanceIntentAuthorizationDigestShape(
        authorization, against: request),
      intentDigest: OfflineCashNoritoV1.acceptanceIntentDigestShape(intent, against: request),
      exactAmount: intent.exactAmount,
      senderOneTimeCommitment: intent.senderOneTimeCommitment,
      recoveryID: digest(0x2b), cancellationNullifier: digest(0x2c),
      equivalentDeliverySlotCommitment: digest(0x2d))
    return try OfflineCashNoCommitClosureV1(
      statement: statement, request: request, intentAuthorization: authorization,
      acceptanceTicket: ticket,
      proof: pairedProof(
        semanticDigest: OfflineCashNoritoV1.noCommitClosureStatementDigestShape(statement),
        tag: 0x40))
  }

  private func pairedProof(semanticDigest: Data, tag: UInt8) throws
    -> OfflineCashPairedProofV1
  {
    try OfflineCashPairedProofV1(
      eqProtocolDigest: digest(tag), epProtocolDigest: digest(tag &+ 1),
      semanticDigest: semanticDigest, guardEqCredentialAudit: digest(tag &+ 2),
      guardEpCredentialAudit: digest(tag &+ 3), eqDeferredAudit: digest(tag &+ 4),
      epDeferredAudit: digest(tag &+ 5), eqProof: Data([tag]),
      epProof: Data([tag &+ 1]),
      eqHistory: Data(repeating: tag, count: OfflineCashWireV1.historyAccumulatorBytes),
      epHistory: Data(repeating: tag &+ 1, count: OfflineCashWireV1.historyAccumulatorBytes))
  }
}
