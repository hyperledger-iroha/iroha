import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaSpartanWalletV1Tests: XCTestCase {
  func testCompletedNonSendHasNoPaymentButHeadAndCompletionRemainRequired() throws {
    let empty = KagemushaSpartanReplyV1(status: 1, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data())
    guard case .released(let bytes) = try KagemushaSpartanRetainedOutputV1(empty, part: .payment) else {
      return XCTFail("Completed operation must remain released when it has no outgoing Payment")
    }
    XCTAssertTrue(bytes.isEmpty)
    for part: KagemushaSpartanOutputV1 in [.head, .completion] {
      XCTAssertThrowsError(try KagemushaSpartanRetainedOutputV1(empty, part: part))
    }
    let pending = KagemushaSpartanReplyV1(status: 2, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data([1]))
    XCTAssertThrowsError(try KagemushaSpartanRetainedOutputV1(pending, part: .payment))
  }
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
  func testSignedTimeAnchorReviewBindsItsFloorAndHeads() throws {
    var bytes = Data(repeating: 0, count: 243)
    bytes[0] = 2; bytes[2] = 7
    for (index, offset) in [3, 35, 67, 99, 131, 179, 211].enumerated() {
      bytes.replaceSubrange(offset..<offset + 32, with: repeatElement(UInt8(index + 1), count: 32))
    }
    bytes[170] = 0x80
    bytes[178] = 0x80
    bytes[171] = 1
    let anchor = try KagemushaSpartanTimeAnchorProjectionV2(bytes)
    XCTAssertEqual(anchor.issuerTime, UInt64(1) << 63)
    XCTAssertEqual(anchor.successorFloor, (UInt64(1) << 63) + 1)
    for (index, field) in [anchor.schemeId, anchor.walletId, anchor.operationId, anchor.capsuleDigest,
      anchor.anchorDigest, anchor.beforeHead, anchor.afterHead].enumerated() {
      XCTAssertEqual(field, Data(repeating: UInt8(index + 1), count: 32))
    }
    XCTAssertThrowsError(try KagemushaSpartanReviewProjectionV1(bytes))
    XCTAssertThrowsError(try KagemushaSpartanTimeAnchorProjectionV2(projection()))
    for malformed in [Data(bytes.dropLast()), bytes + Data([0])] {
      XCTAssertThrowsError(try KagemushaSpartanTimeAnchorProjectionV2(malformed))
    }
    for (offset, value) in [(0, UInt8(1)), (1, 1), (2, 6), (178, 0)] {
      var malformed = bytes; malformed[offset] = value
      XCTAssertThrowsError(try KagemushaSpartanTimeAnchorProjectionV2(malformed))
    }
    bytes[171] = 0
    XCTAssertEqual(try KagemushaSpartanTimeAnchorProjectionV2(bytes).successorFloor, anchor.issuerTime)
  }

  private func unloadReviewOriginal() throws -> Data {
    var directory = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
    for _ in 0..<8 {
      let file = directory.appendingPathComponent("fixtures/kagemusha/spartan_unload_review_v3.bin")
      if FileManager.default.fileExists(atPath: file.path) { return try Data(contentsOf: file) }
      directory.deleteLastPathComponent()
    }
    throw CocoaError(.fileNoSuchFile)
  }

  func testUnloadReviewBindsBothDestinationAccountsAndCharge() throws {
    let wire = try unloadReviewOriginal()
    let review = try KagemushaSpartanUnloadProjectionV3(wire)
    XCTAssertEqual(review.faceDebit, .init(low: 4, high: 0))
    XCTAssertEqual(review.onlineCharge, .init(low: 1, high: 0))
    XCTAssertEqual(review.netToAccount, .init(low: 3, high: 0))
    let beneficiary = try XCTUnwrap(review.beneficiaryAccountBytes)
    XCTAssertNotEqual(beneficiary, review.credentialAccountBytes)
    XCTAssertEqual(try AccountAddress.fromCanonicalBytes(beneficiary).canonicalBytes(), beneficiary)
    XCTAssertEqual(try AccountAddress.fromCanonicalBytes(review.credentialAccountBytes).canonicalBytes(),
      review.credentialAccountBytes)
    XCTAssertThrowsError(try KagemushaSpartanReviewProjectionV1(wire))
    XCTAssertThrowsError(try KagemushaSpartanTimeAnchorProjectionV2(wire))

    var free = Data(wire.prefix(421 + review.credentialAccountBytes.count))
    free.append(contentsOf: [0, 0])
    for range in [227..<243, 259..<291, 323..<355] { free.resetBytes(in: range) }
    free[243] = 4
    XCTAssertNil(try KagemushaSpartanUnloadProjectionV3(free).beneficiaryAccountBytes)
    var borrowed = wire
    borrowed.resetBytes(in: 211..<227); borrowed[219] = 1
    borrowed.replaceSubrange(243..<259, with: [UInt8](repeating: 255, count: 8) + [UInt8](repeating: 0, count: 8))
    XCTAssertEqual(try KagemushaSpartanUnloadProjectionV3(borrowed).netToAccount, .init(low: .max, high: 0))
    var allCharged = wire
    allCharged[227] = 4; allCharged[243] = 0
    XCTAssertEqual(try KagemushaSpartanUnloadProjectionV3(allCharged).netToAccount, .init(low: 0, high: 0))

    for change in [0, 1, 2, 211, 243, 420] {
      var invalid = wire; invalid[change] ^= 0x80
      XCTAssertThrowsError(try KagemushaSpartanUnloadProjectionV3(invalid))
    }
    for erase in [163..<195, 211..<227, 227..<243, 259..<291, 323..<355, 419..<421] {
      var invalid = wire; invalid.resetBytes(in: erase)
      XCTAssertThrowsError(try KagemushaSpartanUnloadProjectionV3(invalid))
    }
    for invalid in [Data(wire.dropLast()), wire + Data([0]), Data(repeating: 0, count: 16_385)] {
      XCTAssertThrowsError(try KagemushaSpartanUnloadProjectionV3(invalid))
    }
  }

  func testExecuteValidatesNativeSequenceAndStatusSpecificReason() throws {
    func execution(_ status: Int32, low: UInt64 = 0, high: UInt64 = 0, detail: UInt32 = 0) throws -> KagemushaSpartanExecutionV1 {
      try .init(.init(status: status, sequenceLow: low, sequenceHigh: high, detail: detail,
        bytes: status == 1 ? Data([1]) : Data()))
    }
    for status in Int32(1)...5 {
      XCTAssertThrowsError(try execution(status, low: 1))
      XCTAssertThrowsError(try execution(status, high: 1))
      if status != 3 { XCTAssertThrowsError(try execution(status, detail: 1)) }
    }
    for reason in UInt32(0)...2 { XCTAssertEqual(try execution(3, detail: reason).detail, reason) }
    XCTAssertThrowsError(try execution(3, detail: 3))
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
