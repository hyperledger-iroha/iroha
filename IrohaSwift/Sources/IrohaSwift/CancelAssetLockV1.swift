import Foundation

/// Validation failures for the first-release `CancelAssetLock` instruction.
public enum CancelAssetLockV1Error: Error, LocalizedError, Equatable, Sendable {
    case invalidLockId
    case invalidEscrowId
    case invalidExpectedRemainingAmount
    case nonPositiveExpectedRemainingAmount
    case invalidJSON(String)
    case invalidNorito(String)

    public var errorDescription: String? {
        switch self {
        case .invalidLockId:
            "lockId must be exact non-empty text without surrounding whitespace/BOM and at most 4096 UTF-8 bytes"
        case .invalidEscrowId:
            "escrowId must be an exact canonical marked hash:<HEX>#<CRC16> literal"
        case .invalidExpectedRemainingAmount:
            "expectedRemainingAmount must be a canonical non-negative Kotodama V1 quantity"
        case .nonPositiveExpectedRemainingAmount:
            "expectedRemainingAmount must be greater than zero"
        case let .invalidJSON(reason):
            "CancelAssetLock JSON is invalid: \(reason)"
        case let .invalidNorito(reason):
            "CancelAssetLock Norito is invalid: \(reason)"
        }
    }
}

/// Canonical first-release compare-and-cancel instruction for a generic asset lock.
///
/// The signed `expectedRemainingAmount` is a compare-and-set precondition. The
/// ledger rejects cancellation if committed state no longer has that exact
/// remaining quantity.
public struct CancelAssetLockInstructionV1: Equatable, Sendable {
    /// Canonical dynamic-instruction wire identifier.
    public static let wireId = "iroha.instruction.v1::escrow::CancelAssetLock"

    /// Concrete Norito schema name used only to frame the typed payload.
    static let schemaName = "iroha_data_model::isi::escrow::CancelAssetLock"

    /// Maximum UTF-8 bytes accepted for the lock-id preimage in V1.
    public static let maxLockIdUTF8BytesV1 = 4096

    /// Canonical native escrow hash literal.
    public let escrowId: String

    /// Exact positive remaining quantity observed in finalized ledger state.
    public let expectedRemainingAmount: KotodamaQuantity

    /// Derive the native escrow id from an exact lock id with Blake2b-256.
    ///
    /// The Iroha hash marker bit and canonical CRC16 literal are applied after
    /// hashing, matching `EscrowId(Hash::new(lockId))` in the native model.
    public init(
        lockId: String,
        expectedRemainingAmount: String
    ) throws {
        let quantity = try Self.canonicalPositiveQuantity(expectedRemainingAmount)
        try self.init(
            validatedEscrowId: Self.escrowId(forLockId: lockId),
            validatedExpectedRemainingAmount: quantity
        )
    }

    /// Derive the native escrow id while accepting an already validated quantity.
    public init(
        lockId: String,
        expectedRemainingAmount: KotodamaQuantity
    ) throws {
        try self.init(
            validatedEscrowId: Self.escrowId(forLockId: lockId),
            validatedExpectedRemainingAmount: Self.positiveQuantity(expectedRemainingAmount)
        )
    }

    /// Construct from a finalized ledger's exact canonical escrow-id literal.
    public init(
        escrowId: String,
        expectedRemainingAmount: String
    ) throws {
        let quantity = try Self.canonicalPositiveQuantity(expectedRemainingAmount)
        try self.init(
            validatedEscrowId: Self.canonicalEscrowId(escrowId),
            validatedExpectedRemainingAmount: quantity
        )
    }

    /// Construct from exact canonical typed values.
    public init(
        escrowId: String,
        expectedRemainingAmount: KotodamaQuantity
    ) throws {
        try self.init(
            validatedEscrowId: Self.canonicalEscrowId(escrowId),
            validatedExpectedRemainingAmount: Self.positiveQuantity(expectedRemainingAmount)
        )
    }

    private init(
        validatedEscrowId: String,
        validatedExpectedRemainingAmount: KotodamaQuantity
    ) {
        escrowId = validatedEscrowId
        expectedRemainingAmount = validatedExpectedRemainingAmount
    }

    /// Derive the exact native `EscrowId(Hash::new(lockId))` literal.
    public static func escrowId(forLockId lockId: String) throws -> String {
        let bytes = Data(lockId.utf8)
        let scalars = lockId.unicodeScalars
        guard !bytes.isEmpty,
              bytes.count <= Self.maxLockIdUTF8BytesV1,
              !scalars.allSatisfy(isLockIdWhitespace),
              scalars.first.map({ !isLockIdWhitespace($0) }) == true,
              scalars.last.map({ !isLockIdWhitespace($0) }) == true
        else {
            throw CancelAssetLockV1Error.invalidLockId
        }
        var digest = Blake2b.hash256(bytes)
        digest[digest.index(before: digest.endIndex)] |= 1
        return hashLiteral(for: digest)
    }

