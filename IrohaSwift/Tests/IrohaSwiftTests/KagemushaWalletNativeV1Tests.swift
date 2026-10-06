import Foundation
import XCTest

@testable import IrohaSwift

/// Managed result contracts; authenticated native artifact execution remains separately gated.
final class KagemushaWalletNativeV1Tests: XCTestCase {
  func testExactBytesAndDistinctCompletionOutcomes() throws {
    let bytes = Data([0, 255, 0, 7])
    for status in Int32(0)...10 {
      let value = try KagemushaWalletCallV1(
        status: status, sequenceLow: 7, sequenceHigh: 2,
        detail: 0, bytes: status == 1 || status == 10 ? bytes : Data())
      XCTAssertEqual(value.status, status)
      XCTAssertEqual(value.sequenceHigh, 2)
      if status == 1 { XCTAssertEqual(value.bytes, bytes) }
    }
  }
  func testMalformedNativeOutputNeverBecomesCompletion() {
    for (status, bytes) in [
      (Int32(1), Data()), (2, Data([1])), (11, Data()), (-1, Data()),
      (1, Data(repeating: 1, count: 10_001)),
    ] {
      XCTAssertThrowsError(
        try KagemushaWalletCallV1(
          status: status, sequenceLow: 0,
          sequenceHigh: 0, detail: 0, bytes: bytes))
    }
  }
}
