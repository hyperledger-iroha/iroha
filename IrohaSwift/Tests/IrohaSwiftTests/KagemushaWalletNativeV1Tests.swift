import Foundation
import XCTest

@testable import IrohaSwift

/// Managed result contracts; authenticated native artifact execution remains separately gated.
final class KagemushaWalletNativeV1Tests: XCTestCase {
  func testExactBytesAndDistinctCompletionOutcomes() throws {
    let bytes = Data([0, 255, 0, 7])
    for status in Int32(0)...11 {
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
      (Int32(1), Data()), (10, Data()), (2, Data([1])), (12, Data()), (-1, Data()),
      (1, Data(repeating: 1, count: 10_001)),
    ] {
      XCTAssertThrowsError(
        try KagemushaWalletCallV1(
          status: status, sequenceLow: 0,
          sequenceHigh: 0, detail: 0, bytes: bytes)
      ) { error in
        XCTAssertEqual(error as? KagemushaWalletErrorV1, .invalidNativeOutput)
      }
    }
    for status in Int32(0)...11 where status != 1 && status != 10 {
      XCTAssertThrowsError(
        try KagemushaWalletCallV1(
          status: status, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data([1])))
    }
  }
  func testCompletionPreservesTheInclusiveEnvelopeBound() throws {
    let bytes = Data(repeating: 0xff, count: 10_000)
    let value = try KagemushaWalletCallV1(
      status: 1, sequenceLow: UInt64.max, sequenceHigh: UInt64.max, detail: 0, bytes: bytes)
    XCTAssertEqual(value.bytes, bytes)
    XCTAssertEqual(value.sequenceLow, UInt64.max)
    XCTAssertEqual(value.sequenceHigh, UInt64.max)
  }
  func testTypedLifecycleInputsHaveExactOriginalBoundsAndScalarProjection() throws {
    let identity = Data(repeating: 1, count: 32)
    for selector in UInt32(0)...9 {
      let limits: [Int] = selector == 0 ? [512, 16_384, 0] : selector == 1 ? [10_000, 0, 0]
        : selector == 2 ? [10_000, 1_024, 10_000] : selector == 5 ? [65_536 * 34 + 512, 10_000, 0]
        : selector == 6 ? [512, 10_000, 0] : selector == 7 ? [8_192, 10_000, 0]
        : selector == 9 ? [0, 0, 0] : [1_024, 10_000, 0]
      let originals = limits.map { Data(repeating: 7, count: $0 == 0 ? 0 : 1) }
      let amount = KagemushaWalletUInt128V1(low: selector == 8 ? UInt64.max : 0, high: selector == 8 ? UInt64.max : 0)
      let input = try KagemushaWalletOperationInputV1(requestId: identity, selector: selector, amount: amount, first: originals[0], second: originals[1], third: originals[2])
      input.withRequest { value in
        XCTAssertEqual(value.pointee.selector, selector)
        XCTAssertEqual(value.pointee.amount.low, amount.low)
        XCTAssertEqual(value.pointee.amount.high, amount.high)
        XCTAssertEqual(Data(bytes: value.pointee.request_id!, count: 32), identity)
        XCTAssertEqual(value.pointee.first_length, originals[0].count)
        XCTAssertEqual(value.pointee.second_length, originals[1].count)
        XCTAssertEqual(value.pointee.third_length, originals[2].count)
      }
      for index in 0..<3 {
        var changed = originals
        changed[index] = Data(repeating: 0, count: limits[index] + 1)
        XCTAssertThrowsError(try KagemushaWalletOperationInputV1(requestId: identity, selector: selector, amount: amount, first: changed[0], second: changed[1], third: changed[2]))
      }
    }
    XCTAssertThrowsError(try KagemushaWalletOperationInputV1(requestId: Data(repeating: 0, count: 32), selector: 9))
    XCTAssertThrowsError(try KagemushaWalletOperationInputV1(requestId: identity, selector: 10))
    XCTAssertThrowsError(try KagemushaWalletOperationInputV1(requestId: identity, selector: 8))
    XCTAssertThrowsError(try KagemushaWalletOperationInputV1(requestId: identity, selector: 8, amount: .init(low: 1, high: 0), first: Data([1])))
    XCTAssertNoThrow(try KagemushaWalletOperationInputV1(requestId: identity, selector: 8, amount: .init(low: 1, high: 0)))
  }

}
