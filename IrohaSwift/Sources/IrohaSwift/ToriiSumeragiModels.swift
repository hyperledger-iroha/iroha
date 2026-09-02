import Foundation

/// A frozen Sumeragi v2 height-context identifier in Torii's canonical JSON shape.
public struct ToriiSumeragiV2HeightContextID: Decodable, Sendable, Equatable {
    public let hash: String

    public init(from decoder: Decoder) throws {
        var container = try decoder.unkeyedContainer()
        let hash = try container.decode(String.self)
        guard container.isAtEnd, ToriiNativeAmxWire.isCanonicalHash(hash) else {
            throw DecodingError.dataCorruptedError(
                in: container,
                debugDescription: "height_context_id must contain exactly one canonical hash"
            )
        }
        self.hash = hash
    }
}

/// A Sumeragi v2 consensus round reported by the status endpoint.
public struct ToriiSumeragiV2ConsensusRound: Decodable, Sendable, Equatable {
    public let contextID: ToriiSumeragiV2HeightContextID
    public let height: UInt64
    public let view: UInt64

    private enum CodingKeys: String, CodingKey {
        case contextID = "context_id"
        case height
        case view
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["context_id", "height", "view"],
            context: "Sumeragi v2 consensus round"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        contextID = try container.decode(ToriiSumeragiV2HeightContextID.self, forKey: .contextID)
        height = try container.decode(UInt64.self, forKey: .height)
        view = try container.decode(UInt64.self, forKey: .view)
    }
}

/// The only global Sumeragi v2 quorum-certificate phases.
public enum ToriiSumeragiV2GlobalPhase: String, Decodable, Sendable, Equatable {
    case prepare
    case commit

    private enum CodingKeys: String, CodingKey {
        case phase
        case details
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["phase", "details"],
            context: "Sumeragi v2 global phase"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let raw = try container.decode(String.self, forKey: .phase)
        guard container.contains(.details), try container.decodeNil(forKey: .details) else {
            throw DecodingError.dataCorruptedError(
                forKey: .details,
                in: container,
                debugDescription: "Sumeragi v2 global phase details must be null"
            )
        }
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .phase,
                in: container,
                debugDescription: "unknown Sumeragi v2 global phase: \(raw)"
            )
        }
        self = value
    }
}

/// A block subject authenticated by Sumeragi v2 votes and certificates.
public struct ToriiSumeragiV2BlockSubject: Decodable, Sendable, Equatable {
    public let parentBlockHash: String?
    public let blockHash: String
    public let payloadHash: String

    private enum CodingKeys: String, CodingKey {
        case parentBlockHash = "parent_block_hash"
        case blockHash = "block_hash"
        case payloadHash = "payload_hash"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["parent_block_hash", "block_hash", "payload_hash"],
            context: "Sumeragi v2 block subject"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        if let raw = try container.decodeIfPresent(String.self, forKey: .parentBlockHash) {
            parentBlockHash = try ToriiNativeAmxWire.canonicalHash(
                raw,
                key: .parentBlockHash,
                container: container,
                field: "Sumeragi v2 parent_block_hash"
            )
        } else {
            parentBlockHash = nil
        }
        blockHash = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .blockHash),
            key: .blockHash,
            container: container,
            field: "Sumeragi v2 block_hash"
        )
        payloadHash = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .payloadHash),
            key: .payloadHash,
            container: container,
            field: "Sumeragi v2 payload_hash"
        )
    }
}

/// Exact Merkle root and non-zero leaf count of canonical lane-finality statements.
public struct ToriiSumeragiV2LaneFinalityManifestCommitment:
    Decodable, Sendable, Equatable
{
    public static let maximumLeafCount: UInt64 = 1024

    public let root: String
    public let leafCount: UInt64

    private enum CodingKeys: String, CodingKey {
        case root
        case leafCount = "leaf_count"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["root", "leaf_count"],
            context: "Sumeragi v2 lane-finality manifest commitment"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        root = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .root),
            key: .root,
            container: container,
            field: "Sumeragi v2 lane-finality manifest root"
        )
        leafCount = try container.decode(UInt64.self, forKey: .leafCount)
        guard leafCount > 0, leafCount <= Self.maximumLeafCount else {
            throw DecodingError.dataCorruptedError(
                forKey: .leafCount,
                in: container,
                debugDescription:
                    "Sumeragi v2 lane-finality manifest leaf_count exceeds the non-empty bound"
            )
        }
    }
}

/// Exact merge-ledger entry identity authenticated by Sumeragi v2 finality.
public struct ToriiSumeragiV2MergeCarrierCommitment: Decodable, Sendable, Equatable {
    public static let canonicalVersion: UInt16 = 1

    public let version: UInt16
    public let entryHash: String

    private enum CodingKeys: String, CodingKey {
        case version
        case entryHash = "entry_hash"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["version", "entry_hash"],
            context: "Sumeragi v2 merge-carrier commitment"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        version = try container.decode(UInt16.self, forKey: .version)
        guard version == Self.canonicalVersion else {
            throw DecodingError.dataCorruptedError(
                forKey: .version,
                in: container,
                debugDescription: "Sumeragi v2 merge-carrier version is unsupported"
            )
        }
        entryHash = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .entryHash),
            key: .entryHash,
            container: container,
            field: "Sumeragi v2 merge-carrier entry_hash"
        )
    }
}

/// Deterministic execution result authenticated by a Sumeragi v2 quorum certificate.
public struct ToriiSumeragiV2ExecutionCommitment: Decodable, Sendable, Equatable {
    public static let canonicalNativeAmxApplicationManifestVersion: UInt16 = 1
    public static let maximumNativeAmxApplicationManifestLeafCount: UInt32 = 1024
    public static let nativeAmxApplicationManifestEmptyRoot =
        "hash:45A5D35A09D284480FBA74A402D7F303B82DA0C153FC1E1083AEFC822ED07C2D#7C0F"

    public let parentStateRoot: String
    public let postStateRoot: String
    public let ordinaryWritesRoot: String
    public let offlineCashTopUpRoot: String?
    public let offlineCashTopUpCount: UInt32
    public let nativeAmxApplicationManifestVersion: UInt16
    public let nativeAmxApplicationManifestRoot: String
    public let nativeAmxApplicationManifestCount: UInt32
    public let laneFinalityManifest: ToriiSumeragiV2LaneFinalityManifestCommitment?
    public let mergeCarrier: ToriiSumeragiV2MergeCarrierCommitment?
    public let executedBlockWireLen: UInt64
    public let executedBlockWireHash: String

    private enum CodingKeys: String, CodingKey {
        case parentStateRoot = "parent_state_root"
        case postStateRoot = "post_state_root"
        case ordinaryWritesRoot = "ordinary_writes_root"
        case offlineCashTopUpRoot = "offline_cash_top_up_root"
        case offlineCashTopUpCount = "offline_cash_top_up_count"
        case nativeAmxApplicationManifestVersion =
            "native_amx_application_manifest_version"
        case nativeAmxApplicationManifestRoot = "native_amx_application_manifest_root"
        case nativeAmxApplicationManifestCount = "native_amx_application_manifest_count"
        case laneFinalityManifest = "lane_finality_manifest"
        case mergeCarrier = "merge_carrier"
        case executedBlockWireLen = "executed_block_wire_len"
        case executedBlockWireHash = "executed_block_wire_hash"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: [
                "parent_state_root", "post_state_root", "ordinary_writes_root",
                "offline_cash_top_up_root", "offline_cash_top_up_count",
                "native_amx_application_manifest_version",
                "native_amx_application_manifest_root",
                "native_amx_application_manifest_count",
                "lane_finality_manifest",
                "merge_carrier",
                "executed_block_wire_len",
                "executed_block_wire_hash",
            ],
            context: "Sumeragi v2 execution commitment"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        parentStateRoot = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .parentStateRoot),
            key: .parentStateRoot,
            container: container,
            field: "Sumeragi v2 parent_state_root"
        )
        postStateRoot = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .postStateRoot),
            key: .postStateRoot,
            container: container,
            field: "Sumeragi v2 post_state_root"
        )
        ordinaryWritesRoot = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .ordinaryWritesRoot),
            key: .ordinaryWritesRoot,
            container: container,
            field: "Sumeragi v2 ordinary_writes_root"
        )
        if let raw = try container.decodeIfPresent(String.self, forKey: .offlineCashTopUpRoot) {
            offlineCashTopUpRoot = try ToriiNativeAmxWire.canonicalHash(
                raw,
                key: .offlineCashTopUpRoot,
                container: container,
                field: "Sumeragi v2 offline_cash_top_up_root"
            )
        } else {
            offlineCashTopUpRoot = nil
        }
        offlineCashTopUpCount = try container.decode(UInt32.self, forKey: .offlineCashTopUpCount)
        guard (offlineCashTopUpCount == 0) == (offlineCashTopUpRoot == nil) else {
            throw DecodingError.dataCorruptedError(
                forKey: .offlineCashTopUpCount,
                in: container,
                debugDescription:
                    "Sumeragi v2 offline-cash top-up count/root projection is not canonical"
            )
        }
        nativeAmxApplicationManifestVersion = try container.decode(
            UInt16.self,
            forKey: .nativeAmxApplicationManifestVersion
        )
        guard nativeAmxApplicationManifestVersion ==
            Self.canonicalNativeAmxApplicationManifestVersion else {
            throw DecodingError.dataCorruptedError(
                forKey: .nativeAmxApplicationManifestVersion,
                in: container,
                debugDescription:
                    "Sumeragi v2 Native AMX application-manifest version is unsupported"
            )
        }
        nativeAmxApplicationManifestRoot = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .nativeAmxApplicationManifestRoot),
            key: .nativeAmxApplicationManifestRoot,
            container: container,
            field: "Sumeragi v2 native_amx_application_manifest_root"
        )
        nativeAmxApplicationManifestCount = try container.decode(
            UInt32.self,
            forKey: .nativeAmxApplicationManifestCount
        )
        guard nativeAmxApplicationManifestCount <=
            Self.maximumNativeAmxApplicationManifestLeafCount,
            (nativeAmxApplicationManifestCount == 0) ==
                (nativeAmxApplicationManifestRoot ==
                    Self.nativeAmxApplicationManifestEmptyRoot) else {
            throw DecodingError.dataCorruptedError(
                forKey: .nativeAmxApplicationManifestCount,
                in: container,
                debugDescription:
                    "Sumeragi v2 Native AMX application-manifest count/root projection is not canonical"
            )
        }
        guard container.contains(.laneFinalityManifest) else {
            throw DecodingError.keyNotFound(
                CodingKeys.laneFinalityManifest,
                DecodingError.Context(
                    codingPath: container.codingPath,
                    debugDescription:
                        "Sumeragi v2 lane_finality_manifest is mandatory on the wire"
                )
            )
        }
        laneFinalityManifest = try container.decodeIfPresent(
            ToriiSumeragiV2LaneFinalityManifestCommitment.self,
            forKey: .laneFinalityManifest
        )
        guard container.contains(.mergeCarrier) else {
            throw DecodingError.keyNotFound(
                CodingKeys.mergeCarrier,
                DecodingError.Context(
                    codingPath: container.codingPath,
                    debugDescription: "Sumeragi v2 merge_carrier is mandatory on the wire"
                )
            )
        }
        mergeCarrier = try container.decodeIfPresent(
            ToriiSumeragiV2MergeCarrierCommitment.self,
            forKey: .mergeCarrier
        )
        executedBlockWireLen = try container.decode(
            UInt64.self,
            forKey: .executedBlockWireLen
        )
        guard executedBlockWireLen != 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .executedBlockWireLen,
                in: container,
                debugDescription: "Sumeragi v2 executed block wire length must be non-zero"
            )
        }
        executedBlockWireHash = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .executedBlockWireHash),
            key: .executedBlockWireHash,
            container: container,
            field: "Sumeragi v2 executed_block_wire_hash"
        )
    }
}

