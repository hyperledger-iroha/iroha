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
        let value = try ToriiJSONValue.decodeExact(from: data)
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

    func testSccpUInt128CanonicalParserEnforcesTheFullWireRange() {
        let maximumSafeJSONInteger = "9007199254740991"
        let firstExtendedJSONInteger = "9007199254740992"
        let maximumUInt64 = "18446744073709551615"
        let maximum = "340282366920938463463374607431768211455"
        XCTAssertEqual(SccpUInt128.parse("0")?.decimalString, "0")
        XCTAssertEqual(
            SccpUInt128.parse(maximumSafeJSONInteger)?.decimalString,
            maximumSafeJSONInteger
        )
        XCTAssertEqual(
            SccpUInt128.parse(firstExtendedJSONInteger)?.decimalString,
            firstExtendedJSONInteger
        )
        XCTAssertEqual(SccpUInt128.parse(maximumUInt64)?.decimalString, maximumUInt64)
        XCTAssertEqual(SccpUInt128.parse(maximum)?.decimalString, maximum)
        XCTAssertNil(SccpUInt128.parse(""))
        XCTAssertNil(SccpUInt128.parse("00"))
        XCTAssertNil(SccpUInt128.parse("01"))
        XCTAssertNil(SccpUInt128.parse("-1"))
        XCTAssertNil(SccpUInt128.parse("+1"))
        XCTAssertNil(SccpUInt128.parse("1.0"))
        XCTAssertNil(SccpUInt128.parse("1e0"))
        XCTAssertNil(SccpUInt128.parse("١"))
        XCTAssertNil(SccpUInt128.parse("340282366920938463463374607431768211456"))
    }

    func testExactDecoderClassifiesUnsignedIntegerBoundaries() throws {
        let maximum = "340282366920938463463374607431768211455"
        let data = Data(
            "[0,9007199254740991,9007199254740992,18446744073709551615,\(maximum)]".utf8
        )
        let value = try ToriiJSONValue.decodeExact(from: data)
        guard case .array(let values) = value else {
            return XCTFail("boundary payload must decode as an array")
        }
        XCTAssertEqual(values.count, 5)
        XCTAssertEqual(values[0], .number(0))
        XCTAssertEqual(values[1], .number(9_007_199_254_740_991))
        XCTAssertEqual(values[2], .integer("9007199254740992"))
        XCTAssertEqual(values[3], .integer("18446744073709551615"))
        XCTAssertEqual(values[4], .integer(maximum))
    }

    func testExactDecoderPreservesNestedWideIntegerLexemes() throws {
        let maximum = "340282366920938463463374607431768211455"
        let data = Data(
            "{\"outer\":[{\"amount\":\(maximum)},{\"amount\":18446744073709551615}]}".utf8
        )
        let value = try ToriiJSONValue.decodeExact(from: data)
        guard case .array(let outer)? = value["outer"], outer.count == 2 else {
            return XCTFail("nested payload must contain two entries")
        }
        XCTAssertEqual(outer[0]["amount"], .integer(maximum))
        XCTAssertEqual(outer[1]["amount"], .integer("18446744073709551615"))
    }

    func testExactEncoderEmitsTheFullUnsignedRange() throws {
        let maximumUInt64 = "18446744073709551615"
        let maximumUInt128 = "340282366920938463463374607431768211455"
        let value = ToriiJSONValue.array([
            .integer("0"),
            .integer(maximumUInt64),
            .integer(maximumUInt128),
        ])
        XCTAssertEqual(
            String(decoding: try value.encodedData(), as: UTF8.self),
            "[0,\(maximumUInt64),\(maximumUInt128)]"
        )
        let pretty = try value.encodedData(prettyPrinted: true)
        XCTAssertEqual(
            try ToriiJSONValue.decodeExact(from: pretty),
            .array([.number(0), .integer(maximumUInt64), .integer(maximumUInt128)])
        )
    }

    func testUInt128OverflowAndUnavailableRawLexemesFailClosed() throws {
        let overflow = Data("340282366920938463463374607431768211456".utf8)
        XCTAssertThrowsError(try ToriiJSONValue.decodeExact(from: overflow))
        XCTAssertThrowsError(try ToriiJSONValue.decodeExact(from: Data("01".utf8)))

        XCTAssertEqual(
            try JSONDecoder().decode(
                ToriiJSONValue.self,
                from: Data("18446744073709551615".utf8)
            ),
            .integer("18446744073709551615")
        )
        let maximum = Data("340282366920938463463374607431768211455".utf8)
        XCTAssertThrowsError(try JSONDecoder().decode(ToriiJSONValue.self, from: maximum))
        XCTAssertThrowsError(
            try JSONEncoder().encode(
                ToriiJSONValue.integer("340282366920938463463374607431768211455")
            )
        )
    }

    func testExactDecoderRejectsDuplicateKeysAndWideSignedIntegers() throws {
        let maximum = "340282366920938463463374607431768211455"
        let duplicate = Data("{\"amount\":\(maximum),\"amount\":0}".utf8)
        XCTAssertThrowsError(try ToriiJSONValue.decodeExact(from: duplicate))
        XCTAssertEqual(
            try ToriiJSONValue.decodeExact(from: Data("-1".utf8)),
            .number(-1)
        )
        XCTAssertThrowsError(
            try ToriiJSONValue.decodeExact(from: Data("-9007199254740992".utf8))
        )
    }

    func testGovernanceLargeIntegersAreLimitedToSccpCaps() throws {
        let exact = "1000000000000000000000"
        let data = Data("{\"max_wrapped_supply\":\(exact)}".utf8)
        let exactIntegerLexemes = try governanceExactJSONIntegerLexemes(data)
        let decoder = JSONDecoder()
        decoder.userInfo[governanceExactIntegerLexemesUserInfoKey] = exactIntegerLexemes
        let proposal = try decoder.decode(ToriiJSONValue.self, from: data)
        XCTAssertNoThrow(
            try governanceRequireExactJSONIntegers(
                proposal,
                codingPath: [],
                context: "proposal",
                exactIntegerLexemes: exactIntegerLexemes
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
