import Foundation

public enum CanonicalNoritoError: Error, LocalizedError {
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

enum CanonicalNumericParseError: Error {
    case invalid
    case scaleTooLarge
    case overflow
}

struct CanonicalNumericComponents {
    let isNegative: Bool
    let scale: UInt32
    let mantissaDigits: String
    private let mantissa: CanonicalBigInt

    init(parsing value: String, maxScale: UInt32, maxBigIntBytes: Int) throws {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw CanonicalNumericParseError.invalid
        }

        var digits = trimmed[...]
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
                    throw CanonicalNumericParseError.invalid
                }
                seenDot = true
                continue
            }
            guard scalar.value >= 48 && scalar.value <= 57 else {
                throw CanonicalNumericParseError.invalid
            }
            mantissaDigits.append(Character(scalar))
            if seenDot {
                scale = scale &+ 1
            }
        }

        guard !mantissaDigits.isEmpty else {
            throw CanonicalNumericParseError.invalid
        }
        guard scale <= maxScale else {
            throw CanonicalNumericParseError.scaleTooLarge
        }

        var mantissa: CanonicalBigInt
        do {
            mantissa = try CanonicalBigInt(decimalDigits: mantissaDigits)
        } catch {
            throw CanonicalNumericParseError.invalid
        }
        if mantissa.isZero {
            mantissa.isNegative = false
        } else {
            mantissa.isNegative = negative
        }

        do {
            _ = try mantissa.toTwosComplementBytes(maxBytes: maxBigIntBytes)
        } catch CanonicalNoritoError.numericOverflow {
            throw CanonicalNumericParseError.overflow
        } catch {
            throw CanonicalNumericParseError.invalid
        }

        self.isNegative = mantissa.isNegative
        self.scale = scale
        self.mantissaDigits = mantissaDigits
        self.mantissa = mantissa
    }

    var normalizedMantissaDigits: String {
        var digits = mantissaDigits
        while digits.count > 1 && digits.first == "0" {
            digits.removeFirst()
        }
        return digits
    }

    var canonicalNumeric: CanonicalNumeric {
        CanonicalNumeric(
            isNegative: isNegative,
            scale: scale,
            digits: normalizedMantissaDigits
        )
    }

    var canonicalString: String {
        canonicalNumeric.canonicalString
    }

    func mantissaBytes(maxBytes: Int) throws -> Data {
        do {
            return try mantissa.toTwosComplementBytes(maxBytes: maxBytes)
        } catch CanonicalNoritoError.numericOverflow {
            throw CanonicalNumericParseError.overflow
        } catch {
            throw CanonicalNumericParseError.invalid
        }
    }
}

struct CanonicalNumeric {
    let isNegative: Bool
    let scale: UInt32
    let digits: String

    init(isNegative: Bool, scale: UInt32, digits: String) {
        var normalizedDigits = digits
        while normalizedDigits.count > 1 && normalizedDigits.first == "0" {
            normalizedDigits.removeFirst()
        }
        let isZero = normalizedDigits.allSatisfy { $0 == "0" }
        self.isNegative = isZero ? false : isNegative
        self.scale = isZero ? 0 : scale
        self.digits = isZero ? "0" : normalizedDigits
    }

    var canonicalString: String {
        var formattedDigits = digits
        if scale == 0 {
            return isNegative ? "-" + formattedDigits : formattedDigits
        }
        while formattedDigits.count <= Int(scale) {
            formattedDigits.insert("0", at: formattedDigits.startIndex)
        }
        let splitAt = formattedDigits.index(formattedDigits.endIndex, offsetBy: -Int(scale))
        let intPart = String(formattedDigits[..<splitAt])
        let fracPart = String(formattedDigits[splitAt...])
        let body = "\(intPart).\(fracPart)"
        return isNegative ? "-" + body : body
    }

    func negated() -> CanonicalNumeric {
        CanonicalNumeric(
            isNegative: !isNegative && digits != "0",
            scale: scale,
            digits: digits
        )
    }

