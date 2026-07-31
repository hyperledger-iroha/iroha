import Foundation

private struct ToriiDaCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int?

    init?(stringValue: String) {
        self.stringValue = stringValue
        intValue = nil
    }

    init?(intValue: Int) {
        stringValue = String(intValue)
        self.intValue = intValue
    }
}

private func rejectUnknownDaFields(
    from decoder: Decoder,
    allowed: Set<String>,
    type: String
) throws {
    let container = try decoder.container(keyedBy: ToriiDaCodingKey.self)
    guard container.allKeys.allSatisfy({ allowed.contains($0.stringValue) }) else {
        throw DecodingError.dataCorrupted(
            .init(
                codingPath: decoder.codingPath,
                debugDescription: "\(type) contains an unknown field"
            )
        )
    }
}

private func requireExactDaText(_ value: String, field: String) throws -> String {
    guard !value.isEmpty,
          value == value.trimmingCharacters(in: .whitespacesAndNewlines) else {
        throw ToriiClientError.invalidPayload("\(field) must be exact non-empty text")
    }
    return value
}

func requireDaPinIntentAlias(_ value: String, field: String) throws -> String {
    guard value.utf8.count <= 256 else {
        throw ToriiClientError.invalidPayload(
            "\(field) must contain at most 256 UTF-8 bytes"
        )
    }
    return value
}

private func requireCanonicalDaOwner(_ value: String?, field: String) throws -> String? {
    guard let value else {
        return nil
    }
    let exact = try requireExactDaText(value, field: field)
    let prefix = try AccountAddress.inspectI105NetworkPrefix(exact).chainDiscriminant
    let address = try AccountAddress.fromI105(exact, expectedPrefix: prefix)
    guard try address.toI105(networkPrefix: prefix) == exact else {
        throw ToriiClientError.invalidPayload("\(field) must be a canonical I105 AccountId")
    }
    return exact
}

/// Canonical Norito JSON wrapper for a fixed 32-byte DA digest.
public struct ToriiDaDigest32: Codable, Sendable, Equatable, Hashable {
    public let bytes: [UInt8]

    public init(bytes: [UInt8]) throws {
        guard bytes.count == 32 else {
            throw ToriiClientError.invalidPayload("DA digest must contain exactly 32 bytes")
        }
        self.bytes = bytes
    }

    public init(hex: String) throws {
        var body = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if body.hasPrefix("0x") || body.hasPrefix("0X") {
            body = String(body.dropFirst(2))
        }
        guard body.count == 64, let data = Data(hexString: body), data.count == 32 else {
            throw ToriiClientError.invalidPayload("DA digest must be a 32-byte hex string")
        }
        try self.init(bytes: Array(data))
    }

    public var hex: String {
        Data(bytes).hexEncodedString()
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let wrapper = try container.decode([[UInt8]].self)
        guard wrapper.count == 1, wrapper[0].count == 32 else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "DA digest must use the canonical one-element, 32-byte Norito JSON wrapper"
            )
        }
        self.bytes = wrapper[0]
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode([bytes])
    }
}

/// Canonical checksummed Iroha hash literal used by DA proof fields.
public struct ToriiDaHash: Codable, Sendable, Equatable, Hashable {
    public let literal: String

    public init(_ literal: String) throws {
        guard Self.isCanonical(literal) else {
            throw ToriiClientError.invalidPayload(
                "DA hash must be a canonical `hash:<64 uppercase hex>#<4 uppercase hex>` literal"
            )
        }
        self.literal = literal
    }

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let literal = try container.decode(String.self)
        guard Self.isCanonical(literal) else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "DA hash is not a canonical checksummed Iroha hash literal"
            )
        }
        self.literal = literal
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(literal)
    }

    private static func isCanonical(_ literal: String) -> Bool {
        guard literal.hasPrefix("hash:"), literal.count == 5 + 64 + 1 + 4 else {
            return false
        }
        let bodyStart = literal.index(literal.startIndex, offsetBy: 5)
        let separator = literal.index(bodyStart, offsetBy: 64)
        guard literal[separator] == "#" else {
            return false
        }
        let digest = literal[bodyStart..<separator]
        let checksum = literal[literal.index(after: separator)...]
        guard [digest, checksum].allSatisfy({ component in
            component.utf8.allSatisfy { byte in
                (0x30...0x39).contains(byte) || (0x41...0x46).contains(byte)
            }
        }),
        let checksumValue = UInt16(checksum, radix: 16),
        let lastByte = UInt8(digest.suffix(2), radix: 16),
        lastByte & 1 == 1 else {
            return false
        }
        var crc = UInt16.max
        for byte in "hash:\(digest)".utf8 {
            crc ^= UInt16(byte) << 8
            for _ in 0..<8 {
                crc = (crc & 0x8000) != 0 ? (crc << 1) ^ 0x1021 : crc << 1
            }
        }
        return checksumValue == crc
    }
}

