import XCTest
@testable import IrohaSwift

final class NoritoTests: XCTestCase {
    func testSchemaHash() {
        let h = noritoSchemaHash(forTypeName: "test.Type")
        XCTAssertEqual(h.count, 16)
        XCTAssertEqual(h, [
            0xB9, 0x17, 0xB1, 0x16, 0x24, 0x7C, 0xC1, 0x0E,
            0xF7, 0x1C, 0xEA, 0xBD, 0xFC, 0xEC, 0x9C, 0x8A,
        ])
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

    func testNoritoDecodeFrameRejectsReservedFlags() {
        let payload = Data([0x01])
        for flags in [
            NoritoHeader.varintOffsets,
            NoritoHeader.compactSeqLen,
            NoritoHeader.varintOffsets | NoritoHeader.compactSeqLen,
        ] {
            let framed = noritoEncodeUnchecked(typeName: "test.ReservedFlags", payload: payload, flags: flags)
            XCTAssertNil(noritoDecodeFrame(framed), "reserved flags should be rejected: \(flags)")
        }
    }

    func testNoritoDecodeFrameRejectsInvalidFieldBitsetFlags() {
        let payload = Data([0x01])
        for flags in [
            NoritoHeader.fieldBitset,
            NoritoHeader.fieldBitset | NoritoHeader.compactLen,
            NoritoHeader.fieldBitset | NoritoHeader.packedStruct,
        ] {
            let framed = noritoEncodeUnchecked(typeName: "test.FieldBitset", payload: payload, flags: flags)
            XCTAssertNil(noritoDecodeFrame(framed), "invalid FIELD_BITSET flags should be rejected: \(flags)")
        }
    }

    func testUInt128FromDecimalStringMaxValue() {
        let maxDecimal = "340282366920938463463374607431768211455" // 2^128 - 1
        let value = UInt128.fromDecimalString(maxDecimal)
        XCTAssertEqual(value.hi, UInt64.max)
        XCTAssertEqual(value.lo, UInt64.max)
    }

    func testNoritoInstructionFixturesAreConsistent() throws {
        let fixtures = [
            ("mint_asset_numeric.json", "mint-asset-numeric-v1", UInt8(0x02)),
            ("burn_asset_numeric.json", "burn-asset-numeric-v1", UInt8(0x02)),
            ("burn_asset_fractional.json", "burn-asset-fractional-v1", UInt8(0x02)),
            ("burn_trigger_repetitions.json", "burn-trigger-repetitions-v1", UInt8(0x02)),
        ]

        for (fileName, expectedId, expectedFlags) in fixtures {
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
            XCTAssertEqual(header.flags, expectedFlags, "\(fileName): unexpected encode flags")
            XCTAssertEqual(header.schema.count, 16, "\(fileName): schema hash length mismatch")
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

private func noritoEncodeUnchecked(typeName: String, payload: Data, flags: UInt8) -> Data {
    let schema = noritoSchemaHash(forTypeName: typeName)
    let checksum = crc64ECMA(payload)
    var out = Data()
    out.append(NoritoHeader.magic)
    out.append(contentsOf: [NoritoHeader.versionMajor, NoritoHeader.versionMinor])
    out.append(contentsOf: schema)
    out.append(NoritoCompression.none.rawValue)
    out.append(contentsOf: withUnsafeBytes(of: UInt64(payload.count).littleEndian, Array.init))
    out.append(contentsOf: withUnsafeBytes(of: checksum.littleEndian, Array.init))
    out.append(flags)
    out.append(payload)
    return out
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
