import Foundation

// MARK: - Decoding Errors

public enum OfflineNoritoDecodingError: Error, LocalizedError, Sendable {
    case truncatedPayload
    case invalidFrame
    case schemaHashMismatch(expected: String)
    case invalidField(String)
    case invalidEnumTag(UInt32)
    case trailingBytes(Int)
    case invalidJSON(String)

    public var errorDescription: String? {
        switch self {
        case .truncatedPayload:
            return "Norito payload ended unexpectedly."
        case .invalidFrame:
            return "Invalid Norito frame (bad magic, version, or checksum)."
        case .schemaHashMismatch(let expected):
            return "Norito schema hash does not match expected type: \(expected)"
        case .invalidField(let reason):
            return "Invalid Norito field: \(reason)"
        case .invalidEnumTag(let tag):
            return "Unknown Norito enum discriminant: \(tag)"
        case .trailingBytes(let count):
            return "Norito payload contains \(count) unexpected trailing bytes."
        case .invalidJSON(let reason):
            return "Invalid JSON in Norito metadata: \(reason)"
        }
    }
}

// MARK: - Reader

public struct OfflineNoritoReader {
    private let data: Data
    public private(set) var offset: Int = 0

    public init(data: Data) {
        self.data = data
    }

    public var remainingCount: Int { data.count - offset }

    public mutating func readUInt8() throws -> UInt8 {
        guard offset < data.count else {
            throw OfflineNoritoDecodingError.truncatedPayload
        }
        let value = data[data.startIndex + offset]
        offset += 1
        return value
    }

    public mutating func readUInt16LE() throws -> UInt16 {
        let bytes = try readBytes(2)
        var value: UInt16 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 2)
        }
        return UInt16(littleEndian: value)
    }

    public mutating func readUInt32LE() throws -> UInt32 {
        let bytes = try readBytes(4)
        var value: UInt32 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 4)
        }
        return UInt32(littleEndian: value)
    }

    public mutating func readUInt64LE() throws -> UInt64 {
        let bytes = try readBytes(8)
        var value: UInt64 = 0
        bytes.withUnsafeBytes { buffer in
            guard let base = buffer.baseAddress else { return }
            memcpy(&value, base, 8)
        }
        return UInt64(littleEndian: value)
    }

    public mutating func readBytes(_ count: Int) throws -> Data {
        guard count >= 0 else { return Data() }
        guard offset + count <= data.count else {
            throw OfflineNoritoDecodingError.truncatedPayload
        }
        let start = data.startIndex + offset
        let result = Data(data[start..<(start + count)])
        offset += count
        return result
    }

    /// Read a length-prefixed field: `[u64 LE length][bytes]`.
    public mutating func readField() throws -> Data {
        let length = try readUInt64LE()
        guard length <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("field length overflow")
        }
        return try readBytes(Int(length))
    }

    /// Read a length-prefixed field if enough bytes remain, otherwise return `nil`.
    /// Used for `#[norito(default)]` fields that may be absent in older data.
    public mutating func readFieldIfAvailable() throws -> Data? {
        guard remainingCount >= 8 else { return nil }
        return try readField()
    }
}

// MARK: - Decode Primitives

extension OfflineNorito {

    /// Unwrap a Norito envelope, validating the schema hash against the expected type name.
    static func unwrap(_ data: Data, expectedTypeName: String) throws -> Data {
        guard let frame = noritoDecodeFrame(data) else {
            throw OfflineNoritoDecodingError.invalidFrame
        }
        let expectedSchema = noritoSchemaHash(forTypeName: expectedTypeName)
        guard frame.header.schema == expectedSchema else {
            throw OfflineNoritoDecodingError.schemaHashMismatch(expected: expectedTypeName)
        }
        return frame.payload
    }

    /// Decode a Norito-encoded string: `[u64 byte_count][utf8_bytes]`.
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

    /// Decode a 32-byte hash (raw, no length prefix).
    static func decodeHash(_ data: Data) throws -> Data {
        guard data.count == 32 else {
            throw OfflineNoritoDecodingError.invalidField("hash must be 32 bytes, got \(data.count)")
        }
        return data
    }

    /// Decode a UInt64 from 8 LE bytes.
    static func decodeUInt64Field(_ data: Data) throws -> UInt64 {
        var reader = OfflineNoritoReader(data: data)
        return try reader.readUInt64LE()
    }

    /// Decode a UInt32 from 4 LE bytes.
    static func decodeUInt32Field(_ data: Data) throws -> UInt32 {
        var reader = OfflineNoritoReader(data: data)
        return try reader.readUInt32LE()
    }

