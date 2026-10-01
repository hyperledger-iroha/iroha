import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Untrusted projection/grammar tests; no endpoint supplies a native capability here.
final class KagemushaAppPlatformPreparedProjectionV1Tests: XCTestCase {
  func testC451PreparationRejectsRetiredLayoutAndSubstitutedNativeScope() throws {
    let f = try preparation()
    let p = try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: digest(0x11), enrollmentChallengeHash: nil)
    XCTAssertEqual(p.platform, 4); XCTAssertEqual(p.appleCounterFloor, 7)
    var enumTaggedC = f
    let cStart = Data("iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0".utf8).count + 8
    enumTaggedC[7][cStart + 2] = 4
    enumTaggedC[4] = Data(SHA256.hash(data: enumTaggedC[7]))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: enumTaggedC,
      approvalID: digest(0x11), enrollmentChallengeHash: nil))
    XCTAssertEqual(p.approval!.clientDataHash, Data(SHA256.hash(data: f[1])))
    for index in [0, 2, 3, 4, 5, 6, 8, 9, 10, 11, 12, 13] {
      var bad = f; bad[index] = Data()
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: bad,
        approvalID: digest(0x11), enrollmentChallengeHash: nil), "field \(index)")
    }
    var retired = f
    let domain = Data("iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0".utf8)
    retired[7].replaceSubrange(domain.count..<(domain.count + 8), with: u64(443))
    retired[7].removeSubrange((domain.count + 8 + 427)..<(domain.count + 8 + 435))
    retired[4] = Data(SHA256.hash(data: retired[7]))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: retired,
      approvalID: digest(0x11), enrollmentChallengeHash: nil))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: digest(0x12), enrollmentChallengeHash: nil))
  }

  func testBootstrapCannotBecomeOrdinaryWDespiteMatchingSubjectHash() throws {
    var f = try preparation()
    f[13][331] = 0
    f[13].replaceSubrange(428..<460, with: Data(repeating: 0, count: 32))
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    f[1].replaceSubrange((start + 195)..<(start + 227), with: Data(SHA256.hash(data: f[13])))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: digest(0x11), enrollmentChallengeHash: nil))
  }

  func testPreparationRejectsCredentialAndGenerationSubstitutionDespiteRecomputedSubjectHash() throws {
    let f = try preparation()
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    for changedCredential in [true, false] {
      var bad = f
      if changedCredential {
        bad[13].replaceSubrange(155..<187, with: digest(0x67))
      } else {
        bad[13].replaceSubrange(323..<331, with: u64(3))
      }
      bad[1].replaceSubrange((start + 195)..<(start + 227),
        with: Data(SHA256.hash(data: bad[13])))
      // The standalone codec still accepts a correctly hashed S/W pair; only
      // the called native preparation projection binds the original C/credential.
      XCTAssertNoThrow(try KagemushaAppApprovalSigningProjectionV1(
        nativeSigningBytes: bad[1], nativeFinancialSubject: bad[13]))
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: bad,
        approvalID: digest(0x11), enrollmentChallengeHash: nil))
    }
  }

  func testPreparationRejectsOriginalCScopeSubstitutionDespiteRecomputedSubjectHash() throws {
    let f = try preparation()
    let start = KagemushaAppApprovalSigningProjectionV1.signingDomain.count + 8
    for (range, replacement) in [(59..<91, digest(0x91)), (187..<219, digest(0x92)),
      (219..<251, digest(0x93)), (251..<283, digest(0x94)), (283..<291, u64(2))] {
      var bad = f
      bad[13].replaceSubrange(range, with: replacement)
      bad[1].replaceSubrange((start + 195)..<(start + 227),
        with: Data(SHA256.hash(data: bad[13])))
      XCTAssertNoThrow(try KagemushaAppApprovalSigningProjectionV1(
        nativeSigningBytes: bad[1], nativeFinancialSubject: bad[13]))
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: bad,
        approvalID: digest(0x11), enrollmentChallengeHash: nil), "range \(range)")
    }
  }

  func testEnrollmentProjectionRequiresEmptyCredentialAndFinancialSubject() throws {
    var f = try preparation()
    f[1] = enrollmentPossession(key: f[6]); f[8] = Data(); f[13] = Data()
    let p = try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: nil, enrollmentChallengeHash: Data(SHA256.hash(data:challenge())))
    XCTAssertNil(p.approval); XCTAssertTrue(p.credentialDigest.isEmpty)
    var oldAttempt=f
    let eStart=Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8).count+8
    oldAttempt[1].replaceSubrange((eStart+3)..<(eStart+35),with:digest(1))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields:oldAttempt,
      approvalID:nil,enrollmentChallengeHash:Data(SHA256.hash(data:challenge()))))
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields:f,
      approvalID:nil,enrollmentChallengeHash:digest(1)))

    for index in [8, 13] {
      var bad = f; bad[index] = digest(0x55)
      XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: bad,
        approvalID: nil, enrollmentChallengeHash: Data(SHA256.hash(data:challenge()))))
    }
    XCTAssertThrowsError(try KagemushaAppPlatformPreparedProjectionV1(nativeFields: f,
      approvalID: digest(1), enrollmentChallengeHash: nil))
  }

  func testFixedReceiptRejectsOmittedTrailingAndCrossPlatformCounterFields() throws {
    let r = receipt(raw: Data([1, 2, 3]))
    let p = try KagemushaAppPlatformReceiptProjectionV1(r)
    XCTAssertEqual(r.count, 184); XCTAssertEqual(p.appleCounter, 8)
    let nonzeroBasedSlice = (Data(repeating: 0, count: 7) + r).dropFirst(7)
    XCTAssertEqual(try KagemushaAppPlatformReceiptProjectionV1(nonzeroBasedSlice).appleCounter, 8)
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(Data(r.dropLast())))
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(r + Data([0])))
    var bad = r; bad[179] = 0
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(bad))
    bad = r; bad[10] = 0
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(bad))
    bad = r; bad.replaceSubrange(180..<184, with: Data(repeating: 0, count: 4))
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(bad))
  }

  func testPhaseGrammarRejectsUnknownAndRecoveredEvidenceSubstitution() throws {
    let ticket = u64(9), raw = Data([1, 2, 3])
    try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval, [u32(3), ticket, raw])
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval,
      [u32(8), ticket]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval,
      [u32(3), ticket, Data(repeating: 1, count: 4097)]))
    try KagemushaAppPlatformFrameV1.validateResponse(.appOperationApproval, [u32(5), ticket],
      [Data([2]), raw, receipt(raw: raw)])
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appOperationApproval,
      [u32(5), ticket], [Data([2]), Data([4]), receipt(raw: raw)]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
      [u32(5), ticket], [Data([2]), raw, receipt(raw: raw)]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appOperationApproval,
      [u32(2), ticket], [Data([1]), raw, Data()]))
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appOperationApproval,
      [u32(3), ticket, raw], [digest(1)]))
  }

  private func preparation() throws -> [Data] {
    let point = try P256.Signing.PrivateKey(rawRepresentation: digest(7)).publicKey.x963Representation
    let key = Data(SHA256.hash(data: point))
    let fixture = try String(contentsOfFile: #filePath.replacingOccurrences(
      of: "KagemushaAppPlatformPreparedProjectionV1Tests.swift",
      with: "Fixtures/kagemusha_app_platform_messages_v1.tsv"), encoding: .utf8)
    let row = fixture.split(separator: "\n").first { $0.hasPrefix("s_mint_native_first\t") }!
    let hex = row.split(separator: "\t")[1]
    var subject = Data(stride(from: 0, to: hex.count, by: 2).map { offset -> UInt8 in
      let start = hex.index(hex.startIndex, offsetBy: offset)
      return UInt8(hex[start..<hex.index(start, offsetBy: 2)], radix: 16)!
    })
    // These are explicitly untrusted correlated projections; the actual Rust
    // codec vectors remain byte-exact in their separate conformance test.
    subject.replaceSubrange(155..<187, with: digest(0x66))
    subject.replaceSubrange(323..<331, with: u64(2))
    subject.replaceSubrange(59..<91, with: digest(7))
    subject.replaceSubrange(187..<219, with: digest(5))
    subject.replaceSubrange(219..<251, with: digest(6))
    subject.replaceSubrange(251..<283, with: digest(8))
    subject.replaceSubrange(283..<291, with: u64(1))
    var w = Data("iroha:kagemusha:v1:app-operation-approval\0".utf8) + u64(275) + Data([1, 0, 1])
    for d in [digest(0x11), digest(0x22), digest(4), digest(11), key, digest(0x66),
      Data(SHA256.hash(data: Data(subject))), digest(0x88)] { w.append(d) }
    w.append(u64(1000)); w.append(u64(121000))
    let c = challenge()
    return [u64(9), w, Data([4]), Data(key.base64EncodedString().utf8),
      Data(SHA256.hash(data: c)), point, key, c, digest(0x66), digest(0x99),
      u32(7), Data([0]), digest(0xaa), Data(subject)]
  }

  private func challenge() -> Data {
    var c = Data("iroha:kagemusha:v1:ordinary-app-enrollment-challenge\0".utf8)
      + u64(451) + Data([1, 0, 2])
    for i in 1...13 { c.append(digest(UInt8(i))) }
    for i in [UInt64(1), 2, 1000, 121000] { c.append(u64(i)) }
    return c
  }
  private func enrollmentPossession(key: Data) -> Data {
    var e = Data("iroha:kagemusha:v1:app-enrollment-possession\0".utf8)
      + u64(371) + Data([1, 0, 1])
    for d in [Data(SHA256.hash(data:challenge())), digest(2), digest(3), digest(4), digest(5), digest(11),
      digest(7), digest(8), digest(6), key, digest(0x77)] { e.append(d) }
    e.append(u64(1000)); e.append(u64(121000)); return e
  }
  private func receipt(raw: Data) -> Data {
    var r = Data("KGMAPP1\0".utf8) + Data([1, 0, 1]) + u64(9)
    for d in [digest(0x11), digest(0x99), digest(0x22), Data(SHA256.hash(data: raw)), digest(0x66)] {
      r.append(d)
    }
    r.append(1); r.append(u32(8)); return r
  }
  private func digest(_ byte: UInt8) -> Data { Data(repeating: byte, count: 32) }
  private func u32(_ value: UInt32) -> Data {
    var value = value.littleEndian; return withUnsafeBytes(of: &value) { Data($0) }
  }
  private func u64(_ value: UInt64) -> Data {
    var value = value.littleEndian; return withUnsafeBytes(of: &value) { Data($0) }
  }
}