/// A stable reference to a Sumeragi v2 quorum certificate.
public struct ToriiSumeragiV2QuorumCertificateRef: Decodable, Sendable, Equatable {
    public let round: ToriiSumeragiV2ConsensusRound
    public let proposalRound: ToriiSumeragiV2ConsensusRound
    public let phase: ToriiSumeragiV2GlobalPhase
    public let subject: ToriiSumeragiV2BlockSubject
    public let executionCommitment: ToriiSumeragiV2ExecutionCommitment

    private enum CodingKeys: String, CodingKey {
        case round
        case proposalRound = "proposal_round"
        case phase
        case subject
        case executionCommitment = "execution_commitment"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["round", "proposal_round", "phase", "subject", "execution_commitment"],
            context: "Sumeragi v2 quorum-certificate reference"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        round = try container.decode(ToriiSumeragiV2ConsensusRound.self, forKey: .round)
        proposalRound = try container.decode(
            ToriiSumeragiV2ConsensusRound.self,
            forKey: .proposalRound
        )
        phase = try container.decode(ToriiSumeragiV2GlobalPhase.self, forKey: .phase)
        subject = try container.decode(ToriiSumeragiV2BlockSubject.self, forKey: .subject)
        executionCommitment = try container.decode(
            ToriiSumeragiV2ExecutionCommitment.self,
            forKey: .executionCommitment
        )
    }
}

/// A stable reference to the most recently installed timeout certificate.
public struct ToriiSumeragiV2TimeoutCertificateRef: Decodable, Sendable, Equatable {
    public let round: ToriiSumeragiV2ConsensusRound
    public let highestPrepareQC: ToriiSumeragiV2QuorumCertificateRef?
    public let certificateHash: String

    private enum CodingKeys: String, CodingKey {
        case round
        case highestPrepareQC = "highest_prepare_qc"
        case certificateHash = "certificate_hash"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["round", "highest_prepare_qc", "certificate_hash"],
            context: "Sumeragi v2 timeout-certificate reference"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        round = try container.decode(ToriiSumeragiV2ConsensusRound.self, forKey: .round)
        highestPrepareQC = try container.decodeIfPresent(
            ToriiSumeragiV2QuorumCertificateRef.self,
            forKey: .highestPrepareQC
        )
        certificateHash = try ToriiNativeAmxWire.canonicalHash(
            container.decode(String.self, forKey: .certificateHash),
            key: .certificateHash,
            container: container,
            field: "Sumeragi v2 timeout certificate hash"
        )
    }
}

/// High-level phase of the single authoritative Sumeragi v2 reducer.
public enum ToriiSumeragiV2StatusPhase: String, Decodable, Sendable, Equatable {
    case awaitingProposal = "awaiting_proposal"
    case reconstructingPayload = "reconstructing_payload"
    case validatingPayload = "validating_payload"
    case prepare
    case commit
    case pendingApply = "pending_apply"

    private enum CodingKeys: String, CodingKey {
        case phase
        case details
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["phase", "details"],
            context: "Sumeragi v2 status phase"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let raw = try container.decode(String.self, forKey: .phase)
        guard container.contains(.details), try container.decodeNil(forKey: .details) else {
            throw DecodingError.dataCorruptedError(
                forKey: .details,
                in: container,
                debugDescription: "Sumeragi v2 status phase details must be null"
            )
        }
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .phase,
                in: container,
                debugDescription: "unknown Sumeragi v2 status phase: \(raw)"
            )
        }
        self = value
    }
}

/// Local body availability/application state of the Sumeragi v2 reducer.
public enum ToriiSumeragiV2BodyState: String, Decodable, Sendable, Equatable {
    case missing
    case reconstructing
    case stored
    case validated
    case pendingApply = "pending_apply"
    case applied

    private enum CodingKeys: String, CodingKey {
        case state
        case details
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["state", "details"],
            context: "Sumeragi v2 body state"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let raw = try container.decode(String.self, forKey: .state)
        guard container.contains(.details), try container.decodeNil(forKey: .details) else {
            throw DecodingError.dataCorruptedError(
                forKey: .details,
                in: container,
                debugDescription: "Sumeragi v2 body-state details must be null"
            )
        }
        guard let value = Self(rawValue: raw) else {
            throw DecodingError.dataCorruptedError(
                forKey: .state,
                in: container,
                debugDescription: "unknown Sumeragi v2 body state: \(raw)"
            )
        }
        self = value
    }
}

private protocol ToriiSumeragiV2TaggedUnitSchema {
    static var tag: String { get }
    static var values: Set<String> { get }
}

private struct ToriiSumeragiV2TaggedUnit<Schema: ToriiSumeragiV2TaggedUnitSchema>:
    Decodable
{
    let value: String

    init(from decoder: Decoder) throws {
        let container = try decoder.container(
            keyedBy: ToriiSumeragiV2DynamicCodingKey.self
        )
        let allowed = Set([Schema.tag, "details"])
        if let unknown = container.allKeys.first(where: {
            !allowed.contains($0.stringValue)
        }) {
            throw DecodingError.dataCorruptedError(
                forKey: unknown,
                in: container,
                debugDescription:
                    "unknown Sumeragi v2 \(Schema.tag) field \(unknown.stringValue)"
            )
        }
        guard let tagKey = ToriiSumeragiV2DynamicCodingKey(stringValue: Schema.tag),
              let detailsKey = ToriiSumeragiV2DynamicCodingKey(stringValue: "details")
        else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: decoder.codingPath,
                    debugDescription: "invalid Sumeragi v2 tagged-unit schema"
                )
            )
        }
        value = try container.decode(String.self, forKey: tagKey)
        guard Schema.values.contains(value) else {
            throw DecodingError.dataCorruptedError(
                forKey: tagKey,
                in: container,
                debugDescription: "unknown Sumeragi v2 \(Schema.tag) value \(value)"
            )
        }
        guard container.contains(detailsKey), try container.decodeNil(forKey: detailsKey) else {
            throw DecodingError.dataCorruptedError(
                forKey: detailsKey,
                in: container,
                debugDescription: "Sumeragi v2 \(Schema.tag) details must be null"
            )
        }
    }
}

private enum ToriiSumeragiV2ConsensusModeSchema: ToriiSumeragiV2TaggedUnitSchema {
    static let tag = "mode"
    static let values: Set<String> = ["permissioned", "npos"]
}

private enum ToriiSumeragiV2OutboundIntentKindSchema:
    ToriiSumeragiV2TaggedUnitSchema
{
    static let tag = "kind"
    static let values: Set<String> = [
        "proposal", "prepare_vote", "commit_vote", "prepare_qc", "commit_qc",
        "timeout_vote", "timeout_certificate",
    ]
}

private enum ToriiSumeragiV2OutboundIntentStageSchema:
    ToriiSumeragiV2TaggedUnitSchema
{
    static let tag = "stage"
    static let values: Set<String> = [
        "pending_persistence", "pending_signature", "queued", "sent",
    ]
}

private enum ToriiSumeragiV2LocalWorkStageSchema: ToriiSumeragiV2TaggedUnitSchema {
    static let tag = "stage"
    static let values: Set<String> = ["idle", "queued", "running", "complete"]
}