    /// Decode a UInt16 from 2 LE bytes.
    static func decodeUInt16Field(_ data: Data) throws -> UInt16 {
        var reader = OfflineNoritoReader(data: data)
        return try reader.readUInt16LE()
    }

    /// Decode a Bool from 1 byte.
    static func decodeBoolField(_ data: Data) throws -> Bool {
        guard data.count == 1 else {
            throw OfflineNoritoDecodingError.invalidField("bool must be 1 byte")
        }
        return data[data.startIndex] != 0
    }

    /// Decode `Vec<u8>` (flat): `[u64 count][raw bytes]`.
    static func decodeBytesVec(_ data: Data) throws -> Data {
        var reader = OfflineNoritoReader(data: data)
        let count = try reader.readUInt64LE()
        guard count <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("bytes vec length overflow")
        }
        return try reader.readBytes(Int(count))
    }

    /// Decode `ConstVec<u8>` (per-element): `[u64 count]{[u64 len=1][u8]}*`.
    static func decodeConstVec(_ data: Data) throws -> Data {
        var reader = OfflineNoritoReader(data: data)
        let count = try reader.readUInt64LE()
        guard count <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("const vec length overflow")
        }
        var result = Data(capacity: Int(count))
        for _ in 0..<Int(count) {
            let len = try reader.readUInt64LE()
            guard len == 1 else {
                throw OfflineNoritoDecodingError.invalidField(
                    "ConstVec<u8> element length must be 1, got \(len)")
            }
            result.append(try reader.readUInt8())
        }
        return result
    }

    /// Decode fixed-length per-element bytes without a leading count.
    /// Used for `fixedBytesPayload` (e.g., verifying key commitment, envelope hash).
    /// Format: `{[u64 len=1][u8]}*` until data is exhausted.
    static func decodeFixedBytes(_ data: Data) throws -> Data {
        var reader = OfflineNoritoReader(data: data)
        var result = Data()
        while reader.remainingCount > 0 {
            let len = try reader.readUInt64LE()
            guard len == 1 else {
                throw OfflineNoritoDecodingError.invalidField(
                    "fixed bytes element length must be 1, got \(len)")
            }
            result.append(try reader.readUInt8())
        }
        return result
    }

    /// Decode an `Option<T>`: `[u8 tag]` where 0=None, 1=Some(`[u64 len][payload]`).
    static func decodeOption<T>(_ data: Data, decode: (Data) throws -> T) throws -> T? {
        var reader = OfflineNoritoReader(data: data)
        let tag = try reader.readUInt8()
        switch tag {
        case 0:
            return nil
        case 1:
            let payload = try reader.readField()
            return try decode(payload)
        default:
            throw OfflineNoritoDecodingError.invalidField("option tag must be 0 or 1, got \(tag)")
        }
    }

    /// Decode `Vec<T>`: `[u64 count]{[u64 len][item_payload]}*`.
    static func decodeVec<T>(_ data: Data, decode: (Data) throws -> T) throws -> [T] {
        var reader = OfflineNoritoReader(data: data)
        let count = try reader.readUInt64LE()
        guard count <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("vec length overflow")
        }
        var items: [T] = []
        items.reserveCapacity(min(Int(count), 1024))
        for _ in 0..<Int(count) {
            let payload = try reader.readField()
            items.append(try decode(payload))
        }
        return items
    }

    /// Decode a Norito-encoded AccountId back to the I105 account name.
    ///
    /// Binary format (controller only, matching Rust `AccountId::NoritoSerialize`):
    /// ```
    /// [u32 tag][u64 key_field_len][key_payload]
    /// ```
    /// tag=0 means Account variant. The key payload is a multihash hex string
    /// (e.g. `ed01203699A6...`). This method parses it back to an I105 account name.
    public static func decodeAccountId(_ data: Data) throws -> String {
        var reader = OfflineNoritoReader(data: data)
        let tag = try reader.readUInt32LE()
        guard tag == 0 else {
            throw OfflineNoritoDecodingError.invalidField(
                "unsupported AccountSignatory tag: \(tag)")
        }
        let nameData = try reader.readField()
        let controllerName = try decodeString(nameData)

        // Production binaries store a multihash hex string (e.g. "ed01203699A6…").
        // Test binaries (no native bridge) store an I105 name directly.
        // Detect format: multihash hex is pure hex starting with a known function code.
        if let address = try? parseMultihashHexToAddress(controllerName) {
            return try address.toI105Default()
        }
        // Already an I105 name (test path)
        return controllerName
    }

    /// Parse a multihash hex string (e.g. `ed01203699A6...`) into an AccountAddress.
    ///
    /// Multihash format: `[varint function_code][varint digest_length][digest_bytes]`
    /// All encoded as hex. Function codes: ed25519=0xed, secp256k1=0xe7, mlDsa=0xee, sm2=0x1306.
    private static func parseMultihashHexToAddress(_ hex: String) throws -> AccountAddress {
        guard let bytes = Data(hexString: hex), !bytes.isEmpty else {
            throw OfflineNoritoDecodingError.invalidField("invalid multihash hex: \(hex)")
        }
        var cursor = 0
        let (functionCode, fcLen) = try decodeVarint(bytes, at: cursor)
        cursor += fcLen
        let (digestLength, dlLen) = try decodeVarint(bytes, at: cursor)
        cursor += dlLen
        guard cursor + Int(digestLength) == bytes.count else {
            throw OfflineNoritoDecodingError.invalidField(
                "multihash length mismatch: expected \(digestLength) bytes, have \(bytes.count - cursor)")
        }
        let publicKey = bytes[cursor ..< cursor + Int(digestLength)]

        let algorithm: String
        switch functionCode {
        case 0xed: algorithm = "ed25519"
        case 0xe7: algorithm = "secp256k1"
        case 0xee: algorithm = "ml-dsa"
        case 0x1306: algorithm = "sm2"
        default:
            throw OfflineNoritoDecodingError.invalidField(
                "unknown multihash function code: 0x\(String(functionCode, radix: 16))")
        }
        return try AccountAddress.fromAccount(publicKey: Data(publicKey), algorithm: algorithm)
    }

    /// Decode an unsigned varint from `data` starting at `offset`.
    /// Returns (value, bytesConsumed).
    private static func decodeVarint(_ data: Data, at offset: Int) throws -> (UInt64, Int) {
        var value: UInt64 = 0
        var shift: UInt64 = 0
        var i = offset
        while i < data.count {
            let byte = data[i]
            value |= UInt64(byte & 0x7F) << shift
            i += 1
            if byte & 0x80 == 0 {
                return (value, i - offset)
            }
            shift += 7
            if shift >= 64 {
                throw OfflineNoritoDecodingError.invalidField("varint overflow")
            }
        }
        throw OfflineNoritoDecodingError.invalidField("truncated varint")
    }

    /// Decode a Norito-encoded AssetId back to a `norito:HEX` literal.
    private static let assetIdTypeName = "iroha_data_model::asset::id::model::AssetId"

    static func decodeAssetId(_ data: Data) throws -> String {
        guard !data.isEmpty else {
            throw OfflineNoritoDecodingError.invalidField("asset id must not be empty")
        }
        // Re-wrap raw AssetId bytes with NRT envelope so the norito:<hex>
        // literal matches the on-chain format the server expects.
        let wrapped = noritoEncode(typeName: assetIdTypeName, payload: data)
        return "norito:" + wrapped.hexLowercased()
    }

    /// Decode a legacy textual `AssetDefinitionId` payload from its Norito binary representation.
    ///
    /// Binary layout: `[domain_field][name_field]`
    /// where domain = `[str_field]` and name = `[str_field]`.
    /// Returns `(name, domain)` tuple — e.g. `("usd", "wonderland")`.
    static func decodeAssetDefinitionIdDirect(_ data: Data) throws -> (name: String, domain: String) {
        var defReader = OfflineNoritoReader(data: data)
        let domainData = try defReader.readField()
        var domainReader = OfflineNoritoReader(data: domainData)
        let domainStrData = try domainReader.readField()
        let domain = try decodeString(domainStrData)
        let nameData = try defReader.readField()
        let name = try decodeString(nameData)
        return (name: name, domain: domain)
    }

    /// Decode the `AssetDefinitionId` component from a full Norito `AssetId` payload.
    ///
    /// Binary layout: `[account_field][definition_field]`
    /// The definition field contains either:
    /// - `[domain_field][name_field]` (legacy layout)
    /// - ConstVec<u8> encoding of 16-byte UUID: `{[u64=1][u8]}*16` (current Iroha layout)
    static func decodeAssetDefinitionIdFromAssetId(_ data: Data) throws -> (name: String, domain: String) {
        var reader = OfflineNoritoReader(data: data)
        _ = try reader.readField() // skip account
        let defData = try reader.readField()
        // Try legacy [domain][name] layout first
        if let result = try? decodeAssetDefinitionIdDirect(defData) {
            return result
        }
        // Try ConstVec<u8> layout: 16 bytes encoded as {[u64=1][u8]}*16 = 144 bytes
        if defData.count == 144, let aidBytes = try? decodeFixedBytes(defData), aidBytes.count == 16 {
            guard let address = AssetDefinitionAddress.encode(uuidBytes: aidBytes) else {
                throw OfflineNoritoDecodingError.invalidField("cannot encode canonical asset definition address")
            }
            return (name: address, domain: "")
        }
        throw OfflineNoritoDecodingError.invalidField("cannot decode AssetDefinitionId from \(defData.count) bytes")
    }

    /// Extract account ID string from a full Norito `AssetId` payload.
    ///
    /// The account field layout: `[u32 controller_tag][field(string)]`
    /// where the string is the I105-encoded account identifier.
    public static func accountIdFromAssetIdPayload(_ data: Data) -> String? {
        guard let payload = try? {
            var bytes = data
            if let frame = noritoDecodeFrame(data) {
                bytes = frame.payload
            } else if bytes.count > 40,
                      bytes[0] == 0x4E, bytes[1] == 0x52, bytes[2] == 0x54, bytes[3] == 0x30 {
                bytes = Data(bytes.dropFirst(40))
            }
            return bytes
        }() else { return nil }

        do {
            var reader = OfflineNoritoReader(data: payload)
            let accountData = try reader.readField()
            // Account: [u32 tag][field(string)]
            var accountReader = OfflineNoritoReader(data: accountData)
            _ = try accountReader.readUInt32LE() // controller tag
            let stringField = try accountReader.readField()
            let multihash = try decodeString(stringField)
            // Convert multihash "ed0120<hex>" to I105 format.
            // Multihash layout: "ed" (algorithm) + "0120" (varint + length) + 64 hex chars (32 bytes key)
            let mhLower = multihash.lowercased()
            if mhLower.hasPrefix("ed0120"), multihash.count == 70 {
                let keyHex = String(multihash.dropFirst(6))
                if let keyBytes = Data(hexString: keyHex),
                   let address = try? AccountAddress.fromAccount(publicKey: keyBytes, algorithm: "ed25519"),
                   let i105 = try? address.toI105Default() {
                    return i105
                }
            }
            return multihash
        } catch {
            return nil
        }
    }

    /// Decode a `norito:hex` asset ID literal to the current asset-definition identifier form.
    ///
    /// Handles two binary layouts:
    /// - Full `AssetId` (from NoritoBridge FFI): `{account}{definition}` — may have NRT envelope
    /// - Bare `AssetDefinitionId` (from encode→decode roundtrip): `{domain}{name}`
    ///
    /// Returns the canonical unprefixed Base58 asset-definition ID for current
    /// 16-byte layouts, or the legacy `name#domain` form for older textual
    /// payloads. Returns `nil` if the literal is not in `norito:` format or
    /// cannot be parsed.
    public static func assetDefinitionIdFromLiteral(_ literal: String) -> String? {
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.lowercased().hasPrefix("norito:") else { return nil }
        let hex = String(trimmed.dropFirst("norito:".count))
        guard !hex.isEmpty, hex.count.isMultiple(of: 2),
              var bytes = Data(hexString: hex) else { return nil }
        // Use proper frame parsing if NRT header present
        if let frame = noritoDecodeFrame(bytes) {
            bytes = frame.payload
        } else if bytes.count > 40,
                  bytes[0] == 0x4E, bytes[1] == 0x52, bytes[2] == 0x54, bytes[3] == 0x30 {
            bytes = Data(bytes.dropFirst(40))
        }
        // Try full AssetId first (account + definition), then bare AssetDefinitionId.
        if let (name, domain) = try? decodeAssetDefinitionIdFromAssetId(bytes) {
            if domain.isEmpty {
                return name
            }
            return "\(name)#\(domain)"
        }
        if bytes.count == 16, let address = AssetDefinitionAddress.encode(uuidBytes: bytes) {
            return address
        }
        if let (name, domain) = try? decodeAssetDefinitionIdDirect(bytes) {
            return "\(name)#\(domain)"
        }
        return nil
    }

    /// Decode a Norito-encoded Numeric value back to a decimal string.
    ///
    /// Format: `[bigint_field][scale_field]`
    /// - bigint_field: `[u32 byte_count][twos_complement_le_bytes]`
    /// - scale_field: `[u32 scale]`
    static func decodeNumeric(_ data: Data) throws -> String {
        var reader = OfflineNoritoReader(data: data)
        // Read bigint field
        let bigintFieldData = try reader.readField()
        var bigintReader = OfflineNoritoReader(data: bigintFieldData)
        let byteCount = try bigintReader.readUInt32LE()
        let mantissaBytes = try bigintReader.readBytes(Int(byteCount))
        // Read scale field
        let scaleFieldData = try reader.readField()
        let scale = try decodeUInt32Field(scaleFieldData)
        return try twosComplementToDecimal(mantissaBytes, scale: Int(scale))
    }

    /// Decode PoseidonDigest: `[u64 len][32 bytes]`.
    static func decodePoseidonDigest(_ data: Data) throws -> Data {
        var reader = OfflineNoritoReader(data: data)
        let length = try reader.readUInt64LE()
        guard length == 32 else {
            throw OfflineNoritoDecodingError.invalidField(
                "poseidon digest must be 32 bytes, got \(length)")
        }
        return try reader.readBytes(32)
    }

    /// Decode metadata: `[u64 count]{[u64 entry_len][entry]}*`
    ///
    /// Each entry: `[u64 name_field_len][name_payload][u64 json_field_len][json_payload]`
    static func decodeMetadata(_ data: Data) throws -> [String: ToriiJSONValue] {
        var reader = OfflineNoritoReader(data: data)
        let count = try reader.readUInt64LE()
        guard count <= UInt64(Int.max) else {
            throw OfflineNoritoDecodingError.invalidField("metadata count overflow")
        }
        var result: [String: ToriiJSONValue] = [:]
        for _ in 0..<Int(count) {
            let entryData = try reader.readField()
            var entryReader = OfflineNoritoReader(data: entryData)
            // Read name field → decode string
            let nameFieldData = try entryReader.readField()
            let key = try decodeString(nameFieldData)
            // Read json outer field → inner field → decode string → parse JSON
            let jsonOuterData = try entryReader.readField()
            var jsonOuterReader = OfflineNoritoReader(data: jsonOuterData)
            let jsonInnerData = try jsonOuterReader.readField()
            let jsonString = try decodeString(jsonInnerData)
            let value = try parseJSON(jsonString)
            result[key] = value
        }
        return result
    }

    // MARK: - Internal Helpers

    private static func twosComplementToDecimal(_ bytes: Data, scale: Int) throws -> String {
        guard !bytes.isEmpty else {
            return scale > 0 ? "0." + String(repeating: "0", count: scale) : "0"
        }

        let isNegative = (bytes[bytes.startIndex + bytes.count - 1] & 0x80) != 0

        var magnitude: [UInt8]
        if isNegative {
            magnitude = Array(bytes)
            for i in magnitude.indices {
                magnitude[i] = ~magnitude[i]
            }
            var carry: UInt8 = 1
            for i in magnitude.indices {
                let sum = UInt16(magnitude[i]) + UInt16(carry)
                magnitude[i] = UInt8(sum & 0xFF)
                carry = sum > 0xFF ? 1 : 0
                if carry == 0 { break }
            }
        } else {
            magnitude = Array(bytes)
        }

        while magnitude.count > 1 && magnitude.last == 0 {
            magnitude.removeLast()
        }

        var digits = magnitudeToDecimal(magnitude)
        if digits.isEmpty { digits = "0" }

        if scale > 0 {
            while digits.count <= scale {
                digits = "0" + digits
            }
            let insertPos = digits.count - scale
            digits = String(digits.prefix(insertPos)) + "." + String(digits.suffix(scale))
        }

        if isNegative && digits != "0" && !digits.allSatisfy({ $0 == "0" || $0 == "." }) {
            digits = "-" + digits
        }

        return digits
    }

    private static func magnitudeToDecimal(_ bytes: [UInt8]) -> String {
        guard !bytes.isEmpty else { return "0" }
        // Pack bytes into u32 limbs (little-endian)
        var limbs: [UInt32] = []
        var i = 0
        while i < bytes.count {
            var limb: UInt32 = 0
            for j in 0..<4 {
                if i + j < bytes.count {
                    limb |= UInt32(bytes[i + j]) << (j * 8)
                }
            }
            limbs.append(limb)
            i += 4
        }
        // Repeated division by 10
        var digitChars: [Character] = []
        while !limbs.allSatisfy({ $0 == 0 }) {
            var remainder: UInt64 = 0
            for idx in (0..<limbs.count).reversed() {
                let dividend = (remainder << 32) | UInt64(limbs[idx])
                limbs[idx] = UInt32(dividend / 10)
                remainder = dividend % 10
            }
            digitChars.append(Character(String(remainder)))
            while limbs.count > 1 && limbs.last == 0 {
                limbs.removeLast()
            }
        }
        if digitChars.isEmpty { return "0" }
        return String(digitChars.reversed())
    }

    private static func parseJSON(_ jsonString: String) throws -> ToriiJSONValue {
        guard let data = jsonString.data(using: .utf8) else {
            throw OfflineNoritoDecodingError.invalidJSON("invalid UTF-8")
        }
        guard let raw = try? JSONSerialization.jsonObject(with: data, options: .fragmentsAllowed) else {
            throw OfflineNoritoDecodingError.invalidJSON(jsonString)
        }
        return convertToToriiJSON(raw)
    }

    private static func convertToToriiJSON(_ value: Any) -> ToriiJSONValue {
        if value is NSNull {
            return .null
        }
        if let number = value as? NSNumber {
            if CFBooleanGetTypeID() == CFGetTypeID(number) {
                return .bool(number.boolValue)
            }
            return .number(number.doubleValue)
        }
        if let string = value as? String {
            return .string(string)
        }
        if let array = value as? [Any] {
            return .array(array.map(convertToToriiJSON))
        }
        if let dict = value as? [String: Any] {
            var result: [String: ToriiJSONValue] = [:]
            for (key, val) in dict {
                result[key] = convertToToriiJSON(val)
            }
            return .object(result)
        }
        return .null
    }
}

