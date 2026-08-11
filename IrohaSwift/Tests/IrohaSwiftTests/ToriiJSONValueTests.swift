import Foundation
import XCTest
@testable import IrohaSwift

final class ToriiJSONValueTests: XCTestCase {
    func testCanonicalNoritoMetadataUsesOneLexicalForm() throws {
        XCTAssertEqual(try CanonicalNorito.jsonString(from: .number(-0.0)), "-0.0")
        XCTAssertEqual(
            try CanonicalNorito.jsonString(
                from: .object(["\u{10000}": .number(1), "\u{e000}": .number(2)])
            ),
            "{\"\u{e000}\":2,\"\u{10000}\":1}"
        )
        XCTAssertEqual(
            try CanonicalNorito.jsonString(from: .string("\u{08}\u{0c}\u{1f}")),
            "\"\\b\\f\\u001f\""
        )
    }

    func testNormalizedStringRejectsOutOfRangeInteger() {
        let tooLarge = Double(Int.max)
        let value = ToriiJSONValue.number(tooLarge)
        XCTAssertNil(value.normalizedString)
    }

    func testNormalizedStringAcceptsLargestRepresentableInRangeInteger() {
        let value = Double(Int.max).nextDown
        XCTAssertEqual(ToriiJSONValue.number(value).normalizedString, String(Int(value)))
    }

    func testNormalizedUInt64RejectsFractionalNumber() {
        let value = ToriiJSONValue.number(12.5)
        XCTAssertNil(value.normalizedUInt64)
    }

    func testNormalizedUInt64RejectsRoundedUpperBoundary() {
        let value = ToriiJSONValue.number(Double(UInt64.max))
        XCTAssertNil(value.normalizedUInt64)
    }

    func testNormalizedUInt64AcceptsLargestRepresentableInRangeInteger() {
        let value = Double(UInt64.max).nextDown
        XCTAssertEqual(ToriiJSONValue.number(value).normalizedUInt64, UInt64(value))
    }

    func testNormalizedInt64RejectsFractionalNumber() {
        let value = ToriiJSONValue.number(-3.75)
        XCTAssertNil(value.normalizedInt64)
    }

    func testNormalizedInt64RejectsRoundedUpperBoundary() {
        let value = ToriiJSONValue.number(Double(Int64.max))
        XCTAssertNil(value.normalizedInt64)
    }

}
