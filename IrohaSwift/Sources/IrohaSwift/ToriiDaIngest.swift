import Foundation
import CryptoKit

private let ToriiDaEd25519FunctionCode: UInt8 = 0xED

public enum ToriiDaBlobClass: Sendable, Equatable {
    case taikaiSegment
    case nexusLaneSidecar
    case governanceArtifact
    case custom(UInt16)
}

public enum ToriiDaFecScheme: Sendable, Equatable {
    case rs12_10
    case rsWin14_10
    case rs18_14
    case custom(UInt16)
}

public struct ToriiDaErasureProfile: Sendable, Equatable {
    public var dataShards: UInt16
    public var parityShards: UInt16
    public var chunkAlignment: UInt16
    public var fecScheme: ToriiDaFecScheme

    public init(dataShards: UInt16 = 10,
                parityShards: UInt16 = 4,
                chunkAlignment: UInt16 = 10,
                fecScheme: ToriiDaFecScheme = .rs12_10) {
        self.dataShards = dataShards
        self.parityShards = parityShards
        self.chunkAlignment = chunkAlignment
        self.fecScheme = fecScheme
    }

    public static var `default`: ToriiDaErasureProfile { ToriiDaErasureProfile() }
}

public enum ToriiDaStorageClass: Sendable, Equatable {
    case hot
    case warm
    case cold
}

public struct ToriiDaRetentionPolicy: Sendable, Equatable {
    public var hotRetentionSecs: UInt64
    public var coldRetentionSecs: UInt64
    public var requiredReplicas: UInt16
    public var storageClass: ToriiDaStorageClass
    public var governanceTag: String

    public init(hotRetentionSecs: UInt64 = 7 * 24 * 60 * 60,
                coldRetentionSecs: UInt64 = 90 * 24 * 60 * 60,
                requiredReplicas: UInt16 = 3,
                storageClass: ToriiDaStorageClass = .hot,
                governanceTag: String = "da.default") {
        self.hotRetentionSecs = hotRetentionSecs
        self.coldRetentionSecs = coldRetentionSecs
        self.requiredReplicas = requiredReplicas
        self.storageClass = storageClass
        self.governanceTag = governanceTag
    }

    public static var `default`: ToriiDaRetentionPolicy { ToriiDaRetentionPolicy() }
}

public enum ToriiDaCompression: Sendable, Equatable {
    case identity
    case gzip
    case deflate
    case zstd
}

public enum ToriiDaMetadataVisibility: Sendable, Equatable {
    case `public`
    case governanceOnly
}

public enum ToriiDaMetadataEncryption: Sendable, Equatable {
    case none
    case chacha20Poly1305(keyLabel: String?)
}

public struct ToriiDaMetadataEntry: Sendable, Equatable {
    public var key: String
    public var value: Data
    public var visibility: ToriiDaMetadataVisibility
    public var encryption: ToriiDaMetadataEncryption

    public init(key: String,
                value: Data,
                visibility: ToriiDaMetadataVisibility = .public,
                encryption: ToriiDaMetadataEncryption = .none) {
        self.key = key
        self.value = value
        self.visibility = visibility
        self.encryption = encryption
    }
}

public struct ToriiDaBlobSubmission: Sendable {
    public var payload: Data
    public var chunkSize: Int
    public var laneId: UInt64
    public var epoch: UInt64
    public var sequence: UInt64
    public var blobClass: ToriiDaBlobClass
    public var codec: String
    public var erasureProfile: ToriiDaErasureProfile
    public var retentionPolicy: ToriiDaRetentionPolicy
    public var compression: ToriiDaCompression
    public var metadata: [ToriiDaMetadataEntry]
    public var noritoManifest: Data?
    public var clientBlobId: Data?
    public var submitterPublicKeyHex: String?
    public var signatureHex: String?
    public var privateKey: Data?
    public var privateKeyHex: String?