// MARK: - Model Decode Extensions

extension OfflineAllowanceCommitment {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let assetIdData = try reader.readField()
        let amountData = try reader.readField()
        let commitmentData = try reader.readField()
        self.init(
            assetId: try OfflineNorito.decodeAssetId(assetIdData),
            amount: try OfflineNorito.decodeNumeric(amountData),
            commitment: try OfflineNorito.decodeBytesVec(commitmentData)
        )
    }
}

extension OfflinePlatformTokenSnapshot {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let policyData = try reader.readField()
        let jwsData = try reader.readField()
        self.init(
            policy: try OfflineNorito.decodeString(policyData),
            attestationJwsB64: try OfflineNorito.decodeString(jwsData)
        )
    }
}

extension AppleAppAttestProof {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let keyIdData = try reader.readField()
        let counterData = try reader.readField()
        let assertionData = try reader.readField()
        let challengeHashData = try reader.readField()
        self.init(
            keyId: try OfflineNorito.decodeString(keyIdData),
            counter: try OfflineNorito.decodeUInt64Field(counterData),
            assertion: try OfflineNorito.decodeBytesVec(assertionData),
            challengeHash: try OfflineNorito.decodeHash(challengeHashData)
        )
    }
}

extension AndroidMarkerKeyProof {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let seriesData = try reader.readField()
        let counterData = try reader.readField()
        let markerPubKeyData = try reader.readField()
        let markerSigData = try reader.readField()
        let attestationData = try reader.readField()
        self.init(
            series: try OfflineNorito.decodeString(seriesData),
            counter: try OfflineNorito.decodeUInt64Field(counterData),
            markerPublicKey: try OfflineNorito.decodeBytesVec(markerPubKeyData),
            markerSignature: try OfflineNorito.decodeOption(markerSigData,
                                                            decode: OfflineNorito.decodeBytesVec),
            attestation: try OfflineNorito.decodeBytesVec(attestationData)
        )
    }
}

