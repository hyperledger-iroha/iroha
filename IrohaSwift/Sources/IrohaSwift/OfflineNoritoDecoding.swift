import Foundation

enum OfflineNoritoDecodingError: Error, LocalizedError, Sendable {
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

struct OfflineNoritoReader {
    private let data: Data
    private(set) var offset: Int = 0

    init(data: Data) {
        self.data = data
    }

    mutating func readUInt8() throws -> UInt8 {
        guard offset < data.count else {
            throw OfflineNoritoDecodingError.truncatedPayload
        }
        let value = data[data.startIndex + offset]
        offset += 1
        return value
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
        guard offset + count <= data.count else {
            throw OfflineNoritoDecodingError.truncatedPayload
        }
        let start = data.startIndex + offset
        let result = Data(data[start..<(start + count)])
        offset += count
        return result
    }

    mutating func readField() throws -> Data {
        let length = try readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("field length overflow")
        }
        return try readBytes(Int(length))
    }
}

struct ParsedPublicAssetLiteral {
    let assetDefinitionId: String
    let accountId: String
    let dataspaceId: UInt64?
}

extension OfflineNorito {
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
            scope.lowercased().hasPrefix("dataspace:"),
            !rawDataspace.isEmpty,
            let parsedDataspaceId = UInt64(rawDataspace) else {
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

    static func decodeString(_ data: Data) throws -> String {
        var reader = OfflineNoritoReader(data: data)
        let length = try reader.readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("string length overflow")
        }
        let bytes = try reader.readBytes(Int(length))
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw OfflineNoritoDecodingError.invalidField("invalid UTF-8 in string")
        }
        return value
    }

    /// Decode an AccountId string from a Norito-encoded string field.
    public static func decodeAccountId(_ data: Data) throws -> String {
        return try decodeString(data)
    }

    public static func assetDefinitionIdFromLiteral(_ literal: String) -> String? {
        if let parsed = parsePublicAssetIdLiteral(literal) {
            return parsed.assetDefinitionId
        }
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            return nil
        }
        guard !trimmed.contains("#"),
              AssetDefinitionAddress.decode(trimmed) != nil else {
            return nil
        }
        return trimmed
    }

    static func accountIdFromLiteral(_ literal: String) -> String? {
        parsePublicAssetIdLiteral(literal)?.accountId
    }
}
