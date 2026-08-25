import CryptoKit
import Foundation
import XCTest
import IrohaSwift

/// Native-independent transport framing only. The fixture's one-byte Norito
/// body is deliberately not passed to the typed Kagemusha semantic adapter.
final class IrohaPeerKagemushaStructuralRailFixtureTests: XCTestCase {
    func testQualifiedStructuralFixtureCrossesIPMQRNFCAndNearbyByteForByte() throws {
        let fixture = try loadFixture()
        XCTAssertEqual(fixture["semantic_valid"] as? Bool, false)
        let norito = try dictionary("norito", in: fixture)
        let archive = try hex(string("archive_hex", in: norito))
        XCTAssertEqual(archive.count, 49)

        let message = try IrohaPeerWireMessageV1(
            profile: .kagemusha,
            kind: .receiveRequest,
            schemaVersion: 0x0102,
            canonicalPayload: archive
        )
        let ipm = try dictionary("ipm1", in: fixture)
        XCTAssertEqual(message.encoded.count, try integer("message_bytes", in: ipm))
        XCTAssertEqual(message.canonicalHash.lowercaseHex, try string("canonical_hash_hex", in: ipm))
        XCTAssertEqual(message.wireHash.lowercaseHex, try string("wire_hash_hex", in: ipm))
        XCTAssertEqual(message.encoded, try hex(string("encoded_hex", in: ipm)))
        XCTAssertEqual(try IrohaPeerWireMessageV1.decode(message.encoded), message)

        let qr = try dictionary("qr", in: fixture)
        let text = try XCTUnwrap(IrohaPeerQRCodecV1.staticCompleteTextCandidate(for: message))
        XCTAssertEqual(try integer("frame_count", in: qr), 1)
        XCTAssertEqual(text, try string("static_text", in: qr))
        switch try IrohaPeerQRScanSessionV1().ingest(text) {
        case .completed(let scanned): XCTAssertEqual(scanned.message, message)
        default: XCTFail("single-frame structural fixture must complete")
        }

        let nfc = try dictionary("nfc", in: fixture)
        let nfcSession = try hex(string("session_hex", in: nfc))
        var receiverCard = try IrohaPeerNfcReceiverSessionV1(
            sessionID: nfcSession,
            receiveRequest: message.encoded
        )
        XCTAssertEqual(
            try receiverCard.info().encode(),
            try hex(string("info_hex", in: nfc))
        )
        let read = IrohaPeerNfcCommandV1.readRequest(
            sessionID: nfcSession,
            requestCanonicalHash: message.canonicalHash,
            offset: 0,
            length: message.encoded.count
        )
        XCTAssertEqual(
            try IrohaPeerNfcAPDUCodecV1.encode(read),
            try hex(string("read_request_apdu_hex", in: nfc))
        )
        XCTAssertEqual(
            try receiverCard.handle(read),
            try hex(string("read_request_response_hex", in: nfc))
        )

        let nearby = try dictionary("nearby", in: fixture)
        let nearbySession = try hex(string("session_hex", in: nearby))
        let requestHash = try hex(string("request_hash_hex", in: nearby))
        XCTAssertEqual(requestHash, message.canonicalHash)
        var sender = try IrohaPeerNearbySessionV1(
            profile: .kagemusha,
            localRole: .sender,
            sessionID: nearbySession,
            requestCanonicalHash: requestHash,
            deviceCertificate: try hex(string("sender_certificate_hex", in: nearby)),
            nonce: Data(
                repeating: UInt8(try integer("sender_nonce_repeat_byte", in: nearby)),
                count: 32
            ),
            ephemeralPrivateKey: try fixedP256Key(
                scalar: UInt8(try integer("sender_private_scalar", in: nearby))
            )
        )
        var receiver = try IrohaPeerNearbySessionV1(
            profile: .kagemusha,
            localRole: .receiver,
            sessionID: nearbySession,
            requestCanonicalHash: requestHash,
            deviceCertificate: try hex(string("receiver_certificate_hex", in: nearby)),
            nonce: Data(
                repeating: UInt8(try integer("receiver_nonce_repeat_byte", in: nearby)),
                count: 32
            ),
            ephemeralPrivateKey: try fixedP256Key(
                scalar: UInt8(try integer("receiver_private_scalar", in: nearby))
            )
        )
        try sender.acceptPeerHello(receiver.localHello)
        try receiver.acceptPeerHello(sender.localHello)
        let senderAuthentication = try sender.makeAuthentication(
            signature: try hex(string("sender_authentication_signature_hex", in: nearby))
        )
        let receiverAuthentication = try receiver.makeAuthentication(
            signature: try hex(string("receiver_authentication_signature_hex", in: nearby))
        )
        XCTAssertEqual(
            senderAuthentication.transcriptHash.lowercaseHex,
            try string("transcript_hash_hex", in: nearby)
        )
        let acceptAll: IrohaPeerNearbySessionV1.SignatureVerifier = { _, _, _, _ in true }
        try sender.acceptPeerAuthentication(receiverAuthentication, verifier: acceptAll)
        try receiver.acceptPeerAuthentication(senderAuthentication, verifier: acceptAll)
        let record = try sender.seal(message.encoded)
        XCTAssertEqual(record.encode(), try hex(string("sender_record_hex", in: nearby)))
        XCTAssertEqual(
            try receiver.open(IrohaPeerNearbyEncryptedRecordV1.decode(record.encode())),
            message.encoded
        )
    }

    private func loadFixture() throws -> [String: Any] {
        let sourceDirectory = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .deletingLastPathComponent()
        let url = sourceDirectory
            .appendingPathComponent("fixtures/offline/kagemusha_peer_transport_v2.json")
            .standardizedFileURL
        let value = try JSONSerialization.jsonObject(with: Data(contentsOf: url))
        return try XCTUnwrap(value as? [String: Any])
    }

    private func dictionary(
        _ key: String,
        in value: [String: Any]
    ) throws -> [String: Any] {
        try XCTUnwrap(value[key] as? [String: Any], key)
    }

    private func string(_ key: String, in value: [String: Any]) throws -> String {
        try XCTUnwrap(value[key] as? String, key)
    }

    private func integer(_ key: String, in value: [String: Any]) throws -> Int {
        try XCTUnwrap(value[key] as? NSNumber, key).intValue
    }

    private func hex(_ value: String) throws -> Data {
        try XCTUnwrap(Data(hexString: value))
    }

    private func fixedP256Key(scalar: UInt8) throws -> P256.KeyAgreement.PrivateKey {
        try P256.KeyAgreement.PrivateKey(
            rawRepresentation: Data(repeating: 0, count: 31) + Data([scalar])
        )
    }
}

private extension Data {
    var lowercaseHex: String {
        map { String(format: "%02x", $0) }.joined()
    }
}
