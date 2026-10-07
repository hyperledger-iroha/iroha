import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaWalletSetupV1Tests: XCTestCase {
  func testFeeAndLedgerBoundariesKeepOriginalsSeparateFromAcknowledgement() throws {
    let id = Data(repeating: 7, count: 32)
    XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: 20, identity: id))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 20))
    for selector in UInt32(21)...23 {
      XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: selector, first: Data([1])))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, identity: id, first: Data([1])))
    }
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 21, first: Data(repeating: 1, count: 21_025)))
    XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: 24))
    XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: 25, identity: id, first: Data([1]), second: Data([2])))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 25, identity: id, first: Data([1])))
    for status in Int32(31)...35 {
      let bytes = status == 31 ? Data(repeating: 1, count: 21_024) : status == 33 ? id : Data()
      let result = try KagemushaWalletCallV1(status: status, sequenceLow: status == 33 ? .max : 0, sequenceHigh: 0, detail: 0, bytes: bytes)
      XCTAssertThrowsError(try result.completion())
      if status == 33 { XCTAssertEqual(try KagemushaWalletLedgerProgressV1(result).height, UInt64.max) }
    }
    XCTAssertThrowsError(try KagemushaWalletCallV1(status: 31, sequenceLow: 1, sequenceHigh: 0, detail: 0, bytes: Data([1])))
    XCTAssertThrowsError(try KagemushaWalletCallV1(status: 33, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: id))
    XCTAssertThrowsError(try KagemushaWalletCallV1(status: 35, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: id))
    let claim = KagemushaWalletFeeClaimV1(payment: id, request: Data([2]))
    XCTAssertEqual(claim.payment, id); XCTAssertEqual(claim.request, Data([2]))
  }

  func testCreditedProjectionSelectsOnlyDurableReceiveOrStatusBytes() throws {
    for (status, selector) in [(Int32(1), UInt32(16)), (10, 17)] {
      let result = try KagemushaWalletCallV1(status: status, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data([0, 255, 1]))
      let input = try result.creditedInput()
      XCTAssertEqual(input.selector, selector)
      XCTAssertEqual(input.first, result.bytes)
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, first: Data(repeating: 1, count: 10_001)))
      XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: selector, token: 1, first: result.bytes))
    }
    for status in [Int32(0), 2, 3, 4, 5, 6, 7, 8, 9, 11, 12] {
      let call = try KagemushaWalletCallV1(status: status, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: status == 12 ? Data([1]) : Data())
      XCTAssertThrowsError(try call.creditedInput())
    }
  }
  func testBackgroundStatusIsSeparateFromCompletionAndPreservesUnsignedBacklog() throws {
    XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: 18))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 18, first: Data([1])))
    for phase in UInt32(0)...2 {
      let call = try KagemushaWalletCallV1(status: 29, sequenceLow: .max, sequenceHigh: 2, detail: phase | 12, bytes: Data())
      let status = try KagemushaWalletBackgroundStatusV1(call)
      XCTAssertEqual(status.phase.rawValue, phase)
      XCTAssertTrue(status.eligible)
      XCTAssertEqual(status.observedBacklog, .init(low: .max, high: 2))
      XCTAssertThrowsError(try call.completion())
    }
    let unobserved = try KagemushaWalletCallV1(status: 29, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data())
    XCTAssertNil(try KagemushaWalletBackgroundStatusV1(unobserved).observedBacklog)
    for detail in [UInt32(3), 16] {
      let bad = try KagemushaWalletCallV1(status: 29, sequenceLow: 0, sequenceHigh: 0, detail: detail, bytes: Data())
      XCTAssertThrowsError(try KagemushaWalletBackgroundStatusV1(bad))
    }
    let unknown = try KagemushaWalletCallV1(status: 29, sequenceLow: 1, sequenceHigh: 0, detail: 0, bytes: Data())
    XCTAssertThrowsError(try KagemushaWalletBackgroundStatusV1(unknown))
  }
  func testCloseLoadsTransportHasNoForeignBodyAndSeparateResult() throws {
    XCTAssertNoThrow(try KagemushaWalletSetupInputV1(selector: 19, identity: Data(repeating: 1, count: 32)))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 19, identity: Data(repeating: 1, count: 32), first: Data([1])))
    XCTAssertThrowsError(try KagemushaWalletSetupInputV1(selector: 19))
    let valid = try KagemushaWalletCallV1(status: 30, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data(repeating: 1, count: 16_384))
    XCTAssertThrowsError(try valid.completion())
    for count in [0, 16_385] { XCTAssertThrowsError(try KagemushaWalletCallV1(status: 30, sequenceLow: 0, sequenceHigh: 0, detail: 0, bytes: Data(repeating: 1, count: count))) }
    XCTAssertThrowsError(try KagemushaWalletCallV1(status: 30, sequenceLow: 1, sequenceHigh: 0, detail: 0, bytes: Data([1])))
  }
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