    public init(payload: Data,
                chunkSize: Int = 262_144,
                laneId: UInt64 = 0,
                epoch: UInt64 = 0,
                sequence: UInt64 = 0,
                blobClass: ToriiDaBlobClass = .taikaiSegment,
                codec: String = "application/octet-stream",
                erasureProfile: ToriiDaErasureProfile = .default,
                retentionPolicy: ToriiDaRetentionPolicy = .default,
                compression: ToriiDaCompression = .identity,
                metadata: [ToriiDaMetadataEntry] = [],
                noritoManifest: Data? = nil,
                clientBlobId: Data? = nil,
                submitterPublicKeyHex: String? = nil,
                signatureHex: String? = nil,
                privateKey: Data? = nil,
                privateKeyHex: String? = nil) {
        self.payload = payload
        self.chunkSize = chunkSize
        self.laneId = laneId
        self.epoch = epoch
        self.sequence = sequence
        self.blobClass = blobClass
        self.codec = codec
        self.erasureProfile = erasureProfile
        self.retentionPolicy = retentionPolicy
        self.compression = compression
        self.metadata = metadata
        self.noritoManifest = noritoManifest
        self.clientBlobId = clientBlobId
        self.submitterPublicKeyHex = submitterPublicKeyHex
        self.signatureHex = signatureHex
        self.privateKey = privateKey
        self.privateKeyHex = privateKeyHex
    }
}

public struct ToriiDaIngestArtifacts: Sendable, Equatable {
    public let clientBlobIdHex: String
    public let submitterPublicKeyHex: String
    public let signatureHex: String
    public let payloadLength: Int
}

public struct ToriiDaIngestSubmitResult: Sendable, Equatable {
    public let status: String
    public let duplicate: Bool
    public let receipt: ToriiDaIngestReceipt?
    public let artifacts: ToriiDaIngestArtifacts
    public let pdpCommitmentHeaderBase64: String?

    public init(status: String,
                duplicate: Bool,
                receipt: ToriiDaIngestReceipt?,
                artifacts: ToriiDaIngestArtifacts,
                pdpCommitmentHeaderBase64: String?) {
        self.status = status
        self.duplicate = duplicate
        self.receipt = receipt
        self.artifacts = artifacts
        self.pdpCommitmentHeaderBase64 = pdpCommitmentHeaderBase64
    }
}

public struct ToriiDaRentQuote: Decodable, Sendable, Equatable {
    public let baseRentMicro: String
    public let protocolReserveMicro: String
    public let providerRewardMicro: String
    public let pdpBonusMicro: String
    public let potrBonusMicro: String
    public let egressCreditPerGibMicro: String

    public var baseRentDecimal: Decimal? { Decimal(string: baseRentMicro) }
    public var protocolReserveDecimal: Decimal? { Decimal(string: protocolReserveMicro) }
    public var providerRewardDecimal: Decimal? { Decimal(string: providerRewardMicro) }
    public var pdpBonusDecimal: Decimal? { Decimal(string: pdpBonusMicro) }
    public var potrBonusDecimal: Decimal? { Decimal(string: potrBonusMicro) }
    public var egressCreditPerGibDecimal: Decimal? { Decimal(string: egressCreditPerGibMicro) }

    private enum CodingKeys: String, CodingKey {
        case baseRent = "base_rent"
        case protocolReserve = "protocol_reserve"
        case providerReward = "provider_reward"
        case pdpBonus = "pdp_bonus"
        case potrBonus = "potr_bonus"
        case egressCreditPerGib = "egress_credit_per_gib"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.baseRentMicro = try Self.decodeMicroAmount(from: container, key: .baseRent)
        self.protocolReserveMicro = try Self.decodeMicroAmount(from: container, key: .protocolReserve)
        self.providerRewardMicro = try Self.decodeMicroAmount(from: container, key: .providerReward)
        self.pdpBonusMicro = try Self.decodeMicroAmount(from: container, key: .pdpBonus)
        self.potrBonusMicro = try Self.decodeMicroAmount(from: container, key: .potrBonus)
        self.egressCreditPerGibMicro = try Self.decodeMicroAmount(from: container, key: .egressCreditPerGib)
    }

