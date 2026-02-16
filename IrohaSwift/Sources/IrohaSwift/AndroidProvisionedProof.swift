import Foundation

/// Swift representation of the Norito `AndroidProvisionedProof` payload
/// emitted by `cargo xtask offline-provision`.
public struct AndroidProvisionedProof: Codable, Sendable, Equatable {
    /// Schema label (for example `offline_provisioning_v1`).
    public let manifestSchema: String
    /// Optional schema version.
    public let manifestVersion: Int?
    /// Timestamp (ms since epoch) when the manifest was issued.
    public let manifestIssuedAtMs: UInt64
    /// Canonical Norito hash literal describing the receipt challenge.
    public let challengeHashLiteral: String
    /// Monotonic counter enforced per `{schema}::{device_id}` scope.
    public let counter: UInt64
    /// Manifest metadata describing the inspected device.
    public let deviceManifest: [String: ToriiJSONValue]
    /// Canonical uppercase inspector signature (hex-encoded Ed25519 signature).
    public let inspectorSignatureHex: String

    private enum CodingKeys: String, CodingKey {
        case manifestSchema = "manifest_schema"
        case manifestVersion = "manifest_version"
        case manifestIssuedAtMs = "manifest_issued_at_ms"
        case challengeHashLiteral = "challenge_hash"
        case counter
        case deviceManifest = "device_manifest"
        case inspectorSignatureHex = "inspector_signature"
    }

