import Foundation

/// Fail-closed validation for authenticated finalized privacy-state queries.
public enum PrivacyFinalizedStateQueryErrorV1: Error, LocalizedError, Equatable, Sendable {
    case invalidFixed32(field: String)
    case unsupportedProofManagedProtocol
    case invalidProjection(field: String)

    public var errorDescription: String? {
        switch self {
        case let .invalidFixed32(field):
            return "Finalized privacy-state \(field) must contain exactly 32 non-zero bytes."
        case .unsupportedProofManagedProtocol:
            return "Proof-managed pool state supports only FCMP++, private IVM, or PQ-MASP."
        case let .invalidProjection(field):
            return "Authenticated finalized privacy-state projection has invalid \(field)."
        }
    }
}

enum PrivacyFinalizedStateQueryIdV1: UInt32, Sendable {
    case zkAceReplayNullifier = 97
    case proofManagedPoolState = 98
    case orchardPoolState = 99
    case orchardNullifier = 100
    case anonymousPgcPoolState = 101
    case zkAmsAdmission = 102
    case zkAmsProvision = 103
    case zkX509CertificateNullifier = 104
}

struct PrivacyAuthenticatedStateQueryPreparationV1: Sendable {
    let archive: Data
    let signingDigest: Data
}

protocol PrivacyFinalizedStateRequestV1 {
    static var queryId: PrivacyFinalizedStateQueryIdV1 { get }
    var protocolIndex: UInt32 { get }
    var requestBinding: Data { get }
}

private func privacyFinalizedFixed32(_ value: Data, field: String) throws -> Data {
    guard value.count == 32, value.contains(where: { $0 != 0 }) else {
        throw PrivacyFinalizedStateQueryErrorV1.invalidFixed32(field: field)
    }
    return Data(value)
}

private func privacyFinalizedBinding(_ chunks: [Data]) -> Data {
    var binding = Data()
    binding.reserveCapacity(chunks.count * 32)
    for chunk in chunks { binding.append(chunk) }
    return binding
}

public struct PrivacyZkAceReplayNullifierRequestV1: Equatable, Sendable,
    PrivacyFinalizedStateRequestV1
{
    static let queryId = PrivacyFinalizedStateQueryIdV1.zkAceReplayNullifier
    let protocolIndex: UInt32 = 0
    public let policyId: Data
    public let replayNullifier: Data
    var requestBinding: Data { privacyFinalizedBinding([policyId, replayNullifier]) }

    public init(policyId: Data, replayNullifier: Data) throws {
        self.policyId = try privacyFinalizedFixed32(policyId, field: "policy id")
        self.replayNullifier = try privacyFinalizedFixed32(
            replayNullifier,
            field: "replay nullifier"
        )
    }
}

public struct PrivacyProofManagedPoolStateRequestV1: Equatable, Sendable,
    PrivacyFinalizedStateRequestV1
{
    static let queryId = PrivacyFinalizedStateQueryIdV1.proofManagedPoolState
    public let protocolId: PrivacyProtocolIdV1
    public let poolId: Data
    var requestBinding: Data { poolId }
    var protocolIndex: UInt32 {
        switch protocolId {
        case .moneroFcmpPlusPlusV1: return 0
        case .irohaIvmPrivateNoteStarkV1: return 1
        case .pqMaspStarkV0: return 2
        default: preconditionFailure("validated proof-managed protocol escaped its closed union")
        }
    }

    public init(protocolId: PrivacyProtocolIdV1, poolId: Data) throws {
        guard [.moneroFcmpPlusPlusV1, .irohaIvmPrivateNoteStarkV1, .pqMaspStarkV0]
            .contains(protocolId) else {
            throw PrivacyFinalizedStateQueryErrorV1.unsupportedProofManagedProtocol
        }
        self.protocolId = protocolId
        self.poolId = try privacyFinalizedFixed32(poolId, field: "pool id")
    }
}

public struct PrivacyOrchardPoolStateRequestV1: Equatable, Sendable,
    PrivacyFinalizedStateRequestV1
{
    static let queryId = PrivacyFinalizedStateQueryIdV1.orchardPoolState
    let protocolIndex: UInt32 = 0
    public let poolId: Data
    var requestBinding: Data { poolId }

    public init(poolId: Data) throws {
        self.poolId = try privacyFinalizedFixed32(poolId, field: "pool id")
    }
}