public enum ToriiDaProofScheme: String, Sendable, Equatable, Hashable {
    case merkleSha256 = "MerkleSha256"
}

extension ToriiDaProofScheme: Codable {
    private enum CodingKeys: String, CodingKey {
        case type
        case value
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: ["type", "value"],
            type: "DA proof scheme"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let raw = try container.decode(String.self, forKey: .type)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "unsupported DA proof scheme"
            )
        }
        guard container.contains(.value), try container.decodeNil(forKey: .value) else {
            throw DecodingError.dataCorruptedError(
                forKey: .value,
                in: container,
                debugDescription: "unit DA proof schemes require a null value"
            )
        }
        self = value
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(rawValue, forKey: .type)
        try container.encodeNil(forKey: .value)
    }
}

extension ToriiDaStorageClass: Codable {
    private enum CodingKeys: String, CodingKey {
        case type
        case value
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: ["type", "value"],
            type: "DA storage class"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let raw = try container.decode(String.self, forKey: .type)
        switch raw {
        case "Hot":
            self = .hot
        case "Warm":
            self = .warm
        case "Cold":
            self = .cold
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .type,
                in: container,
                debugDescription: "unsupported DA storage class"
            )
        }
        guard container.contains(.value), try container.decodeNil(forKey: .value) else {
            throw DecodingError.dataCorruptedError(
                forKey: .value,
                in: container,
                debugDescription: "unit DA storage classes require a null value"
            )
        }
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        let wireName: String
        switch self {
        case .hot:
            wireName = "Hot"
        case .warm:
            wireName = "Warm"
        case .cold:
            wireName = "Cold"
        }
        try container.encode(wireName, forKey: .type)
        try container.encodeNil(forKey: .value)
    }
}

public struct ToriiDaProofPolicy: Codable, Sendable, Equatable {
    public let laneId: UInt32
    public let dataspaceId: UInt64
    public let alias: String
    public let proofScheme: ToriiDaProofScheme

    private enum CodingKeys: String, CodingKey {
        case laneId = "lane_id"
        case dataspaceId = "dataspace_id"
        case alias
        case proofScheme = "proof_scheme"
    }

    public init(
        laneId: UInt32,
        dataspaceId: UInt64,
        alias: String,
        proofScheme: ToriiDaProofScheme
    ) throws {
        self.laneId = laneId
        self.dataspaceId = dataspaceId
        self.alias = try requireExactDaText(alias, field: "DA policy alias")
        self.proofScheme = proofScheme
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: ["lane_id", "dataspace_id", "alias", "proof_scheme"],
            type: "DA proof policy"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            laneId: container.decode(UInt32.self, forKey: .laneId),
            dataspaceId: container.decode(UInt64.self, forKey: .dataspaceId),
            alias: container.decode(String.self, forKey: .alias),
            proofScheme: container.decode(ToriiDaProofScheme.self, forKey: .proofScheme)
        )
    }
}

public struct ToriiDaProofPolicyBundle: Codable, Sendable, Equatable {
    public let version: UInt16
    public let policyHash: ToriiDaHash
    public let policies: [ToriiDaProofPolicy]

    private enum CodingKeys: String, CodingKey {
        case version
        case policyHash = "policy_hash"
        case policies
    }

    public init(
        version: UInt16,
        policyHash: ToriiDaHash,
        policies: [ToriiDaProofPolicy]
    ) throws {
        guard version == 1 else {
            throw ToriiClientError.invalidPayload(
                "only DA proof-policy bundle V1 is supported"
            )
        }
        self.version = version
        self.policyHash = policyHash
        self.policies = policies
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: ["version", "policy_hash", "policies"],
            type: "DA proof-policy bundle"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            version: container.decode(UInt16.self, forKey: .version),
            policyHash: container.decode(ToriiDaHash.self, forKey: .policyHash),
            policies: container.decode([ToriiDaProofPolicy].self, forKey: .policies)
        )
    }
}

