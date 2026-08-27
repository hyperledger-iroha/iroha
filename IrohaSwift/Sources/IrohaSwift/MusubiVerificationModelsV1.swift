import Foundation

// MARK: - Publication mutation values

private func musubiDecodeVarintV1(_ bytes: [UInt8], from start: Int) throws -> (UInt64, Int) {
    var value: UInt64 = 0
    var shift: UInt64 = 0
    var index = start
    while index < bytes.count, shift <= 63 {
        let byte = bytes[index]
        index += 1
        let payload = UInt64(byte & 0x7f)
        guard shift != 63 || payload <= 1 else {
            throw MusubiV1Error.invalidValue("Musubi public-key multihash varint overflows.")
        }
        value |= payload << shift
        if byte & 0x80 == 0 {
            guard index - start == 1 || payload != 0 else {
                throw MusubiV1Error.invalidValue(
                    "Musubi public-key multihash uses a noncanonical varint."
                )
            }
            return (value, index)
        }
        shift += 7
    }
    throw MusubiV1Error.invalidValue("Musubi public-key multihash is truncated.")
}

private func musubiSigningAlgorithmV1(_ code: UInt64) -> SigningAlgorithm? {
    switch code {
    case 0xed: return .ed25519
    case 0xe7: return .secp256k1
    case 0xea: return .blsNormal
    case 0xeb: return .blsSmall
    case 0xee: return .mlDsa
    case 0x1200: return .gost2012_256A
    case 0x1201: return .gost2012_256B
    case 0x1202: return .gost2012_256C
    case 0x1203: return .gost2012_512A
    case 0x1204: return .gost2012_512B
    case 0x1306: return .sm2
    default: return nil
    }
}

/// Canonical controller key and typed-signature bytes used by Musubi signed proofs.
public struct MusubiControllerApprovalV1: Codable, Hashable, Sendable, Comparable {
    public let publicKey: String
    public let signature: String
    let algorithm: SigningAlgorithm
    let publicKeyPayload: [UInt8]
    let signaturePayload: [UInt8]

    public init(publicKey: String, signature: String) throws {
        guard publicKey == publicKey.trimmingCharacters(in: .whitespacesAndNewlines),
              let encodedKey = Data(hexString: publicKey) else {
            throw MusubiV1Error.invalidValue("Musubi approval public key is not canonical hex.")
        }
        let keyBytes = [UInt8](encodedKey)
        let (code, codeEnd) = try musubiDecodeVarintV1(keyBytes, from: 0)
        let (length, payloadStart) = try musubiDecodeVarintV1(keyBytes, from: codeEnd)
        guard let algorithm = musubiSigningAlgorithmV1(code),
              length <= UInt64(Int.max),
              keyBytes.count - payloadStart == Int(length) else {
            throw MusubiV1Error.invalidValue("Musubi approval public key has an invalid multihash.")
        }
        let keyPayload = Data(keyBytes[payloadStart...])
        guard CanonicalNorito.publicKeyMultihash(
            algorithm: algorithm,
            payload: keyPayload
        ) == publicKey else {
            throw MusubiV1Error.invalidValue("Musubi approval public key is noncanonical.")
        }
        if algorithm == .ed25519, !Ed25519PublicKeyAdmission.isValidPublicKey(keyPayload) {
            throw MusubiV1Error.invalidValue("Musubi approval Ed25519 public key is invalid.")
        }

        guard signature == signature.uppercased(), !signature.hasPrefix("0X"),
              let signatureBytes = Data(hexString: signature),
              !signatureBytes.isEmpty, signatureBytes.count <= 16_384,
              signatureBytes.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi approval signature is not canonical hex.")
        }
        if algorithm == .ed25519,
           !Ed25519SignatureAdmission.isValidSignature(signatureBytes) {
            throw MusubiV1Error.invalidValue("Musubi approval Ed25519 signature is invalid.")
        }
        self.publicKey = publicKey
        self.signature = signature
        self.algorithm = algorithm
        self.publicKeyPayload = [UInt8](keyPayload)
        self.signaturePayload = [UInt8](signatureBytes)
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["public_key", "signature"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            publicKey: container.decode(String.self, forKey: .publicKey),
            signature: container.decode(String.self, forKey: .signature)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(publicKey, forKey: .publicKey)
        try container.encode(signature, forKey: .signature)
    }

    public static func < (lhs: Self, rhs: Self) -> Bool {
        if lhs.algorithm.noritoDiscriminant != rhs.algorithm.noritoDiscriminant {
            return lhs.algorithm.noritoDiscriminant < rhs.algorithm.noritoDiscriminant
        }
        return lhs.publicKeyPayload.lexicographicallyPrecedes(rhs.publicKeyPayload)
    }

