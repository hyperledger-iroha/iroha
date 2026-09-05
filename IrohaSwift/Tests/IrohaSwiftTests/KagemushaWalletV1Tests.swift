import XCTest
@testable import IrohaSwift

final class KagemushaWalletV1Tests: XCTestCase {
  func testWalletReservationRejectsProviderIdentitySubstitution() throws {
    let operationID = Data(repeating: 10, count: 32)
    XCTAssertEqual(try kagemushaReserveOperationIDV1(operationID) { $0 }, operationID)
    XCTAssertThrowsError(
      try kagemushaReserveOperationIDV1(operationID) { _ in Data(repeating: 11, count: 32) })
    var called = false
    XCTAssertThrowsError(
      try kagemushaReserveOperationIDV1(Data(repeating: 0, count: 32)) { value in
        called = true
        return value
      })
    XCTAssertFalse(called)
  }

  func testV1MonetaryOperationsAreTheSixAggregateBalanceTransitions() {
    XCTAssertEqual(
      KagemushaOperationKindV1.allCases.map(\.rawValue),
      [0, 1, 2, 3, 4, 5]
    )
    XCTAssertEqual(KagemushaOperationKindV1.receiveFold.rawValue, 3)
    XCTAssertEqual(KagemushaOperationKindV1.rotate.rawValue, 5)
  }
}
