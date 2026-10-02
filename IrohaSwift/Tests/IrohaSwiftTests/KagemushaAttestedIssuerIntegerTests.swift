import Foundation
import XCTest
@testable import IrohaSwift

final class KagemushaAttestedIssuerIntegerTests: XCTestCase {
    func testFractionsAndNonFiniteNumbersCannotBecomeIssuerIntegers() {
        for value in [2.5, -0.5, Double.nan, .infinity, -.infinity] {
            assertRejected(NSNumber(value: value))
        }
    }

    func testUnsignedBoundsPreserveIntegerPrecision() throws {
        for value in [UInt64(0), 2, UInt64(Int64.max), UInt64(Int64.max) + 1, .max] {
            XCTAssertEqual(
                try KagemushaAttestedIssuerClient.integer(["value": NSNumber(value: value)], "value"),
                value
            )
        }
        assertRejected(NSNumber(value: -1))
        assertRejected(NSNumber(value: Int64.min))
        assertRejected(NSNumber(value: Double(UInt64.max)))
        XCTAssertEqual(
            try KagemushaAttestedIssuerClient.integer(["value": NSNumber(value: 2.0)], "value"),
            2
        )
    }

    func testOnlyNumericJSONValuesAreAccepted() {
        for value: Any in [true, false, "2", NSNull()] {
            assertRejected(value)
        }
        XCTAssertThrowsError(try KagemushaAttestedIssuerClient.integer([:], "value"))
    }

    func testParsedJSONPreservesFullIntegersAndRejectsFractionsAndOverflow() throws {
        for (text, expected) in [
            ("0", UInt64(0)),
            ("9007199254740993", 9_007_199_254_740_993),
            ("18446744073709551615", UInt64.max),
        ] {
            let object = try XCTUnwrap(
                JSONSerialization.jsonObject(with: Data("{\"value\":\(text)}".utf8)) as? [String: Any]
            )
            XCTAssertEqual(try KagemushaAttestedIssuerClient.integer(object, "value"), expected)
        }
        for text in ["2.5", "-0.5", "-1", "true", "18446744073709551616"] {
            let object = try XCTUnwrap(
                JSONSerialization.jsonObject(with: Data("{\"value\":\(text)}".utf8)) as? [String: Any]
            )
            XCTAssertThrowsError(try KagemushaAttestedIssuerClient.integer(object, "value"))
        }
    }

    private func assertRejected(_ value: Any, file: StaticString = #filePath, line: UInt = #line) {
        XCTAssertThrowsError(
            try KagemushaAttestedIssuerClient.integer(["value": value], "value"),
            file: file,
            line: line
        ) { error in
            guard case KagemushaError.invalidIssuerResponse("value") = error else {
                return XCTFail("Unexpected error: \(error)", file: file, line: line)
            }
        }
    }
}
