import Foundation

public enum ConfidentialEncryptedPayloadError: Error, Sendable, Equatable {
    case unsupportedVersion(UInt8)
    case invalidEphemeralKeyLength(Int)
    case invalidNonceLength(Int)
    case ciphertextTooLarge
    case truncatedPayload
    case varintOverflow
    case trailingBytes(Int)
}

extension ConfidentialEncryptedPayloadError: LocalizedError {
    public var errorDescription: String? {
        switch self {
        case let .unsupportedVersion(version):
            return "Unsupported encrypted payload version: \(version)."
        case let .invalidEphemeralKeyLength(length):
            return "Ephemeral public key must be 32 bytes; got \(length)."
        case let .invalidNonceLength(length):
            return "Nonce must be 24 bytes; got \(length)."
        case .ciphertextTooLarge:
            return "Ciphertext length exceeds supported range."
        case .truncatedPayload:
            return "Encrypted payload bytes ended unexpectedly."
        case .varintOverflow:
            return "Length varint overflow while decoding encrypted payload."
        case let .trailingBytes(count):
            return "Encrypted payload contains \(count) trailing bytes."
        }
    }
}

public struct ConfidentialEncryptedPayload: Equatable, Sendable {
    public static let v1: UInt8 = 1
    private static let typeName = "iroha_data_model::confidential::ConfidentialEncryptedPayload"

    public let version: UInt8
    public let ephemeralPublicKey: Data
    public let nonce: Data
    public let ciphertext: Data

    public init(version: UInt8 = ConfidentialEncryptedPayload.v1,
                ephemeralPublicKey: Data,
                nonce: Data,
                ciphertext: Data) throws {
        guard ephemeralPublicKey.count == 32 else {
            throw ConfidentialEncryptedPayloadError.invalidEphemeralKeyLength(ephemeralPublicKey.count)
        }
        guard nonce.count == 24 else {
            throw ConfidentialEncryptedPayloadError.invalidNonceLength(nonce.count)
        }
        guard version == ConfidentialEncryptedPayload.v1 else {
            throw ConfidentialEncryptedPayloadError.unsupportedVersion(version)
        }
        self.version = version
        self.ephemeralPublicKey = ephemeralPublicKey
        self.nonce = nonce
        self.ciphertext = ciphertext
    }

    public func serializedPayload() throws -> Data {
        guard ciphertext.count <= Int(UInt32.max) else {
            throw ConfidentialEncryptedPayloadError.ciphertextTooLarge
        }
        if let native = NoritoNativeBridge.shared.encodeConfidentialPayload(ephemeralPublicKey: ephemeralPublicKey,
                                                                             nonce: nonce,
                                                                             ciphertext: ciphertext) {
            return native
        }
        return ConfidentialEncryptedPayload.encodeFallback(version: version,
                                                           ephemeralPublicKey: ephemeralPublicKey,
                                                           nonce: nonce,
                                                           ciphertext: ciphertext)
    }

    public func noritoEnvelope(flags: UInt8 = 0x04) throws -> Data {
        let payload = try serializedPayload()
        return noritoEncode(typeName: Self.typeName, payload: payload, flags: flags)
    }

    private static func encodeFallback(version: UInt8,
                                       ephemeralPublicKey: Data,
                                       nonce: Data,
                                       ciphertext: Data) -> Data {
        var out = Data(capacity: 1 + ephemeralPublicKey.count + nonce.count + ciphertext.count + 10)
        out.append(version)
        out.append(ephemeralPublicKey)
        out.append(nonce)
        out.append(encodeVarint(UInt64(ciphertext.count)))
        out.append(ciphertext)
        return out
    }

    private static func encodeVarint(_ value: UInt64) -> Data {
        var remaining = value
        var encoded = Data()
        repeat {
            var byte = UInt8(remaining & 0x7F)
            remaining >>= 7
            if remaining != 0 {
                byte |= 0x80
            }
            encoded.append(byte)
        } while remaining != 0
        return encoded
    }

    public func asHexDictionary() -> [String: String] {
        [
            "version": String(format: "%02x", version),
            "ephemeral_pubkey": ephemeralPublicKey.map { String(format: "%02x", $0) }.joined(),
            "nonce": nonce.map { String(format: "%02x", $0) }.joined(),
            "ciphertext": ciphertext.map { String(format: "%02x", $0) }.joined(),
        ]
    }

    public static func deserialize(from payload: Data) throws -> ConfidentialEncryptedPayload {
        var reader = NoritoReader(bytes: payload)
        let version = try reader.readU8()
        let ephemeral = try reader.readFixed(count: 32)
        let nonce = try reader.readFixed(count: 24)
        let length = try reader.readVarint()
        guard length <= UInt64(Int.max) else {
            throw ConfidentialEncryptedPayloadError.ciphertextTooLarge
        }
        let ciphertext = try reader.readFixed(count: Int(length))
        let remaining = reader.remainingCount
        guard remaining == 0 else {
            throw ConfidentialEncryptedPayloadError.trailingBytes(remaining)
        }
        return try ConfidentialEncryptedPayload(version: version,
                                                ephemeralPublicKey: ephemeral,
                                                nonce: nonce,
                                                ciphertext: ciphertext)
    }
}

private struct NoritoReader {
    private let bytes: Data
    private(set) var index: Int = 0

    init(bytes: Data) { self.bytes = bytes }

    var remainingCount: Int { bytes.count - index }

    mutating func readU8() throws -> UInt8 {
        guard index < bytes.count else {
            throw ConfidentialEncryptedPayloadError.truncatedPayload
        }
        let value = bytes[index]
        index += 1
        return value
    }

    mutating func readFixed(count: Int) throws -> Data {
        guard count >= 0 else { return Data() }
        guard index + count <= bytes.count else {
            throw ConfidentialEncryptedPayloadError.truncatedPayload
        }
        let slice = bytes[index..<(index + count)]
        index += count
        return Data(slice)
    }

    mutating func readVarint() throws -> UInt64 {
        var value: UInt64 = 0
        var shift: UInt8 = 0
        while true {
            let byte = try readU8()
            value |= UInt64(byte & 0x7F) << shift
            if (byte & 0x80) == 0 { break }
            shift += 7
            if shift >= 64 {
                throw ConfidentialEncryptedPayloadError.varintOverflow
            }
        }
        return value
    }
}
