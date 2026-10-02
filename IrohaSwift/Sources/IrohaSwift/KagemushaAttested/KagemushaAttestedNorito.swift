import Foundation

/// Failures of the strict attested-suite Norito codec.
enum KagemushaAttestedCodecError: Error, Equatable, Sendable {
    case truncated
    case trailingBytes
    case nonCanonical(String)
    case invalidField(String)
    case wrongSchema
    case tooLarge(actual: Int, maximum: Int)
}

/// Canonical compact Norito writer (`COMPACT_LEN`): every struct field is a varint-length-prefixed
/// frame, sequences carry a fixed `u64` count and length-prefixed elements, `Option` is a tag
/// byte followed by a length-prefixed value, and `[u8; N]` struct fields are raw bytes after
/// their length prefix.
struct KagemushaAttestedWriter {
    private(set) var data = Data()

    init() {}

    mutating func raw(_ bytes: Data) { data.append(bytes) }

    mutating func varint(_ value: UInt64) {
        var remaining = value
        while remaining >= 0x80 {
            data.append(UInt8(remaining & 0x7f) | 0x80)
            remaining >>= 7
        }
        data.append(UInt8(remaining))
    }

    /// One length-prefixed field.
    mutating func field(_ payload: Data) {
        varint(UInt64(payload.count))
        data.append(payload)
    }

    mutating func u8(_ value: UInt8) { field(Data([value])) }
    mutating func u16(_ value: UInt16) { field(.kagemushaLE(value)) }
    mutating func u32(_ value: UInt32) { field(.kagemushaLE(value)) }
    mutating func u64(_ value: UInt64) { field(.kagemushaLE(value)) }
    mutating func bool(_ value: Bool) { field(Data([value ? 1 : 0])) }

    /// A `[u8; N]` struct field.
    mutating func array(_ bytes: Data, _ count: Int) {
        precondition(bytes.count == count, "fixed array length")
        field(bytes)
    }

    /// A `String` struct field.
    mutating func string(_ value: String) {
        field(Self.stringValue(value))
    }

    /// An `Option<T>` struct field whose `Some` payload is `encoded`.
    mutating func option(_ encoded: Data?) {
        field(Self.optionValue(encoded))
    }

    /// A `Vec<T>` struct field whose element payloads are `elements`.
    mutating func sequence(_ elements: [Data]) {
        field(Self.sequenceValue(elements))
    }

    /// A nested struct or enum field.
    mutating func nested(_ payload: Data) { field(payload) }

    /// Bare `String` value: varint length then UTF-8.
    static func stringValue(_ value: String) -> Data {
        var writer = KagemushaAttestedWriter()
        let bytes = Data(value.utf8)
        writer.varint(UInt64(bytes.count))
        writer.raw(bytes)
        return writer.data
    }

    /// Bare `Option<T>` value.
    static func optionValue(_ encoded: Data?) -> Data {
        var writer = KagemushaAttestedWriter()
        if let encoded {
            writer.raw(Data([1]))
            writer.field(encoded)
        } else {
            writer.raw(Data([0]))
        }
        return writer.data
    }

    /// Bare `Vec<T>` value (not `Vec<u8>`).
    static func sequenceValue(_ elements: [Data]) -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(.kagemushaLE(UInt64(elements.count)))
        for element in elements { writer.field(element) }
        return writer.data
    }

    /// Bare `[u8; N]` value outside a derived struct field (for example a `Vec<[u8; 32]>`
    /// element): every byte is its own length-prefixed element.
    static func genericByteArrayValue(_ bytes: Data) -> Data {
        var out = Data(capacity: bytes.count * 2)
        for byte in bytes {
            out.append(1)
            out.append(byte)
        }
        return out
    }

    /// Bare enum value: `u32` discriminant then length-prefixed fields.
    static func enumValue(_ discriminant: UInt32, _ fields: [Data]) -> Data {
        var writer = KagemushaAttestedWriter()
        writer.raw(.kagemushaLE(discriminant))
        for field in fields { writer.field(field) }
        return writer.data
    }
}

/// Strict reader mirroring ``KagemushaAttestedWriter``. Every varint must be minimal and every
/// fixed-width field must have its exact width.
struct KagemushaAttestedReader {
    private let bytes: [UInt8]
    private(set) var offset = 0

    init(_ data: Data) { bytes = [UInt8](data) }

    var isAtEnd: Bool { offset == bytes.count }

    mutating func raw(_ count: Int) throws -> Data {
        guard count >= 0, count <= bytes.count - offset else {
            throw KagemushaAttestedCodecError.truncated
        }
        defer { offset += count }
        return Data(bytes[offset..<(offset + count)])
    }

