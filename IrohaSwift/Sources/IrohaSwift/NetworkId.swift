import Foundation

/// Failure to construct an exact genesis-derived network identity.
public enum NetworkIdError: Error, Equatable, LocalizedError, Sendable {
    /// The supplied value was not the canonical checksummed Iroha hash form.
    case invalidCanonicalLiteral
    /// The supplied raw identity did not contain exactly 32 marked hash bytes.
    case invalidRawBytes

    public var errorDescription: String? {
        switch self {
        case .invalidCanonicalLiteral:
            return "NetworkId must be an exact canonical checksummed 32-byte Iroha hash literal."
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

    /// Canonical checksummed `hash:<64 uppercase hex>#<CRC16>` representation.
    public let literal: String

    private let storage: Data

    /// Construct from one exact canonical checksummed hash literal.
    public init(literal: String) throws {
        let utf8 = Array(literal.utf8)
        guard utf8.count == 74,
              Array(utf8[0..<5]) == Array("hash:".utf8),
              utf8[69] == 0x23,
              utf8[5..<69].allSatisfy(Self.isUpperHex),
              utf8[70..<74].allSatisfy(Self.isUpperHex),
              let suppliedChecksum = UInt16(String(decoding: utf8[70..<74], as: UTF8.self), radix: 16),
              suppliedChecksum == Self.crc16(utf8[0..<69]),
              let bytes = Self.decodeHex(utf8[5..<69]),
              bytes.count == Self.byteCount,
              bytes[Self.byteCount - 1] & 1 == 1 else {
            throw NetworkIdError.invalidCanonicalLiteral
        }
        self.literal = literal
        self.storage = Data(bytes)
    }

    /// Construct from the exact raw genesis-header hash bytes.
    public init(bytes: Data) throws {
        guard bytes.count == Self.byteCount,
              bytes[Self.byteCount - 1] & 1 == 1 else {
            throw NetworkIdError.invalidRawBytes
        }
        let body = bytes.map { String(format: "%02X", $0) }.joined()
        let prefix = "hash:\(body)"
        let checksum = Self.crc16(prefix.utf8)
        self.literal = "\(prefix)#\(String(format: "%04X", checksum))"
        self.storage = bytes
    }

    /// Defensive copy of the exact 32-byte genesis-header hash.
    public var bytes: Data { Data(storage) }

    public var description: String { literal }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        do {
            try self.init(literal: container.decode(String.self))
        } catch {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "NetworkId must be an exact canonical checksummed Iroha hash literal."
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(literal)
    }

    private static func isUpperHex(_ byte: UInt8) -> Bool {
        (byte >= 0x30 && byte <= 0x39) || (byte >= 0x41 && byte <= 0x46)
    }

    private static func hexNibble(_ byte: UInt8) -> UInt8? {
        switch byte {
        case 0x30...0x39: return byte - 0x30
        case 0x41...0x46: return byte - 0x41 + 10
        default: return nil
        }
    }

    private static func decodeHex(_ body: ArraySlice<UInt8>) -> [UInt8]? {
        guard body.count == byteCount * 2 else { return nil }
        let input = Array(body)
        var output = [UInt8]()
        output.reserveCapacity(byteCount)
        for index in stride(from: 0, to: input.count, by: 2) {
            guard let high = hexNibble(input[index]),
                  let low = hexNibble(input[index + 1]) else { return nil }
            output.append((high << 4) | low)
        }
        return output
    }

    private static func crc16<S: Sequence>(_ bytes: S) -> UInt16 where S.Element == UInt8 {
        var crc: UInt16 = 0xFFFF
        for byte in bytes {
            crc ^= UInt16(byte) << 8
            for _ in 0..<8 {
                crc = (crc & 0x8000) != 0 ? (crc << 1) ^ 0x1021 : crc << 1
            }
        }
        return crc
    }
}