    private enum CodingKeys: String, CodingKey {
        case publicKey = "public_key"
        case signature
    }
}

/// Governed identity of a provider-ingest completion signer policy.
public struct MusubiProviderIngestCompletionSignerPolicyV1: Codable, Hashable, Sendable {
    public let policyID: [UInt8]
    public let revision: UInt64
    public let predecessorDigest: [UInt8]?
    public let policyDigest: [UInt8]

    public init(
        policyID: [UInt8],
        revision: UInt64,
        predecessorDigest: [UInt8]?,
        policyDigest: [UInt8]
    ) throws {
        let predecessorIsCanonical = revision == 1
            ? predecessorDigest == nil
            : predecessorDigest?.count == 32
                && predecessorDigest?.contains(where: { $0 != 0 }) == true
        guard policyID.count == 32, policyID.contains(where: { $0 != 0 }),
              revision > 0, predecessorIsCanonical,
              policyDigest.count == 32, policyDigest.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi provider completion signer policy is invalid."
            )
        }
        self.policyID = policyID
        self.revision = revision
        self.predecessorDigest = predecessorDigest
        self.policyDigest = policyDigest
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            ["policy_id", "revision", "predecessor_digest", "policy_digest"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            policyID: container.decode([UInt8].self, forKey: .policyID),
            revision: container.decode(UInt64.self, forKey: .revision),
            predecessorDigest: container.decodeIfPresent(
                [UInt8].self,
                forKey: .predecessorDigest
            ),
            policyDigest: container.decode([UInt8].self, forKey: .policyDigest)
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(policyID, forKey: .policyID)
        try container.encode(revision, forKey: .revision)
        if let predecessorDigest {
            try container.encode(predecessorDigest, forKey: .predecessorDigest)
        } else {
            try container.encodeNil(forKey: .predecessorDigest)
        }
        try container.encode(policyDigest, forKey: .policyDigest)
    }

    private enum CodingKeys: String, CodingKey {
        case policyID = "policy_id"
        case revision
        case predecessorDigest = "predecessor_digest"
        case policyDigest = "policy_digest"
    }
}

/// Chain-authoritative provider owner and governed completion signer policy.
public struct MusubiProviderIngestCompletionAuthorityV1: Codable, Hashable, Sendable {
    public let providerOwner: String
    public let signerPolicy: MusubiProviderIngestCompletionSignerPolicyV1

    public init(
        providerOwner: String,
        signerPolicy: MusubiProviderIngestCompletionSignerPolicyV1
    ) throws {
        _ = try CanonicalNorito.encodeCompactAccountId(providerOwner)
        self.providerOwner = providerOwner
        self.signerPolicy = signerPolicy
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["provider_owner", "signer_policy"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            providerOwner: container.decode(String.self, forKey: .providerOwner),
            signerPolicy: container.decode(
                MusubiProviderIngestCompletionSignerPolicyV1.self,
                forKey: .signerPolicy
            )
        )
    }

    private enum CodingKeys: String, CodingKey {
        case providerOwner = "provider_owner"
        case signerPolicy = "signer_policy"
    }
}

/// Finalized committed-chain anchor carried by one provider completion.
public struct MusubiProviderIngestFinalizedAnchorV1: Codable, Hashable, Sendable {
    public let height: UInt64
    public let blockHash: [UInt8]

    public init(height: UInt64, blockHash: [UInt8]) throws {
        guard height > 0, blockHash.count == 32,
              blockHash.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi provider finalized anchor is invalid.")
        }
        self.height = height
        self.blockHash = blockHash
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["height", "block_hash"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            height: container.decode(UInt64.self, forKey: .height),
            blockHash: container.decode([UInt8].self, forKey: .blockHash)
        )
    }

    private enum CodingKeys: String, CodingKey {
        case height
        case blockHash = "block_hash"
    }
}

/// Exact parsed-bundle and finalized-replication completion binding.
public struct MusubiProviderBundleVerificationBindingV1: Codable, Hashable, Sendable {
    public let networkId: NetworkId
    public let providerID: MusubiDigest32V1
    public let completedBy: String
    public let completionAuthority: MusubiProviderIngestCompletionAuthorityV1
    public let replicationOrder: MusubiDigest32V1
    public let assignmentRevision: UInt64
    public let completionEpoch: UInt64
    public let finalizedAnchor: MusubiProviderIngestFinalizedAnchorV1
    public let archiveID: MusubiDigest32V1
    public let bundleDigest: MusubiDigest32V1
    public let descriptorDigest: MusubiDigest32V1
    public let semanticReleaseManifestDigest: MusubiDigest32V1
    public let verificationLockDigest: MusubiDigest32V1
    public let sourceTreeDigest: MusubiDigest32V1

