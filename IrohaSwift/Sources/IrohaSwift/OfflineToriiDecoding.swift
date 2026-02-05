import Foundation

enum OfflineToriiDecodingError: Error, LocalizedError {
    case missingField(String)
    case invalidField(String)

    var errorDescription: String? {
        switch self {
        case let .missingField(field):
            return "Missing required field: \(field)"
        case let .invalidField(field):
            return "Invalid field value: \(field)"
        }
    }
}

enum OfflineToriiDecoding {
    static func decodeCertificate(from value: ToriiJSONValue) throws -> OfflineWalletCertificate {
        let object = try requireObject(value, field: "certificate")
        let controller = try requireString(object, field: "controller")
        let operatorId = try requireString(object, field: "operator")
        let allowance = try decodeAllowance(from: object)
        let spendPublicKey = try requireString(object, field: "spend_public_key")
        let attestationReport = try requireBytes(object, field: "attestation_report")
        let issuedAtMs = try requireUInt64(object, field: "issued_at_ms")
        let expiresAtMs = try requireUInt64(object, field: "expires_at_ms")
        let policy = try decodePolicy(from: object)
        let operatorSignature = try requireHexData(object, field: "operator_signature")
        let metadata = try decodeMetadata(object, field: "metadata")
        let verdictId = try optionalHashData(object, field: "verdict_id")
        let attestationNonce = try optionalHashData(object, field: "attestation_nonce")
        let refreshAtMs = try optionalUInt64(object, field: "refresh_at_ms")
        return OfflineWalletCertificate(
            controller: controller,
            operatorId: operatorId,
            allowance: allowance,
            spendPublicKey: spendPublicKey,
            attestationReport: attestationReport,
            issuedAtMs: issuedAtMs,
            expiresAtMs: expiresAtMs,
            policy: policy,
            operatorSignature: operatorSignature,
            metadata: metadata,
            verdictId: verdictId,
            attestationNonce: attestationNonce,
            refreshAtMs: refreshAtMs
        )
    }

    private static func decodeAllowance(from object: [String: ToriiJSONValue]) throws -> OfflineAllowanceCommitment {
        let allowanceObject = try requireObject(object, field: "allowance")
        let assetId = try requireString(allowanceObject, field: "asset")
        _ = try OfflineNorito.encodeAssetId(assetId)
        let amount = try requireNumericString(allowanceObject, field: "amount")
        let commitment = try requireBytes(allowanceObject, field: "commitment")
        return OfflineAllowanceCommitment(assetId: assetId, amount: amount, commitment: commitment)
    }

    private static func decodePolicy(from object: [String: ToriiJSONValue]) throws -> OfflineWalletPolicy {
        let policyObject = try requireObject(object, field: "policy")
        let maxBalance = try requireNumericString(policyObject, field: "max_balance")
        let maxTxValue = try requireNumericString(policyObject, field: "max_tx_value")
        let expiresAtMs = try requireUInt64(policyObject, field: "expires_at_ms")
        return OfflineWalletPolicy(maxBalance: maxBalance, maxTxValue: maxTxValue, expiresAtMs: expiresAtMs)
    }

    private static func decodeMetadata(_ object: [String: ToriiJSONValue],
                                       field: String) throws -> [String: ToriiJSONValue] {
        guard let value = object[field] else {
            return [:]
        }
        switch value {
        case .null:
            return [:]
        case .object(let metadata):
            _ = try OfflineNorito.encodeMetadata(metadata)
            return metadata
        default:
            throw OfflineToriiDecodingError.invalidField(field)
        }
    }

    private static func requireObject(_ value: ToriiJSONValue,
                                      field: String) throws -> [String: ToriiJSONValue] {
        guard case let .object(object) = value else {
            throw OfflineToriiDecodingError.invalidField(field)
        }
        return object
    }

    private static func requireObject(_ object: [String: ToriiJSONValue],
                                      field: String) throws -> [String: ToriiJSONValue] {
        guard let value = object[field] else {
            throw OfflineToriiDecodingError.missingField(field)
        }
        return try requireObject(value, field: field)
    }

    private static func requireString(_ object: [String: ToriiJSONValue],
                                      field: String) throws -> String {
        guard let value = object[field]?.normalizedString else {
            throw OfflineToriiDecodingError.missingField(field)
        }
        guard !value.isEmpty else {
            throw OfflineToriiDecodingError.invalidField(field)
        }
        return value
    }

    private static func requireNumericString(_ object: [String: ToriiJSONValue],
                                             field: String) throws -> String {
        let value = try requireString(object, field: field)
        _ = try OfflineNorito.encodeNumeric(value)
        return value
    }

