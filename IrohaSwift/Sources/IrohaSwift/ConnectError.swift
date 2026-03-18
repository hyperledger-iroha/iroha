import Foundation

/// Shared Connect error categories surfaced across SDKs.
public enum ConnectErrorCategory: String, Codable, Equatable, Sendable {
    case transport
    case codec
    case authorization
    case timeout
    case queueOverflow
    case internalError = "internal"
}

/// Canonical Connect error wrapper exported by SDK APIs and telemetry hooks.
public struct ConnectError: Error, LocalizedError, Sendable {
    public let category: ConnectErrorCategory
    public let code: String
    public let message: String
    public let underlying: Error?

    public init(category: ConnectErrorCategory,
                code: String,
                message: String,
                underlying: Error? = nil) {
        self.category = category
        self.code = code
        self.message = message.isEmpty ? code : message
        self.underlying = underlying
    }

    public var errorDescription: String? { message }

    /// Convenience method for telemetry exporters that need structured attributes.
    public func telemetryAttributes(fatal: Bool = false, httpStatus: Int? = nil) -> [String: String] {
        var attributes: [String: String] = [
            "category": category.rawValue,
            "code": code,
            "fatal": fatal ? "true" : "false"
        ]
        if let httpStatus {
            attributes["http_status"] = String(httpStatus)
        }
        if let underlying {
            attributes["underlying"] = String(describing: underlying)
        }
        return attributes
    }
}

public protocol ConnectErrorConvertible: Error {
    var connectError: ConnectError { get }
}

extension ConnectError: ConnectErrorConvertible {
    public var connectError: ConnectError { self }
}

public extension Error {
    /// Maps any `Error` into the shared `ConnectError` taxonomy.
    func asConnectError() -> ConnectError {
        if let convertible = self as? ConnectErrorConvertible {
            return convertible.connectError
        }
        let description: String
        if let localized = (self as? LocalizedError)?.errorDescription, !localized.isEmpty {
            description = localized
        } else {
            description = String(describing: self)
        }
        return ConnectError(category: .internalError,
                            code: "unknown_error",
                            message: description,
                            underlying: self)
    }
}

// MARK: - SDK-specific conformances

extension ConnectClient.ClientError: ConnectErrorConvertible {
    public var connectError: ConnectError {
        switch self {
        case .alreadyStarted:
            return ConnectError(category: .internalError,
                                code: "client.already_started",
                                message: errorDescription ?? "",
                                underlying: self)
        case .closed:
            return ConnectError(category: .transport,
                                code: "client.closed",
                                message: errorDescription ?? "",
                                underlying: self)
        case .unknownPayload:
            return ConnectError(category: .codec,
                                code: "client.unknown_payload",
                                message: errorDescription ?? "",
                                underlying: self)
        }
    }
}

extension ConnectSessionError: ConnectErrorConvertible {
    public var connectError: ConnectError {
        switch self {
        case .streamEnded:
            return ConnectError(category: .transport,
                                code: "session.stream_ended",
                                message: errorDescription ?? "",
                                underlying: self)
        case .sessionClosed:
            return ConnectError(category: .internalError,
                                code: "session.closed",
                                message: errorDescription ?? "",
                                underlying: self)
        case .missingDecryptionKeys:
            return ConnectError(category: .internalError,
                                code: "session.missing_direction_keys",
                                message: errorDescription ?? "",
                                underlying: self)
        case .flowControlExceeded(let direction):
            return ConnectError(category: .queueOverflow,
                                code: "session.flow_control_exceeded",
                                message: "Flow control window exhausted for direction \(direction.rawValue)",
                                underlying: self)
        case .clientError(let error):
            return error.connectError
        case .envelopeError(let error):
            return error.connectError
        case .unknown(let description):
            return ConnectError(category: .internalError,
                                code: "session.unknown",
                                message: description,
                                underlying: self)
        }
    }
}

extension ConnectEnvelopeError: ConnectErrorConvertible {
    public var connectError: ConnectError {
        let message = errorDescription ?? ""
        switch self {
        case .bridgeUnavailable:
            return ConnectError(category: .codec,
                                code: "envelope.bridge_unavailable",
                                message: message,
                                underlying: self)
        case .unsupportedFrameKind:
            return ConnectError(category: .codec,
                                code: "envelope.unsupported_frame",
                                message: message,
                                underlying: self)
        case .invalidEnvelope:
            return ConnectError(category: .codec,
                                code: "envelope.invalid_envelope",
                                message: message,
                                underlying: self)
        case .invalidPayload:
            return ConnectError(category: .codec,
                                code: "envelope.invalid_payload",
                                message: message,
                                underlying: self)
        case .invalidBase64(let field):
            return ConnectError(category: .codec,
                                code: "envelope.invalid_base64",
                                message: "Invalid base64 field '\(field)'.",
                                underlying: self)
        case .unknownPayloadKind(let kind):
            return ConnectError(category: .codec,
                                code: "envelope.unknown_payload_kind",
                                message: "Unknown payload kind '\(kind)'.",
                                underlying: self)
        }
    }
}

extension ConnectEnvelopeError {
    /// Convenience helper for telemetry pipelines.
    public var telemetryCode: String { connectError.code }
}