extension ToriiDaRetentionPolicy: Codable {
    private enum CodingKeys: String, CodingKey {
        case hotRetentionSecs = "hot_retention_secs"
        case coldRetentionSecs = "cold_retention_secs"
        case requiredReplicas = "required_replicas"
        case storageClass = "storage_class"
        case governanceTag = "governance_tag"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: [
                "hot_retention_secs",
                "cold_retention_secs",
                "required_replicas",
                "storage_class",
                "governance_tag",
            ],
            type: "DA retention policy"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let wrapper = try container.decode([String].self, forKey: .governanceTag)
        guard wrapper.count == 1 else {
            throw DecodingError.dataCorruptedError(
                forKey: .governanceTag,
                in: container,
                debugDescription: "DA governance tag must use a one-element wrapper"
            )
        }
        self.init(
            hotRetentionSecs: try container.decode(UInt64.self, forKey: .hotRetentionSecs),
            coldRetentionSecs: try container.decode(UInt64.self, forKey: .coldRetentionSecs),
            requiredReplicas: try container.decode(UInt16.self, forKey: .requiredReplicas),
            storageClass: try container.decode(ToriiDaStorageClass.self, forKey: .storageClass),
            governanceTag: wrapper[0]
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(hotRetentionSecs, forKey: .hotRetentionSecs)
        try container.encode(coldRetentionSecs, forKey: .coldRetentionSecs)
        try container.encode(requiredReplicas, forKey: .requiredReplicas)
        try container.encode(storageClass, forKey: .storageClass)
        try container.encode([governanceTag], forKey: .governanceTag)
    }
}

public struct ToriiDaCommitmentRecord: Codable, Sendable, Equatable {
    public let laneId: UInt32
    public let epoch: UInt64
    public let sequence: UInt64
    public let clientBlobId: ToriiDaDigest32
    public let manifestHash: ToriiDaDigest32
    public let proofScheme: ToriiDaProofScheme
    public let chunkRoot: ToriiDaHash
    public let proofDigest: ToriiDaHash?
    public let retentionClass: ToriiDaRetentionPolicy
    public let storageTicket: ToriiDaDigest32
    public let acknowledgementSignature: String

    private enum CodingKeys: String, CodingKey {
        case laneId = "lane_id"
        case epoch
        case sequence
        case clientBlobId = "client_blob_id"
        case manifestHash = "manifest_hash"
        case proofScheme = "proof_scheme"
        case chunkRoot = "chunk_root"
        case proofDigest = "proof_digest"
        case retentionClass = "retention_class"
        case storageTicket = "storage_ticket"
        case acknowledgementSignature = "acknowledgement_sig"
    }

    public init(
        laneId: UInt32,
        epoch: UInt64,
        sequence: UInt64,
        clientBlobId: ToriiDaDigest32,
        manifestHash: ToriiDaDigest32,
        proofScheme: ToriiDaProofScheme,
        chunkRoot: ToriiDaHash,
        proofDigest: ToriiDaHash?,
        retentionClass: ToriiDaRetentionPolicy,
        storageTicket: ToriiDaDigest32,
        acknowledgementSignature: String
    ) throws {
        guard acknowledgementSignature.utf8.count == 128,
              acknowledgementSignature.utf8.allSatisfy({
                  (0x30...0x39).contains($0) || (0x41...0x46).contains($0)
              }) else {
            throw ToriiClientError.invalidPayload(
                "DA acknowledgement signature must contain 64 canonical uppercase bytes"
            )
        }
        self.laneId = laneId
        self.epoch = epoch
        self.sequence = sequence
        self.clientBlobId = clientBlobId
        self.manifestHash = manifestHash
        self.proofScheme = proofScheme
        self.chunkRoot = chunkRoot
        self.proofDigest = proofDigest
        self.retentionClass = retentionClass
        self.storageTicket = storageTicket
        self.acknowledgementSignature = acknowledgementSignature
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: [
                "lane_id",
                "epoch",
                "sequence",
                "client_blob_id",
                "manifest_hash",
                "proof_scheme",
                "chunk_root",
                "proof_digest",
                "retention_class",
                "storage_ticket",
                "acknowledgement_sig",
            ],
            type: "DA commitment record"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard container.contains(.proofDigest) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: decoder.codingPath,
                    debugDescription: "DA commitment optional fields must be explicit on the wire"
                )
            )
        }
        try self.init(
            laneId: container.decode(UInt32.self, forKey: .laneId),
            epoch: container.decode(UInt64.self, forKey: .epoch),
            sequence: container.decode(UInt64.self, forKey: .sequence),
            clientBlobId: container.decode(ToriiDaDigest32.self, forKey: .clientBlobId),
            manifestHash: container.decode(ToriiDaDigest32.self, forKey: .manifestHash),
            proofScheme: container.decode(ToriiDaProofScheme.self, forKey: .proofScheme),
            chunkRoot: container.decode(ToriiDaHash.self, forKey: .chunkRoot),
            proofDigest: container.decodeIfPresent(ToriiDaHash.self, forKey: .proofDigest),
            retentionClass: container.decode(
                ToriiDaRetentionPolicy.self,
                forKey: .retentionClass
            ),
            storageTicket: container.decode(ToriiDaDigest32.self, forKey: .storageTicket),
            acknowledgementSignature: container.decode(
                String.self,
                forKey: .acknowledgementSignature
            )
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(laneId, forKey: .laneId)
        try container.encode(epoch, forKey: .epoch)
        try container.encode(sequence, forKey: .sequence)
        try container.encode(clientBlobId, forKey: .clientBlobId)
        try container.encode(manifestHash, forKey: .manifestHash)
        try container.encode(proofScheme, forKey: .proofScheme)
        try container.encode(chunkRoot, forKey: .chunkRoot)
        if let proofDigest {
            try container.encode(proofDigest, forKey: .proofDigest)
        } else {
            try container.encodeNil(forKey: .proofDigest)
        }
        try container.encode(retentionClass, forKey: .retentionClass)
        try container.encode(storageTicket, forKey: .storageTicket)
        try container.encode(acknowledgementSignature, forKey: .acknowledgementSignature)
    }
}