    public init(
        networkId: NetworkId,
        providerID: MusubiDigest32V1,
        completedBy: String,
        completionAuthority: MusubiProviderIngestCompletionAuthorityV1,
        replicationOrder: MusubiDigest32V1,
        assignmentRevision: UInt64,
        completionEpoch: UInt64,
        finalizedAnchor: MusubiProviderIngestFinalizedAnchorV1,
        archiveID: MusubiDigest32V1,
        bundleDigest: MusubiDigest32V1,
        descriptorDigest: MusubiDigest32V1,
        semanticReleaseManifestDigest: MusubiDigest32V1,
        verificationLockDigest: MusubiDigest32V1,
        sourceTreeDigest: MusubiDigest32V1
    ) throws {
        let completedByPayload = try CanonicalNorito.encodeCompactAccountId(completedBy)
        let providerOwnerPayload = try CanonicalNorito.encodeCompactAccountId(
            completionAuthority.providerOwner
        )
        guard completedByPayload == providerOwnerPayload,
              assignmentRevision > 0, completionEpoch > 0,
              [
                  providerID, replicationOrder, archiveID, bundleDigest,
                  descriptorDigest, semanticReleaseManifestDigest,
                  verificationLockDigest, sourceTreeDigest,
              ].allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi provider bundle verification binding is invalid."
            )
        }
        self.networkId = networkId
        self.providerID = providerID
        self.completedBy = completedBy
        self.completionAuthority = completionAuthority
        self.replicationOrder = replicationOrder
        self.assignmentRevision = assignmentRevision
        self.completionEpoch = completionEpoch
        self.finalizedAnchor = finalizedAnchor
        self.archiveID = archiveID
        self.bundleDigest = bundleDigest
        self.descriptorDigest = descriptorDigest
        self.semanticReleaseManifestDigest = semanticReleaseManifestDigest
        self.verificationLockDigest = verificationLockDigest
        self.sourceTreeDigest = sourceTreeDigest
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            [
                "network_id", "provider_id", "completed_by",
                "completion_authority", "replication_order", "assignment_revision",
                "completion_epoch", "finalized_anchor", "archive_id", "bundle_digest",
                "descriptor_digest", "semantic_release_manifest_digest",
                "verification_lock_digest", "source_tree_digest",
            ]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            networkId: container.decode(NetworkId.self, forKey: .networkId),
            providerID: MusubiDigest32V1(
                bytes: musubiProviderIDBytesV1(
                    container.decode(MusubiProviderIDJSONV1.self, forKey: .providerID).value
                )
            ),
            completedBy: container.decode(String.self, forKey: .completedBy),
            completionAuthority: container.decode(
                MusubiProviderIngestCompletionAuthorityV1.self,
                forKey: .completionAuthority
            ),
            replicationOrder: container.decode(
                MusubiDigest32V1.self,
                forKey: .replicationOrder
            ),
            assignmentRevision: container.decode(UInt64.self, forKey: .assignmentRevision),
            completionEpoch: container.decode(UInt64.self, forKey: .completionEpoch),
            finalizedAnchor: container.decode(
                MusubiProviderIngestFinalizedAnchorV1.self,
                forKey: .finalizedAnchor
            ),
            archiveID: container.decode(MusubiDigest32V1.self, forKey: .archiveID),
            bundleDigest: container.decode(MusubiDigest32V1.self, forKey: .bundleDigest),
            descriptorDigest: container.decode(
                MusubiDigest32V1.self,
                forKey: .descriptorDigest
            ),
            semanticReleaseManifestDigest: container.decode(
                MusubiDigest32V1.self,
                forKey: .semanticReleaseManifestDigest
            ),
            verificationLockDigest: container.decode(
                MusubiDigest32V1.self,
                forKey: .verificationLockDigest
            ),
            sourceTreeDigest: container.decode(
                MusubiDigest32V1.self,
                forKey: .sourceTreeDigest
            )
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(networkId, forKey: .networkId)
        try container.encode(
            MusubiProviderIDJSONV1(Data(providerID.bytes).hexEncodedString().uppercased()),
            forKey: .providerID
        )
        try container.encode(completedBy, forKey: .completedBy)
        try container.encode(completionAuthority, forKey: .completionAuthority)
        try container.encode(replicationOrder, forKey: .replicationOrder)
        try container.encode(assignmentRevision, forKey: .assignmentRevision)
        try container.encode(completionEpoch, forKey: .completionEpoch)
        try container.encode(finalizedAnchor, forKey: .finalizedAnchor)
        try container.encode(archiveID, forKey: .archiveID)
        try container.encode(bundleDigest, forKey: .bundleDigest)
        try container.encode(descriptorDigest, forKey: .descriptorDigest)
        try container.encode(
            semanticReleaseManifestDigest,
            forKey: .semanticReleaseManifestDigest
        )
        try container.encode(verificationLockDigest, forKey: .verificationLockDigest)
        try container.encode(sourceTreeDigest, forKey: .sourceTreeDigest)
    }

    private enum CodingKeys: String, CodingKey {
        case networkId = "network_id"
        case providerID = "provider_id"
        case completedBy = "completed_by"
        case completionAuthority = "completion_authority"
        case replicationOrder = "replication_order"
        case assignmentRevision = "assignment_revision"
        case completionEpoch = "completion_epoch"
        case finalizedAnchor = "finalized_anchor"
        case archiveID = "archive_id"
        case bundleDigest = "bundle_digest"
        case descriptorDigest = "descriptor_digest"
        case semanticReleaseManifestDigest = "semantic_release_manifest_digest"
        case verificationLockDigest = "verification_lock_digest"
        case sourceTreeDigest = "source_tree_digest"
    }
}