public struct PrivacyOrchardNullifierRequestV1: Equatable, Sendable,
    PrivacyFinalizedStateRequestV1
{
    static let queryId = PrivacyFinalizedStateQueryIdV1.orchardNullifier
    let protocolIndex: UInt32 = 0
    public let poolId: Data
    public let nullifier: Data
    var requestBinding: Data { privacyFinalizedBinding([poolId, nullifier]) }

    public init(poolId: Data, nullifier: Data) throws {
        self.poolId = try privacyFinalizedFixed32(poolId, field: "pool id")
        self.nullifier = try privacyFinalizedFixed32(nullifier, field: "Orchard nullifier")
    }
}

public struct PrivacyAnonymousPgcPoolStateRequestV1: Equatable, Sendable,
    PrivacyFinalizedStateRequestV1
{
    static let queryId = PrivacyFinalizedStateQueryIdV1.anonymousPgcPoolState
    let protocolIndex: UInt32 = 0
    public let poolId: Data
    var requestBinding: Data { poolId }

    public init(poolId: Data) throws {
        self.poolId = try privacyFinalizedFixed32(poolId, field: "pool id")
    }
}

public struct PrivacyZkAmsAdmissionRequestV1: Equatable, Sendable,
    PrivacyFinalizedStateRequestV1
{
    static let queryId = PrivacyFinalizedStateQueryIdV1.zkAmsAdmission
    let protocolIndex: UInt32 = 0
    public let issuerId: Data
    public let registryId: Data
    public let policyId: Data
    public let phcHash: Data
    var requestBinding: Data {
        privacyFinalizedBinding([issuerId, registryId, policyId, phcHash])
    }

    public init(issuerId: Data, registryId: Data, policyId: Data, phcHash: Data) throws {
        self.issuerId = try privacyFinalizedFixed32(issuerId, field: "issuer id")
        self.registryId = try privacyFinalizedFixed32(registryId, field: "registry id")
        self.policyId = try privacyFinalizedFixed32(policyId, field: "policy id")
        self.phcHash = try privacyFinalizedFixed32(phcHash, field: "PHC hash")
    }
}

public struct PrivacyZkAmsProvisionRequestV1: Equatable, Sendable,
    PrivacyFinalizedStateRequestV1
{
    static let queryId = PrivacyFinalizedStateQueryIdV1.zkAmsProvision
    let protocolIndex: UInt32 = 0
    public let issuerId: Data
    public let registryId: Data
    public let policyId: Data
    public let keyImage: Data
    var requestBinding: Data {
        privacyFinalizedBinding([issuerId, registryId, policyId, keyImage])
    }

    public init(issuerId: Data, registryId: Data, policyId: Data, keyImage: Data) throws {
        self.issuerId = try privacyFinalizedFixed32(issuerId, field: "issuer id")
        self.registryId = try privacyFinalizedFixed32(registryId, field: "registry id")
        self.policyId = try privacyFinalizedFixed32(policyId, field: "policy id")
        self.keyImage = try privacyFinalizedFixed32(keyImage, field: "key image")
    }
}

public struct PrivacyZkX509CertificateNullifierRequestV1: Equatable, Sendable,
    PrivacyFinalizedStateRequestV1
{
    static let queryId = PrivacyFinalizedStateQueryIdV1.zkX509CertificateNullifier
    let protocolIndex: UInt32 = 0
    public let trustAnchorId: Data
    public let policyId: Data
    public let nullifier: Data
    var requestBinding: Data {
        privacyFinalizedBinding([trustAnchorId, policyId, nullifier])
    }

    public init(trustAnchorId: Data, policyId: Data, nullifier: Data) throws {
        self.trustAnchorId = try privacyFinalizedFixed32(
            trustAnchorId,
            field: "trust-anchor id"
        )
        self.policyId = try privacyFinalizedFixed32(policyId, field: "policy id")
        self.nullifier = try privacyFinalizedFixed32(
            nullifier,
            field: "certificate nullifier"
        )
    }
}

@propertyWrapper
public struct PrivacyFinalizedFixed32BytesV1: Decodable, Equatable, Sendable {
    public let wrappedValue: Data

