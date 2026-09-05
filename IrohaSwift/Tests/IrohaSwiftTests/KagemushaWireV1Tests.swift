import XCTest
@testable import IrohaSwift

final class KagemushaWireV1Tests: XCTestCase {
  func testV1PeerExchangeHasOnlyThreeCanonicalMessages() {
    XCTAssertEqual(IrohaPeerWireKindV1.allCases.map(\.rawValue), [1, 2, 3])
    XCTAssertEqual(
      IrohaPeerWireKindV1.allCases.map(\.requiredKagemushaCanonicalSchema),
      [
        "iroha_data_model::kagemusha::kagemusha_v1::KagemushaPaymentRequestV1",
        "iroha_data_model::kagemusha::kagemusha_v1::KagemushaPaymentV1",
        "iroha_data_model::kagemusha::kagemusha_v1::KagemushaAcknowledgementV1",
      ]
    )
    XCTAssertEqual(IrohaPeerWireKindV1.allCases.map(\.requiredKagemushaPayloadAlignment), [16, 16, 2])
  }

  func testV1TextTransportUsesKagemushaPrefix() {
    XCTAssertEqual(KagemushaWireV1.textPrefix, "kgm1:")
  }

  func testSharedFixtureUsesOnlyTheThreeMessageProtocol() throws {
    let fixture = try loadFixture()
    XCTAssertEqual(try XCTUnwrap(fixture["fixture_version"] as? Int), 1)
    XCTAssertNil(fixture["acceptance_intent"])
    XCTAssertNil(fixture["acceptance_ticket"])
    XCTAssertNil(fixture["complete_five_message"])

    let requestBytes = try fixtureBytes(fixture, section: "payment_request")
    let paymentBytes = try fixtureBytes(fixture, section: "payment")
    let acknowledgementBytes = try fixtureBytes(fixture, section: "acknowledgement")
    let request = try KagemushaNoritoV1.decodePaymentRequestShapeExact(requestBytes)
    let payment = try KagemushaNoritoV1.decodePaymentShapeExact(
      paymentBytes, against: request)
    let acknowledgement = try KagemushaNoritoV1.decodeAcknowledgementShapeExact(
      acknowledgementBytes, against: request, payment: payment)

    XCTAssertEqual(try KagemushaNoritoV1.encodePaymentRequestShape(request), requestBytes)
    XCTAssertEqual(
      try KagemushaNoritoV1.encodePaymentShape(payment, against: request), paymentBytes)
    XCTAssertEqual(
      try KagemushaNoritoV1.encodeAcknowledgementShape(
        acknowledgement, against: request, payment: payment),
      acknowledgementBytes)
  }

  private func loadFixture() throws -> [String: Any] {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    while directory.path != "/" {
      let candidate = directory.appendingPathComponent("fixtures/offline/kagemusha_v1.json")
      if FileManager.default.fileExists(atPath: candidate.path) {
        return try XCTUnwrap(
          JSONSerialization.jsonObject(with: Data(contentsOf: candidate)) as? [String: Any])
      }
      directory.deleteLastPathComponent()
    }
    throw NSError(domain: "KagemushaWireV1Tests", code: 1)
  }

  private func fixtureBytes(_ fixture: [String: Any], section: String) throws -> Data {
    let object = try XCTUnwrap(fixture[section] as? [String: Any])
    let hex = try XCTUnwrap(object["norito_hex"] as? String)
    guard hex.count.isMultiple(of: 2) else {
      throw NSError(domain: "KagemushaWireV1Tests", code: 2)
    }
    var bytes = Data()
    var index = hex.startIndex
    while index != hex.endIndex {
      let end = hex.index(index, offsetBy: 2)
      guard let byte = UInt8(hex[index..<end], radix: 16) else {
        throw NSError(domain: "KagemushaWireV1Tests", code: 3)
      }
      bytes.append(byte)
      index = end
    }
    return bytes
  }
}
