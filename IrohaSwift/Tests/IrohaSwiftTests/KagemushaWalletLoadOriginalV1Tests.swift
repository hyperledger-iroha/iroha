import Foundation
import XCTest
@testable import IrohaSwift

/// Owned input DATA tests only. No Native wallet, proof or verified receipt is substituted.
final class KagemushaWalletLoadOriginalV1Tests: XCTestCase {
  private func selection() throws -> ToriiKagemushaWalletLoadSelectionV1 {
    try .init(schemeID: Data(repeating: 1, count: 32), walletID: Data(repeating: 2, count: 32), requestID: Data(repeating: 3, count: 32))
  }
  func testOwnedFramesSurviveCallerAndAccessorMutations() throws {
    var receipt = Data([7, 8]), finality = Data([9, 10])
    let input = try KagemushaWalletLoadOriginalInputV1(selection: selection(), payer: "bounded-I105-input", receipt: receipt, finality: finality)
    receipt[0] = 0; finality[0] = 0
    XCTAssertEqual(input.receipt, Data([7, 8])); XCTAssertEqual(input.finality, Data([9, 10]))
    var returned = input.receipt; returned[0] = 0
    XCTAssertEqual(input.receipt, Data([7, 8]))
  }
  func testAbsentOrOversizedOriginalIsRejectedBeforeNative() throws {
    for bad in [Data(), Data(repeating: 0, count: 513)] {
      XCTAssertThrowsError(try KagemushaWalletLoadOriginalInputV1(selection: selection(), payer: "x", receipt: bad, finality: Data([1])))
    }
    for bad in [Data(), Data(repeating: 0, count: 16385)] {
      XCTAssertThrowsError(try KagemushaWalletLoadOriginalInputV1(selection: selection(), payer: "x", receipt: Data([1]), finality: bad))
    }
  }
  func testAccountHeaderTextHasFiniteAsciiByteBound() throws {
    for bad in ["", String(repeating: "x", count: 1025), " x", "x\n", "\0", "é"] {
      XCTAssertThrowsError(try KagemushaWalletLoadOriginalInputV1(selection: selection(), payer: bad, receipt: Data([1]), finality: Data([1])))
    }
    XCTAssertEqual(try KagemushaWalletLoadOriginalInputV1(selection: selection(), payer: String(repeating: "x", count: 1024), receipt: Data([1]), finality: Data([1])).payer.count, 1024)
  }
  func testIndependentRouteSelectorsAndMaximumFramesRemainExactData() throws {
    let input = try KagemushaWalletLoadOriginalInputV1(selection: selection(), payer: "x", receipt: Data(repeating: 7, count: 512), finality: Data(repeating: 8, count: 16384))
    XCTAssertEqual(input.schemeID, Data(repeating: 1, count: 32)); XCTAssertEqual(input.walletID, Data(repeating: 2, count: 32))
    XCTAssertEqual(input.requestID, Data(repeating: 3, count: 32)); XCTAssertEqual(input.receipt.count, 512); XCTAssertEqual(input.finality.count, 16384)
  }
}