    private static func decodeMicroAmount(from container: KeyedDecodingContainer<CodingKeys>,
                                          key: CodingKeys) throws -> String {
        if let stringValue = try? container.decode(String.self, forKey: key) {
            let trimmed = stringValue.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmed.isEmpty else {
                throw ToriiClientError.invalidPayload("rent_quote field \(key.stringValue) was empty")
            }
            guard isAsciiDigits(trimmed) else {
                throw ToriiClientError.invalidPayload("rent_quote field \(key.stringValue) must be an unsigned integer string")
            }
            return trimmed
        }
        if let decimalValue = try? container.decode(Decimal.self, forKey: key) {
            var source = decimalValue
            var rounded = Decimal()
            NSDecimalRound(&rounded, &source, 0, .plain)
            guard rounded == source else {
                throw ToriiClientError.invalidPayload("rent_quote field \(key.stringValue) must be an integer")
            }
            let number = NSDecimalNumber(decimal: rounded)
            let zero = NSDecimalNumber(value: 0)
            guard number.compare(zero) != .orderedAscending else {
                throw ToriiClientError.invalidPayload("rent_quote field \(key.stringValue) must be non-negative")
            }
            let stringValue = number.stringValue
            guard !stringValue.isEmpty, isAsciiDigits(stringValue) else {
                throw ToriiClientError.invalidPayload("rent_quote field \(key.stringValue) must be an unsigned integer")
            }
            return stringValue
        }
        throw ToriiClientError.invalidPayload("rent_quote field \(key.stringValue) was missing or invalid")
    }

    private static func isAsciiDigits(_ value: String) -> Bool {
        guard !value.isEmpty else { return false }
        for scalar in value.unicodeScalars {
            guard scalar.value >= 48 && scalar.value <= 57 else {
                return false
            }
        }
        return true
    }
}

public struct ToriiDaStripeLayout: Decodable, Sendable, Equatable {
    public let totalStripes: UInt32
    public let shardsPerStripe: UInt32
    public let rowParityStripes: UInt32

    private enum CodingKeys: String, CodingKey {
        case totalStripes = "total_stripes"
        case shardsPerStripe = "shards_per_stripe"
        case rowParityStripes = "row_parity_stripes"
    }

    public init(totalStripes: UInt32 = 0,
                shardsPerStripe: UInt32 = 0,
                rowParityStripes: UInt32 = 0) {
        self.totalStripes = totalStripes
        self.shardsPerStripe = shardsPerStripe
        self.rowParityStripes = rowParityStripes
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.totalStripes = try container.decodeIfPresent(UInt32.self, forKey: .totalStripes) ?? 0
        self.shardsPerStripe = try container.decodeIfPresent(UInt32.self, forKey: .shardsPerStripe) ?? 0
        self.rowParityStripes = try container.decodeIfPresent(UInt32.self, forKey: .rowParityStripes) ?? 0
    }
}

public struct ToriiDaIngestReceipt: Decodable, Sendable, Equatable {
    public let clientBlobId: Data
    public let laneId: UInt64
    public let epoch: UInt64
    public let blobHash: Data
    public let chunkRoot: Data
    public let manifestHash: Data
    public let storageTicket: Data
    public let pdpCommitment: Data?
    public let stripeLayout: ToriiDaStripeLayout
    public let queuedAtUnix: UInt64
    public let rentQuote: ToriiDaRentQuote?
    public let operatorSignatureHex: String

    public var clientBlobIdHex: String { clientBlobId.upperHexString() }
    public var blobHashHex: String { blobHash.upperHexString() }
    public var chunkRootHex: String { chunkRoot.upperHexString() }
    public var manifestHashHex: String { manifestHash.upperHexString() }
    public var storageTicketHex: String { storageTicket.upperHexString() }