private enum ToriiSumeragiV2QueueKindSchema: ToriiSumeragiV2TaggedUnitSchema {
    static let tag = "queue"
    static let values: Set<String> = [
        "ingress", "deferred_normal", "deferred_progress", "deferred_completion",
        "runtime_normal", "runtime_progress", "runtime_completion",
        "effect_completion", "network_ingress", "effect_dispatch",
    ]
}

private enum ToriiSumeragiV2ProgressTransitionSchema:
    ToriiSumeragiV2TaggedUnitSchema
{
    static let tag = "transition"
    static let values: Set<String> = [
        "proposal_admitted", "body_available", "body_stored", "body_validated",
        "prepare_vote_admitted", "commit_vote_admitted", "timeout_vote_admitted",
        "prepare_quorum", "lock_installed", "commit_quorum",
        "timeout_certificate_installed", "decision_persisted", "applied",
        "successor_height_activated", "recovery_replayed",
    ]
}

private enum ToriiSumeragiV2LivenessBlockerSchema:
    ToriiSumeragiV2TaggedUnitSchema
{
    static let tag = "blocker"
    static let values: Set<String> = [
        "missing_proposal", "body_unavailable", "prepare_quorum_missing",
        "commit_quorum_missing", "timeout_certificate_missing",
        "scheduler_starvation", "application_pending",
        "successor_activation_pending", "local_control_pending",
    ]
}

private enum ToriiSumeragiV2IgnoreReasonSchema: ToriiSumeragiV2TaggedUnitSchema {
    static let tag = "reason"
    static let values: Set<String> = [
        "wrong_height", "wrong_view", "stale_generation", "busy", "duplicate",
        "no_matching_work", "observer", "view_closed", "already_decided",
        "recovery_pending", "irrelevant_view", "unsafe_proposal",
    ]
}

/// Frozen validator-election context governing an authoritative status height.
public struct ToriiSumeragiV2HeightContextStatus: Decodable, Sendable, Equatable {
    public let epoch: UInt64
    public let epochEndHeight: UInt64
    public let mode: String
    public let epochSeed: [UInt8]
    public let validatorCount: UInt32
    public let minSigners: UInt32
    public let totalPower: UInt64

    private enum CodingKeys: String, CodingKey {
        case epoch
        case epochEndHeight = "epoch_end_height"
        case mode
        case epochSeed = "epoch_seed"
        case validatorCount = "validator_count"
        case quorum
    }

    private struct Quorum: Decodable {
        let minSigners: UInt32
        let totalPower: UInt64

        private enum CodingKeys: String, CodingKey {
            case minSigners = "min_signers"
            case totalPower = "total_power"
        }

        init(from decoder: Decoder) throws {
            try rejectUnknownNativeAmxFields(
                from: decoder,
                allowed: ["min_signers", "total_power"],
                context: "Sumeragi v2 height-context quorum"
            )
            let container = try decoder.container(keyedBy: CodingKeys.self)
            minSigners = try container.decode(UInt32.self, forKey: .minSigners)
            totalPower = try container.decode(UInt64.self, forKey: .totalPower)
        }
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: [
                "epoch", "epoch_end_height", "mode", "epoch_seed",
                "validator_count", "quorum",
            ],
            context: "Sumeragi v2 height context"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        epoch = try container.decode(UInt64.self, forKey: .epoch)
        epochEndHeight = try container.decode(UInt64.self, forKey: .epochEndHeight)
        mode = try container.decode(
            ToriiSumeragiV2TaggedUnit<ToriiSumeragiV2ConsensusModeSchema>.self,
            forKey: .mode
        ).value
        epochSeed = try container.decode([UInt8].self, forKey: .epochSeed)
        validatorCount = try container.decode(UInt32.self, forKey: .validatorCount)
        let quorum = try container.decode(Quorum.self, forKey: .quorum)
        minSigners = quorum.minSigners
        totalPower = quorum.totalPower
        let expectedMinSigners = validatorCount == 0
            ? 0
            : validatorCount - (validatorCount - 1) / 3
        guard epochEndHeight > 0,
              epochSeed.count == 32,
              validatorCount >= 4,
              validatorCount <= 31,
              (validatorCount - 1) % 3 == 0,
              minSigners == expectedMinSigners,
              totalPower == UInt64(validatorCount)
        else {
            throw DecodingError.dataCorruptedError(
                forKey: .quorum,
                in: container,
                debugDescription: "Sumeragi v2 height context is inconsistent"
            )
        }
    }
}

/// Authenticated durable CommitQC summary carried by authoritative status.
public struct ToriiSumeragiV2CommitQcStatus: Decodable, Sendable, Equatable {
    public let certificate: ToriiSumeragiV2QuorumCertificateRef
    public let validatorCount: UInt32
    public let signerCount: UInt32
    public let minSigners: UInt32
    public let signedPower: UInt64
    public let totalPower: UInt64

    private enum CodingKeys: String, CodingKey {
        case certificate
        case validatorCount = "validator_count"
        case signerCount = "signer_count"
        case minSigners = "min_signers"
        case signedPower = "signed_power"
        case totalPower = "total_power"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: [
                "certificate", "validator_count", "signer_count", "min_signers",
                "signed_power", "total_power",
            ],
            context: "Sumeragi v2 CommitQC status"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        certificate = try container.decode(
            ToriiSumeragiV2QuorumCertificateRef.self,
            forKey: .certificate
        )
        validatorCount = try container.decode(UInt32.self, forKey: .validatorCount)
        signerCount = try container.decode(UInt32.self, forKey: .signerCount)
        minSigners = try container.decode(UInt32.self, forKey: .minSigners)
        signedPower = try container.decode(UInt64.self, forKey: .signedPower)
        totalPower = try container.decode(UInt64.self, forKey: .totalPower)
        guard validatorCount >= 4,
              validatorCount <= 31,
              (validatorCount - 1) % 3 == 0,
              signerCount <= validatorCount,
              minSigners == validatorCount - (validatorCount - 1) / 3,
              signerCount == minSigners,
              totalPower == UInt64(validatorCount),
              signedPower == UInt64(signerCount)
        else {
            throw DecodingError.dataCorruptedError(
                forKey: .signerCount,
                in: container,
                debugDescription: "Sumeragi v2 CommitQC quorum is inconsistent"
            )
        }
    }
}

/// Partial authenticated vote-quorum state for one exact proposal.
public struct ToriiSumeragiV2VoteQuorumStatus: Decodable, Sendable, Equatable {
    public let round: ToriiSumeragiV2ConsensusRound
    public let proposalRound: ToriiSumeragiV2ConsensusRound
    public let subject: ToriiSumeragiV2BlockSubject
    public let executionCommitment: ToriiSumeragiV2ExecutionCommitment
    public let signerCount: UInt32
    public let signedPower: UInt64
    public let minSigners: UInt32
    public let totalPower: UInt64

    private enum CodingKeys: String, CodingKey {
        case round
        case proposalRound = "proposal_round"
        case subject
        case executionCommitment = "execution_commitment"
        case signerCount = "signer_count"
        case signedPower = "signed_power"
        case minSigners = "min_signers"
        case totalPower = "total_power"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: [
                "round", "proposal_round", "subject", "execution_commitment",
                "signer_count", "signed_power", "min_signers", "total_power",
            ],
            context: "Sumeragi v2 vote quorum status"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        round = try container.decode(ToriiSumeragiV2ConsensusRound.self, forKey: .round)
        proposalRound = try container.decode(
            ToriiSumeragiV2ConsensusRound.self,
            forKey: .proposalRound
        )
        subject = try container.decode(ToriiSumeragiV2BlockSubject.self, forKey: .subject)
        executionCommitment = try container.decode(
            ToriiSumeragiV2ExecutionCommitment.self,
            forKey: .executionCommitment
        )
        signerCount = try container.decode(UInt32.self, forKey: .signerCount)
        signedPower = try container.decode(UInt64.self, forKey: .signedPower)
        minSigners = try container.decode(UInt32.self, forKey: .minSigners)
        totalPower = try container.decode(UInt64.self, forKey: .totalPower)
        guard totalPower >= 4,
              totalPower <= 31,
              (totalPower - 1) % 3 == 0,
              signerCount <= UInt32(totalPower),
              minSigners == UInt32(totalPower) - (UInt32(totalPower) - 1) / 3,
              signedPower == UInt64(signerCount),
              proposalRound == round else {
            throw DecodingError.dataCorruptedError(
                forKey: .signerCount,
                in: container,
                debugDescription: "Sumeragi v2 vote quorum is inconsistent"
            )
        }
    }
}

/// Partial authenticated timeout-quorum state for one exact round.
public struct ToriiSumeragiV2TimeoutQuorumStatus: Decodable, Sendable, Equatable {
    public let round: ToriiSumeragiV2ConsensusRound
    public let signerCount: UInt32
    public let signedPower: UInt64
    public let minSigners: UInt32
    public let totalPower: UInt64
    public let certificateFormed: Bool