/// Version-one provider parsed-bundle statement.
public struct MusubiProviderBundleVerificationPayloadV1: Codable, Hashable, Sendable {
    public let version: UInt8
    public let binding: MusubiProviderBundleVerificationBindingV1

    public init(
        version: UInt8 = 1,
        binding: MusubiProviderBundleVerificationBindingV1
    ) throws {
        guard version == 1 else {
            throw MusubiV1Error.unsupportedVersion(
                "Musubi provider bundle verification payload must be V1."
            )
        }
        self.version = version
        self.binding = binding
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["version", "binding"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            version: container.decode(UInt8.self, forKey: .version),
            binding: container.decode(
                MusubiProviderBundleVerificationBindingV1.self,
                forKey: .binding
            )
        )
    }

    private enum CodingKeys: String, CodingKey { case version, binding }
}

public typealias MusubiProviderBundleVerificationApprovalV1 = MusubiControllerApprovalV1

/// Signed provider proof that a canonical bundle was parsed before completion.
public struct MusubiProviderBundleVerificationAttestationV1: Codable, Hashable, Sendable {
    public let payload: MusubiProviderBundleVerificationPayloadV1
    public let approvals: [MusubiProviderBundleVerificationApprovalV1]

    public init(
        payload: MusubiProviderBundleVerificationPayloadV1,
        approvals: [MusubiProviderBundleVerificationApprovalV1]
    ) throws {
        guard !approvals.isEmpty, approvals.count <= 64,
              zip(approvals, approvals.dropFirst()).allSatisfy({ $0.0 < $0.1 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi provider bundle approvals must be bounded, sorted, and unique."
            )
        }
        self.payload = payload
        self.approvals = approvals
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["payload", "approvals"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            payload: container.decode(
                MusubiProviderBundleVerificationPayloadV1.self,
                forKey: .payload
            ),
            approvals: container.decode(
                [MusubiProviderBundleVerificationApprovalV1].self,
                forKey: .approvals
            )
        )
    }

    private enum CodingKeys: String, CodingKey { case payload, approvals }
}

/// Immutable archive/order/provider identity of one registered provider proof.
public struct MusubiProviderBundleAttestationKeyV1: Codable, Hashable, Sendable {
    public let archiveID: MusubiDigest32V1
    public let replicationOrder: MusubiDigest32V1
    public let providerID: MusubiDigest32V1

    public init(
        archiveID: MusubiDigest32V1,
        replicationOrder: MusubiDigest32V1,
        providerID: MusubiDigest32V1
    ) throws {
        guard [archiveID, replicationOrder, providerID].allSatisfy({
            $0.bytes.contains(where: { $0 != 0 })
        }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi provider bundle attestation key must be non-zero."
            )
        }
        self.archiveID = archiveID
        self.replicationOrder = replicationOrder
        self.providerID = providerID
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(decoder, ["archive_id", "replication_order", "provider_id"])
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            archiveID: container.decode(MusubiDigest32V1.self, forKey: .archiveID),
            replicationOrder: container.decode(
                MusubiDigest32V1.self,
                forKey: .replicationOrder
            ),
            providerID: MusubiDigest32V1(
                bytes: musubiProviderIDBytesV1(
                    container.decode(MusubiProviderIDJSONV1.self, forKey: .providerID).value
                )
            )
        )
    }

    public func encode(to encoder: Encoder) throws {
        var container = encoder.container(keyedBy: CodingKeys.self)
        try container.encode(archiveID, forKey: .archiveID)
        try container.encode(replicationOrder, forKey: .replicationOrder)
        try container.encode(
            MusubiProviderIDJSONV1(Data(providerID.bytes).hexEncodedString().uppercased()),
            forKey: .providerID
        )
    }

    private enum CodingKeys: String, CodingKey {
        case archiveID = "archive_id"
        case replicationOrder = "replication_order"
        case providerID = "provider_id"
    }
}

