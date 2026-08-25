import Foundation

/// Failure to construct an exact genesis-derived network identity.
public enum NetworkIdError: Error, Equatable, LocalizedError, Sendable {
    /// The supplied value was not exact lowercase marked 32-byte hash text.
    case invalidCanonicalLiteral
    /// The supplied raw identity did not contain exactly 32 marked hash bytes.
    case invalidRawBytes

    public var errorDescription: String? {
        switch self {
        case .invalidCanonicalLiteral:
            return "NetworkId must be an exact lowercase marked 32-byte Iroha hash literal."
        case .invalidRawBytes:
            return "NetworkId must contain exactly 32 genesis-hash bytes with the Iroha hash marker bit set."
        }
    }
}

/// Exact immutable identity of one Iroha network.
///
/// `NetworkId` is derived from the consensus hash of the genesis header. It is
/// deliberately distinct from a human-readable chain name, which is never a
/// signing or replay-protection domain.
public struct NetworkId: Equatable, Hashable, Sendable, Codable, CustomStringConvertible {
    /// Exact byte width of the genesis-header hash.
    public static let byteCount = 32

    /// Canonical raw 64-character lowercase hexadecimal representation.
    public let literal: String

    private let storage: Data

    /// Construct from one exact lowercase marked 32-byte hash literal.
    public init(literal: String) throws {
        guard let bytes = CanonicalIrohaHashText.decode(literal) else {
            throw NetworkIdError.invalidCanonicalLiteral
        }
        self.literal = literal
        self.storage = bytes
    }

    /// Construct from the exact raw genesis-header hash bytes.
    public init(bytes: Data) throws {
        guard bytes.count == Self.byteCount,
              bytes[Self.byteCount - 1] & 1 == 1 else {
            throw NetworkIdError.invalidRawBytes
        }
        guard let literal = CanonicalIrohaHashText.encode(bytes) else {
            throw NetworkIdError.invalidRawBytes
        }
        self.literal = literal
        self.storage = bytes
    }

    /// Defensive copy of the exact 32-byte genesis-header hash.
    public var bytes: Data { Data(storage) }

    public var description: String { literal }

    /// Tagged checksummed form required only by the Norito JSON codec.
    var noritoJSONLiteral: String {
        guard let literal = CanonicalIrohaHashJSONLiteral.encode(storage) else {
            preconditionFailure("a validated NetworkId must have a Norito JSON literal")
        }
        return literal
    }

    init(noritoJSONLiteral: String) throws {
        guard let bytes = CanonicalIrohaHashJSONLiteral.decode(noritoJSONLiteral) else {
            throw NetworkIdError.invalidCanonicalLiteral
        }
        try self.init(bytes: bytes)
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        do {
            try self.init(noritoJSONLiteral: container.decode(String.self))
        } catch {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "NetworkId JSON must be an exact checksummed Iroha hash literal."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(noritoJSONLiteral)
    }
}
