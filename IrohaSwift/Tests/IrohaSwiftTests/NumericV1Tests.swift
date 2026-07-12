import XCTest
@testable import IrohaSwift

final class NumericV1Tests: XCTestCase {
    func testExactValuesCanonicalizeWithoutHostFloatingPoint() throws {
        XCTAssertEqual(try KotodamaInt("-129").canonicalString, "-129")
        XCTAssertEqual(try KotodamaDecimal("1.2300").canonicalString, "1.23")
        XCTAssertEqual(try KotodamaDecimal("0.000").canonicalString, "0")
        XCTAssertEqual(try KotodamaQuantity("12.50").canonicalString, "12.5")
        assertCode(.negativeQuantity) { _ = try KotodamaQuantity("-0.1") }
        assertCode(.mantissaOverflow) {
            _ = try KotodamaQuantity("-" + String(repeating: "9", count: 154))
        }
        XCTAssertEqual(try KotodamaDecimal("1.00000000000000000000000000000").canonicalString, "1")
        assertCode(.invalidScale) { _ = try KotodamaDecimal("0.00000000000000000000000000001") }
        assertCode(.invalidText) { _ = try KotodamaInt("01") }
        XCTAssertEqual(try KotodamaNumericV1Codec.decodeDecimalJSON("1.23").canonicalString, "1.23")
        XCTAssertEqual(try KotodamaNumericV1Codec.decodeQuantityJSON("0").canonicalString, "0")
        for alternate in ["+1", "01", "1.", ".5", "1e0", "-0", "-0.0", "1.0", "1.2300", "0.0"] {
            assertCode(.invalidText) { _ = try KotodamaNumericV1Codec.decodeDecimalJSON(alternate) }
        }
        for alternate in ["+1", "01", "-0", "1.0", "1e0"] {
            assertCode(.invalidText) { _ = try KotodamaNumericV1Codec.decodeIntJSON(alternate) }
        }
        assertCode(.invalidText) { _ = try KotodamaNumericV1Codec.decodeQuantityJSON("1.0") }
        assertCode(.negativeQuantity) { _ = try KotodamaNumericV1Codec.decodeQuantityJSON("-1") }
    }

    func testCanonicalFramesAndEnvelopesRoundtrip() throws {
        let integer = try KotodamaInt("-129")
        let integerEnvelope = try KotodamaNumericV1Codec.encodeIntEnvelope(integer)
        XCTAssertEqual(Array(integerEnvelope.prefix(2)), [0x00, 0x11])
        XCTAssertEqual(
            try KotodamaNumericV1Codec.decodeIntFrame(
                KotodamaNumericV1Codec.encodeIntFrame(integer)
            ),
            integer
        )
        XCTAssertEqual(
            try KotodamaNumericV1Codec.decodeIntEnvelope(
                integerEnvelope
            ),
            integer
        )

        let decimal = try KotodamaDecimal("-1.25")
        let decimalEnvelope = try KotodamaNumericV1Codec.encodeDecimalEnvelope(decimal)
        XCTAssertEqual(Array(decimalEnvelope.prefix(2)), [0x00, 0x12])
        XCTAssertEqual(
            try KotodamaNumericV1Codec.decodeDecimalEnvelope(
                decimalEnvelope
            ),
            decimal
        )

        let quantity = try KotodamaQuantity("1.25")
        let quantityEnvelope = try KotodamaNumericV1Codec.encodeQuantityEnvelope(quantity)
        XCTAssertEqual(Array(quantityEnvelope.prefix(2)), [0x00, 0x13])
        XCTAssertEqual(
            try KotodamaNumericV1Codec.decodeQuantityEnvelope(
                quantityEnvelope
            ),
            quantity
        )

        assertCode(.wrongType) {
            _ = try KotodamaNumericV1Codec.decodeDecimalEnvelope(
                KotodamaNumericV1Codec.encodeIntEnvelope(try KotodamaInt("1"))
            )
        }
    }