    /// Emit the exact two-field instruction envelope used by the JSON API.
    public func noritoJSON() throws -> NoritoJSON {
        try NoritoJSON.fromJSONObject([
            "CancelAssetLock": bareJSONObject(),
        ])
    }

    /// Emit the bare two-field JSON value used by the reference fixture inventory.
    public func bareNoritoJSON() throws -> NoritoJSON {
        try NoritoJSON.fromJSONObject(bareJSONObject())
    }

    /// Encode the schema-bound bare `CancelAssetLock` Norito archive.
    public func noritoArchive() throws -> Data {
        var payload = CompactNoritoWriter()
        try payload.writeField(Self.escrowBytes(from: escrowId))
        try payload.writeField(
            CanonicalNorito.encodeCompactQuantity(
                expectedRemainingAmount.canonicalString
            )
        )
        return noritoEncode(
            typeName: Self.schemaName,
            payload: payload.data,
            flags: NoritoHeader.compactLen
        )
    }

    /// Wrap the canonical archive for inclusion in an executable transaction batch.
    public func transactionInstructionFrame() throws -> TransactionInstructionFrame {
        try TransactionInstructionFrame(
            wireName: Self.wireId,
            framedPayload: noritoArchive()
        )
    }

    /// Strictly decode a wrapped instruction JSON value.
    public static func decodeInstructionJSON(
        _ instruction: NoritoJSON
    ) throws -> CancelAssetLockInstructionV1 {
        try rejectDuplicateKeys(in: instruction.data)
        guard let outer = try jsonObject(instruction.data) as? [String: Any],
              Set(outer.keys) == ["CancelAssetLock"],
              let body = outer["CancelAssetLock"] as? [String: Any]
        else {
            throw CancelAssetLockV1Error.invalidJSON(
                "expected exactly one CancelAssetLock instruction variant"
            )
        }
        return try decodeBody(body)
    }

    /// Strictly decode the bare JSON value used by the appeal-finance fixtures.
    public static func decodeBareJSON(
        _ data: Data
    ) throws -> CancelAssetLockInstructionV1 {
        try rejectDuplicateKeys(in: data)
        guard let body = try jsonObject(data) as? [String: Any] else {
            throw CancelAssetLockV1Error.invalidJSON("expected an object")
        }
        return try decodeBody(body)
    }

    /// Strictly decode and canonically re-encode a bare Norito archive.
    public static func decodeNoritoArchive(
        _ archive: Data
    ) throws -> CancelAssetLockInstructionV1 {
        guard let frame = noritoDecodeFrame(archive) else {
            throw CancelAssetLockV1Error.invalidNorito("invalid frame")
        }
        guard frame.header.schema == noritoSchemaHash(forTypeName: Self.schemaName) else {
            throw CancelAssetLockV1Error.invalidNorito("schema hash mismatch")
        }
        guard frame.header.compression == .none,
              frame.header.flags == NoritoHeader.compactLen,
              frame.paddingLength == 0
        else {
            throw CancelAssetLockV1Error.invalidNorito(
                "expected canonical compact-length framing without compression or padding"
            )
        }

        var reader = CanonicalNoritoReader(data: frame.payload)
        let escrowBytes: Data
        let quantityBytes: Data
        do {
            escrowBytes = try reader.readCompactField()
            quantityBytes = try reader.readCompactField()
        } catch {
            throw CancelAssetLockV1Error.invalidNorito("missing required field")
        }
        guard reader.remaining() == 0 else {
            throw CancelAssetLockV1Error.invalidNorito("unexpected trailing field or byte")
        }
        let quantity = try decodePositiveQuantity(quantityBytes)
        let value = try CancelAssetLockInstructionV1(
            escrowId: canonicalEscrowId(from: escrowBytes),
            expectedRemainingAmount: quantity
        )
        guard try value.noritoArchive() == archive else {
            throw CancelAssetLockV1Error.invalidNorito("archive is not byte-canonical")
        }
        return value
    }

    private func bareJSONObject() -> [String: Any] {
        [
            "escrow_id": escrowId,
            "expected_remaining_amount": expectedRemainingAmount.canonicalString,
        ]
    }

