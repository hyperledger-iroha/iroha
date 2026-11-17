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

    // MARK: - Layout-directed reads (`norito.md`; see `NoritoLayoutWriter`)

    /// Per-value length prefix of a field, sequence element, string or blob:
    /// a `u64` in the fixed layout, a shortest-form varint otherwise.
    mutating func readLength(layout: NoritoWireLayout) throws -> UInt64 {
        switch layout {
        case .fixed:
            return try readUInt64LE()
        case .compact, .dense:
            return try readVarint()
        }
    }

    /// Element count of a sequence, map or set: a `u64` in the current layouts,
    /// a shortest-form varint in the dense layout.
    mutating func readCount(layout: NoritoWireLayout) throws -> UInt64 {
        switch layout {
        case .fixed, .compact:
            return try readUInt64LE()
        case .dense:
            return try readVarint()
        }
    }

    /// Enum discriminant: a `u32` in the current layouts, its shortest-form
    /// varint in the dense layout.
    mutating func readEnumTag(layout: NoritoWireLayout) throws -> UInt32 {
        switch layout {
        case .fixed, .compact:
            return try readUInt32LE()
        case .dense:
            let tag = try readVarint()
            guard tag <= UInt64(UInt32.max) else {
                throw CanonicalNoritoDecodingError.invalidField("enum discriminant overflow")
            }
            return UInt32(tag)
        }
    }

    /// One length-prefixed value in `layout`.
    mutating func readField(layout: NoritoWireLayout) throws -> Data {
        let length = try readLength(layout: layout)
        guard length <= UInt64(Int.max) else {
            throw CanonicalNoritoDecodingError.invalidField("field length overflow")
        }
        return try readBytes(Int(length))
    }

    /// The `count` elements of a fixed `[u8; N]` or of a `u8` element sequence:
    /// canonical `[len = 1][byte]` runs in the current layouts, raw bytes in the
    /// dense layout.
    mutating func readByteFields(count: Int, layout: NoritoWireLayout) throws -> Data {
        guard count >= 0 else {
            throw CanonicalNoritoDecodingError.truncatedPayload
        }
        if layout == .dense {
            return try readBytes(count)
        }
        // Every framed element takes at least two bytes.
        guard count <= remaining() / 2 else {
            throw CanonicalNoritoDecodingError.truncatedPayload
        }
        var bytes = Data()
        bytes.reserveCapacity(count)
        for _ in 0..<count {
            guard try readLength(layout: layout) == 1 else {
                throw CanonicalNoritoDecodingError.invalidField("byte element is not canonical")
            }
            bytes.append(try readUInt8())
        }
        return bytes
    }

    /// `Vec<u8>`: `[count][raw bytes]` in every layout.
    mutating func readBytesVec(layout: NoritoWireLayout) throws -> Data {
        let count = try readCount(layout: layout)
        guard count <= UInt64(remaining()) else {
            throw CanonicalNoritoDecodingError.truncatedPayload
        }
        return try readBytes(Int(count))
    }

    /// `ConstVec<u8>` and every other `u8` element sequence: `[count]` then the
    /// elements as `readByteFields` reads them.
    mutating func readConstVec(layout: NoritoWireLayout) throws -> Data {
        let count = try readCount(layout: layout)
        guard count <= UInt64(remaining()) else {
            throw CanonicalNoritoDecodingError.truncatedPayload
        }
        return try readByteFields(count: Int(count), layout: layout)
    }

    /// Refuse a payload that the decoder did not consume completely.
    func requireFinished() throws {
        guard remaining() == 0 else {
            throw CanonicalNoritoDecodingError.invalidField("trailing bytes")
        }
    }
}

/// Exact decoders for the shapes `NoritoLayoutCodec` encodes. Each one reads a
/// complete payload in the named layout and refuses truncated input, trailing
/// bytes and non-canonical lengths; none tries another layout.
extension NoritoLayoutCodec {
    /// `String`: `[len][UTF-8 bytes]`.
    static func decodeString(_ data: Data, layout: NoritoWireLayout) throws -> String {
        var reader = CanonicalNoritoReader(data: data)
        let length = try reader.readLength(layout: layout)
        guard length <= UInt64(Int.max) else {
            throw CanonicalNoritoDecodingError.invalidField("string length overflow")
        }
        let bytes = try reader.readBytes(Int(length))
        try reader.requireFinished()
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw CanonicalNoritoDecodingError.invalidField("invalid UTF-8 in string")
        }
        return value
    }

    /// `Option<T>`: a one-byte tag, then the length-prefixed value when present.
    static func decodeOption<T>(
        _ data: Data,
        layout: NoritoWireLayout,
        decode: (Data) throws -> T
    ) throws -> T? {
        var reader = CanonicalNoritoReader(data: data)
        switch try reader.readUInt8() {
        case 0:
            try reader.requireFinished()
            return nil
        case 1:
            let value = try decode(try reader.readField(layout: layout))
            try reader.requireFinished()
            return value
        default:
            throw CanonicalNoritoDecodingError.invalidField("invalid option tag")
        }
    }

    /// A generic element sequence: `[count]` then every element length-prefixed.
    static func decodeVec<T>(
        _ data: Data,
        layout: NoritoWireLayout,
        decode: (Data) throws -> T
    ) throws -> [T] {
        var reader = CanonicalNoritoReader(data: data)
        let count = try reader.readCount(layout: layout)
        // Every element carries at least its own length prefix.
        guard count <= UInt64(reader.remaining()) else {
            throw CanonicalNoritoDecodingError.truncatedPayload
        }
        var values: [T] = []
        values.reserveCapacity(Int(count))
        for _ in 0..<count {
            values.append(try decode(try reader.readField(layout: layout)))
        }
        try reader.requireFinished()
        return values
    }

    /// `Vec<u8>`: `[count][raw bytes]` in every layout.
    static func decodeBytesVec(_ data: Data, layout: NoritoWireLayout) throws -> Data {
        var reader = CanonicalNoritoReader(data: data)
        let bytes = try reader.readBytesVec(layout: layout)
        try reader.requireFinished()
        return bytes
    }

    /// `ConstVec<u8>` and every other `u8` element sequence.
    static func decodeConstVec(_ data: Data, layout: NoritoWireLayout) throws -> Data {
        var reader = CanonicalNoritoReader(data: data)
        let bytes = try reader.readConstVec(layout: layout)
        try reader.requireFinished()
        return bytes
    }

    /// A fixed `[u8; N]` outside a record field: exactly `count` elements.
    static func decodeByteArray(
        _ data: Data,
        count: Int,
        layout: NoritoWireLayout
    ) throws -> Data {
        var reader = CanonicalNoritoReader(data: data)
        let bytes = try reader.readByteFields(count: count, layout: layout)
        try reader.requireFinished()
        return bytes
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
        try NoritoLayoutCodec.decodeString(data, layout: .fixed)
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