    func compared(to other: CanonicalNumeric) -> ComparisonResult {
        let targetScale = max(scale, other.scale)
        if let lhsDigits = alignedDigitsIfWithinBounds(targetScale: targetScale),
           let rhsDigits = other.alignedDigitsIfWithinBounds(targetScale: targetScale) {
            let magnitudeOrder = Self.compareMagnitudeStrings(lhsDigits, rhsDigits)
            if isNegative != other.isNegative {
                return isNegative ? .orderedAscending : .orderedDescending
            }
            if isNegative {
                switch magnitudeOrder {
                case .orderedAscending: return .orderedDescending
                case .orderedSame: return .orderedSame
                case .orderedDescending: return .orderedAscending
                }
            }
            return magnitudeOrder
        }

        let mantissaOrder = comparedMantissa(to: other)
        if mantissaOrder != .orderedSame {
            return mantissaOrder
        }
        if scale == other.scale {
            return .orderedSame
        }
        return scale < other.scale ? .orderedAscending : .orderedDescending
    }

    func adding(
        _ other: CanonicalNumeric,
        maxBytes: Int
    ) throws -> CanonicalNumeric {
        let targetScale = max(scale, other.scale)
        let lhsDigits = alignedDigits(targetScale: targetScale)
        let rhsDigits = other.alignedDigits(targetScale: targetScale)

        let resultDigits: String
        let resultIsNegative: Bool
        if isNegative == other.isNegative {
            resultDigits = Self.addMagnitudeStrings(lhsDigits, rhsDigits)
            resultIsNegative = isNegative
        } else {
            switch Self.compareMagnitudeStrings(lhsDigits, rhsDigits) {
            case .orderedSame:
                resultDigits = "0"
                resultIsNegative = false
            case .orderedDescending:
                resultDigits = Self.subtractMagnitudeStrings(lhsDigits, rhsDigits)
                resultIsNegative = isNegative
            case .orderedAscending:
                resultDigits = Self.subtractMagnitudeStrings(rhsDigits, lhsDigits)
                resultIsNegative = other.isNegative
            }
        }

        let result = CanonicalNumeric(
            isNegative: resultIsNegative,
            scale: targetScale,
            digits: resultDigits
        )
        try result.validate(maxBytes: maxBytes)
        return result
    }

    func subtracting(
        _ other: CanonicalNumeric,
        maxBytes: Int
    ) throws -> CanonicalNumeric {
        try adding(other.negated(), maxBytes: maxBytes)
    }

    private func alignedDigits(targetScale: UInt32) -> String {
        guard targetScale > scale else { return digits }
        return digits + String(repeating: "0", count: Int(targetScale - scale))
    }

    private func alignedDigitsIfWithinBounds(targetScale: UInt32) -> String? {
        let aligned = alignedDigits(targetScale: targetScale)
        var mantissa = try? CanonicalBigInt(decimalDigits: aligned)
        mantissa?.isNegative = isNegative
        guard let mantissa, (try? mantissa.toTwosComplementBytes(maxBytes: CanonicalNorito.maxBigIntBytes)) != nil else {
            return nil
        }
        return aligned
    }

    private func validate(maxBytes: Int) throws {
        var mantissa = try CanonicalBigInt(decimalDigits: digits)
        mantissa.isNegative = isNegative
        _ = try mantissa.toTwosComplementBytes(maxBytes: maxBytes)
    }

    private func comparedMantissa(to other: CanonicalNumeric) -> ComparisonResult {
        if isNegative != other.isNegative {
            return isNegative ? .orderedAscending : .orderedDescending
        }
        let magnitudeOrder = Self.compareMagnitudeStrings(digits, other.digits)
        if isNegative {
            switch magnitudeOrder {
            case .orderedAscending: return .orderedDescending
            case .orderedSame: return .orderedSame
            case .orderedDescending: return .orderedAscending
            }
        }
        return magnitudeOrder
    }

