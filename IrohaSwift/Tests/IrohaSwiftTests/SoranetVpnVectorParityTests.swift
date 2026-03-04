import Foundation
import XCTest
@testable import IrohaSwift

private enum VpnVectorError: Error {
    case fixtureMissing
    case invalidHex(String)
    case invalidLength(expected: Int, actual: Int)
    case payloadOverrun(declared: Int, total: Int)
    case unknownClass(String)
}

private struct VpnVectorFixtureFile: Decodable {
    let flow_label_bits: UInt8
    let vectors: [VpnVectorFixture]
}

private struct VpnVectorFixture: Decodable {
    let name: String
    let `class`: String
    let flags: UInt8
    let circuit_id_hex: String
    let flow_label: UInt32
    let sequence: UInt64
    let ack: UInt64
    let padding_budget_ms: UInt16
    let payload_hex: String
    let frame_hex: String
}

private struct ParsedFrame {
    let version: UInt8
    let klass: UInt8
    let flags: UInt8
    let circuitId: Data
    let flowLabel: UInt32
    let sequence: UInt64
    let ack: UInt64
    let paddingBudgetMs: UInt16
    let payload: Data
    let padding: Data
}

private extension Data {
    init(hex: String) throws {
        let trimmed = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count % 2 == 0 else {
            throw VpnVectorError.invalidHex(trimmed)
        }
        var result = Data()
        var index = trimmed.startIndex
        while index < trimmed.endIndex {
            let next = trimmed.index(index, offsetBy: 2)
            guard next <= trimmed.endIndex else {
                throw VpnVectorError.invalidHex(trimmed)
            }
            let byteString = String(trimmed[index..<next])
            guard let byte = UInt8(byteString, radix: 16) else {
                throw VpnVectorError.invalidHex(trimmed)
            }
            result.append(byte)
            index = next
        }
        self = result
    }

    func hexString() -> String {
        map { String(format: "%02x", $0) }.joined()
    }
}

private func classTag(for label: String) throws -> UInt8 {
    switch label {
    case "data":
        return 0
    case "cover":
        return 1
    case "keep-alive":
        return 2
    case "control":
        return 3
    default:
        throw VpnVectorError.unknownClass(label)
    }
}

private func parseFrame(_ frame: Data) throws -> ParsedFrame {
    guard frame.count == SoranetVpnCellHeader.cellSize else {
        throw VpnVectorError.invalidLength(
            expected: SoranetVpnCellHeader.cellSize,
            actual: frame.count
        )
    }
    let version = frame[0]
    let klass = frame[1]
    let flags = frame[2]
    let circuitId = frame[3..<19]
    let flowLabel = frame[19..<22].reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
    let sequence = frame[22..<30].reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
    let ack = frame[30..<38].reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
    let paddingBudgetMs = frame[38..<40].reduce(UInt16(0)) { ($0 << 8) | UInt16($1) }
    let payloadLen = Int(frame[40..<42].reduce(UInt16(0)) { ($0 << 8) | UInt16($1) })
    let payloadStart = SoranetVpnCellHeader.headerSize
    let payloadEnd = payloadStart + payloadLen
    guard payloadEnd <= frame.count else {
        throw VpnVectorError.payloadOverrun(declared: payloadLen, total: frame.count)
    }
    let payload = frame[payloadStart..<payloadEnd]
    let padding = frame[payloadEnd..<frame.count]
    return ParsedFrame(
        version: version,
        klass: klass,
        flags: flags,
        circuitId: Data(circuitId),
        flowLabel: flowLabel,
        sequence: sequence,
        ack: ack,
        paddingBudgetMs: paddingBudgetMs,
        payload: Data(payload),
        padding: Data(padding)
    )
}

final class SoranetVpnVectorParityTests: XCTestCase {
    func testVectorsRoundTripAcrossRustAndSwift() throws {
        guard let url = Bundle.module.url(forResource: "vpn_vectors", withExtension: "json") else {
            throw VpnVectorError.fixtureMissing
        }
        let data = try Data(contentsOf: url)
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .useDefaultKeys
        let fixture = try decoder.decode(VpnVectorFixtureFile.self, from: data)
        XCTAssertEqual(fixture.flow_label_bits, 24)

        for vector in fixture.vectors {
            let classTag = try classTag(for: vector.class)
            let frame = try Data(hex: vector.frame_hex)
            let parsed = try parseFrame(frame)
            XCTAssertEqual(parsed.version, 1, vector.name)
            XCTAssertEqual(parsed.klass, classTag, vector.name)
            XCTAssertEqual(parsed.flags, vector.flags, vector.name)
            XCTAssertEqual(parsed.circuitId.hexString(), vector.circuit_id_hex, vector.name)
            XCTAssertEqual(parsed.flowLabel, vector.flow_label, vector.name)
            XCTAssertEqual(parsed.sequence, vector.sequence, vector.name)
            XCTAssertEqual(parsed.ack, vector.ack, vector.name)
            XCTAssertEqual(parsed.paddingBudgetMs, vector.padding_budget_ms, vector.name)
            XCTAssertEqual(parsed.payload.hexString(), vector.payload_hex, vector.name)
            XCTAssertTrue(parsed.padding.allSatisfy { $0 == 0 }, vector.name)

            let flowLabelMasked = parsed.flowLabel & 0x00FF_FFFF
            XCTAssertEqual(flowLabelMasked, parsed.flowLabel, vector.name)

            let header = try SoranetVpnCellHeader(
                circuitId: parsed.circuitId,
                flowLabel: parsed.flowLabel,
                sequence: parsed.sequence,
                ack: parsed.ack,
                klass: classTag,
                flags: parsed.flags,
                paddingBudgetMs: parsed.paddingBudgetMs
            )
            let encoded = try header.encode(payload: parsed.payload)
            XCTAssertEqual(encoded.hexString(), vector.frame_hex, vector.name)
        }
    }
}