public struct ToriiDaCommitmentLocation: Codable, Sendable, Equatable, Hashable {
    public let blockHeight: UInt64
    public let indexInBundle: UInt32

    public init(blockHeight: UInt64, indexInBundle: UInt32) throws {
        guard blockHeight > 0 else {
            throw ToriiClientError.invalidPayload("DA block height must be nonzero")
        }
        self.blockHeight = blockHeight
        self.indexInBundle = indexInBundle
    }

    private enum CodingKeys: String, CodingKey {
        case blockHeight = "block_height"
        case indexInBundle = "index_in_bundle"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: ["block_height", "index_in_bundle"],
            type: "DA commitment location"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let blockHeight = try container.decode(UInt64.self, forKey: .blockHeight)
        guard blockHeight > 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .blockHeight,
                in: container,
                debugDescription: "DA block height must be nonzero"
            )
        }
        self.blockHeight = blockHeight
        indexInBundle = try container.decode(UInt32.self, forKey: .indexInBundle)
    }
}

public struct ToriiDaCommitmentWithLocation: Codable, Sendable, Equatable {
    public let commitment: ToriiDaCommitmentRecord
    public let location: ToriiDaCommitmentLocation

    public init(
        commitment: ToriiDaCommitmentRecord,
        location: ToriiDaCommitmentLocation
    ) {
        self.commitment = commitment
        self.location = location
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: ["commitment", "location"],
            type: "located DA commitment"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        commitment = try container.decode(ToriiDaCommitmentRecord.self, forKey: .commitment)
        location = try container.decode(ToriiDaCommitmentLocation.self, forKey: .location)
    }

    private enum CodingKeys: String, CodingKey {
        case commitment
        case location
    }
}

public enum ToriiDaMerkleDirection: String, Sendable, Equatable, Hashable {
    case left = "Left"
    case right = "Right"
}

extension ToriiDaMerkleDirection: Codable {
    private enum CodingKeys: String, CodingKey {
        case direction
        case value
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: ["direction", "value"],
            type: "DA Merkle direction"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let raw = try container.decode(String.self, forKey: .direction)
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .direction,
                in: container,
                debugDescription: "unsupported DA Merkle direction"
            )
        }
        guard container.contains(.value), try container.decodeNil(forKey: .value) else {
            throw DecodingError.dataCorruptedError(
                forKey: .value,
                in: container,
                debugDescription: "unit DA Merkle directions require a null value"
            )
        }
        self = value
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(rawValue, forKey: .direction)
        try container.encodeNil(forKey: .value)
    }
}

public struct ToriiDaMerklePathItem: Codable, Sendable, Equatable, Hashable {
    public let sibling: ToriiDaHash
    public let direction: ToriiDaMerkleDirection