    private enum CodingKeys: String, CodingKey {
        case clientBlobId = "client_blob_id"
        case laneId = "lane_id"
        case epoch
        case blobHash = "blob_hash"
        case chunkRoot = "chunk_root"
        case manifestHash = "manifest_hash"
        case storageTicket = "storage_ticket"
        case pdpCommitment = "pdp_commitment"
        case stripeLayout = "stripe_layout"
        case queuedAtUnix = "queued_at_unix"
        case rentQuote = "rent_quote"
        case operatorSignature = "operator_signature"
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.clientBlobId = try Self.decodeDigest(from: container, key: .clientBlobId)
        self.laneId = try container.decode(UInt64.self, forKey: .laneId)
        self.epoch = try container.decode(UInt64.self, forKey: .epoch)
        self.blobHash = try Self.decodeDigest(from: container, key: .blobHash)
        self.chunkRoot = try Self.decodeDigest(from: container, key: .chunkRoot)
        self.manifestHash = try Self.decodeDigest(from: container, key: .manifestHash)
        self.storageTicket = try Self.decodeDigest(from: container, key: .storageTicket)
        if let commitmentB64 = try container.decodeIfPresent(String.self, forKey: .pdpCommitment) {
            guard let decoded = Data(base64Encoded: commitmentB64) else {
                throw ToriiClientError.invalidPayload("receipt.pdp_commitment was not valid base64")
            }
            self.pdpCommitment = decoded
        } else {
            self.pdpCommitment = nil
        }
        self.stripeLayout = try container.decodeIfPresent(ToriiDaStripeLayout.self, forKey: .stripeLayout)
            ?? ToriiDaStripeLayout()
        self.queuedAtUnix = try container.decode(UInt64.self, forKey: .queuedAtUnix)
        self.rentQuote = try container.decodeIfPresent(ToriiDaRentQuote.self, forKey: .rentQuote)
        let signature = try container.decode(String.self, forKey: .operatorSignature)
        self.operatorSignatureHex = signature.uppercased()
    }

    private static func decodeDigest(from container: KeyedDecodingContainer<CodingKeys>,
                                     key: CodingKeys) throws -> Data {
        if let wrapped = try? container.decode([[UInt8]].self, forKey: key),
           let first = wrapped.first {
            return Data(first)
        }
        if let flat = try? container.decode([UInt8].self, forKey: key) {
            return Data(flat)
        }
        throw ToriiClientError.invalidPayload("receipt field \(key.stringValue) must be a byte tuple")
    }
}

struct ToriiDaIngestSubmitPayload: Decodable {
    let status: String
    let duplicate: Bool
    let receipt: ToriiDaIngestReceipt?

    private enum CodingKeys: String, CodingKey {
        case status
        case duplicate
        case receipt
    }
}

struct ToriiDaIngestRequestBuilder {
    let submission: ToriiDaBlobSubmission
    let allowUnsigned: Bool

    init(submission: ToriiDaBlobSubmission, allowUnsigned: Bool = false) {
        self.submission = submission
        self.allowUnsigned = allowUnsigned
    }

    func makeRequestBody() throws -> (body: Data, artifacts: ToriiDaIngestArtifacts) {
        guard !submission.payload.isEmpty else {
            throw ToriiClientError.invalidPayload("payload must contain at least one byte")
        }
        let chunkSize = try normalizeInteger(submission.chunkSize,
                                             field: "chunkSize",
                                             allowZero: false,
                                             upperBound: Int(UInt32.max))
        let digestResult = try resolveClientBlobId()
        let signatureResult = try resolveSignatureDigest()

        var payload: [String: Any] = [:]
        payload["client_blob_id"] = digestResult.encodedTuple
        payload["lane_id"] = NSNumber(value: submission.laneId)
        payload["epoch"] = NSNumber(value: submission.epoch)
        payload["sequence"] = NSNumber(value: submission.sequence)
        payload["blob_class"] = encodeBlobClass(submission.blobClass)
        payload["codec"] = [submission.codec]
        payload["erasure_profile"] = encodeErasureProfile(submission.erasureProfile)
        payload["retention_policy"] = encodeRetentionPolicy(submission.retentionPolicy)
        payload["chunk_size"] = NSNumber(value: chunkSize)
        payload["total_size"] = NSNumber(value: submission.payload.count)
        payload["compression"] = encodeCompression(submission.compression)
        if let manifest = submission.noritoManifest {
            payload["norito_manifest"] = manifest.base64EncodedString()
        } else {
            payload["norito_manifest"] = NSNull()
        }
        payload["payload"] = submission.payload.base64EncodedString()
        payload["metadata"] = encodeMetadata(submission.metadata)
        payload["submitter"] = signatureResult.submitter
        payload["signature"] = signatureResult.signatureHex

        let body = try JSONSerialization.data(withJSONObject: payload, options: [])
        let artifacts = ToriiDaIngestArtifacts(
            clientBlobIdHex: digestResult.digest.upperHexString(),
            submitterPublicKeyHex: signatureResult.submitter,
            signatureHex: signatureResult.signatureHex,
            payloadLength: submission.payload.count
        )
        return (body, artifacts)
    }

