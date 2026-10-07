import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaWalletOpenV1Tests: XCTestCase {
  func testOriginalIntakeBoundsAndNativeLayout() throws {
    let one = Data([7])
    let value = try KagemushaWalletOpenOriginalsV1(credential: one, enrollmentCertificates: one, account: one, assetScope: one)
    value.withRequest {
      XCTAssertEqual($0.pointee.credential_length, 1)
      XCTAssertEqual($0.pointee.certificates_length, 1)
      XCTAssertEqual($0.pointee.account_length, 1)
      XCTAssertEqual($0.pointee.asset_length, 1)
      XCTAssertEqual($0.pointee.credential!.pointee, 7)
    }
    for role in 0..<4 {
      var values = [one, one, one, one]
      values[role] = Data(repeating: 0, count: [1_024, 10_000, 4_096, 1_024][role] + 1)
      XCTAssertThrowsError(try KagemushaWalletOpenOriginalsV1(credential: values[0], enrollmentCertificates: values[1], account: values[2], assetScope: values[3]))
      values[role] = Data()
      XCTAssertThrowsError(try KagemushaWalletOpenOriginalsV1(credential: values[0], enrollmentCertificates: values[1], account: values[2], assetScope: values[3]))
    }
    XCTAssertThrowsError(try KagemushaWalletRuntimeV1(nativeRuntimeHandle: 0))
  }
  func testAccountChallengeAndOpenedHandleAreDistinctBoundedOutcomes() throws {
    XCTAssertNoThrow(try KagemushaWalletCallV1(status: 15, sequenceLow: 1, sequenceHigh: 0, detail: 0, bytes: Data(repeating: 1, count: 32)))
    XCTAssertNoThrow(try KagemushaWalletCallV1(status: 16, sequenceLow: 1, sequenceHigh: 0, detail: 0, bytes: Data()))
    for count in [0, 31, 33] {
      XCTAssertThrowsError(try KagemushaWalletCallV1(status: 15, sequenceLow: 1, sequenceHigh: 0, detail: 0, bytes: Data(repeating: 1, count: count)))
    }
    XCTAssertThrowsError(try KagemushaWalletCallV1(status: 16, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data()))
    XCTAssertThrowsError(try KagemushaWalletCallV1(status: 16, sequenceLow: 1, sequenceHigh: 1, detail: 0, bytes: Data()))
  }
}
