import CryptoKit
import Foundation
import XCTest
@testable import IrohaSwift

/// Cross-SDK gate for Rust-authored Offline Cash V1 semantics and the shared IPM1/IQR1 vector.
final class OfflineCashInteropFixtureV1Tests: XCTestCase {
    private static let rustFixtureSHA256 =
        "dc56c0852d926c9496c6f24e59e9143d28be5529e635473355bb2a8c696de257"
    private static let transportFixtureSHA256 =
        "f61c3f5be020dd99d034b89cc17f0e44e10ed8516e821caf109c3743a8f176b4"

    func testRustFixturePinsSemanticProfile3MessagesAndExactKgm2Archives() throws {
        let fixtureData = try data(forFixture: "offline_cash_peer_transport_v1.json")
        XCTAssertEqual(Self.rustFixtureSHA256, sha256Hex(fixtureData))
        let fixture = try dictionary(from: fixtureData)
        XCTAssertEqual(try string("schema", in: fixture), "iroha.offline-cash.peer-transport.v1")
        XCTAssertEqual(try integer("native_bridge_abi", in: fixture), 22)

        let transport = try dictionary("transport", in: fixture)
        XCTAssertEqual(try integer("iroha_peer_wire_profile", in: transport), 3)
        XCTAssertEqual(try integer("native_text_schema_version", in: transport), 0x0100)
        XCTAssertEqual(try string("text_prefix", in: transport), "kgm2:")

        let limits = try dictionary("limits", in: fixture)
        XCTAssertEqual(try integer("payment_request_raw_max_bytes", in: limits), 768)
        XCTAssertEqual(try integer("payment_raw_max_bytes", in: limits), 7_936)
        XCTAssertEqual(try integer("acknowledgement_raw_max_bytes", in: limits), 256)
        XCTAssertEqual(try integer("payment_request_text_max_bytes", in: limits), 1_029)
        XCTAssertEqual(try integer("payment_text_max_bytes", in: limits), 10_587)
        XCTAssertEqual(try integer("acknowledgement_text_max_bytes", in: limits), 347)
        XCTAssertEqual(try integer("raw_session_max_bytes", in: limits), 9_211)
        XCTAssertEqual(try integer("text_session_max_bytes", in: limits), 12_288)

        let messages = try dictionary("messages", in: fixture)
        try assertMessage(
            try dictionary("payment_request", in: messages),
            kind: .receiveRequest,
            peerKind: "receive_request",
            stage: "receiver_payment_request",
            kindID: 1,
            rawMaximum: 768,
            textMaximum: 1_029,
            rawLength: 533,
            textLength: 716
        )
        try assertMessage(
            try dictionary("payment", in: messages),
            kind: .payment,
            peerKind: "payment",
            stage: "sender_payment",
            kindID: 2,
            rawMaximum: 7_936,
            textMaximum: 10_587,
            rawLength: 2_067,
            textLength: 2_761
        )
        try assertMessage(
            try dictionary("acknowledgement", in: messages),
            kind: .acknowledgement,
            peerKind: "acknowledgement",
            stage: "receiver_acknowledgement_after_persist",
            kindID: 3,
            rawMaximum: 256,
            textMaximum: 347,
            rawLength: 249,
            textLength: 337
        )

        let session = try dictionary("session", in: fixture)
        XCTAssertEqual(try integer("raw_norito_bytes", in: session), 2_849)
        XCTAssertEqual(try integer("kgm2_text_bytes", in: session), 3_814)
    }

