import XCTest
@testable import IrohaSwift

final class KagemushaWalletV1Tests: XCTestCase {
  func testV1MonetaryOperationsAreTheSixAggregateBalanceTransitions() {
    XCTAssertEqual(
      KagemushaOperationKindV1.allCases.map(\.rawValue),
      [0, 1, 2, 3, 4, 5]
    )
    XCTAssertEqual(KagemushaOperationKindV1.receiveFold.rawValue, 3)
    XCTAssertEqual(KagemushaOperationKindV1.rotate.rawValue, 5)
  }
}