    private enum CodingKeys: String, CodingKey {
        case round
        case signerCount = "signer_count"
        case signedPower = "signed_power"
        case minSigners = "min_signers"
        case totalPower = "total_power"
        case certificateFormed = "certificate_formed"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: [
                "round", "signer_count", "signed_power", "min_signers",
                "total_power", "certificate_formed",
            ],
            context: "Sumeragi v2 timeout quorum status"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        round = try container.decode(ToriiSumeragiV2ConsensusRound.self, forKey: .round)
        signerCount = try container.decode(UInt32.self, forKey: .signerCount)
        signedPower = try container.decode(UInt64.self, forKey: .signedPower)
        minSigners = try container.decode(UInt32.self, forKey: .minSigners)
        totalPower = try container.decode(UInt64.self, forKey: .totalPower)
        certificateFormed = try container.decode(Bool.self, forKey: .certificateFormed)
        guard totalPower >= 4,
              totalPower <= 31,
              (totalPower - 1) % 3 == 0,
              signerCount <= UInt32(totalPower),
              minSigners == UInt32(totalPower) - (UInt32(totalPower) - 1) / 3,
              signedPower == UInt64(signerCount),
              !certificateFormed
                || signerCount >= minSigners else {
            throw DecodingError.dataCorruptedError(
                forKey: .signerCount,
                in: container,
                debugDescription: "Sumeragi v2 timeout quorum is inconsistent"
            )
        }
    }
}

/// One durable protocol intent retained for outbound service.
public struct ToriiSumeragiV2OutboundIntentStatus: Decodable, Sendable, Equatable {
    public let kind: String
    public let round: ToriiSumeragiV2ConsensusRound
    public let proposalRound: ToriiSumeragiV2ConsensusRound?
    public let subject: ToriiSumeragiV2BlockSubject?
    public let executionCommitment: ToriiSumeragiV2ExecutionCommitment?
    public let stage: String

    private enum CodingKeys: String, CodingKey {
        case kind
        case round
        case proposalRound = "proposal_round"
        case subject
        case executionCommitment = "execution_commitment"
        case stage
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: [
                "kind", "round", "proposal_round", "subject",
                "execution_commitment", "stage",
            ],
            context: "Sumeragi v2 outbound intent"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        kind = try container.decode(
            ToriiSumeragiV2TaggedUnit<ToriiSumeragiV2OutboundIntentKindSchema>.self,
            forKey: .kind
        ).value
        round = try container.decode(ToriiSumeragiV2ConsensusRound.self, forKey: .round)
        proposalRound = try container.decodeIfPresent(
            ToriiSumeragiV2ConsensusRound.self,
            forKey: .proposalRound
        )
        subject = try container.decodeIfPresent(
            ToriiSumeragiV2BlockSubject.self,
            forKey: .subject
        )
        executionCommitment = try container.decodeIfPresent(
            ToriiSumeragiV2ExecutionCommitment.self,
            forKey: .executionCommitment
        )
        stage = try container.decode(
            ToriiSumeragiV2TaggedUnit<ToriiSumeragiV2OutboundIntentStageSchema>.self,
            forKey: .stage
        ).value
    }
}

/// Current local body, validation, application, and handoff pipeline.
public struct ToriiSumeragiV2WorkStatus: Decodable, Sendable, Equatable {
    public let candidate: String
    public let bodyRecovery: String
    public let bodyStore: String
    public let validation: String
    public let application: String
    public let successorHeight: String

    private enum CodingKeys: String, CodingKey {
        case candidate
        case bodyRecovery = "body_recovery"
        case bodyStore = "body_store"
        case validation
        case application
        case successorHeight = "successor_height"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: [
                "candidate", "body_recovery", "body_store", "validation",
                "application", "successor_height",
            ],
            context: "Sumeragi v2 local work"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        func stage(_ key: CodingKeys) throws -> String {
            try container.decode(
                ToriiSumeragiV2TaggedUnit<ToriiSumeragiV2LocalWorkStageSchema>.self,
                forKey: key
            ).value
        }
        candidate = try stage(.candidate)
        bodyRecovery = try stage(.bodyRecovery)
        bodyStore = try stage(.bodyStore)
        validation = try stage(.validation)
        application = try stage(.application)
        successorHeight = try stage(.successorHeight)
    }
}

/// Occupancy and fairness state for one bounded local queue.
public struct ToriiSumeragiV2QueueStatus: Decodable, Sendable, Equatable {
    public let queue: String
    public let depth: UInt32
    public let capacity: UInt32
    public let oldestAgeMs: UInt64?
    public let serviceDebt: UInt64

    private enum CodingKeys: String, CodingKey {
        case queue
        case depth
        case capacity
        case oldestAgeMs = "oldest_age_ms"
        case serviceDebt = "service_debt"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["queue", "depth", "capacity", "oldest_age_ms", "service_debt"],
            context: "Sumeragi v2 queue status"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        queue = try container.decode(
            ToriiSumeragiV2TaggedUnit<ToriiSumeragiV2QueueKindSchema>.self,
            forKey: .queue
        ).value
        depth = try container.decode(UInt32.self, forKey: .depth)
        capacity = try container.decode(UInt32.self, forKey: .capacity)
        oldestAgeMs = try container.decodeIfPresent(UInt64.self, forKey: .oldestAgeMs)
        serviceDebt = try container.decode(UInt64.self, forKey: .serviceDebt)
        guard capacity > 0,
              depth <= capacity,
              (depth == 0) == (oldestAgeMs == nil) else {
            throw DecodingError.dataCorruptedError(
                forKey: .depth,
                in: container,
                debugDescription: "Sumeragi v2 queue depth exceeds capacity"
            )
        }
    }
}

/// Last tracked reducer transition and its local age.
public struct ToriiSumeragiV2ProgressTransitionStatus:
    Decodable, Sendable, Equatable
{
    public let generation: UInt64
    public let round: ToriiSumeragiV2ConsensusRound
    public let transition: String
    public let ageMs: UInt64

    private enum CodingKeys: String, CodingKey {
        case generation
        case round
        case transition
        case ageMs = "age_ms"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["generation", "round", "transition", "age_ms"],
            context: "Sumeragi v2 progress transition"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        generation = try container.decode(UInt64.self, forKey: .generation)
        round = try container.decode(ToriiSumeragiV2ConsensusRound.self, forKey: .round)
        transition = try container.decode(
            ToriiSumeragiV2TaggedUnit<ToriiSumeragiV2ProgressTransitionSchema>.self,
            forKey: .transition
        ).value
        ageMs = try container.decode(UInt64.self, forKey: .ageMs)
    }
}

/// Per-height counter for one closed input-ignore reason.
public struct ToriiSumeragiV2IgnoreCount: Decodable, Sendable, Equatable {
    public let reason: String
    public let count: UInt64

    private enum CodingKeys: String, CodingKey {
        case reason
        case count
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: ["reason", "count"],
            context: "Sumeragi v2 ignore count"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        reason = try container.decode(
            ToriiSumeragiV2TaggedUnit<ToriiSumeragiV2IgnoreReasonSchema>.self,
            forKey: .reason
        ).value
        count = try container.decode(UInt64.self, forKey: .count)
    }
}

/// Authoritative progress diagnostics for the active Sumeragi v2 height.
public struct ToriiSumeragiV2LivenessStatus: Decodable, Sendable, Equatable {
    public let generation: UInt64
    public let prepareQuorums: [ToriiSumeragiV2VoteQuorumStatus]
    public let commitQuorums: [ToriiSumeragiV2VoteQuorumStatus]
    public let timeoutQuorums: [ToriiSumeragiV2TimeoutQuorumStatus]
    public let outboundIntents: [ToriiSumeragiV2OutboundIntentStatus]
    public let work: ToriiSumeragiV2WorkStatus
    public let queues: [ToriiSumeragiV2QueueStatus]
    public let lastProgress: ToriiSumeragiV2ProgressTransitionStatus?
    public let noProgressAgeMs: UInt64
    public let blocker: String?
    public let ignoreCounts: [ToriiSumeragiV2IgnoreCount]

    private enum CodingKeys: String, CodingKey {
        case generation
        case prepareQuorums = "prepare_quorums"
        case commitQuorums = "commit_quorums"
        case timeoutQuorums = "timeout_quorums"
        case outboundIntents = "outbound_intents"
        case work
        case queues
        case lastProgress = "last_progress"
        case noProgressAgeMs = "no_progress_age_ms"
        case blocker
        case ignoreCounts = "ignore_counts"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: [
                "generation", "prepare_quorums", "commit_quorums",
                "timeout_quorums", "outbound_intents", "work", "queues",
                "last_progress", "no_progress_age_ms", "blocker", "ignore_counts",
            ],
            context: "Sumeragi v2 liveness status"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        generation = try container.decode(UInt64.self, forKey: .generation)
        prepareQuorums = try container.decode(
            [ToriiSumeragiV2VoteQuorumStatus].self,
            forKey: .prepareQuorums
        )
        commitQuorums = try container.decode(
            [ToriiSumeragiV2VoteQuorumStatus].self,
            forKey: .commitQuorums
        )
        timeoutQuorums = try container.decode(
            [ToriiSumeragiV2TimeoutQuorumStatus].self,
            forKey: .timeoutQuorums
        )
        outboundIntents = try container.decode(
            [ToriiSumeragiV2OutboundIntentStatus].self,
            forKey: .outboundIntents
        )
        work = try container.decode(ToriiSumeragiV2WorkStatus.self, forKey: .work)
        queues = try container.decode([ToriiSumeragiV2QueueStatus].self, forKey: .queues)
        lastProgress = try container.decodeIfPresent(
            ToriiSumeragiV2ProgressTransitionStatus.self,
            forKey: .lastProgress
        )
        noProgressAgeMs = try container.decode(UInt64.self, forKey: .noProgressAgeMs)
        if container.contains(.blocker), !(try container.decodeNil(forKey: .blocker)) {
            blocker = try container.decode(
                ToriiSumeragiV2TaggedUnit<ToriiSumeragiV2LivenessBlockerSchema>.self,
                forKey: .blocker
            ).value
        } else {
            blocker = nil
        }
        ignoreCounts = try container.decode(
            [ToriiSumeragiV2IgnoreCount].self,
            forKey: .ignoreCounts
        )
        guard prepareQuorums.count <= 31,
              commitQuorums.count <= 32,
              timeoutQuorums.count <= 31,
              outboundIntents.count <= 7,
              queues.count <= 10,
              ignoreCounts.count <= 12,
              Set(queues.map(\.queue)).count == queues.count,
              Set(ignoreCounts.map(\.reason)).count == ignoreCounts.count,
              lastProgress.map({ $0.generation <= generation }) != false
        else {
            throw DecodingError.dataCorruptedError(
                forKey: .queues,
                in: container,
                debugDescription: "Sumeragi v2 liveness vector exceeds its protocol bound"
            )
        }
    }
}

