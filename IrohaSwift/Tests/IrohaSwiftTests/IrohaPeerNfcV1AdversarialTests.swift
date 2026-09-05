import XCTest
@testable import IrohaSwift

final class IrohaPeerNfcV1AdversarialTests: XCTestCase {
  func testUnknownPublicMessageTagIsRejected() {
    XCTAssertNil(IrohaPeerWireKindV1(rawValue: 0))
    XCTAssertNil(IrohaPeerWireKindV1(rawValue: 4))
    XCTAssertNil(IrohaPeerWireKindV1(rawValue: 5))
  }
}
