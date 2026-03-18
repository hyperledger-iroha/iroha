import XCTest
@testable import IrohaSwift

final class ConnectEnvelopeCodecTests: XCTestCase {
    private func requireConnectBridge() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCodecAvailable ||
                      !NoritoNativeBridge.shared.isConnectCryptoAvailable,
                      "NoritoBridge connect codec/crypto unavailable")
    }

    func testEncryptSignResultOkRoundTrip() throws {
        try requireConnectBridge()

        let signature = Data((0..<64).map(UInt8.init))
        let key = Data(repeating: 0x4A, count: 32)
        let sessionID = Data(repeating: 0x5B, count: 32)

        let ciphertext = try ConnectEnvelopeCodec.encryptSignResultOk(sequence: 7,
                                                                      signature: signature,
                                                                      algorithm: "ed25519",
                                                                      key: key,
                                                                      sessionID: sessionID,
                                                                      direction: .walletToApp)
        let frame = try ConnectCodec.decode(ciphertext)
        let envelope = try ConnectEnvelope.decrypt(frame: frame, symmetricKey: key)
        XCTAssertEqual(envelope.sequence, 7)
        if case let .signResultOk(payload) = envelope.payload {
            XCTAssertEqual(payload.signature, signature)
            XCTAssertEqual(payload.algorithm.lowercased(), "ed25519")
        } else {
            XCTFail("Expected SignResultOk payload")
        }
    }

    func testEncryptControlRejectRoundTrip() throws {
        try requireConnectBridge()

        let key = Data(repeating: 0x2D, count: 32)
        let sessionID = Data(repeating: 0x3E, count: 32)

        let ciphertext = try ConnectEnvelopeCodec.encryptControlReject(sequence: 11,
                                                                       code: 403,
                                                                       codeID: "USER_DENIED",
                                                                       reason: "Rejected in test",
                                                                       key: key,
                                                                       sessionID: sessionID,
                                                                       direction: .walletToApp)
        let frame = try ConnectCodec.decode(ciphertext)
        let envelope = try ConnectEnvelope.decrypt(frame: frame, symmetricKey: key)
        XCTAssertEqual(envelope.sequence, 11)
        if case let .controlReject(reject) = envelope.payload {
            XCTAssertEqual(reject.code, 403)
            XCTAssertEqual(reject.codeID, "USER_DENIED")
            XCTAssertEqual(reject.reason, "Rejected in test")
        } else {
            XCTFail("Expected Control Reject payload")
        }
    }

    func testEncryptEnvelopeValidatesLengths() {
        let envelope = Data([0x01, 0x02])
        XCTAssertThrowsError(try ConnectEnvelopeCodec.encryptEnvelope(envelope,
                                                                      key: Data(),
                                                                      sessionID: Data(repeating: 0x01, count: 32),
                                                                      direction: .walletToApp)) { error in
            XCTAssertEqual(error as? ConnectEnvelopeCodecError,
                           .invalidKeyLength(expected: 32, actual: 0))
        }
        XCTAssertThrowsError(try ConnectEnvelopeCodec.encryptEnvelope(envelope,
                                                                      key: Data(repeating: 0x01, count: 32),
                                                                      sessionID: Data(),
                                                                      direction: .walletToApp)) { error in
            XCTAssertEqual(error as? ConnectEnvelopeCodecError,
                           .invalidSessionIdentifierLength(expected: 32, actual: 0))
        }
    }
}