    private func resolveClientBlobId() throws -> (digest: Data, encodedTuple: [[NSNumber]]) {
        let digest: Data
        if let explicit = submission.clientBlobId {
            guard explicit.count == 32 else {
                throw ToriiClientError.invalidPayload("clientBlobId must contain exactly 32 bytes")
            }
            digest = explicit
        } else if let hashed = NoritoNativeBridge.shared.blake3Hash(data: submission.payload) {
            guard hashed.count == 32 else {
                throw ToriiClientError.invalidPayload("blake3 hash returned an unexpected digest length")
            }
            digest = hashed
        } else {
            throw ToriiClientError.invalidPayload(
                NoritoNativeBridge.bridgeUnavailableMessage(
                    "NoritoBridge must be linked (or clientBlobId provided) to derive the DA payload digest."
                )
            )
        }
        let encoded = [digest.map { NSNumber(value: $0) }]
        return (digest, encoded)
    }

    private func resolveSignatureDigest() throws -> (submitter: String, signatureHex: String) {
        if let signatureHex = submission.signatureHex {
            let canonicalSignature = try canonicalizeHex(signatureHex, field: "signatureHex")
            if let explicitSubmitter = submission.submitterPublicKeyHex {
                let submitter = try canonicalizePublicKey(explicitSubmitter)
                return (submitter, canonicalSignature)
            }
            guard let derived = try deriveSubmitterMultihash() else {
                throw ToriiClientError.invalidPayload(
                    "submitterPublicKeyHex or privateKey is required when signatureHex is provided"
                )
            }
            return (derived, canonicalSignature)
        }
        guard let privateKey = try loadSigningKey() else {
            if allowUnsigned {
                let digest = SHA256.hash(data: submission.payload)
                let signature = Data(digest).upperHexString()
                let submitter = submission.submitterPublicKeyHex
                    ?? String(repeating: "00", count: 64)
                return (submitter, signature)
            }
            throw ToriiClientError.invalidPayload(
                "privateKey or privateKeyHex is required to sign the payload"
            )
        }
        let signature = try privateKey.signature(for: submission.payload)
        let signatureHex = signature.upperHexString()
        let submitter = try submission.submitterPublicKeyHex.map { try canonicalizePublicKey($0) }
            ?? encodeEd25519Multihash(privateKey.publicKey.rawRepresentation)
        return (submitter, signatureHex)
    }

    private func loadSigningKey() throws -> Curve25519.Signing.PrivateKey? {
        if let explicit = submission.privateKey {
            return try Curve25519.Signing.PrivateKey(rawRepresentation: explicit)
        }
        if let hex = submission.privateKeyHex {
            guard let decoded = Data(hexString: hex) else {
                throw ToriiClientError.invalidPayload("privateKeyHex must be a valid hex string")
            }
            return try Curve25519.Signing.PrivateKey(rawRepresentation: decoded)
        }
        return nil
    }

    private func deriveSubmitterMultihash() throws -> String? {
        guard let key = try loadSigningKey() else {
            return nil
        }
        return encodeEd25519Multihash(key.publicKey.rawRepresentation)
    }