    public init(from decoder: Decoder) throws {
        let bytes = try decoder.singleValueContainer().decode([UInt8].self)
        guard bytes.count == 32, bytes.contains(where: { $0 != 0 }) else {
            throw PrivacyFinalizedStateQueryErrorV1.invalidProjection(field: "fixed32 bytes")
        }
        wrappedValue = Data(bytes)
    }
}

@propertyWrapper
public struct PrivacyFinalizedCanonicalHashV1: Decodable, Equatable, Sendable {
    public let wrappedValue: Data

    public init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer().decode(String.self)
        guard let hash = try? NetworkId(literal: value) else {
            throw PrivacyFinalizedStateQueryErrorV1.invalidProjection(
                field: "canonical Iroha hash literal"
            )
        }
        wrappedValue = hash.bytes
    }
}

@propertyWrapper
public struct PrivacyFinalizedUInt64V1: Decodable, Equatable, Sendable {
    public let wrappedValue: UInt64

    public init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer().decode(String.self)
        guard !value.isEmpty,
              value.utf8.allSatisfy({ (0x30...0x39).contains($0) }),
              (value == "0" || !value.hasPrefix("0")),
              let integer = UInt64(value) else {
            throw PrivacyFinalizedStateQueryErrorV1.invalidProjection(field: "u64")
        }
        wrappedValue = integer
    }
}

@propertyWrapper
public struct PrivacyFinalizedUInt32V1: Decodable, Equatable, Sendable {
    public let wrappedValue: UInt32

    public init(from decoder: Decoder) throws {
        let value = try decoder.singleValueContainer().decode(String.self)
        guard !value.isEmpty,
              value.utf8.allSatisfy({ (0x30...0x39).contains($0) }),
              (value == "0" || !value.hasPrefix("0")),
              let integer = UInt32(value) else {
            throw PrivacyFinalizedStateQueryErrorV1.invalidProjection(field: "u32")
        }
        wrappedValue = integer
    }
}

@propertyWrapper
public struct PrivacyFinalizedProtocolIdV1: Decodable, Equatable, Sendable {
    public let wrappedValue: PrivacyProtocolIdV1

    private enum CodingKeys: String, CodingKey { case protocolId = "protocol"; case value }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let label = try container.decode(String.self, forKey: .protocolId)
        guard container.contains(.value), try container.decodeNil(forKey: .value),
              let protocolId = PrivacyProtocolIdV1(rawValue: label) else {
            throw PrivacyFinalizedStateQueryErrorV1.invalidProjection(field: "protocol id")
        }
        wrappedValue = protocolId
    }
}

public enum PrivacyFinalizedRootRoleV1: String, Decodable, Equatable, Sendable {
    case pgcAccountState = "PgcAccountState"
    case accountRegistry = "AccountRegistry"
    case revocation = "Revocation"
    case certificateAuthorityMembership = "CertificateAuthorityMembership"
    case noteCommitmentAnchor = "NoteCommitmentAnchor"
    case outputSet = "OutputSet"
    case programState = "ProgramState"
}

@propertyWrapper
public struct PrivacyFinalizedRootRoleProjectionV1: Decodable, Equatable, Sendable {
    public let wrappedValue: PrivacyFinalizedRootRoleV1

    private enum CodingKeys: String, CodingKey { case role; case value }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let label = try container.decode(String.self, forKey: .role)
        guard container.contains(.value), try container.decodeNil(forKey: .value),
              let role = PrivacyFinalizedRootRoleV1(rawValue: label) else {
            throw PrivacyFinalizedStateQueryErrorV1.invalidProjection(field: "root role")
        }
        wrappedValue = role
    }
}

public enum PrivacyFinalizedAssetBalanceScopeV1: Decodable, Equatable, Sendable {
    case global
    case dataspace(UInt64)

    private enum CodingKeys: String, CodingKey { case kind; case content }