    public init(sibling: ToriiDaHash, direction: ToriiDaMerkleDirection) {
        self.sibling = sibling
        self.direction = direction
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: ["sibling", "direction"],
            type: "DA Merkle path item"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        sibling = try container.decode(ToriiDaHash.self, forKey: .sibling)
        direction = try container.decode(ToriiDaMerkleDirection.self, forKey: .direction)
    }

    private enum CodingKeys: String, CodingKey {
        case sibling
        case direction
    }
}

public struct ToriiDaCommitmentProof: Codable, Sendable, Equatable {
    public let commitment: ToriiDaCommitmentRecord
    public let location: ToriiDaCommitmentLocation
    /// Header commitment to the V1 tree version, leaf count, and Merkle root.
    public let bundleHash: ToriiDaHash
    public let bundleLength: UInt32
    public let root: ToriiDaHash
    public let path: [ToriiDaMerklePathItem]

    private enum CodingKeys: String, CodingKey {
        case commitment
        case location
        case bundleHash = "bundle_hash"
        case bundleLength = "bundle_len"
        case root
        case path
    }

    public init(
        commitment: ToriiDaCommitmentRecord,
        location: ToriiDaCommitmentLocation,
        bundleHash: ToriiDaHash,
        bundleLength: UInt32,
        root: ToriiDaHash,
        path: [ToriiDaMerklePathItem]
    ) throws {
        guard bundleLength > 0 else {
            throw ToriiClientError.invalidPayload("DA proof bundle length must be nonzero")
        }
        guard path.count <= 32,
              daMerklePathMatchesLocation(
                  path,
                  index: location.indexInBundle,
                  width: bundleLength
              ) else {
            throw ToriiClientError.invalidPayload(
                "DA Merkle path shape does not match its bundle location"
            )
        }
        self.commitment = commitment
        self.location = location
        self.bundleHash = bundleHash
        self.bundleLength = bundleLength
        self.root = root
        self.path = path
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: [
                "commitment",
                "location",
                "bundle_hash",
                "bundle_len",
                "root",
                "path",
            ],
            type: "DA commitment proof"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            commitment: container.decode(
                ToriiDaCommitmentRecord.self,
                forKey: .commitment
            ),
            location: container.decode(ToriiDaCommitmentLocation.self, forKey: .location),
            bundleHash: container.decode(ToriiDaHash.self, forKey: .bundleHash),
            bundleLength: container.decode(UInt32.self, forKey: .bundleLength),
            root: container.decode(ToriiDaHash.self, forKey: .root),
            path: container.decode([ToriiDaMerklePathItem].self, forKey: .path)
        )
    }
}

public struct ToriiDaPinIntent: Codable, Sendable, Equatable {
    public let laneId: UInt32
    public let epoch: UInt64
    public let sequence: UInt64
    public let storageTicket: ToriiDaDigest32
    public let manifestHash: ToriiDaDigest32
    public let alias: String?
    public let owner: String?

    private enum CodingKeys: String, CodingKey {
        case laneId = "lane_id"
        case epoch
        case sequence
        case storageTicket = "storage_ticket"
        case manifestHash = "manifest_hash"
        case alias
        case owner
    }

    public init(
        laneId: UInt32,
        epoch: UInt64,
        sequence: UInt64,
        storageTicket: ToriiDaDigest32,
        manifestHash: ToriiDaDigest32,
        alias: String?,
        owner: String?
    ) throws {
        self.laneId = laneId
        self.epoch = epoch
        self.sequence = sequence
        self.storageTicket = storageTicket
        self.manifestHash = manifestHash
        self.alias = try alias.map {
            try requireDaPinIntentAlias($0, field: "DA pin alias")
        }
        self.owner = try requireCanonicalDaOwner(owner, field: "DA pin owner")
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: [
                "lane_id",
                "epoch",
                "sequence",
                "storage_ticket",
                "manifest_hash",
                "alias",
                "owner",
            ],
            type: "DA pin intent"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        guard container.contains(.alias),
              container.contains(.owner) else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: decoder.codingPath,
                    debugDescription: "DA pin-intent optional fields must be explicit on the wire"
                )
            )
        }
        try self.init(
            laneId: container.decode(UInt32.self, forKey: .laneId),
            epoch: container.decode(UInt64.self, forKey: .epoch),
            sequence: container.decode(UInt64.self, forKey: .sequence),
            storageTicket: container.decode(ToriiDaDigest32.self, forKey: .storageTicket),
            manifestHash: container.decode(ToriiDaDigest32.self, forKey: .manifestHash),
            alias: container.decodeIfPresent(String.self, forKey: .alias),
            owner: container.decodeIfPresent(String.self, forKey: .owner)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(laneId, forKey: .laneId)
        try container.encode(epoch, forKey: .epoch)
        try container.encode(sequence, forKey: .sequence)
        try container.encode(storageTicket, forKey: .storageTicket)
        try container.encode(manifestHash, forKey: .manifestHash)
        if let alias {
            try container.encode(
                requireDaPinIntentAlias(alias, field: "DA pin alias"),
                forKey: .alias
            )
        } else {
            try container.encodeNil(forKey: .alias)
        }
        if let owner {
            try container.encode(
                requireCanonicalDaOwner(owner, field: "DA pin owner"),
                forKey: .owner
            )
        } else {
            try container.encodeNil(forKey: .owner)
        }
    }
}