    mutating func varint() throws -> UInt64 {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        var count = 0
        while true {
            guard offset < bytes.count else { throw KagemushaAttestedCodecError.truncated }
            let byte = bytes[offset]
            offset += 1
            count += 1
            guard count <= 10 else { throw KagemushaAttestedCodecError.nonCanonical("varint width") }
            let chunk = UInt64(byte & 0x7f)
            guard shift < 64, !(shift == 63 && chunk > 1) else {
                throw KagemushaAttestedCodecError.nonCanonical("varint overflow")
            }
            value |= chunk << shift
            if byte & 0x80 == 0 {
                guard count == 1 || byte != 0 else {
                    throw KagemushaAttestedCodecError.nonCanonical("varint padding")
                }
                return value
            }
            shift += 7
        }
    }

    mutating func field() throws -> Data {
        let length = try varint()
        guard length <= UInt64(bytes.count - offset) else {
            throw KagemushaAttestedCodecError.truncated
        }
        return try raw(Int(length))
    }

    mutating func exactField(_ count: Int, _ name: String) throws -> Data {
        let value = try field()
        guard value.count == count else { throw KagemushaAttestedCodecError.invalidField(name) }
        return value
    }

    mutating func u8(_ name: String) throws -> UInt8 {
        try exactField(1, name)[0]
    }

    mutating func u16(_ name: String) throws -> UInt16 {
        Self.littleEndian(try exactField(2, name))
    }

    mutating func u32(_ name: String) throws -> UInt32 {
        Self.littleEndian(try exactField(4, name))
    }

    mutating func u64(_ name: String) throws -> UInt64 {
        Self.littleEndian(try exactField(8, name))
    }

    mutating func bool(_ name: String) throws -> Bool {
        switch try u8(name) {
        case 0: return false
        case 1: return true
        default: throw KagemushaAttestedCodecError.invalidField(name)
        }
    }

    mutating func array(_ count: Int, _ name: String) throws -> Data {
        try exactField(count, name)
    }

    mutating func string(_ name: String, maximumBytes: Int = 1_024) throws -> String {
        try Self.stringValue(field(), name, maximumBytes: maximumBytes)
    }

    mutating func option<T>(_ name: String, _ decode: (Data) throws -> T) throws -> T? {
        try Self.optionValue(field(), name, decode)
    }

    mutating func sequence<T>(
        _ name: String,
        maximumCount: Int,
        _ decode: (Data) throws -> T
    ) throws -> [T] {
        try Self.sequenceValue(field(), name, maximumCount: maximumCount, decode)
    }

    func finish() throws {
        guard isAtEnd else { throw KagemushaAttestedCodecError.trailingBytes }
    }

    static func stringValue(_ payload: Data, _ name: String, maximumBytes: Int) throws -> String {
        var reader = KagemushaAttestedReader(payload)
        let length = try reader.varint()
        guard length <= UInt64(maximumBytes) else { throw KagemushaAttestedCodecError.invalidField(name) }
        let bytes = try reader.raw(Int(length))
        try reader.finish()
        guard let value = String(data: bytes, encoding: .utf8) else {
            throw KagemushaAttestedCodecError.invalidField(name)
        }
        return value
    }

    static func optionValue<T>(_ payload: Data, _ name: String, _ decode: (Data) throws -> T) throws -> T? {
        var reader = KagemushaAttestedReader(payload)
        switch try reader.raw(1)[0] {
        case 0:
            try reader.finish()
            return nil
        case 1:
            let value = try decode(reader.field())
            try reader.finish()
            return value
        default:
            throw KagemushaAttestedCodecError.invalidField(name)
        }
    }

    static func sequenceValue<T>(
        _ payload: Data,
        _ name: String,
        maximumCount: Int,
        _ decode: (Data) throws -> T
    ) throws -> [T] {
        var reader = KagemushaAttestedReader(payload)
        let count: UInt64 = littleEndian(try reader.raw(8))
        guard count <= UInt64(maximumCount) else { throw KagemushaAttestedCodecError.invalidField(name) }
        var values: [T] = []
        values.reserveCapacity(Int(count))
        for _ in 0..<count {
            values.append(try decode(reader.field()))
        }
        try reader.finish()
        return values
    }

    /// Inverse of ``KagemushaAttestedWriter/genericByteArrayValue(_:)``.
    static func genericByteArrayValue(_ payload: Data, count: Int, _ name: String) throws -> Data {
        guard payload.count == count * 2 else { throw KagemushaAttestedCodecError.invalidField(name) }
        var out = Data(capacity: count)
        var index = payload.startIndex
        while index < payload.endIndex {
            guard payload[index] == 1 else { throw KagemushaAttestedCodecError.invalidField(name) }
            out.append(payload[index + 1])
            index += 2
        }
        return out
    }