/// Authoritative protocol-v2-only snapshot returned by `/v1/sumeragi/status`.
public struct ToriiSumeragiStatusSnapshot: Decodable, Sendable, Equatable {
    public let protocolVersion: UInt16
    public let nodeFingerprint: String
    public let buildFingerprint: String
    public let configFingerprint: String
    /// Whether the local consensus process has fail-stopped and must be restarted.
    public let restartRequired: Bool
    public let heightContextID: ToriiSumeragiV2HeightContextID
    public let height: UInt64
    public let view: UInt64
    public let phase: ToriiSumeragiV2StatusPhase
    public let leader: UInt32
    public let lockedPrepareQC: ToriiSumeragiV2QuorumCertificateRef?
    public let highestPrepareQC: ToriiSumeragiV2QuorumCertificateRef?
    public let lastTimeoutCertificate: ToriiSumeragiV2TimeoutCertificateRef?
    public let bodyState: ToriiSumeragiV2BodyState
    public let pendingPersistenceID: UInt64?
    public let lastCommittedHeight: UInt64
    public let lastCommittedSubject: ToriiSumeragiV2BlockSubject?
    public let heightContext: ToriiSumeragiV2HeightContextStatus
    public let lastCommitQC: ToriiSumeragiV2CommitQcStatus?
    public let liveness: ToriiSumeragiV2LivenessStatus

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case protocolVersion = "protocol_version"
        case nodeFingerprint = "node_fingerprint"
        case buildFingerprint = "build_fingerprint"
        case configFingerprint = "config_fingerprint"
        case restartRequired = "restart_required"
        case heightContextID = "height_context_id"
        case height
        case view
        case phase
        case leader
        case lockedPrepareQC = "locked_prepare_qc"
        case highestPrepareQC = "highest_prepare_qc"
        case lastTimeoutCertificate = "last_timeout_certificate"
        case bodyState = "body_state"
        case pendingPersistenceID = "pending_persistence_id"
        case lastCommittedHeight = "last_committed_height"
        case lastCommittedSubject = "last_committed_subject"
        case heightContext = "height_context"
        case lastCommitQC = "last_commit_qc"
        case liveness
    }

    public init(from decoder: Decoder) throws {
        let dynamic = try decoder.container(keyedBy: ToriiSumeragiV2DynamicCodingKey.self)
        let allowed = Set(CodingKeys.allCases.map(\.rawValue))
        if let unknown = dynamic.allKeys.first(where: { !allowed.contains($0.stringValue) }) {
            throw DecodingError.dataCorruptedError(
                forKey: unknown,
                in: dynamic,
                debugDescription: "unknown Sumeragi v2 status field \(unknown.stringValue)"
            )
        }
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let protocolVersion = try container.decode(UInt16.self, forKey: .protocolVersion)
        guard protocolVersion == SumeragiV2ConsensusMessage.protocolVersion else {
            throw DecodingError.dataCorruptedError(
                forKey: .protocolVersion,
                in: container,
                debugDescription: "unsupported Sumeragi status protocol version \(protocolVersion)"
            )
        }
        self.protocolVersion = protocolVersion
        let nodeFingerprint = try container.decode(String.self, forKey: .nodeFingerprint)
        let buildFingerprint = try container.decode(String.self, forKey: .buildFingerprint)
        let configFingerprint = try container.decode(String.self, forKey: .configFingerprint)
        guard ToriiNativeAmxWire.isCanonicalHash(nodeFingerprint),
              ToriiNativeAmxWire.isCanonicalHash(buildFingerprint),
              ToriiNativeAmxWire.isCanonicalHash(configFingerprint) else {
            throw DecodingError.dataCorruptedError(
                forKey: .nodeFingerprint,
                in: container,
                debugDescription: "Sumeragi v2 fingerprints must be canonical Norito hashes"
            )
        }
        self.nodeFingerprint = nodeFingerprint
        self.buildFingerprint = buildFingerprint
        self.configFingerprint = configFingerprint
        self.restartRequired = try container.decode(Bool.self, forKey: .restartRequired)
        self.heightContextID = try container.decode(
            ToriiSumeragiV2HeightContextID.self,
            forKey: .heightContextID
        )
        self.height = try container.decode(UInt64.self, forKey: .height)
        self.view = try container.decode(UInt64.self, forKey: .view)
        self.phase = try container.decode(ToriiSumeragiV2StatusPhase.self, forKey: .phase)
        self.leader = try container.decode(UInt32.self, forKey: .leader)
        self.lockedPrepareQC = try container.decodeIfPresent(
            ToriiSumeragiV2QuorumCertificateRef.self,
            forKey: .lockedPrepareQC
        )
        self.highestPrepareQC = try container.decodeIfPresent(
            ToriiSumeragiV2QuorumCertificateRef.self,
            forKey: .highestPrepareQC
        )
        self.lastTimeoutCertificate = try container.decodeIfPresent(
            ToriiSumeragiV2TimeoutCertificateRef.self,
            forKey: .lastTimeoutCertificate
        )
        self.bodyState = try container.decode(ToriiSumeragiV2BodyState.self, forKey: .bodyState)
        self.pendingPersistenceID = try container.decodeIfPresent(
            UInt64.self,
            forKey: .pendingPersistenceID
        )
        self.lastCommittedHeight = try container.decode(
            UInt64.self,
            forKey: .lastCommittedHeight
        )
        self.lastCommittedSubject = try container.decodeIfPresent(
            ToriiSumeragiV2BlockSubject.self,
            forKey: .lastCommittedSubject
        )
        self.heightContext = try container.decode(
            ToriiSumeragiV2HeightContextStatus.self,
            forKey: .heightContext
        )
        self.lastCommitQC = try container.decodeIfPresent(
            ToriiSumeragiV2CommitQcStatus.self,
            forKey: .lastCommitQC
        )
        self.liveness = try container.decode(
            ToriiSumeragiV2LivenessStatus.self,
            forKey: .liveness
        )

        guard self.height > 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .height,
                in: container,
                debugDescription: "Sumeragi status height must be positive"
            )
        }
        guard self.pendingPersistenceID != 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .pendingPersistenceID,
                in: container,
                debugDescription: "Sumeragi status persistence identifier must be non-zero"
            )
        }
        let phaseBodyIsValid: Bool
        switch (self.phase, self.bodyState) {
        case (.awaitingProposal, .missing),
             (.reconstructingPayload, .reconstructing),
             (.validatingPayload, .stored),
             (.prepare, .validated),
             (.commit, .validated),
             (.pendingApply, .pendingApply),
             (.pendingApply, .applied):
            phaseBodyIsValid = true
        default:
            phaseBodyIsValid = false
        }
        guard phaseBodyIsValid else {
            throw DecodingError.dataCorruptedError(
                forKey: .bodyState,
                in: container,
                debugDescription: "Sumeragi status phase and body state are inconsistent"
            )
        }
        if self.phase == .commit, self.lockedPrepareQC == nil {
            throw DecodingError.dataCorruptedError(
                forKey: .lockedPrepareQC,
                in: container,
                debugDescription: "Sumeragi status commit phase requires a PrepareQC lock"
            )
        }
        if self.phase == .prepare, self.lockedPrepareQC != nil {
            throw DecodingError.dataCorruptedError(
                forKey: .lockedPrepareQC,
                in: container,
                debugDescription: "Sumeragi status prepare phase cannot carry a PrepareQC lock"
            )
        }
        if self.phase == .pendingApply {
            guard self.lastCommittedHeight == self.height,
                  self.lastCommittedSubject != nil else {
                throw DecodingError.dataCorruptedError(
                    forKey: .lastCommittedHeight,
                    in: container,
                    debugDescription: "pending-apply status must carry the current decided height and subject"
                )
            }
        } else if self.lastCommittedHeight >= self.height {
            throw DecodingError.dataCorruptedError(
                forKey: .lastCommittedHeight,
                in: container,
                debugDescription: "non-decided Sumeragi status must have a committed height below the active height"
            )
        }
        if self.lastCommittedHeight == 0,
           self.lastCommittedSubject != nil || self.lastCommitQC != nil {
            throw DecodingError.dataCorruptedError(
                forKey: .lastCommittedSubject,
                in: container,
                debugDescription: "pre-genesis commit frontier cannot carry a subject"
            )
        }
        guard (self.lastCommittedSubject == nil) == (self.lastCommitQC == nil) else {
            throw DecodingError.dataCorruptedError(
                forKey: .lastCommitQC,
                in: container,
                debugDescription: "Sumeragi status commit frontier must carry both subject and CommitQC"
            )
        }
        guard self.heightContext.epochEndHeight >= self.height,
              self.leader < self.heightContext.validatorCount else {
            throw DecodingError.dataCorruptedError(
                forKey: .heightContext,
                in: container,
                debugDescription: "Sumeragi status frozen height context is inconsistent"
            )
        }
        if let lastCommitQC {
            guard let lastCommittedSubject = self.lastCommittedSubject,
                  lastCommitQC.certificate.phase == .commit,
                  lastCommitQC.certificate.round.height == self.lastCommittedHeight,
                  lastCommitQC.certificate.proposalRound.contextID
                    == lastCommitQC.certificate.round.contextID,
                  lastCommitQC.certificate.proposalRound.height
                    == lastCommitQC.certificate.round.height,
                  lastCommitQC.certificate.proposalRound.view
                    <= lastCommitQC.certificate.round.view,
                  lastCommitQC.certificate.subject == lastCommittedSubject else {
                throw DecodingError.dataCorruptedError(
                    forKey: .lastCommitQC,
                    in: container,
                    debugDescription: "Sumeragi status CommitQC does not certify its commit frontier"
                )
            }
            if lastCommitQC.certificate.round.contextID == self.heightContextID {
                guard lastCommitQC.validatorCount == self.heightContext.validatorCount,
                      lastCommitQC.minSigners == self.heightContext.minSigners,
                      lastCommitQC.totalPower == self.heightContext.totalPower else {
                    throw DecodingError.dataCorruptedError(
                        forKey: .lastCommitQC,
                        in: container,
                        debugDescription: "Sumeragi status CommitQC quorum does not match its height context"
                    )
                }
            }
        }

        func validatePrepare(_ certificate: ToriiSumeragiV2QuorumCertificateRef) throws {
            guard certificate.round.contextID == self.heightContextID else {
                throw DecodingError.dataCorruptedError(
                    forKey: .heightContextID,
                    in: container,
                    debugDescription: "Sumeragi status certificate context does not match the active context"
                )
            }
            guard certificate.round.height == self.height else {
                throw DecodingError.dataCorruptedError(
                    forKey: .height,
                    in: container,
                    debugDescription: "Sumeragi status certificate height does not match the active height"
                )
            }
            guard certificate.phase == .prepare else {
                throw DecodingError.dataCorruptedError(
                    forKey: .highestPrepareQC,
                    in: container,
                    debugDescription: "Sumeragi status QC reference must be a PrepareQC"
                )
            }
            guard certificate.proposalRound == certificate.round else {
                throw DecodingError.dataCorruptedError(
                    forKey: .highestPrepareQC,
                    in: container,
                    debugDescription: "Sumeragi status PrepareQC proposal round must match its voting round"
                )
            }
            guard certificate.round.view <= self.view else {
                throw DecodingError.dataCorruptedError(
                    forKey: .view,
                    in: container,
                    debugDescription: "Sumeragi status QC reference is from a future view"
                )
            }
        }
        if let lockedPrepareQC { try validatePrepare(lockedPrepareQC) }
        if let highestPrepareQC { try validatePrepare(highestPrepareQC) }
        switch (self.lockedPrepareQC, self.highestPrepareQC) {
        case (.some, .none):
            throw DecodingError.dataCorruptedError(
                forKey: .highestPrepareQC,
                in: container,
                debugDescription: "Sumeragi status lock requires a highest PrepareQC"
            )
        case let (.some(locked), .some(highest)) where locked.round.view > highest.round.view:
            throw DecodingError.dataCorruptedError(
                forKey: .highestPrepareQC,
                in: container,
                debugDescription: "Sumeragi status lock is above its highest PrepareQC"
            )
        case let (.some(locked), .some(highest))
            where locked.round.view == highest.round.view && locked != highest:
            throw DecodingError.dataCorruptedError(
                forKey: .highestPrepareQC,
                in: container,
                debugDescription: "Sumeragi status lock and highest PrepareQC conflict at the same view"
            )
        default:
            break
        }
        if let timeout = self.lastTimeoutCertificate {
            guard timeout.round.contextID == self.heightContextID,
                  timeout.round.height == self.height else {
                throw DecodingError.dataCorruptedError(
                    forKey: .lastTimeoutCertificate,
                    in: container,
                    debugDescription: "Sumeragi status timeout certificate context or height mismatch"
                )
            }
            guard timeout.round.view < self.view else {
                throw DecodingError.dataCorruptedError(
                    forKey: .lastTimeoutCertificate,
                    in: container,
                    debugDescription: "Sumeragi status timeout certificate must precede the current view"
                )
            }
            if let highest = timeout.highestPrepareQC {
                try validatePrepare(highest)
                guard highest.round.view <= timeout.round.view else {
                    throw DecodingError.dataCorruptedError(
                        forKey: .lastTimeoutCertificate,
                        in: container,
                        debugDescription: "Sumeragi status timeout certificate carries a PrepareQC from a future view"
                    )
                }
            }
        }
    }
}