    private func encodeBlobClass(_ value: ToriiDaBlobClass) -> [String: Any] {
        switch value {
        case .taikaiSegment:
            return encodeTaggedEnum(tag: "class", variant: "TaikaiSegment", value: nil)
        case .nexusLaneSidecar:
            return encodeTaggedEnum(tag: "class", variant: "NexusLaneSidecar", value: nil)
        case .governanceArtifact:
            return encodeTaggedEnum(tag: "class", variant: "GovernanceArtifact", value: nil)
        case .custom(let code):
            return encodeTaggedEnum(tag: "class", variant: "Custom", value: NSNumber(value: code))
        }
    }

    private func encodeErasureProfile(_ profile: ToriiDaErasureProfile) -> [String: Any] {
        return [
            "data_shards": NSNumber(value: profile.dataShards),
            "parity_shards": NSNumber(value: profile.parityShards),
            "chunk_alignment": NSNumber(value: profile.chunkAlignment),
            "fec_scheme": encodeFecScheme(profile.fecScheme)
        ]
    }

    private func encodeFecScheme(_ value: ToriiDaFecScheme) -> [String: Any] {
        switch value {
        case .rs12_10:
            return encodeTaggedEnum(tag: "scheme", variant: "Rs12_10", value: nil)
        case .rsWin14_10:
            return encodeTaggedEnum(tag: "scheme", variant: "RsWin14_10", value: nil)
        case .rs18_14:
            return encodeTaggedEnum(tag: "scheme", variant: "Rs18_14", value: nil)
        case .custom(let code):
            return encodeTaggedEnum(tag: "scheme", variant: "Custom", value: NSNumber(value: code))
        }
    }

    private func encodeRetentionPolicy(_ policy: ToriiDaRetentionPolicy) -> [String: Any] {
        return [
            "hot_retention_secs": NSNumber(value: policy.hotRetentionSecs),
            "cold_retention_secs": NSNumber(value: policy.coldRetentionSecs),
            "required_replicas": NSNumber(value: policy.requiredReplicas),
            "storage_class": encodeStorageClass(policy.storageClass),
            "governance_tag": [policy.governanceTag]
        ]
    }

    private func encodeStorageClass(_ storageClass: ToriiDaStorageClass) -> [String: Any] {
        switch storageClass {
        case .hot:
            return encodeTaggedEnum(tag: "type", variant: "Hot", value: nil)
        case .warm:
            return encodeTaggedEnum(tag: "type", variant: "Warm", value: nil)
        case .cold:
            return encodeTaggedEnum(tag: "type", variant: "Cold", value: nil)
        }
    }

    private func encodeCompression(_ compression: ToriiDaCompression) -> [String: Any] {
        switch compression {
        case .identity:
            return encodeTaggedEnum(tag: "kind", variant: "Identity", value: nil)
        case .gzip:
            return encodeTaggedEnum(tag: "kind", variant: "Gzip", value: nil)
        case .deflate:
            return encodeTaggedEnum(tag: "kind", variant: "Deflate", value: nil)
        case .zstd:
            return encodeTaggedEnum(tag: "kind", variant: "Zstd", value: nil)
        }
    }

    private func encodeMetadata(_ entries: [ToriiDaMetadataEntry]) -> [String: Any] {
        if entries.isEmpty {
            return ["items": []]
        }
        let encoded = entries.map { entry -> [String: Any] in
            var result: [String: Any] = [
                "key": entry.key,
                "value": entry.value.base64EncodedString(),
                "visibility": encodeTaggedEnum(tag: "visibility",
                                               variant: entry.visibility == .public ? "Public" : "GovernanceOnly",
                                               value: nil)
            ]
            result["encryption"] = encodeMetadataEncryption(entry.encryption)
            return result
        }
        return ["items": encoded]
    }