    public init(manifestSchema: String,
                manifestVersion: Int?,
                manifestIssuedAtMs: UInt64,
                challengeHashLiteral: String,
                counter: UInt64,
                deviceManifest: [String: ToriiJSONValue],
                inspectorSignatureHex: String) throws {
        guard !manifestSchema.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw AndroidProvisionedProofError.invalidManifestSchema
        }
        guard !deviceManifest.isEmpty else {
            throw AndroidProvisionedProofError.invalidDeviceManifest("device_manifest is empty")
        }
        self.manifestSchema = manifestSchema
        self.manifestVersion = manifestVersion
        self.manifestIssuedAtMs = manifestIssuedAtMs
        self.counter = counter
        self.deviceManifest = deviceManifest
        self.challengeHashLiteral = try Self.canonicalHashLiteral(from: challengeHashLiteral)
        self.inspectorSignatureHex = try Self.normalizeSignature(inspectorSignatureHex)
        guard deviceId != nil else {
            throw AndroidProvisionedProofError.invalidDeviceManifest(
                "android.provisioned.device_id is required"
            )
        }
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let schema = try container.decode(String.self, forKey: .manifestSchema)
        let version = try container.decodeIfPresent(Int.self, forKey: .manifestVersion)
        let issuedAt = try container.decode(UInt64.self, forKey: .manifestIssuedAtMs)
        let hashLiteral = try container.decode(String.self, forKey: .challengeHashLiteral)
        let counter = try container.decode(UInt64.self, forKey: .counter)
        let manifest = try container.decode([String: ToriiJSONValue].self, forKey: .deviceManifest)
        let signature = try container.decode(String.self, forKey: .inspectorSignatureHex)
        do {
            try self.init(manifestSchema: schema,
                          manifestVersion: version,
                          manifestIssuedAtMs: issuedAt,
                          challengeHashLiteral: hashLiteral,
                          counter: counter,
                          deviceManifest: manifest,
                          inspectorSignatureHex: signature)
        } catch let error as AndroidProvisionedProofError {
            throw DecodingError.dataCorruptedError(forKey: .deviceManifest,
                                                   in: container,
                                                   debugDescription: error.localizedDescription)
        }
    }

    /// Returns the canonical hex body of the challenge hash (without prefix/checksum).
    public var challengeHashHex: String {
        Self.extractHashBody(from: challengeHashLiteral)
    }

    /// Returns the decoded challenge hash bytes.
    public var challengeHashData: Data? {
        Data(hexString: challengeHashHex)
    }

    /// Returns the decoded inspector signature bytes.
    public var inspectorSignatureData: Data? {
        Data(hexString: inspectorSignatureHex)
    }

    /// Returns the provisioned device identifier, if present in the manifest.
    public var deviceId: String? {
        deviceManifest["android.provisioned.device_id"]?.normalizedString
    }

    /// Encodes the proof back into JSON.
    public func encodedData(prettyPrinted: Bool = true) throws -> Data {
        let encoder = JSONEncoder()
        if prettyPrinted {
            encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        } else {
            encoder.outputFormatting = [.sortedKeys]
        }
        return try encoder.encode(self)
    }

    /// Loads a proof from disk.
    public static func load(from url: URL,
                            decoder: JSONDecoder = JSONDecoder()) throws -> AndroidProvisionedProof {
        let data = try Data(contentsOf: url)
        return try decoder.decode(AndroidProvisionedProof.self, from: data)
    }

    // MARK: - Validation helpers

    private static func canonicalHashLiteral(from raw: String) throws -> String {
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw AndroidProvisionedProofError.invalidHashLiteral
        }
        if trimmed.lowercased().hasPrefix("hash:") {
            try validateHashLiteral(trimmed)
            return trimmed
        }
        guard trimmed.count == 64, Data(hexString: trimmed) != nil else {
            throw AndroidProvisionedProofError.invalidHashLiteral
        }
        let normalized = trimmed.uppercased()
        let checksum = crc16(tag: "hash", body: normalized)
        return "hash:\(normalized)#\(String(format: "%04X", checksum))"
    }

    private static func normalizeSignature(_ value: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw AndroidProvisionedProofError.invalidSignature
        }
        guard trimmed.count == 128, Data(hexString: trimmed) != nil else {
            throw AndroidProvisionedProofError.invalidSignature
        }
        return trimmed.uppercased()
    }

    private static func validateHashLiteral(_ literal: String) throws {
        let trimmed = literal.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.lowercased().hasPrefix("hash:") else {
            throw AndroidProvisionedProofError.invalidHashLiteral
        }
        guard let separator = trimmed.lastIndex(of: "#") else {
            throw AndroidProvisionedProofError.invalidHashLiteral
        }
        let bodyStart = trimmed.index(trimmed.startIndex, offsetBy: 5)
        let body = String(trimmed[bodyStart..<separator])
        let checksum = String(trimmed[trimmed.index(after: separator)...])
        guard body.count == 64, Data(hexString: body) != nil else {
            throw AndroidProvisionedProofError.invalidHashLiteral
        }
        guard checksum.count == 4, Int(checksum, radix: 16) != nil else {
            throw AndroidProvisionedProofError.invalidHashLiteral
        }
        let expected = crc16(tag: "hash", body: body.uppercased())
        let provided = UInt16(checksum, radix: 16) ?? 0
        guard expected == provided else {
            throw AndroidProvisionedProofError.invalidHashLiteral
        }
    }

    private static func extractHashBody(from literal: String) -> String {
        guard let separator = literal.lastIndex(of: "#") else {
            return literal
        }
        let start = literal.index(literal.startIndex, offsetBy: 5)
        return String(literal[start..<separator])
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

public enum AndroidProvisionedProofError: LocalizedError {
    case invalidManifestSchema
    case invalidDeviceManifest(String)
    case invalidHashLiteral
    case invalidSignature

    public var errorDescription: String? {
        switch self {
        case .invalidManifestSchema:
            return "manifest_schema must not be empty"
        case .invalidDeviceManifest(let reason):
            return reason
        case .invalidHashLiteral:
            return "challenge_hash must be a canonical Norito hash literal"
        case .invalidSignature:
            return "inspector_signature must contain 64 bytes encoded as uppercase hex"
        }
    }
}

extension AndroidProvisionedProof {
    func noritoPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeString(manifestSchema))
        writer.writeField(try OfflineNorito.encodeOption(manifestVersion, encode: { value in
            guard value >= 0 else {
                throw OfflineNoritoError.invalidLength("manifest_version")
            }
            return OfflineNorito.encodeUInt32(UInt32(value))
        }))
        writer.writeField(OfflineNorito.encodeUInt64(manifestIssuedAtMs))
        guard let challenge = challengeHashData else {
            throw OfflineNoritoError.invalidHash("challenge_hash")
        }
        writer.writeField(try OfflineNorito.encodeHash(challenge))
        writer.writeField(OfflineNorito.encodeUInt64(counter))
        writer.writeField(try OfflineNorito.encodeMetadata(deviceManifest))
        guard let signature = inspectorSignatureData else {
            throw OfflineNoritoError.invalidHex("inspector_signature")
        }
        writer.writeField(OfflineNorito.encodeConstVec(signature))
        return writer.data
    }

    func noritoEncoded() throws -> Data {
        OfflineNorito.wrap(typeName: Self.noritoTypeName, payload: try noritoPayload())
    }

    func manifestSigningBytes() throws -> Data {
        OfflineNorito.wrap(typeName: Self.signingPayloadTypeName, payload: try manifestSigningPayload())
    }

    private func manifestSigningPayload() throws -> Data {
        var writer = OfflineNoritoWriter()
        writer.writeField(OfflineNorito.encodeString(manifestSchema))
        writer.writeField(try OfflineNorito.encodeOption(manifestVersion, encode: { value in
            guard value >= 0 else {
                throw OfflineNoritoError.invalidLength("manifest_version")
            }
            return OfflineNorito.encodeUInt32(UInt32(value))
        }))
        writer.writeField(OfflineNorito.encodeUInt64(manifestIssuedAtMs))
        guard let challenge = challengeHashData else {
            throw OfflineNoritoError.invalidHash("challenge_hash")
        }
        writer.writeField(try OfflineNorito.encodeHash(challenge))
        writer.writeField(OfflineNorito.encodeUInt64(counter))
        writer.writeField(try OfflineNorito.encodeMetadata(deviceManifest))
        return writer.data
    }

    private static let noritoTypeName = "iroha_data_model::offline::model::AndroidProvisionedProof"
    private static let signingPayloadTypeName =
        "iroha_data_model::offline::model::AndroidProvisionedManifestSignaturePayload"
}
