import Foundation

enum CanonicalNoritoDecodingError: Error, LocalizedError, Sendable {
    case truncatedPayload
    case invalidField(String)

    var errorDescription: String? {
        switch self {
        case .truncatedPayload:
            return "Norito payload ended unexpectedly."
        case .invalidField(let reason):
            return "Invalid Norito field: \(reason)"
        }
    }
}

struct CanonicalNoritoReader {
    private let data: Data
    private(set) var offset: Int = 0

    init(data: Data) {
        self.data = data
    }

    mutating func readUInt8() throws -> UInt8 {
        guard offset < data.count else {
            throw CanonicalNoritoDecodingError.truncatedPayload
        }
        let value = data[data.startIndex + offset]
        offset += 1
        return value
    }

    mutating func readUInt16LE() throws -> UInt16 {
        let bytes = try readBytes(2)
        var value: UInt16 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 2)
        }
        return UInt16(littleEndian: value)
    }

    mutating func readUInt32LE() throws -> UInt32 {
        let bytes = try readBytes(4)
        var value: UInt32 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 4)
        }
        return UInt32(littleEndian: value)
    }

    mutating func readUInt64LE() throws -> UInt64 {
        let bytes = try readBytes(8)
        var value: UInt64 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 8)
        }
        return UInt64(littleEndian: value)
    }

    mutating func readBytes(_ count: Int) throws -> Data {
        guard count >= 0, offset <= data.count - count else {
            throw CanonicalNoritoDecodingError.truncatedPayload
        }
        let start = data.startIndex + offset
        let result = Data(data[start..<(start + count)])
        offset += count
        return result
    }

    mutating func readVarint() throws -> UInt64 {
        var shift: UInt64 = 0
        var value: UInt64 = 0
        var byteCount = 0
        while true {
            let byte = try readUInt8()
            byteCount += 1
            guard byteCount <= 10, shift < 64 else {
                throw CanonicalNoritoDecodingError.invalidField("varint length overflow")
            }
            if shift == 63, (byte & 0x7e) != 0 {
                throw CanonicalNoritoDecodingError.invalidField("varint value overflow")
            }
            value |= UInt64(byte & 0x7f) << shift
            if (byte & 0x80) == 0 {
                if byteCount > 1, byte == 0 {
                    throw CanonicalNoritoDecodingError.invalidField("non-canonical varint")
                }
                return value
            }
            shift += 7
        }
    }

    mutating func readField() throws -> Data {
        let length = try readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw CanonicalNoritoDecodingError.invalidField("field length overflow")
        }
        return try readBytes(Int(length))
    }

    mutating func readCompactField() throws -> Data {
        let length = try readVarint()
        guard length <= UInt64(Int.max) else {
            throw CanonicalNoritoDecodingError.invalidField("field length overflow")
        }
        return try readBytes(Int(length))
    }

    func remaining() -> Int {
        data.count - offset
    }
}

struct ParsedPublicAssetLiteral {
    let assetDefinitionId: String
    let accountId: String
    let dataspaceId: UInt64?
}

extension CanonicalNorito {
    static func parsePublicAssetIdLiteral(_ literal: String) -> ParsedPublicAssetLiteral? {
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              trimmed == literal,
              !trimmed.contains(where: \.isWhitespace) else {
            return nil
        }
        let components = trimmed.split(separator: "#", omittingEmptySubsequences: false)
        guard (components.count == 2 || components.count == 3),
              !components[0].isEmpty,
              !components[1].isEmpty else {
            return nil
        }
        let assetDefinitionId = String(components[0])
        guard AssetDefinitionAddress.decode(assetDefinitionId) != nil else {
            return nil
        }
        let accountId = String(components[1])
        guard (try? AccountAddress.parseEncoded(accountId)) != nil else {
            return nil
        }
        var dataspaceId: UInt64?
        if components.count == 3 {
            let scope = String(components[2])
            guard let rawDataspace = scope.split(
                separator: ":",
                maxSplits: 1,
                omittingEmptySubsequences: false
            ).dropFirst().first,
            scope.hasPrefix("dataspace:"),
            let parsedDataspaceId = parseCanonicalDataspaceId(rawDataspace) else {
                return nil
            }
            dataspaceId = parsedDataspaceId
        }
        return ParsedPublicAssetLiteral(
            assetDefinitionId: assetDefinitionId,
            accountId: accountId,
            dataspaceId: dataspaceId
        )
    }

    private static func parseCanonicalDataspaceId(_ raw: Substring) -> UInt64? {
        let text = String(raw)
        guard !text.isEmpty,
              (text == "0" || !text.hasPrefix("0")),
              text.unicodeScalars.allSatisfy({ scalar in
                  scalar.value >= 48 && scalar.value <= 57
              }) else {
            return nil
        }
        return UInt64(text)
    }

    static func decodeString(_ data: Data) throws -> String {
        var reader = CanonicalNoritoReader(data: data)
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw CanonicalNoritoDecodingError.invalidField("string length overflow")
        }
        let bytes = try reader.readBytes(Int(length))
        guard reader.remaining() == 0 else {
            throw CanonicalNoritoDecodingError.invalidField("trailing bytes")
        }
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw CanonicalNoritoDecodingError.invalidField("invalid UTF-8 in string")
        }
        return value
    }

    /// Decode an AccountId string from a Norito-encoded string field.
    public static func decodeAccountId(_ data: Data) throws -> String {
        try decodeString(data)
    }

    public static func assetDefinitionIdFromLiteral(_ literal: String) -> String? {
        if let parsed = parsePublicAssetIdLiteral(literal) {
            return parsed.assetDefinitionId
        }
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty,
              !trimmed.contains("#"),
              AssetDefinitionAddress.decode(trimmed) != nil else {
            return nil
        }
        return trimmed
    }

    static func accountIdFromLiteral(_ literal: String) -> String? {
        parsePublicAssetIdLiteral(literal)?.accountId
    }
}