/// Complete immutable provider proof returned by the exact audit query.
public struct MusubiProviderBundleAttestationRecordV1: Codable, Hashable, Sendable {
    public let key: MusubiProviderBundleAttestationKeyV1
    public let attestationDigest: MusubiProviderBundleAttestationDigestV1
    public let attestation: MusubiProviderBundleVerificationAttestationV1
    public let registeredBy: String
    public let registeredAtHeight: UInt64

    public init(
        key: MusubiProviderBundleAttestationKeyV1,
        attestationDigest: MusubiProviderBundleAttestationDigestV1,
        attestation: MusubiProviderBundleVerificationAttestationV1,
        registeredBy: String,
        registeredAtHeight: UInt64
    ) throws {
        let binding = attestation.payload.binding
        guard key.archiveID == binding.archiveID,
              key.replicationOrder == binding.replicationOrder,
              key.providerID == binding.providerID,
              attestationDigest == (try musubiProviderBundleAttestationDigestV1(attestation)),
              registeredAtHeight > 0 else {
            throw MusubiV1Error.invalidValue(
                "Musubi provider attestation record is inconsistent with its signed binding."
            )
        }
        _ = try CanonicalNorito.encodeCompactAccountId(registeredBy)
        self.key = key
        self.attestationDigest = attestationDigest
        self.attestation = attestation
        self.registeredBy = registeredBy
        self.registeredAtHeight = registeredAtHeight
    }

    public init(from decoder: Decoder) throws {
        try musubiRequireExactKeys(
            decoder,
            ["key", "attestation_digest", "attestation", "registered_by", "registered_at_height"]
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        try self.init(
            key: container.decode(MusubiProviderBundleAttestationKeyV1.self, forKey: .key),
            attestationDigest: container.decode(
                MusubiProviderBundleAttestationDigestV1.self,
                forKey: .attestationDigest
            ),
            attestation: container.decode(
                MusubiProviderBundleVerificationAttestationV1.self,
                forKey: .attestation
            ),
            registeredBy: container.decode(String.self, forKey: .registeredBy),
            registeredAtHeight: container.decode(UInt64.self, forKey: .registeredAtHeight)
        )
    }

    /// Require this audit record to carry the requested immutable proof identity.
    public func requireMatches(_ request: MusubiProviderBundleAttestationKeyV1) throws {
        guard key == request else {
            throw MusubiV1Error.invalidValue(
                "Musubi provider attestation response does not match the exact request."
            )
        }
    }

    private enum CodingKeys: String, CodingKey {
        case key
        case attestationDigest = "attestation_digest"
        case attestation
        case registeredBy = "registered_by"
        case registeredAtHeight = "registered_at_height"
    }
}

/// Exact IVM ABI V1 binding embedded in a release or verification node.
public struct MusubiAbiBindingV1: Hashable, Sendable {
    public let abiVersion: UInt16
    public let abiHash: [UInt8]

    public init(abiVersion: UInt16 = 1, abiHash: [UInt8]) throws {
        guard abiVersion == 1, abiHash.count == 32,
              abiHash.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi ABI binding is invalid.")
        }
        self.abiVersion = abiVersion
        self.abiHash = abiHash
    }
}

/// Normal dependency requirement in a published release manifest.
public struct MusubiDependencyReqV1: Hashable, Sendable {
    public let alias: String
    public let package: MusubiPackageIdV1
    public let requirement: MusubiVersionReqV1

    public init(
        alias: String,
        package: MusubiPackageIdV1,
        requirement: MusubiVersionReqV1
    ) throws {
        try musubiRequireName(alias, field: "Musubi dependency alias")
        try musubiValidateVersionRequirementV1(requirement)
        self.alias = alias
        self.package = package
        self.requirement = requirement
    }
}

/// Dependency kind recorded in an exact verification graph.
public enum MusubiDependencyKindV1: UInt32, Hashable, Sendable {
    case normal = 0
    case development = 1
}

/// Parent-local exact edge in a publication proof.
public struct MusubiExactDependencyEdgeV1: Hashable, Sendable {
    public let alias: String
    public let kind: MusubiDependencyKindV1
    public let package: MusubiPackageIdV1
    public let requirement: MusubiVersionReqV1
    public let selected: MusubiReleaseIdV1