/// Durable-application state of one Native AMX participant control.
public enum ToriiSumeragiNativeAmxParticipantApplicationState:
    String, Decodable, Sendable, Equatable
{
    case certifiedPendingCarrier = "certified_pending_carrier"
    case committedEvidencePending = "committed_evidence_pending"
    case durablyApplied = "durably_applied"
    case conflict
}

/// One bounded Native AMX participant-application diagnostics row.
public struct ToriiSumeragiNativeAmxParticipantApplication: Decodable, Sendable, Equatable {
    public let laneID: UInt32
    public let dataspaceID: UInt64
    public let laneIncarnation: String
    public let participantHeight: UInt64
    public let participantView: UInt64
    public let predecessorHeight: UInt64
    public let predecessorDescriptorHash: String?
    public let descriptorHash: String
    public let proposalHash: String
    public let settlementHash: String
    public let sourceCount: UInt64
    public let applicationBlockHeight: UInt64?
    public let applicationBlockHash: String?
    public let state: ToriiSumeragiNativeAmxParticipantApplicationState

    private enum CodingKeys: String, CodingKey {
        case laneID = "lane_id"
        case dataspaceID = "dataspace_id"
        case laneIncarnation = "lane_incarnation"
        case participantHeight = "participant_height"
        case participantView = "participant_view"
        case predecessorHeight = "predecessor_height"
        case predecessorDescriptorHash = "predecessor_descriptor_hash"
        case descriptorHash = "descriptor_hash"
        case proposalHash = "proposal_hash"
        case settlementHash = "settlement_hash"
        case sourceCount = "source_count"
        case applicationBlockHeight = "application_block_height"
        case applicationBlockHash = "application_block_hash"
        case state
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: [
                "lane_id", "dataspace_id", "lane_incarnation", "participant_height",
                "participant_view", "predecessor_height", "predecessor_descriptor_hash",
                "descriptor_hash", "proposal_hash", "settlement_hash", "source_count",
                "application_block_height", "application_block_hash", "state",
            ],
            context: "Native AMX participant application diagnostics"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        laneID = try container.decode(UInt32.self, forKey: .laneID)
        dataspaceID = try container.decode(UInt64.self, forKey: .dataspaceID)
        laneIncarnation = try container.decode(String.self, forKey: .laneIncarnation)
        participantHeight = try container.decode(UInt64.self, forKey: .participantHeight)
        participantView = try container.decode(UInt64.self, forKey: .participantView)
        predecessorHeight = try container.decode(UInt64.self, forKey: .predecessorHeight)
        predecessorDescriptorHash = try container.decodeIfPresent(
            String.self,
            forKey: .predecessorDescriptorHash
        )
        descriptorHash = try container.decode(String.self, forKey: .descriptorHash)
        proposalHash = try container.decode(String.self, forKey: .proposalHash)
        settlementHash = try container.decode(String.self, forKey: .settlementHash)
        sourceCount = try container.decode(UInt64.self, forKey: .sourceCount)
        applicationBlockHeight = try container.decodeIfPresent(
            UInt64.self,
            forKey: .applicationBlockHeight
        )
        applicationBlockHash = try container.decodeIfPresent(
            String.self,
            forKey: .applicationBlockHash
        )
        state = try container.decode(
            ToriiSumeragiNativeAmxParticipantApplicationState.self,
            forKey: .state
        )

        let hashes = [laneIncarnation, descriptorHash, proposalHash, settlementHash]
        guard hashes.allSatisfy(ToriiNativeAmxWire.isCanonicalHash),
              predecessorDescriptorHash.map(ToriiNativeAmxWire.isCanonicalHash) ?? true,
              applicationBlockHash.map(ToriiNativeAmxWire.isCanonicalHash) ?? true else {
            throw DecodingError.dataCorruptedError(
                forKey: .descriptorHash,
                in: container,
                debugDescription: "Native AMX participant diagnostics hashes must be canonical"
            )
        }
        guard participantHeight > 0,
              predecessorHeight < UInt64.max,
              predecessorHeight + 1 == participantHeight,
              (predecessorHeight == 0) == (predecessorDescriptorHash == nil) else {
            throw DecodingError.dataCorruptedError(
                forKey: .predecessorHeight,
                in: container,
                debugDescription: "Native AMX participant diagnostics predecessor is inconsistent"
            )
        }
        guard (1...4_096).contains(sourceCount) else {
            throw DecodingError.dataCorruptedError(
                forKey: .sourceCount,
                in: container,
                debugDescription: "Native AMX participant diagnostics source count is out of bounds"
            )
        }
        guard (applicationBlockHeight == nil) == (applicationBlockHash == nil),
              applicationBlockHeight != 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .applicationBlockHeight,
                in: container,
                debugDescription: "Native AMX participant diagnostics application identity is inconsistent"
            )
        }
        let requiresApplicationBlock =
            state == .committedEvidencePending || state == .durablyApplied
        guard (applicationBlockHeight != nil) == requiresApplicationBlock else {
            throw DecodingError.dataCorruptedError(
                forKey: .state,
                in: container,
                debugDescription:
                    "Native AMX participant diagnostics state and application identity are inconsistent"
            )
        }
    }
}