public struct ToriiDaPinIntentWithLocation: Codable, Sendable, Equatable {
    public let intent: ToriiDaPinIntent
    public let location: ToriiDaCommitmentLocation

    public init(intent: ToriiDaPinIntent, location: ToriiDaCommitmentLocation) {
        self.intent = intent
        self.location = location
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: ["intent", "location"],
            type: "located DA pin intent"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        intent = try container.decode(ToriiDaPinIntent.self, forKey: .intent)
        location = try container.decode(ToriiDaCommitmentLocation.self, forKey: .location)
    }

    private enum CodingKeys: String, CodingKey {
        case intent
        case location
    }
}

public struct ToriiDaPinIntentProof: Codable, Sendable, Equatable {
    public let intent: ToriiDaPinIntent
    public let location: ToriiDaCommitmentLocation
    /// Header commitment to the V1 tree version, leaf count, and Merkle root.
    public let bundleHash: ToriiDaHash
    public let bundleLength: UInt32
    public let root: ToriiDaHash
    public let path: [ToriiDaMerklePathItem]

    private enum CodingKeys: String, CodingKey {
        case intent
        case location
        case bundleHash = "bundle_hash"
        case bundleLength = "bundle_len"
        case root
        case path
    }

    public init(
        intent: ToriiDaPinIntent,
        location: ToriiDaCommitmentLocation,
        bundleHash: ToriiDaHash,
        bundleLength: UInt32,
        root: ToriiDaHash,
        path: [ToriiDaMerklePathItem]
    ) throws {
        guard bundleLength > 0 else {
            throw ToriiClientError.invalidPayload("DA proof bundle length must be nonzero")
        }
        guard path.count <= 32,
              daMerklePathMatchesLocation(
                  path,
                  index: location.indexInBundle,
                  width: bundleLength
              ) else {
            throw ToriiClientError.invalidPayload(
                "DA Merkle path shape does not match its bundle location"
            )
        }
        self.intent = intent
        self.location = location
        self.bundleHash = bundleHash
        self.bundleLength = bundleLength
        self.root = root
        self.path = path
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownDaFields(
            from: decoder,
            allowed: [
                "intent",
                "location",
                "bundle_hash",
                "bundle_len",
                "root",
                "path",
            ],
            type: "DA pin-intent proof"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            intent: container.decode(ToriiDaPinIntent.self, forKey: .intent),
            location: container.decode(ToriiDaCommitmentLocation.self, forKey: .location),
            bundleHash: container.decode(ToriiDaHash.self, forKey: .bundleHash),
            bundleLength: container.decode(UInt32.self, forKey: .bundleLength),
            root: container.decode(ToriiDaHash.self, forKey: .root),
            path: container.decode([ToriiDaMerklePathItem].self, forKey: .path)
        )
    }
}

private func daMerklePathMatchesLocation(
    _ path: [ToriiDaMerklePathItem],
    index initialIndex: UInt32,
    width initialWidth: UInt32
) -> Bool {
    guard initialWidth > 0, initialIndex < initialWidth else {
        return false
    }
    var index = UInt64(initialIndex)
    var width = UInt64(initialWidth)
    var pathIndex = 0
    while width > 1 {
        let expected: ToriiDaMerkleDirection?
        if index % 2 == 1 {
            expected = .left
        } else if index + 1 < width {
            expected = .right
        } else {
            expected = nil
        }
        if let expected {
            guard path.indices.contains(pathIndex),
                  path[pathIndex].direction == expected else {
                return false
            }
            pathIndex += 1
        }
        index /= 2
        width = width / 2 + width % 2
    }
    return pathIndex == path.count
}