    private func encodeMetadataEncryption(_ encryption: ToriiDaMetadataEncryption) -> [String: Any] {
        switch encryption {
        case .none:
            return ["cipher": "None", "params": NSNull()]
        case .chacha20Poly1305(let label):
            let params: Any = label.map { ["key_label": $0] } ?? NSNull()
            return ["cipher": "ChaCha20Poly1305", "params": params]
        }
    }

    private func encodeTaggedEnum(tag: String,
                                  variant: String,
                                  value: Any?) -> [String: Any] {
        var result: [String: Any] = [tag: variant]
        result["value"] = value ?? NSNull()
        return result
    }

    private func normalizeInteger(_ value: Int,
                                  field: String,
                                  allowZero: Bool,
                                  upperBound: Int) throws -> Int {
        if !allowZero && value == 0 {
            throw ToriiClientError.invalidPayload("\(field) must be greater than zero")
        }
        if value < 0 {
            throw ToriiClientError.invalidPayload("\(field) must be non-negative")
        }
        if value > upperBound {
            throw ToriiClientError.invalidPayload("\(field) exceeds allowed range")
        }
        return value
    }

    private func canonicalizeHex(_ value: String, field: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
            .replacingOccurrences(of: "^0x", with: "", options: .regularExpression)
        guard !trimmed.isEmpty, trimmed.count % 2 == 0 else {
            throw ToriiClientError.invalidPayload("\(field) must be an even-length hex string")
        }
        guard trimmed.range(of: "^[0-9a-fA-F]+$", options: .regularExpression) != nil else {
            throw ToriiClientError.invalidPayload("\(field) must be a hex string")
        }
        return trimmed.uppercased()
    }

    private func canonicalizePublicKey(_ value: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("submitterPublicKeyHex must be a non-empty string")
        }
        if let separator = trimmed.firstIndex(of: ":") {
            let body = String(trimmed[trimmed.index(after: separator)...])
            return try canonicalizeMultihashHex(body)
        }
        return try canonicalizeMultihashHex(trimmed)
    }

    private func canonicalizeMultihashHex(_ value: String) throws -> String {
        let cleaned = try canonicalizeHex(value, field: "submitterPublicKeyHex")
        guard let bytes = Data(hexString: cleaned) else {
            throw ToriiClientError.invalidPayload("submitterPublicKeyHex must decode to bytes")
        }
        guard !bytes.isEmpty else {
            throw ToriiClientError.invalidPayload("submitterPublicKeyHex must contain multihash bytes")
        }
        // Basic validation: ensure we can walk the varints.
        var index = 0
        try skipVarint(bytes: bytes, index: &index, field: "submitterPublicKeyHex.fn")
        try skipVarint(bytes: bytes, index: &index, field: "submitterPublicKeyHex.len")
        guard index < bytes.count else {
            throw ToriiClientError.invalidPayload("submitterPublicKeyHex missing payload bytes")
        }
        return cleaned
    }

    private func skipVarint(bytes: Data, index: inout Int, field: String) throws {
        var consumed = false
        while index < bytes.count {
            let byte = bytes[index]
            index += 1
            consumed = true
            if (byte & 0x80) == 0 {
                break
            }
        }
        if !consumed || index > bytes.count {
            throw ToriiClientError.invalidPayload("\(field) missing varint bytes")
        }
    }

    private func encodeEd25519Multihash(_ publicKey: Data) -> String {
        var bytes: [UInt8] = []
        bytes.append(contentsOf: encodeVarint(Int(ToriiDaEd25519FunctionCode)))
        bytes.append(contentsOf: encodeVarint(publicKey.count))
        bytes.append(contentsOf: publicKey)
        return Data(bytes).upperHexString()
    }

    private func encodeVarint(_ value: Int) -> [UInt8] {
        var remaining = value
        var output: [UInt8] = []
        repeat {
            var next = UInt8(remaining & 0x7F)
            remaining >>= 7
            if remaining != 0 {
                next |= 0x80
            }
            output.append(next)
        } while remaining != 0
        return output
    }
}

extension Data {
    func upperHexString() -> String {
        map { String(format: "%02X", $0) }.joined()
    }
}