    func testProfile3PaymentMatchesSharedIPM1AndIQR1Vector() throws {
        let fixtureData = try data(forFixture: "offline_cash_profile3_ipm_iqr_v1.json")
        XCTAssertEqual(Self.transportFixtureSHA256, sha256Hex(fixtureData))
        let fixture = try dictionary(from: fixtureData)
        XCTAssertEqual(try string("schema", in: fixture), "iroha.offline-cash.profile3-ipm-iqr.v1")
        XCTAssertTrue(try string("source", in: fixture).contains("Rust does not generate IPM1 or IQR1"))

        let semanticFixture = try dictionary("semantic_fixture", in: fixture)
        XCTAssertEqual(try string("sha256_hex", in: semanticFixture), Self.rustFixtureSHA256)
        XCTAssertEqual(try string("message", in: semanticFixture), "payment")
        let rustFixture = try dictionary(
            from: data(forFixture: "offline_cash_peer_transport_v1.json")
        )
        let payment = try dictionary(
            "payment",
            in: dictionary("messages", in: rustFixture)
        )
        XCTAssertEqual(
            try string("kgm2_text_sha256_hex", in: payment),
            try string("kgm2_text_sha256_hex", in: semanticFixture)
        )

        let canonicalText = Data(try string("kgm2_text", in: payment).utf8)
        let message = try IrohaPeerWireMessageV1(
            profile: .offlineCashV1,
            kind: .payment,
            schemaVersion: 0x0100,
            canonicalPayload: canonicalText,
            compressionPolicy: .peerOptimized
        )

        let transport = try dictionary("transport", in: fixture)
        XCTAssertEqual(try integer("profile", in: transport), 3)
        XCTAssertEqual(try string("payload_kind", in: transport), "payment")
        XCTAssertEqual(try integer("payload_kind_id", in: transport), 2)
        XCTAssertEqual(try integer("schema_version", in: transport), 0x0100)
        XCTAssertEqual(try string("compression_policy", in: transport), "peer_optimized")
        XCTAssertEqual(message.encoding, .zlib)
        XCTAssertEqual(try string("selected_content_encoding", in: transport), "zlib")
        XCTAssertEqual(try integer("canonical_payload_bytes", in: transport), canonicalText.count)
        XCTAssertEqual(try string("canonical_hash_hex", in: transport), message.canonicalHash.lowercaseHex)
        XCTAssertEqual(try string("wire_hash_hex", in: transport), message.wireHash.lowercaseHex)
        XCTAssertEqual(try string("stream_id_hex", in: transport), message.streamID.lowercaseHex)
        XCTAssertEqual(try integer("encoded_body_bytes", in: transport), message.encodedBody.count)
        XCTAssertEqual(
            try string("encoded_body_sha256_hex", in: transport),
            sha256Hex(message.encodedBody)
        )
        XCTAssertEqual(try integer("ipm1_bytes", in: transport), message.encoded.count)
        XCTAssertEqual(try string("ipm1_sha256_hex", in: transport), sha256Hex(message.encoded))
        XCTAssertEqual(try string("ipm1_encoded_hex", in: transport), message.encoded.lowercaseHex)
        XCTAssertEqual(
            try IrohaPeerWireMessageV1.decode(
                message.encoded,
                expectedProfile: .offlineCashV1,
                expectedKind: .payment
            ),
            message
        )

        let qr = try dictionary("qr", in: fixture)
        XCTAssertTrue(qr["static_complete_text"] is NSNull)
        XCTAssertNil(try IrohaPeerQRCodecV1.staticCompleteTextCandidate(for: message))
        let texts = try IrohaPeerQRCodecV1.animatedFrameTexts(for: message)
        XCTAssertEqual(try integer("animated_frame_count", in: qr), texts.count)
        XCTAssertEqual(try integer("parity_group_width", in: qr), 2)

        let expectedFrames = try dictionaries("frames", in: qr)
        XCTAssertEqual(try expectedFrames.map { try string("text", in: $0) }, texts)
        var frames: [IrohaPeerQRFrameV1] = []
        for (index, expected) in expectedFrames.enumerated() {
            let text = texts[index]
            let frame = try IrohaPeerQRCodecV1.decodeFrame(text)
            frames.append(frame)
            XCTAssertEqual(try integer("sequence", in: expected), index)
            XCTAssertEqual(try string("frame_kind", in: expected), frameKindName(frame.frameKind))
            XCTAssertEqual(try integer("frame_kind_id", in: expected), Int(frame.frameKind.rawValue))
            XCTAssertEqual(frame.profile, .offlineCashV1)
            XCTAssertEqual(frame.payloadKind, .payment)
            XCTAssertEqual(frame.streamID, message.streamID)
            XCTAssertEqual(try integer("index", in: expected), frame.index)
            XCTAssertEqual(try integer("total", in: expected), frame.total)
            XCTAssertEqual(try string("payload_sha256_hex", in: expected), sha256Hex(frame.payload))
            XCTAssertEqual(
                try string("encoded_frame_sha256_hex", in: expected),
                sha256Hex(frame.encoded)
            )
            XCTAssertEqual(
                try string("text_sha256_hex", in: expected),
                sha256Hex(Data(text.utf8))
            )
        }
        XCTAssertEqual(
            try integer("data_frame_total", in: qr),
            frames.filter { $0.frameKind == .data }.count
        )
        assertParity(frames)
    }