    private static func decodeBody(
        _ body: [String: Any]
    ) throws -> CancelAssetLockInstructionV1 {
        let required: Set = ["escrow_id", "expected_remaining_amount"]
        guard Set(body.keys) == required else {
            throw CancelAssetLockV1Error.invalidJSON(
                "expected exactly escrow_id and expected_remaining_amount"
            )
        }
        guard let escrowId = body["escrow_id"] as? String else {
            throw CancelAssetLockV1Error.invalidJSON("escrow_id must be a string")
        }
        guard let expected = body["expected_remaining_amount"] as? String else {
            throw CancelAssetLockV1Error.invalidJSON(
                "expected_remaining_amount must be a string"
            )
        }
        return try CancelAssetLockInstructionV1(
            escrowId: escrowId,
            expectedRemainingAmount: expected
        )
    }

    private static func jsonObject(_ data: Data) throws -> Any {
        do {
            return try JSONSerialization.jsonObject(with: data, options: [])
        } catch {
            throw CancelAssetLockV1Error.invalidJSON("malformed UTF-8 JSON")
        }
    }

    private static func rejectDuplicateKeys(in data: Data) throws {
        do {
            try StrictJSONDuplicateKeyRejector.rejectDuplicateObjectKeys(in: data)
        } catch {
            throw CancelAssetLockV1Error.invalidJSON("duplicate object key")
        }
    }

    private static func canonicalPositiveQuantity(
        _ value: String
    ) throws -> KotodamaQuantity {
        let quantity: KotodamaQuantity
        do {
            quantity = try KotodamaNumericV1Codec.decodeQuantityJSON(value)
        } catch {
            throw CancelAssetLockV1Error.invalidExpectedRemainingAmount
        }
        return try positiveQuantity(quantity)
    }

    private static func positiveQuantity(
        _ value: KotodamaQuantity
    ) throws -> KotodamaQuantity {
        guard value.mantissa.canonicalString != "0" else {
            throw CancelAssetLockV1Error.nonPositiveExpectedRemainingAmount
        }
        return value
    }

    private static func canonicalEscrowId(_ literal: String) throws -> String {
        guard let bytes = escrowBytesIfCanonical(from: literal),
              bytes.last.map({ $0 & 1 == 1 }) == true
        else {
            throw CancelAssetLockV1Error.invalidEscrowId
        }
        return literal
    }

    private static func escrowBytes(from literal: String) throws -> Data {
        guard let bytes = escrowBytesIfCanonical(from: literal),
              bytes.last.map({ $0 & 1 == 1 }) == true
        else {
            throw CancelAssetLockV1Error.invalidEscrowId
        }
        return bytes
    }

    private static func escrowBytesIfCanonical(from literal: String) -> Data? {
        let encoded = Array(literal.utf8)
        let prefix = Array("hash:".utf8)
        guard encoded.count == 74,
              Array(encoded.prefix(prefix.count)) == prefix,
              encoded[69] == 0x23
        else {
            return nil
        }
        let bodyBytes = Array(encoded[5 ..< 69])
        let checksumBytes = Array(encoded[70 ..< 74])
        guard bodyBytes.allSatisfy(isUppercaseHex),
              checksumBytes.allSatisfy(isUppercaseHex),
              let body = String(bytes: bodyBytes, encoding: .utf8),
              let checksum = String(bytes: checksumBytes, encoding: .utf8),
              let suppliedChecksum = UInt16(checksum, radix: 16),
              suppliedChecksum == crc16(Array("hash:\(body)".utf8)),
              let bytes = Data(hexString: body),
              bytes.count == 32
        else {
            return nil
        }
        return bytes
    }

    private static func canonicalEscrowId(from bytes: Data) throws -> String {
        guard bytes.count == 32,
              bytes.last.map({ $0 & 1 == 1 }) == true
        else {
            throw CancelAssetLockV1Error.invalidNorito(
                "escrow_id must be a 32-byte marked hash"
            )
        }
        return hashLiteral(for: bytes)
    }

    private static func hashLiteral(for bytes: Data) -> String {
        let body = bytes.map { String(format: "%02X", $0) }.joined()
        let checksum = crc16(Array("hash:\(body)".utf8))
        return "hash:\(body)#\(String(format: "%04X", checksum))"
    }

    private static func isUppercaseHex(_ byte: UInt8) -> Bool {
        (0x30 ... 0x39).contains(byte) || (0x41 ... 0x46).contains(byte)
    }

    private static func isLockIdWhitespace(_ scalar: UnicodeScalar) -> Bool {
        CharacterSet.whitespacesAndNewlines.contains(scalar) || scalar.value == 0xFEFF
    }

    private static func crc16(_ bytes: [UInt8]) -> UInt16 {
        var crc = UInt16.max
        for byte in bytes {
            crc ^= UInt16(byte) << 8
            for _ in 0 ..< 8 {
                crc = (crc & 0x8000) != 0
                    ? (crc &<< 1) ^ 0x1021
                    : crc &<< 1
            }
        }
        return crc
    }