    public init(
        alias: String,
        kind: MusubiDependencyKindV1,
        package: MusubiPackageIdV1,
        requirement: MusubiVersionReqV1,
        selected: MusubiReleaseIdV1
    ) throws {
        try musubiRequireName(alias, field: "Musubi exact dependency alias")
        try musubiValidateVersionRequirementV1(requirement)
        guard selected.package == package,
              musubiRequirementMatchesV1(requirement, version: selected.version) else {
            throw MusubiV1Error.invalidValue(
                "Musubi exact dependency does not satisfy its package requirement."
            )
        }
        self.alias = alias
        self.kind = kind
        self.package = package
        self.requirement = requirement
        self.selected = selected
    }
}

/// Exact immutable dependency node used in publication verification.
public struct MusubiVerificationNodeV1: Hashable, Sendable {
    public let release: MusubiReleaseIdV1
    public let releaseDigest: MusubiDigest32V1
    public let archiveID: MusubiDigest32V1
    public let sourceDigest: MusubiDigest32V1
    public let interfaceDigest: MusubiDigest32V1
    public let abi: MusubiAbiBindingV1
    public let dependencies: [MusubiExactDependencyEdgeV1]

    public init(
        release: MusubiReleaseIdV1,
        releaseDigest: MusubiDigest32V1,
        archiveID: MusubiDigest32V1,
        sourceDigest: MusubiDigest32V1,
        interfaceDigest: MusubiDigest32V1,
        abi: MusubiAbiBindingV1,
        dependencies: [MusubiExactDependencyEdgeV1]
    ) throws {
        guard dependencies.count <= 256,
              zip(dependencies, dependencies.dropFirst()).allSatisfy({
                  musubiExactDependencyLessV1($0.0, $0.1)
              }),
              [releaseDigest, archiveID, sourceDigest, interfaceDigest]
                  .allSatisfy({ $0.bytes.contains(where: { $0 != 0 }) }) else {
            throw MusubiV1Error.invalidValue("Musubi verification node is invalid.")
        }
        try musubiRequireUniqueParentLocalAliasesV1(
            dependencies.map(\.alias),
            field: "Musubi verification-node dependencies"
        )
        self.release = release
        self.releaseDigest = releaseDigest
        self.archiveID = archiveID
        self.sourceDigest = sourceDigest
        self.interfaceDigest = interfaceDigest
        self.abi = abi
        self.dependencies = dependencies
    }
}

/// Normalized, secret-free exact verification lock packaged with a release.
public struct MusubiVerificationLockV1: Hashable, Sendable {
    public let schema: String
    public let version: UInt8
    public let root: MusubiReleaseIdV1
    public let rootDependencies: [MusubiExactDependencyEdgeV1]
    public let nodes: [MusubiVerificationNodeV1]

    public init(
        schema: String = "musubi-verification-lock",
        version: UInt8 = 1,
        root: MusubiReleaseIdV1,
        rootDependencies: [MusubiExactDependencyEdgeV1],
        nodes: [MusubiVerificationNodeV1]
    ) throws {
        guard schema == "musubi-verification-lock", version == 1,
              rootDependencies.count <= 256, nodes.count <= 1_024,
              rootDependencies.allSatisfy({ $0.kind == .normal }),
              zip(rootDependencies, rootDependencies.dropFirst()).allSatisfy({
                  musubiExactDependencyLessV1($0.0, $0.1)
              }),
              zip(nodes, nodes.dropFirst()).allSatisfy({
                  musubiReleaseLessV1($0.0.release, $0.1.release)
              }) else {
            throw MusubiV1Error.invalidValue("Musubi verification lock is invalid.")
        }
        try musubiRequireUniqueParentLocalAliasesV1(
            rootDependencies.map(\.alias),
            field: "Musubi root dependencies"
        )
        let byRelease = Dictionary(grouping: nodes, by: \.release)
        guard byRelease.count == nodes.count,
              rootDependencies.allSatisfy({ byRelease[$0.selected]?.count == 1 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi verification lock has duplicate or missing nodes."
            )
        }
        var complete = Set<MusubiReleaseIdV1>()
        var visiting = Set<MusubiReleaseIdV1>()
        func visit(_ release: MusubiReleaseIdV1, depth: Int) throws {
            guard depth <= 64 else {
                throw MusubiV1Error.invalidValue("Musubi verification graph exceeds depth 64.")
            }
            if complete.contains(release) { return }
            guard visiting.insert(release).inserted,
                  let node = byRelease[release]?.first else {
                throw MusubiV1Error.invalidValue(
                    "Musubi verification graph contains a cycle or missing node."
                )
            }
            for edge in node.dependencies where edge.kind == .normal {
                try visit(edge.selected, depth: depth + 1)
            }
            visiting.remove(release)
            complete.insert(release)
        }
        for node in nodes { try visit(node.release, depth: 1) }
        self.schema = schema
        self.version = version
        self.root = root
        self.rootDependencies = rootDependencies
        self.nodes = nodes
    }
}