    private func assertMessage(
        _ fixture: [String: Any],
        kind: IrohaPeerWireKindV1,
        peerKind: String,
        stage: String,
        kindID: Int,
        rawMaximum: Int,
        textMaximum: Int,
        rawLength: Int,
        textLength: Int
    ) throws {
        XCTAssertEqual(fixture["semantic_valid"] as? Bool, true)
        XCTAssertEqual(try integer("iroha_peer_wire_profile", in: fixture), 3)
        XCTAssertEqual(try integer("native_text_schema_version", in: fixture), 0x0100)
        XCTAssertEqual(try integer("payload_kind_id", in: fixture), kindID)
        XCTAssertEqual(Int(kind.rawValue), kindID)
        XCTAssertEqual(try string("peer_payload_kind", in: fixture), peerKind)
        XCTAssertEqual(try string("stage", in: fixture), stage)
        XCTAssertEqual(try integer("maximum_raw_norito_bytes", in: fixture), rawMaximum)
        XCTAssertEqual(try integer("maximum_kgm2_text_bytes", in: fixture), textMaximum)
        XCTAssertEqual(try integer("raw_norito_bytes", in: fixture), rawLength)
        XCTAssertEqual(try integer("kgm2_text_bytes", in: fixture), textLength)

        let raw = try hex(string("raw_norito_hex", in: fixture))
        let text = try string("kgm2_text", in: fixture)
        let textData = Data(text.utf8)
        XCTAssertEqual(raw.count, rawLength)
        XCTAssertEqual(textData.count, textLength)
        XCTAssertEqual(text, "kgm2:" + base64URL(raw))
        XCTAssertEqual(try string("raw_norito_sha256_hex", in: fixture), sha256Hex(raw))
        XCTAssertEqual(try string("kgm2_text_sha256_hex", in: fixture), sha256Hex(textData))

        let message = try IrohaPeerWireMessageV1(
            profile: .offlineCashV1,
            kind: kind,
            schemaVersion: 0x0100,
            canonicalPayload: textData,
            compressionPolicy: .disabled
        )
        XCTAssertEqual(message.canonicalPayload, textData)
        XCTAssertEqual(
            try IrohaPeerWireMessageV1.decode(
                message.encoded,
                expectedProfile: .offlineCashV1,
                expectedKind: kind
            ),
            message
        )
    }

    private func assertParity(_ frames: [IrohaPeerQRFrameV1]) {
        let data = Dictionary(
            uniqueKeysWithValues: frames
                .filter { $0.frameKind == .data }
                .map { ($0.index, $0.payload) }
        )
        for parity in frames where parity.frameKind == .parity {
            guard var expected = data[parity.index * 2] else {
                XCTFail("missing first parity shard")
                continue
            }
            if let second = data[parity.index * 2 + 1] {
                for index in expected.indices { expected[index] ^= second[index] }
            }
            XCTAssertEqual(expected, parity.payload)
        }
    }

    private func data(forFixture name: String) throws -> Data {
        try Data(contentsOf: fixtureURL(name))
    }

    private func fixtureURL(_ name: String) -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("fixtures/offline")
            .appendingPathComponent(name)
            .standardizedFileURL
    }

    private func dictionary(from data: Data) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private func dictionary(_ key: String, in value: [String: Any]) throws -> [String: Any] {
        try XCTUnwrap(value[key] as? [String: Any], key)
    }

    private func dictionaries(_ key: String, in value: [String: Any]) throws -> [[String: Any]] {
        try XCTUnwrap(value[key] as? [[String: Any]], key)
    }

    private func string(_ key: String, in value: [String: Any]) throws -> String {
        try XCTUnwrap(value[key] as? String, key)
    }

    private func integer(_ key: String, in value: [String: Any]) throws -> Int {
        try XCTUnwrap(value[key] as? NSNumber, key).intValue
    }

    private func hex(_ value: String) throws -> Data {
        guard value.count.isMultiple(of: 2) else { throw FixtureError.invalidHex }
        var bytes = Data(capacity: value.count / 2)
        var index = value.startIndex
        while index < value.endIndex {
            let end = value.index(index, offsetBy: 2)
            guard let byte = UInt8(value[index..<end], radix: 16) else {
                throw FixtureError.invalidHex
            }
            bytes.append(byte)
            index = end
        }
        return bytes
    }

    private func base64URL(_ data: Data) -> String {
        data.base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }

    private func sha256Hex(_ data: Data) -> String {
        Data(SHA256.hash(data: data)).lowercaseHex
    }

    private func frameKindName(_ kind: IrohaPeerQRFrameKindV1) -> String {
        switch kind {
        case .complete: return "complete"
        case .header: return "header"
        case .data: return "data"
        case .parity: return "parity"
        }
    }

    private enum FixtureError: Error {
        case invalidHex
    }
}

private extension Data {
    var lowercaseHex: String {
        map { String(format: "%02x", $0) }.joined()
    }
}