    public init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .kind) {
        case "Global":
            if container.contains(.content) {
                guard try container.decodeNil(forKey: .content) else {
                    throw PrivacyFinalizedStateQueryErrorV1.invalidProjection(
                        field: "global balance scope"
                    )
                }
            }
            self = .global
        case "Dataspace":
            let text = try container.decode(String.self, forKey: .content)
            guard !text.isEmpty,
                  text.utf8.allSatisfy({ (0x30...0x39).contains($0) }),
                  (text == "0" || !text.hasPrefix("0")),
                  let id = UInt64(text), id > 0 else {
                throw PrivacyFinalizedStateQueryErrorV1.invalidProjection(
                    field: "dataspace balance scope"
                )
            }
            self = .dataspace(id)
        default:
            throw PrivacyFinalizedStateQueryErrorV1.invalidProjection(field: "balance scope")
        }
    }
}

public struct PrivacyProofManagedPoolTransitionViewV1: Decodable, Equatable, Sendable {
    @PrivacyFinalizedFixed32BytesV1 public var statementDigest: Data
    @PrivacyFinalizedUInt64V1 public var successorEpoch: UInt64
    @PrivacyFinalizedUInt64V1 public var admittedAtHeight: UInt64
    @PrivacyFinalizedUInt32V1 public var actionIndex: UInt32
    @PrivacyFinalizedUInt32V1 public var nullifierCount: UInt32
    @PrivacyFinalizedUInt32V1 public var outputCount: UInt32

    private enum CodingKeys: String, CodingKey {
        case statementDigest = "statement_digest"
        case successorEpoch = "successor_epoch"
        case admittedAtHeight = "admitted_at_height"
        case actionIndex = "action_index"
        case nullifierCount = "nullifier_count"
        case outputCount = "output_count"
    }
}

public struct PrivacyOrchardPoolTransitionViewV1: Decodable, Equatable, Sendable {
    @PrivacyFinalizedFixed32BytesV1 public var statementDigest: Data
    @PrivacyFinalizedUInt64V1 public var successorEpoch: UInt64
    @PrivacyFinalizedUInt64V1 public var parentEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var parentRoot: Data
    @PrivacyFinalizedUInt64V1 public var admittedAtHeight: UInt64
    @PrivacyFinalizedUInt32V1 public var actionIndex: UInt32

    private enum CodingKeys: String, CodingKey {
        case statementDigest = "statement_digest"
        case successorEpoch = "successor_epoch"
        case parentEpoch = "parent_epoch"
        case parentRoot = "parent_root"
        case admittedAtHeight = "admitted_at_height"
        case actionIndex = "action_index"
    }
}

public struct PrivacyAnonymousPgcPoolTransitionViewV1: Decodable, Equatable, Sendable {
    @PrivacyFinalizedFixed32BytesV1 public var statementDigest: Data
    @PrivacyFinalizedUInt64V1 public var successorEpoch: UInt64
    @PrivacyFinalizedUInt64V1 public var parentEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var parentRoot: Data
    @PrivacyFinalizedUInt64V1 public var admittedAtHeight: UInt64
    @PrivacyFinalizedUInt32V1 public var actionIndex: UInt32

    private enum CodingKeys: String, CodingKey {
        case statementDigest = "statement_digest"
        case successorEpoch = "successor_epoch"
        case parentEpoch = "parent_epoch"
        case parentRoot = "parent_root"
        case admittedAtHeight = "admitted_at_height"
        case actionIndex = "action_index"
    }
}

public struct PrivacyZkAceReplayNullifierProvenanceV1: Decodable, Equatable, Sendable {
    public let networkId: NetworkId
    @PrivacyFinalizedFixed32BytesV1 public var policyId: Data
    @PrivacyFinalizedFixed32BytesV1 public var replayNullifier: Data
    @PrivacyFinalizedFixed32BytesV1 public var policyRecordDigest: Data
    @PrivacyFinalizedFixed32BytesV1 public var statementDigest: Data
    @PrivacyFinalizedUInt64V1 public var admittedAtHeight: UInt64
    @PrivacyFinalizedUInt32V1 public var actionIndex: UInt32
    @PrivacyFinalizedUInt64V1 public var finalizedHeight: UInt64
    @PrivacyFinalizedCanonicalHashV1 public var finalizedBlockHash: Data

    private enum CodingKeys: String, CodingKey {
        case networkId = "network_id"
        case policyId = "policy_id"
        case replayNullifier = "replay_nullifier"
        case policyRecordDigest = "policy_record_digest"
        case statementDigest = "statement_digest"
        case admittedAtHeight = "admitted_at_height"
        case actionIndex = "action_index"
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
    }
}