extension ConnectCodecError: ConnectErrorConvertible {
    public var connectError: ConnectError {
        let message = errorDescription ?? ""
        switch self {
        case .bridgeUnavailable:
            return ConnectError(category: .codec,
                                code: "codec.bridge_unavailable",
                                message: message,
                                underlying: self)
        case .encodeFailed:
            return ConnectError(category: .codec,
                                code: "codec.encode_failed",
                                message: message,
                                underlying: self)
        case .decodeFailed:
            return ConnectError(category: .codec,
                                code: "codec.decode_failed",
                                message: message,
                                underlying: self)
        }
    }
}

extension ConnectCryptoError: ConnectErrorConvertible {
    public var connectError: ConnectError {
        switch self {
        case .bridgeUnavailable:
            return ConnectError(category: .internalError,
                                code: "crypto.bridge_unavailable",
                                message: errorDescription ?? "",
                                underlying: self)
        case .invalidPrivateKeyLength:
            return ConnectError(category: .internalError,
                                code: "crypto.invalid_private_key_length",
                                message: errorDescription ?? "",
                                underlying: self)
        case .invalidPublicKeyLength:
            return ConnectError(category: .internalError,
                                code: "crypto.invalid_public_key_length",
                                message: errorDescription ?? "",
                                underlying: self)
        case .invalidSessionIdentifierLength:
            return ConnectError(category: .internalError,
                                code: "crypto.invalid_session_identifier_length",
                                message: errorDescription ?? "",
                                underlying: self)
        }
    }
}

/// Errors surfaced by the (upcoming) offline queue manager.
public enum ConnectQueueError: Error, LocalizedError, Sendable {
    case invalidCount(Int)
    case overflow(limit: Int)
    case expired
    case corrupted
    case concurrentAccess
    case hashUnavailable

    public var errorDescription: String? {
        switch self {
        case let .invalidCount(count):
            return "Connect queue pop count must be positive (got \(count))."
        case .overflow(let limit):
            return "Connect queue exceeded its limit (\(limit) frames)."
        case .expired:
            return "Connect queue expired because the session was offline for too long."
        case .corrupted:
            return "Connect queue journal is corrupted and cannot be recovered."
        case .concurrentAccess:
            return "Connect queue is already being processed by another component."
        case .hashUnavailable:
            return NoritoNativeBridge.bridgeUnavailableMessage(
                "Connect queue hashing prerequisites are unavailable."
            )
        }
    }
}

extension ConnectQueueError: ConnectErrorConvertible {
    public var connectError: ConnectError {
        switch self {
        case .invalidCount:
            return ConnectError(category: .internalError,
                                code: "queue.invalid_count",
                                message: errorDescription ?? "",
                                underlying: self)
        case .overflow:
            return ConnectError(category: .queueOverflow,
                                code: "queue.overflow",
                                message: errorDescription ?? "",
                                underlying: self)
        case .expired:
            return ConnectError(category: .timeout,
                                code: "queue.expired",
                                message: errorDescription ?? "",
                                underlying: self)
        case .corrupted:
            return ConnectError(category: .internalError,
                                code: "queue.corrupted",
                                message: errorDescription ?? "",
                                underlying: self)
        case .concurrentAccess:
            return ConnectError(category: .internalError,
                                code: "queue.concurrent_access",
                                message: errorDescription ?? "",
                                underlying: self)
        case .hashUnavailable:
            return ConnectError(category: .internalError,
                                code: "queue.hash_unavailable",
                                message: errorDescription ?? "",
                                underlying: self)
        }
    }
}

// MARK: - Foundation conformances

extension URLError: ConnectErrorConvertible {
    public var connectError: ConnectError {
        let message = localizedDescription
        switch code {
        case .timedOut:
            return ConnectError(category: .timeout,
                                code: "network.timeout",
                                message: message,
                                underlying: self)
        case .cancelled:
            return ConnectError(category: .transport,
                                code: "network.cancelled",
                                message: message,
                                underlying: self)
        case .cannotConnectToHost, .networkConnectionLost, .dnsLookupFailed,
             .notConnectedToInternet, .cannotFindHost, .internationalRoamingOff,
             .callIsActive, .dataNotAllowed:
            return ConnectError(category: .transport,
                                code: "network.unreachable",
                                message: message,
                                underlying: self)
        case .appTransportSecurityRequiresSecureConnection,
             .serverCertificateHasBadDate,
             .serverCertificateUntrusted,
             .serverCertificateHasUnknownRoot,
             .serverCertificateNotYetValid,
             .clientCertificateRejected,
             .clientCertificateRequired,
             .secureConnectionFailed:
            return ConnectError(category: .authorization,
                                code: "network.tls_failure",
                                message: message,
                                underlying: self)
        default:
            return ConnectError(category: .transport,
                                code: "network.failure",
                                message: message,
                                underlying: self)
        }
    }
}

extension DecodingError: ConnectErrorConvertible {
    public var connectError: ConnectError {
        let description: String
        switch self {
        case .typeMismatch(_, let context),
             .valueNotFound(_, let context),
             .keyNotFound(_, let context),
             .dataCorrupted(let context):
            description = context.debugDescription
        @unknown default:
            description = "Unknown decoding error"
        }
        return ConnectError(category: .codec,
                            code: "codec.decoding_failed",
                            message: description,
                            underlying: self)
    }
}

extension EncodingError: ConnectErrorConvertible {
    public var connectError: ConnectError {
        let description: String
        switch self {
        case .invalidValue(_, let context):
            description = context.debugDescription
        @unknown default:
            description = "Unknown encoding error"
        }
        return ConnectError(category: .codec,
                            code: "codec.encoding_failed",
                            message: description,
                            underlying: self)
    }
}