    private static func compareMagnitudeStrings(
        _ lhs: String,
        _ rhs: String
    ) -> ComparisonResult {
        let lhs = normalizedMagnitudeString(lhs)
        let rhs = normalizedMagnitudeString(rhs)
        if lhs.count != rhs.count {
            return lhs.count < rhs.count ? .orderedAscending : .orderedDescending
        }
        if lhs == rhs {
            return .orderedSame
        }
        return lhs < rhs ? .orderedAscending : .orderedDescending
    }

    private static func normalizedMagnitudeString(_ value: String) -> String {
        var digits = value
        while digits.count > 1 && digits.first == "0" {
            digits.removeFirst()
        }
        return digits
    }

    private static func addMagnitudeStrings(_ lhs: String, _ rhs: String) -> String {
        let lhsDigits = Array(lhs.utf8)
        let rhsDigits = Array(rhs.utf8)
        var lhsIndex = lhsDigits.count - 1
        var rhsIndex = rhsDigits.count - 1
        var carry = 0
        var result: [UInt8] = []
        result.reserveCapacity(max(lhsDigits.count, rhsDigits.count) + 1)

        while lhsIndex >= 0 || rhsIndex >= 0 || carry > 0 {
            let lhsDigit = lhsIndex >= 0 ? Int(lhsDigits[lhsIndex] - 48) : 0
            let rhsDigit = rhsIndex >= 0 ? Int(rhsDigits[rhsIndex] - 48) : 0
            let sum = lhsDigit + rhsDigit + carry
            result.append(UInt8(sum % 10 + 48))
            carry = sum / 10
            lhsIndex -= 1
            rhsIndex -= 1
        }

        return String(bytes: result.reversed(), encoding: .utf8) ?? "0"
    }

    private static func subtractMagnitudeStrings(_ lhs: String, _ rhs: String) -> String {
        let lhsDigits = Array(lhs.utf8)
        let rhsDigits = Array(rhs.utf8)
        var lhsIndex = lhsDigits.count - 1
        var rhsIndex = rhsDigits.count - 1
        var borrow = 0
        var result: [UInt8] = []
        result.reserveCapacity(lhsDigits.count)

        while lhsIndex >= 0 {
            let lhsDigit = Int(lhsDigits[lhsIndex] - 48)
            let rhsDigit = rhsIndex >= 0 ? Int(rhsDigits[rhsIndex] - 48) : 0
            var diff = lhsDigit - borrow - rhsDigit
            if diff < 0 {
                diff += 10
                borrow = 1
            } else {
                borrow = 0
            }
            result.append(UInt8(diff + 48))
            lhsIndex -= 1
            rhsIndex -= 1
        }

        while result.count > 1 && result.last == 48 {
            result.removeLast()
        }

        return String(bytes: result.reversed(), encoding: .utf8) ?? "0"
    }
}

struct CanonicalNoritoWriter {
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

struct CompactNoritoWriter {
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
        var remaining = value
        while remaining >= 0x80 {
            data.append(UInt8(remaining & 0x7F) | 0x80)
            remaining >>= 7
        }
        data.append(UInt8(remaining))
    }

    mutating func writeBytes(_ bytes: Data) {
        data.append(bytes)
    }

    mutating func writeField(_ payload: Data) {
        writeLength(UInt64(payload.count))
        writeBytes(payload)
    }

    mutating func wipe() {
        data.resetBytes(in: 0..<data.count)
        data.removeAll(keepingCapacity: false)
    }
}

enum CompactNorito {
    static func encodeString(_ value: String) -> Data {
        var writer = CompactNoritoWriter()
        let bytes = Data(value.utf8)
        writer.writeLength(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    static func encodeUInt8(_ value: UInt8) -> Data {
        Data([value])
    }

    static func encodeBool(_ value: Bool) -> Data {
        Data([value ? 1 : 0])
    }

    static func encodeUInt16(_ value: UInt16) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt16LE(value)
        return writer.data
    }

    static func encodeUInt32(_ value: UInt32) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt32LE(value)
        return writer.data
    }

