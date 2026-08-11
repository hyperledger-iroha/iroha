import Foundation
import CryptoKit

private let ToriiDaEd25519FunctionCode: UInt8 = 0xED
private let ToriiDaIngestSigningDomainV1 = Data("iroha:da-ingest-request:v1\0".utf8)
private let ToriiDaIngestContentDomainV1 = Data("iroha:da-ingest-request:content:v1\0".utf8)

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
    public var metadata: [ToriiDaMetadataEntry]
    public var noritoManifest: Data?
    public var clientBlobId: Data?
    public var signerPublicKeyHex: String?
    public var signatureHex: String?
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
        let data: Data
        if let wrapped = try? container.decode([[UInt8]].self, forKey: key) {
            guard wrapped.count == 1, let first = wrapped.first else {
                throw ToriiClientError.invalidPayload("receipt field \(key.stringValue) must be a single byte tuple")
            }
            data = Data(first)
        } else if let flat = try? container.decode([UInt8].self, forKey: key) {
            data = Data(flat)
        } else {
            throw ToriiClientError.invalidPayload("receipt field \(key.stringValue) must be a byte tuple")
        }
        guard data.count == 32 else {
            throw ToriiClientError.invalidPayload("receipt field \(key.stringValue) must be 32 bytes")
        }
        return data
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

    init(submission: ToriiDaBlobSubmission) {
        self.submission = submission
    }

    func makeRequestBody() throws -> (body: Data, artifacts: ToriiDaIngestArtifacts) {
        guard !submission.payload.isEmpty else {
            throw ToriiClientError.invalidPayload("payload must contain at least one byte")
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
            guard submission.owner == submission.owner.trimmingCharacters(in: .whitespacesAndNewlines) else {
                throw AccountAddressError.unsupportedAddressFormat
            }
            ownerAddress = try AccountAddress.parseEncoded(submission.owner)
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
        payload["payload"] = submission.payload.base64EncodedString()
        payload["metadata"] = encodeMetadata(submission.metadata)
        payload["signatures"] = [[
            "signer": signatureResult.signer,
            "signature": signatureResult.signatureHex,
        ]]

        let body = try JSONSerialization.data(withJSONObject: payload, options: [])
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
        try appendLengthPrefixed(submission.payload, to: &preimage)

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
                let signer = try canonicalizePublicKey(explicitSigner)
                return (signer, canonicalSignature)
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
        let signer = try submission.signerPublicKeyHex.map { try canonicalizePublicKey($0) }
            ?? encodeEd25519Multihash(privateKey.publicKey.rawRepresentation)
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

    private func canonicalizePublicKey(_ value: String) throws -> String {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty else {
            throw ToriiClientError.invalidPayload("signerPublicKeyHex must be a non-empty string")
        }
        if let separator = trimmed.firstIndex(of: ":") {
            let body = String(trimmed[trimmed.index(after: separator)...])
            return try canonicalizeMultihashHex(body)
        }
        return try canonicalizeMultihashHex(trimmed)
    }

    private func canonicalizeMultihashHex(_ value: String) throws -> String {
        let cleaned = try canonicalizeHex(value, field: "signerPublicKeyHex")
        guard let bytes = Data(hexString: cleaned) else {
            throw ToriiClientError.invalidPayload("signerPublicKeyHex must decode to bytes")
        }
        guard !bytes.isEmpty else {
            throw ToriiClientError.invalidPayload("signerPublicKeyHex must contain multihash bytes")
        }
        // Basic validation: ensure we can walk the varints.
        var index = 0
        try skipVarint(bytes: bytes, index: &index, field: "signerPublicKeyHex.fn")
        try skipVarint(bytes: bytes, index: &index, field: "signerPublicKeyHex.len")
        guard index < bytes.count else {
            throw ToriiClientError.invalidPayload("signerPublicKeyHex missing payload bytes")
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