/// Bounded exact resolution proof supplied with publication.
public struct MusubiResolutionProofV1: Hashable, Sendable {
    public let snapshot: MusubiRegistrySnapshotV1
    public let lock: MusubiVerificationLockV1

    public init(snapshot: MusubiRegistrySnapshotV1, lock: MusubiVerificationLockV1) {
        self.snapshot = snapshot
        self.lock = lock
    }
}

/// Immutable release metadata and mutable package metadata projection.
public struct MusubiReleaseMetadataV1: Hashable, Sendable {
    public let description: String?
    public let readme: String?
    public let license: String?
    public let repository: String?
    public let keywords: [String]

    public init(
        description: String? = nil,
        readme: String? = nil,
        license: String? = nil,
        repository: String? = nil,
        keywords: [String] = []
    ) throws {
        if let description {
            try musubiRequireExactText(description, field: "Musubi description")
            guard description.utf8.count <= 4_096 else {
                throw MusubiV1Error.invalidValue("Musubi description exceeds 4096 bytes.")
            }
        }
        for (field, value) in [
            ("readme", readme), ("license", license), ("repository", repository),
        ] {
            if let value {
                try musubiRequireExactText(value, field: "Musubi \(field)")
                guard value.utf8.count <= 2_048 else {
                    throw MusubiV1Error.invalidValue("Musubi \(field) exceeds 2048 bytes.")
                }
            }
        }
        guard keywords.count <= 32 else {
            throw MusubiV1Error.invalidValue("Musubi metadata exceeds 32 keywords.")
        }
        for keyword in keywords {
            try musubiRequireASCIILowerKebab(
                keyword, maximum: 64, field: "Musubi keyword"
            )
        }
        guard zip(keywords, keywords.dropFirst()).allSatisfy({ $0.0 < $0.1 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi keywords must be strictly sorted and unique."
            )
        }
        self.description = description
        self.readme = readme
        self.license = license
        self.repository = repository
        self.keywords = keywords
    }
}

/// First-release Kotodama edition.
public enum MusubiKotodamaEditionV1: UInt32, Hashable, Sendable {
    case v1 = 0
}

/// Immutable registry release manifest.
public struct MusubiReleaseManifestV1: Hashable, Sendable {
    public let release: MusubiReleaseIdV1
    public let edition: MusubiKotodamaEditionV1
    public let abi: MusubiAbiBindingV1
    public let dependencies: [MusubiDependencyReqV1]
    public let exports: [String]
    public let interfaceDigest: MusubiDigest32V1
    public let metadata: MusubiReleaseMetadataV1
    public let archiveID: MusubiDigest32V1
    public let verificationLockDigest: MusubiDigest32V1

    public init(
        release: MusubiReleaseIdV1,
        edition: MusubiKotodamaEditionV1 = .v1,
        abi: MusubiAbiBindingV1,
        dependencies: [MusubiDependencyReqV1],
        exports: [String],
        interfaceDigest: MusubiDigest32V1,
        metadata: MusubiReleaseMetadataV1,
        archiveID: MusubiDigest32V1,
        verificationLockDigest: MusubiDigest32V1
    ) throws {
        guard dependencies.count <= 256, exports.count <= 1_024,
              zip(dependencies, dependencies.dropFirst()).allSatisfy({
                  musubiDependencyReqLessV1($0.0, $0.1)
              }),
              interfaceDigest.bytes.contains(where: { $0 != 0 }),
              archiveID.bytes.contains(where: { $0 != 0 }),
              verificationLockDigest.bytes.contains(where: { $0 != 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi release manifest is invalid.")
        }
        try musubiRequireUniqueParentLocalAliasesV1(
            dependencies.map(\.alias),
            field: "Musubi manifest dependencies"
        )
        for dependency in dependencies {
            guard dependency.package != release.package else {
                throw MusubiV1Error.invalidValue(
                    "Musubi release cannot depend on its own package."
                )
            }
        }
        for export in exports { try musubiRequireName(export, field: "Musubi export") }
        guard zip(exports, exports.dropFirst()).allSatisfy({
            musubiCompareStringV1($0.0, $0.1) < 0
        }) else {
            throw MusubiV1Error.invalidValue("Musubi exports must be sorted and unique.")
        }
        self.release = release
        self.edition = edition
        self.abi = abi
        self.dependencies = dependencies
        self.exports = exports
        self.interfaceDigest = interfaceDigest
        self.metadata = metadata
        self.archiveID = archiveID
        self.verificationLockDigest = verificationLockDigest
    }
}

/// Publication payload binding a release to its exact dependency proof.
public struct MusubiPublicationV1: Hashable, Sendable {
    public let manifest: MusubiReleaseManifestV1
    public let resolution: MusubiResolutionProofV1