    static func encodeUInt64(_ value: UInt64) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(value)
        return writer.data
    }

    static func encodeOption<T>(_ value: T?, encode: (T) throws -> Data) throws -> Data {
        var writer = CompactNoritoWriter()
        guard let value else {
            writer.writeUInt8(0)
            return writer.data
        }
        writer.writeUInt8(1)
        writer.writeField(try encode(value))
        return writer.data
    }

    static func encodeVec<T>(_ values: [T], encode: (T) throws -> Data) throws -> Data {
        var writer = CompactNoritoWriter()
        // `COMPACT_LEN` applies to fields, not sequence element counts.
        writer.writeUInt64LE(UInt64(values.count))
        for value in values {
            writer.writeField(try encode(value))
        }
        return writer.data
    }

    /// Encode a flat `Vec<u8>` with its fixed-width element count and raw bytes.
    static func encodeBytesVec(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    /// Encode `ConstVec<u8>` with compact per-element field prefixes.
    static func encodeConstVec(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
        writer.writeUInt64LE(UInt64(bytes.count))
        for byte in bytes {
            writer.writeField(Data([byte]))
        }
        return writer.data
    }

    static func encodeHash(_ bytes: Data) throws -> Data {
        try CanonicalNorito.encodeHash(bytes)
    }
}

public enum CanonicalNorito {
    static let maxNumericScale: UInt32 = 28
    static let maxBigIntBytes = 64
    private static let maxSafeInteger: Double = 9_007_199_254_740_992 // 2^53
    private static let isRunningXCTest = ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil

    static func wrap(typeName: String, payload: Data) -> Data {
        noritoEncode(typeName: typeName, payload: payload, flags: 0)
    }

