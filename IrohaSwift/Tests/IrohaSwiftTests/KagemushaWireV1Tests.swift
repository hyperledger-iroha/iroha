import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaWireV1Tests: XCTestCase {
  func testLifecycleOperationInventoryMatchesRust() {
    XCTAssertEqual(
      KagemushaOperationKindV1.allCases,
      [.bootstrap, .mintFold, .sendSplit, .receiveFold, .redeemSplit, .rotate])
    XCTAssertEqual(
      KagemushaOperationKindV1.allCases.map(\.rawValue),
      (0...5).map { UInt32($0) })
  }

  func testRustCanonicalThreeMessageFixtureRoundTripsByteForByte() throws {
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
        ("payment", "payment", 2, .payment, .payment),
        ("acknowledgement", "acknowledgement", 3, .acknowledgement, .acknowledgement),
      ]
    let order = try XCTUnwrap(fixture["ipm1_message_order"] as? [[String: Any]])
    XCTAssertEqual(order.count, expectedOrder.count)

    var sections: [String: Data] = [:]
    var completeTextBytes = 0
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
      completeTextBytes += text.utf8.count
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
          message.encoded,
          expectedProfile: .kagemushaV1,
          expectedKind: expected.wireKind
        ).canonicalPayload,
        raw)
    }

    let requestRaw = try XCTUnwrap(sections["payment_request"])
    let paymentRaw = try XCTUnwrap(sections["payment"])
    let acknowledgementRaw = try XCTUnwrap(sections["acknowledgement"])
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(requestRaw)
    let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
      paymentRaw,
      against: request)
    let acknowledgement = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(
      acknowledgementRaw,
      against: request,
      payment: payment)
    XCTAssertEqual(try KagemushaNoritoV1.encodePaymentRequestShape(request), requestRaw)
    XCTAssertEqual(
      try KagemushaNoritoV1.encodePaymentShape(payment, against: request),
      paymentRaw)
    XCTAssertEqual(
      try KagemushaNoritoV1.encodeAcknowledgementShape(
        acknowledgement,
        against: request,
        payment: payment),
      acknowledgementRaw)

    let identities = try XCTUnwrap(fixture["identity_vectors"] as? [String: Any])
    for (key, actual) in [
      ("payment_request_digest_hex", KagemushaNoritoV1.paymentRequestDigest(request)),
      (
        "prepared_transfer_digest_hex",
        try KagemushaNoritoV1.preparedTransferDigestShape(
          request: request,
          senderBeforeCommitment: payment.output.senderBeforeCommitment,
          senderAfterCommitment: payment.output.senderAfterCommitment,
          transitionNullifier: payment.output.transitionNullifier,
          ciphertextCommitment: payment.output.ciphertextCommitment)
      ),
      ("payment_output_digest_hex", KagemushaNoritoV1.paymentOutputDigestShape(payment.output)),
      (
        "payment_body_digest_hex",
        try KagemushaNoritoV1.paymentBodyDigestShape(
          output: payment.output,
          encryptedCredit: payment.encryptedCredit)
      ),
      (
        "payment_digest_hex",
        try KagemushaNoritoV1.paymentDigestShape(payment, against: request)
      ),
      ("ciphertext_digest_hex", KagemushaNoritoV1.ciphertextDigestShape(payment.encryptedCredit)),
      ("credit_id_hex", payment.output.creditID),
      ("acknowledgement_digest_hex", Data(SHA256.hash(data: acknowledgementRaw))),
    ] {
      XCTAssertEqual(actual, try fixtureHexString(XCTUnwrap(identities[key] as? String)), key)
    }

    let proofRaw = try fixtureHex(XCTUnwrap(fixture["payment_proof"] as? [String: Any]))
    XCTAssertEqual(try KagemushaNoritoV1.encodePaymentProofShape(payment.proof), proofRaw)
    XCTAssertEqual(try KagemushaNoritoV1.decodePaymentProofShapeExact(proofRaw), payment.proof)
    let certificateRaw = try fixtureHex(
      XCTUnwrap(fixture["commit_certificate"] as? [String: Any]))
    XCTAssertEqual(
      try KagemushaNoritoV1.encodeCommitCertificateShape(payment.commitCertificate),
      certificateRaw)
    XCTAssertEqual(
      try KagemushaNoritoV1.decodeCommitCertificateShapeExact(certificateRaw),
      payment.commitCertificate)

    let complete = try XCTUnwrap(fixture["complete_exchange"] as? [String: Any])
    let completeRawBytes = expectedOrder.reduce(0) {
      $0 + sections[$1.section, default: Data()].count
    }
    XCTAssertEqual(complete["messages"] as? [String], expectedOrder.map(\.name))
    XCTAssertEqual(completeRawBytes, try XCTUnwrap(complete["raw_bytes"] as? Int))
    XCTAssertEqual(completeTextBytes, try XCTUnwrap(complete["text_bytes"] as? Int))
    XCTAssertEqual(
      try KagemushaNoritoV1.validateCompleteExchangeShape(
        request: request,
        payment: payment,
        acknowledgement: acknowledgement),
      completeRawBytes)
  }

  func testSoleTextPrefixAndCanonicalPayloadKinds() throws {
    XCTAssertEqual(KagemushaWireV1.textPrefix, "kgm1:")
    let expected: [KagemushaWirePayloadKindV1] = [
      .paymentRequest, .payment, .acknowledgement, .mintAuthorization, .mintCredit,
      .redemptionVoucher,
    ]
    XCTAssertEqual(KagemushaWirePayloadKindV1.allCases, expected)
    for kind in KagemushaWirePayloadKindV1.allCases {
      let bytes = Data([0xa5])
      let text = try KagemushaWireV1.encodeText(bytes, kind: kind)
      XCTAssertTrue(text.hasPrefix("kgm1:"))
      XCTAssertEqual(try KagemushaWireV1.decodeText(text, kind: kind), bytes)
      XCTAssertThrowsError(
        try KagemushaWireV1.decodeText("oc" + "1:" + String(text.dropFirst(5)), kind: kind))
    }
  }

  func testParityNativeStateCommitmentRequiresBothComponents() throws {
    let value = try KagemushaPastaStateCommitmentV1(
      eq: Data(repeating: 0x11, count: 32), ep: Data(repeating: 0x22, count: 32))
    XCTAssertFalse(value.isZero)
    XCTAssertEqual(value.eq.count, 32)
    XCTAssertEqual(value.ep.count, 32)
    XCTAssertEqual(KagemushaNoritoV1.pastaStateCommitment(value).count, 32)
    XCTAssertThrowsError(
      try KagemushaPastaStateCommitmentV1(
        eq: Data(repeating: 0x11, count: 31), ep: Data(repeating: 0x22, count: 32)))
  }

  func testCompactHistoryIndependentBoundsRemainFixed() {
    XCTAssertEqual(KagemushaWireV1.maximumPairedProofBytes, 6_528)
    XCTAssertEqual(KagemushaWireV1.maximumCompleteExchangeRawBytes, 9_211)
    XCTAssertEqual(KagemushaWireV1.maximumCompleteExchangeTextBytes, 12_288)
  }

  private func loadCanonicalFixture() throws -> [String: Any] {
    var current = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while current.path != "/" {
      let candidate = current.appendingPathComponent("fixtures/offline/kagemusha_v1.json")
      if FileManager.default.fileExists(atPath: candidate.path) {
        return try XCTUnwrap(
          JSONSerialization.jsonObject(with: Data(contentsOf: candidate)) as? [String: Any])
      }
      current.deleteLastPathComponent()
    }
    throw NSError(
      domain: "KagemushaWireV1Tests",
      code: -1,
      userInfo: [NSLocalizedDescriptionKey: "fixtures/offline/kagemusha_v1.json was not found"])
  }

  private func fixtureHex(_ section: [String: Any]) throws -> Data {
    try fixtureHexString(XCTUnwrap(section["norito_hex"] as? String))
  }

  private func fixtureHexString(_ hex: String) throws -> Data {
    guard hex.count.isMultiple(of: 2) else {
      throw NSError(
        domain: "KagemushaWireV1Tests",
        code: -2,
        userInfo: [NSLocalizedDescriptionKey: "fixture hex length is odd"])
    }
    var bytes = Data()
    bytes.reserveCapacity(hex.count / 2)
    var index = hex.startIndex
    while index < hex.endIndex {
      let end = hex.index(index, offsetBy: 2)
      guard let byte = UInt8(hex[index..<end], radix: 16) else {
        throw NSError(
          domain: "KagemushaWireV1Tests",
          code: -3,
          userInfo: [NSLocalizedDescriptionKey: "fixture hex is invalid"])
      }
      bytes.append(byte)
      index = end
    }
    return bytes
  }
}