extension AndroidProvisionedProof {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let schemaData = try reader.readField()
        let versionData = try reader.readField()
        let issuedAtData = try reader.readField()
        let challengeData = try reader.readField()
        let counterData = try reader.readField()
        let manifestData = try reader.readField()
        let signatureData = try reader.readField()

        let version: Int? = try OfflineNorito.decodeOption(versionData) { d in
            Int(try OfflineNorito.decodeUInt32Field(d))
        }
        let challengeHash = try OfflineNorito.decodeHash(challengeData)
        let signature = try OfflineNorito.decodeConstVec(signatureData)

        try self.init(
            manifestSchema: OfflineNorito.decodeString(schemaData),
            manifestVersion: version,
            manifestIssuedAtMs: OfflineNorito.decodeUInt64Field(issuedAtData),
            challengeHashLiteral: challengeHash.hexUppercased(),
            counter: OfflineNorito.decodeUInt64Field(counterData),
            deviceManifest: OfflineNorito.decodeMetadata(manifestData),
            inspectorSignatureHex: signature.hexUppercased()
        )
    }
}

extension OfflinePlatformProof {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let tag = try reader.readUInt32LE()
        let payload = try reader.readField()
        switch tag {
        case 0:
            self = .appleAppAttest(try AppleAppAttestProof(noritoPayload: payload))
        case 1:
            self = .androidMarkerKey(try AndroidMarkerKeyProof(noritoPayload: payload))
        case 2:
            self = .provisioned(try AndroidProvisionedProof(noritoPayload: payload))
        default:
            throw OfflineNoritoDecodingError.invalidEnumTag(tag)
        }
    }
}

