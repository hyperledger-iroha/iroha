import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Actual frame and called projection tests only. Synthetic nonzero correlation
/// fields here supply no native endpoint, admitted owner or issuer credential.
final class KagemushaAppEnrollmentFinalIdentityFrameV1Tests: XCTestCase {
  func testOnlyPossessionCanSupplyBoundedFinalCredentialOriginal() throws {
    let ticket = u64(9)
    for size in [1, 16384] {
      let f = [u32(8), ticket, Data(repeating: 7, count: size)]
      // Shape accepts bounded untrusted bytes; native canonical/signature admission
      // remains mandatory and is not simulated by this test.
      try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession, f)
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appOperationApproval, f))
    }
    for archive in [Data(), Data(repeating: 7, count: 16385)] {
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession,
        [u32(8), ticket, archive]))
    }
    for bad in [[u32(8), ticket], [u32(8), ticket, Data([1]), Data([2])],
      [u32(8), Data(repeating: 0, count: 8), Data([1])]] {
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateRequest(.appEnrollmentPossession, bad))
    }
  }

  func testFinalIdentityProjectionRejectsMissingOrForeignPendingScope() throws {
    let scope = digest(0x99), credential = digest(0x44)
    let accepted = try KagemushaAppEnrollmentFinalIdentityProjectionV1(
      nativeFields: [credential, scope], originalPendingScope: scope)
    XCTAssertEqual(accepted.credentialDigest, credential)
    XCTAssertEqual(accepted.pendingScope, scope)
    for f in [[credential], [credential, scope, digest(1)], [Data(), scope],
      [Data(repeating: 0, count: 32), scope], [credential, digest(0x98)]] {
      XCTAssertThrowsError(try KagemushaAppEnrollmentFinalIdentityProjectionV1(
        nativeFields: f, originalPendingScope: scope))
    }
    XCTAssertThrowsError(try KagemushaAppEnrollmentFinalIdentityProjectionV1(
      nativeFields: [credential, scope], originalPendingScope: Data(repeating: 0, count: 32)))
  }

  func testFinalIdentityResponseCannotReplacePossessionReceipt() throws {
    let ticket = u64(9), scope = digest(0x99), credential = digest(0x44)
    let request = [u32(8), ticket, Data([7])]
    try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
      request, [credential, scope])
    for f in [[credential], [credential, scope, digest(1)], [credential, Data()],
      [credential, Data(repeating: 0, count: 32)], [receipt(raw: Data([1]))]] {
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
        request, f))
    }
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
      [u32(4), ticket], [credential, scope]))
    XCTAssertThrowsError(try KagemushaAppPlatformReceiptProjectionV1(credential + scope))
  }

  func testConsumedOriginalRecoveryPreservesPurposeTwoAndExactEvidence() throws {
    let ticket = u64(9), raw = Data([1, 2, 3]), originalReceipt = receipt(raw: Data([1, 2, 3]))
    try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
      [u32(5), ticket], [Data([2]), raw, originalReceipt])
    // The grammar has no handset timestamp or renewed C. The native owner alone
    // checks consumed-at originals and current policy/credential/Integrity on recovery.
    for f in [[Data([0]), raw, originalReceipt], [Data([2]), Data([4]), originalReceipt]] {
      XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
        [u32(5), ticket], f))
    }
    var foreignPurpose = originalReceipt; foreignPurpose[10] = 1
    XCTAssertThrowsError(try KagemushaAppPlatformFrameV1.validateResponse(.appEnrollmentPossession,
      [u32(5), ticket], [Data([2]), raw, foreignPurpose]))
  }

  private func receipt(raw: Data) -> Data {
    var r = Data("KGMAPP1\0".utf8) + Data([1, 0, 2]) + u64(9)
    for d in [digest(0x11), digest(0x99), digest(0x22), Data(SHA256.hash(data: raw)), digest(0x99)] {
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