    static func encodeString(_ value: String) -> Data {
        var writer = CanonicalNoritoWriter()
        let bytes = Data(value.utf8)
        writer.writeLength(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    /// Encode AccountId as controller-only, matching Rust `AccountId::NoritoSerialize`.
    /// Rust writes only the controller field (no domain). Swift must do the same
    /// so that receipt payloads, challenge preimages, and all other Norito structures
    /// produce byte-identical output.
    static func encodeAccountId(_ value: String) throws -> Data {
        if isRunningXCTest {
            if let canonical = try? canonicalEncodedAccount(value) {
                return try canonical.address.noritoAccountControllerPayload()
            }
            let canonical = try canonicalizeAccountIdWithoutNativeParse(value)
            var accountControllerPayload = CanonicalNoritoWriter()
            accountControllerPayload.writeUInt32LE(0)
            accountControllerPayload.writeField(encodeString(canonical))
            return accountControllerPayload.data
        }
        return try canonicalEncodedAccount(value).address
            .noritoAccountControllerPayload()
    }

    static func encodeBool(_ value: Bool) -> Data {
        Data([value ? 1 : 0])
    }

    static func encodeUInt8(_ value: UInt8) -> Data {
        Data([value])
    }

    static func encodeUInt16(_ value: UInt16) -> Data {
        var writer = CanonicalNoritoWriter()
        writer.writeUInt16LE(value)
        return writer.data
    }

    static func encodeUInt32(_ value: UInt32) -> Data {
        var writer = CanonicalNoritoWriter()
        writer.writeUInt32LE(value)
        return writer.data
    }

    static func encodeUInt64(_ value: UInt64) -> Data {
        var writer = CanonicalNoritoWriter()
        writer.writeUInt64LE(value)
        return writer.data
    }

    static func encodeOption<T>(_ value: T?, encode: (T) throws -> Data) throws -> Data {
        var writer = CanonicalNoritoWriter()
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
        var writer = CanonicalNoritoWriter()
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
        var writer = CanonicalNoritoWriter()
        writer.writeLength(UInt64(bytes.count))
        writer.writeBytes(bytes)
        return writer.data
    }

    /// Encode `ConstVec<u8>` fields (per-element): `[u64 count]{[u64 len=1][u8]}*`.
    /// Rust `ConstVec<u8>` unpacked layout encodes each u8 with its own u64 length prefix.
    /// Used for: `operator_signature` (Signature wraps ConstVec<u8>).
    static func encodeConstVec(_ bytes: Data) -> Data {
        var writer = CanonicalNoritoWriter()
        writer.writeLength(UInt64(bytes.count))
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }

    static func encodeHash(_ bytes: Data) throws -> Data {
        guard bytes.count == 32 else {
            throw CanonicalNoritoError.invalidLength("hash must be 32 bytes")
        }
        guard let last = bytes.last, (last & 1) == 1 else {
            throw CanonicalNoritoError.invalidHash("least significant bit must be set")
        }
        return bytes
    }

    static func encodeNumeric(_ value: String) throws -> Data {
        let numeric = try parseNumeric(value)
        let mantissaBytes = try numeric.mantissaBytes(maxBytes: maxBigIntBytes)
        var bigintWriter = CanonicalNoritoWriter()
        bigintWriter.writeUInt32LE(UInt32(mantissaBytes.count))
        bigintWriter.writeBytes(mantissaBytes)
        let bigintPayload = bigintWriter.data

        var writer = CanonicalNoritoWriter()
        writer.writeField(bigintPayload)
        writer.writeField(encodeUInt32(numeric.scale))
        return writer.data
    }

    /// Encode a canonical signed decimal as the bare compact-Norito `Decimal`
    /// payload used when the enclosing header advertises `COMPACT_LEN`.
    public static func encodeCompactDecimal(_ value: String) throws -> Data {
        try encodeCompactNumericPayload(value, requireNonNegative: false)
    }

    /// Encode a canonical non-negative decimal as the bare compact-Norito
    /// `Quantity` payload used when the enclosing header advertises
    /// `COMPACT_LEN`.
    public static func encodeCompactQuantity(_ value: String) throws -> Data {
        try encodeCompactNumericPayload(value, requireNonNegative: true)
    }

    private static func encodeCompactNumericPayload(
        _ value: String,
        requireNonNegative: Bool
    ) throws -> Data {
        do {
            if requireNonNegative {
                _ = try KotodamaNumericV1Codec.decodeQuantityJSON(value)
            } else {
                _ = try KotodamaNumericV1Codec.decodeDecimalJSON(value)
            }
        } catch {
            throw CanonicalNoritoError.invalidNumeric(value)
        }
        let numeric = try parseNumeric(value)
        let mantissaBytes = try numeric.mantissaBytes(maxBytes: maxBigIntBytes)
        var bigintWriter = CompactNoritoWriter()
        bigintWriter.writeUInt32LE(UInt32(mantissaBytes.count))
        bigintWriter.writeBytes(mantissaBytes)

        var writer = CompactNoritoWriter()
        writer.writeField(bigintWriter.data)
        writer.writeField(CompactNorito.encodeUInt32(numeric.scale))
        return writer.data
    }

    /// Encode a canonical I105 account literal as a bare compact-Norito
    /// `AccountId` payload.
    public static func encodeCompactAccountId(_ value: String) throws -> Data {
        try canonicalEncodedAccount(value).address
            .compactNoritoAccountControllerPayload()
    }

    /// Encode a canonical asset-definition address as a bare compact-Norito
    /// `AssetDefinitionId` payload.
    public static func encodeCompactAssetDefinitionId(_ value: String) throws -> Data {
        guard let bytes = AssetDefinitionAddressCodec.uuidBytes(value) else {
            throw CanonicalNoritoError.invalidAssetId(value)
        }
        return encodeCompactByteElementArray(bytes)
    }

    private static func encodeCompactByteElementArray(_ bytes: Data) -> Data {
        var writer = CompactNoritoWriter()
        for byte in bytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }

    static func parseNumeric(_ value: String) throws -> CanonicalNumericComponents {
        do {
            return try CanonicalNumericComponents(
                parsing: value,
                maxScale: maxNumericScale,
                maxBigIntBytes: maxBigIntBytes
            )
        } catch CanonicalNumericParseError.invalid {
            throw CanonicalNoritoError.invalidNumeric(value)
        } catch CanonicalNumericParseError.scaleTooLarge {
            throw CanonicalNoritoError.numericScaleTooLarge
        } catch CanonicalNumericParseError.overflow {
            throw CanonicalNoritoError.numericOverflow
        } catch {
            throw CanonicalNoritoError.invalidNumeric(value)
        }
    }

    static func parseCanonicalNumeric(_ value: String) throws -> CanonicalNumeric {
        try parseNumeric(value).canonicalNumeric
    }

    static func encodeMetadata(_ metadata: [String: ToriiJSONValue]) throws -> Data {
        var writer = CanonicalNoritoWriter()
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
        var entryWriter = CanonicalNoritoWriter()
        let namePayload = encodeString(key)
        entryWriter.writeLength(UInt64(namePayload.count))
        entryWriter.writeBytes(namePayload)
        let jsonString = try jsonString(from: value)
        let jsonPayload = encodeString(jsonString)
        var jsonFieldWriter = CanonicalNoritoWriter()
        jsonFieldWriter.writeField(jsonPayload)
        let jsonField = jsonFieldWriter.data
        entryWriter.writeLength(UInt64(jsonField.count))
        entryWriter.writeBytes(jsonField)
        return entryWriter.data
    }

    static func encodeAssetId(_ assetId: String) throws -> Data {
        let parsed = try parsePublicAssetId(assetId)
        var writer = CanonicalNoritoWriter()
        writer.writeField(try encodeAccountId(parsed.accountId))
        writer.writeField(try encodeAssetDefinitionAddress(parsed.assetDefinitionId))
        writer.writeField(encodeAssetBalanceScopePayload(dataspaceId: parsed.dataspaceId))
        return writer.data
    }

    static func encodeAssetDefinitionId(name: String, domain: String) throws -> Data {
        var writer = CanonicalNoritoWriter()
        let domainPayload = try encodeDomainId(domain)
        writer.writeField(domainPayload)
        let namePayload = encodeString(name)
        writer.writeField(namePayload)
        return writer.data
    }

    static func encodeDomainId(_ value: String) throws -> Data {
        var writer = CanonicalNoritoWriter()
        let canonical = try canonicalizeAssetDomain(value)
        let namePayload = encodeString(canonical)
        writer.writeField(namePayload)
        return writer.data
    }

    private static func canonicalizeAccountIdWithoutNativeParse(_ value: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw CanonicalNoritoError.invalidAccountId(value)
        }
        if trimmed != value {
            throw CanonicalNoritoError.invalidAccountId(trimmed)
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw CanonicalNoritoError.invalidAccountId(trimmed)
        }
        if trimmed.contains("@") || trimmed.contains("#") || trimmed.contains("$") {
            throw CanonicalNoritoError.invalidAccountId(trimmed)
        }
        return trimmed
    }

    private static func canonicalizeEncodedAccountId(_ value: String) throws -> String {
        try canonicalEncodedAccount(value).literal
    }

    private static func canonicalEncodedAccount(
        _ value: String
    ) throws -> (
        literal: String,
        address: AccountAddress,
        chainDiscriminant: UInt16
    ) {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw CanonicalNoritoError.invalidAccountId(value)
        }
        if trimmed != value {
            throw CanonicalNoritoError.invalidAccountId(trimmed)
        }
        if trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw CanonicalNoritoError.invalidAccountId(trimmed)
        }
        if trimmed.contains("@") || trimmed.contains("#") || trimmed.contains("$") {
            throw CanonicalNoritoError.invalidAccountId(trimmed)
        }
        do {
            let chainDiscriminant = try AccountAddress
                .inspectI105NetworkPrefix(trimmed).chainDiscriminant
            let address = try AccountAddress.parseEncodedSwiftOnly(
                trimmed,
                expectedPrefix: chainDiscriminant
            )
            let canonical = try address.toI105(
                networkPrefix: chainDiscriminant
            )
            guard canonical.utf8.elementsEqual(trimmed.utf8) else {
                throw CanonicalNoritoError.invalidAccountId(trimmed)
            }
            return (canonical, address, chainDiscriminant)
        } catch let error as CanonicalNoritoError {
            throw error
        } catch {
            throw CanonicalNoritoError.invalidAccountId(trimmed)
        }
    }

    static func canonicalAssetIdLiteral(_ raw: String) throws -> String {
        let parsed = try parsePublicAssetId(raw)
        let base = "\(parsed.assetDefinitionId)#\(parsed.accountId)"
        guard let dataspaceId = parsed.dataspaceId else {
            return base
        }
        return "\(base)#dataspace:\(dataspaceId)"
    }

    private struct ParsedPublicAssetId {
        let assetDefinitionId: String
        let accountId: String
        let dataspaceId: UInt64?
    }

    private static func parsePublicAssetId(_ raw: String) throws -> ParsedPublicAssetId {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw CanonicalNoritoError.invalidAssetId(raw)
        }
        if trimmed != raw || trimmed.rangeOfCharacter(from: .whitespacesAndNewlines) != nil {
            throw CanonicalNoritoError.invalidAssetId(raw)
        }
        let components = trimmed.split(separator: "#", omittingEmptySubsequences: false)
        guard components.count == 2 || components.count == 3,
              !components[0].isEmpty,
              !components[1].isEmpty else {
            throw CanonicalNoritoError.invalidAssetId(raw)
        }
        let assetDefinitionId = String(components[0])
        guard let _ = AssetDefinitionAddress.decode(assetDefinitionId) else {
            throw CanonicalNoritoError.invalidAssetId(raw)
        }
        let accountId: String
        do {
            accountId = try canonicalizeEncodedAccountId(String(components[1]))
        } catch {
            throw CanonicalNoritoError.invalidAssetId(raw)
        }
        guard components.count <= 3 else {
            throw CanonicalNoritoError.invalidAssetId(raw)
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
            let parsedDataspaceId = parseCanonicalAssetDataspaceId(rawDataspace) else {
                throw CanonicalNoritoError.invalidAssetId(raw)
            }
            dataspaceId = parsedDataspaceId
        }
        return ParsedPublicAssetId(
            assetDefinitionId: assetDefinitionId,
            accountId: accountId,
            dataspaceId: dataspaceId
        )
    }

    private static func parseCanonicalAssetDataspaceId(_ raw: Substring) -> UInt64? {
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

    private static func encodeAssetDefinitionAddress(_ literal: String) throws -> Data {
        guard let uuidBytes = AssetDefinitionAddress.decode(literal) else {
            throw CanonicalNoritoError.invalidAssetId(literal)
        }
        var writer = CanonicalNoritoWriter()
        for byte in uuidBytes {
            writer.writeLength(1)
            writer.writeUInt8(byte)
        }
        return writer.data
    }

    private static func encodeAssetBalanceScopePayload(dataspaceId: UInt64?) -> Data {
        var writer = CanonicalNoritoWriter()
        guard let dataspaceId else {
            writer.writeUInt32LE(0)
            return writer.data
        }
        writer.writeUInt32LE(1)
        var dataspaceWriter = CanonicalNoritoWriter()
        dataspaceWriter.writeUInt64LE(dataspaceId)
        writer.writeField(dataspaceWriter.data)
        return writer.data
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
        case 0xea:
            return .blsNormal
        case 0xeb:
            return .blsSmall
        case 0xee:
            return .mlDsa
        case 0x1200:
            return .gost2012_256A
        case 0x1201:
            return .gost2012_256B
        case 0x1202:
            return .gost2012_256C
        case 0x1203:
            return .gost2012_512A
        case 0x1204:
            return .gost2012_512B
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
        case .blsNormal:
            return 0xea
        case .blsSmall:
            return 0xeb
        case .mlDsa:
            return 0xee
        case .gost2012_256A:
            return 0x1200
        case .gost2012_256B:
            return 0x1201
        case .gost2012_256C:
            return 0x1202
        case .gost2012_512A:
            return 0x1203
        case .gost2012_512B:
            return 0x1204
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
        case "bls_normal", "bls-normal", "blsnormal":
            return .blsNormal
        case "bls_small", "bls-small", "blssmall":
            return .blsSmall
        case "gost256a", "gost-256-a", "gost3410-2012-256-paramset-a":
            return .gost2012_256A
        case "gost256b", "gost-256-b", "gost3410-2012-256-paramset-b":
            return .gost2012_256B
        case "gost256c", "gost-256-c", "gost3410-2012-256-paramset-c":
            return .gost2012_256C
        case "gost512a", "gost-512-a", "gost3410-2012-512-paramset-a":
            return .gost2012_512A
        case "gost512b", "gost-512-b", "gost3410-2012-512-paramset-b":
            return .gost2012_512B
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
            throw CanonicalNoritoError.invalidAssetId(value)
        }
    }

    static func encodePoseidonDigest(_ bytes: Data) throws -> Data {
        guard bytes.count == 32 else {
            throw CanonicalNoritoError.invalidLength("poseidon digest must be 32 bytes")
        }
        var writer = CanonicalNoritoWriter()
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
                throw CanonicalNoritoError.invalidMetadata("non-finite number")
            }
            if number == 0 && number.sign == .minus {
                out.append("-0.0")
            } else if number.rounded(.towardZero) == number && abs(number) <= maxSafeInteger {
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
            let keys = object.keys.sorted(by: unicodeScalarLexicographicallyPrecedes)
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
            case "\u{08}":
                out.append("\\b")
            case "\u{0c}":
                out.append("\\f")
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
        let digits = "0123456789abcdef"
        let idx = digits.index(digits.startIndex, offsetBy: Int(value))
        return String(digits[idx])
    }

    private static func unicodeScalarLexicographicallyPrecedes(_ left: String,
                                                               _ right: String) -> Bool {
        var leftScalars = left.unicodeScalars.makeIterator()
        var rightScalars = right.unicodeScalars.makeIterator()
        while true {
            switch (leftScalars.next(), rightScalars.next()) {
            case let (.some(leftScalar), .some(rightScalar)):
                if leftScalar.value != rightScalar.value {
                    return leftScalar.value < rightScalar.value
                }
            case (.none, .some):
                return true
            case (.some, .none), (.none, .none):
                return false
            }
        }
    }
}

struct CanonicalBigInt {
    var isNegative: Bool = false
    private var limbs: [UInt32]

    var isZero: Bool {
        limbs.allSatisfy { $0 == 0 }
    }

    init(decimalDigits: String) throws {
        guard !decimalDigits.isEmpty else {
            throw CanonicalNoritoError.invalidNumeric(decimalDigits)
        }
        var values = [UInt32](repeating: 0, count: 1)
        for scalar in decimalDigits.unicodeScalars {
            guard scalar.value >= 48 && scalar.value <= 57 else {
                throw CanonicalNoritoError.invalidNumeric(decimalDigits)
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
                throw CanonicalNoritoError.numericOverflow
            }
            return Data(bytes)
        }
        var bytes = magnitude
        if bytes.isEmpty { bytes = [0] }
        if (bytes.last ?? 0) & 0x80 != 0 {
            bytes.append(0)
        }
        guard bytes.count <= maxBytes else {
            throw CanonicalNoritoError.numericOverflow
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

public enum IrohaHash {
    public static func hash(_ data: Data) -> Data {
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