extension OfflineBalanceProof {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let initialCommitmentData = try reader.readField()
        let resultingCommitmentData = try reader.readField()
        let claimedDeltaData = try reader.readField()
        let zkProofData = try reader.readField()
        self.init(
            initialCommitment: try OfflineAllowanceCommitment(noritoPayload: initialCommitmentData),
            resultingCommitment: try OfflineNorito.decodeBytesVec(resultingCommitmentData),
            claimedDelta: try OfflineNorito.decodeNumeric(claimedDeltaData),
            zkProof: try OfflineNorito.decodeOption(zkProofData,
                                                     decode: OfflineNorito.decodeBytesVec)
        )
    }
}

extension OfflineCertificateBalanceProof {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let certIdData = try reader.readField()
        let proofData = try reader.readField()
        self.init(
            senderCertificateId: try OfflineNorito.decodeHash(certIdData),
            balanceProof: try OfflineBalanceProof(noritoPayload: proofData)
        )
    }
}

extension OfflinePoseidonDigest {
    init(noritoPayload data: Data) throws {
        self.init(bytes: try OfflineNorito.decodePoseidonDigest(data))
    }
}

extension OfflineAggregateProofEnvelope {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let versionData = try reader.readField()
        let rootData = try reader.readField()
        let proofSumData = try reader.readField()
        let proofCounterData = try reader.readField()
        let proofReplayData = try reader.readField()
        let metadataData = try reader.readField()
        self.init(
            version: try OfflineNorito.decodeUInt16Field(versionData),
            receiptsRoot: try OfflinePoseidonDigest(noritoPayload: rootData),
            proofSum: try OfflineNorito.decodeOption(proofSumData,
                                                     decode: OfflineNorito.decodeBytesVec),
            proofCounter: try OfflineNorito.decodeOption(proofCounterData,
                                                         decode: OfflineNorito.decodeBytesVec),
            proofReplay: try OfflineNorito.decodeOption(proofReplayData,
                                                        decode: OfflineNorito.decodeBytesVec),
            metadata: try OfflineNorito.decodeMetadata(metadataData)
        )
    }
}