    /// Split a bare enum value into its discriminant and length-prefixed fields.
    static func enumValue(_ payload: Data, _ name: String) throws -> (UInt32, KagemushaAttestedReader) {
        var reader = KagemushaAttestedReader(payload)
        let discriminant: UInt32 = littleEndian(try reader.raw(4))
        return (discriminant, reader)
    }

    static func littleEndian<T: FixedWidthInteger>(_ data: Data) -> T {
        var value: T = 0
        for (index, byte) in data.enumerated() {
            value |= T(byte) << (index * 8)
        }
        return value
    }
}

/// A canonical attested-suite Norito value with an explicit schema name.
protocol KagemushaAttestedNoritoValue: Sendable {
    /// Unqualified type name under ``KagemushaAttestedFraming/schemaNamespace``.
    static var schemaType: String { get }
    /// Upper bound on the complete framed value.
    static var maximumCanonicalBytes: Int { get }
    /// Bare payload bytes.
    func encodePayload() -> Data
    /// Strictly decode bare payload bytes.
    static func decodePayload(_ payload: Data) throws -> Self
}

extension KagemushaAttestedNoritoValue {
    /// Complete canonical frame: Norito header, no padding, payload.
    var canonicalBytes: Data {
        KagemushaAttestedFraming.frame(type: Self.schemaType, payload: encodePayload())
    }

    /// Decode one complete canonical frame. The bytes must re-encode identically.
    static func decodeCanonical(_ bytes: Data) throws -> Self {
        guard bytes.count <= maximumCanonicalBytes else {
            throw KagemushaAttestedCodecError.tooLarge(actual: bytes.count, maximum: maximumCanonicalBytes)
        }
        let payload = try KagemushaAttestedFraming.payload(of: bytes, type: schemaType)
        let value = try decodePayload(payload)
        guard value.canonicalBytes == bytes else {
            throw KagemushaAttestedCodecError.nonCanonical(schemaType)
        }
        return value
    }

    /// Decode a nested bare payload; nested values must also re-encode identically.
    static func decodeNested(_ payload: Data) throws -> Self {
        let value = try decodePayload(payload)
        guard value.encodePayload() == payload else {
            throw KagemushaAttestedCodecError.nonCanonical(schemaType)
        }
        return value
    }
}

/// Norito framing for the suite: canonical compact layout, no compression, CRC64 checksum,
/// explicit schema names under `iroha_data_model::kagemusha::kagemusha_attested_v1::`.
enum KagemushaAttestedFraming {
    static let schemaNamespace = "iroha_data_model::kagemusha::kagemusha_attested_v1::"
    /// Suite types hold at most `u64` scalars, so their archived alignment never exceeds the
    /// 40-byte header's natural alignment and no header padding is emitted.
    static let payloadAlignment = 8

    static func schemaName(_ type: String) -> String { schemaNamespace + type }

    static func schemaHash(_ type: String) -> [UInt8] {
        noritoSchemaHash(forTypeName: schemaName(type))
    }

    static func frame(type: String, payload: Data) -> Data {
        noritoEncode(
            typeName: schemaName(type),
            payload: payload,
            flags: NoritoHeader.compactLen,
            payloadAlignment: payloadAlignment)
    }

    /// Validate the header of one complete frame and return its payload.
    static func payload(of bytes: Data, type: String) throws -> Data {
        guard let decoded = noritoDecodeFrame(bytes) else {
            throw KagemushaAttestedCodecError.nonCanonical("frame")
        }
        guard decoded.header.schema == schemaHash(type) else {
            throw KagemushaAttestedCodecError.wrongSchema
        }
        guard decoded.header.compression == .none,
              decoded.header.flags == NoritoHeader.compactLen,
              decoded.paddingLength == 0,
              !decoded.payload.isEmpty
        else { throw KagemushaAttestedCodecError.nonCanonical("header") }
        return decoded.payload
    }

    /// The schema type of a complete frame among `candidates`, without decoding its payload.
    static func schemaType(of bytes: Data, among candidates: [String]) -> String? {
        guard bytes.count >= NoritoHeader.encodedLength,
              bytes.prefix(4) == NoritoHeader.magic else { return nil }
        let schema = [UInt8](bytes[bytes.startIndex + 6..<bytes.startIndex + 22])
        return candidates.first { schemaHash($0) == schema }
    }
}
