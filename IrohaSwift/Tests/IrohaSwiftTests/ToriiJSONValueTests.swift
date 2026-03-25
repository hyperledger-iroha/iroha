import Foundation
import XCTest
@testable import IrohaSwift

final class ToriiJSONValueTests: XCTestCase {
    func testNormalizedStringRejectsOutOfRangeInteger() {
        let tooLarge = Double(Int.max) * 2
        let value = ToriiJSONValue.number(tooLarge)
        XCTAssertNil(value.normalizedString)
    }

    func testNormalizedUInt64RejectsFractionalNumber() {
        let value = ToriiJSONValue.number(12.5)
        XCTAssertNil(value.normalizedUInt64)
    }

    func testNormalizedInt64RejectsFractionalNumber() {
        let value = ToriiJSONValue.number(-3.75)
        XCTAssertNil(value.normalizedInt64)
    }

}
