import Foundation

public enum ConnectEnvelopeError: Error, LocalizedError, Sendable {
    case bridgeUnavailable
    case unsupportedFrameKind
    case invalidEnvelope
    case invalidPayload
    case invalidBase64(String)
    case unknownPayloadKind(String)

    public var errorDescription: String? {
        switch self {
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "NoritoBridge connect envelope helpers are unavailable."
            )
        case .unsupportedFrameKind:
            return "Connect envelope decryption requires a ciphertext frame."
        case .invalidEnvelope:
            return "Failed to decode the decrypted connect envelope."
        case .invalidPayload:
            return "The decrypted connect envelope payload is missing required fields."
        case let .invalidBase64(field):
            return "Invalid base64 value for connect envelope field '\(field)'."
        case let .unknownPayloadKind(kind):
            return "Unknown connect envelope payload kind '\(kind)'."
        }
    }
}

public struct ConnectEnvelope: Equatable, Sendable {
    public let sequence: UInt64
    public let payload: ConnectEnvelopePayload

    public init(sequence: UInt64, payload: ConnectEnvelopePayload) {
        self.sequence = sequence
        self.payload = payload
    }

    /// Decrypt a ciphertext frame into a structured envelope using the provided symmetric key.
    public static func decrypt(frame: ConnectFrame, symmetricKey: Data) throws -> ConnectEnvelope {
        guard case .ciphertext = frame.kind else {
            throw ConnectEnvelopeError.unsupportedFrameKind
        }
        guard let encodedFrame = NoritoNativeBridge.shared.encodeConnectFrame(frame) else {
            throw ConnectEnvelopeError.bridgeUnavailable
        }
        guard let envelopeBytes = NoritoNativeBridge.shared.connectDecryptCiphertext(key: symmetricKey, frame: encodedFrame) else {
            throw ConnectEnvelopeError.bridgeUnavailable
        }
        if let jsonBytes = NoritoNativeBridge.shared.decodeEnvelopeJSON(envelopeBytes) {
            return try ConnectEnvelope.decode(jsonData: jsonBytes)
        }
        if let decoded = try? ConnectEnvelope.decode(jsonData: envelopeBytes) {
            return decoded
        }
        throw ConnectEnvelopeError.invalidEnvelope
    }

    static func decode(jsonData: Data) throws -> ConnectEnvelope {
        let object = try JSONSerialization.jsonObject(with: jsonData, options: [])
        guard let root = object as? [String: Any],
              let seqValue = StrictJSONNumber.uint64(from: root["seq"]),
              let payloadObject = root["payload"] as? [String: Any],
              payloadObject.count == 1,
              let (kind, payloadValue) = payloadObject.first
        else {
            throw ConnectEnvelopeError.invalidEnvelope
        }

        let payload = try ConnectEnvelopePayload(kind: kind, payload: payloadValue)
        return ConnectEnvelope(sequence: seqValue, payload: payload)
    }
}

public enum ConnectEnvelopePayload: Equatable, Sendable {
    case signRequestTx(txBytes: Data)
    case signRequestRaw(domainTag: String, bytes: Data)
    case signResultOk(signature: ConnectWalletSignature)
    case signResultErr(code: String, message: String)
    case displayRequest(title: String, body: String)
    case controlClose(ConnectClose)
    case controlReject(ConnectReject)
    case balanceSnapshot(ConnectBalanceSnapshot)

    init(kind: String, payload: Any) throws {
        switch kind {
        case "SignRequestTx":
            guard let dict = payload as? [String: Any] else {
                throw ConnectEnvelopeError.invalidPayload
            }
            let data = try Self.decodeBase64(dict["tx_bytes_b64"], field: "tx_bytes_b64")
            self = .signRequestTx(txBytes: data)
        case "SignRequestRaw":
            guard let dict = payload as? [String: Any],
                  let domainTag = dict["domain_tag"] as? String
            else {
                throw ConnectEnvelopeError.invalidPayload
            }
            let data = try Self.decodeBase64(dict["bytes_b64"], field: "bytes_b64")
            self = .signRequestRaw(domainTag: domainTag, bytes: data)
        case "SignResultOk":
            guard let dict = payload as? [String: Any],
                  let algorithm = dict["algorithm"] as? String
            else {
                throw ConnectEnvelopeError.invalidPayload
            }
            let signatureData = try Self.decodeBase64(dict["signature_b64"], field: "signature_b64")
            self = .signResultOk(signature: ConnectWalletSignature(algorithm: algorithm, signature: signatureData))
        case "SignResultErr":
            guard let dict = payload as? [String: Any],
                  let code = dict["code"] as? String,
                  let message = dict["message"] as? String
            else {
                throw ConnectEnvelopeError.invalidPayload
            }
            self = .signResultErr(code: code, message: message)
        case "DisplayRequest":
            guard let dict = payload as? [String: Any],
                  let title = dict["title"] as? String,
                  let body = dict["body"] as? String
            else {
                throw ConnectEnvelopeError.invalidPayload
            }
            self = .displayRequest(title: title, body: body)
        case "Control":
            guard let dict = payload as? [String: Any],
                  dict.count == 1,
                  let (controlKind, controlPayload) = dict.first,
                  let controlDict = controlPayload as? [String: Any]
            else {
                throw ConnectEnvelopeError.invalidPayload
            }
            switch controlKind {
            case "Close":
                guard let whoRaw = controlDict["who"] as? String,
                      let code = StrictJSONNumber.uint16(from: controlDict["code"]),
                      let retryable = controlDict["retryable"] as? Bool
                else {
                    throw ConnectEnvelopeError.invalidPayload
                }
                let role: ConnectRole
                switch whoRaw.lowercased() {
                case "app":
                    role = .app
                case "wallet":
                    role = .wallet
                default:
                    throw ConnectEnvelopeError.invalidPayload
                }
                let reason = (controlDict["reason"] as? String).flatMap { $0.isEmpty ? nil : $0 }
                let close = ConnectClose(role: role, code: code, reason: reason, retryable: retryable)
                self = .controlClose(close)
            case "Reject":
                guard let code = StrictJSONNumber.uint16(from: controlDict["code"]),
                      let codeId = controlDict["code_id"] as? String,
                      let reason = controlDict["reason"] as? String
                else {
                    throw ConnectEnvelopeError.invalidPayload
                }
                let reject = ConnectReject(code: code, codeID: codeId, reason: reason)
                self = .controlReject(reject)
            default:
                throw ConnectEnvelopeError.unknownPayloadKind(controlKind)
            }
        case "BalanceSnapshot":
            guard let dict = payload as? [String: Any] else {
                throw ConnectEnvelopeError.invalidPayload
            }
            let snapshot = try ConnectBalanceSnapshot(json: dict)
            self = .balanceSnapshot(snapshot)
        default:
            throw ConnectEnvelopeError.unknownPayloadKind(kind)
        }
    }

    public var control: ConnectControl? {
        switch self {
        case .controlClose(let close):
            return .close(close)
        case .controlReject(let reject):
            return .reject(reject)
        default:
            return nil
        }
    }
}

private extension ConnectEnvelopePayload {
    static func decodeBase64(_ value: Any?, field: String) throws -> Data {
        guard let string = value as? String else {
            throw ConnectEnvelopeError.invalidPayload
        }
        guard let data = Data(base64Encoded: string) else {
            throw ConnectEnvelopeError.invalidBase64(field)
        }
        return data
    }
}
