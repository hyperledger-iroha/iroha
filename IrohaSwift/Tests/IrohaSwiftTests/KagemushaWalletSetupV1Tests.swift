import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaWalletSetupV1Tests: XCTestCase {
  func testActivationTransportHasItsOwnBoundAndNoForeignInputs() throws {
    XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: 15))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 15, first: Data([1])))
    XCTAssertNoThrow(try KagemushaWalletCallV1(status: 17, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data(repeating: 1, count: 16_384)))
    for count in [0, 16_385] { XCTAssertThrowsError(try KagemushaWalletCallV1(status: 17, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data(repeating: 1, count: count))) }
    XCTAssertThrowsError(try KagemushaWalletCallV1(status: 17, sequenceLow: 1, sequenceHigh: 0, detail: 0, bytes: Data([1])))
    XCTAssertThrowsError(try KagemushaWalletCallV1(status: 1, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data(repeating: 1, count: 10_001)))
  }
  func testTypedSetupHasExactUnusedFieldsAndUnsignedAmount() throws {
    let id = Data(repeating: 7, count: 32)
    let offer = try KagemushaWalletSetupInputV1(selector: 1, identity: id, amount: .init(low: .max, high: .max))
    offer.withRequest {
      XCTAssertEqual($0.pointee.selector, 1)
      XCTAssertEqual($0.pointee.amount.high, .max)
      XCTAssertEqual($0.pointee.amount.low, .max)
      XCTAssertEqual($0.pointee.token, 0)
      XCTAssertEqual($0.pointee.first_length, 0)
    }
    XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: 0))
    XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: 4))
    XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: 6, token: 1))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 6))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 6, token: 1, first: Data([1])))
    XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: 2, identity: id, first: Data([1])))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 2, identity: id, first: Data([1]), second: Data([2])))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 2, identity: id, first: Data([1]), second: Data([2]), third: Data(repeating: 1, count: 513)))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 4, identity: id))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 5, token: 0, first: Data([1]), second: Data([1])))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 1, identity: id))
    for selector in UInt32(7)...14 {
      XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: selector, first: Data([1])))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, first: Data(repeating: 1, count: 10_001)))
    }
  }
  func testSetupResultsKeepTimeChallengeSeparateFromCompletion() throws {
    let challenge = try KagemushaWalletCallV1(status: 13, sequenceLow: 7, sequenceHigh: 0, detail: 0, bytes: Data(repeating: 1, count: 32))
    XCTAssertEqual(challenge.status, 13)
    XCTAssertNoThrow(try KagemushaWalletCallV1(status: 12, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data([1])))
    XCTAssertNoThrow(try KagemushaWalletCallV1(status: 14, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data()))
    for count in [0, 31, 33] {
      XCTAssertThrowsError(try KagemushaWalletCallV1(status: 13, sequenceLow: 7, sequenceHigh: 0, detail: 0, bytes: Data(repeating: 1, count: count)))
    }
    XCTAssertThrowsError(try KagemushaWalletCallV1(status: 13, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data(repeating: 1, count: 32)))
  }
  func testValidSetupAndOpenResultsCannotBecomeMonetaryCompletion() throws {
    for status in Int32(12)...17 {
      let sequence: UInt64 = [13, 15, 16].contains(status) ? 1 : 0
      let bytes = [14, 16].contains(status) ? Data()
        : Data(repeating: 1, count: [13, 15].contains(status) ? 32 : 1)
      let result = try KagemushaWalletCallV1(status: status, sequenceLow: sequence,
        sequenceHigh: 0, detail: 0, bytes: bytes)
      XCTAssertThrowsError(try result.completion()) { error in
        XCTAssertEqual(error as? KagemushaWalletErrorV1, .invalidNativeOutput)
      }
    }
  }
}