extension ProofAttachment.VerifyingKeyReference {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let backendData = try reader.readField()
        let nameData = try reader.readField()
        self.init(
            backend: try OfflineNorito.decodeString(backendData),
            name: try OfflineNorito.decodeString(nameData)
        )
    }
}

extension ProofAttachment.VerifyingKeyInline {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let backendData = try reader.readField()
        let bytesData = try reader.readField()
        self.init(
            backend: try OfflineNorito.decodeString(backendData),
            bytes: try OfflineNorito.decodeBytesVec(bytesData)
        )
    }
}

extension ProofAttachment {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let backendData = try reader.readField()
        let proofBoxData = try reader.readField()
        let vkRefData = try reader.readField()
        let vkInlineData = try reader.readField()

        let backend = try OfflineNorito.decodeString(backendData)

        // proofBox: [backend_field][bytes_field]
        var proofBoxReader = OfflineNoritoReader(data: proofBoxData)
        _ = try proofBoxReader.readField() // skip proof box backend (same as outer)
        let proofBytesData = try proofBoxReader.readField()
        let proof = try OfflineNorito.decodeBytesVec(proofBytesData)

        let vkRef: VerifyingKeyReference? = try OfflineNorito.decodeOption(vkRefData) {
            try VerifyingKeyReference(noritoPayload: $0)
        }
        let vkInline: VerifyingKeyInline? = try OfflineNorito.decodeOption(vkInlineData) {
            try VerifyingKeyInline(noritoPayload: $0)
        }

