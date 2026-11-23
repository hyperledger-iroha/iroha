import Foundation
import CryptoKit

private let ToriiDaIngestSigningDomainV1 = Data("iroha:da-ingest-request:v1\0".utf8)
private let ToriiDaIngestContentDomainV1 = Data("iroha:da-ingest-request:content:v1\0".utf8)

func requireExactDaFields<K: CodingKey>(
    from decoder: Decoder,
    codingKeys: [K],
    context: String
) throws {
    let raw = try decoder.container(keyedBy: ToriiAnyCodingKey.self)
    let expected = Set(codingKeys.map(\.stringValue))
    if let unknown = raw.allKeys.first(where: { !expected.contains($0.stringValue) }) {
        throw DecodingError.dataCorruptedError(
            forKey: unknown,
            in: raw,
            debugDescription: "\(context) contains unknown field `\(unknown.stringValue)`"
        )
    }
    let present = Set(raw.allKeys.map(\.stringValue))
    if let missing = codingKeys.first(where: { !present.contains($0.stringValue) }) {
        throw DecodingError.keyNotFound(
            ToriiAnyCodingKey(missing.stringValue),
            DecodingError.Context(
                codingPath: decoder.codingPath,
                debugDescription: "\(context) is missing required field `\(missing.stringValue)`"
            )
        )
    }
}

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
    public var rowParityStripes: UInt16
    public var chunkAlignment: UInt16
    public var fecScheme: ToriiDaFecScheme

    public init(dataShards: UInt16 = 10,
                parityShards: UInt16 = 4,
                rowParityStripes: UInt16 = 0,
                chunkAlignment: UInt16 = 10,
                fecScheme: ToriiDaFecScheme = .rs12_10) {
        self.dataShards = dataShards
        self.parityShards = parityShards
        self.rowParityStripes = rowParityStripes
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

extension ToriiDaRetentionPolicy: Decodable {
    private enum CodingKeys: String, CodingKey, CaseIterable {
        case hot = "hot_retention_secs"
        case cold = "cold_retention_secs"
        case replicas = "required_replicas"
        case storage = "storage_class"
        case tag = "governance_tag"
    }

    private enum StorageKeys: String, CodingKey, CaseIterable {
        case type, value
    }

    public init(from decoder: Decoder) throws {
        try requireExactDaFields(from: decoder, codingKeys: CodingKeys.allCases, context: "retention_policy")
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        let policy = try fields.superDecoder(forKey: .storage)
        try requireExactDaFields(from: policy, codingKeys: StorageKeys.allCases, context: "storage_class")
        let storage = try policy.container(keyedBy: StorageKeys.self)
        guard try storage.decodeNil(forKey: .value) else {
            throw ToriiClientError.invalidPayload("storage_class must have a null value")
        }
        let storageClass: ToriiDaStorageClass
        switch try storage.decode(String.self, forKey: .type) {
        case "Hot": storageClass = .hot
        case "Warm": storageClass = .warm
        case "Cold": storageClass = .cold
        default: throw ToriiClientError.invalidPayload("unknown DA storage class")
        }
        let tag = try fields.decode([String].self, forKey: .tag)
        let replicas = try fields.decode(UInt16.self, forKey: .replicas)
        guard tag.count == 1, let governanceTag = tag.first, !governanceTag.isEmpty, replicas > 0 else {
            throw ToriiClientError.invalidPayload("retention_policy requires a governance tag and a positive replica count")
        }
        self.init(hotRetentionSecs: try fields.decode(UInt64.self, forKey: .hot),
                  coldRetentionSecs: try fields.decode(UInt64.self, forKey: .cold),
                  requiredReplicas: replicas, storageClass: storageClass, governanceTag: governanceTag)
    }
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
    public var networkId: NetworkId
    public var owner: String
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
    /// Exact compressed wire bytes when compression is selected. The payload remains the original data.
    public var compressedPayload: Data?
    var wireBytes: Data { compressedPayload ?? payload }
    public var metadata: [ToriiDaMetadataEntry]
    public var noritoManifest: Data?
    public var clientBlobId: Data?
    public var signerPublicKeyHex: String?
    public var signatureHex: String?
    /// External witness over the independently verified pin scope on a follow-up submission.
    public var pinScopeSignature: Data?
    public var privateKey: Data?
    public var privateKeyHex: String?

    public init(networkId: NetworkId,
                owner: String,
                payload: Data,
                chunkSize: Int = 262_144,
                laneId: UInt64 = 0,
                epoch: UInt64 = 0,
                sequence: UInt64 = 0,
                blobClass: ToriiDaBlobClass = .taikaiSegment,
                codec: String = "application/octet-stream",
                erasureProfile: ToriiDaErasureProfile = .default,
                retentionPolicy: ToriiDaRetentionPolicy = .default,
                compression: ToriiDaCompression = .identity,
                compressedPayload: Data? = nil,
                metadata: [ToriiDaMetadataEntry] = [],
                noritoManifest: Data? = nil,
                clientBlobId: Data? = nil,
                signerPublicKeyHex: String? = nil,
                signatureHex: String? = nil,
                privateKey: Data? = nil,
                privateKeyHex: String? = nil) {
        self.networkId = networkId
        self.owner = owner
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
        self.compressedPayload = compressedPayload
        self.metadata = metadata
        self.noritoManifest = noritoManifest
        self.clientBlobId = clientBlobId
        self.signerPublicKeyHex = signerPublicKeyHex
        self.signatureHex = signatureHex
        self.privateKey = privateKey
        self.privateKeyHex = privateKeyHex
    }
}

public struct ToriiDaIngestArtifacts: Sendable, Equatable {
    public let clientBlobIdHex: String
    public let payloadHashHex: String
    public let signerPublicKeyHex: String
    public let signatureHex: String
    public let signingDigestHex: String
    public let payloadLength: Int
}

public struct ToriiDaIngestSubmitResult: Sendable, Equatable {
    public let status: String
    public let duplicate: Bool
    public let receipt: ToriiDaIngestReceipt?
    public let pinScope: ToriiDaPinScope?
    public let artifacts: ToriiDaIngestArtifacts
    public let pdpCommitmentHeaderBase64: String?

    public init(status: String,
                duplicate: Bool,
                receipt: ToriiDaIngestReceipt?,
                pinScope: ToriiDaPinScope? = nil,
                artifacts: ToriiDaIngestArtifacts,
                pdpCommitmentHeaderBase64: String?) {
        self.status = status
        self.duplicate = duplicate
        self.receipt = receipt
        self.pinScope = pinScope
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
    public let rowParityStripes: UInt16

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case totalStripes = "total_stripes"
        case shardsPerStripe = "shards_per_stripe"
        case rowParityStripes = "row_parity_stripes"
    }

    public init(totalStripes: UInt32 = 0,
                shardsPerStripe: UInt32 = 0,
                rowParityStripes: UInt16 = 0) {
        self.totalStripes = totalStripes
        self.shardsPerStripe = shardsPerStripe
        self.rowParityStripes = rowParityStripes
    }

    public init(from decoder: Decoder) throws {
        try requireExactDaFields(
            from: decoder,
            codingKeys: CodingKeys.allCases,
            context: "receipt.stripe_layout"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.totalStripes = try container.decode(UInt32.self, forKey: .totalStripes)
        self.shardsPerStripe = try container.decode(UInt32.self, forKey: .shardsPerStripe)
        self.rowParityStripes = try container.decode(UInt16.self, forKey: .rowParityStripes)
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
    public let retentionPolicy: ToriiDaRetentionPolicy
    public let rentQuote: ToriiDaRentQuote
    public let operatorSignatureHex: String

    public var clientBlobIdHex: String { clientBlobId.upperHexString() }
    public var blobHashHex: String { blobHash.upperHexString() }
    public var chunkRootHex: String { chunkRoot.upperHexString() }
    public var manifestHashHex: String { manifestHash.upperHexString() }
    public var storageTicketHex: String { storageTicket.upperHexString() }

    private enum CodingKeys: String, CodingKey, CaseIterable {
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
        case retentionPolicy = "retention_policy"
        case rentQuote = "rent_quote"
        case operatorSignature = "operator_signature"
    }

    public init(from decoder: Decoder) throws {
        try requireExactDaFields(
            from: decoder,
            codingKeys: CodingKeys.allCases,
            context: "DA ingest receipt"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        self.clientBlobId = try decodeDaDigest(from: container, key: .clientBlobId)
        self.laneId = try container.decode(UInt64.self, forKey: .laneId)
        self.epoch = try container.decode(UInt64.self, forKey: .epoch)
        self.blobHash = try decodeDaDigest(from: container, key: .blobHash)
        self.chunkRoot = try decodeDaDigest(from: container, key: .chunkRoot)
        self.manifestHash = try decodeDaDigest(from: container, key: .manifestHash)
        self.storageTicket = try decodeDaDigest(from: container, key: .storageTicket)
        if let commitmentB64 = try container.decodeIfPresent(String.self, forKey: .pdpCommitment) {
            guard let decoded = Data(base64Encoded: commitmentB64) else {
                throw ToriiClientError.invalidPayload("receipt.pdp_commitment was not valid base64")
            }
            self.pdpCommitment = decoded
        } else {
            self.pdpCommitment = nil
        }
        self.stripeLayout = try container.decode(ToriiDaStripeLayout.self, forKey: .stripeLayout)
        self.queuedAtUnix = try container.decode(UInt64.self, forKey: .queuedAtUnix)
        self.retentionPolicy = try container.decode(ToriiDaRetentionPolicy.self, forKey: .retentionPolicy)
        self.rentQuote = try container.decode(ToriiDaRentQuote.self, forKey: .rentQuote)
        let signature = try container.decode(String.self, forKey: .operatorSignature)
        self.operatorSignatureHex = try requireCanonicalDaSignature(signature, field: "DA receipt operator")
    }

}

private func decodeDaDigest<Key: CodingKey>(from fields: KeyedDecodingContainer<Key>, key: Key) throws -> Data {
    let tuple = try fields.decode([[UInt8]].self, forKey: key)
    guard tuple.count == 1, let bytes = tuple.first, bytes.count == 32 else {
        throw ToriiClientError.invalidPayload("DA field \(key.stringValue) requires a single 32-byte digest tuple")
    }
    return Data(bytes)
}

/// Producer scope returned by Torii, including the digest of the complete proposed commitment.
public struct ToriiDaPinScope: Codable, Equatable, Sendable {
    public let networkId: NetworkId
    public let owner: String
    public let requestAuthorizationDigest: Data
    public let laneId: UInt32
    public let epoch: UInt64
    public let sequence: UInt64
    public let storageTicket: Data
    public let manifestHash: Data
    public let commitmentDigest: Data
    public let alias: String?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case networkId = "network_id", owner, laneId = "lane_id", epoch, sequence, alias
        case requestAuthorizationDigest = "request_authorization_digest", storageTicket = "storage_ticket"
        case manifestHash = "manifest_hash", commitmentDigest = "commitment_digest"
    }

    public init(from decoder: Decoder) throws {
        try requireExactDaFields(from: decoder, codingKeys: CodingKeys.allCases, context: "DA pin scope")
        let fields = try decoder.container(keyedBy: CodingKeys.self)
        networkId = try fields.decode(NetworkId.self, forKey: .networkId)
        owner = try fields.decode(String.self, forKey: .owner)
        _ = try exactCanonicalToriiAccountAddress(owner)
        requestAuthorizationDigest = try decodeDaDigest(from: fields, key: .requestAuthorizationDigest)
        laneId = try fields.decode(UInt32.self, forKey: .laneId)
        epoch = try fields.decode(UInt64.self, forKey: .epoch)
        sequence = try fields.decode(UInt64.self, forKey: .sequence)
        storageTicket = try decodeDaDigest(from: fields, key: .storageTicket)
        manifestHash = try decodeDaDigest(from: fields, key: .manifestHash)
        commitmentDigest = try decodeDaDigest(from: fields, key: .commitmentDigest)
        alias = try fields.decodeIfPresent(String.self, forKey: .alias).map {
            try requireDaPinIntentAlias($0, field: "DA pin alias")
        }
    }

    public func encode(to encoder: Encoder) throws {
        var fields = encoder.container(keyedBy: CodingKeys.self)
        try fields.encode(networkId, forKey: .networkId)
        try fields.encode(owner, forKey: .owner)
        try fields.encode([Array(requestAuthorizationDigest)], forKey: .requestAuthorizationDigest)
        try fields.encode(laneId, forKey: .laneId)
        try fields.encode(epoch, forKey: .epoch)
        try fields.encode(sequence, forKey: .sequence)
        try fields.encode([Array(storageTicket)], forKey: .storageTicket)
        try fields.encode([Array(manifestHash)], forKey: .manifestHash)
        try fields.encode([Array(commitmentDigest)], forKey: .commitmentDigest)
        try fields.encode(alias, forKey: .alias)
    }
}

struct ToriiDaIngestSubmitPayload: Decodable {
    let status: String
    let duplicate: Bool
    let receipt: ToriiDaIngestReceipt?
    let pinScope: ToriiDaPinScope?

    private enum CodingKeys: String, CodingKey {
        case status
        case duplicate
        case receipt
        case pinScope = "pin_scope"
    }
}

enum ToriiDaUpload {
    static let partBytes = 1024 * 1024

    /// Submit a signed metadata body with an empty payload over individually authenticated parts.
    static func transfer<Result>(_ submission: ToriiDaBlobSubmission, metadata: Data,
                                 post: (String, Data, Int) async throws -> (Data, Result)) async throws -> (Data, Result) {
        let bytes = submission.wireBytes
        guard bytes.count > partBytes, bytes.count <= 64 * partBytes,
              let hash = NoritoNativeBridge.shared.blake3Hash(data: bytes), hash.count == 32,
              let request = try JSONSerialization.jsonObject(with: metadata) as? [String: Any],
              request["payload"] as? String == "" else {
            throw ToriiClientError.invalidPayload("invalid signed multipart DA metadata or payload")
        }
        let upload: [String: Any] = ["network_id": submission.networkId.literal, "lane_id": submission.laneId,
                                      "epoch": submission.epoch, "wire_bytes": bytes.count, "wire_hash": [Array(hash)]]
        let count = (bytes.count + partBytes - 1) / partBytes
        func exchange(_ kind: String, _ value: Any, _ status: Int) async throws -> (Data, Result) {
            let action: [String: Any] = ["kind": kind, "value": value]
            let body = try JSONSerialization.data(withJSONObject: ["upload": upload, "action": action], options: [.withoutEscapingSlashes])
            return try await post("/v1/da/ingest/parts", body, status)
        }
        func progress(_ kind: String, _ value: Any) async throws -> Int {
            let (body, _) = try await exchange(kind, value, 200)
            let next = try JSONDecoder().decode(DaUploadProgress.self, from: body).nextPart
            guard (0...count).contains(next) else {
                throw ToriiClientError.invalidPayload("invalid DA upload progress")
            }
            return next
        }
        let start = try await progress("Begin", NSNull())
        for index in start..<count {
            let first = bytes.startIndex + index * partBytes
            let end = min(first + partBytes, bytes.endIndex)
            let part: [String: Any] = ["index": index, "bytes_b64": bytes[first..<end].base64EncodedString()]
            guard try await progress("Put", part) == index + 1 else {
                throw ToriiClientError.invalidPayload("inconsistent DA part acknowledgment")
            }
        }
        return try await exchange("Finish", request, 202)
    }

    private struct DaUploadProgress: Decodable {
        let nextPart: Int

        private enum CodingKeys: String, CodingKey, CaseIterable {
            case nextPart = "next_part"
        }

        init(from decoder: Decoder) throws {
            try requireExactDaFields(from: decoder, codingKeys: CodingKeys.allCases, context: "DA upload progress")
            nextPart = try decoder.container(keyedBy: CodingKeys.self).decode(Int.self, forKey: .nextPart)
        }
    }
}

struct ToriiDaIngestRequestBuilder {
    let submission: ToriiDaBlobSubmission

    init(submission: ToriiDaBlobSubmission) {
        self.submission = submission
    }

    func makeRequestBody(includePayload: Bool = true) throws -> (body: Data, artifacts: ToriiDaIngestArtifacts) {
        guard !submission.payload.isEmpty else {
            throw ToriiClientError.invalidPayload("payload must contain at least one byte")
        }
        guard submission.payload.count <= 64 * 1024 * 1024 else {
            throw ToriiClientError.invalidPayload("DA payload exceeds 64 MiB")
        }
        guard (submission.compression == .identity) == (submission.compressedPayload == nil),
              !submission.wireBytes.isEmpty, submission.wireBytes.count <= 64 * 1024 * 1024 else {
            throw ToriiClientError.invalidPayload("DA compression requires a bounded compressed payload")
        }
        let chunkSize = try normalizeInteger(submission.chunkSize,
                                             field: "chunkSize",
                                             allowZero: false,
                                             upperBound: Int(UInt32.max))
        guard UInt32(exactly: submission.laneId) != nil else {
            throw ToriiClientError.invalidPayload("laneId exceeds UInt32 range")
        }
        let ownerAddress: AccountAddress
        do {
            ownerAddress = try exactCanonicalToriiAccountAddress(submission.owner).address
        } catch {
            throw ToriiClientError.invalidPayload("owner must be an exact canonical I105 account id")
        }
        let digestResult = try resolveClientBlobId()
        let payloadHash = try resolvePayloadHash()
        let signingDigest = try makeSigningDigest(
            clientBlobId: digestResult.digest,
            payloadHash: payloadHash,
            ownerAddress: ownerAddress,
            chunkSize: UInt32(chunkSize)
        )
        let signatureResult = try resolveSignatureDigest(signingDigest: signingDigest)

        var payload: [String: Any] = [:]
        payload["network_id"] = submission.networkId.literal
        payload["owner"] = submission.owner
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
        payload["payload_hash"] = [payloadHash.map { NSNumber(value: $0) }]
        payload["compression"] = encodeCompression(submission.compression)
        if let manifest = submission.noritoManifest {
            payload["norito_manifest"] = manifest.base64EncodedString()
        } else {
            payload["norito_manifest"] = NSNull()
        }
        payload["payload"] = includePayload ? submission.wireBytes.base64EncodedString() : ""
        payload["metadata"] = encodeMetadata(submission.metadata)
        payload["signatures"] = [[
            "signer": signatureResult.signer,
            "signature": signatureResult.signatureHex,
        ]]

        if let witness = submission.pinScopeSignature {
            guard witness.count == 64 else {
                throw ToriiClientError.invalidPayload("pin scope requires a 64-byte Ed25519 signature")
            }
            payload["pin_scope_signatures"] = [["signer": signatureResult.signer, "signature": witness.upperHexString()]]
        } else {
            payload["pin_scope_signatures"] = []
        }
        let body = try JSONSerialization.data(withJSONObject: payload, options: [.withoutEscapingSlashes])
        let artifacts = ToriiDaIngestArtifacts(
            clientBlobIdHex: digestResult.digest.upperHexString(),
            payloadHashHex: payloadHash.upperHexString(),
            signerPublicKeyHex: signatureResult.signer,
            signatureHex: signatureResult.signatureHex,
            signingDigestHex: signingDigest.upperHexString(),
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

    private func resolvePayloadHash() throws -> Data {
        guard let digest = NoritoNativeBridge.shared.blake3Hash(data: submission.payload),
              digest.count == 32 else {
            throw ToriiClientError.invalidPayload(
                NoritoNativeBridge.bridgeUnavailableMessage(
                    "NoritoBridge must be linked to derive the canonical DA payload commitment."
                )
            )
        }
        return digest
    }

    private func makeSigningDigest(clientBlobId: Data,
                                   payloadHash: Data,
                                   ownerAddress: AccountAddress,
                                   chunkSize: UInt32) throws -> Data {
        guard clientBlobId.count == 32 else {
            throw ToriiClientError.invalidPayload("clientBlobId must contain exactly 32 bytes")
        }
        guard payloadHash.count == 32 else {
            throw ToriiClientError.invalidPayload("payload hash must contain exactly 32 bytes")
        }
        guard let laneId = UInt32(exactly: submission.laneId),
              let totalSize = UInt64(exactly: submission.payload.count),
              let metadataCount = UInt64(exactly: submission.metadata.count) else {
            throw ToriiClientError.invalidPayload("DA signing intent exceeds supported integer range")
        }

        var preimage = ToriiDaIngestContentDomainV1
        preimage.append(clientBlobId)

        let blobClass: (UInt8, UInt16)
        switch submission.blobClass {
        case .taikaiSegment:
            blobClass = (0, 0)
        case .nexusLaneSidecar:
            blobClass = (1, 0)
        case .governanceArtifact:
            blobClass = (2, 0)
        case .custom(let value):
            blobClass = (3, value)
        }
        preimage.append(blobClass.0)
        appendLittleEndian(blobClass.1, to: &preimage)
        try appendLengthPrefixed(submission.codec, to: &preimage)

        appendLittleEndian(submission.erasureProfile.dataShards, to: &preimage)
        appendLittleEndian(submission.erasureProfile.parityShards, to: &preimage)
        appendLittleEndian(submission.erasureProfile.rowParityStripes, to: &preimage)
        appendLittleEndian(submission.erasureProfile.chunkAlignment, to: &preimage)
        let fecScheme: (UInt8, UInt16)
        switch submission.erasureProfile.fecScheme {
        case .rs12_10:
            fecScheme = (0, 0)
        case .rsWin14_10:
            fecScheme = (1, 0)
        case .rs18_14:
            fecScheme = (2, 0)
        case .custom(let value):
            fecScheme = (3, value)
        }
        preimage.append(fecScheme.0)
        appendLittleEndian(fecScheme.1, to: &preimage)

        let retention = submission.retentionPolicy
        appendLittleEndian(retention.hotRetentionSecs, to: &preimage)
        appendLittleEndian(retention.coldRetentionSecs, to: &preimage)
        appendLittleEndian(retention.requiredReplicas, to: &preimage)
        switch retention.storageClass {
        case .hot:
            preimage.append(0)
        case .warm:
            preimage.append(1)
        case .cold:
            preimage.append(2)
        }
        try appendLengthPrefixed(retention.governanceTag, to: &preimage)

        appendLittleEndian(chunkSize, to: &preimage)
        switch submission.compression {
        case .identity:
            preimage.append(0)
        case .gzip:
            preimage.append(1)
        case .deflate:
            preimage.append(2)
        case .zstd:
            preimage.append(3)
        }

        if let manifest = submission.noritoManifest {
            preimage.append(1)
            try appendLengthPrefixed(manifest, to: &preimage)
        } else {
            preimage.append(0)
        }
        try appendLengthPrefixed(submission.wireBytes, to: &preimage)

        appendLittleEndian(metadataCount, to: &preimage)
        for entry in submission.metadata {
            try appendLengthPrefixed(entry.key, to: &preimage)
            try appendLengthPrefixed(entry.value, to: &preimage)
            preimage.append(entry.visibility == .public ? 0 : 1)
            switch entry.encryption {
            case .none:
                preimage.append(0)
            case .chacha20Poly1305(let keyLabel):
                preimage.append(1)
                if let keyLabel {
                    preimage.append(1)
                    try appendLengthPrefixed(keyLabel, to: &preimage)
                } else {
                    preimage.append(0)
                }
            }
        }

        guard let contentHash = NoritoNativeBridge.shared.blake3Hash(data: preimage),
              contentHash.count == 32 else {
            throw ToriiClientError.invalidPayload(
                NoritoNativeBridge.bridgeUnavailableMessage(
                    "NoritoBridge must be linked to hash the canonical DA signing intent."
                )
            )
        }
        var authorization = ToriiDaIngestSigningDomainV1
        authorization.append(submission.networkId.bytes)
        try appendLengthPrefixed(ownerAddress.canonicalBytes(), to: &authorization)
        appendLittleEndian(laneId, to: &authorization)
        appendLittleEndian(submission.epoch, to: &authorization)
        appendLittleEndian(submission.sequence, to: &authorization)
        authorization.append(payloadHash)
        appendLittleEndian(totalSize, to: &authorization)
        authorization.append(contentHash)
        guard let digest = NoritoNativeBridge.shared.blake3Hash(data: authorization),
              digest.count == 32 else {
            throw ToriiClientError.invalidPayload(
                NoritoNativeBridge.bridgeUnavailableMessage(
                    "NoritoBridge must be linked to hash the canonical DA authorization."
                )
            )
        }
        return digest
    }

    private func appendLittleEndian<T: FixedWidthInteger>(_ value: T,
                                                          to output: inout Data) {
        var littleEndian = value.littleEndian
        Swift.withUnsafeBytes(of: &littleEndian) { output.append(contentsOf: $0) }
    }

    private func appendLengthPrefixed(_ value: String,
                                      to output: inout Data) throws {
        try appendLengthPrefixed(Data(value.utf8), to: &output)
    }

    private func appendLengthPrefixed(_ value: Data,
                                      to output: inout Data) throws {
        guard let length = UInt64(exactly: value.count) else {
            throw ToriiClientError.invalidPayload("DA signing field exceeds UInt64 length")
        }
        appendLittleEndian(length, to: &output)
        output.append(value)
    }

    private func resolveSignatureDigest(signingDigest: Data) throws
        -> (signer: String, signatureHex: String) {
        if let signatureHex = submission.signatureHex {
            let canonicalSignature = try canonicalizeHex(signatureHex, field: "signatureHex")
            if let explicitSigner = submission.signerPublicKeyHex {
                _ = try governancePublicKeyOrderV1(explicitSigner, codingPath: [])
                return (explicitSigner, canonicalSignature)
            }
            guard let derived = try deriveSignerMultihash() else {
                throw ToriiClientError.invalidPayload(
                    "signerPublicKeyHex or privateKey is required when signatureHex is provided"
                )
            }
            return (derived, canonicalSignature)
        }
        guard let privateKey = try loadSigningKey() else {
            throw ToriiClientError.invalidPayload(
                "privateKey or privateKeyHex is required to sign the payload"
            )
        }
        let signature = try privateKey.signature(for: signingDigest)
        let signatureHex = signature.upperHexString()
        let signer = CanonicalNorito.publicKeyMultihash(algorithm: .ed25519, payload: privateKey.publicKey.rawRepresentation)
        if let explicit = submission.signerPublicKeyHex, explicit != signer {
            throw ToriiClientError.invalidPayload("signerPublicKeyHex must match the signing private key")
        }
        return (signer, signatureHex)
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

    private func deriveSignerMultihash() throws -> String? {
        guard let key = try loadSigningKey() else {
            return nil
        }
        return CanonicalNorito.publicKeyMultihash(algorithm: .ed25519, payload: key.publicKey.rawRepresentation)
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
            "row_parity_stripes": NSNumber(value: profile.rowParityStripes),
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
}

extension Data {
    func upperHexString() -> String {
        map { String(format: "%02X", $0) }.joined()
    }
}
