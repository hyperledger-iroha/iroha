import Foundation
import XCTest
@testable import IrohaSwift

final class NativeSumeragiStatusWireTests: XCTestCase {
    private func sample() throws -> Data {
        try XCTUnwrap(NativeStatusFixtures.rows().first { $0.0 == "validator" }).2
    }
    private func frame(_ payload: Data) -> Data {
        noritoEncode(typeName: "iroha_data_model::sumeragi::SumeragiStatus", payload: payload,
                     flags: NoritoHeader.compactLen, payloadAlignment: 8)
    }
    func testEveryRustFrameMatchesJsonAndReencodesExactly() throws {
        let rows = try NativeStatusFixtures.rows()
        XCTAssertEqual(rows.count, 8)
        for (name, json, wire) in rows {
            let expected = try ToriiSumeragiStatusSnapshot.parseJSON(json)
            let actual = try SumeragiStatusWire.decodeCanonical(wire)
            XCTAssertEqual(expected, actual, name)
            XCTAssertEqual(try SumeragiStatusWire.encode(actual), wire, name)
            var changed = wire
            changed.resetBytes(in: 0..<changed.count)
            XCTAssertEqual(try SumeragiStatusWire.encode(actual), wire, "decoder retains independent value")
            switch actual.halted {
            case .safetyViolation(let height)?, .applyDiverged(let height)?, .publicationRecoveryRequired(let height)?:
                XCTAssertEqual(height, UInt64.max)
            default: break
            }
        }
    }
    func testMalformedHeadersCompressionAndEveryTruncatedPrefixFailClosed() throws {
        let wire = try sample()
        for offset in [0, 4, 5, 6, 22, 23, 31, 39] {
            var changed = wire; changed[offset] ^= 1
            XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(changed), "header \(offset)")
        }
        XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(Data(wire.dropFirst(40))))
        XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(wire + Data([0])))
        XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(Data(wire.prefix(40)) + Data([0]) + Data(wire.dropFirst(40))))
        XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(Data(repeating: 0, count: 1_048_577)))
        for count in 0..<wire.count {
            XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(Data(wire.prefix(count))), "prefix \(count)")
        }
    }
    func testChecksummedPayloadsRejectRetiredProtocolAndNoncanonicalFields() throws {
        let payload = try XCTUnwrap(noritoDecodeFrame(sample())).payload
        XCTAssertEqual(payload[0], 2)
        for version: UInt8 in [0, 4, 6, 7, 9] {
            var changed = payload; changed[1] = version; changed[2] = 0
            XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(frame(changed)))
        }
        XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(frame(Data([0x82, 0]) + Data(payload.dropFirst()))))
        XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(frame(Data([0xff, 0xff, 0xff, 1]))))
        XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(frame(payload + Data([0]))))
        var invalidOption = payload; invalidOption[37] = 2
        XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(frame(invalidOption)))
    }

    func testBeaconSessionRequiresCanonicalNestedFixedArrayLayout() throws {
        var payload = try NoritoFieldFrames.Reader(XCTUnwrap(noritoDecodeFrame(sample())).payload)
        var fields = try (0..<21).map { _ in try payload.field() }
        try payload.finish()
        var option = NoritoFieldFrames.Reader(fields[2])
        XCTAssertEqual(try option.raw(1), Data([1]))
        var horizon = try NoritoFieldFrames.Reader(option.field())
        try option.finish()
        var nested = try (0..<5).map { _ in try horizon.field() }
        try horizon.finish()
        // Option<[u8;32]> frames each element; raw32 is a different layout even
        // when the enclosing frame has a correct schema and checksum.
        nested[2] = Data([1]) + NoritoFieldFrames.record([Data(repeating: 0xab, count: 32)])
        fields[2] = Data([1]) + NoritoFieldFrames.record([NoritoFieldFrames.record(nested)])
        XCTAssertThrowsError(try SumeragiStatusWire.decodeCanonical(frame(NoritoFieldFrames.record(fields))))
    }
}
