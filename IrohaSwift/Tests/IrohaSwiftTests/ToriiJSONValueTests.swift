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

    func testUInt128JSONIntegerRoundTripsWithoutDoubleRounding() throws {
        let maximum = "340282366920938463463374607431768211455"
        let data = Data("{\"maximum\":\(maximum)}".utf8)
        let value = try JSONDecoder().decode(ToriiJSONValue.self, from: data)
        XCTAssertEqual(value["maximum"], .integer(maximum))
        XCTAssertEqual(
            String(decoding: try value.encodedData(), as: UTF8.self),
            "{\"maximum\":\(maximum)}"
        )
        XCTAssertEqual(
            try CanonicalNorito.jsonString(from: value),
            "{\"maximum\":\(maximum)}"
        )
    }

    func testUInt128JSONIntegerRejectsNoncanonicalManualValue() {
        XCTAssertThrowsError(try ToriiJSONValue.integer("01").encodedData())
        XCTAssertThrowsError(
            try ToriiJSONValue.integer("340282366920938463463374607431768211456")
                .encodedData()
        )
        XCTAssertThrowsError(
            try CanonicalNorito.jsonString(from: .integer("340282366920938463463374607431768211456"))
        )
    }

    func testUInt128OverflowNeverBecomesAnExactIntegerValue() throws {
        let overflow = Data("340282366920938463463374607431768211456".utf8)
        let decoded = try JSONDecoder().decode(ToriiJSONValue.self, from: overflow)
        if case .integer = decoded {
            XCTFail("UInt128.max + 1 must not be represented as an exact integer")
        }
        XCTAssertThrowsError(
            try governanceRequireExactJSONIntegers(
                .object(["max_wrapped_supply": decoded]),
                codingPath: [],
                context: "proposal"
            )
        )
    }

    func testGovernanceLargeIntegersAreLimitedToSccpCaps() throws {
        let exact = "1000000000000000000000"
        XCTAssertNoThrow(
            try governanceRequireExactJSONIntegers(
                .object(["max_wrapped_supply": .integer(exact)]),
                codingPath: [],
                context: "proposal"
            )
        )
        XCTAssertThrowsError(
            try governanceRequireExactJSONIntegers(
                .object(["checkpoint_height": .integer(exact)]),
                codingPath: [],
                context: "proposal"
            )
        )
    }

}
