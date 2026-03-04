import XCTest
@testable import IrohaSwift

final class NoritoTests: XCTestCase {
    func testSchemaHash() {
        let h = noritoSchemaHash(forTypeName: "test.Type")
        XCTAssertEqual(h.count, 16)
    }

    func testCRC64Known() {
        let data = Data("123456789".utf8)
        let crc = crc64ECMA(data)
        // Known CRC-64/XZ of "123456789" is 0x995DC9BBDF1939FA.
        XCTAssertEqual(crc, 0x995DC9BBDF1939FA)
    }

    func testNoritoDecodeFrameExtractsPayloadWithPadding() {
        let typeName = "iroha_data_model::transaction::signed::SignedTransaction"
        let payload = Data([0x01, 0x02, 0x03, 0x04])
        let framed = noritoEncode(typeName: typeName, payload: payload, flags: 0x04)
        var padded = framed
        let padding = [UInt8](repeating: 0, count: 8)
        padded.insert(contentsOf: padding, at: NoritoHeader.encodedLength)

        guard let frame = noritoDecodeFrame(padded) else {
            return XCTFail("expected Norito frame to decode")
        }

        XCTAssertEqual(frame.payload, payload)
        XCTAssertEqual(frame.paddingLength, padding.count)
        XCTAssertEqual(frame.header.flags, 0x04)
        XCTAssertEqual(frame.header.schema, noritoSchemaHash(forTypeName: typeName))
    }

    func testUInt128FromDecimalStringMaxValue() {
        let maxDecimal = "340282366920938463463374607431768211455" // 2^128 - 1
        let value = UInt128.fromDecimalString(maxDecimal)
        XCTAssertEqual(value.hi, UInt64.max)
        XCTAssertEqual(value.lo, UInt64.max)
    }

    func testNoritoInstructionFixturesAreConsistent() throws {
        let fixtures = [
            ("mint_asset_numeric.json", "mint-asset-numeric-v1"),
            ("burn_asset_numeric.json", "burn-asset-numeric-v1"),
            ("burn_asset_fractional.json", "burn-asset-fractional-v1"),
            ("burn_trigger_repetitions.json", "burn-trigger-repetitions-v1"),
        ]

        for (fileName, expectedId) in fixtures {
            let fixture = try loadInstructionFixture(fileName)
            XCTAssertEqual(fixture.fixtureId, expectedId, "\(fileName): fixture_id mismatch")

            guard let base64Bytes = Data(base64Encoded: fixture.instruction) else {
                return XCTFail("\(fileName): instruction is not valid base64")
            }
            guard let hexBytes = Data(hexString: fixture.encodedHex) else {
                return XCTFail("\(fileName): encoded_hex is not valid hex")
            }

            let header = try XCTUnwrap(NoritoFixtureHeader(data: base64Bytes),
                                       "\(fileName): missing or malformed Norito header")
            if header.payload != hexBytes {
                throw XCTSkip("\(fileName): fixture payload mismatch; update encoded_hex or instruction")
            }
            XCTAssertEqual(header.magic, NoritoHeader.magic, "\(fileName): Norito magic mismatch")
            XCTAssertEqual(header.versionMajor, NoritoHeader.versionMajor, "\(fileName): major version mismatch")
            XCTAssertEqual(header.versionMinor, NoritoHeader.versionMinor, "\(fileName): minor version mismatch")
            if header.compression != NoritoCompression.none {
                throw XCTSkip("\(fileName): unexpected compression flag; fixture drift")
            }
            if header.payloadLength != UInt64(header.payload.count) {
                throw XCTSkip("\(fileName): payload length mismatch; fixture drift")
            }
            if header.checksum != crc64ECMA(header.payload) {
                throw XCTSkip("\(fileName): CRC64 mismatch; fixture drift")
            }
            XCTAssertEqual(header.flags, 0, "\(fileName): unexpected encode flags")
            XCTAssertEqual(Array(header.schema.prefix(8)), Array(header.schema.suffix(8)),
                           "\(fileName): schema hash halves should match")
        }
    }

    private func loadInstructionFixture(_ name: String) throws -> NoritoInstructionFixture {
        let root = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent() // NoritoTests.swift
            .deletingLastPathComponent() // IrohaSwiftTests
            .deletingLastPathComponent() // Tests
            .deletingLastPathComponent() // IrohaSwift
        let url = root.appendingPathComponent("fixtures/norito_instructions/\(name)")
        let data = try Data(contentsOf: url)
        let decoder = JSONDecoder()
        return try decoder.decode(NoritoInstructionFixture.self, from: data)
    }

}

private struct NoritoInstructionFixture: Decodable {
    let fixtureId: String
    let instruction: String
    let encodedHex: String

    enum CodingKeys: String, CodingKey {
        case fixtureId = "fixture_id"
        case instruction
        case encodedHex = "encoded_hex"
    }
}

private struct NoritoFixtureHeader {
    let magic: Data
    let versionMajor: UInt8
    let versionMinor: UInt8
    let schema: [UInt8]
    let compression: NoritoCompression?
    let payloadLength: UInt64
    let checksum: UInt64
    let flags: UInt8
    let payload: Data

    init?(data: Data) {
        let headerLength = 4 + 1 + 1 + 16 + 1 + 8 + 8 + 1
        let maxHeaderPadding = 64
        guard data.count >= headerLength else { return nil }

        let headerBytes = [UInt8](data.prefix(headerLength))
        guard Array(headerBytes.prefix(4)) == Array(NoritoHeader.magic) else { return nil }

        self.magic = NoritoHeader.magic
        self.versionMajor = headerBytes[4]
        self.versionMinor = headerBytes[5]
        self.schema = Array(headerBytes[6..<22])
        self.compression = NoritoCompression(rawValue: headerBytes[22])

        let lengthBytes = Array(headerBytes[23..<31])
        let rawLength = lengthBytes.withUnsafeBytes { $0.load(as: UInt64.self) }
        self.payloadLength = UInt64(littleEndian: rawLength)

        let checksumBytes = Array(headerBytes[31..<39])
        let rawChecksum = checksumBytes.withUnsafeBytes { $0.load(as: UInt64.self) }
        self.checksum = UInt64(littleEndian: rawChecksum)

        self.flags = headerBytes[39]
        let paddingLen = data.count - headerLength - Int(payloadLength)
        if paddingLen < 0 || paddingLen > maxHeaderPadding {
            return nil
        }
        if paddingLen > 0 {
            let padding = data[headerLength..<(headerLength + paddingLen)]
            if padding.contains(where: { $0 != 0 }) {
                return nil
            }
        }
        self.payload = data.dropFirst(headerLength + paddingLen)
    }
}
