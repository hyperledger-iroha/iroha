import XCTest
@testable import IrohaSwift

final class KagemushaWireV1Tests: XCTestCase {
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
    XCTAssertEqual(KagemushaWireV1.maximumSessionRawBytes, 9_211)
    XCTAssertEqual(KagemushaWireV1.maximumSessionTextBytes, 12_288)
  }
}