    func testSigned512BitEndpointsRoundtrip() throws {
        let minimum = "-" + decimalPowerOfTwo(511)
        let maximum = decimalSubtractOne(decimalPowerOfTwo(511))
        for text in [minimum, maximum] {
            let value = try KotodamaInt(text)
            let frame = try KotodamaNumericV1Codec.encodeIntFrame(value)
            XCTAssertEqual(frame.count, 108)
            XCTAssertEqual(try KotodamaNumericV1Codec.decodeIntFrame(frame), value)
        }
        assertCode(.mantissaOverflow) { _ = try KotodamaInt(decimalPowerOfTwo(511)) }
        assertCode(.mantissaOverflow) { _ = try KotodamaInt("-" + decimalAddOne(decimalPowerOfTwo(511))) }
        assertCode(.mantissaOverflow) { _ = try KotodamaInt(String(repeating: "1", count: 10_000)) }
        assertCode(.invalidText) { _ = try KotodamaInt(String(repeating: "x", count: 10_000)) }
        assertCode(.mantissaOverflow) { _ = try KotodamaDecimal(String(repeating: "1", count: 10_000)) }
        XCTAssertEqual(
            try KotodamaDecimal("1." + String(repeating: "0", count: 10_000)).canonicalString,
            "1"
        )
        XCTAssertEqual(try KotodamaDecimal(maximum + ".0").canonicalString, maximum)
        assertCode(.mantissaOverflow) { _ = try KotodamaDecimal(maximum + ".1") }
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
        retired[2] = 2
        assertCode(.typeNotAllowed) {
            _ = try KotodamaNumericV1Codec.decodeIntEnvelope(retired)
        }

        var knownWrong = try KotodamaNumericV1Codec.encodeIntEnvelope(KotodamaInt("1"))
        knownWrong[0] = 0
        knownWrong[1] = 0x01
        knownWrong[2] = 2
        assertCode(.wrongType) {
            _ = try KotodamaNumericV1Codec.decodeIntEnvelope(knownWrong)
        }

        var unknown = try KotodamaNumericV1Codec.encodeIntEnvelope(KotodamaInt("1"))
        unknown[0] = 0
        unknown[1] = 0x14
        unknown[2] = 2
        assertCode(.unknownType) {
            _ = try KotodamaNumericV1Codec.decodeIntEnvelope(unknown)
        }
    }