    private static func requireUInt64(_ object: [String: ToriiJSONValue],
                                      field: String) throws -> UInt64 {
        guard let value = object[field]?.normalizedUInt64 else {
            throw OfflineToriiDecodingError.missingField(field)
        }
        return value
    }

    private static func optionalUInt64(_ object: [String: ToriiJSONValue],
                                       field: String) throws -> UInt64? {
        guard let value = object[field] else {
            return nil
        }
        if case .null = value {
            return nil
        }
        guard let parsed = value.normalizedUInt64 else {
            throw OfflineToriiDecodingError.invalidField(field)
        }
        return parsed
    }

    private static func requireHexData(_ object: [String: ToriiJSONValue],
                                       field: String) throws -> Data {
        let hex = try requireString(object, field: field)
        guard let data = Data(hexString: hex) else {
            throw OfflineToriiDecodingError.invalidField(field)
        }
        return data
    }

    private static func requireBytes(_ object: [String: ToriiJSONValue],
                                     field: String) throws -> Data {
        guard let value = object[field] else {
            throw OfflineToriiDecodingError.missingField(field)
        }
        return try decodeBytes(value, field: field)
    }

    private static func decodeBytes(_ value: ToriiJSONValue, field: String) throws -> Data {
        switch value {
        case .array(let items):
            var bytes = Data(capacity: items.count)
            for (index, item) in items.enumerated() {
                guard case let .number(number) = item, number.isFinite else {
                    throw OfflineToriiDecodingError.invalidField("\(field)[\(index)]")
                }
                let rounded = number.rounded(.towardZero)
                guard rounded == number, rounded >= 0, rounded <= 255 else {
                    throw OfflineToriiDecodingError.invalidField("\(field)[\(index)]")
                }
                bytes.append(UInt8(rounded))
            }
            return bytes
        case .string(let string):
            if let data = Data(base64Encoded: string) {
                return data
            }
            if let data = Data(hexString: string) {
                return data
            }
            throw OfflineToriiDecodingError.invalidField(field)
        default:
            throw OfflineToriiDecodingError.invalidField(field)
        }
    }

    private static func optionalHashData(_ object: [String: ToriiJSONValue],
                                         field: String) throws -> Data? {
        guard let value = object[field] else {
            return nil
        }
        if case .null = value {
            return nil
        }
        guard case let .string(string) = value else {
            throw OfflineToriiDecodingError.invalidField(field)
        }
        let bytes = try parseHashLiteral(string, field: field)
        _ = try OfflineNorito.encodeHash(bytes)
        return bytes
    }

    private static func parseHashLiteral(_ value: String, field: String) throws -> Data {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.lowercased().hasPrefix("hash:") {
            guard let separator = trimmed.lastIndex(of: "#") else {
                throw OfflineToriiDecodingError.invalidField(field)
            }
            let bodyStart = trimmed.index(trimmed.startIndex, offsetBy: 5)
            let body = String(trimmed[bodyStart..<separator])
            let checksum = String(trimmed[trimmed.index(after: separator)...])
            guard body.count == 64, let data = Data(hexString: body) else {
                throw OfflineToriiDecodingError.invalidField(field)
            }
            guard checksum.count == 4, let checksumValue = UInt16(checksum, radix: 16) else {
                throw OfflineToriiDecodingError.invalidField(field)
            }
            let expected = crc16(tag: "hash", body: body.uppercased())
            guard expected == checksumValue else {
                throw OfflineToriiDecodingError.invalidField(field)
            }
            return data
        }
        guard trimmed.count == 64, let data = Data(hexString: trimmed) else {
            throw OfflineToriiDecodingError.invalidField(field)
        }
        return data
    }

    private static func crc16(tag: String, body: String) -> UInt16 {
        var crc: UInt16 = 0xFFFF
        for byte in tag.utf8 {
            crc = updateCrc(crc, value: byte)
        }
        crc = updateCrc(crc, value: Character(":").asciiValue ?? 0)
        for byte in body.utf8 {
            crc = updateCrc(crc, value: byte)
        }
        return crc
    }

    private static func updateCrc(_ crc: UInt16, value: UInt8) -> UInt16 {
        var current = crc ^ UInt16(value) << 8
        for _ in 0..<8 {
            if current & 0x8000 != 0 {
                current = (current << 1) ^ 0x1021
            } else {
                current <<= 1
            }
        }
        return current & 0xFFFF
    }
}

public extension OfflineWalletCertificate {
    init(toriiValue: ToriiJSONValue) throws {
        self = try OfflineToriiDecoding.decodeCertificate(from: toriiValue)
    }
}
