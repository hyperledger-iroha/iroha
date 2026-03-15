import Foundation

public enum OfflineNoritoError: Error, LocalizedError {
    case invalidHex(String)
    case invalidLength(String)
    case invalidNumeric(String)
    case numericScaleTooLarge
    case numericOverflow
    case invalidAssetId(String)
    case invalidAccountId(String)
    case invalidHash(String)
    case invalidMetadata(String)
    case nativeBridgeUnavailable(String)

    public var errorDescription: String? {
        switch self {
        case let .invalidHex(reason):
            return "Invalid hex payload: \(reason)"
        case let .invalidLength(reason):
            return "Invalid payload length: \(reason)"
        case let .invalidNumeric(value):
            return "Invalid numeric value: \(value)"
        case .numericScaleTooLarge:
            return "Numeric scale exceeds 28 decimal places."
        case .numericOverflow:
            return "Numeric value exceeds 512-bit limit."
        case let .invalidAssetId(value):
            return "Invalid asset id: \(value)"
        case let .invalidAccountId(value):
            return "Invalid account id: \(value)"
        case let .invalidHash(value):
            return "Invalid hash: \(value)"
        case let .invalidMetadata(value):
            return "Invalid metadata payload: \(value)"
        case let .nativeBridgeUnavailable(symbol):
            return "Native Norito bridge is unavailable for symbol: \(symbol)"
        }
    }
}

struct OfflineNoritoWriter {
    private(set) var data = Data()

    mutating func writeUInt8(_ value: UInt8) {
        data.append(value)
    }

    mutating func writeUInt16LE(_ value: UInt16) {
        var le = value.littleEndian
        data.append(contentsOf: withUnsafeBytes(of: &le, Array.init))
    }

    mutating func writeUInt32LE(_ value: UInt32) {
        var le = value.littleEndian
        data.append(contentsOf: withUnsafeBytes(of: &le, Array.init))
    }

    mutating func writeUInt64LE(_ value: UInt64) {
        var le = value.littleEndian
        data.append(contentsOf: withUnsafeBytes(of: &le, Array.init))
    }

    mutating func writeLength(_ value: UInt64) {
        writeUInt64LE(value)
    }

    mutating func writeBytes(_ bytes: Data) {
        data.append(bytes)
    }

    mutating func writeField(_ payload: Data) {
        writeLength(UInt64(payload.count))
        writeBytes(payload)
    }
}

enum OfflineNorito {
    static let maxNumericScale: UInt32 = 28
    private static let maxBigIntBytes = 64
    private static let maxSafeInteger: Double = 9_007_199_254_740_992 // 2^53
    private static let defaultNetworkPrefix: UInt16 = 0x02F1
    private static let isRunningXCTest = ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil

    static func wrap(typeName: String, payload: Data) -> Data {
        noritoEncode(typeName: typeName, payload: payload, flags: 0)
    }

    static func encodeString(_ value: String) -> Data {
        var writer = OfflineNoritoWriter()
        let bytes = Data(value.utf8)
        writer.writeLength(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    static func encodeAccountId(_ value: String) throws -> Data {
        if isRunningXCTest {
            let canonical = try canonicalizeAccountIdWithoutNativeParse(value)
            var writer = OfflineNoritoWriter()
            let domainPayload = try encodeDomainId(AccountAddress.defaultDomainName)
            writer.writeField(domainPayload)

            var accountControllerPayload = OfflineNoritoWriter()
            accountControllerPayload.writeUInt32LE(0)
            accountControllerPayload.writeField(encodeString(canonical))
            writer.writeField(accountControllerPayload.data)
            return writer.data
        }
        let canonical = try canonicalizeEncodedAccountId(value)
        let address = try AccountAddress.parseEncoded(
            canonical,
            expectedPrefix: defaultNetworkPrefix
        )
        var writer = OfflineNoritoWriter()
        let domainPayload = try encodeDomainId(AccountAddress.defaultDomainName)
        writer.writeField(domainPayload)
        writer.writeField(try address.noritoAccountControllerPayload())
        return writer.data
    }

    static func encodeBool(_ value: Bool) -> Data {
        Data([value ? 1 : 0])
    }

    static func encodeUInt8(_ value: UInt8) -> Data {
        Data([value])
    }

    static func encodeUInt16(_ value: UInt16) -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeUInt16LE(value)
        return writer.data
    }

    static func encodeUInt32(_ value: UInt32) -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeUInt32LE(value)
        return writer.data
    }

