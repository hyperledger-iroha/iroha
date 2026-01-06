import XCTest
@testable import IrohaSwift

final class ConnectEnvelopeTests: XCTestCase {
    private func requireBridge() throws {
        try XCTSkipIf(!NoritoNativeBridge.shared.isConnectCryptoAvailable,
                      "NoritoBridge connect crypto symbols not linked")
    }

    func testDecodeSignRequestTxPayload() throws {
        let bytes = Data([0xDE, 0xAD, 0xBE, 0xEF])
        let payload: [String: Any] = [
            "tx_bytes_b64": bytes.base64EncodedString()
        ]

        let decoded = try ConnectEnvelopePayload(kind: "SignRequestTx", payload: payload)
        guard case .signRequestTx(let decodedBytes) = decoded else {
            XCTFail("Expected SignRequestTx payload")
            return
        }
        XCTAssertEqual(decodedBytes, bytes)
    }

    func testDecodeSignResultOkInvalidBase64() {
        let payload: [String: Any] = [
            "algorithm": "ed25519",
            "signature_b64": "!!!not-base64!!!"
        ]

        XCTAssertThrowsError(try ConnectEnvelopePayload(kind: "SignResultOk", payload: payload)) { error in
            guard case ConnectEnvelopeError.invalidBase64("signature_b64") = error else {
                XCTFail("Expected invalidBase64 signature_b64 error, got \(error)")
                return
            }
        }
    }

    func testDecodeRejectsFractionalSequence() throws {
        let payload: [String: Any] = [
            "DisplayRequest": ["title": "Hello", "body": "World"]
        ]
        let envelope: [String: Any] = [
            "seq": 1.5,
            "payload": payload
        ]
        let json = try JSONSerialization.data(withJSONObject: envelope, options: [])

        XCTAssertThrowsError(try ConnectEnvelope.decode(jsonData: json)) { error in
            guard case ConnectEnvelopeError.invalidEnvelope = error else {
                XCTFail("Expected invalidEnvelope, got \(error)")
                return
            }
        }
    }

    func testDecodeRejectsOutOfRangeControlCode() {
        let payload: [String: Any] = [
            "Close": [
                "who": "wallet",
                "code": 70000,
                "retryable": false
            ]
        ]

        XCTAssertThrowsError(try ConnectEnvelopePayload(kind: "Control", payload: payload)) { error in
            guard case ConnectEnvelopeError.invalidPayload = error else {
                XCTFail("Expected invalidPayload, got \(error)")
                return
            }
        }
    }

    func testDecodeRejectsMultiplePayloadKinds() throws {
        let payload: [String: Any] = [
            "DisplayRequest": ["title": "Hello", "body": "World"],
            "SignResultErr": ["code": "oops", "message": "bad"]
        ]
        let envelope: [String: Any] = [
            "seq": 1,
            "payload": payload
        ]
        let json = try JSONSerialization.data(withJSONObject: envelope, options: [])

        XCTAssertThrowsError(try ConnectEnvelope.decode(jsonData: json)) { error in
            guard case ConnectEnvelopeError.invalidEnvelope = error else {
                XCTFail("Expected invalidEnvelope, got \(error)")
                return
            }
        }
    }

    func testDecodeRejectsMultipleControlKinds() {
        let payload: [String: Any] = [
            "Close": ["who": "app", "code": 1000, "retryable": false],
            "Reject": ["code": 1000, "code_id": "X", "reason": "dup"]
        ]

        XCTAssertThrowsError(try ConnectEnvelopePayload(kind: "Control", payload: payload)) { error in
            guard case ConnectEnvelopeError.invalidPayload = error else {
                XCTFail("Expected invalidPayload, got \(error)")
                return
            }
        }
    }

    func testDecryptControlCloseEnvelope() throws {
        try requireBridge()
        let key = Data(repeating: 0xCD, count: 32)
        let sessionID = Data(repeating: 0xAB, count: 32)
        let sequence: UInt64 = 42
        let reason = "session complete"

        guard let envelopeBytes = NoritoNativeBridge.shared.encodeEnvelopeControlClose(sequence: sequence,
                                                                                       who: .wallet,
                                                                                       code: 1000,
                                                                                       reason: reason,
                                                                                       retryable: false) else {
            throw XCTSkip("NoritoBridge encodeEnvelopeControlClose not linked")
        }
        guard let frameBytes = NoritoNativeBridge.shared.connectEncryptEnvelope(key: key,
                                                                                sessionID: sessionID,
                                                                                direction: .walletToApp,
                                                                                envelope: envelopeBytes) else {
            throw XCTSkip("NoritoBridge connectEncryptEnvelope not linked")
        }

        let frame = try ConnectCodec.decode(frameBytes)
        let decrypted = try ConnectEnvelope.decrypt(frame: frame, symmetricKey: key)
        XCTAssertEqual(decrypted.sequence, sequence)
        if case .controlClose(let close) = decrypted.payload {
            XCTAssertEqual(close.role, .wallet)
            XCTAssertEqual(close.code, 1000)
            XCTAssertEqual(close.reason, reason)
            XCTAssertFalse(close.retryable)
        } else {
            XCTFail("Expected control close payload")
        }
    }
}
