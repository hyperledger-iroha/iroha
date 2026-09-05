import XCTest
@testable import IrohaSwiftMobileTransports

final class IrohaPeerNfcProgressPublicationPolicyV1Tests: XCTestCase {
  func testProgressStagesDescribeTheThreeMessageExchange() {
    XCTAssertEqual(
      IrohaPeerNfcProgressStageV1.allCases,
      [
        .sessionActive,
        .tagDetected,
        .requestRead,
        .ownerAuthRequested,
        .ownerAuthSucceeded,
        .paymentPrepared,
        .paymentStaged,
        .acknowledgementReceived,
        .acknowledgementPersisted,
        .complete,
      ]
    )
  }
}