    private static func decodePositiveQuantity(
        _ payload: Data
    ) throws -> KotodamaQuantity {
        var reader = CanonicalNoritoReader(data: payload)
        let mantissaPayload: Data
        let scalePayload: Data
        do {
            mantissaPayload = try reader.readCompactField()
            scalePayload = try reader.readCompactField()
        } catch {
            throw CancelAssetLockV1Error.invalidNorito(
                "expected_remaining_amount is truncated"
            )
        }
        guard reader.remaining() == 0 else {
            throw CancelAssetLockV1Error.invalidNorito(
                "expected_remaining_amount contains trailing bytes"
            )
        }

        var mantissaReader = CanonicalNoritoReader(data: mantissaPayload)
        let byteCount: UInt32
        do {
            byteCount = try mantissaReader.readUInt32LE()
        } catch {
            throw CancelAssetLockV1Error.invalidNorito("quantity mantissa is truncated")
        }
        guard byteCount <= 64,
              Int(byteCount) == mantissaReader.remaining()
        else {
            throw CancelAssetLockV1Error.invalidNorito(
                "quantity mantissa length is invalid"
            )
        }
        let mantissa: Data
        do {
            mantissa = try mantissaReader.readBytes(Int(byteCount))
        } catch {
            throw CancelAssetLockV1Error.invalidNorito("quantity mantissa is truncated")
        }
        guard scalePayload.count == 4 else {
            throw CancelAssetLockV1Error.invalidNorito(
                "quantity scale must contain exactly four bytes"
            )
        }
        var scaleReader = CanonicalNoritoReader(data: scalePayload)
        let scale: UInt32
        do {
            scale = try scaleReader.readUInt32LE()
        } catch {
            throw CancelAssetLockV1Error.invalidNorito("quantity scale is truncated")
        }
        guard scale <= 28 else {
            throw CancelAssetLockV1Error.invalidNorito("quantity scale exceeds 28")
        }

        let digits = try positiveDecimalDigits(fromLittleEndianTwos: mantissa)
        let literal = scaledDecimal(digits, scale: Int(scale))
        let quantity: KotodamaQuantity
        do {
            quantity = try KotodamaNumericV1Codec.decodeQuantityJSON(literal)
        } catch {
            throw CancelAssetLockV1Error.invalidNorito(
                "expected_remaining_amount is noncanonical"
            )
        }
        return try positiveQuantity(quantity)
    }

    private static func positiveDecimalDigits(
        fromLittleEndianTwos bytes: Data
    ) throws -> String {
        guard !bytes.isEmpty else {
            throw CancelAssetLockV1Error.nonPositiveExpectedRemainingAmount
        }
        guard !(bytes.count == 1 && bytes[bytes.startIndex] == 0),
              bytes.last.map({ $0 & 0x80 == 0 }) == true
        else {
            throw CancelAssetLockV1Error.invalidNorito(
                "quantity mantissa is zero, negative, or noncanonical"
            )
        }
        if bytes.count > 1,
           bytes.last == 0
        {
            let previous = bytes[bytes.index(bytes.endIndex, offsetBy: -2)]
            guard previous & 0x80 != 0 else {
                throw CancelAssetLockV1Error.invalidNorito(
                    "quantity mantissa has redundant sign extension"
                )
            }
        }

        var decimalDigits: [UInt8] = [0]
        for byte in bytes.reversed() {
            var carry = Int(byte)
            for index in decimalDigits.indices {
                let value = Int(decimalDigits[index]) * 256 + carry
                decimalDigits[index] = UInt8(value % 10)
                carry = value / 10
            }
            while carry > 0 {
                decimalDigits.append(UInt8(carry % 10))
                carry /= 10
            }
        }
        while decimalDigits.count > 1, decimalDigits.last == 0 {
            decimalDigits.removeLast()
        }
        guard !(decimalDigits.count == 1 && decimalDigits[0] == 0) else {
            throw CancelAssetLockV1Error.nonPositiveExpectedRemainingAmount
        }
        return String(
            bytes: decimalDigits.reversed().map { $0 + 0x30 },
            encoding: .utf8
        )!
    }

    private static func scaledDecimal(_ digits: String, scale: Int) -> String {
        guard scale > 0 else {
            return digits
        }
        var padded = digits
        if padded.count <= scale {
            padded = String(repeating: "0", count: scale + 1 - padded.count) + padded
        }
        let split = padded.index(padded.endIndex, offsetBy: -scale)
        return "\(padded[..<split]).\(padded[split...])"
    }
}