    public init(manifest: MusubiReleaseManifestV1, resolution: MusubiResolutionProofV1) throws {
        guard resolution.lock.root == manifest.release,
              manifest.dependencies.count == resolution.lock.rootDependencies.count else {
            throw MusubiV1Error.invalidValue(
                "Musubi publication proof does not bind the release manifest."
            )
        }
        for (manifestDependency, exact) in zip(
            manifest.dependencies, resolution.lock.rootDependencies
        ) {
            guard exact.kind == .normal,
                  exact.alias == manifestDependency.alias,
                  exact.package == manifestDependency.package,
                  exact.requirement == manifestDependency.requirement else {
                throw MusubiV1Error.invalidValue(
                    "Musubi publication direct dependency proof is inconsistent."
                )
            }
        }
        self.manifest = manifest
        self.resolution = resolution
    }
}

/// Canonical generation-bound namespace delegation payload.
public struct MusubiNamespaceDelegationPayloadV1: Hashable, Sendable {
    public let version: UInt8
    public let namespaceBinding: MusubiDigest32V1
    public let ownerGeneration: UInt64
    public let owner: String
    public let delegate: String
    public let expiresAtHeight: UInt64

    public init(
        version: UInt8 = 1,
        namespaceBinding: MusubiDigest32V1,
        ownerGeneration: UInt64,
        owner: String,
        delegate: String,
        expiresAtHeight: UInt64
    ) throws {
        _ = try CanonicalNorito.encodeCompactAccountId(owner)
        _ = try CanonicalNorito.encodeCompactAccountId(delegate)
        guard version == 1, namespaceBinding.bytes.contains(where: { $0 != 0 }),
              ownerGeneration > 0, expiresAtHeight > 0 else {
            throw MusubiV1Error.invalidValue("Musubi namespace delegation payload is invalid.")
        }
        self.version = version
        self.namespaceBinding = namespaceBinding
        self.ownerGeneration = ownerGeneration
        self.owner = owner
        self.delegate = delegate
        self.expiresAtHeight = expiresAtHeight
    }
}

public typealias MusubiNamespaceDelegationApprovalV1 = MusubiControllerApprovalV1

/// Generation-bound authority to claim an absent package in one namespace.
public struct MusubiNamespaceDelegationV1: Hashable, Sendable {
    public let payload: MusubiNamespaceDelegationPayloadV1
    public let approvals: [MusubiNamespaceDelegationApprovalV1]

    public init(
        payload: MusubiNamespaceDelegationPayloadV1,
        approvals: [MusubiNamespaceDelegationApprovalV1]
    ) throws {
        guard !approvals.isEmpty, approvals.count <= 64,
              zip(approvals, approvals.dropFirst()).allSatisfy({ $0.0 < $0.1 }) else {
            throw MusubiV1Error.invalidValue(
                "Musubi namespace delegation approvals must be sorted and unique."
            )
        }
        self.payload = payload
        self.approvals = approvals
    }
}

/// Admission mode for new Musubi archives, releases, and aliases.
public enum MusubiRegistryAdmissionModeV1: UInt32, Hashable, Sendable {
    case closed = 0
    case allowlisted = 1
    case open = 2
}

/// Prospective permanent-alias price policy denominated in whole XOR.
public struct MusubiAliasPricingPolicyV1: Hashable, Sendable {
    public let revision: UInt64
    public let length1Xor: UInt64
    public let length2Xor: UInt64
    public let length3Xor: UInt64
    public let length4Xor: UInt64
    public let length5To32Xor: UInt64

    public init(
        revision: UInt64,
        length1Xor: UInt64,
        length2Xor: UInt64,
        length3Xor: UInt64,
        length4Xor: UInt64,
        length5To32Xor: UInt64
    ) throws {
        guard revision > 0,
              [length1Xor, length2Xor, length3Xor, length4Xor, length5To32Xor]
                  .allSatisfy({ $0 > 0 }) else {
            throw MusubiV1Error.invalidValue("Musubi alias pricing policy is invalid.")
        }
        self.revision = revision
        self.length1Xor = length1Xor
        self.length2Xor = length2Xor
        self.length3Xor = length3Xor
        self.length4Xor = length4Xor
        self.length5To32Xor = length5To32Xor
    }
}
