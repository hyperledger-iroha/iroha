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