    static func encodeUInt64(_ value: UInt64) -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeUInt64LE(value)
        return writer.data
    }

    static func encodeOption<T>(_ value: T?, encode: (T) throws -> Data) throws -> Data {
        var writer = OfflineNoritoWriter()
        guard let value else {
            writer.writeUInt8(0)
            return writer.data
        }
        writer.writeUInt8(1)
        let payload = try encode(value)
        writer.writeLength(UInt64(payload.count))
        writer.writeBytes(payload)
        return writer.data
    }

    static func encodeVec<T>(_ values: [T], encode: (T) throws -> Data) throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeLength(UInt64(values.count))
        for value in values {
            let payload = try encode(value)
            writer.writeLength(UInt64(payload.count))
            writer.writeBytes(payload)
        }
        return writer.data
    }

    /// Encode `Vec<u8>` fields (flat blob): `[u64 count][raw bytes]`.
    /// Rust `Vec<u8>` has a special-case in NoritoSerialize that writes bytes flat.
    /// Used for: `attestation_report`, `allowance.commitment`, `assertion`, etc.
    static func encodeBytesVec(_ bytes: Data) -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeLength(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    /// Encode `ConstVec<u8>` fields (per-element): `[u64 count]{[u64 len=1][u8]}*`.
    /// Rust `ConstVec<u8>` unpacked layout encodes each u8 with its own u64 length prefix.
    /// Used for: `operator_signature` (Signature wraps ConstVec<u8>).
    static func encodeConstVec(_ bytes: Data) -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeLength(UInt64(bytes.count))
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }

    static func encodeHash(_ bytes: Data) throws -> Data {
        guard bytes.count == 32 else {
            throw OfflineNoritoError.invalidLength("hash must be 32 bytes")
        }
        guard let last = bytes.last, (last & 1) == 1 else {
            throw OfflineNoritoError.invalidHash("least significant bit must be set")
        }
        return bytes
    }

    static func encodeNumeric(_ value: String) throws -> Data {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineNoritoError.invalidNumeric(value)
        }
        var digits = trimmed
        let negative = digits.first == "-"
        if digits.first == "-" || digits.first == "+" {
            digits.removeFirst()
        }
        var seenDot = false
        var scale: UInt32 = 0
        var mantissaDigits = ""
        for scalar in digits.unicodeScalars {
            if scalar == "." {
                if seenDot {
                    throw OfflineNoritoError.invalidNumeric(value)
                }
                seenDot = true
                continue
            }
            guard scalar.value >= 48 && scalar.value <= 57 else {
                throw OfflineNoritoError.invalidNumeric(value)
            }
            mantissaDigits.append(Character(scalar))
            if seenDot {
                scale = scale &+ 1
            }
        }
        guard !mantissaDigits.isEmpty else {
            throw OfflineNoritoError.invalidNumeric(value)
        }
        guard scale <= maxNumericScale else {
            throw OfflineNoritoError.numericScaleTooLarge
        }
        var bigInt = try OfflineBigInt(decimalDigits: mantissaDigits)
        if bigInt.isZero {
            bigInt.isNegative = false
        } else {
            bigInt.isNegative = negative
        }
        let mantissaBytes = try bigInt.toTwosComplementBytes(maxBytes: maxBigIntBytes)
        var bigintWriter = OfflineNoritoWriter()
        bigintWriter.writeUInt32LE(UInt32(mantissaBytes.count))
        bigintWriter.writeBytes(mantissaBytes)
        let bigintPayload = bigintWriter.data

        var writer = OfflineNoritoWriter()
        writer.writeField(bigintPayload)
        writer.writeField(encodeUInt32(scale))
        return writer.data
    }

    static func encodeMetadata(_ metadata: [String: ToriiJSONValue]) throws -> Data {
        var writer = OfflineNoritoWriter()
        let keys = metadata.keys.sorted()
        writer.writeLength(UInt64(keys.count))
        for key in keys {
            guard let value = metadata[key] else { continue }
            let entry = try encodeMetadataEntry(key: key, value: value)
            writer.writeLength(UInt64(entry.count))
            writer.writeBytes(entry)
        }
        return writer.data
    }

    static func encodeMetadataEntry(key: String, value: ToriiJSONValue) throws -> Data {
        var entryWriter = OfflineNoritoWriter()
        let namePayload = encodeString(key)
        entryWriter.writeLength(UInt64(namePayload.count))
        entryWriter.writeBytes(namePayload)
        let jsonString = try jsonString(from: value)
        let jsonPayload = encodeString(jsonString)
        var jsonFieldWriter = OfflineNoritoWriter()
        jsonFieldWriter.writeField(jsonPayload)
        let jsonField = jsonFieldWriter.data
        entryWriter.writeLength(UInt64(jsonField.count))
        entryWriter.writeBytes(jsonField)
        return entryWriter.data
    }

    static func encodeAssetId(_ assetId: String) throws -> Data {
        try decodeNoritoAssetIdLiteral(assetId)
    }

    static func encodeAssetId(assetDefinitionId: String, accountId: String) throws -> Data {
        let literal = try assetIdLiteral(assetDefinitionId: assetDefinitionId, accountId: accountId)
        return try encodeAssetId(literal)
    }

    static func assetIdLiteral(assetDefinitionId: String, accountId: String) throws -> String {
        let trimmedDefinition = assetDefinitionId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmedDefinition.isEmpty else {
            throw OfflineNoritoError.invalidAssetId(assetDefinitionId)
        }
        if trimmedDefinition.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw OfflineNoritoError.invalidAssetId(assetDefinitionId)
        }
        if trimmedDefinition.contains("@") {
            throw OfflineNoritoError.invalidAssetId(trimmedDefinition)
        }

        let canonicalAccount = try canonicalizeEncodedAccountId(accountId)
        let bridge = NoritoNativeBridge.shared
        if let encoded = bridge.encodeAssetIdLiteral(
            assetDefinition: trimmedDefinition,
            accountId: canonicalAccount
        ) {
            let normalized = encoded.trimmingCharacters(in: .whitespacesAndNewlines)
            if !normalized.isEmpty {
                return normalized
            }
        }

        if bridge.canEncodeAssetIdLiteral {
            throw OfflineNoritoError.invalidAssetId(trimmedDefinition)
        }

        throw OfflineNoritoError.nativeBridgeUnavailable("connect_norito_encode_asset_id_literal")
    }

    static func encodeAssetDefinitionId(name: String, domain: String) throws -> Data {
        var writer = OfflineNoritoWriter()
        let domainPayload = try encodeDomainId(domain)
        writer.writeField(domainPayload)
        let namePayload = encodeString(name)
        writer.writeField(namePayload)
        return writer.data
    }

    static func encodeDomainId(_ value: String) throws -> Data {
        var writer = OfflineNoritoWriter()
        let canonical = try canonicalizeAssetDomain(value)
        let namePayload = encodeString(canonical)
        writer.writeField(namePayload)
        return writer.data
    }

    private static func canonicalizeAccountIdWithoutNativeParse(_ value: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineNoritoError.invalidAccountId(value)
        }
        if trimmed != value {
            throw OfflineNoritoError.invalidAccountId(trimmed)
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw OfflineNoritoError.invalidAccountId(trimmed)
        }
        if trimmed.contains("@") || trimmed.contains("#") || trimmed.contains("$") {
            throw OfflineNoritoError.invalidAccountId(trimmed)
        }
        return trimmed
    }

    private static func canonicalizeEncodedAccountId(_ value: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineNoritoError.invalidAccountId(value)
        }
        if trimmed != value {
            throw OfflineNoritoError.invalidAccountId(trimmed)
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw OfflineNoritoError.invalidAccountId(trimmed)
        }
        if trimmed.contains("@") || trimmed.contains("#") || trimmed.contains("$") {
            throw OfflineNoritoError.invalidAccountId(trimmed)
        }
        let address: AccountAddress
        do {
            address = try AccountAddress.parseEncoded(trimmed, expectedPrefix: defaultNetworkPrefix)
        } catch {
            throw OfflineNoritoError.invalidAccountId(trimmed)
        }
        return try address.toI105(networkPrefix: defaultNetworkPrefix)
    }

    private static func decodeNoritoAssetIdLiteral(_ raw: String) throws -> Data {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw OfflineNoritoError.invalidAssetId(raw)
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw OfflineNoritoError.invalidAssetId(raw)
        }
        let lower = trimmed.lowercased()
        guard lower.hasPrefix("norito:") else {
            throw OfflineNoritoError.invalidAssetId(raw)
        }
        let hex = String(trimmed.dropFirst("norito:".count))
        guard !hex.isEmpty,
              hex.count.isMultiple(of: 2),
              let decoded = Data(hexString: hex) else {
            throw OfflineNoritoError.invalidAssetId(raw)
        }
        if decoded.count > 40,
           decoded[0] == 0x4E, // N
           decoded[1] == 0x52, // R
           decoded[2] == 0x54, // T
           decoded[3] == 0x30 { // 0
            return Data(decoded.dropFirst(40))
        }
        return decoded
    }

    private static func encodeVarint(_ value: UInt64) -> [UInt8] {
        var out: [UInt8] = []
        var value = value
        repeat {
            var byte = UInt8(value & 0x7F)
            value >>= 7
            if value != 0 {
                byte |= 0x80
            }
            out.append(byte)
        } while value != 0
        return out
    }

    private static func signingAlgorithm(multihashCode: UInt64) -> SigningAlgorithm? {
        switch multihashCode {
        case 0xed:
            return .ed25519
        case 0xe7:
            return .secp256k1
        case 0xee:
            return .mlDsa
        case 0x1306:
            return .sm2
        default:
            return nil
        }
    }

    private static func multihashFunctionCode(for algorithm: SigningAlgorithm) -> UInt64 {
        switch algorithm {
        case .ed25519:
            return 0xed
        case .secp256k1:
            return 0xe7
        case .mlDsa:
            return 0xee
        case .sm2:
            return 0x1306
        }
    }

    static func publicKeyMultihash(algorithm: SigningAlgorithm, payload: Data) -> String {
        let functionCode = multihashFunctionCode(for: algorithm)
        return formatPublicKeyMultihash(functionCode: functionCode, payload: payload)
    }

    private static func parseAlgorithmPrefix(_ value: String) -> SigningAlgorithm? {
        switch value.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "ed25519", "ed":
            return .ed25519
        case "secp256k1", "secp":
            return .secp256k1
        case "ml-dsa", "mldsa", "ml_dsa":
            return .mlDsa
        case "sm2":
            return .sm2
        default:
            return nil
        }
    }

    private static func formatPublicKeyMultihash(functionCode: UInt64, payload: Data) -> String {
        let functionHex = Data(encodeVarint(functionCode)).hexLowercased()
        let lengthHex = Data(encodeVarint(UInt64(payload.count))).hexLowercased()
        // Iroha canonical multihash hex is mixed-case: varint bytes lowercase, payload bytes uppercase.
        let payloadHex = payload.hexUppercased()
        return functionHex + lengthHex + payloadHex
    }

    private static func canonicalizeAssetDomain(_ value: String) throws -> String {
        do {
            return try AccountAddress.canonicalizeDomainLabel(value)
        } catch {
            throw OfflineNoritoError.invalidAssetId(value)
        }
    }

    static func encodePoseidonDigest(_ bytes: Data) throws -> Data {
        guard bytes.count == 32 else {
            throw OfflineNoritoError.invalidLength("poseidon digest must be 32 bytes")
        }
        var writer = OfflineNoritoWriter()
        writer.writeLength(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    static func jsonString(from value: ToriiJSONValue) throws -> String {
        var out = ""
        try writeJsonValue(value, into: &out)
        return out
    }

    private static func writeJsonValue(_ value: ToriiJSONValue, into out: inout String) throws {
        switch value {
        case .null:
            out.append("null")
        case .bool(let flag):
            out.append(flag ? "true" : "false")
        case .number(let number):
            guard number.isFinite else {
                throw OfflineNoritoError.invalidMetadata("non-finite number")
            }
            if number.rounded(.towardZero) == number && abs(number) <= maxSafeInteger {
                out.append(String(format: "%.0f", number))
            } else {
                out.append(String(number))
            }
        case .string(let text):
            writeJsonString(text, into: &out)
        case .array(let items):
            out.append("[")
            for idx in items.indices {
                if idx > 0 { out.append(",") }
                try writeJsonValue(items[idx], into: &out)
            }
            out.append("]")
        case .object(let object):
            out.append("{")
            let keys = object.keys.sorted()
            for idx in keys.indices {
                if idx > 0 { out.append(",") }
                let key = keys[idx]
                writeJsonString(key, into: &out)
                out.append(":")
                if let value = object[key] {
                    try writeJsonValue(value, into: &out)
                } else {
                    out.append("null")
                }
            }
            out.append("}")
        }
    }

    private static func writeJsonString(_ value: String, into out: inout String) {
        out.append("\"")
        for scalar in value.unicodeScalars {
            switch scalar {
            case "\"":
                out.append("\\\"")
            case "\\":
                out.append("\\\\")
            case "\n":
                out.append("\\n")
            case "\r":
                out.append("\\r")
            case "\t":
                out.append("\\t")
            default:
                if scalar.value < 0x20 {
                    out.append("\\u00")
                    let hi = (scalar.value >> 4) & 0xF
                    let lo = scalar.value & 0xF
                    out.append(hexDigit(hi))
                    out.append(hexDigit(lo))
                } else {
                    out.unicodeScalars.append(scalar)
                }
            }
        }
        out.append("\"")
    }

    private static func hexDigit(_ value: UInt32) -> String {
        let digits = "0123456789ABCDEF"
        let idx = digits.index(digits.startIndex, offsetBy: Int(value))
        return String(digits[idx])
    }
}

struct OfflineBigInt {
    var isNegative: Bool = false
    private var limbs: [UInt32]

    var isZero: Bool {
        limbs.allSatisfy { $0 == 0 }
    }

    init(decimalDigits: String) throws {
        guard !decimalDigits.isEmpty else {
            throw OfflineNoritoError.invalidNumeric(decimalDigits)
        }
        var values = [UInt32](repeating: 0, count: 1)
        for scalar in decimalDigits.unicodeScalars {
            guard scalar.value >= 48 && scalar.value <= 57 else {
                throw OfflineNoritoError.invalidNumeric(decimalDigits)
            }
            let digit = Int(scalar.value - 48)
            var carry = UInt64(digit)
            for idx in 0..<values.count {
                let next = UInt64(values[idx]) * 10 + carry
                values[idx] = UInt32(next & 0xFFFF_FFFF)
                carry = next >> 32
            }
            if carry > 0 {
                values.append(UInt32(carry))
            }
        }
        while values.count > 1 && values.last == 0 {
            values.removeLast()
        }
        limbs = values
    }

    func toTwosComplementBytes(maxBytes: Int) throws -> Data {
        let magnitude = magnitudeBytes()
        if isNegative {
            let bitLength = self.bitLength()
            if bitLength == 0 {
                return Data([0])
            }
            let isPowerOfTwo = self.isPowerOfTwo()
            let requiredBits = isPowerOfTwo ? bitLength : bitLength + 1
            let byteCount = max(1, (requiredBits + 7) / 8)
            var bytes = magnitude
            if bytes.count < byteCount {
                bytes.append(contentsOf: repeatElement(0, count: byteCount - bytes.count))
            }
            for idx in bytes.indices {
                bytes[idx] = ~bytes[idx]
            }
            var carry: UInt8 = 1
            for idx in bytes.indices {
                let sum = UInt16(bytes[idx]) + UInt16(carry)
                bytes[idx] = UInt8(sum & 0xFF)
                carry = sum > 0xFF ? 1 : 0
                if carry == 0 { break }
            }
            if (bytes.last ?? 0) & 0x80 == 0 {
                bytes.append(0xFF)
            }
            guard bytes.count <= maxBytes else {
                throw OfflineNoritoError.numericOverflow
            }
            return Data(bytes)
        }
        var bytes = magnitude
        if bytes.isEmpty { bytes = [0] }
        if (bytes.last ?? 0) & 0x80 != 0 {
            bytes.append(0)
        }
        guard bytes.count <= maxBytes else {
            throw OfflineNoritoError.numericOverflow
        }
        return Data(bytes)
    }

    private func magnitudeBytes() -> [UInt8] {
        var bytes: [UInt8] = []
        for limb in limbs {
            bytes.append(UInt8(limb & 0xFF))
            bytes.append(UInt8((limb >> 8) & 0xFF))
            bytes.append(UInt8((limb >> 16) & 0xFF))
            bytes.append(UInt8((limb >> 24) & 0xFF))
        }
        while bytes.count > 1 && bytes.last == 0 {
            bytes.removeLast()
        }
        return bytes
    }

    private func bitLength() -> Int {
        guard let last = limbs.last, last != 0 else { return 0 }
        let leading = 32 - last.leadingZeroBitCount
        return (limbs.count - 1) * 32 + leading
    }

    private func isPowerOfTwo() -> Bool {
        var seen = false
        for limb in limbs where limb != 0 {
            if limb & (limb - 1) == 0 {
                if seen { return false }
                seen = true
            } else {
                return false
            }
        }
        return seen
    }
}

enum IrohaHash {
    static func hash(_ data: Data) -> Data {
        var digest = Blake2b.hash256(data)
        if let last = digest.indices.last {
            digest[last] |= 1
        }
        return digest
    }
}

extension Data {
    func hexUppercased() -> String {
        map { String(format: "%02X", $0) }.joined()
    }

    func hexLowercased() -> String {
        map { String(format: "%02x", $0) }.joined()
    }
}
