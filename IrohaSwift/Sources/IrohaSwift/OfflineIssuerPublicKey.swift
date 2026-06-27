import Foundation

public enum OfflineIssuerPublicKeyError: Error, Equatable, LocalizedError, Sendable {
    case missing
    case surroundingWhitespace
    case invalidBase64
    case invalidLength(expected: Int, actual: Int)

    public var errorDescription: String? {
        switch self {
        case .missing:
            return "Offline issuer public key is missing."
        case .surroundingWhitespace:
            return "Offline issuer public key must not contain surrounding whitespace."
        case .invalidBase64:
            return "Offline issuer public key is not valid base64."
        case let .invalidLength(expected, actual):
            return "Offline issuer public key must be \(expected) bytes, got \(actual)."
        }
    }
}

public struct OfflineIssuerPublicKey: Equatable, Sendable {
    public static let rawEd25519ByteCount = 32

    public let rawRepresentation: Data
    public let encoded: String

    public init(_ value: String) throws {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineIssuerPublicKeyError.missing
        }
        guard trimmed == value else {
            throw OfflineIssuerPublicKeyError.surroundingWhitespace
        }
        guard !value.contains("=") else {
            throw OfflineIssuerPublicKeyError.invalidBase64
        }
        guard value.unicodeScalars.allSatisfy({ $0.value > 0x20 && $0.value <= 0x7E }),
              let rawRepresentation = Self.decodeBase64OrBase64URL(value) else {
            throw OfflineIssuerPublicKeyError.invalidBase64
        }
        guard rawRepresentation.count == Self.rawEd25519ByteCount else {
            throw OfflineIssuerPublicKeyError.invalidLength(
                expected: Self.rawEd25519ByteCount,
                actual: rawRepresentation.count
            )
        }
        self.rawRepresentation = rawRepresentation
        self.encoded = value
    }

    public static func isValid(_ value: String?) -> Bool {
        guard let value else { return false }
        return (try? Self(value)) != nil
    }

    public static func sanitized(_ value: String?) -> String? {
        guard let value,
              let key = try? Self(value) else {
            return nil
        }
        return key.encoded
    }

    private static func decodeBase64OrBase64URL(_ value: String) -> Data? {
        if let data = Data(base64Encoded: value) {
            return data
        }
        var base64 = value
            .replacingOccurrences(of: "-", with: "+")
            .replacingOccurrences(of: "_", with: "/")
        let remainder = base64.count % 4
        if remainder > 0 {
            base64.append(String(repeating: "=", count: 4 - remainder))
        }
        return Data(base64Encoded: base64)
    }
}
