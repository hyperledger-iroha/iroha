import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaSpartanWalletV1Tests: XCTestCase {
  func testAdmissionRejectsAnotherCredentialProfileBeforeNative() throws {
    for count in [0, 532, 595, 597] {
      XCTAssertThrowsError(try KagemushaSpartanAccountOriginalsV1(credential: Data(repeating: 0, count: count),
        enrollmentCertificate: Data(repeating: 0, count: 172), account: Data([1]), assetScope: Data([2])))
    }
    for count in [0, 171, 173] {
      XCTAssertThrowsError(try KagemushaSpartanAccountOriginalsV1(credential: Data(repeating: 0, count: 596),
        enrollmentCertificate: Data(repeating: 0, count: count), account: Data([1]), assetScope: Data([2])))
    }
  }
  private func projection() -> Data {
    var bytes = [UInt8](repeating: 0, count: 291)
    bytes[0] = 1; bytes[2] = 3
    for (index, offset) in [3, 35, 67, 99, 163, 195, 227, 259].enumerated() {
      for at in offset..<offset + 32 { bytes[at] = UInt8(index + 1) }
    }
    for (offset, word) in [(131, UInt64.max), (139, UInt64(1) << 63), (147, UInt64(7)), (155, UInt64(9))] {
      for index in 0..<8 { bytes[offset + index] = UInt8(truncatingIfNeeded: word >> (index * 8)) }
    }
    return Data(bytes)
  }
  func testReviewPreservesUnsignedAmountsAndBoundIdentities() throws {
    var bytes = projection()
    let review = try KagemushaSpartanReviewProjectionV1(bytes)
    bytes.resetBytes(in: 0..<bytes.count)
    XCTAssertEqual(review.operation, .send)
    XCTAssertEqual(review.amount, .init(low: .max, high: 1 << 63))
    XCTAssertEqual(review.fee, .init(low: 7, high: 9))
    for (index, identity) in [review.schemeId, review.walletId, review.operationId, review.capsuleDigest,
      review.receiverWalletId, review.receiverAccountId, review.beforeHead, review.afterHead].enumerated() {
      XCTAssertEqual(identity, Data(repeating: UInt8(index + 1), count: 32))
    }
  }
  func testReviewRejectsUnsupportedVersionKindAndLength() {
    for length in [0, 290, 292] {
      XCTAssertThrowsError(try KagemushaSpartanReviewProjectionV1(Data(repeating: 0, count: length)))
    }
    for (offset, byte) in [(0, UInt8(2)), (1, UInt8(1)), (2, UInt8(0)), (2, UInt8(5))] {
      var bytes = projection(); bytes[offset] = byte
      XCTAssertThrowsError(try KagemushaSpartanReviewProjectionV1(bytes))
    }
  }
  func testReplyDomainRejectsFalseCompleteAndInvalidToken() throws {
    func reply(_ status: Int32, _ token: UInt64 = 0, _ bytes: Data = Data()) -> KagemushaSpartanReplyV1 {
      .init(status: status, sequenceLow: token, sequenceHigh: 0, detail: 0, bytes: bytes)
    }
    XCTAssertThrowsError(try reply(82, 0, projection()).requireShape(status: 82, count: 291, token: true))
    XCTAssertThrowsError(try reply(83, 1, Data(repeating: 0, count: 514)).requireShape(status: 83, count: 514))
    XCTAssertThrowsError(try KagemushaSpartanExecutionV1(reply(1)))
    XCTAssertThrowsError(try KagemushaSpartanExecutionV1(reply(2, 0, Data([1]))))
    XCTAssertThrowsError(try KagemushaSpartanExecutionV1(reply(84)))
    XCTAssertEqual(try KagemushaSpartanExecutionV1(reply(2)).status, .pending)
    XCTAssertEqual(try KagemushaSpartanExecutionV1(reply(1, 0, Data([1, 2, 3]))).completion, Data([1, 2, 3]))
  }
}