public struct PrivacyProofManagedPoolStateViewV1: Decodable, Equatable, Sendable {
    public let networkId: NetworkId
    @PrivacyFinalizedProtocolIdV1 public var protocolId: PrivacyProtocolIdV1
    @PrivacyFinalizedFixed32BytesV1 public var poolId: Data
    public let assetDefinitionId: String
    @PrivacyFinalizedRootRoleProjectionV1 public var rootRole: PrivacyFinalizedRootRoleV1
    @PrivacyFinalizedFixed32BytesV1 public var bootstrapDigest: Data
    @PrivacyFinalizedFixed32BytesV1 public var initialRoot: Data
    @PrivacyFinalizedUInt64V1 public var currentEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var currentRoot: Data
    @PrivacyFinalizedUInt64V1 public var outputCount: UInt64
    @PrivacyFinalizedUInt64V1 public var bootstrapAdmittedAtHeight: UInt64
    public let latestTransition: PrivacyProofManagedPoolTransitionViewV1?
    @PrivacyFinalizedUInt64V1 public var finalizedHeight: UInt64
    @PrivacyFinalizedCanonicalHashV1 public var finalizedBlockHash: Data

    private enum CodingKeys: String, CodingKey {
        case networkId = "network_id"
        case protocolId = "protocol_id"
        case poolId = "pool_id"
        case assetDefinitionId = "asset_definition_id"
        case rootRole = "root_role"
        case bootstrapDigest = "bootstrap_digest"
        case initialRoot = "initial_root"
        case currentEpoch = "current_epoch"
        case currentRoot = "current_root"
        case outputCount = "output_count"
        case bootstrapAdmittedAtHeight = "bootstrap_admitted_at_height"
        case latestTransition = "latest_transition"
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
    }
}

public struct PrivacyOrchardPoolStateViewV1: Decodable, Equatable, Sendable {
    public let networkId: NetworkId
    @PrivacyFinalizedFixed32BytesV1 public var poolId: Data
    public let assetDefinitionId: String
    public let publicBalanceScope: PrivacyFinalizedAssetBalanceScopeV1
    public let reserveAccount: String
    @PrivacyFinalizedFixed32BytesV1 public var bootstrapDigest: Data
    @PrivacyFinalizedUInt64V1 public var currentEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var currentRoot: Data
    @PrivacyFinalizedUInt64V1 public var treeSize: UInt64
    public let latestTransition: PrivacyOrchardPoolTransitionViewV1?
    @PrivacyFinalizedUInt64V1 public var finalizedHeight: UInt64
    @PrivacyFinalizedCanonicalHashV1 public var finalizedBlockHash: Data

    private enum CodingKeys: String, CodingKey {
        case networkId = "network_id"
        case poolId = "pool_id"
        case assetDefinitionId = "asset_definition_id"
        case publicBalanceScope = "public_balance_scope"
        case reserveAccount = "reserve_account"
        case bootstrapDigest = "bootstrap_digest"
        case currentEpoch = "current_epoch"
        case currentRoot = "current_root"
        case treeSize = "tree_size"
        case latestTransition = "latest_transition"
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
    }
}

public struct PrivacyOrchardNullifierProvenanceV1: Decodable, Equatable, Sendable {
    public let networkId: NetworkId
    @PrivacyFinalizedFixed32BytesV1 public var poolId: Data
    @PrivacyFinalizedFixed32BytesV1 public var nullifier: Data
    @PrivacyFinalizedFixed32BytesV1 public var bootstrapDigest: Data
    @PrivacyFinalizedFixed32BytesV1 public var statementDigest: Data
    @PrivacyFinalizedUInt64V1 public var admittedAtHeight: UInt64
    @PrivacyFinalizedUInt32V1 public var actionIndex: UInt32
    @PrivacyFinalizedUInt64V1 public var finalizedHeight: UInt64
    @PrivacyFinalizedCanonicalHashV1 public var finalizedBlockHash: Data

    private enum CodingKeys: String, CodingKey {
        case networkId = "network_id"
        case poolId = "pool_id"
        case nullifier
        case bootstrapDigest = "bootstrap_digest"
        case statementDigest = "statement_digest"
        case admittedAtHeight = "admitted_at_height"
        case actionIndex = "action_index"
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
    }
}