    func testConsumesRustAuthoredSharedGoldenFixture() throws {
        let fixtureURL = repositoryRoot()
            .appendingPathComponent("fixtures/numeric_v1_golden.json")
        let object = try XCTUnwrap(
            try JSONSerialization.jsonObject(with: Data(contentsOf: fixtureURL)) as? [String: Any]
        )
        XCTAssertEqual(object["format"] as? String, "iroha.numeric.v1")
        XCTAssertEqual((object["signed_bits"] as? NSNumber)?.intValue, 512)
        XCTAssertEqual((object["maximum_scale"] as? NSNumber)?.intValue, 28)

        for vector in try XCTUnwrap(object["text"] as? [[String: Any]]) {
            let input = try XCTUnwrap(vector["input"] as? String)
            let canonical: String
            switch vector["kind"] as? String {
            case "decimal": canonical = try KotodamaDecimal(input).canonicalString
            case "quantity": canonical = try KotodamaQuantity(input).canonicalString
            default: return XCTFail("unknown text fixture kind")
            }
            XCTAssertEqual(canonical, vector["canonical"] as? String, vector["id"] as? String ?? "")
        }

        for vector in try XCTUnwrap(object["valid"] as? [[String: Any]]) {
            let id = try XCTUnwrap(vector["id"] as? String)
            let kind = try XCTUnwrap(vector["kind"] as? String)
            let canonical = try XCTUnwrap(vector["canonical"] as? String)
            let fixtureFrame = try Data(hex: try XCTUnwrap(vector["frame_hex"] as? String))
            let fixtureEnvelope = try Data(hex: try XCTUnwrap(vector["envelope_hex"] as? String))
            let frame: Data
            let envelope: Data
            let decodedFrame: String
            let decodedEnvelope: String
            switch kind {
            case "int":
                let value = try KotodamaNumericV1Codec.decodeIntJSON(canonical)
                frame = try KotodamaNumericV1Codec.encodeIntFrame(value)
                envelope = try KotodamaNumericV1Codec.encodeIntEnvelope(value)
                decodedFrame = try KotodamaNumericV1Codec.decodeIntFrame(fixtureFrame).canonicalString
                decodedEnvelope = try KotodamaNumericV1Codec.decodeIntEnvelope(fixtureEnvelope).canonicalString
            case "decimal":
                let value = try KotodamaNumericV1Codec.decodeDecimalJSON(canonical)
                frame = try KotodamaNumericV1Codec.encodeDecimalFrame(value)
                envelope = try KotodamaNumericV1Codec.encodeDecimalEnvelope(value)
                decodedFrame = try KotodamaNumericV1Codec.decodeDecimalFrame(fixtureFrame).canonicalString
                decodedEnvelope = try KotodamaNumericV1Codec.decodeDecimalEnvelope(fixtureEnvelope).canonicalString
            case "quantity":
                let value = try KotodamaNumericV1Codec.decodeQuantityJSON(canonical)
                frame = try KotodamaNumericV1Codec.encodeQuantityFrame(value)
                envelope = try KotodamaNumericV1Codec.encodeQuantityEnvelope(value)
                decodedFrame = try KotodamaNumericV1Codec.decodeQuantityFrame(fixtureFrame).canonicalString
                decodedEnvelope = try KotodamaNumericV1Codec.decodeQuantityEnvelope(fixtureEnvelope).canonicalString
            default:
                return XCTFail("unknown fixture kind \(kind)")
            }
            XCTAssertEqual(Data(frame.dropFirst(40)).hex, vector["body_hex"] as? String, "\(id) body")
            XCTAssertEqual(frame.hex, vector["frame_hex"] as? String, "\(id) frame")
            XCTAssertEqual(envelope.hex, vector["envelope_hex"] as? String, "\(id) envelope")
            XCTAssertEqual(decodedFrame, canonical, "\(id) frame decode")
            XCTAssertEqual(decodedEnvelope, canonical, "\(id) envelope decode")
        }

        for vector in try XCTUnwrap(object["invalid"] as? [[String: Any]]) {
            let input = try XCTUnwrap(vector["input"] as? String)
            let decodeAs = try XCTUnwrap(vector["decode_as"] as? String)
            let expected = try XCTUnwrap(
                KotodamaNumericV1ErrorCode(rawValue: try XCTUnwrap(vector["expected"] as? String))
            )
            let bytes = try Data(hex: try XCTUnwrap(vector["hex"] as? String))
            assertCode(expected) {
                switch (input, decodeAs) {
                case ("frame", "int"): _ = try KotodamaNumericV1Codec.decodeIntFrame(bytes)
                case ("frame", "decimal"): _ = try KotodamaNumericV1Codec.decodeDecimalFrame(bytes)
                case ("frame", "quantity"): _ = try KotodamaNumericV1Codec.decodeQuantityFrame(bytes)
                case ("envelope", "int"): _ = try KotodamaNumericV1Codec.decodeIntEnvelope(bytes)
                case ("envelope", "decimal"): _ = try KotodamaNumericV1Codec.decodeDecimalEnvelope(bytes)
                case ("envelope", "quantity"): _ = try KotodamaNumericV1Codec.decodeQuantityEnvelope(bytes)
                default: XCTFail("unknown fixture decoder \(input)/\(decodeAs)")
                }
            }
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

    private func repositoryRoot() -> URL {
        var current = URL(fileURLWithPath: #filePath).deletingLastPathComponent()
        while current.path != "/" {
            if FileManager.default.fileExists(
                atPath: current.appendingPathComponent("fixtures/numeric_v1_golden.json").path
            ) {
                return current
            }
            current.deleteLastPathComponent()
        }
        XCTFail("fixtures/numeric_v1_golden.json was not found")
        return URL(fileURLWithPath: "/")
    }
}

private extension Data {
    init(hex: String) throws {
        guard hex.count.isMultiple(of: 2),
              hex.range(of: #"^(?:[0-9a-f]{2})*$"#, options: .regularExpression) != nil else {
            throw KotodamaNumericV1Error(code: .invalidText, message: "fixture hex is malformed")
        }
        self.init()
        var index = hex.startIndex
        while index < hex.endIndex {
            let end = hex.index(index, offsetBy: 2)
            append(UInt8(hex[index..<end], radix: 16)!)
            index = end
        }
    }

    var hex: String {
        map { String(format: "%02x", $0) }.joined()
    }
}
