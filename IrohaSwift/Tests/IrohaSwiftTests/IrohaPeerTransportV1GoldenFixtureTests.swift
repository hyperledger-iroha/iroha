import Foundation
import XCTest
@testable import IrohaSwift

final class IrohaPeerTransportV1GoldenFixtureTests: XCTestCase {
    func testStaticAndZlibMessagesMatchCrossSDKGoldenFixture() throws {
        let fixture = try loadFixture()
        let profile = try XCTUnwrap(IrohaPeerWireProfileV1(rawValue: fixture.profile))
        let kind = try XCTUnwrap(IrohaPeerWireKindV1(rawValue: fixture.kind))

        let staticMessage = try IrohaPeerWireMessageV1(
            profile: profile,
            kind: kind,
            schemaVersion: fixture.schemaVersion,
            canonicalPayload: Data(fixture.canonicalUtf8.utf8)
        )
        XCTAssertEqual(staticMessage.canonicalHash.hexEncodedString(), fixture.canonicalHashHex)
        XCTAssertEqual(staticMessage.wireHash.hexEncodedString(), fixture.wireHashHex)
        XCTAssertEqual(staticMessage.encoded.hexEncodedString(), fixture.ipm1Hex)
        XCTAssertEqual(
            try XCTUnwrap(IrohaPeerQRCodecV1.staticCompleteTextCandidate(for: staticMessage)),
            fixture.iqr1
        )

        let zlibCanonical = Data(
            repeating: fixture.zlib.canonicalRepeatByte,
            count: fixture.zlib.canonicalCount
        )
        let zlibMessage = try IrohaPeerWireMessageV1(
            profile: profile,
            kind: kind,
            schemaVersion: fixture.schemaVersion,
            canonicalPayload: zlibCanonical,
            compressionPolicy: .peerOptimized
        )
        XCTAssertEqual(zlibMessage.encoding, .zlib)
        XCTAssertEqual(zlibMessage.encodedBody.hexEncodedString(), fixture.zlib.encodedBodyHex)
        XCTAssertEqual(zlibMessage.canonicalHash.hexEncodedString(), fixture.zlib.canonicalHashHex)
        XCTAssertEqual(zlibMessage.wireHash.hexEncodedString(), fixture.zlib.wireHashHex)
        XCTAssertEqual(zlibMessage.encoded.hexEncodedString(), fixture.zlib.ipm1Hex)
        XCTAssertEqual(
            try XCTUnwrap(IrohaPeerQRCodecV1.staticCompleteTextCandidate(for: zlibMessage)),
            fixture.zlib.iqr1
        )
    }

    func testAnimatedFrameOrderAndHashesMatchCrossSDKGoldenFixture() throws {
        let fixture = try loadFixture()
        XCTAssertEqual(fixture.animated.canonicalGenerator, "lcg32-high-byte")
        let profile = try XCTUnwrap(IrohaPeerWireProfileV1(rawValue: fixture.animated.profile))
        let kind = try XCTUnwrap(IrohaPeerWireKindV1(rawValue: fixture.animated.kind))
        let message = try IrohaPeerWireMessageV1(
            profile: profile,
            kind: kind,
            schemaVersion: fixture.animated.schemaVersion,
            canonicalPayload: lcgHighBytes(
                seed: fixture.animated.canonicalSeed,
                count: fixture.animated.canonicalCount
            )
        )
        XCTAssertEqual(message.wireHash.hexEncodedString(), fixture.animated.wireHashHex)

        let frames = try IrohaPeerQRCodecV1.animatedFrameTexts(for: message)
            .map { try IrohaPeerQRCodecV1.decodeFrame($0) }
        XCTAssertEqual(
            frames.map { "\($0.frameKind.rawValue):\($0.index):\($0.total)" },
            fixture.animated.frameKindIndexTotal
        )
        XCTAssertEqual(
            frames.map { Blake2b.hash256($0.encoded).hexEncodedString() },
            fixture.animated.frameBlake2b256Hex
        )
    }

    private func loadFixture() throws -> PeerTransportFixture {
        let sourceDirectory = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let url = sourceDirectory
            .appendingPathComponent("fixtures/offline/peer_transport_v1.json")
            .standardizedFileURL
        return try JSONDecoder().decode(
            PeerTransportFixture.self,
            from: Data(contentsOf: url)
        )
    }

    private func lcgHighBytes(seed: UInt32, count: Int) -> Data {
        var state = seed
        return Data((0..<count).map { _ in
            state = state &* 1_664_525 &+ 1_013_904_223
            return UInt8(truncatingIfNeeded: state >> 24)
        })
    }
}

private struct PeerTransportFixture: Decodable {
    let profile: UInt16
    let kind: UInt8
    let schemaVersion: UInt16
    let canonicalUtf8: String
    let canonicalHashHex: String
    let wireHashHex: String
    let ipm1Hex: String
    let iqr1: String
    let animated: Animated
    let zlib: Zlib

    enum CodingKeys: String, CodingKey {
        case profile
        case kind
        case schemaVersion = "schema_version"
        case canonicalUtf8 = "canonical_utf8"
        case canonicalHashHex = "canonical_hash_hex"
        case wireHashHex = "wire_hash_hex"
        case ipm1Hex = "ipm1_hex"
        case iqr1
        case animated
        case zlib
    }

    struct Animated: Decodable {
        let canonicalGenerator: String
        let canonicalSeed: UInt32
        let canonicalCount: Int
        let profile: UInt16
        let kind: UInt8
        let schemaVersion: UInt16
        let wireHashHex: String
        let frameKindIndexTotal: [String]
        let frameBlake2b256Hex: [String]

        enum CodingKeys: String, CodingKey {
            case canonicalGenerator = "canonical_generator"
            case canonicalSeed = "canonical_seed"
            case canonicalCount = "canonical_count"
            case profile
            case kind
            case schemaVersion = "schema_version"
            case wireHashHex = "wire_hash_hex"
            case frameKindIndexTotal = "frame_kind_index_total"
            case frameBlake2b256Hex = "frame_blake2b_256_hex"
        }
    }

    struct Zlib: Decodable {
        let canonicalRepeatByte: UInt8
        let canonicalCount: Int
        let encodedBodyHex: String
        let canonicalHashHex: String
        let wireHashHex: String
        let ipm1Hex: String
        let iqr1: String

        enum CodingKeys: String, CodingKey {
            case canonicalRepeatByte = "canonical_repeat_byte"
            case canonicalCount = "canonical_count"
            case encodedBodyHex = "encoded_body_hex"
            case canonicalHashHex = "canonical_hash_hex"
            case wireHashHex = "wire_hash_hex"
            case ipm1Hex = "ipm1_hex"
            case iqr1
        }
    }
}