public struct PrivacyAnonymousPgcPoolStateViewV1: Decodable, Equatable, Sendable {
    public let networkId: NetworkId
    @PrivacyFinalizedFixed32BytesV1 public var poolId: Data
    @PrivacyFinalizedUInt32V1 public var totalSupply: UInt32
    @PrivacyFinalizedFixed32BytesV1 public var bootstrapRoot: Data
    @PrivacyFinalizedFixed32BytesV1 public var bootstrapDigest: Data
    @PrivacyFinalizedFixed32BytesV1 public var bootstrapProofDigest: Data
    @PrivacyFinalizedUInt64V1 public var currentEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var currentRoot: Data
    @PrivacyFinalizedUInt32V1 public var accountCount: UInt32
    @PrivacyFinalizedUInt64V1 public var currentStateAdmittedAtHeight: UInt64
    public let latestTransition: PrivacyAnonymousPgcPoolTransitionViewV1?
    @PrivacyFinalizedUInt64V1 public var finalizedHeight: UInt64
    @PrivacyFinalizedCanonicalHashV1 public var finalizedBlockHash: Data

    private enum CodingKeys: String, CodingKey {
        case networkId = "network_id"
        case poolId = "pool_id"
        case totalSupply = "total_supply"
        case bootstrapRoot = "bootstrap_root"
        case bootstrapDigest = "bootstrap_digest"
        case bootstrapProofDigest = "bootstrap_proof_digest"
        case currentEpoch = "current_epoch"
        case currentRoot = "current_root"
        case accountCount = "account_count"
        case currentStateAdmittedAtHeight = "current_state_admitted_at_height"
        case latestTransition = "latest_transition"
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
    }
}

public struct PrivacyZkAmsAdmissionViewV1: Decodable, Equatable, Sendable {
    public let networkId: NetworkId
    @PrivacyFinalizedFixed32BytesV1 public var issuerId: Data
    @PrivacyFinalizedFixed32BytesV1 public var registryId: Data
    @PrivacyFinalizedFixed32BytesV1 public var policyId: Data
    @PrivacyFinalizedFixed32BytesV1 public var phcHash: Data
    @PrivacyFinalizedFixed32BytesV1 public var seedPublicKey: Data
    @PrivacyFinalizedFixed32BytesV1 public var bootstrapDigest: Data
    @PrivacyFinalizedFixed32BytesV1 public var issuerPolicyRecordDigest: Data
    @PrivacyFinalizedFixed32BytesV1 public var policyDigest: Data
    @PrivacyFinalizedFixed32BytesV1 public var registryRecordDigest: Data
    @PrivacyFinalizedUInt64V1 public var parentEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var parentRoot: Data
    @PrivacyFinalizedUInt32V1 public var anchorIndex: UInt32
    @PrivacyFinalizedUInt32V1 public var batchSize: UInt32
    @PrivacyFinalizedUInt64V1 public var successorEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var successorRoot: Data
    @PrivacyFinalizedFixed32BytesV1 public var statementDigest: Data
    @PrivacyFinalizedUInt64V1 public var admittedAtHeight: UInt64
    @PrivacyFinalizedUInt32V1 public var actionIndex: UInt32
    @PrivacyFinalizedUInt64V1 public var finalizedHeight: UInt64
    @PrivacyFinalizedCanonicalHashV1 public var finalizedBlockHash: Data

    private enum CodingKeys: String, CodingKey {
        case networkId = "network_id"
        case issuerId = "issuer_id"
        case registryId = "registry_id"
        case policyId = "policy_id"
        case phcHash = "phc_hash"
        case seedPublicKey = "seed_public_key"
        case bootstrapDigest = "bootstrap_digest"
        case issuerPolicyRecordDigest = "issuer_policy_record_digest"
        case policyDigest = "policy_digest"
        case registryRecordDigest = "registry_record_digest"
        case parentEpoch = "parent_epoch"
        case parentRoot = "parent_root"
        case anchorIndex = "anchor_index"
        case batchSize = "batch_size"
        case successorEpoch = "successor_epoch"
        case successorRoot = "successor_root"
        case statementDigest = "statement_digest"
        case admittedAtHeight = "admitted_at_height"
        case actionIndex = "action_index"
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
    }
}