public enum ToriiSumeragiAutonomousLaneExecutionStage:
    String, Decodable, Sendable, Equatable
{
    case reservationsDurable = "reservations_durable"
    case executablePayloadDurable = "executable_payload_durable"
    case payloadAvailabilityCertified = "payload_availability_certified"
    case laneCertified = "lane_certified"
    case certifiedBundleDurable = "certified_bundle_durable"
    case mergeCandidateDurable = "merge_candidate_durable"
    case globalCarrierCommitted = "global_carrier_committed"
    case kuraWsvApplicationReceiptDurable = "kura_wsv_application_receipt_durable"
    case queueFinalized = "queue_finalized"
    case conflict
}

public enum ToriiSumeragiAutonomousLaneExecutionStuckReason:
    String, Decodable, Sendable, Equatable
{
    case awaitingExecutablePayload = "awaiting_executable_payload"
    case awaitingPayloadAvailability = "awaiting_payload_availability"
    case awaitingLaneCertification = "awaiting_lane_certification"
    case certifiedBundleUnavailable = "certified_bundle_unavailable"
    case awaitingMergeSelection = "awaiting_merge_selection"
    case awaitingGlobalCarrier = "awaiting_global_carrier"
    case awaitingApplicationReceipt = "awaiting_application_receipt"
    case queueFinalizationUnverifiable = "queue_finalization_unverifiable"
    case evidenceConflict = "evidence_conflict"
}

public struct ToriiSumeragiAutonomousLaneExecution: Decodable, Sendable, Equatable {
    public let laneID: UInt32
    public let dataspaceID: UInt64
    public let laneIncarnation: String
    public let laneBlockHeight: UInt64
    public let laneBlockView: UInt64
    public let proposalHeight: UInt64
    public let proposalView: UInt64?
    public let reservationOwnerHash: String
    public let proposalIdentityHash: String
    public let reservationGroupHash: String
    public let proposalHash: String?
    public let descriptorHash: String?
    public let executablePayloadHash: String?
    public let sourceBundleHash: String?
    public let mergeEntryHash: String?
    public let applicationBlockHeight: UInt64?
    public let applicationBlockHash: String?
    public let reservationCount: UInt64
    public let transactionCount: UInt64
    public let highestDurableStage: ToriiSumeragiAutonomousLaneExecutionStage
    public let stuckReason: ToriiSumeragiAutonomousLaneExecutionStuckReason?

    private enum CodingKeys: String, CodingKey, CaseIterable {
        case laneID = "lane_id", dataspaceID = "dataspace_id"
        case laneIncarnation = "lane_incarnation"
        case laneBlockHeight = "lane_block_height", laneBlockView = "lane_block_view"
        case proposalHeight = "proposal_height", proposalView = "proposal_view"
        case reservationOwnerHash = "reservation_owner_hash"
        case proposalIdentityHash = "proposal_identity_hash"
        case reservationGroupHash = "reservation_group_hash"
        case proposalHash = "proposal_hash", descriptorHash = "descriptor_hash"
        case executablePayloadHash = "executable_payload_hash"
        case sourceBundleHash = "source_bundle_hash", mergeEntryHash = "merge_entry_hash"
        case applicationBlockHeight = "application_block_height"
        case applicationBlockHash = "application_block_hash"
        case reservationCount = "reservation_count", transactionCount = "transaction_count"
        case highestDurableStage = "highest_durable_stage", stuckReason = "stuck_reason"
    }

    public init(from decoder: Decoder) throws {
        try rejectUnknownNativeAmxFields(
            from: decoder,
            allowed: Set(CodingKeys.allCases.map(\.rawValue)),
            context: "Autonomous lane execution diagnostics"
        )
        let container = try decoder.container(keyedBy: CodingKeys.self)
        laneID = try container.decode(UInt32.self, forKey: .laneID)
        dataspaceID = try container.decode(UInt64.self, forKey: .dataspaceID)
        laneIncarnation = try container.decode(String.self, forKey: .laneIncarnation)
        laneBlockHeight = try container.decode(UInt64.self, forKey: .laneBlockHeight)
        laneBlockView = try container.decode(UInt64.self, forKey: .laneBlockView)
        proposalHeight = try container.decode(UInt64.self, forKey: .proposalHeight)
        proposalView = try container.decodeIfPresent(UInt64.self, forKey: .proposalView)
        reservationOwnerHash = try container.decode(String.self, forKey: .reservationOwnerHash)
        proposalIdentityHash = try container.decode(String.self, forKey: .proposalIdentityHash)
        reservationGroupHash = try container.decode(String.self, forKey: .reservationGroupHash)
        proposalHash = try container.decodeIfPresent(String.self, forKey: .proposalHash)
        descriptorHash = try container.decodeIfPresent(String.self, forKey: .descriptorHash)
        executablePayloadHash = try container.decodeIfPresent(String.self, forKey: .executablePayloadHash)
        sourceBundleHash = try container.decodeIfPresent(String.self, forKey: .sourceBundleHash)
        mergeEntryHash = try container.decodeIfPresent(String.self, forKey: .mergeEntryHash)
        applicationBlockHeight = try container.decodeIfPresent(UInt64.self, forKey: .applicationBlockHeight)
        applicationBlockHash = try container.decodeIfPresent(String.self, forKey: .applicationBlockHash)
        reservationCount = try container.decode(UInt64.self, forKey: .reservationCount)
        transactionCount = try container.decode(UInt64.self, forKey: .transactionCount)
        highestDurableStage = try container.decode(
            ToriiSumeragiAutonomousLaneExecutionStage.self, forKey: .highestDurableStage
        )
        stuckReason = try container.decodeIfPresent(
            ToriiSumeragiAutonomousLaneExecutionStuckReason.self, forKey: .stuckReason
        )
        let hashes = [
            laneIncarnation, reservationOwnerHash, proposalIdentityHash, reservationGroupHash,
        ] + [
            proposalHash, descriptorHash, executablePayloadHash, sourceBundleHash,
            mergeEntryHash, applicationBlockHash,
        ].compactMap { $0 }
        guard hashes.allSatisfy(ToriiNativeAmxWire.isCanonicalHash),
              laneBlockHeight > 0, proposalHeight > 0,
              transactionCount > 0, transactionCount <= 4_096,
              reservationCount <= 4_096,
              (applicationBlockHeight == nil) == (applicationBlockHash == nil),
              applicationBlockHeight != 0 else {
            throw DecodingError.dataCorruptedError(
                forKey: .highestDurableStage, in: container,
                debugDescription: "Autonomous lane execution diagnostics are inconsistent"
            )
        }
        let expectedReason: ToriiSumeragiAutonomousLaneExecutionStuckReason?
        switch highestDurableStage {
        case .reservationsDurable:
            expectedReason = .awaitingExecutablePayload
        case .executablePayloadDurable:
            expectedReason = .awaitingPayloadAvailability
        case .payloadAvailabilityCertified:
            expectedReason = .awaitingLaneCertification
        case .laneCertified:
            expectedReason = .certifiedBundleUnavailable
        case .certifiedBundleDurable:
            expectedReason = .awaitingMergeSelection
        case .mergeCandidateDurable:
            expectedReason = .awaitingGlobalCarrier
        case .globalCarrierCommitted:
            expectedReason = .awaitingApplicationReceipt
        case .kuraWsvApplicationReceiptDurable:
            expectedReason = .queueFinalizationUnverifiable
        case .queueFinalized:
            expectedReason = nil
        case .conflict:
            expectedReason = .evidenceConflict
        }
        guard stuckReason == expectedReason else {
            throw DecodingError.dataCorruptedError(
                forKey: .stuckReason, in: container,
                debugDescription: "Autonomous stage and stuck reason disagree"
            )
        }
        if highestDurableStage != .conflict, reservationCount != transactionCount {
            throw DecodingError.dataCorruptedError(
                forKey: .reservationCount, in: container,
                debugDescription: "Autonomous reservation and transaction counts disagree"
            )
        }
        guard (proposalHash == nil) == (descriptorHash == nil) else {
            throw DecodingError.dataCorruptedError(
                forKey: .proposalHash, in: container,
                debugDescription: "Autonomous proposal and descriptor hashes must appear together"
            )
        }
        if highestDurableStage != .conflict,
           (highestDurableStage == .reservationsDurable) != (proposalHash == nil) {
            throw DecodingError.dataCorruptedError(
                forKey: .proposalHash, in: container,
                debugDescription: "Autonomous finalized identity disagrees with durable stage"
            )
        }
        if highestDurableStage == .reservationsDurable, proposalView != nil {
            throw DecodingError.dataCorruptedError(
                forKey: .proposalView, in: container,
                debugDescription: "Autonomous proposal view disagrees with durable stage"
            )
        }
        guard highestDurableStage == .conflict || evidenceGeometryMatches else {
            throw DecodingError.dataCorruptedError(
                forKey: .highestDurableStage, in: container,
                debugDescription: "Autonomous evidence does not match durable stage"
            )
        }
    }