        let verifyingKey: VerifyingKey
        if let ref = vkRef {
            verifyingKey = .reference(ref)
        } else if let inline = vkInline {
            verifyingKey = .inline(inline)
        } else {
            throw ProofAttachmentError.missingVerifyingKey
        }

        // Optional trailing fields (#[norito(default)])
        var vkCommitment: Data?
        var envelope: Data?
        if let commitmentFieldData = try reader.readFieldIfAvailable() {
            vkCommitment = try OfflineNorito.decodeOption(commitmentFieldData,
                                                          decode: OfflineNorito.decodeFixedBytes)
            if let envelopeFieldData = try reader.readFieldIfAvailable() {
                envelope = try OfflineNorito.decodeOption(envelopeFieldData,
                                                          decode: OfflineNorito.decodeFixedBytes)
            }
        }

        try self.init(
            backend: backend,
            proof: proof,
            verifyingKey: verifyingKey,
            verifyingKeyCommitment: vkCommitment,
            envelopeHash: envelope
        )
    }
}

extension OfflineProofAttachmentList {
    init(noritoPayload data: Data) throws {
        self.init(
            attachments: try OfflineNorito.decodeVec(data) {
                try ProofAttachment(noritoPayload: $0)
            }
        )
    }
}

extension OfflineSpendReceipt {
    init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let txIdData = try reader.readField()
        let fromData = try reader.readField()
        let toData = try reader.readField()
        let assetIdData = try reader.readField()
        let amountData = try reader.readField()
        let issuedAtData = try reader.readField()
        let invoiceIdData = try reader.readField()
        let platformProofData = try reader.readField()
        let platformSnapshotData = try reader.readField()
        let senderCertIdData = try reader.readField()
        let senderSigData = try reader.readField()
        self.init(
            txId: try OfflineNorito.decodeHash(txIdData),
            from: try OfflineNorito.decodeAccountId(fromData),
            to: try OfflineNorito.decodeAccountId(toData),
            assetId: try OfflineNorito.decodeAssetId(assetIdData),
            amount: try OfflineNorito.decodeNumeric(amountData),
            issuedAtMs: try OfflineNorito.decodeUInt64Field(issuedAtData),
            invoiceId: try OfflineNorito.decodeString(invoiceIdData),
            platformProof: try OfflinePlatformProof(noritoPayload: platformProofData),
            platformSnapshot: try OfflineNorito.decodeOption(platformSnapshotData) {
                try OfflinePlatformTokenSnapshot(noritoPayload: $0)
            },
            senderCertificateId: try OfflineNorito.decodeHash(senderCertIdData),
            senderSignature: try OfflineNorito.decodeConstVec(senderSigData)
        )
    }
}