public struct PrivacyZkAmsProvisionViewV1: Decodable, Equatable, Sendable {
    public let networkId: NetworkId
    @PrivacyFinalizedFixed32BytesV1 public var issuerId: Data
    @PrivacyFinalizedFixed32BytesV1 public var registryId: Data
    @PrivacyFinalizedFixed32BytesV1 public var policyId: Data
    @PrivacyFinalizedFixed32BytesV1 public var keyImage: Data
    public let accountId: String
    @PrivacyFinalizedFixed32BytesV1 public var bootstrapDigest: Data
    @PrivacyFinalizedFixed32BytesV1 public var issuerPolicyRecordDigest: Data
    @PrivacyFinalizedFixed32BytesV1 public var policyDigest: Data
    @PrivacyFinalizedFixed32BytesV1 public var registryRecordDigest: Data
    @PrivacyFinalizedUInt64V1 public var registryEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var registryRoot: Data
    @PrivacyFinalizedFixed32BytesV1 public var statementDigest: Data
    @PrivacyFinalizedUInt64V1 public var admittedAtHeight: UInt64
    @PrivacyFinalizedUInt32V1 public var actionIndex: UInt32
    @PrivacyFinalizedUInt64V1 public var finalizedHeight: UInt64
    @PrivacyFinalizedCanonicalHashV1 public var finalizedBlockHash: Data

    private enum CodingKeys: String, CodingKey {
        case networkId = "network_id"
        case issuerId = "issuer_id"
        case registryId = "registry_id"
        case policyId = "policy_id"
        case keyImage = "key_image"
        case accountId = "account_id"
        case bootstrapDigest = "bootstrap_digest"
        case issuerPolicyRecordDigest = "issuer_policy_record_digest"
        case policyDigest = "policy_digest"
        case registryRecordDigest = "registry_record_digest"
        case registryEpoch = "registry_epoch"
        case registryRoot = "registry_root"
        case statementDigest = "statement_digest"
        case admittedAtHeight = "admitted_at_height"
        case actionIndex = "action_index"
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
    }
}

public struct PrivacyZkX509CertificateNullifierProvenanceV1: Decodable, Equatable, Sendable {
    public let networkId: NetworkId
    @PrivacyFinalizedFixed32BytesV1 public var trustAnchorId: Data
    @PrivacyFinalizedFixed32BytesV1 public var policyId: Data
    @PrivacyFinalizedFixed32BytesV1 public var nullifier: Data
    @PrivacyFinalizedFixed32BytesV1 public var trustAnchorRecordDigest: Data
    @PrivacyFinalizedUInt64V1 public var trustAnchorRecordEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var certificatePolicyRecordDigest: Data
    @PrivacyFinalizedUInt64V1 public var certificatePolicyRecordEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var crlRecordDigest: Data
    @PrivacyFinalizedUInt64V1 public var crlRecordEpoch: UInt64
    @PrivacyFinalizedFixed32BytesV1 public var statementDigest: Data
    @PrivacyFinalizedUInt64V1 public var admittedAtHeight: UInt64
    @PrivacyFinalizedUInt32V1 public var actionIndex: UInt32
    @PrivacyFinalizedUInt64V1 public var finalizedHeight: UInt64
    @PrivacyFinalizedCanonicalHashV1 public var finalizedBlockHash: Data

    private enum CodingKeys: String, CodingKey {
        case networkId = "network_id"
        case trustAnchorId = "trust_anchor_id"
        case policyId = "policy_id"
        case nullifier
        case trustAnchorRecordDigest = "trust_anchor_record_digest"
        case trustAnchorRecordEpoch = "trust_anchor_record_epoch"
        case certificatePolicyRecordDigest = "certificate_policy_record_digest"
        case certificatePolicyRecordEpoch = "certificate_policy_record_epoch"
        case crlRecordDigest = "crl_record_digest"
        case crlRecordEpoch = "crl_record_epoch"
        case statementDigest = "statement_digest"
        case admittedAtHeight = "admitted_at_height"
        case actionIndex = "action_index"
        case finalizedHeight = "finalized_height"
        case finalizedBlockHash = "finalized_block_hash"
    }
}