    private var evidenceGeometryMatches: Bool {
        let hasPayload = executablePayloadHash != nil
        let hasBundle = sourceBundleHash != nil
        let hasMerge = mergeEntryHash != nil
        let hasCarrier = applicationBlockHeight != nil
        switch highestDurableStage {
        case .reservationsDurable:
            return !hasPayload && !hasBundle && !hasMerge && !hasCarrier
        case .executablePayloadDurable, .payloadAvailabilityCertified, .laneCertified:
            return hasPayload && !hasBundle && !hasMerge && !hasCarrier
        case .certifiedBundleDurable:
            return hasPayload && hasBundle && !hasMerge && !hasCarrier
        case .mergeCandidateDurable, .globalCarrierCommitted:
            return hasPayload && hasBundle && hasMerge && !hasCarrier
        case .kuraWsvApplicationReceiptDurable, .queueFinalized:
            return hasPayload && hasBundle && hasMerge && hasCarrier
        case .conflict:
            return true
        }
    }
}

/// Non-authoritative operator and lane diagnostics returned by `/v1/sumeragi/diagnostics`.
public struct ToriiSumeragiDiagnosticsSnapshot: Decodable, Sendable {
    private static let maximumNativeAmxParticipantApplications = 1_024
    private static let maximumAutonomousLaneExecutions = 128

    public let pipelineExecution: ToriiJSONValue
    public let txQueueDepth: UInt64
    public let txQueueCapacity: UInt64
    public let txQueueRetainedBytes: UInt64
    public let txQueueMaxRetainedBytes: UInt64
    public let txQueueSaturated: Bool
    public let txQueueSaturatedByCount: Bool
    public let txQueueSaturatedByBytes: Bool
    public let txQueueSaturatedByAge: Bool
    public let txQueueOldestQueuedAgeMs: UInt64
    public let npos: ToriiJSONValue?
    public let laneCommitments: [ToriiLaneCommitmentSnapshot]
    public let dataspaceCommitments: [ToriiDataspaceCommitmentSnapshot]
    public let laneSettlementCommitments: [ToriiLaneSettlementCommitment]
    public let laneRelayEnvelopes: [ToriiLaneRelayEnvelope]
    public let lanePayloadOwnerships: [ToriiJSONValue]
    public let committedLaneBlocks: [ToriiJSONValue]
    public let laneBlockSessions: [ToriiJSONValue]
    public let laneGovernanceSealedTotal: UInt32
    public let laneGovernanceSealedAliases: [String]
    public let laneGovernance: [ToriiJSONValue]
    public let nativeAmxParticipantApplications:
        [ToriiSumeragiNativeAmxParticipantApplication]
    public let autonomousLaneExecutions: [ToriiSumeragiAutonomousLaneExecution]
    /// Original diagnostics fields, including future-neutral typed subtrees.
    public let fields: [String: ToriiJSONValue]

    private static let allowedFields: Set<String> = [
        "pipeline_execution", "tx_queue_depth", "tx_queue_capacity",
        "tx_queue_retained_bytes", "tx_queue_max_retained_bytes", "tx_queue_saturated",
        "tx_queue_saturated_by_count", "tx_queue_saturated_by_bytes",
        "tx_queue_saturated_by_age", "tx_queue_oldest_queued_age_ms", "npos",
        "lane_commitments", "dataspace_commitments", "lane_settlement_commitments",
        "lane_relay_envelopes", "lane_payload_ownerships", "committed_lane_blocks",
        "lane_block_sessions", "lane_governance_sealed_total",
        "lane_governance_sealed_aliases", "lane_governance",
        "native_amx_participant_applications",
        "autonomous_lane_executions",
    ]

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        let raw = try container.decode([String: ToriiJSONValue].self)
        if let unknown = raw.keys.first(where: { !Self.allowedFields.contains($0) }) {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: decoder.codingPath,
                    debugDescription: "unknown Sumeragi diagnostics field \(unknown)"
                )
            )
        }
        self.fields = raw

        func requiredValue(_ key: String) throws -> ToriiJSONValue {
            guard let value = raw[key] else {
                throw DecodingError.keyNotFound(
                    ToriiSumeragiV2DynamicCodingKey(stringValue: key)!,
                    .init(codingPath: decoder.codingPath, debugDescription: "missing \(key)")
                )
            }
            if case .null = value {
                throw DecodingError.valueNotFound(
                    ToriiJSONValue.self,
                    .init(codingPath: decoder.codingPath, debugDescription: "\(key) must not be null")
                )
            }
            return value
        }
        func decode<T: Decodable>(_ type: T.Type, _ key: String) throws -> T {
            let value = try requiredValue(key)
            do {
                return try JSONDecoder().decode(type, from: JSONEncoder().encode(value))
            } catch {
                throw DecodingError.dataCorrupted(
                    .init(
                        codingPath: decoder.codingPath,
                        debugDescription: "invalid Sumeragi diagnostics field \(key): \(error)"
                    )
                )
            }
        }

        self.pipelineExecution = try requiredValue("pipeline_execution")
        self.txQueueDepth = try decode(UInt64.self, "tx_queue_depth")
        self.txQueueCapacity = try decode(UInt64.self, "tx_queue_capacity")
        self.txQueueRetainedBytes = try decode(UInt64.self, "tx_queue_retained_bytes")
        self.txQueueMaxRetainedBytes = try decode(UInt64.self, "tx_queue_max_retained_bytes")
        self.txQueueSaturated = try decode(Bool.self, "tx_queue_saturated")
        self.txQueueSaturatedByCount = try decode(Bool.self, "tx_queue_saturated_by_count")
        self.txQueueSaturatedByBytes = try decode(Bool.self, "tx_queue_saturated_by_bytes")
        self.txQueueSaturatedByAge = try decode(Bool.self, "tx_queue_saturated_by_age")
        self.txQueueOldestQueuedAgeMs = try decode(UInt64.self, "tx_queue_oldest_queued_age_ms")
        if let npos = raw["npos"], case .null = npos {
            self.npos = nil
        } else {
            self.npos = raw["npos"]
        }
        self.laneCommitments = try decode([ToriiLaneCommitmentSnapshot].self, "lane_commitments")
        self.dataspaceCommitments = try decode(
            [ToriiDataspaceCommitmentSnapshot].self,
            "dataspace_commitments"
        )
        self.laneSettlementCommitments = try decode(
            [ToriiLaneSettlementCommitment].self,
            "lane_settlement_commitments"
        )
        self.laneRelayEnvelopes = try decode(
            [ToriiLaneRelayEnvelope].self,
            "lane_relay_envelopes"
        )
        self.lanePayloadOwnerships = try decode([ToriiJSONValue].self, "lane_payload_ownerships")
        self.committedLaneBlocks = try decode([ToriiJSONValue].self, "committed_lane_blocks")
        self.laneBlockSessions = try decode([ToriiJSONValue].self, "lane_block_sessions")
        self.laneGovernanceSealedTotal = try decode(UInt32.self, "lane_governance_sealed_total")
        self.laneGovernanceSealedAliases = try decode(
            [String].self,
            "lane_governance_sealed_aliases"
        )
        self.laneGovernance = try decode([ToriiJSONValue].self, "lane_governance")
        self.nativeAmxParticipantApplications = try decode(
            [ToriiSumeragiNativeAmxParticipantApplication].self,
            "native_amx_participant_applications"
        )
        self.autonomousLaneExecutions = try decode(
            [ToriiSumeragiAutonomousLaneExecution].self,
            "autonomous_lane_executions"
        )
        guard autonomousLaneExecutions.count <= Self.maximumAutonomousLaneExecutions else {
            throw DecodingError.dataCorrupted(
                .init(codingPath: decoder.codingPath,
                      debugDescription: "Autonomous lane diagnostics exceed the 128-row limit")
            )
        }
        var previousAutonomousKey: String?
        for row in autonomousLaneExecutions {
            let key = String(format: "%010u:%020llu:%@:%020llu:%020llu:%020llu:%@",
                             row.laneID, row.dataspaceID, row.laneIncarnation,
                             row.laneBlockHeight, row.laneBlockView, row.proposalHeight,
                             row.proposalIdentityHash)
            guard previousAutonomousKey.map({ $0 < key }) ?? true else {
                throw DecodingError.dataCorrupted(
                    .init(codingPath: decoder.codingPath,
                          debugDescription: "Autonomous lane diagnostics must be strictly ordered")
                )
            }
            previousAutonomousKey = key
        }
        guard nativeAmxParticipantApplications.count
                <= Self.maximumNativeAmxParticipantApplications else {
            throw DecodingError.dataCorrupted(
                .init(
                    codingPath: decoder.codingPath,
                    debugDescription:
                        "Native AMX participant diagnostics exceed the 1024-row limit"
                )
            )
        }
        var previousRoute: (UInt32, UInt64, String)?
        for row in nativeAmxParticipantApplications {
            let route = (row.laneID, row.dataspaceID, row.laneIncarnation)
            if let previousRoute {
                let ordered = previousRoute.0 < route.0
                    || (previousRoute.0 == route.0 && previousRoute.1 < route.1)
                    || (previousRoute.0 == route.0
                        && previousRoute.1 == route.1
                        && previousRoute.2 < route.2)
                guard ordered else {
                    throw DecodingError.dataCorrupted(
                        .init(
                            codingPath: decoder.codingPath,
                            debugDescription:
                                "Native AMX participant diagnostics must be strictly ordered by route and incarnation"
                        )
                    )
                }
            }
            previousRoute = route
        }
    }

    public subscript(field name: String) -> ToriiJSONValue? {
        fields[name]
    }
}

private struct ToriiSumeragiV2DynamicCodingKey: CodingKey {
    let stringValue: String
    let intValue: Int?

    init?(stringValue: String) {
        self.stringValue = stringValue
        self.intValue = nil
    }

    init?(intValue: Int) {
        self.stringValue = String(intValue)
        self.intValue = intValue
    }
}