extension OfflineToOnlineTransfer {

    /// Decode from raw Norito payload bytes (without NRT envelope).
    public init(noritoPayload data: Data) throws {
        var reader = OfflineNoritoReader(data: data)
        let bundleIdData = try reader.readField()
        let receiverData = try reader.readField()
        let depositAccountData = try reader.readField()
        let receiptsData = try reader.readField()
        let balanceProofData = try reader.readField()

        let bundleId = try OfflineNorito.decodeHash(bundleIdData)
        let receiver = try OfflineNorito.decodeAccountId(receiverData)
        let depositAccount = try OfflineNorito.decodeAccountId(depositAccountData)
        let receipts = try OfflineNorito.decodeVec(receiptsData) {
            try OfflineSpendReceipt(noritoPayload: $0)
        }
        let balanceProof = try OfflineBalanceProof(noritoPayload: balanceProofData)

        // Optional #[norito(default)] fields
        let balanceProofs: [OfflineCertificateBalanceProof]?
        if let fieldData = try reader.readFieldIfAvailable() {
            balanceProofs = try OfflineNorito.decodeOption(fieldData) { d in
                try OfflineNorito.decodeVec(d) {
                    try OfflineCertificateBalanceProof(noritoPayload: $0)
                }
            }
        } else {
            balanceProofs = nil
        }

        let aggregateProof: OfflineAggregateProofEnvelope?
        if let fieldData = try reader.readFieldIfAvailable() {
            aggregateProof = try OfflineNorito.decodeOption(fieldData) {
                try OfflineAggregateProofEnvelope(noritoPayload: $0)
            }
        } else {
            aggregateProof = nil
        }

        let attachments: OfflineProofAttachmentList?
        if let fieldData = try reader.readFieldIfAvailable() {
            attachments = try OfflineNorito.decodeOption(fieldData) {
                try OfflineProofAttachmentList(noritoPayload: $0)
            }
        } else {
            attachments = nil
        }

        let platformSnapshot: OfflinePlatformTokenSnapshot?
        if let fieldData = try reader.readFieldIfAvailable() {
            platformSnapshot = try OfflineNorito.decodeOption(fieldData) {
                try OfflinePlatformTokenSnapshot(noritoPayload: $0)
            }
        } else {
            platformSnapshot = nil
        }

        self.init(
            bundleId: bundleId,
            receiver: receiver,
            depositAccount: depositAccount,
            receipts: receipts,
            balanceProof: balanceProof,
            balanceProofs: balanceProofs,
            aggregateProof: aggregateProof,
            attachments: attachments,
            platformSnapshot: platformSnapshot
        )
    }

    /// Decode from a complete Norito envelope (NRT header + payload).
    public init(noritoEncoded data: Data) throws {
        let typeName = "iroha_data_model::offline::model::OfflineToOnlineTransfer"
        let payload = try OfflineNorito.unwrap(data, expectedTypeName: typeName)
        try self.init(noritoPayload: payload)
    }
}
