import XCTest
@testable import IrohaSwift

final class NumericV1Tests: XCTestCase {
    func testExactValuesCanonicalizeWithoutHostFloatingPoint() throws {
        XCTAssertEqual(try KotodamaInt("-129").canonicalString, "-129")
        XCTAssertEqual(try KotodamaDecimal("1.2300").canonicalString, "1.23")
        XCTAssertEqual(try KotodamaDecimal("0.000").canonicalString, "0")
        XCTAssertEqual(try KotodamaQuantity("12.50").canonicalString, "12.5")
        assertCode(.negativeQuantity) { _ = try KotodamaQuantity("-0.1") }
        assertCode(.invalidScale) {
            _ = try KotodamaDecimal("1.00000000000000000000000000000")
        }
        assertCode(.invalidText) { _ = try KotodamaInt("01") }
    }

    func testCanonicalFramesAndEnvelopesRoundtrip() throws {
        let integer = try KotodamaInt("-129")
        XCTAssertEqual(
            try KotodamaNumericV1Codec.decodeIntFrame(
                KotodamaNumericV1Codec.encodeIntFrame(integer)
            ),
            integer
        )
        XCTAssertEqual(
            try KotodamaNumericV1Codec.decodeIntEnvelope(
                KotodamaNumericV1Codec.encodeIntEnvelope(integer)
            ),
            integer
        )

        let decimal = try KotodamaDecimal("-1.25")
        XCTAssertEqual(
            try KotodamaNumericV1Codec.decodeDecimalEnvelope(
                KotodamaNumericV1Codec.encodeDecimalEnvelope(decimal)
            ),
            decimal
        )

        let quantity = try KotodamaQuantity("1.25")
        XCTAssertEqual(
            try KotodamaNumericV1Codec.decodeQuantityEnvelope(
                KotodamaNumericV1Codec.encodeQuantityEnvelope(quantity)
            ),
            quantity
        )

        assertCode(.wrongType) {
            _ = try KotodamaNumericV1Codec.decodeDecimalEnvelope(
                KotodamaNumericV1Codec.encodeIntEnvelope(try KotodamaInt("1"))
            )
        }
    }

    func testSigned4096BitEndpointsRoundtrip() throws {
        let minimum = "-" + decimalPowerOfTwo(4095)
        let maximum = decimalSubtractOne(decimalPowerOfTwo(4095))
        for text in [minimum, maximum] {
            let value = try KotodamaInt(text)
            let frame = try KotodamaNumericV1Codec.encodeIntFrame(value)
            XCTAssertEqual(frame.count, 556)
            XCTAssertEqual(try KotodamaNumericV1Codec.decodeIntFrame(frame), value)
        }
        assertCode(.mantissaOverflow) { _ = try KotodamaInt(decimalPowerOfTwo(4095)) }
        assertCode(.mantissaOverflow) { _ = try KotodamaInt("-" + decimalAddOne(decimalPowerOfTwo(4095))) }
    }

    func testMalformedAuthenticatedInputsAreRejected() throws {
        let frame = try KotodamaNumericV1Codec.encodeIntFrame(KotodamaInt("128"))
        for length in 0..<frame.count {
            XCTAssertThrowsError(try KotodamaNumericV1Codec.decodeIntFrame(Data(frame.prefix(length))))
        }

        var badChecksum = frame
        badChecksum[badChecksum.count - 1] ^= 1
        assertCode(.checksumMismatch) {
            _ = try KotodamaNumericV1Codec.decodeIntFrame(badChecksum)
        }

        var badHash = try KotodamaNumericV1Codec.encodeIntEnvelope(KotodamaInt("1"))
        badHash[badHash.count - 1] ^= 1
        assertCode(.payloadHashMismatch) {
            _ = try KotodamaNumericV1Codec.decodeIntEnvelope(badHash)
        }

        var retired = try KotodamaNumericV1Codec.encodeIntEnvelope(KotodamaInt("1"))
        retired[0] = 0
        retired[1] = 0x10
        assertCode(.typeNotAllowed) {
            _ = try KotodamaNumericV1Codec.decodeIntEnvelope(retired)
        }
    }

    private func assertCode(
        _ expected: KotodamaNumericV1ErrorCode,
        file: StaticString = #filePath,
        line: UInt = #line,
        _ body: () throws -> Void
    ) {
        XCTAssertThrowsError(try body(), file: file, line: line) { error in
            XCTAssertEqual((error as? KotodamaNumericV1Error)?.code, expected, file: file, line: line)
        }
    }

    private func decimalPowerOfTwo(_ exponent: Int) -> String {
        var digits = [1]
        for _ in 0..<exponent {
            var carry = 0
            for index in digits.indices.reversed() {
                let value = digits[index] * 2 + carry
                digits[index] = value % 10
                carry = value / 10
            }
            if carry != 0 { digits.insert(carry, at: 0) }
        }
        return digits.map(String.init).joined()
    }

    private func decimalSubtractOne(_ value: String) -> String {
        var digits = value.compactMap(\.wholeNumberValue)
        var index = digits.count - 1
        while digits[index] == 0 {
            digits[index] = 9
            index -= 1
        }
        digits[index] -= 1
        return digits.map(String.init).joined()
    }

    private func decimalAddOne(_ value: String) -> String {
        var digits = value.compactMap(\.wholeNumberValue)
        var index = digits.count - 1
        while index >= 0 && digits[index] == 9 {
            digits[index] = 0
            index -= 1
        }
        if index < 0 { digits.insert(1, at: 0) } else { digits[index] += 1 }
        return digits.map(String.init).joined()
    }
}
