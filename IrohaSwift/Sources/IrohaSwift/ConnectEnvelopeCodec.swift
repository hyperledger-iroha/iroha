import Foundation

public enum ConnectEnvelopeCodecError: Error, LocalizedError, Sendable, Equatable {
    case bridgeUnavailable
    case encodeFailed
    case encryptFailed
    case invalidKeyLength(expected: Int, actual: Int)
    case invalidSessionIdentifierLength(expected: Int, actual: Int)

    public var errorDescription: String? {
        switch self {
        case .bridgeUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "NoritoBridge connect envelope codec helpers are unavailable."
            )
        case .encodeFailed:
            return "Failed to encode connect envelope payload."
        case .encryptFailed:
            return "Failed to encrypt connect envelope payload."
        case let .invalidKeyLength(expected, actual):
            return "Connect envelope key must be \(expected) bytes (got \(actual))."
        case let .invalidSessionIdentifierLength(expected, actual):
            return "Connect session identifiers must be \(expected) bytes (got \(actual))."
        }
    }
}

public enum ConnectEnvelopeCodec {
    private static let keyLength = 32

    public static func encodeSignResultOk(sequence: UInt64,
                                          algorithm: String? = nil,
                                          signature: Data) throws -> Data {
        guard let envelope = NoritoNativeBridge.shared.encodeEnvelopeSignResultOk(sequence: sequence,
                                                                                  algorithm: algorithm,
                                                                                  signature: signature) else {
            throw mapBridgeFailure()
        }
        return envelope
    }

    public static func encodeSignResultErr(sequence: UInt64,
                                           code: String,
                                           message: String) throws -> Data {
        guard let envelope = NoritoNativeBridge.shared.encodeEnvelopeSignResultErr(sequence: sequence,
                                                                                   code: code,
                                                                                   message: message) else {
            throw mapBridgeFailure()
        }
        return envelope
    }

    public static func encodeControlReject(sequence: UInt64,
                                           code: UInt16,
                                           codeID: String,
                                           reason: String) throws -> Data {
        guard let envelope = NoritoNativeBridge.shared.encodeEnvelopeControlReject(sequence: sequence,
                                                                                   code: code,
                                                                                   codeID: codeID,
                                                                                   reason: reason) else {
            throw mapBridgeFailure()
        }
        return envelope
    }

    public static func encodeControlClose(sequence: UInt64,
                                          role: ConnectRole,
                                          code: UInt16,
                                          reason: String?,
                                          retryable: Bool) throws -> Data {
        guard let envelope = NoritoNativeBridge.shared.encodeEnvelopeControlClose(sequence: sequence,
                                                                                  who: role,
                                                                                  code: code,
                                                                                  reason: reason,
                                                                                  retryable: retryable) else {
            throw mapBridgeFailure()
        }
        return envelope
    }

    public static func encryptEnvelope(_ envelope: Data,
                                       key: Data,
                                       sessionID: Data,
                                       direction: ConnectDirection) throws -> Data {
        guard key.count == keyLength else {
            throw ConnectEnvelopeCodecError.invalidKeyLength(expected: keyLength, actual: key.count)
        }
        guard sessionID.count == keyLength else {
            throw ConnectEnvelopeCodecError.invalidSessionIdentifierLength(expected: keyLength,
                                                                          actual: sessionID.count)
        }
        guard let ciphertextFrame = NoritoNativeBridge.shared.connectEncryptEnvelope(key: key,
                                                                                     sessionID: sessionID,
                                                                                     direction: direction,
                                                                                     envelope: envelope) else {
            throw mapEncryptionFailure()
        }
        return ciphertextFrame
    }

    public static func encryptSignResultOk(sequence: UInt64,
                                           signature: Data,
                                           algorithm: String? = nil,
                                           key: Data,
                                           sessionID: Data,
                                           direction: ConnectDirection) throws -> Data {
        let envelope = try encodeSignResultOk(sequence: sequence, algorithm: algorithm, signature: signature)
        return try encryptEnvelope(envelope, key: key, sessionID: sessionID, direction: direction)
    }

    public static func encryptSignResultErr(sequence: UInt64,
                                            code: String,
                                            message: String,
                                            key: Data,
                                            sessionID: Data,
                                            direction: ConnectDirection) throws -> Data {
        let envelope = try encodeSignResultErr(sequence: sequence, code: code, message: message)
        return try encryptEnvelope(envelope, key: key, sessionID: sessionID, direction: direction)
    }

    public static func encryptControlReject(sequence: UInt64,
                                            code: UInt16,
                                            codeID: String,
                                            reason: String,
                                            key: Data,
                                            sessionID: Data,
                                            direction: ConnectDirection) throws -> Data {
        let envelope = try encodeControlReject(sequence: sequence, code: code, codeID: codeID, reason: reason)
        return try encryptEnvelope(envelope, key: key, sessionID: sessionID, direction: direction)
    }

    public static func encryptControlClose(sequence: UInt64,
                                           role: ConnectRole,
                                           code: UInt16,
                                           reason: String?,
                                           retryable: Bool,
                                           key: Data,
                                           sessionID: Data,
                                           direction: ConnectDirection) throws -> Data {
        let envelope = try encodeControlClose(sequence: sequence,
                                              role: role,
                                              code: code,
                                              reason: reason,
                                              retryable: retryable)
        return try encryptEnvelope(envelope, key: key, sessionID: sessionID, direction: direction)
    }

    private static func mapBridgeFailure() -> ConnectEnvelopeCodecError {
        if !NoritoNativeBridge.shared.isConnectCryptoAvailable {
            return .bridgeUnavailable
        }
        return .encodeFailed
    }

    private static func mapEncryptionFailure() -> ConnectEnvelopeCodecError {
        if !NoritoNativeBridge.shared.isConnectCryptoAvailable {
            return .bridgeUnavailable
        }
        return .encryptFailed
    }
}
