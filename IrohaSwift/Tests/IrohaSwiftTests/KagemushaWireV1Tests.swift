import CryptoKit
import XCTest

@testable import IrohaSwift

final class KagemushaWireV1Tests: XCTestCase {
  func testRustCanonicalFiveMessageFixtureRoundTripsByteForByte() throws {
    let fixture = try loadCanonicalFixture()
    XCTAssertEqual(try XCTUnwrap(fixture["fixture_version"] as? Int), 1)
    XCTAssertEqual(try XCTUnwrap(fixture["protocol"] as? String), "KAGEMUSHA")
    XCTAssertEqual(try XCTUnwrap(fixture["text_prefix"] as? String), "kgm1:")

    let expectedOrder:
      [(
        section: String, name: String, tag: Int,
        wireKind: IrohaPeerWireKindV1,
        textKind: KagemushaWirePayloadKindV1
      )] = [
        ("payment_request", "request", 1, .request, .paymentRequest),
        (
          "acceptance_intent", "intent", 2, .intent,
          .acceptanceIntent
        ),
        ("acceptance_ticket", "ticket", 3, .ticket, .acceptanceTicket),
        ("payment", "payment", 4, .payment, .payment),
        ("acknowledgement", "acknowledgement", 5, .acknowledgement, .acknowledgement),
      ]
    let order = try XCTUnwrap(fixture["ipm1_message_order"] as? [[String: Any]])
    XCTAssertEqual(order.count, expectedOrder.count)

    var sections: [String: Data] = [:]
    for (index, expected) in expectedOrder.enumerated() {
      XCTAssertEqual(try XCTUnwrap(order[index]["kind"] as? String), expected.name)
      XCTAssertEqual(try XCTUnwrap(order[index]["tag"] as? Int), expected.tag)
      XCTAssertEqual(expected.wireKind.rawValue, UInt8(expected.tag))

      let section = try XCTUnwrap(fixture[expected.section] as? [String: Any])
      XCTAssertEqual(try XCTUnwrap(section["ipm1_kind"] as? Int), expected.tag)
      let raw = try fixtureHex(section)
      sections[expected.section] = raw
      XCTAssertEqual(raw.count, try XCTUnwrap(section["raw_bytes"] as? Int))
      XCTAssertEqual(
        SHA256.hash(data: raw).map { String(format: "%02x", $0) }.joined(),
        try XCTUnwrap(section["sha256"] as? String))
      let text = try XCTUnwrap(section["kgm1"] as? String)
      XCTAssertTrue(text.hasPrefix("kgm1:"))
      XCTAssertEqual(try KagemushaWireV1.decodeText(text, kind: expected.textKind), raw)
      XCTAssertEqual(try KagemushaWireV1.encodeText(raw, kind: expected.textKind), text)

      let message = try IrohaPeerWireMessageV1(
        profile: .kagemushaV1,
        kind: expected.wireKind,
        schemaVersion: 1,
        canonicalPayload: raw)
      XCTAssertEqual(message.encoded[8], UInt8(expected.tag))
      XCTAssertEqual(
        try IrohaPeerWireMessageV1.decode(
          message.encoded, expectedProfile: .kagemushaV1,
          expectedKind: expected.wireKind
        ).canonicalPayload,
        raw)
    }

    let requestRaw = try XCTUnwrap(sections["payment_request"])
    let intentRaw = try XCTUnwrap(sections["acceptance_intent"])
    let ticketRaw = try XCTUnwrap(sections["acceptance_ticket"])
    let paymentRaw = try XCTUnwrap(sections["payment"])
    let acknowledgementRaw = try XCTUnwrap(sections["acknowledgement"])
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(requestRaw)
    let intent = try KagemushaNoritoV1.decodeAcceptanceIntentShapeExact(
      intentRaw, against: request)
    let ticket = try KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
      ticketRaw, against: request, intent: intent)
    let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
      paymentRaw, against: request, intent: intent, ticket: ticket)
    let acknowledgement = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(
      acknowledgementRaw, against: request, intent: intent,
      ticket: ticket, payment: payment)
    let proofRaw = try fixtureHex(XCTUnwrap(fixture["payment_proof"] as? [String: Any]))
    let certificateRaw = try fixtureHex(XCTUnwrap(fixture["commit_certificate"] as? [String: Any]))
    XCTAssertEqual(try KagemushaNoritoV1.encodePaymentProofShape(payment.proof), proofRaw)
    XCTAssertEqual(try KagemushaNoritoV1.decodePaymentProofShapeExact(proofRaw), payment.proof)
    XCTAssertEqual(
      try KagemushaNoritoV1.encodeCommitCertificateShape(payment.commitCertificate), certificateRaw)
    XCTAssertEqual(
      try KagemushaNoritoV1.decodeCommitCertificateShapeExact(certificateRaw),
      payment.commitCertificate)
    let identities = try XCTUnwrap(fixture["identity_vectors"] as? [String: Any])
    for (key, actual) in [
      ("payment_request_digest_hex", KagemushaNoritoV1.paymentRequestDigest(request)),
      (
        "acceptance_ticket_digest_hex",
        try KagemushaNoritoV1.acceptanceTicketDigestShape(
          ticket, against: request, intent: intent)
      ),
      ("payment_output_digest_hex", KagemushaNoritoV1.paymentOutputDigestShape(payment.output)),
      ("ciphertext_digest_hex", KagemushaNoritoV1.ciphertextDigestShape(payment.encryptedCredit)),
      (
        "payment_body_digest_hex",
        try KagemushaNoritoV1.paymentBodyDigestShape(
          output: payment.output, encryptedCredit: payment.encryptedCredit)
      ),
    ] {
      XCTAssertEqual(
        actual, try XCTUnwrap(Data(hexString: XCTUnwrap(identities[key] as? String))), key)
    }
    let normalized = try XCTUnwrap(fixture["identity_normalization"] as? [String: Any])
    for (name, payload, digest, type, alignment) in [
      (
        "asset", request.asset.canonicalPayload,
        KagemushaNoritoV1.assetIdentityDigestShape(request.asset),
        "iroha_data_model::asset::id::model::AssetDefinitionId", 1
      ),
      (
        "account", request.recipient.canonicalPayload,
        KagemushaNoritoV1.accountIdentityDigestShape(request.recipient),
        "iroha_data_model::account::model::AccountId", 8
      ),
    ] {
      let entry = try XCTUnwrap(normalized[name] as? [String: Any])
      XCTAssertEqual(entry["rust_type_name"] as? String, type)
      XCTAssertEqual(entry["rust_alignment_bytes"] as? Int, alignment)
      XCTAssertEqual(
        try fixtureHex(entry),
        noritoEncode(
          typeName: type, payload: payload, flags: NoritoHeader.compactLen,
          payloadAlignment: alignment))
      XCTAssertEqual(
        digest,
        try XCTUnwrap(
          Data(
            hexString: XCTUnwrap(
              entry["canonical_digest_hex"] as? String))))
    }
    let transcripts = try XCTUnwrap(fixture["semantic_transcripts"] as? [String: Any])
    for (name, signingBytes, unsignedLength) in [
      ("payment_request", try KagemushaNoritoV1.paymentRequestSigningBytesShape(request), 315),
      (
        "acceptance_ticket",
        try KagemushaNoritoV1.acceptanceTicketSigningBytesShape(
          ticket, against: request, intent: intent), 150
      ),
    ] {
      let entry = try XCTUnwrap(transcripts[name] as? [String: Any])
      XCTAssertEqual(entry["unsigned_transcript_bytes"] as? Int, unsignedLength)
      XCTAssertEqual(
        signingBytes,
        try XCTUnwrap(
          Data(
            hexString: XCTUnwrap(
              entry["signing_bytes_hex"] as? String))))
    }
    for (name, expectedLength) in [
      ("payment_request", 379), ("acceptance_intent", 114), ("acceptance_ticket", 214),
      ("prepared_transfer", 210), ("payment_output", 198), ("payment_body", 64),
    ] {
      let entry = try XCTUnwrap(transcripts[name] as? [String: Any])
      let transcript = try XCTUnwrap(Data(hexString: XCTUnwrap(entry["transcript_hex"] as? String)))
      XCTAssertEqual(transcript.count, expectedLength)
      XCTAssertEqual(entry["transcript_bytes"] as? Int, expectedLength)
      XCTAssertEqual(
        transcriptDigest(try XCTUnwrap(entry["digest_domain"] as? String), transcript),
        try XCTUnwrap(Data(hexString: XCTUnwrap(entry["canonical_digest_hex"] as? String))))
    }
    XCTAssertEqual(
      try KagemushaNoritoV1.acceptanceIntentDigestShape(
        intent, against: request),
      try XCTUnwrap(
        Data(
          hexString: XCTUnwrap(
            identities["acceptance_intent_digest_hex"] as? String))))
    XCTAssertEqual(
      payment.output.transitionNullifier,
      try XCTUnwrap(
        Data(
          hexString: XCTUnwrap(
            identities["transition_nullifier_hex"] as? String))))
    XCTAssertEqual(
      try KagemushaNoritoV1.preparedTransferDigestShape(
        request: request, intent: intent, ticket: ticket,
        transitionNullifier: payment.output.transitionNullifier,
        ciphertextCommitment: payment.output.ciphertextCommitment),
      try XCTUnwrap(
        Data(
          hexString: XCTUnwrap(
            identities["prepared_transfer_digest_hex"] as? String))))
    let fixtureCreditID = try XCTUnwrap(
      Data(
        hexString: XCTUnwrap(
          identities["credit_id_hex"] as? String)))
    XCTAssertEqual(
      payment.output.creditID,
      fixtureCreditID)
    XCTAssertEqual(
      try KagemushaNoritoV1.expectedPeerCreditIDShape(
        payment.output, request: request, intent: intent),
      fixtureCreditID)
    let opening = try XCTUnwrap(identities["peer_credit_opening"] as? [String: Any])
    XCTAssertEqual(
      try KagemushaNoritoV1.peerCreditOpeningCommitmentShape(
        requestDigest: XCTUnwrap(
          Data(
            hexString: XCTUnwrap(
              opening["request_digest_hex"] as? String))),
        recipientOneTimeKey: KagemushaX25519PublicKeyV1(
          rawBytes: XCTUnwrap(
            Data(hexString: XCTUnwrap(opening["recipient_one_time_key_hex"] as? String)))),
        amount: KagemushaUInt128V1(UInt64(XCTUnwrap(opening["amount"] as? Int))),
        creditCommitmentOpening: XCTUnwrap(
          Data(
            hexString: XCTUnwrap(
              opening["credit_commitment_opening_hex"] as? String))),
        recipientBindingOpening: XCTUnwrap(
          Data(
            hexString: XCTUnwrap(
              opening["recipient_binding_opening_hex"] as? String))),
        recoveryNonce: XCTUnwrap(
          Data(
            hexString: XCTUnwrap(
              opening["recovery_nonce_hex"] as? String)))),
      try XCTUnwrap(Data(hexString: XCTUnwrap(opening["commitment_hex"] as? String))))

    XCTAssertEqual(try KagemushaNoritoV1.encodePaymentRequestShape(request), requestRaw)
    XCTAssertEqual(
      try KagemushaNoritoV1.encodeAcceptanceIntentShape(
        intent, against: request),
      intentRaw)
    XCTAssertEqual(
      try KagemushaNoritoV1.encodeAcceptanceTicketShape(
        ticket, against: request, intent: intent),
      ticketRaw)
    XCTAssertEqual(
      try KagemushaNoritoV1.encodePaymentShape(
        payment, against: request, intent: intent, ticket: ticket),
      paymentRaw)
    XCTAssertEqual(
      try KagemushaNoritoV1.encodeAcknowledgementShape(
        acknowledgement, against: request, intent: intent,
        ticket: ticket, payment: payment),
      acknowledgementRaw)

    let completeBytes = expectedOrder.reduce(0) { total, entry in
      total + sections[entry.section, default: Data()].count
    }
    let complete = try XCTUnwrap(fixture["complete_five_message"] as? [String: Any])
    XCTAssertEqual(completeBytes, try XCTUnwrap(complete["raw_bytes"] as? Int))
    XCTAssertEqual(
      try KagemushaNoritoV1.validateCompleteExchangeShape(
        request: request, intent: intent, ticket: ticket,
        payment: payment, acknowledgement: acknowledgement),
      completeBytes)
  }

  func testFrozenFiveMessageIPM1Order() {
    XCTAssertEqual(
      IrohaPeerWireKindV1.allCases.map(\.rawValue),
      [1, 2, 3, 4, 5])
    XCTAssertEqual(IrohaPeerWireKindV1.request.rawValue, 1)
    XCTAssertEqual(IrohaPeerWireKindV1.intent.rawValue, 2)
    XCTAssertEqual(IrohaPeerWireKindV1.ticket.rawValue, 3)
    XCTAssertEqual(IrohaPeerWireKindV1.payment.rawValue, 4)
    XCTAssertEqual(IrohaPeerWireKindV1.acknowledgement.rawValue, 5)
  }

  func testAllFourRequestModesHaveExactCircuitTranscripts() throws {
    let ranged = try KagemushaAmountPolicyV1(
      minimumAmount: KagemushaUInt128V1(2),
      maximumAmount: KagemushaUInt128V1(9))
    let modes: [KagemushaPaymentRequestModeV1] = [
      .singleExact(try KagemushaSingleExactV1(amount: KagemushaUInt128V1(7))),
      .partialUntilTotal(
        try KagemushaPartialUntilTotalV1(totalAmount: KagemushaUInt128V1(11))),
      .boundedMultiPayment(
        try KagemushaBoundedMultiPaymentV1(amountPolicy: ranged, maxPayments: 3)),
      .openReceive(KagemushaOpenReceiveV1(amountPolicy: ranged)),
    ]

    XCTAssertEqual(modes.map(\.wireTag), [0, 1, 2, 3])
    let expectedModeDigests = [
      "0ec60f7ba37f01113cda5000d8974a4bd3c73d8e83f8266ce83f88dfafdf9d9c",
      "006051f9455376f5d01242b5dbbde856f70831d50996bd68c6ca04e5a9689945",
      "b3f6c798ad5305285482642a9b4e630f7a20b4a1ba621fe922588a91d315ae7e",
      "770e4e937a11cc379397696fd72957653ef97caff49d2bafaeb3ea8e2ffa6705",
    ]
    for (mode, expectedDigest) in zip(modes, expectedModeDigests) {
      XCTAssertEqual(mode.canonicalCircuitTranscript.count, 37)
      XCTAssertEqual(mode.canonicalCircuitTranscript.first, UInt8(mode.wireTag))
      XCTAssertEqual(
        KagemushaNoritoV1.paymentRequestModeDigestShape(mode),
        try XCTUnwrap(Data(hexString: expectedDigest)))
    }
    XCTAssertTrue(modes[0].acceptsPaymentAmount(KagemushaUInt128V1(7)))
    XCTAssertFalse(modes[0].acceptsPaymentAmount(KagemushaUInt128V1(6)))
    XCTAssertTrue(modes[1].acceptsPaymentAmount(KagemushaUInt128V1(10)))
    XCTAssertFalse(modes[2].acceptsPaymentAmount(KagemushaUInt128V1(10)))
    XCTAssertTrue(modes[3].acceptsPaymentAmount(KagemushaUInt128V1(2)))
  }

  func testFixedTranscriptsBindFreshTicketKeyAndTicketIndependentCreditID() throws {
    let fixture = try loadCanonicalFixture()
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
      fixtureHex(XCTUnwrap(fixture["payment_request"] as? [String: Any])))
    let intent = try KagemushaNoritoV1.decodeAcceptanceIntentShapeExact(
      fixtureHex(XCTUnwrap(fixture["acceptance_intent"] as? [String: Any])), against: request)
    let ticket = try KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
      fixtureHex(XCTUnwrap(fixture["acceptance_ticket"] as? [String: Any])),
      against: request, intent: intent)
    let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
      fixtureHex(XCTUnwrap(fixture["payment"] as? [String: Any])),
      against: request, intent: intent, ticket: ticket)

    let requestDomain = Data("iroha:kagemusha:v1:payment-request-signing".utf8)
    let requestSigning = try KagemushaNoritoV1.paymentRequestSigningBytesShape(request)
    XCTAssertEqual(requestSigning.prefix(requestDomain.count), requestDomain)
    XCTAssertEqual(requestSigning[requestDomain.count], 0)
    XCTAssertEqual(requestSigning.count, requestDomain.count + 1 + 315)
    var requestTranscript = Data(requestSigning.dropFirst(requestDomain.count + 1))
    requestTranscript.append(request.signature.rawBytes)
    XCTAssertEqual(requestTranscript.count, 379)
    XCTAssertEqual(
      KagemushaNoritoV1.paymentRequestDigest(request),
      transcriptDigest("iroha:kagemusha:v1:payment-request", requestTranscript))

    var intentTranscript = Data([1, 0])
    intentTranscript.append(intent.requestDigest)
    intentTranscript.append(intent.intentID)
    intentTranscript.append(intent.exactAmount.littleEndianBytes)
    intentTranscript.append(intent.senderOneTimeCommitment)
    XCTAssertEqual(intentTranscript.count, 114)
    let intentDigest = KagemushaNoritoV1.acceptanceIntentDigestShape(intent)
    XCTAssertEqual(
      intentDigest, transcriptDigest("iroha:kagemusha:v1:acceptance-intent", intentTranscript))

    let ticketDomain = Data("iroha:kagemusha:v1:acceptance-ticket-signing".utf8)
    let ticketSigning = try KagemushaNoritoV1.acceptanceTicketSigningBytesShape(
      ticket, against: request, intent: intent)
    XCTAssertEqual(ticketSigning.prefix(ticketDomain.count), ticketDomain)
    XCTAssertEqual(ticketSigning[ticketDomain.count], 0)
    XCTAssertEqual(ticketSigning.count, ticketDomain.count + 1 + 150)
    var ticketTranscript = Data(ticketSigning.dropFirst(ticketDomain.count + 1))
    ticketTranscript.append(ticket.signature.rawBytes)
    XCTAssertEqual(ticketTranscript.count, 214)
    let ticketDigest = try KagemushaNoritoV1.acceptanceTicketDigestShape(
      ticket, against: request, intent: intent)
    XCTAssertEqual(
      ticketDigest, transcriptDigest("iroha:kagemusha:v1:acceptance-ticket", ticketTranscript))

    var preparedTranscript = Data([1, 0])
    preparedTranscript.append(KagemushaNoritoV1.paymentRequestDigest(request))
    preparedTranscript.append(intentDigest)
    preparedTranscript.append(ticketDigest)
    preparedTranscript.append(intent.exactAmount.littleEndianBytes)
    preparedTranscript.append(payment.output.transitionNullifier)
    preparedTranscript.append(ticket.recipientOneTimeKey.rawBytes)
    preparedTranscript.append(payment.output.ciphertextCommitment)
    XCTAssertEqual(preparedTranscript.count, 210)
    let preparedDigest = try KagemushaNoritoV1.preparedTransferDigestShape(
      request: request, intent: intent, ticket: ticket,
      transitionNullifier: payment.output.transitionNullifier,
      ciphertextCommitment: payment.output.ciphertextCommitment)
    XCTAssertEqual(
      preparedDigest, transcriptDigest("iroha:kagemusha:v1:prepared-transfer", preparedTranscript))

    let otherTicket = try KagemushaAcceptanceTicketV1(
      acceptanceTicketID: ticket.acceptanceTicketID,
      recipientOneTimeKey: KagemushaX25519PublicKeyV1(rawBytes: digest(0x91)),
      reservedInboxBytes: ticket.reservedInboxBytes,
      issuedAtMS: ticket.issuedAtMS, expiresAtMS: ticket.expiresAtMS, signature: ticket.signature)
    XCTAssertNotEqual(ticket.recipientOneTimeKey, otherTicket.recipientOneTimeKey)
    XCTAssertNotEqual(
      ticketDigest,
      try KagemushaNoritoV1.acceptanceTicketDigestShape(
        otherTicket, against: request, intent: intent))
    XCTAssertNotEqual(
      preparedDigest,
      try KagemushaNoritoV1.preparedTransferDigestShape(
        request: request, intent: intent, ticket: otherTicket,
        transitionNullifier: payment.output.transitionNullifier,
        ciphertextCommitment: payment.output.ciphertextCommitment))
    XCTAssertThrowsError(
      try KagemushaNoritoV1.encodePaymentShape(
        payment, against: request, intent: intent, ticket: otherTicket))

    var creditPreimage = Data("iroha:kagemusha:v1:credit-id".utf8)
    creditPreimage.append(UInt8(0))
    creditPreimage.append(payment.output.transitionNullifier)
    creditPreimage.append(intentDigest)
    XCTAssertEqual(payment.output.creditID, Data(SHA256.hash(data: creditPreimage)))
    let reboundOutput = try KagemushaPaymentOutputV1(
      acceptanceIntentDigest: intentDigest, acceptanceTicketDigest: digest(0x92),
      transitionNullifier: payment.output.transitionNullifier, creditID: payment.output.creditID,
      ciphertextCommitment: digest(0x93), commitEvidence: payment.output.commitEvidence)
    XCTAssertEqual(
      payment.output.creditID,
      try KagemushaNoritoV1.expectedPeerCreditIDShape(
        reboundOutput, request: request, intent: intent))

    let context = try KagemushaNoritoV1.peerCreditContextShape(
      output: payment.output, request: request, intent: intent, ticket: ticket)
    XCTAssertEqual(context.recipientOneTimeKey, ticket.recipientOneTimeKey)
    XCTAssertEqual(context.preparedTransferDigest, preparedDigest)
  }

  func testPaymentBodyExcludesProofButBindsActualCiphertextAndCertificateSeparately() throws {
    let fixture = try loadCanonicalFixture()
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
      fixtureHex(XCTUnwrap(fixture["payment_request"] as? [String: Any])))
    let intent = try KagemushaNoritoV1.decodeAcceptanceIntentShapeExact(
      fixtureHex(XCTUnwrap(fixture["acceptance_intent"] as? [String: Any])), against: request)
    let ticket = try KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
      fixtureHex(XCTUnwrap(fixture["acceptance_ticket"] as? [String: Any])),
      against: request, intent: intent)
    let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
      fixtureHex(XCTUnwrap(fixture["payment"] as? [String: Any])),
      against: request, intent: intent, ticket: ticket)
    var outputTranscript = Data([1, 0])
    for field in [
      payment.output.acceptanceIntentDigest, payment.output.acceptanceTicketDigest,
      payment.output.transitionNullifier, payment.output.creditID,
      payment.output.ciphertextCommitment,
    ] {
      outputTranscript.append(field)
    }
    var evidenceTag = payment.output.commitEvidence.wireTag.littleEndian
    outputTranscript.append(withUnsafeBytes(of: &evidenceTag) { Data($0) })
    outputTranscript.append(payment.output.commitEvidence.evidenceCommitment)
    XCTAssertEqual(outputTranscript.count, 198)
    let outputDigest = KagemushaNoritoV1.paymentOutputDigestShape(payment.output)
    XCTAssertEqual(
      outputDigest, transcriptDigest("iroha:kagemusha:v1:send-split-statement", outputTranscript))
    var bodyTranscript = outputDigest
    bodyTranscript.append(KagemushaNoritoV1.ciphertextDigestShape(payment.encryptedCredit))
    let body = try KagemushaNoritoV1.paymentBodyDigestShape(
      output: payment.output, encryptedCredit: payment.encryptedCredit)
    XCTAssertEqual(body, transcriptDigest("iroha:kagemusha:v1:payment-body", bodyTranscript))
    XCTAssertEqual(body, payment.proof.semanticDigest)
    XCTAssertEqual(
      payment.proof.commitCertificateDigest,
      KagemushaNoritoV1.commitCertificateDigestShape(payment.commitCertificate))

    var randomizedProof = payment.proof.eqProof
    randomizedProof[randomizedProof.startIndex] ^= 1
    let proof = try KagemushaPaymentProofV1(
      eqProtocolDigest: payment.proof.eqProtocolDigest,
      epProtocolDigest: payment.proof.epProtocolDigest,
      semanticDigest: payment.proof.semanticDigest,
      candidateEnvelopeDigest: payment.proof.candidateEnvelopeDigest,
      commitCertificateDigest: payment.proof.commitCertificateDigest,
      eqDeferredAudit: payment.proof.eqDeferredAudit,
      epDeferredAudit: payment.proof.epDeferredAudit,
      eqProof: randomizedProof, epProof: payment.proof.epProof,
      eqHistory: payment.proof.eqHistory, epHistory: payment.proof.epHistory)
    let randomized = try KagemushaPaymentV1(
      output: payment.output, encryptedCredit: payment.encryptedCredit,
      commitCertificate: payment.commitCertificate, proof: proof)
    XCTAssertEqual(
      body,
      try KagemushaNoritoV1.paymentBodyDigestShape(
        output: randomized.output, encryptedCredit: randomized.encryptedCredit))
    XCTAssertNotEqual(
      try KagemushaNoritoV1.paymentDigestShape(
        payment, against: request, intent: intent, ticket: ticket),
      try KagemushaNoritoV1.paymentDigestShape(
        randomized, against: request, intent: intent, ticket: ticket))
    let encrypted = try KagemushaNoritoV1.decodeEncryptedCreditEnvelopeShapeExact(
      payment.encryptedCredit)
    var alteredCiphertext = encrypted.ciphertextAndTag
    alteredCiphertext[alteredCiphertext.startIndex] ^= 1
    let alteredEnvelope = try KagemushaEncryptedCreditEnvelopeV1(
      ephemeralX25519PublicKey: encrypted.ephemeralX25519PublicKey, nonce: encrypted.nonce,
      ciphertextAndTag: alteredCiphertext)
    let alteredBytes = try KagemushaNoritoV1.encodeEncryptedCreditEnvelopeShape(alteredEnvelope)
    XCTAssertNotEqual(
      body,
      try KagemushaNoritoV1.paymentBodyDigestShape(
        output: payment.output, encryptedCredit: alteredBytes))
    XCTAssertThrowsError(
      try KagemushaNoritoV1.paymentBodyDigestShape(
        output: payment.output, encryptedCredit: payment.encryptedCredit + Data([1])))
  }

  func testNoCommitIsFailClosedAndRetiredPeerAuthorityIsAbsent() throws {
    let fixture = try loadCanonicalFixture()
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
      fixtureHex(XCTUnwrap(fixture["payment_request"] as? [String: Any])))
    let intent = try KagemushaNoritoV1.decodeAcceptanceIntentShapeExact(
      fixtureHex(XCTUnwrap(fixture["acceptance_intent"] as? [String: Any])), against: request)
    let ticket = try KagemushaNoritoV1.decodeAcceptanceTicketShapeExact(
      fixtureHex(XCTUnwrap(fixture["acceptance_ticket"] as? [String: Any])),
      against: request, intent: intent)
    let closure = try KagemushaNoCommitClosureV1(
      statement: KagemushaNoCommitClosureStatementV1(
        acceptanceIntentDigest: KagemushaNoritoV1.acceptanceIntentDigestShape(intent),
        acceptanceTicketDigest: KagemushaNoritoV1.acceptanceTicketDigestShape(
          ticket, against: request, intent: intent),
        preparedTransferDigest: digest(0xa1), recoveryID: digest(0xa2),
        cancellationNullifier: digest(0xa3), equivalentDeliverySlotCommitment: digest(0xa4)))
    let raw = try KagemushaNoritoV1.encodeNoCommitClosureShape(closure)
    XCTAssertEqual(try KagemushaNoritoV1.decodeNoCommitClosureShapeExact(raw), closure)
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeNoCommitClosureShapeExact(
        raw, against: request, intent: intent, ticket: ticket))
    for file in ["KagemushaModelsV1.swift", "KagemushaNoritoV1.swift", "KagemushaWalletV1.swift"] {
      let source = try String(contentsOf: sourceFile(named: file), encoding: .utf8)
      for retired in [
        ["AcceptanceIntent", "Authorization"], ["terminal", "Signature"],
        ["TerminalSigning", "BytesShape"], ["closeUncommitted", "Acceptance"],
      ] {
        XCTAssertFalse(source.contains(retired.joined()), file)
      }
    }
  }

  func testStandaloneHardwareShapesAndDeviceKeyReference() throws {
    let fixture = try loadCanonicalFixture()
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
      fixtureHex(XCTUnwrap(fixture["payment_request"] as? [String: Any])))
    let credential = request.hardwareCredential
    let profile = try KagemushaHardwareProfileV1(
      hardwareProfileID: credential.hardwareProfileID, providerID: digest(0xc0),
      platformClass: .appleOEMService, productClassDigest: digest(0xc1),
      firmwarePolicyDigest: credential.firmwarePolicyDigest,
      enrollmentAttestationVerifierDigest: digest(0xc2), attestationTrustRootsDigest: digest(0xc3),
      allowedSuiteCommitment: digest(0xc4), policyEpoch: credential.policyEpoch,
      governanceCredentialPublicKey: credential.devicePublicKey,
      capabilityMask: KagemushaWireV1.requiredHardwareCapabilityMask,
      qualificationReportDigest: digest(0xc5), validFromMS: 0, expiresAtMS: UInt64.max)
    let profileRaw = try KagemushaNoritoV1.encodeHardwareProfileShape(profile)
    let credentialRaw = try KagemushaNoritoV1.encodeHardwareCredentialShape(credential)
    XCTAssertLessThanOrEqual(profileRaw.count, 512)
    XCTAssertLessThanOrEqual(credentialRaw.count, 768)
    XCTAssertEqual(try KagemushaNoritoV1.decodeHardwareProfileShapeExact(profileRaw), profile)
    XCTAssertEqual(
      try KagemushaNoritoV1.decodeHardwareCredentialShapeExact(credentialRaw), credential)
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeHardwareProfileShapeExact(profileRaw + Data([0])))
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeHardwareCredentialShapeExact(credentialRaw + Data([0])))
    XCTAssertThrowsError(try KagemushaNoritoV1.decodeHardwareProfileShapeExact(credentialRaw))
    XCTAssertThrowsError(try KagemushaNoritoV1.decodeHardwareCredentialShapeExact(profileRaw))
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeHardwareProfileShapeExact(Data(repeating: 1, count: 513)))
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeHardwareCredentialShapeExact(Data(repeating: 1, count: 769)))
    var reference = Data("iroha:kagemusha:v1:device-key-reference".utf8)
    reference.append(UInt8(0))
    reference.append(credential.devicePublicKey.sec1Bytes)
    XCTAssertEqual(
      KagemushaNoritoV1.deviceKeyReferenceShape(credential.devicePublicKey),
      Data(SHA256.hash(data: reference)))
    XCTAssertEqual(
      KagemushaNoritoV1.deviceKeyReferenceShape(credential.devicePublicKey),
      credential.deviceKeyReference)
  }

  func testStandaloneRedemptionProofRejectsPaymentSchemaAndNonExactArchives() throws {
    let fixture = try loadCanonicalFixture()
    let paymentRaw = try fixtureHex(XCTUnwrap(fixture["payment_proof"] as? [String: Any]))
    let payment = try KagemushaNoritoV1.decodePaymentProofShapeExact(paymentRaw)
    let redemption = try KagemushaRedemptionProofV1(
      eqProtocolDigest: payment.eqProtocolDigest, epProtocolDigest: payment.epProtocolDigest,
      semanticDigest: payment.semanticDigest,
      candidateEnvelopeDigest: payment.candidateEnvelopeDigest,
      commitCertificateDigest: payment.commitCertificateDigest,
      eqDeferredAudit: payment.eqDeferredAudit,
      epDeferredAudit: payment.epDeferredAudit, eqProof: payment.eqProof, epProof: payment.epProof,
      eqHistory: payment.eqHistory, epHistory: payment.epHistory)
    let raw = try KagemushaNoritoV1.encodeRedemptionProofShape(redemption)
    XCTAssertEqual(try KagemushaNoritoV1.decodeRedemptionProofShapeExact(raw), redemption)
    XCTAssertNotEqual(raw, paymentRaw)
    XCTAssertThrowsError(try KagemushaNoritoV1.decodeRedemptionProofShapeExact(paymentRaw))
    XCTAssertThrowsError(try KagemushaNoritoV1.decodePaymentProofShapeExact(raw))
    XCTAssertThrowsError(try KagemushaNoritoV1.decodeRedemptionProofShapeExact(raw + Data([0])))
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeRedemptionProofShapeExact(Data(raw.dropLast())))
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeRedemptionProofShapeExact(
        Data(repeating: 1, count: Int(KagemushaWireV1.maximumRedemptionProofBytes) + 1)))
  }

  func testMintCreditShapeBindsDerivedIDProofCiphertextAndExactAuthorization() throws {
    let (credit, authorization) = try mintFixture()
    let raw = try KagemushaNoritoV1.encodeMintCreditShape(credit, against: authorization)
    XCTAssertEqual(try KagemushaNoritoV1.decodeMintCreditShapeExact(raw), credit)
    XCTAssertEqual(
      try KagemushaNoritoV1.decodeMintCreditShapeExact(raw, against: authorization), credit)
    XCTAssertEqual(
      try KagemushaNoritoV1.expectedMintCreditIDShape(credit.statement),
      credit.statement.lifecycle.creditID)
    XCTAssertEqual(
      try KagemushaNoritoV1.mintCreditStatementDigestShape(credit.statement),
      credit.proof.semanticDigest)
    XCTAssertEqual(
      try KagemushaNoritoV1.mintAuthorizationDigestShape(authorization),
      credit.statement.mintAuthorizationDigest)
    XCTAssertEqual(
      try KagemushaNoritoV1.mintAuthorizationContextDigestShape(authorization.statement.context),
      credit.statement.authorizationContextDigest)
    let aad = try KagemushaNoritoV1.encryptedCreditAADForMintShape(authorization.statement)
    XCTAssertEqual(aad.purpose, .mint)
    XCTAssertEqual(aad.contextDigest, credit.statement.authorizationContextDigest)
    XCTAssertEqual(aad.creditID, credit.statement.lifecycle.creditID)
    XCTAssertEqual(aad.amount, credit.statement.amount)
    XCTAssertEqual(aad.issuanceOrTransitionCommitment, credit.statement.issuanceCommitment)

    for mutation in ["creditID", "liabilityPool", "semantic", "ciphertext"] {
      let (changed, _) = try mintFixture(mutation: mutation)
      XCTAssertThrowsError(try KagemushaNoritoV1.encodeMintCreditShape(changed), mutation)
    }
    XCTAssertThrowsError(try mintFixture(mutation: "operation"))
    XCTAssertThrowsError(try mintFixture(mutation: "contextPool"))
    for mutation in ["authorizationDigest", "artifactManifest", "release", "amount", "issuance"] {
      let (changed, _) = try mintFixture(mutation: mutation)
      // Standalone public consistency is not authority for a different authorization.
      let changedRaw = try KagemushaNoritoV1.encodeMintCreditShape(changed)
      XCTAssertThrowsError(
        try KagemushaNoritoV1.encodeMintCreditShape(changed, against: authorization), mutation)
      XCTAssertThrowsError(
        try KagemushaNoritoV1.decodeMintCreditShapeExact(changedRaw, against: authorization),
        mutation)
    }
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeMintCreditShapeExact(raw + Data([0]), against: authorization))
  }

  func testEncryptedCreditEnvelopeRequiresFixedCanonicalLength() throws {
    let key = try KagemushaX25519PublicKeyV1(rawBytes: digest(0x76))
    let nonce = Data(repeating: 0x77, count: KagemushaWireV1.xchachaNonceBytes)
    let envelope = try KagemushaEncryptedCreditEnvelopeV1(
      ephemeralX25519PublicKey: key, nonce: nonce,
      ciphertextAndTag: Data(
        repeating: 0x78, count: KagemushaWireV1.encryptedCreditCiphertextAndTagBytes))
    let bytes = try KagemushaNoritoV1.encodeEncryptedCreditEnvelopeShape(envelope)
    XCTAssertEqual(KagemushaWireV1.creditOpeningCanonicalBytes, 200)
    XCTAssertEqual(envelope.ciphertextAndTag.count, 216)
    XCTAssertEqual(bytes.count, KagemushaWireV1.encryptedCreditCanonicalBytes)
    XCTAssertEqual(bytes.count, 327)
    XCTAssertEqual(
      try KagemushaNoritoV1.decodeEncryptedCreditEnvelopeShapeExact(bytes), envelope)
    for length in [0, 16, 17, 64, 215, 217, 384] {
      XCTAssertThrowsError(
        try KagemushaEncryptedCreditEnvelopeV1(
          ephemeralX25519PublicKey: key, nonce: nonce,
          ciphertextAndTag: Data(repeating: 0x78, count: length)),
        "ciphertext length \(length) must not cross the canonical V1 boundary")
    }
  }

  func testDeviceMintStageBodiesAreCanonicalBoundedAndCreditBound() throws {
    let (credit, authorization) = try mintFixture()
    let authorizationBytes = try KagemushaNoritoV1.encodeMintAuthorizationShape(authorization)
    let creditBytes = try KagemushaNoritoV1.encodeMintCreditShape(
      credit, against: authorization)
    let command = try KagemushaDeviceMintStageCommandV1(
      canonicalAuthorization: authorizationBytes,
      canonicalMintCredit: creditBytes)
    let commandBytes = try KagemushaNoritoV1.encodeDeviceMintStageCommandShape(command)
    XCTAssertLessThanOrEqual(
      commandBytes.count, KagemushaNoritoV1.maximumDeviceMintStageCommandBytes)
    XCTAssertEqual(
      try KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(commandBytes), command)

    for disposition in KagemushaDeviceMintStageDispositionV1.allCases {
      let result = try KagemushaDeviceMintStageResultV1(
        disposition: disposition, creditID: credit.statement.lifecycle.creditID)
      let resultBytes = try KagemushaNoritoV1.encodeDeviceMintStageResultShape(result)
      XCTAssertLessThanOrEqual(
        resultBytes.count, KagemushaNoritoV1.maximumDeviceMintStageResultBytes)
      XCTAssertEqual(
        try KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(
          resultBytes, against: command),
        result)
      XCTAssertThrowsError(
        try KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(
          resultBytes + Data([0]), against: command))
    }

    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(commandBytes + Data([0])))
    let wrongResult = try KagemushaDeviceMintStageResultV1(
      disposition: .staged, creditID: digest(0xe1))
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(
        KagemushaNoritoV1.encodeDeviceMintStageResultShape(wrongResult), against: command))
    XCTAssertThrowsError(
      try KagemushaDeviceMintStageCommandV1(
        canonicalAuthorization: Data(
          repeating: 1, count: KagemushaWireV1.maximumMintAuthorizationBytes + 1),
        canonicalMintCredit: creditBytes))
  }

  func testRustCanonicalDeviceMintStageFixtureRoundTripsByteForByte() throws {
    let fixture = try loadCanonicalFixture(named: "kagemusha_device_mint_stage_v1.json")
    XCTAssertEqual(try XCTUnwrap(fixture["fixture_version"] as? Int), 1)
    XCTAssertEqual(try XCTUnwrap(fixture["protocol"] as? String), "KAGEMUSHA")
    XCTAssertEqual(try XCTUnwrap(fixture["operation"] as? Int), 21)
    XCTAssertEqual(try XCTUnwrap(fixture["structural_only"] as? Bool), true)
    let authorization = try fixtureHex(
      XCTUnwrap(fixture["authorization"] as? [String: Any]), field: "hex")
    let credit = try fixtureHex(
      XCTUnwrap(fixture["mint_credit"] as? [String: Any]), field: "hex")
    let commandSection = try XCTUnwrap(fixture["command"] as? [String: Any])
    let commandBytes = try fixtureHex(commandSection, field: "hex")
    let command = try KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(commandBytes)
    XCTAssertEqual(command.canonicalAuthorization, authorization)
    XCTAssertEqual(command.canonicalMintCredit, credit)
    XCTAssertEqual(try KagemushaNoritoV1.encodeDeviceMintStageCommandShape(command), commandBytes)
    XCTAssertEqual(try XCTUnwrap(commandSection["alignment"] as? Int), 8)
    XCTAssertEqual(try XCTUnwrap(commandSection["raw_bytes"] as? Int), commandBytes.count)

    let creditID = try fixtureHexString(XCTUnwrap(fixture["credit_id_hex"] as? String))
    for (name, disposition) in [
      ("staged_result", KagemushaDeviceMintStageDispositionV1.staged),
      ("exact_duplicate_result", KagemushaDeviceMintStageDispositionV1.exactDuplicate),
    ] {
      let section = try XCTUnwrap(fixture[name] as? [String: Any])
      let bytes = try fixtureHex(section, field: "hex")
      let result = try KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(
        bytes, against: command)
      XCTAssertEqual(result.disposition, disposition)
      XCTAssertEqual(result.creditID, creditID)
      XCTAssertEqual(try KagemushaNoritoV1.encodeDeviceMintStageResultShape(result), bytes)
      XCTAssertEqual(try XCTUnwrap(section["alignment"] as? Int), 2)
      XCTAssertEqual(try XCTUnwrap(section["raw_bytes"] as? Int), bytes.count)
    }
  }

  func testDeviceMintStageDecodersAcceptNonzeroBasedDataSlices() throws {
    let fixture = try loadCanonicalFixture(named: "kagemusha_device_mint_stage_v1.json")
    func slice(_ bytes: Data) -> Data {
      var carrier = Data(repeating: 0xa5, count: 9)
      carrier.append(bytes)
      let result = carrier.dropFirst(9)
      XCTAssertEqual(result.startIndex, 9)
      return result
    }
    let commandBytes = try fixtureHex(
      XCTUnwrap(fixture["command"] as? [String: Any]), field: "hex")
    let command = try KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(slice(commandBytes))
    XCTAssertEqual(try KagemushaNoritoV1.encodeDeviceMintStageCommandShape(command), commandBytes)
    let authorization = try KagemushaNoritoV1.decodeMintAuthorizationShapeExact(
      slice(command.canonicalAuthorization))
    let credit = try KagemushaNoritoV1.decodeMintCreditShapeExact(
      slice(command.canonicalMintCredit), against: authorization)
    for name in ["staged_result", "exact_duplicate_result"] {
      let bytes = try fixtureHex(XCTUnwrap(fixture[name] as? [String: Any]), field: "hex")
      let result = try KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(
        slice(bytes), against: command)
      XCTAssertEqual(result.creditID, credit.statement.lifecycle.creditID)
      XCTAssertEqual(try KagemushaNoritoV1.encodeDeviceMintStageResultShape(result), bytes)
    }
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeDeviceMintStageCommandShapeExact(slice(Data([0]))))
    XCTAssertThrowsError(
      try KagemushaNoritoV1.decodeDeviceMintStageResultShapeExact(slice(Data([0]))))
  }

  func testNativeBridgeRequiresMintStageValidatorSymbols() throws {
    let source = try String(contentsOf: sourceFile(named: "NativeBridge.swift"), encoding: .utf8)
    let start = try XCTUnwrap(source.range(of: "private static let requiredSymbols = ["))
    let end = try XCTUnwrap(
      source.range(
        of: "] + parliamentTimedOvnWalletRequiredSymbols",
        range: start.upperBound..<source.endIndex))
    let inventory = source[start.upperBound..<end.lowerBound]
    for symbol in [
      "connect_norito_kagemusha_device_mint_stage_command_v1_validate",
      "connect_norito_kagemusha_device_mint_stage_result_v1_validate",
    ] {
      XCTAssertTrue(inventory.contains("\"\(symbol)\""), symbol)
    }
  }

  func testPeerOpeningRejectsZeroAmountAndReservedDigest() throws {
    let key = try KagemushaX25519PublicKeyV1(rawBytes: digest(0xb1))
    for (requestDigest, amount) in [
      (digest(0), KagemushaUInt128V1(1)),
      (digest(0xb2), KagemushaUInt128V1(0)),
    ] {
      XCTAssertThrowsError(
        try KagemushaNoritoV1.peerCreditOpeningCommitmentShape(
          requestDigest: requestDigest, recipientOneTimeKey: key, amount: amount,
          creditCommitmentOpening: digest(0xb3), recipientBindingOpening: digest(0xb4),
          recoveryNonce: digest(0xb5)))
    }
  }

  private func transcriptDigest(_ domain: String, _ transcript: Data) -> Data {
    var bytes = Data(domain.utf8)
    bytes.append(UInt8(0))
    var length = UInt64(transcript.count).littleEndian
    bytes.append(withUnsafeBytes(of: &length) { Data($0) })
    bytes.append(transcript)
    return Data(SHA256.hash(data: bytes))
  }

  func testReceiveFoldBatchAlwaysCarriesSixteenPaddedSlots() throws {
    var active: [KagemushaReceiveFoldBatchSlotV1] = []
    for index in 0..<16 {
      let amount = KagemushaUInt128V1(UInt64(index + 1))
      let slot = try KagemushaReceiveFoldBatchSlotV1(
        amount: amount,
        creditID: digest(UInt8(0x10 + index)),
        recipientLaneID: digest(UInt8(0x30 + index)),
        incomingProofBindingDigest: digest(UInt8(0x50 + index)),
        envelopeDigest: digest(UInt8(0x70 + index)))
      active.append(slot)
    }

    for occupancy in 1...16 {
      let batch = try KagemushaReceiveFoldBatchV1(
        activeSlots: Array(active.prefix(occupancy)))
      XCTAssertEqual(batch.activeCount, UInt8(occupancy))
      XCTAssertEqual(batch.paddedSlots.count, 16)
      XCTAssertEqual(batch.canonicalBody.count, 2_305)
      XCTAssertTrue(batch.paddedSlots.prefix(occupancy).allSatisfy { !$0.isPadding })
      XCTAssertTrue(batch.paddedSlots.dropFirst(occupancy).allSatisfy { $0.isPadding })
      XCTAssertEqual(KagemushaNoritoV1.receiveFoldBatchDigestShape(batch).count, 32)
    }

    XCTAssertThrowsError(
      try KagemushaReceiveFoldBatchV1(activeSlots: []))
    XCTAssertThrowsError(
      try KagemushaReceiveFoldBatchV1(activeSlots: [active[0], active[0]]))
  }

  func testSoleTextPrefixAndCanonicalPayloadKinds() throws {
    XCTAssertEqual(KagemushaWireV1.textPrefix, "kgm1:")
    let expected: [KagemushaWirePayloadKindV1] = [
      .paymentRequest, .acceptanceIntent,
      .acceptanceTicket, .payment, .acknowledgement, .noCommitClosure,
      .mintAuthorization, .mintCredit, .redemptionVoucher,
    ]
    XCTAssertEqual(KagemushaWirePayloadKindV1.allCases, expected)
    for kind in expected {
      let bytes = Data([0xa5])
      let text = try KagemushaWireV1.encodeText(bytes, kind: kind)
      XCTAssertTrue(text.hasPrefix("kgm1:"))
      XCTAssertEqual(try KagemushaWireV1.decodeText(text, kind: kind), bytes)
      let invalidPrefix = "invalid-v1:"
      XCTAssertThrowsError(
        try KagemushaWireV1.decodeText(
          invalidPrefix + String(text.dropFirst(5)), kind: kind))
    }
  }

  func testExactAggregateAndRecoveryBounds() {
    XCTAssertEqual(KagemushaWireV1.targetPreTicketExchangeRawBytes, 1_376)
    XCTAssertEqual(KagemushaWireV1.maximumPreTicketExchangeRawBytes, 1_376)
    XCTAssertEqual(KagemushaWireV1.maximumPreTicketExchangeTextBytes, 1_851)
    XCTAssertEqual(KagemushaWireV1.targetCompleteExchangeRawBytes, 8_960)
    XCTAssertEqual(KagemushaWireV1.maximumCompleteExchangeRawBytes, 9_211)
    XCTAssertEqual(KagemushaWireV1.maximumCompleteExchangeTextBytes, 12_288)
    XCTAssertEqual(KagemushaWireV1.minimumAcceptanceTicketInboxBytes, 8_320)
    XCTAssertEqual(KagemushaWireV1.minimumPaymentOutboxBytes, 25_728)
    XCTAssertEqual(KagemushaWireV1.minimumRedemptionOutboxBytes, 26_112)
  }

  func testEveryTextEnvelopeAcceptsItsExactCompleteBound() throws {
    for kind in KagemushaWirePayloadKindV1.allCases {
      let raw = Data(repeating: 0xa5, count: kind.maximumRawBytes)
      let text = try KagemushaWireV1.encodeText(raw, kind: kind)
      XCTAssertEqual(text.utf8.count, kind.maximumTextBytes)
      XCTAssertEqual(try KagemushaWireV1.decodeText(text, kind: kind), raw)
      XCTAssertThrowsError(
        try KagemushaWireV1.encodeText(raw + Data([0xa5]), kind: kind))
      XCTAssertThrowsError(try KagemushaWireV1.decodeText(text + "A", kind: kind))
    }
  }

  // These opaque proof bytes test codec consistency only, never native monetary authority.
  private func mintFixture(mutation: String? = nil) throws -> (
    KagemushaMintCreditV1, KagemushaMintAuthorizationV1
  ) {
    let fixture = try loadCanonicalFixture()
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(
      fixtureHex(XCTUnwrap(fixture["payment_request"] as? [String: Any])))
    return try MintFixture.make(request: request, mutation: mutation)
  }

  /// Shared structural fixture for codec and host orchestration tests, with no proof authority.
  enum MintFixture {
    static func make(request: KagemushaPaymentRequestV1, mutation: String? = nil) throws -> (
      KagemushaMintCreditV1, KagemushaMintAuthorizationV1
    ) {
      let context = try KagemushaMintAuthorizationContextV1(
        operationID: digest(0x70),
        releaseID: mutation == "release" ? digest(0xee) : request.releaseID,
        suiteID: request.hardwareCredential.suiteID, vkDigest: digest(0x71),
        artifactManifestDigest: digest(0x72), networkID: request.networkID,
        asset: request.asset, assetIncarnation: request.assetIncarnation, scale: request.scale,
        liabilityPoolID: mutation == "contextPool" ? digest(0xe4) : request.liabilityPoolID,
        amount: KagemushaUInt128V1(mutation == "amount" ? 41 : 40),
        payer: request.recipient, recipient: request.recipient,
        hardwareCredentialID: request.hardwareCredential.credentialID,
        hardwareProfileID: request.hardwareCredential.hardwareProfileID,
        policyEpoch: request.hardwareCredential.policyEpoch,
        recipientCredentialCommitment: digest(0x73),
        creditCommitment: digest(0x74),
        recipientOneTimeKey: KagemushaX25519PublicKeyV1(rawBytes: digest(0x75)))
      let encrypted = try KagemushaEncryptedCreditEnvelopeV1(
        ephemeralX25519PublicKey: KagemushaX25519PublicKeyV1(rawBytes: digest(0x76)),
        nonce: Data(repeating: 0x77, count: KagemushaWireV1.xchachaNonceBytes),
        ciphertextAndTag: Data(
          repeating: 0x78, count: KagemushaWireV1.encryptedCreditCiphertextAndTagBytes))
      let ciphertextDigest = try KagemushaNoritoV1.ciphertextDigestShape(
        KagemushaNoritoV1.encodeEncryptedCreditEnvelopeShape(encrypted))
      let contextDigest = try KagemushaNoritoV1.mintAuthorizationContextDigestShape(context)
      let issuance = digest(mutation == "issuance" ? 0xed : 0x79)
      let zero = Data(repeating: 0, count: 32)
      func statement(_ creditID: Data, _ authorizationDigest: Data, mutated: Bool = false) throws
        -> KagemushaMintCreditStatementV1
      {
        let wrongOperation = mutated && mutation == "operation"
        let lifecycle = try KagemushaLifecycleBindingV1(
          networkID: context.networkID, suiteID: context.suiteID, vkDigest: context.vkDigest,
          releaseID: context.releaseID, asset: context.asset,
          assetIncarnation: context.assetIncarnation,
          scale: context.scale,
          liabilityPoolID: mutated && mutation == "liabilityPool"
            ? digest(0xec) : context.liabilityPoolID,
          hardwareProfileID: context.hardwareProfileID, policyEpoch: context.policyEpoch,
          operationKind: wrongOperation ? .sendSplit : .mintFold,
          requestID: wrongOperation ? digest(0xeb) : zero,
          acceptanceTicketID: wrongOperation ? digest(0xea) : zero,
          creditID: mutated && mutation == "creditID" ? digest(0xe9) : creditID,
          ciphertextDigest: ciphertextDigest)
        return try KagemushaMintCreditStatementV1(
          lifecycle: lifecycle,
          recipientCredentialCommitment: context.recipientCredentialCommitment,
          authorizationContextDigest: contextDigest,
          mintAuthorizationDigest: mutated && mutation == "authorizationDigest"
            ? digest(0xe8) : authorizationDigest,
          amount: context.amount, issuanceCommitment: issuance, recipient: context.recipient,
          creditCommitment: context.creditCommitment, mintedAtMS: 1_000)
      }
      let provisional = try statement(digest(0x7a), digest(0x7b))
      let creditID = try KagemushaNoritoV1.expectedMintCreditIDShape(provisional)
      let authorizationStatement = try KagemushaMintAuthorizationStatementV1(
        context: context, issuanceCommitment: issuance, creditID: creditID,
        ciphertextDigest: ciphertextDigest)
      let authorization = try KagemushaMintAuthorizationV1(
        statement: authorizationStatement,
        proof: shapeProof(
          semanticDigest: KagemushaNoritoV1.mintAuthorizationStatementDigestShape(
            authorizationStatement)))
      let authorizationDigest = try KagemushaNoritoV1.mintAuthorizationDigestShape(authorization)
      let validStatement = try statement(creditID, authorizationDigest)
      let finalStatement = try statement(creditID, authorizationDigest, mutated: true)
      let semantic = try KagemushaNoritoV1.mintCreditStatementDigestShape(
        ["creditID", "liabilityPool"].contains(mutation ?? "") ? validStatement : finalStatement)
      let finalEncrypted = try KagemushaEncryptedCreditEnvelopeV1(
        ephemeralX25519PublicKey: encrypted.ephemeralX25519PublicKey, nonce: encrypted.nonce,
        ciphertextAndTag: mutation == "ciphertext"
          ? Data(repeating: 0xe7, count: KagemushaWireV1.encryptedCreditCiphertextAndTagBytes)
          : encrypted.ciphertextAndTag)
      let credit = try KagemushaMintCreditV1(
        statement: finalStatement,
        proof: shapeProof(semanticDigest: mutation == "semantic" ? digest(0xe6) : semantic),
        finalityCertificateBinding: digest(0x7c), finalityAuthorityHead: digest(0x7d),
        finalityGenesisRosterID: digest(0x7e), finalityProofBindingDigest: digest(0x7f),
        encryptedCredit: finalEncrypted,
        artifactManifestDigest: mutation == "artifactManifest"
          ? digest(0xe5) : context.artifactManifestDigest)
      return (credit, authorization)
    }

    private static func shapeProof(semanticDigest: Data) throws -> KagemushaPairedProofV1 {
      try KagemushaPairedProofV1(
        eqProtocolDigest: digest(0x80), epProtocolDigest: digest(0x81),
        semanticDigest: semanticDigest,
        guardEqCredentialAudit: digest(0x82), guardEpCredentialAudit: digest(0x83),
        eqDeferredAudit: digest(0x84), epDeferredAudit: digest(0x85), eqProof: Data([0x86]),
        epProof: Data([0x87]),
        eqHistory: Data(repeating: 0x88, count: KagemushaWireV1.historyAccumulatorBytes),
        epHistory: Data(repeating: 0x89, count: KagemushaWireV1.historyAccumulatorBytes))
    }

    private static func digest(_ byte: UInt8) -> Data {
      Data(repeating: byte, count: 32)
    }
  }

  private func digest(_ byte: UInt8) -> Data {
    Data(repeating: byte, count: 32)
  }

  private func loadCanonicalFixture(
    named filename: String = "kagemusha_v1.json"
  ) throws -> [String: Any] {
    var current = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while current.path != "/" {
      let candidate = current.appendingPathComponent("fixtures/offline/\(filename)")
      if FileManager.default.fileExists(atPath: candidate.path) {
        return try XCTUnwrap(
          JSONSerialization.jsonObject(with: Data(contentsOf: candidate)) as? [String: Any])
      }
      current.deleteLastPathComponent()
    }
    throw NSError(
      domain: "KagemushaWireV1Tests", code: 1,
      userInfo: [NSLocalizedDescriptionKey: "fixtures/offline/\(filename) was not found"])
  }

  private func sourceFile(named name: String) throws -> URL {
    var current = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while current.path != "/" {
      let candidate =
        current
        .appendingPathComponent("Sources/IrohaSwift")
        .appendingPathComponent(name)
      if FileManager.default.fileExists(atPath: candidate.path) { return candidate }
      current.deleteLastPathComponent()
    }
    throw NSError(
      domain: "KagemushaWireV1Tests", code: 4,
      userInfo: [NSLocalizedDescriptionKey: "KAGEMUSHA Swift source was not found"])
  }

  private func fixtureHex(_ section: [String: Any], field: String = "norito_hex") throws -> Data {
    try fixtureHexString(XCTUnwrap(section[field] as? String))
  }

  private func fixtureHexString(_ hex: String) throws -> Data {
    guard hex.utf8.count.isMultiple(of: 2) else {
      throw NSError(
        domain: "KagemushaWireV1Tests", code: 2,
        userInfo: [NSLocalizedDescriptionKey: "fixture hex length is odd"])
    }
    var bytes = Data()
    var index = hex.startIndex
    while index != hex.endIndex {
      let next = hex.index(index, offsetBy: 2)
      guard let byte = UInt8(hex[index..<next], radix: 16) else {
        throw NSError(
          domain: "KagemushaWireV1Tests", code: 3,
          userInfo: [NSLocalizedDescriptionKey: "fixture hex is invalid"])
      }
      bytes.append(byte)
      index = next
    }
    return bytes
  }
}
