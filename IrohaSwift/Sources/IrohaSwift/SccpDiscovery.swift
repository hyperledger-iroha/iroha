import CryptoKit
import Foundation

private let maximumUInt128 = "340282366920938463463374607431768211455"
private let maximumTonCoins = "1329227995784915872903807060280344575"
private let keccak256EmptyBytes = Data(hexString: "C5D2460186F7233C927E7DB2DCC703C0E500B653CA82273B7BFAD8045D85A470")!

/// Fixed SCCP V1 route-registry capacity limits.
public struct SccpRegistryLimits: Equatable, Sendable {
    public let maxGovernedLanes: UInt32
    public let maxLiveGovernedRoutes: UInt32
    public let maxLiveRoutesPerLane: UInt32
    public let maxRetainedRoutesPerLane: UInt32
    public let maxRetainedNativeTrustAnchorsPerLane: UInt32
}

/// Consensus-critical SCCP proof and deterministic verifier-work limits.
public struct SccpResourceLimits: Equatable, Sendable {
    public let maxOutboundMessagesPerBlock: UInt32
    public let maxOutboundMessagePayloadBytes: UInt64
    public let maxPendingOutboundMessages: UInt64
    public let maxPendingOutboundPayloadBytes: UInt64
    public let maxProofsPerTransaction: UInt32
    public let maxProofsPerBlock: UInt32
    public let maxProofBytesPerProof: UInt64
    public let maxProofBytesPerTransaction: UInt64
    public let maxProofBytesPerBlock: UInt64
    public let maxNativeHeadersPerTransaction: UInt32
    public let maxNativeHeadersPerBlock: UInt32
    public let maxEthereumLightClientUpdatesPerTransaction: UInt32
    public let maxEthereumLightClientUpdatesPerBlock: UInt32
    public let maxNativeHeaderBytesPerTransaction: UInt64
    public let maxNativeHeaderBytesPerBlock: UInt64
    public let maxSecp256k1RecoveriesPerTransaction: UInt32
    public let maxSecp256k1RecoveriesPerBlock: UInt32
    public let maxBlsAggregateChecksPerTransaction: UInt32
    public let maxBlsAggregateChecksPerBlock: UInt32
    public let maxBlsSignerContributionsPerTransaction: UInt32
    public let maxBlsSignerContributionsPerBlock: UInt32
    public let maxEd25519SignatureChecksPerTransaction: UInt32
    public let maxEd25519SignatureChecksPerBlock: UInt32
    public let maxEd25519ValidatorKeyChecksPerTransaction: UInt32
    public let maxEd25519ValidatorKeyChecksPerBlock: UInt32
    public let maxBn254PairingChecksPerTransaction: UInt32
    public let maxBn254PairingChecksPerBlock: UInt32
    public let maxBls12381PairingChecksPerTransaction: UInt32
    public let maxBls12381PairingChecksPerBlock: UInt32
}

/// Stable first-release SCCP HTTP surface. Every path is fixed and query-free except recent-message pagination.
public struct SccpCapabilities: Equatable, Sendable {
    public let version: UInt8
    public let registryRevision: String
    public let registryPath: String
    public let messageBundlePath: String
    public let proofRequestPath: String
    public let recentMessagesPath: String
    public let soraOutboundMaterialPath: String
    public let registryLimits: SccpRegistryLimits
    public let resourceLimits: SccpResourceLimits
    public let proofSubmitPath: String?
    public let nativeMessageSubmitPath: String?

    public static func parse(_ data: Data) throws -> Self {
        try SccpExactParser.capabilities(data)
    }
}

/// Closed destination proof backend selected by governed route state.
public enum SccpDestinationProofBackendV1: String, CaseIterable, Sendable {
    case evmGroth16Bn254 = "evm_groth16_bn254_v1"
    case tronGroth16Bn254 = "tron_groth16_bn254_v1"
    case tonGroth16Bls12381 = "ton_groth16_bls12381_v1"
}

/// Closed directional activation state of an immutable route revision.
public enum SccpRouteActivationV1: String, CaseIterable, Sendable {
    case staged
    case bidirectional
    case inboundOnly = "inbound_only"
    case paused
    case retired

    public var allowsOutbound: Bool { self == .bidirectional }
    public var allowsInbound: Bool { self == .bidirectional || self == .inboundOnly }
}

/// Governed native checkpoint shared by every route in one exact inbound lane.
public struct SccpNativeTrustAnchorV1: Equatable, Sendable {
    public let backend: SccpNativeBackendV1
    public let anchorHash: Data
    public let checkpointHeight: UInt64
}

/// Complete retained checkpoint interval admitted for delayed claims on a retired route.
public struct SccpInboundFinalityCutoffV1: Equatable, Sendable {
    public let trustAnchorHash: Data
    public let maxAnchorIntervalHeight: UInt64
}

/// Governance-registered portable verification-key identity for SORA-side execution proofs.
public struct SccpPortableVerifyingKeyReferenceV1: Equatable, Sendable {
    public let backend: String
    public let name: String
    public let version: UInt32
    public let commitment: Data
}

/// Mandatory SORA-side proved burn-and-record execution policy for one route.
public struct SccpSoraOutboundExecutionPolicyV1: Equatable, Sendable {
    public let version: UInt8
    public let semantics: String
    public let contractArtifactSha256: Data
    public let verifyingKeyReference: SccpPortableVerifyingKeyReferenceV1
    public let gasLimit: UInt64
}

/// Closed curve-specific semantic circuit admitted by SCCP V1.
public enum SccpSemanticProofProfileKindV1: String, Sendable {
    case groth16Bn254 = "sora_taira_finality_inclusion_groth16_bn254"
    case groth16Bls12381 = "sora_taira_finality_inclusion_groth16_bls12381"
}

/// Immutable commitments identifying one audited semantic circuit.
public struct SccpSemanticProofProfileV1: Equatable, Sendable {
    public let kind: SccpSemanticProofProfileKindV1
    public let circuitCommitment: Data
    public let witnessGeneratorCommitment: Data
    public let publicSignalSchemaHash: Data
    public let profileHash: Data
}

/// Immutable Taira finality checkpoint exposed as Groth16 public signal 10.
public struct SccpSoraFinalityAnchorV1: Equatable, Sendable {
    public let protocolVersion: UInt16
    public let chainIdHash: Data
    public let checkpointHeight: UInt64
    public let checkpointBlockHash: Data
    public let checkpointContextId: Data
    public let checkpointFinalityArtifactHash: Data
    public let anchorHash: Data
}

/// Required typed proof policy bound into every EVM/TRON destination deployment.
public struct SccpOutboundProofPolicyV1: Equatable, Sendable {
    public let version: UInt8
    public let semanticProfile: SccpSemanticProofProfileV1
    public let soraFinalityAnchor: SccpSoraFinalityAnchorV1
}

/// Exact EVM/TRON destination deployment. The full 38-word BN254 key is validated before construction.
public struct SccpEvmTronDestinationDeploymentV1: Equatable, Sendable {
    public let tokenAddress: Data
    public let tokenCodeHash: Data
    public let verifierAddress: Data
    public let verifierCodeHash: Data
    public let verifierKeyHash: Data
    public let outboundProofPolicy: SccpOutboundProofPolicyV1
    public let routeAddress: Data
    public let routeCodeHash: Data
    public let replayVerifierAddress: Data
    public let replayVerifierCodeHash: Data
    public let mintBreakerAddress: Data
    public let mintBreakerCodeHash: Data
    public let tairaToTokenMultiplier: UInt64
    public let maxWrappedSupply: String
    public let destinationBindingHash: Data
}

/// Exact ordered five-key TON mint-breaker guardian set.
public struct SccpTonMintBreakerGuardianKeysV1: Equatable, Sendable {
    public let guardian0: Data
    public let guardian1: Data
    public let guardian2: Data
    public let guardian3: Data
    public let guardian4: Data

    public init(
        guardian0: Data,
        guardian1: Data,
        guardian2: Data,
        guardian3: Data,
        guardian4: Data
    ) throws {
        let keys = [guardian0, guardian1, guardian2, guardian3, guardian4]
        guard keys.allSatisfy({ $0.count == 32 && $0.contains(where: { $0 != 0 }) }),
              zip(keys, keys.dropFirst()).allSatisfy({ $0.lexicographicallyPrecedes($1) })
        else {
            throw SccpV1Error.invalid(
                "TON mint-breaker guardians must be five nonzero, strictly increasing 32-byte keys"
            )
        }
        self.guardian0 = Data(guardian0)
        self.guardian1 = Data(guardian1)
        self.guardian2 = Data(guardian2)
        self.guardian3 = Data(guardian3)
        self.guardian4 = Data(guardian4)
    }

    /// Keys in canonical TON StateInit and SCCP hash-preimage order.
    public var ordered: [Data] { [guardian0, guardian1, guardian2, guardian3, guardian4] }
}

/// Exact TON Jetton route with an embedded BLS12-381 Groth16 verifier.
public struct SccpTonDestinationDeploymentV1: Equatable, Sendable {
    public let jettonMasterAddress: SccpTonAddressV1
    public let jettonMasterCodeHash: Data
    public let jettonMasterInitialDataHash: Data
    public let jettonWalletCodeHash: Data
    public let routeAddress: SccpTonAddressV1
    public let routeCodeHash: Data
    public let routeInitialDataHash: Data
    public let embeddedVerifierCodeHash: Data
    public let verifierCircuitHash: Data
    public let verifierKeyHash: Data
    public let proofProfileCommitment: Data
    public let mintBreakerGuardianKeys: SccpTonMintBreakerGuardianKeysV1
    public let outboundProofPolicy: SccpOutboundProofPolicyV1
    public let tairaToTokenMultiplier: UInt64
    public let maxWrappedSupply: String
    public let destinationBindingHash: Data

    /// Validate a positive outbound Jetton amount against the immutable route cap.
    public func validateJettonAmount(_ amount: String) throws {
        guard !amount.isEmpty,
              amount.utf8.allSatisfy({ (48...57).contains($0) }),
              amount.first != "0",
              amount.count < maxWrappedSupply.count
                  || amount.count == maxWrappedSupply.count && amount <= maxWrappedSupply
        else {
            throw SccpV1Error.invalid("TON Jetton amount must be positive and no greater than max_wrapped_supply")
        }
    }
}

/// Closed, family-specific destination deployment.
public enum SccpDestinationDeploymentV1: Equatable, Sendable {
    case evm(SccpEvmTronDestinationDeploymentV1)
    case tron(SccpEvmTronDestinationDeploymentV1)
    case ton(SccpTonDestinationDeploymentV1)

    public var family: SccpDestinationProofBackendV1 {
        switch self {
        case .evm: .evmGroth16Bn254
        case .tron: .tronGroth16Bn254
        case .ton: .tonGroth16Bls12381
        }
    }

    public var verifierKeyHash: Data {
        switch self {
        case let .evm(value), let .tron(value): value.verifierKeyHash
        case let .ton(value): value.verifierKeyHash
        }
    }

    public var outboundProofPolicy: SccpOutboundProofPolicyV1 {
        switch self {
        case let .evm(value), let .tron(value): value.outboundProofPolicy
        case let .ton(value): value.outboundProofPolicy
        }
    }

    public var routeCodeHash: Data {
        switch self {
        case let .evm(value), let .tron(value): value.routeCodeHash
        case let .ton(value): value.routeCodeHash
        }
    }

    public var destinationBindingHash: Data {
        switch self {
        case let .evm(value), let .tron(value): value.destinationBindingHash
        case let .ton(value): value.destinationBindingHash
        }
    }

    public var tairaToTokenMultiplier: UInt64 {
        switch self {
        case let .evm(value), let .tron(value): value.tairaToTokenMultiplier
        case let .ton(value): value.tairaToTokenMultiplier
        }
    }

    public var maxWrappedSupply: String {
        switch self {
        case let .evm(value), let .tron(value): value.maxWrappedSupply
        case let .ton(value): value.maxWrappedSupply
        }
    }
}

/// One complete immutable route revision from the consensus registry.
public struct SccpGovernedRouteV1: Equatable, Sendable {
    public let lane: SccpLaneIdV1
    public let routeId: String
    public let assetKey: String
    public let revision: UInt32
    public let activation: SccpRouteActivationV1
    public let inboundFinalityCutoff: SccpInboundFinalityCutoffV1?
    public let sourceEmitter: SccpSourceEmitterV1
    public let destination: SccpDestinationDeploymentV1
    public let soraOutboundExecutionPolicy: SccpSoraOutboundExecutionPolicyV1
    public let assetDefinitionId: String
    public let payloadAmountScale: UInt32
    public let maxOutstandingLiability: String
    public let routeConfigurationHash: Data
}

/// One exact external-to-Taira lane and all immutable route revisions governed beneath it.
public struct SccpGovernedLaneV1: Equatable, Sendable {
    public let lane: SccpLaneIdV1
    public let nativeTrustAnchors: [SccpNativeTrustAnchorV1]
    public let currentNativeTrustAnchorHash: Data?
    public let routes: [SccpGovernedRouteV1]
}

/// Authoritative typed SCCP registry returned by `GET /v1/sccp/registry`.
public struct SccpRegistryV1: Equatable, Sendable {
    public let version: UInt8
    public let lanes: [SccpGovernedLaneV1]
    public let rawJSON: Data

    public static func parse(_ data: Data) throws -> Self {
        try SccpExactParser.registry(data)
    }
}

/// Exact JSON representation of a finalized SCCP bundle.
public struct SccpMessageBundleV1: Equatable, Sendable {
    public let version: UInt8
    public let commitmentRoot: String
    public let messageId: String
    public let targetDomain: UInt32
    public let rawJSON: Data

    public static func parse(_ data: Data) throws -> Self {
        try SccpExactParser.bundle(data)
    }
}

/// Exact state-derived Groth16 request returned by `GET /v1/sccp/proof-requests/{message_id}`.
public struct SccpGroth16Bls12381PublicSignalsV1: Equatable, Sendable {
    public let messageId: String
    public let payloadHash: String
    public let targetDomain: String
    public let commitmentRoot: String
    public let finalityHeight: String
    public let finalityBlockHash: String
    public let sourceDomain: String
    public let statementHash: String
    public let destinationBindingHash: String
    public let routeConfigurationHash: String
    public let soraFinalityAnchorHash: String
}

/// Exact state-derived Groth16 request returned by `GET /v1/sccp/proof-requests/{message_id}`.
public struct SccpGroth16ProofRequestV1: Equatable, Sendable {
    public let version: UInt8
    public let backend: SccpDestinationProofBackendV1
    public let sourceNetwork: SccpNetworkV1
    public let targetNetwork: SccpNetworkV1
    public let messageId: String
    public let payloadHash: String
    public let targetDomain: UInt32
    public let commitmentRoot: String
    public let finalityHeight: UInt64
    public let finalityBlockHash: String
    public let publicSignals: SccpGroth16Bls12381PublicSignalsV1?
    public let verifierKeyHash: String
    public let verifierCircuitHash: String?
    public let proofProfileCommitment: String?
    public let semanticProofProfile: SccpSemanticProofProfileV1
    public let semanticProofProfileHash: String
    public let soraFinalityAnchor: SccpSoraFinalityAnchorV1
    public let soraFinalityAnchorHash: String
    public let statementHash: String
    public let destinationBindingHash: String
    public let routeConfigurationHash: String
    public let requestHash: String
    public let bundleBytes: Data
    public let rawJSON: Data

    public static func parse(_ data: Data) throws -> Self {
        try SccpExactParser.proofRequest(data)
    }
}

public struct SccpRecentMessageLinks: Equatable, Sendable {
    public let bundlePath: String
    public let proofRequestPath: String
}

public struct SccpRecentMessage: Equatable, Sendable {
    public let height: UInt64
    public let commitmentIndex: UInt32
    public let messageIdHex: String
    public let kind: SccpPayloadKindV1
    public let lane: SccpLaneIdV1
    public let destinationBindingHash: String
    public let routeConfigurationHash: String
    public let assetId: String?
    public let routeId: String?
    public let recipient: String?
    public let amount: String
    public let payloadProjectionJSON: Data
    public let links: SccpRecentMessageLinks
}

/// Exact compound continuation for newest-first SCCP discovery.
public struct SccpRecentCursor: Equatable, Sendable {
    public let from: UInt64
    public let afterIndex: UInt32
}

public struct SccpRecentMessages: Equatable, Sendable {
    public let items: [SccpRecentMessage]
    public let next: SccpRecentCursor?

    public static func parse(_ data: Data) throws -> Self {
        try SccpExactParser.recent(data)
    }
}

enum SccpExactParser {
    private static let jsonSafeIntegerMaximum: UInt64 = (1 << 53) - 1

    private static let capabilityPaths: [String: String] = [
        "registry_path": "/v1/sccp/registry",
        "message_bundle_path": "/v1/sccp/proofs/message/{message_id}",
        "proof_request_path": "/v1/sccp/proof-requests/{message_id}",
        "recent_messages_path": "/v1/sccp/messages/recent",
        "sora_outbound_material_path": "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material",
        "proof_submit_path": "/v1/bridge/proofs/submit",
        "native_message_submit_path": "/v1/bridge/messages",
    ]
    private static let routeKeyCharacters = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyz0123456789_-")
    private static let bn254BaseField = Data([
        0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
        0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
    ])
    private static let bls12381BaseField = Data([
        0x1a, 0x01, 0x11, 0xea, 0x39, 0x7f, 0xe6, 0x9a, 0x4b, 0x1b, 0xa7, 0xb6, 0x43, 0x4b, 0xac, 0xd7,
        0x64, 0x77, 0x4b, 0x84, 0xf3, 0x85, 0x12, 0xbf, 0x67, 0x30, 0xd2, 0xa0, 0xf6, 0xb0, 0xf6, 0x24,
        0x1e, 0xab, 0xff, 0xfe, 0xb1, 0x53, 0xff, 0xff, 0xb9, 0xfe, 0xff, 0xff, 0xff, 0xff, 0xaa, 0xab,
    ])
    private static let bls12381ScalarField = Data([
        0x73, 0xed, 0xa7, 0x53, 0x29, 0x9d, 0x7d, 0x48, 0x33, 0x39, 0xd8, 0x08, 0x09, 0xa1, 0xd8, 0x05,
        0x53, 0xbd, 0xa4, 0x02, 0xff, 0xfe, 0x5b, 0xfe, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x01,
    ])
    private static let tairaChainId = Data([
        0xfc, 0x56, 0x98, 0x4b, 0x2b, 0xe7, 0x43, 0x1d, 0x84, 0x0e, 0x21, 0x51, 0x4d, 0x18, 0x83, 0xf0,
    ])
    private static let publicSignalLabels = [
        "sccp:groth16-bn254:signal:message-id:v1",
        "sccp:groth16-bn254:signal:payload-hash:v1",
        "sccp:groth16-bn254:signal:target-domain:v1",
        "sccp:groth16-bn254:signal:commitment-root:v1",
        "sccp:groth16-bn254:signal:finality-height:v1",
        "sccp:groth16-bn254:signal:finality-block-hash:v1",
        "sccp:groth16-bn254:signal:source-domain:v1",
        "sccp:groth16-bn254:signal:statement-hash:v1",
        "sccp:groth16-bn254:signal:destination-binding-hash:v1",
        "sccp:groth16-bn254:signal:route-configuration-hash:v1",
        "sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1",
    ]
    private static let bls12381PublicSignalLabels = [
        "sccp:groth16-bls12381:signal:message-id:v1",
        "sccp:groth16-bls12381:signal:payload-hash:v1",
        "sccp:groth16-bls12381:signal:target-domain:v1",
        "sccp:groth16-bls12381:signal:commitment-root:v1",
        "sccp:groth16-bls12381:signal:finality-height:v1",
        "sccp:groth16-bls12381:signal:finality-block-hash:v1",
        "sccp:groth16-bls12381:signal:source-domain:v1",
        "sccp:groth16-bls12381:signal:statement-hash:v1",
        "sccp:groth16-bls12381:signal:destination-binding-hash:v1",
        "sccp:groth16-bls12381:signal:route-config-hash:v1",
        "sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
    ]

    static func capabilities(_ data: Data) throws -> SccpCapabilities {
        let root = try SccpStrictJSON.object(data, label: "SCCP capabilities")
        let required: Set<String> = [
            "version", "registry_revision", "registry_path", "message_bundle_path",
            "proof_request_path", "recent_messages_path", "sora_outbound_material_path",
            "registry_limits", "resource_limits",
        ]
        let allowed = required.union(["proof_submit_path", "native_message_submit_path"])
        try SccpStrictJSON.exactFields(root, allowed: allowed, required: required, label: "SCCP capabilities")
        guard try SccpStrictJSON.uint64(root, "version", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("SCCP capability version must be exactly 1")
        }
        let revision = try prefixedHash(root, "registry_revision")
        let registryPath = try fixedPath(root, "registry_path")
        let bundlePath = try fixedPath(root, "message_bundle_path")
        let proofRequestPath = try fixedPath(root, "proof_request_path")
        let recentPath = try fixedPath(root, "recent_messages_path")
        let outboundMaterialPath = try fixedPath(root, "sora_outbound_material_path")
        let registryLimits = try registryLimits(object(root, "registry_limits"))
        let resourceLimits = try resourceLimits(object(root, "resource_limits"))
        let proofSubmitPath = try optionalFixedPath(root, "proof_submit_path")
        let nativePath = try optionalFixedPath(root, "native_message_submit_path")
        guard (proofSubmitPath == nil) == (nativePath == nil) else {
            throw SccpV1Error.invalid("SCCP submission capability paths must be advertised together")
        }
        return SccpCapabilities(
            version: 1,
            registryRevision: revision,
            registryPath: registryPath,
            messageBundlePath: bundlePath,
            proofRequestPath: proofRequestPath,
            recentMessagesPath: recentPath,
            soraOutboundMaterialPath: outboundMaterialPath,
            registryLimits: registryLimits,
            resourceLimits: resourceLimits,
            proofSubmitPath: proofSubmitPath,
            nativeMessageSubmitPath: nativePath
        )
    }

    private static func registryLimits(_ value: [String: Any]) throws -> SccpRegistryLimits {
        let fields: Set<String> = [
            "max_governed_lanes", "max_live_governed_routes", "max_live_routes_per_lane",
            "max_retained_routes_per_lane", "max_retained_native_trust_anchors_per_lane",
        ]
        try SccpStrictJSON.exactFields(value, fields, label: "SCCP registry limits")
        let result = SccpRegistryLimits(
            maxGovernedLanes: try SccpStrictJSON.uint32(
                value, "max_governed_lanes", minimum: 1, maximum: UInt32.max
            ),
            maxLiveGovernedRoutes: try SccpStrictJSON.uint32(
                value, "max_live_governed_routes", minimum: 1, maximum: UInt32.max
            ),
            maxLiveRoutesPerLane: try SccpStrictJSON.uint32(
                value, "max_live_routes_per_lane", minimum: 1, maximum: UInt32.max
            ),
            maxRetainedRoutesPerLane: try SccpStrictJSON.uint32(
                value, "max_retained_routes_per_lane", minimum: 1, maximum: UInt32.max
            ),
            maxRetainedNativeTrustAnchorsPerLane: try SccpStrictJSON.uint32(
                value,
                "max_retained_native_trust_anchors_per_lane",
                minimum: 1,
                maximum: UInt32.max
            )
        )
        guard result == SccpRegistryLimits(
            maxGovernedLanes: 16,
            maxLiveGovernedRoutes: 64,
            maxLiveRoutesPerLane: 8,
            maxRetainedRoutesPerLane: 64,
            maxRetainedNativeTrustAnchorsPerLane: 4_096
        ) else {
            throw SccpV1Error.invalid("SCCP registry limits must equal the fixed V1 capacities")
        }
        return result
    }

    private static func resourceLimits(_ value: [String: Any]) throws -> SccpResourceLimits {
        let fields: Set<String> = [
            "max_outbound_messages_per_block", "max_outbound_message_payload_bytes",
            "max_pending_outbound_messages", "max_pending_outbound_payload_bytes",
            "max_proofs_per_transaction", "max_proofs_per_block", "max_proof_bytes_per_proof",
            "max_proof_bytes_per_transaction", "max_proof_bytes_per_block",
            "max_native_headers_per_transaction", "max_native_headers_per_block",
            "max_ethereum_light_client_updates_per_transaction",
            "max_ethereum_light_client_updates_per_block",
            "max_native_header_bytes_per_transaction", "max_native_header_bytes_per_block",
            "max_secp256k1_recoveries_per_transaction", "max_secp256k1_recoveries_per_block",
            "max_bls_aggregate_checks_per_transaction", "max_bls_aggregate_checks_per_block",
            "max_bls_signer_contributions_per_transaction",
            "max_bls_signer_contributions_per_block",
            "max_ed25519_signature_checks_per_transaction",
            "max_ed25519_signature_checks_per_block",
            "max_ed25519_validator_key_checks_per_transaction",
            "max_ed25519_validator_key_checks_per_block",
            "max_bn254_pairing_checks_per_transaction", "max_bn254_pairing_checks_per_block",
            "max_bls12_381_pairing_checks_per_transaction",
            "max_bls12_381_pairing_checks_per_block",
        ]
        try SccpStrictJSON.exactFields(value, fields, label: "SCCP resource limits")
        let result = SccpResourceLimits(
            maxOutboundMessagesPerBlock: try SccpStrictJSON.uint32(
                value, "max_outbound_messages_per_block", minimum: 1, maximum: UInt32.max
            ),
            maxOutboundMessagePayloadBytes: try SccpStrictJSON.uint64(
                value,
                "max_outbound_message_payload_bytes",
                minimum: 1,
                maximum: jsonSafeIntegerMaximum
            ),
            maxPendingOutboundMessages: try SccpStrictJSON.uint64(
                value,
                "max_pending_outbound_messages",
                minimum: 1,
                maximum: jsonSafeIntegerMaximum
            ),
            maxPendingOutboundPayloadBytes: try SccpStrictJSON.uint64(
                value,
                "max_pending_outbound_payload_bytes",
                minimum: 1,
                maximum: jsonSafeIntegerMaximum
            ),
            maxProofsPerTransaction: try SccpStrictJSON.uint32(
                value, "max_proofs_per_transaction", minimum: 1, maximum: UInt32.max
            ),
            maxProofsPerBlock: try SccpStrictJSON.uint32(
                value, "max_proofs_per_block", minimum: 1, maximum: UInt32.max
            ),
            maxProofBytesPerProof: try SccpStrictJSON.uint64(
                value,
                "max_proof_bytes_per_proof",
                minimum: 1,
                maximum: jsonSafeIntegerMaximum
            ),
            maxProofBytesPerTransaction: try SccpStrictJSON.uint64(
                value,
                "max_proof_bytes_per_transaction",
                minimum: 1,
                maximum: jsonSafeIntegerMaximum
            ),
            maxProofBytesPerBlock: try SccpStrictJSON.uint64(
                value,
                "max_proof_bytes_per_block",
                minimum: 1,
                maximum: jsonSafeIntegerMaximum
            ),
            maxNativeHeadersPerTransaction: try SccpStrictJSON.uint32(
                value, "max_native_headers_per_transaction", minimum: 1, maximum: UInt32.max
            ),
            maxNativeHeadersPerBlock: try SccpStrictJSON.uint32(
                value, "max_native_headers_per_block", minimum: 1, maximum: UInt32.max
            ),
            maxEthereumLightClientUpdatesPerTransaction: try SccpStrictJSON.uint32(
                value,
                "max_ethereum_light_client_updates_per_transaction",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxEthereumLightClientUpdatesPerBlock: try SccpStrictJSON.uint32(
                value,
                "max_ethereum_light_client_updates_per_block",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxNativeHeaderBytesPerTransaction: try SccpStrictJSON.uint64(
                value,
                "max_native_header_bytes_per_transaction",
                minimum: 1,
                maximum: jsonSafeIntegerMaximum
            ),
            maxNativeHeaderBytesPerBlock: try SccpStrictJSON.uint64(
                value,
                "max_native_header_bytes_per_block",
                minimum: 1,
                maximum: jsonSafeIntegerMaximum
            ),
            maxSecp256k1RecoveriesPerTransaction: try SccpStrictJSON.uint32(
                value,
                "max_secp256k1_recoveries_per_transaction",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxSecp256k1RecoveriesPerBlock: try SccpStrictJSON.uint32(
                value, "max_secp256k1_recoveries_per_block", minimum: 1, maximum: UInt32.max
            ),
            maxBlsAggregateChecksPerTransaction: try SccpStrictJSON.uint32(
                value,
                "max_bls_aggregate_checks_per_transaction",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxBlsAggregateChecksPerBlock: try SccpStrictJSON.uint32(
                value, "max_bls_aggregate_checks_per_block", minimum: 1, maximum: UInt32.max
            ),
            maxBlsSignerContributionsPerTransaction: try SccpStrictJSON.uint32(
                value,
                "max_bls_signer_contributions_per_transaction",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxBlsSignerContributionsPerBlock: try SccpStrictJSON.uint32(
                value,
                "max_bls_signer_contributions_per_block",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxEd25519SignatureChecksPerTransaction: try SccpStrictJSON.uint32(
                value,
                "max_ed25519_signature_checks_per_transaction",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxEd25519SignatureChecksPerBlock: try SccpStrictJSON.uint32(
                value,
                "max_ed25519_signature_checks_per_block",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxEd25519ValidatorKeyChecksPerTransaction: try SccpStrictJSON.uint32(
                value,
                "max_ed25519_validator_key_checks_per_transaction",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxEd25519ValidatorKeyChecksPerBlock: try SccpStrictJSON.uint32(
                value,
                "max_ed25519_validator_key_checks_per_block",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxBn254PairingChecksPerTransaction: try SccpStrictJSON.uint32(
                value,
                "max_bn254_pairing_checks_per_transaction",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxBn254PairingChecksPerBlock: try SccpStrictJSON.uint32(
                value,
                "max_bn254_pairing_checks_per_block",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxBls12381PairingChecksPerTransaction: try SccpStrictJSON.uint32(
                value,
                "max_bls12_381_pairing_checks_per_transaction",
                minimum: 1,
                maximum: UInt32.max
            ),
            maxBls12381PairingChecksPerBlock: try SccpStrictJSON.uint32(
                value,
                "max_bls12_381_pairing_checks_per_block",
                minimum: 1,
                maximum: UInt32.max
            )
        )
        guard result.maxOutboundMessagesPerBlock == 512,
              result.maxOutboundMessagePayloadBytes == 4_096
        else {
            throw SccpV1Error.invalid(
                "SCCP fixed outbound message limits must equal 512 messages and 4,096 payload bytes"
            )
        }
        guard result.maxProofBytesPerProof <= result.maxProofBytesPerTransaction else {
            throw SccpV1Error.invalid("SCCP per-proof byte limit exceeds its transaction limit")
        }
        let orderedPairs: [(UInt64, UInt64)] = [
            (UInt64(result.maxProofsPerTransaction), UInt64(result.maxProofsPerBlock)),
            (result.maxProofBytesPerTransaction, result.maxProofBytesPerBlock),
            (UInt64(result.maxNativeHeadersPerTransaction), UInt64(result.maxNativeHeadersPerBlock)),
            (
                UInt64(result.maxEthereumLightClientUpdatesPerTransaction),
                UInt64(result.maxEthereumLightClientUpdatesPerBlock)
            ),
            (result.maxNativeHeaderBytesPerTransaction, result.maxNativeHeaderBytesPerBlock),
            (
                UInt64(result.maxSecp256k1RecoveriesPerTransaction),
                UInt64(result.maxSecp256k1RecoveriesPerBlock)
            ),
            (
                UInt64(result.maxBlsAggregateChecksPerTransaction),
                UInt64(result.maxBlsAggregateChecksPerBlock)
            ),
            (
                UInt64(result.maxBlsSignerContributionsPerTransaction),
                UInt64(result.maxBlsSignerContributionsPerBlock)
            ),
            (
                UInt64(result.maxEd25519SignatureChecksPerTransaction),
                UInt64(result.maxEd25519SignatureChecksPerBlock)
            ),
            (
                UInt64(result.maxEd25519ValidatorKeyChecksPerTransaction),
                UInt64(result.maxEd25519ValidatorKeyChecksPerBlock)
            ),
            (
                UInt64(result.maxBn254PairingChecksPerTransaction),
                UInt64(result.maxBn254PairingChecksPerBlock)
            ),
            (
                UInt64(result.maxBls12381PairingChecksPerTransaction),
                UInt64(result.maxBls12381PairingChecksPerBlock)
            ),
        ]
        guard orderedPairs.allSatisfy({ $0.0 <= $0.1 }) else {
            throw SccpV1Error.invalid("SCCP transaction resource limits must not exceed block limits")
        }
        return result
    }

    static func registry(_ data: Data) throws -> SccpRegistryV1 {
        let root = try SccpStrictJSON.object(data, label: "SCCP registry")
        try SccpStrictJSON.exactFields(root, ["version", "lanes"], label: "SCCP registry")
        guard try SccpStrictJSON.uint64(root, "version", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("SCCP registry version must be exactly 1")
        }
        let laneValues = try array(root, "lanes")
        guard laneValues.count <= 16 else { throw SccpV1Error.invalid("SCCP registry exceeds 16 lanes") }
        var lanes: [SccpGovernedLaneV1] = []
        var laneKeys = Set<String>()
        var routeKeys = Set<String>()
        var bindings = Set<Data>()
        var configurations = Set<Data>()
        var liveRouteCount = 0
        for (index, raw) in laneValues.enumerated() {
            let item = try object(raw, label: "lanes[\(index)]")
            try SccpStrictJSON.exactFields(item, [
                "lane_id", "native_trust_anchors", "current_native_trust_anchor_hash", "routes",
            ], label: "lanes[\(index)]")
            let lane = try inboundLane(object(item, "lane_id"), label: "lanes[\(index)].lane_id")
            let laneKey = "\(lane.source.rawValue)>\(lane.target.rawValue)"
            guard laneKeys.insert(laneKey).inserted else { throw SccpV1Error.invalid("SCCP registry contains a duplicate lane") }
            let anchorValues = try array(item, "native_trust_anchors")
            guard anchorValues.count <= 4_096 else {
                throw SccpV1Error.invalid("SCCP lane exceeds 4,096 retained native trust anchors")
            }
            var anchors: [SccpNativeTrustAnchorV1] = []
            var anchorHashes = Set<Data>()
            for (anchorIndex, rawAnchor) in anchorValues.enumerated() {
                let anchorLabel = "lanes[\(index)].native_trust_anchors[\(anchorIndex)]"
                guard let anchor = try nativeAnchor(rawAnchor, lane: lane, label: anchorLabel) else {
                    throw SccpV1Error.invalid("\(anchorLabel) must not be null")
                }
                guard anchorHashes.insert(anchor.anchorHash).inserted else {
                    throw SccpV1Error.invalid("lanes[\(index)] contains a duplicate native trust-anchor hash")
                }
                if let previous = anchors.last,
                   previous.backend != anchor.backend || anchor.checkpointHeight <= previous.checkpointHeight {
                    throw SccpV1Error.invalid("lanes[\(index)].native_trust_anchors must advance monotonically within one backend")
                }
                anchors.append(anchor)
            }
            let currentAnchorHash: Data?
            if item["current_native_trust_anchor_hash"] is NSNull {
                currentAnchorHash = nil
            } else {
                currentAnchorHash = try upperFixed(item, "current_native_trust_anchor_hash", bytes: 32)
            }
            guard currentAnchorHash == anchors.last?.anchorHash else {
                throw SccpV1Error.invalid("lanes[\(index)].current_native_trust_anchor_hash must name the last retained anchor")
            }
            let anchor = anchors.last
            let routeValues = try array(item, "routes")
            guard !routeValues.isEmpty else { throw SccpV1Error.invalid("SCCP lane must contain at least one route") }
            guard routeValues.count <= 64 else {
                throw SccpV1Error.invalid("SCCP lane exceeds 64 retained route revisions")
            }
            var routes: [SccpGovernedRouteV1] = []
            var laneLiveRouteCount = 0
            for (routeIndex, routeRaw) in routeValues.enumerated() {
                let route = try governedRoute(
                    object(routeRaw, label: "lanes[\(index)].routes[\(routeIndex)]"),
                    expectedLane: lane,
                    nativeAnchor: anchor,
                    label: "lanes[\(index)].routes[\(routeIndex)]"
                )
                let key = "\(laneKey)\0\(route.routeId)\0\(route.assetKey)\0\(route.revision)"
                guard routeKeys.insert(key).inserted else { throw SccpV1Error.invalid("SCCP registry contains a duplicate route key") }
                guard bindings.insert(route.destination.destinationBindingHash).inserted else { throw SccpV1Error.invalid("SCCP registry reuses a destination binding") }
                guard configurations.insert(route.routeConfigurationHash).inserted else { throw SccpV1Error.invalid("SCCP registry reuses a route-configuration hash") }
                if let cutoff = route.inboundFinalityCutoff {
                    guard let anchorIndex = anchors.firstIndex(where: { $0.anchorHash == cutoff.trustAnchorHash }),
                          anchors.indices.contains(anchorIndex + 1),
                          anchors[anchorIndex + 1].checkpointHeight == cutoff.maxAnchorIntervalHeight
                    else {
                        throw SccpV1Error.invalid("lanes[\(index)].routes[\(routeIndex)].inbound_finality_cutoff must close one complete retained anchor interval")
                    }
                }
                if route.activation != .retired {
                    laneLiveRouteCount += 1
                    liveRouteCount += 1
                }
                routes.append(route)
            }
            guard laneLiveRouteCount <= 8 else { throw SccpV1Error.invalid("SCCP lane exceeds 8 live routes") }
            guard liveRouteCount <= 64 else { throw SccpV1Error.invalid("SCCP registry exceeds 64 live routes") }
            try validateLineages(routes)
            lanes.append(SccpGovernedLaneV1(
                lane: lane,
                nativeTrustAnchors: anchors,
                currentNativeTrustAnchorHash: currentAnchorHash,
                routes: routes
            ))
        }
        return SccpRegistryV1(version: 1, lanes: lanes, rawJSON: Data(data))
    }

    static func bundle(_ data: Data) throws -> SccpMessageBundleV1 {
        let root = try SccpStrictJSON.object(data, label: "SCCP message bundle")
        try SccpStrictJSON.exactFields(root, [
            "version", "commitment_root", "commitment", "merkle_proof", "payload", "finality_proof",
        ], label: "SCCP message bundle")
        guard try SccpStrictJSON.uint64(root, "version", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("SCCP message bundle version must be exactly 1")
        }
        let commitmentRoot = try prefixedHash(root, "commitment_root")
        let commitment = try object(root, "commitment")
        try SccpStrictJSON.exactFields(commitment, ["version", "kind", "context", "message_id", "payload_hash"], label: "SCCP message commitment")
        guard try SccpStrictJSON.uint64(commitment, "version", minimum: 1) == 1,
              try SccpStrictJSON.text(commitment, "kind") == "Transfer"
        else { throw SccpV1Error.invalid("SCCP message commitment must be V1 Transfer") }
        let messageId = try prefixedHash(commitment, "message_id")
        let payloadHash = try prefixedHash(commitment, "payload_hash")
        let context = try object(commitment, "context")
        try SccpStrictJSON.exactFields(context, ["lane", "destination_binding_hash", "route_configuration_hash"], label: "SCCP message context")
        let lane = try outboundLane(object(context, "lane"), label: "SCCP message context lane")
        let destination = try prefixedHash(context, "destination_binding_hash")
        let configuration = try prefixedHash(context, "route_configuration_hash")
        try distinctHexRoles([messageId, payloadHash, commitmentRoot, destination, configuration], label: "SCCP bundle hash roles")
        let proof = try object(root, "merkle_proof")
        try SccpStrictJSON.exactFields(proof, ["steps"], label: "SCCP Merkle proof")
        guard try array(proof, "steps").count <= 64 else { throw SccpV1Error.invalid("SCCP Merkle proof is too deep") }
        _ = try variableHex(root, "finality_proof")
        let payload = try object(root, "payload")
        try SccpStrictJSON.exactFields(payload, ["Transfer"], label: "SCCP payload")
        try transferPayload(object(payload, "Transfer"), lane: lane)
        return SccpMessageBundleV1(
            version: 1,
            commitmentRoot: commitmentRoot,
            messageId: messageId,
            targetDomain: lane.target.domainId,
            rawJSON: Data(data)
        )
    }

    static func proofRequest(_ data: Data) throws -> SccpGroth16ProofRequestV1 {
        let root = try SccpStrictJSON.object(data, label: "SCCP proof request")
        let backend = try destinationBackend(object(root, "backend"), label: "SCCP proof request backend")
        var exactFields: Set<String> = [
            "version", "backend", "source_network", "target_network", "public_inputs", "verifying_key",
            "verifier_key_hash", "semantic_proof_profile", "semantic_proof_profile_hash",
            "sora_finality_anchor", "sora_finality_anchor_hash", "bundle_bytes", "statement_hash",
            "destination_binding_hash", "route_configuration_hash", "request_hash",
        ]
        if backend == .tonGroth16Bls12381 {
            exactFields.formUnion(["public_signals", "verifier_circuit_hash", "proof_profile_commitment"])
        }
        try SccpStrictJSON.exactFields(root, exactFields, label: "SCCP proof request")
        guard try SccpStrictJSON.uint64(root, "version", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("SCCP proof request version must be exactly 1")
        }
        let source = try network(object(root, "source_network"), label: "source_network")
        let target = try network(object(root, "target_network"), label: "target_network")
        let backendMatchesTarget: Bool
        switch backend {
        case .evmGroth16Bn254:
            backendMatchesTarget = target.domainId == 1 || target.domainId == 2
        case .tronGroth16Bn254:
            backendMatchesTarget = target.domainId == 3
        case .tonGroth16Bls12381:
            backendMatchesTarget = target.domainId == 4
        }
        guard source == .soraTaira, target.isExternal, backendMatchesTarget else {
            throw SccpV1Error.invalid("SCCP proof backend does not match an exact Taira-to-external lane")
        }
        let inputs = try object(root, "public_inputs")
        try SccpStrictJSON.exactFields(inputs, [
            "version", "message_id", "payload_hash", "target_domain", "commitment_root",
            "finality_height", "finality_block_hash",
        ], label: "SCCP proof public inputs")
        guard try SccpStrictJSON.uint64(inputs, "version", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("SCCP public-input version must be exactly 1")
        }
        let targetDomain = try SccpStrictJSON.uint32(inputs, "target_domain", minimum: 1, maximum: 4)
        guard targetDomain == target.domainId else { throw SccpV1Error.invalid("SCCP target profile/domain mismatch") }
        let messageId = try prefixedHash(inputs, "message_id")
        let payloadHash = try prefixedHash(inputs, "payload_hash")
        let commitmentRoot = try prefixedHash(inputs, "commitment_root")
        let finalityBlockHash = try prefixedHash(inputs, "finality_block_hash")
        let finalityHeight = try decimalUInt64(inputs, "finality_height", minimum: 1)
        let keyBytes: Data
        if backend == .tonGroth16Bls12381 {
            keyBytes = try bls12381VerifyingKey(
                object(root, "verifying_key"),
                label: "SCCP proof verifying key"
            )
        } else {
            keyBytes = try verifyingKey(
                object(root, "verifying_key"),
                label: "SCCP proof verifying key"
            )
        }
        let keyHash = try prefixedHash(root, "verifier_key_hash")
        let derivedKeyHash = backend == .tonGroth16Bls12381
            ? Data(SHA256.hash(data: keyBytes))
            : irohaKeccak256(keyBytes)
        guard "0x" + SccpV1.encodeLowerHex(derivedKeyHash) == keyHash else {
            throw SccpV1Error.invalid("verifier_key_hash does not match the exact verifying key")
        }
        let semantic = try semanticProfile(object(root, "semantic_proof_profile"), label: "semantic_proof_profile")
        let semanticHash = try prefixedHash(root, "semantic_proof_profile_hash")
        guard "0x" + SccpV1.encodeLowerHex(semantic.profileHash) == semanticHash else {
            throw SccpV1Error.invalid("semantic_proof_profile_hash does not match its typed profile")
        }
        let finalityAnchor = try finalityAnchor(object(root, "sora_finality_anchor"), label: "sora_finality_anchor")
        let anchorHash = try prefixedHash(root, "sora_finality_anchor_hash")
        guard "0x" + SccpV1.encodeLowerHex(finalityAnchor.anchorHash) == anchorHash else {
            throw SccpV1Error.invalid("sora_finality_anchor_hash does not match its typed anchor")
        }
        try validateOutboundPolicyRoles(
            semantic,
            finalityAnchor,
            label: "SCCP proof request outbound policy"
        )
        let statement = try prefixedHash(root, "statement_hash")
        let binding = try prefixedHash(root, "destination_binding_hash")
        let configuration = try prefixedHash(root, "route_configuration_hash")
        let request = try prefixedHash(root, "request_hash")
        let verifierCircuitHash: String?
        let proofProfileCommitment: String?
        let publicSignals: SccpGroth16Bls12381PublicSignalsV1?
        if backend == .tonGroth16Bls12381 {
            verifierCircuitHash = try prefixedHash(root, "verifier_circuit_hash")
            proofProfileCommitment = try prefixedHash(root, "proof_profile_commitment")
            guard semantic.kind == .groth16Bls12381,
                  verifierCircuitHash == "0x" + SccpV1.encodeLowerHex(semantic.circuitCommitment),
                  proofProfileCommitment == "0x" + SccpV1.encodeLowerHex(tonProofProfileCommitment())
            else { throw SccpV1Error.invalid("SCCP TON request does not bind the exact BLS12-381 proof profile") }
            publicSignals = try bls12381PublicSignals(
                object(root, "public_signals"),
                messageId: messageId,
                payloadHash: payloadHash,
                targetDomain: targetDomain,
                commitmentRoot: commitmentRoot,
                finalityHeight: finalityHeight,
                finalityBlockHash: finalityBlockHash,
                statementHash: statement,
                destinationBindingHash: binding,
                routeConfigurationHash: configuration,
                soraFinalityAnchorHash: anchorHash
            )
        } else {
            guard semantic.kind == .groth16Bn254 else {
                throw SccpV1Error.invalid("SCCP BN254 request must use the BN254 semantic profile")
            }
            verifierCircuitHash = nil
            proofProfileCommitment = nil
            publicSignals = nil
        }
        try distinctHexRoles([
            messageId, payloadHash, commitmentRoot, finalityBlockHash, keyHash, semanticHash,
            anchorHash, statement, binding, configuration, request,
        ] + [verifierCircuitHash, proofProfileCommitment].compactMap { $0 }, label: "SCCP proof-request hash roles")
        return SccpGroth16ProofRequestV1(
            version: 1,
            backend: backend,
            sourceNetwork: source,
            targetNetwork: target,
            messageId: messageId,
            payloadHash: payloadHash,
            targetDomain: targetDomain,
            commitmentRoot: commitmentRoot,
            finalityHeight: finalityHeight,
            finalityBlockHash: finalityBlockHash,
            publicSignals: publicSignals,
            verifierKeyHash: keyHash,
            verifierCircuitHash: verifierCircuitHash,
            proofProfileCommitment: proofProfileCommitment,
            semanticProofProfile: semantic,
            semanticProofProfileHash: semanticHash,
            soraFinalityAnchor: finalityAnchor,
            soraFinalityAnchorHash: anchorHash,
            statementHash: statement,
            destinationBindingHash: binding,
            routeConfigurationHash: configuration,
            requestHash: request,
            bundleBytes: try variableHex(root, "bundle_bytes"),
            rawJSON: Data(data)
        )
    }

    static func recent(_ data: Data) throws -> SccpRecentMessages {
        let root = try SccpStrictJSON.object(data, label: "SCCP recent messages")
        try SccpStrictJSON.exactFields(
            root,
            allowed: ["items", "next"],
            required: ["items"],
            label: "SCCP recent messages"
        )
        let values = try array(root, "items")
        guard values.count <= 50 else { throw SccpV1Error.invalid("SCCP recent response exceeds 50 items") }
        var items: [SccpRecentMessage] = []
        var ids = Set<String>()
        for (index, raw) in values.enumerated() {
            let item = try object(raw, label: "items[\(index)]")
            let required: Set<String> = [
                "height", "commitment_index", "message_id_hex", "kind", "source_profile",
                "target_profile",
                "destination_binding_hash", "route_configuration_hash", "target_domain",
                "amount", "payload_projection", "links",
            ]
            let allowed = required.union(["asset_id", "route_id", "recipient"])
            try SccpStrictJSON.exactFields(item, allowed: allowed, required: required, label: "items[\(index)]")
            guard try SccpStrictJSON.text(item, "kind") == SccpPayloadKindV1.transfer.rawValue else {
                throw SccpV1Error.invalid("recent SCCP message kind is unknown or retired")
            }
            guard let source = SccpNetworkV1(rawValue: try SccpStrictJSON.text(item, "source_profile")),
                  let target = SccpNetworkV1(rawValue: try SccpStrictJSON.text(item, "target_profile"))
            else { throw SccpV1Error.invalid("recent SCCP message contains an unsupported profile") }
            let lane = try SccpLaneIdV1(source: source, target: target)
            guard lane.isOutbound, source == .soraTaira else { throw SccpV1Error.invalid("recent SCCP lane must be Taira-to-external") }
            let domain = try SccpStrictJSON.uint32(item, "target_domain", minimum: 1, maximum: 4)
            guard domain == target.domainId else { throw SccpV1Error.invalid("recent SCCP target profile/domain mismatch") }
            let id = try unprefixedHash(item, "message_id_hex")
            guard ids.insert(id).inserted else { throw SccpV1Error.invalid("recent SCCP message ids must be unique") }
            let binding = try prefixedHash(item, "destination_binding_hash")
            let configuration = try prefixedHash(item, "route_configuration_hash")
            try distinctHexRoles(["0x" + id, binding, configuration], label: "recent SCCP hash roles")
            let amount = try decimalText(item, "amount", minimum: 1)
            let links = try object(item, "links")
            try SccpStrictJSON.exactFields(links, ["bundle_path", "proof_request_path"], label: "recent SCCP links")
            let expectedBundle = "/v1/sccp/proofs/message/\(id)"
            let expectedRequest = "/v1/sccp/proof-requests/\(id)"
            guard try path(links, "bundle_path") == expectedBundle,
                  try path(links, "proof_request_path") == expectedRequest
            else { throw SccpV1Error.invalid("recent SCCP links must name the exact message") }
            let projection = try payloadProjectionData(
                item,
                field: "payload_projection",
                expectedDestinationDomain: domain,
                expectedAmount: amount,
                label: "items[\(index)].payload_projection"
            )
            let assetId = try optionalText(item, "asset_id")
            let routeId = try optionalText(item, "route_id")
            let recipient = try optionalText(item, "recipient")
            let expectedRoute: String
            switch domain {
            case 1: expectedRoute = "taira_eth_xor"
            case 2: expectedRoute = "taira_bsc_xor"
            case 4: expectedRoute = "taira_ton_xor"
            case 5: expectedRoute = "taira_tron_xor"
            default: throw SccpV1Error.invalid("recent SCCP target domain is unsupported")
            }
            guard (assetId == nil || assetId == "xor"),
                  (routeId == nil || routeId == expectedRoute),
                  recipient == nil
            else { throw SccpV1Error.invalid("recent SCCP summary fields disagree with payload_projection") }
            items.append(SccpRecentMessage(
                height: try SccpStrictJSON.uint64(item, "height", minimum: 1),
                commitmentIndex: try SccpStrictJSON.uint32(
                    item, "commitment_index", minimum: 0, maximum: 511
                ),
                messageIdHex: id,
                kind: .transfer,
                lane: lane,
                destinationBindingHash: binding,
                routeConfigurationHash: configuration,
                assetId: assetId,
                routeId: routeId,
                recipient: recipient,
                amount: amount,
                payloadProjectionJSON: projection,
                links: SccpRecentMessageLinks(bundlePath: expectedBundle, proofRequestPath: expectedRequest)
            ))
        }
        if items.count > 1 {
            for index in 1..<items.count {
                let previous = items[index - 1]
                let current = items[index]
                guard current.height <= previous.height else {
                    throw SccpV1Error.invalid("recent SCCP messages must be newest first")
                }
                if current.height == previous.height {
                    guard current.commitmentIndex == previous.commitmentIndex + 1 else {
                        throw SccpV1Error.invalid(
                            "same-height recent SCCP messages must have contiguous ascending commitment indices"
                        )
                    }
                } else if current.commitmentIndex != 0 {
                    throw SccpV1Error.invalid("an older SCCP block must begin at commitment index zero")
                }
            }
        }
        let next: SccpRecentCursor?
        if let rawNext = root["next"] {
            let cursor = try object(rawNext, label: "SCCP recent messages.next")
            try SccpStrictJSON.exactFields(
                cursor, ["from", "after_index"], label: "SCCP recent messages.next"
            )
            next = SccpRecentCursor(
                from: try SccpStrictJSON.uint64(cursor, "from", minimum: 1),
                afterIndex: try SccpStrictJSON.uint32(
                    cursor, "after_index", minimum: 0, maximum: 511
                )
            )
        } else {
            next = nil
        }
        if let next {
            guard let last = items.last else {
                throw SccpV1Error.invalid(
                    "an empty SCCP recent page must not advertise a continuation"
                )
            }
            guard next.from == last.height, next.afterIndex == last.commitmentIndex else {
                throw SccpV1Error.invalid("SCCP recent continuation must identify the last returned item")
            }
        }
        return SccpRecentMessages(items: items, next: next)
    }

    private static func governedRoute(
        _ item: [String: Any],
        expectedLane: SccpLaneIdV1,
        nativeAnchor: SccpNativeTrustAnchorV1?,
        label: String
    ) throws -> SccpGovernedRouteV1 {
        try SccpStrictJSON.exactFields(item, [
            "lane_id", "route_id", "asset_key", "revision", "activation",
            "inbound_finality_cutoff", "source_identity", "destination",
            "sora_outbound_execution_policy", "settlement",
        ], label: label)
        let lane = try inboundLane(object(item, "lane_id"), label: "\(label).lane_id")
        guard lane == expectedLane else { throw SccpV1Error.invalid("\(label).lane_id does not match its parent lane") }
        let routeId = try routeKey(item, "route_id")
        let assetKey = try routeKey(item, "asset_key")
        let revision = try SccpStrictJSON.uint32(item, "revision", minimum: 1, maximum: UInt32.max)
        let activation = try activation(object(item, "activation"), label: "\(label).activation")
        let cutoff: SccpInboundFinalityCutoffV1?
        if item["inbound_finality_cutoff"] is NSNull {
            cutoff = nil
        } else {
            let cutoffObject = try object(item, "inbound_finality_cutoff")
            try SccpStrictJSON.exactFields(
                cutoffObject,
                ["trust_anchor_hash", "max_anchor_interval_height"],
                label: "\(label).inbound_finality_cutoff"
            )
            cutoff = SccpInboundFinalityCutoffV1(
                trustAnchorHash: try upperFixed(cutoffObject, "trust_anchor_hash", bytes: 32),
                maxAnchorIntervalHeight: try SccpStrictJSON.uint64(
                    cutoffObject,
                    "max_anchor_interval_height",
                    minimum: 1
                )
            )
        }
        guard (activation == .retired) == (cutoff != nil) else {
            throw SccpV1Error.invalid("\(label).inbound_finality_cutoff must be present exactly for a retired route")
        }
        let source = try sourceIdentity(object(item, "source_identity"), expectedLane: lane, label: "\(label).source_identity")
        let destination = try destination(object(item, "destination"), lane: lane, label: "\(label).destination")
        guard sourceMatchesDestination(source, destination) else {
            throw SccpV1Error.invalid("\(label) source identity does not name its destination route deployment")
        }
        let executionPolicy = try soraOutboundExecutionPolicy(
            object(item, "sora_outbound_execution_policy"),
            label: "\(label).sora_outbound_execution_policy"
        )
        let settlement = try object(item, "settlement")
        try SccpStrictJSON.exactFields(
            settlement,
            ["asset_definition_id", "payload_amount_scale", "max_outstanding_liability"],
            label: "\(label).settlement"
        )
        let assetDefinition = try SccpStrictJSON.text(settlement, "asset_definition_id")
        guard assetDefinition == "6TEAJqbb8oEPmLncoNiMRbLEK6tw" else { throw SccpV1Error.invalid("\(label) must settle canonical Taira XOR") }
        let scale = try SccpStrictJSON.uint32(settlement, "payload_amount_scale", minimum: 9, maximum: 9)
        let maxOutstandingLiability = try unsignedIntegerText(
            settlement,
            "max_outstanding_liability",
            maximum: maximumUInt128
        )
        guard multiplyDecimal(maxOutstandingLiability, by: destination.tairaToTokenMultiplier)
            == destination.maxWrappedSupply else {
            throw SccpV1Error.invalid("\(label) wrapped-supply cap does not match its SORA liability cap")
        }
        let configuration = try routeConfigurationHash(
            lane: lane,
            routeId: routeId,
            assetKey: assetKey,
            revision: revision,
            destination: destination
        )
        guard sourceRouteConfigHash(source) == configuration else {
            throw SccpV1Error.invalid("\(label) source route_config_hash does not match the immutable deployment")
        }
        var governedRoles = [
            executionPolicy.contractArtifactSha256,
            executionPolicy.verifyingKeyReference.commitment,
            configuration,
            destination.destinationBindingHash,
            destination.verifierKeyHash,
            destination.outboundProofPolicy.semanticProfile.profileHash,
            destination.outboundProofPolicy.soraFinalityAnchor.anchorHash,
        ]
        if case let .ton(ton) = destination {
            governedRoles.append(ton.jettonMasterInitialDataHash)
            governedRoles.append(ton.routeInitialDataHash)
        }
        guard Set(governedRoles).count == governedRoles.count else {
            throw SccpV1Error.invalid("\(label) reuses a governed execution or deployment hash role")
        }
        if activation.allowsInbound {
            guard let nativeAnchor, nativeAnchor.backend.supports(lane.source) else {
                throw SccpV1Error.invalid("\(label) enables inbound settlement without a matching native trust anchor")
            }
        }
        return SccpGovernedRouteV1(
            lane: lane,
            routeId: routeId,
            assetKey: assetKey,
            revision: revision,
            activation: activation,
            inboundFinalityCutoff: cutoff,
            sourceEmitter: source,
            destination: destination,
            soraOutboundExecutionPolicy: executionPolicy,
            assetDefinitionId: assetDefinition,
            payloadAmountScale: scale,
            maxOutstandingLiability: maxOutstandingLiability,
            routeConfigurationHash: configuration
        )
    }

    private static func soraOutboundExecutionPolicy(
        _ item: [String: Any],
        label: String
    ) throws -> SccpSoraOutboundExecutionPolicyV1 {
        try SccpStrictJSON.exactFields(
            item,
            ["version", "semantics", "contract_artifact_sha256", "vk_ref", "gas_limit"],
            label: label
        )
        guard try SccpStrictJSON.uint64(item, "version", minimum: 1, maximum: 1) == 1,
              try SccpStrictJSON.text(item, "semantics") == "ivm_proved_record_sccp_message_v1"
        else { throw SccpV1Error.invalid("\(label) does not select the exact V1 execution semantics") }
        let contractArtifactSha256 = try upperFixed(item, "contract_artifact_sha256", bytes: 32)
        let reference = try object(item, "vk_ref")
        try SccpStrictJSON.exactFields(
            reference,
            ["backend", "name", "version", "commitment"],
            label: "\(label).vk_ref"
        )
        let backend = try SccpStrictJSON.text(reference, "backend")
        let name = try SccpStrictJSON.text(reference, "name")
        guard portableVerifyingKeyField(backend), portableVerifyingKeyField(name) else {
            throw SccpV1Error.invalid("\(label).vk_ref is not a portable verifying-key identity")
        }
        let keyReference = SccpPortableVerifyingKeyReferenceV1(
            backend: backend,
            name: name,
            version: try SccpStrictJSON.uint32(
                reference,
                "version",
                minimum: 1,
                maximum: UInt32.max
            ),
            commitment: try upperFixed(reference, "commitment", bytes: 32)
        )
        guard contractArtifactSha256 != keyReference.commitment else {
            throw SccpV1Error.invalid("\(label) reuses its artifact and verification-key hash roles")
        }
        return SccpSoraOutboundExecutionPolicyV1(
            version: 1,
            semantics: "ivm_proved_record_sccp_message_v1",
            contractArtifactSha256: contractArtifactSha256,
            verifyingKeyReference: keyReference,
            gasLimit: try SccpStrictJSON.uint64(item, "gas_limit", minimum: 1, maximum: 1_000_000_000)
        )
    }

    private static func portableVerifyingKeyField(_ value: String) -> Bool {
        let bytes = Array(value.utf8)
        func isLowercaseOrDigit(_ byte: UInt8) -> Bool {
            (97...122).contains(byte) || (48...57).contains(byte)
        }
        guard (1...256).contains(bytes.count),
              let first = bytes.first, let last = bytes.last,
              isLowercaseOrDigit(first),
              isLowercaseOrDigit(last),
              !["..", "//", ":::", "/:", ":/", "/.", "./", ":.", ".:"]
                  .contains(where: value.contains)
        else { return false }
        return bytes.allSatisfy {
            isLowercaseOrDigit($0) || [45, 95, 47, 58, 46].contains($0)
        }
    }

    private static func multiplyDecimal(_ value: String, by multiplier: UInt64) -> String {
        var carry: UInt64 = 0
        var digits: [UInt8] = []
        digits.reserveCapacity(value.count + 10)
        for byte in value.utf8.reversed() {
            let product = UInt64(byte - 48) * multiplier + carry
            digits.append(UInt8(product % 10))
            carry = product / 10
        }
        while carry != 0 {
            digits.append(UInt8(carry % 10))
            carry /= 10
        }
        return digits.reversed().map(String.init).joined()
    }

    private static func destination(_ item: [String: Any], lane: SccpLaneIdV1, label: String) throws -> SccpDestinationDeploymentV1 {
        try SccpStrictJSON.exactFields(item, ["family", "deployment"], label: label)
        let familyText = try SccpStrictJSON.text(item, "family")
        switch familyText {
        case "evm", "tron":
            return try evmTronDestination(
                object(item, "deployment"),
                familyText: familyText,
                lane: lane,
                label: "\(label).deployment"
            )
        case "ton":
            return try tonDestination(
                object(item, "deployment"),
                lane: lane,
                label: "\(label).deployment"
            )
        default: throw SccpV1Error.invalid("\(label) family is unsupported or retired")
        }
    }

    private static func evmTronDestination(
        _ deployment: [String: Any],
        familyText: String,
        lane: SccpLaneIdV1,
        label: String
    ) throws -> SccpDestinationDeploymentV1 {
        let family: SccpDestinationProofBackendV1 = familyText == "tron" ? .tronGroth16Bn254 : .evmGroth16Bn254
        guard (family == .tronGroth16Bn254) == lane.source.rawValue.hasPrefix("tron-") else {
            throw SccpV1Error.invalid("\(label) family does not match its lane")
        }
        try SccpStrictJSON.exactFields(deployment, [
            "token_address", "token_code_hash", "verifier_address", "verifier_code_hash",
            "verifying_key", "verifier_key_hash", "outbound_proof_policy", "route_address",
            "route_code_hash", "replay_verifier_address", "replay_verifier_code_hash",
            "mint_breaker_address", "mint_breaker_code_hash", "taira_to_token_multiplier",
            "max_wrapped_supply",
        ], label: "\(label).deployment")
        let addresses = try [
            "token_address", "verifier_address", "route_address", "replay_verifier_address",
            "mint_breaker_address",
        ].map {
            try upperFixed(deployment, $0, bytes: 20)
        }
        let hashes = try [
            "token_code_hash", "verifier_code_hash", "verifier_key_hash", "route_code_hash",
            "replay_verifier_code_hash", "mint_breaker_code_hash",
        ].map {
            try upperFixed(deployment, $0, bytes: 32)
        }
        guard Set(addresses).count == addresses.count, Set(hashes).count == hashes.count else {
            throw SccpV1Error.invalid("\(label).deployment reuses a role-separated address or hash")
        }
        for (index, field) in [
            "token_code_hash", "verifier_code_hash", "route_code_hash",
            "replay_verifier_code_hash", "mint_breaker_code_hash",
        ].enumerated() {
            let hashIndex = index < 2 ? index : index + 1
            try requireRuntimeCodeHash(hashes[hashIndex], field: field)
        }
        let keyBytes = try verifyingKey(object(deployment, "verifying_key"), label: "\(label).deployment.verifying_key")
        guard irohaKeccak256(keyBytes) == hashes[2] else { throw SccpV1Error.invalid("\(label).deployment.verifier_key_hash does not match verifying_key") }
        let policy = try outboundPolicy(object(deployment, "outbound_proof_policy"), label: "\(label).deployment.outbound_proof_policy")
        guard policy.semanticProfile.kind == .groth16Bn254 else {
            throw SccpV1Error.invalid("\(label) requires the BN254 semantic profile")
        }
        let deploymentHashRoles = hashes + [
            policy.semanticProfile.profileHash,
            policy.soraFinalityAnchor.anchorHash,
        ]
        guard Set(deploymentHashRoles).count == deploymentHashRoles.count else {
            throw SccpV1Error.invalid("\(label).deployment reuses a proof-policy or deployment hash role")
        }
        guard try SccpStrictJSON.uint64(deployment, "taira_to_token_multiplier", minimum: 1_000_000_000) == 1_000_000_000 else {
            throw SccpV1Error.invalid("\(label).deployment has the wrong Taira/token multiplier")
        }
        let maxWrappedSupply = try unsignedIntegerText(
            deployment,
            "max_wrapped_supply",
            maximum: maximumUInt128
        )
        let partial = SccpEvmTronDestinationDeploymentV1(
            tokenAddress: addresses[0],
            tokenCodeHash: hashes[0],
            verifierAddress: addresses[1],
            verifierCodeHash: hashes[1],
            verifierKeyHash: hashes[2],
            outboundProofPolicy: policy,
            routeAddress: addresses[2],
            routeCodeHash: hashes[3],
            replayVerifierAddress: addresses[3],
            replayVerifierCodeHash: hashes[4],
            mintBreakerAddress: addresses[4],
            mintBreakerCodeHash: hashes[5],
            tairaToTokenMultiplier: 1_000_000_000,
            maxWrappedSupply: maxWrappedSupply,
            destinationBindingHash: Data()
        )
        let wrapped: SccpDestinationDeploymentV1 = family == .tronGroth16Bn254 ? .tron(partial) : .evm(partial)
        let binding = try destinationBindingHash(lane: lane, destination: wrapped)
        let complete = SccpEvmTronDestinationDeploymentV1(
            tokenAddress: partial.tokenAddress,
            tokenCodeHash: partial.tokenCodeHash,
            verifierAddress: partial.verifierAddress,
            verifierCodeHash: partial.verifierCodeHash,
            verifierKeyHash: partial.verifierKeyHash,
            outboundProofPolicy: partial.outboundProofPolicy,
            routeAddress: partial.routeAddress,
            routeCodeHash: partial.routeCodeHash,
            replayVerifierAddress: partial.replayVerifierAddress,
            replayVerifierCodeHash: partial.replayVerifierCodeHash,
            mintBreakerAddress: partial.mintBreakerAddress,
            mintBreakerCodeHash: partial.mintBreakerCodeHash,
            tairaToTokenMultiplier: partial.tairaToTokenMultiplier,
            maxWrappedSupply: partial.maxWrappedSupply,
            destinationBindingHash: binding
        )
        return family == .tronGroth16Bn254 ? .tron(complete) : .evm(complete)
    }

    private static func tonDestination(
        _ deployment: [String: Any],
        lane: SccpLaneIdV1,
        label: String
    ) throws -> SccpDestinationDeploymentV1 {
        guard lane.source == .tonMainnet else {
            throw SccpV1Error.invalid("\(label) TON family does not match its lane")
        }
        try SccpStrictJSON.exactFields(deployment, [
            "jetton_master_address", "jetton_master_code_hash", "jetton_master_initial_data_hash",
            "jetton_wallet_code_hash", "route_address", "route_code_hash",
            "route_initial_data_hash", "embedded_verifier_code_hash",
            "verifier_circuit_hash", "verifying_key", "verifier_key_hash",
            "proof_profile_commitment", "mint_breaker_guardian_keys", "outbound_proof_policy",
            "taira_to_token_multiplier", "max_wrapped_supply",
        ], label: label)
        let master = try tonAddress(object(deployment, "jetton_master_address"), label: "\(label).jetton_master_address")
        let route = try tonAddress(object(deployment, "route_address"), label: "\(label).route_address")
        guard master != route else { throw SccpV1Error.invalid("\(label) reuses a TON contract address") }
        let masterCode = try upperFixed(deployment, "jetton_master_code_hash", bytes: 32)
        let masterInitialData = try upperFixed(deployment, "jetton_master_initial_data_hash", bytes: 32)
        let walletCode = try upperFixed(deployment, "jetton_wallet_code_hash", bytes: 32)
        let routeCode = try upperFixed(deployment, "route_code_hash", bytes: 32)
        let routeInitialData = try upperFixed(deployment, "route_initial_data_hash", bytes: 32)
        let embeddedCode = try upperFixed(deployment, "embedded_verifier_code_hash", bytes: 32)
        let circuit = try upperFixed(deployment, "verifier_circuit_hash", bytes: 32)
        let keyHash = try upperFixed(deployment, "verifier_key_hash", bytes: 32)
        let profileCommitment = try upperFixed(deployment, "proof_profile_commitment", bytes: 32)
        let guardianObject = try object(deployment, "mint_breaker_guardian_keys")
        try SccpStrictJSON.exactFields(
            guardianObject,
            ["guardian_0", "guardian_1", "guardian_2", "guardian_3", "guardian_4"],
            label: "\(label).mint_breaker_guardian_keys"
        )
        let guardians = try SccpTonMintBreakerGuardianKeysV1(
            guardian0: upperFixed(guardianObject, "guardian_0", bytes: 32),
            guardian1: upperFixed(guardianObject, "guardian_1", bytes: 32),
            guardian2: upperFixed(guardianObject, "guardian_2", bytes: 32),
            guardian3: upperFixed(guardianObject, "guardian_3", bytes: 32),
            guardian4: upperFixed(guardianObject, "guardian_4", bytes: 32)
        )
        let keyBytes = try bls12381VerifyingKey(
            object(deployment, "verifying_key"),
            label: "\(label).verifying_key"
        )
        guard Data(SHA256.hash(data: keyBytes)) == keyHash else {
            throw SccpV1Error.invalid("\(label).verifier_key_hash does not match its BLS12-381 key")
        }
        let policy = try outboundPolicy(object(deployment, "outbound_proof_policy"), label: "\(label).outbound_proof_policy")
        guard policy.semanticProfile.kind == .groth16Bls12381,
              circuit == policy.semanticProfile.circuitCommitment,
              profileCommitment == tonProofProfileCommitment()
        else { throw SccpV1Error.invalid("\(label) does not bind the exact TON proof profile") }
        let hashes = [masterCode, masterInitialData, walletCode, routeCode, routeInitialData,
                      embeddedCode, circuit, keyHash,
                      profileCommitment, policy.semanticProfile.profileHash,
                      policy.soraFinalityAnchor.anchorHash]
        guard Set(hashes).count == hashes.count else {
            throw SccpV1Error.invalid("\(label) reuses a role-separated TON hash")
        }
        guard try SccpStrictJSON.uint64(deployment, "taira_to_token_multiplier", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("\(label) TON multiplier must be exactly 1")
        }
        let maxWrappedSupply = try unsignedIntegerText(
            deployment,
            "max_wrapped_supply",
            maximum: maximumTonCoins
        )
        let partial = SccpTonDestinationDeploymentV1(
            jettonMasterAddress: master,
            jettonMasterCodeHash: masterCode,
            jettonMasterInitialDataHash: masterInitialData,
            jettonWalletCodeHash: walletCode,
            routeAddress: route,
            routeCodeHash: routeCode,
            routeInitialDataHash: routeInitialData,
            embeddedVerifierCodeHash: embeddedCode,
            verifierCircuitHash: circuit,
            verifierKeyHash: keyHash,
            proofProfileCommitment: profileCommitment,
            mintBreakerGuardianKeys: guardians,
            outboundProofPolicy: policy,
            tairaToTokenMultiplier: 1,
            maxWrappedSupply: maxWrappedSupply,
            destinationBindingHash: Data()
        )
        let binding = try destinationBindingHash(lane: lane, destination: .ton(partial))
        return .ton(SccpTonDestinationDeploymentV1(
            jettonMasterAddress: partial.jettonMasterAddress,
            jettonMasterCodeHash: partial.jettonMasterCodeHash,
            jettonMasterInitialDataHash: partial.jettonMasterInitialDataHash,
            jettonWalletCodeHash: partial.jettonWalletCodeHash,
            routeAddress: partial.routeAddress,
            routeCodeHash: partial.routeCodeHash,
            routeInitialDataHash: partial.routeInitialDataHash,
            embeddedVerifierCodeHash: partial.embeddedVerifierCodeHash,
            verifierCircuitHash: partial.verifierCircuitHash,
            verifierKeyHash: partial.verifierKeyHash,
            proofProfileCommitment: partial.proofProfileCommitment,
            mintBreakerGuardianKeys: partial.mintBreakerGuardianKeys,
            outboundProofPolicy: partial.outboundProofPolicy,
            tairaToTokenMultiplier: partial.tairaToTokenMultiplier,
            maxWrappedSupply: partial.maxWrappedSupply,
            destinationBindingHash: binding
        ))
    }

    private static func outboundPolicy(
        _ item: [String: Any],
        label: String
    ) throws -> SccpOutboundProofPolicyV1 {
        try SccpStrictJSON.exactFields(item, ["version", "semantic_profile", "sora_finality_anchor"], label: label)
        guard try SccpStrictJSON.uint64(item, "version", minimum: 1) == 1 else { throw SccpV1Error.invalid("\(label).version must be 1") }
        let semantic = try semanticProfile(object(item, "semantic_profile"), label: "\(label).semantic_profile")
        let anchor = try finalityAnchor(object(item, "sora_finality_anchor"), label: "\(label).sora_finality_anchor")
        try validateOutboundPolicyRoles(semantic, anchor, label: label)
        return SccpOutboundProofPolicyV1(
            version: 1,
            semanticProfile: semantic,
            soraFinalityAnchor: anchor
        )
    }

    private static func validateOutboundPolicyRoles(
        _ semantic: SccpSemanticProofProfileV1,
        _ anchor: SccpSoraFinalityAnchorV1,
        label: String
    ) throws {
        let roles = [semantic.circuitCommitment, semantic.witnessGeneratorCommitment, semantic.publicSignalSchemaHash,
                     semantic.profileHash, anchor.chainIdHash, anchor.checkpointBlockHash,
                     anchor.checkpointContextId, anchor.checkpointFinalityArtifactHash, anchor.anchorHash]
        guard roles.allSatisfy({ !$0.allSatisfy { $0 == 0 } }), Set(roles).count == roles.count else {
            throw SccpV1Error.invalid("\(label) reuses a proof-policy hash role")
        }
    }

    private static func semanticProfile(_ item: [String: Any], label: String) throws -> SccpSemanticProofProfileV1 {
        try SccpStrictJSON.exactFields(item, ["profile", "commitments"], label: label)
        guard let kind = SccpSemanticProofProfileKindV1(rawValue: try SccpStrictJSON.text(item, "profile")) else {
            throw SccpV1Error.invalid("\(label) is unsupported or retired")
        }
        let commitments = try object(item, "commitments")
        try SccpStrictJSON.exactFields(commitments, [
            "version", "circuit_commitment", "witness_generator_commitment", "public_signal_schema_hash",
        ], label: "\(label).commitments")
        guard try SccpStrictJSON.uint64(commitments, "version", minimum: 1) == 1 else { throw SccpV1Error.invalid("\(label) version must be 1") }
        let circuit = try upperFixed(commitments, "circuit_commitment", bytes: 32)
        let witness = try upperFixed(commitments, "witness_generator_commitment", bytes: 32)
        let schema = try upperFixed(commitments, "public_signal_schema_hash", bytes: 32)
        let expectedSchema = kind == .groth16Bn254 ? publicSignalSchemaHash() : bls12381PublicSignalSchemaHash()
        guard schema == expectedSchema, Set([circuit, witness, schema]).count == 3 else {
            throw SccpV1Error.invalid("\(label) does not commit the exact eleven-signal schema")
        }
        let canonical = Data([1, kind == .groth16Bn254 ? 0 : 1, 1]) + circuit + witness + schema
        let hash = irohaKeccak256(Data("sccp:semantic-proof-profile:v1".utf8) + canonical)
        return SccpSemanticProofProfileV1(
            kind: kind,
            circuitCommitment: circuit,
            witnessGeneratorCommitment: witness,
            publicSignalSchemaHash: schema,
            profileHash: hash
        )
    }

    private static func finalityAnchor(_ item: [String: Any], label: String) throws -> SccpSoraFinalityAnchorV1 {
        try SccpStrictJSON.exactFields(item, [
            "version", "source_network", "protocol_version", "chain_id_hash", "checkpoint_height",
            "checkpoint_block_hash", "checkpoint_context_id", "checkpoint_finality_artifact_hash",
        ], label: label)
        guard try SccpStrictJSON.uint64(item, "version", minimum: 1) == 1,
              try network(object(item, "source_network"), label: "\(label).source_network") == .soraTaira
        else { throw SccpV1Error.invalid("\(label) must be a V1 Taira anchor") }
        let protocolVersion = try SccpStrictJSON.uint32(
            item, "protocol_version", minimum: 4, maximum: 4
        )
        let chainHash = try upperFixed(item, "chain_id_hash", bytes: 32)
        guard chainHash == irohaKeccak256(tairaChainId) else { throw SccpV1Error.invalid("\(label).chain_id_hash is not Taira") }
        let checkpointHeight = try SccpStrictJSON.uint64(item, "checkpoint_height", minimum: 1)
        let checkpointHash = try upperFixed(item, "checkpoint_block_hash", bytes: 32)
        let contextId = try upperFixed(item, "checkpoint_context_id", bytes: 32)
        let artifactHash = try upperFixed(item, "checkpoint_finality_artifact_hash", bytes: 32)
        guard Set([chainHash, checkpointHash, contextId, artifactHash]).count == 4 else {
            throw SccpV1Error.invalid("\(label) reuses a finality hash role")
        }
        var canonical = Data([1, SccpNetworkV1.soraTaira.tag])
        appendUInt16LE(UInt16(protocolVersion), to: &canonical)
        canonical.append(chainHash)
        appendUInt64LE(checkpointHeight, to: &canonical)
        canonical.append(checkpointHash)
        canonical.append(contextId)
        canonical.append(artifactHash)
        let hash = irohaKeccak256(Data("sccp:sora-finality-anchor:v1".utf8) + canonical)
        return SccpSoraFinalityAnchorV1(
            protocolVersion: UInt16(protocolVersion),
            chainIdHash: chainHash,
            checkpointHeight: checkpointHeight,
            checkpointBlockHash: checkpointHash,
            checkpointContextId: contextId,
            checkpointFinalityArtifactHash: artifactHash,
            anchorHash: hash
        )
    }

    private static func verifyingKey(_ item: [String: Any], label: String) throws -> Data {
        try SccpStrictJSON.exactFields(item, ["version", "alpha1", "beta2", "gamma2", "delta2", "ic"], label: label)
        guard try SccpStrictJSON.uint64(item, "version", minimum: 1) == 1 else { throw SccpV1Error.invalid("\(label).version must be 1") }
        var words: [Data] = []
        words.append(contentsOf: try g1(object(item, "alpha1"), label: "\(label).alpha1"))
        for field in ["beta2", "gamma2", "delta2"] {
            words.append(contentsOf: try g2(object(item, field), label: "\(label).\(field)"))
        }
        let ic = try object(item, "ic")
        let icFields = ["constant"] + (0...10).map { "signal_\($0)" }
        try SccpStrictJSON.exactFields(ic, Set(icFields), label: "\(label).ic")
        for field in icFields { words.append(contentsOf: try g1(object(ic, field), label: "\(label).ic.\(field)")) }
        guard words.count == 38 else { throw SccpV1Error.invalid("\(label) must contain exactly 38 ABI words") }
        return words.reduce(into: Data()) { $0.append($1) }
    }

    private static func bls12381VerifyingKey(_ item: [String: Any], label: String) throws -> Data {
        try SccpStrictJSON.exactFields(
            item,
            ["version", "alpha1", "beta2", "gamma2", "delta2", "ic"],
            label: label
        )
        guard try SccpStrictJSON.uint64(item, "version", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("\(label).version must be 1")
        }
        let alpha = try upperFixed(item, "alpha1", bytes: 48, allowZero: true)
        let beta = try upperFixed(item, "beta2", bytes: 96, allowZero: true)
        let gamma = try upperFixed(item, "gamma2", bytes: 96, allowZero: true)
        let delta = try upperFixed(item, "delta2", bytes: 96, allowZero: true)
        guard isBls12381G1Compressed(alpha),
              isBls12381G2Compressed(beta),
              isBls12381G2Compressed(gamma),
              isBls12381G2Compressed(delta)
        else { throw SccpV1Error.invalid("\(label) contains a noncanonical compressed BLS12-381 point") }

        let ic = try object(item, "ic")
        let icFields = ["constant"] + (0...10).map { "signal_\($0)" }
        try SccpStrictJSON.exactFields(ic, Set(icFields), label: "\(label).ic")
        let icPoints = try icFields.map {
            try upperFixed(ic, $0, bytes: 48, allowZero: true)
        }
        guard icPoints.allSatisfy(isBls12381G1Compressed) else {
            throw SccpV1Error.invalid("\(label).ic contains a noncanonical compressed BLS12-381 point")
        }
        var canonical = Data([1])
        for point in [alpha, beta, gamma, delta] + icPoints {
            canonical.append(point)
        }
        return canonical
    }

    private static func bls12381PublicSignals(
        _ item: [String: Any],
        messageId: String,
        payloadHash: String,
        targetDomain: UInt32,
        commitmentRoot: String,
        finalityHeight: UInt64,
        finalityBlockHash: String,
        statementHash: String,
        destinationBindingHash: String,
        routeConfigurationHash: String,
        soraFinalityAnchorHash: String
    ) throws -> SccpGroth16Bls12381PublicSignalsV1 {
        let fields = [
            "message_id", "payload_hash", "target_domain", "commitment_root", "finality_height",
            "finality_block_hash", "source_domain", "statement_hash", "destination_binding_hash",
            "route_configuration_hash", "sora_finality_anchor_hash",
        ]
        try SccpStrictJSON.exactFields(item, Set(fields), label: "SCCP TON public signals")
        let encoded = try fields.map { try prefixedHash(item, $0, allowZero: true) }
        let inputWords = [
            try prefixedHashData(messageId),
            try prefixedHashData(payloadHash),
            abiWord(UInt64(targetDomain)),
            try prefixedHashData(commitmentRoot),
            abiWord(finalityHeight),
            try prefixedHashData(finalityBlockHash),
            abiWord(0),
            try prefixedHashData(statementHash),
            try prefixedHashData(destinationBindingHash),
            try prefixedHashData(routeConfigurationHash),
            try prefixedHashData(soraFinalityAnchorHash),
        ]
        let expected = zip(bls12381PublicSignalLabels, inputWords).map {
            "0x" + SccpV1.encodeLowerHex(bls12381SignalWord(label: $0.0, value: $0.1))
        }
        guard encoded == expected else {
            throw SccpV1Error.invalid("SCCP TON public signals do not match their exact request roles")
        }
        return SccpGroth16Bls12381PublicSignalsV1(
            messageId: encoded[0],
            payloadHash: encoded[1],
            targetDomain: encoded[2],
            commitmentRoot: encoded[3],
            finalityHeight: encoded[4],
            finalityBlockHash: encoded[5],
            sourceDomain: encoded[6],
            statementHash: encoded[7],
            destinationBindingHash: encoded[8],
            routeConfigurationHash: encoded[9],
            soraFinalityAnchorHash: encoded[10]
        )
    }

    private static func bls12381SignalWord(label: String, value: Data) -> Data {
        let labelHash = Data(SHA256.hash(data: Data(label.utf8)))
        var word = Data(SHA256.hash(data: labelHash + value))
        while !word.lexicographicallyPrecedes(bls12381ScalarField) {
            word = subtractBigEndian(word, bls12381ScalarField)
        }
        return word
    }

    private static func subtractBigEndian(_ left: Data, _ right: Data) -> Data {
        precondition(left.count == right.count)
        var output = [UInt8](left)
        let subtrahend = [UInt8](right)
        var borrow = 0
        for index in output.indices.reversed() {
            let difference = Int(output[index]) - Int(subtrahend[index]) - borrow
            output[index] = UInt8(truncatingIfNeeded: difference)
            borrow = difference < 0 ? 1 : 0
        }
        precondition(borrow == 0)
        return Data(output)
    }

    private static func prefixedHashData(_ value: String) throws -> Data {
        guard value.count == 66, value.hasPrefix("0x"),
              let decoded = Data(hexString: String(value.dropFirst(2)))
        else { throw SccpV1Error.invalid("hash must be canonical 0x-prefixed hex") }
        return decoded
    }

    private static func g1(_ item: [String: Any], label: String) throws -> [Data] {
        try SccpStrictJSON.exactFields(item, ["x", "y"], label: label)
        let result = try ["x", "y"].map { try upperFixed(item, $0, bytes: 32, allowZero: true) }
        guard result.contains(where: { !$0.allSatisfy { $0 == 0 } }), result.allSatisfy(isBn254Field) else {
            throw SccpV1Error.invalid("\(label) is not a canonical non-infinity BN254 G1 point")
        }
        return result
    }

    private static func g2(_ item: [String: Any], label: String) throws -> [Data] {
        let fields = ["x_c0", "x_c1", "y_c0", "y_c1"]
        try SccpStrictJSON.exactFields(item, Set(fields), label: label)
        let result = try fields.map { try upperFixed(item, $0, bytes: 32, allowZero: true) }
        guard result.contains(where: { !$0.allSatisfy { $0 == 0 } }), result.allSatisfy(isBn254Field) else {
            throw SccpV1Error.invalid("\(label) is not a canonical non-infinity BN254 G2 point")
        }
        return result
    }

    private static func nativeAnchor(_ raw: Any?, lane: SccpLaneIdV1, label: String) throws -> SccpNativeTrustAnchorV1? {
        guard let raw, !(raw is NSNull) else { return nil }
        let item = try object(raw, label: label)
        try SccpStrictJSON.exactFields(item, ["backend", "anchor_hash", "checkpoint_height"], label: label)
        let backendObject = try object(item, "backend")
        try SccpStrictJSON.exactFields(backendObject, ["backend", "protocol"], label: "\(label).backend")
        guard backendObject["protocol"] is NSNull,
              let backend = SccpNativeBackendV1(rawValue: try SccpStrictJSON.text(backendObject, "backend")),
              backend.supports(lane.source)
        else { throw SccpV1Error.invalid("\(label) backend does not match its lane") }
        return SccpNativeTrustAnchorV1(
            backend: backend,
            anchorHash: try upperFixed(item, "anchor_hash", bytes: 32),
            checkpointHeight: try SccpStrictJSON.uint64(item, "checkpoint_height", minimum: 1)
        )
    }

    private static func sourceIdentity(_ item: [String: Any], expectedLane: SccpLaneIdV1, label: String) throws -> SccpSourceEmitterV1 {
        try SccpStrictJSON.exactFields(item, ["lane", "emitter"], label: label)
        guard try inboundLane(object(item, "lane"), label: "\(label).lane") == expectedLane else {
            throw SccpV1Error.invalid("\(label).lane does not match its route")
        }
        let emitter = try object(item, "emitter")
        try SccpStrictJSON.exactFields(emitter, ["emitter", "identity"], label: "\(label).emitter")
        let family = try SccpStrictJSON.text(emitter, "emitter")
        let identity = try object(emitter, "identity")
        switch family {
        case "evm" where !expectedLane.source.rawValue.hasPrefix("tron-"):
            try SccpStrictJSON.exactFields(identity, ["address", "runtime_code_hash", "route_config_hash"], label: "\(label).emitter.identity")
            let address = try upperFixed(identity, "address", bytes: 20)
            let runtime = try upperFixed(identity, "runtime_code_hash", bytes: 32)
            let configuration = try upperFixed(identity, "route_config_hash", bytes: 32)
            return try .validatedEvm(address: address, runtimeCodeHash: runtime, routeConfigHash: configuration)
        case "tron" where expectedLane.source.rawValue.hasPrefix("tron-"):
            try SccpStrictJSON.exactFields(identity, ["address", "runtime_code_hash", "route_config_hash"], label: "\(label).emitter.identity")
            let address = try upperFixed(identity, "address", bytes: 20)
            let runtime = try upperFixed(identity, "runtime_code_hash", bytes: 32)
            let configuration = try upperFixed(identity, "route_config_hash", bytes: 32)
            return try .validatedTron(address: address, runtimeCodeHash: runtime, routeConfigHash: configuration)
        case "ton" where expectedLane.source == .tonMainnet:
            try SccpStrictJSON.exactFields(identity, ["address", "code_hash", "route_config_hash"], label: "\(label).emitter.identity")
            return try .validatedTon(
                address: tonAddress(object(identity, "address"), label: "\(label).emitter.identity.address"),
                codeHash: upperFixed(identity, "code_hash", bytes: 32),
                routeConfigHash: upperFixed(identity, "route_config_hash", bytes: 32)
            )
        default: throw SccpV1Error.invalid("\(label) emitter family does not match its lane")
        }
    }

    private static func sourceMatchesDestination(
        _ source: SccpSourceEmitterV1,
        _ destination: SccpDestinationDeploymentV1
    ) -> Bool {
        switch (source, destination) {
        case let (.evm(address, runtime, _), .evm(deployment)),
             let (.tron(address, runtime, _), .tron(deployment)):
            return address == deployment.routeAddress && runtime == deployment.routeCodeHash
        case let (.ton(address, codeHash, _), .ton(deployment)):
            return address == deployment.routeAddress && codeHash == deployment.routeCodeHash
        default:
            return false
        }
    }

    private static func sourceRouteConfigHash(_ source: SccpSourceEmitterV1) -> Data {
        switch source {
        case let .evm(_, _, value), let .tron(_, _, value), let .ton(_, _, value): value
        }
    }

    private static func activation(_ item: [String: Any], label: String) throws -> SccpRouteActivationV1 {
        try SccpStrictJSON.exactFields(item, ["activation", "direction"], label: label)
        guard item["direction"] is NSNull,
              let activation = SccpRouteActivationV1(rawValue: try SccpStrictJSON.text(item, "activation"))
        else { throw SccpV1Error.invalid("\(label) is unsupported") }
        return activation
    }

    private static func destinationBackend(_ item: [String: Any], label: String) throws -> SccpDestinationProofBackendV1 {
        try SccpStrictJSON.exactFields(item, ["backend", "family"], label: label)
        guard item["family"] is NSNull,
              let backend = SccpDestinationProofBackendV1(rawValue: try SccpStrictJSON.text(item, "backend"))
        else { throw SccpV1Error.invalid("\(label) is unsupported or retired") }
        return backend
    }

    private static func network(_ item: [String: Any], label: String) throws -> SccpNetworkV1 {
        try SccpStrictJSON.exactFields(item, ["network", "profile"], label: label)
        guard item["profile"] is NSNull else { throw SccpV1Error.invalid("\(label).profile must be null") }
        let wire = try SccpStrictJSON.text(item, "network")
        guard wire.utf8.allSatisfy({ (97...122).contains($0) || $0 == 95 }),
              let value = SccpNetworkV1(rawValue: wire.replacingOccurrences(of: "_", with: "-"))
        else { throw SccpV1Error.invalid("\(label) is unsupported or retired") }
        return value
    }

    private static func inboundLane(_ item: [String: Any], label: String) throws -> SccpLaneIdV1 {
        let lane = try lane(item, label: label)
        guard lane.isInbound, lane.target == .soraTaira else { throw SccpV1Error.invalid("\(label) must be external-to-Taira") }
        return lane
    }

    private static func outboundLane(_ item: [String: Any], label: String) throws -> SccpLaneIdV1 {
        let lane = try lane(item, label: label)
        guard lane.isOutbound, lane.source == .soraTaira else { throw SccpV1Error.invalid("\(label) must be Taira-to-external") }
        return lane
    }

    private static func lane(_ item: [String: Any], label: String) throws -> SccpLaneIdV1 {
        try SccpStrictJSON.exactFields(item, ["source", "target"], label: label)
        return try SccpLaneIdV1(
            source: network(object(item, "source"), label: "\(label).source"),
            target: network(object(item, "target"), label: "\(label).target")
        )
    }

    private static func transferPayload(_ item: [String: Any], lane: SccpLaneIdV1) throws {
        try SccpStrictJSON.exactFields(item, [
            "version", "source_domain", "dest_domain", "nonce", "route_revision", "asset_home_domain",
            "asset_id_codec", "asset_id", "amount", "sender_codec", "sender", "recipient_codec",
            "recipient", "route_id_codec", "route_id",
        ], label: "SCCP transfer payload")
        guard try SccpStrictJSON.uint64(item, "version", minimum: 1) == 1,
              try SccpStrictJSON.uint32(item, "source_domain", minimum: 0, maximum: 4) == lane.source.domainId,
              try SccpStrictJSON.uint32(item, "dest_domain", minimum: 0, maximum: 4) == lane.target.domainId,
              try SccpStrictJSON.uint32(item, "route_revision", minimum: 1, maximum: UInt32.max) > 0
        else { throw SccpV1Error.invalid("SCCP transfer payload does not match its exact lane") }
        _ = try decimalUInt64(item, "nonce", minimum: 0)
        let assetHomeDomain = try SccpStrictJSON.uint32(item, "asset_home_domain", minimum: 0, maximum: 5)
        guard [UInt32(0), 1, 2, 4, 5].contains(assetHomeDomain) else {
            throw SccpV1Error.invalid("SCCP transfer asset_home_domain is unsupported or retired")
        }
        let amount = try decimalText(item, "amount", minimum: 1)
        let maximumUInt128 = "340282366920938463463374607431768211455"
        guard amount.count < maximumUInt128.count || amount.count == maximumUInt128.count && amount <= maximumUInt128 else {
            throw SccpV1Error.invalid("SCCP transfer amount must fit UInt128")
        }
        let senderCodec = try SccpStrictJSON.uint32(item, "sender_codec", minimum: 1, maximum: 7)
        let recipientCodec = try SccpStrictJSON.uint32(item, "recipient_codec", minimum: 1, maximum: 7)
        let expectedRecipientCodec: UInt32
        switch lane.target.domainId {
        case 4: expectedRecipientCodec = 7
        case 5: expectedRecipientCodec = 5
        default: expectedRecipientCodec = 2
        }
        guard senderCodec == 1, recipientCodec == expectedRecipientCodec else {
            throw SccpV1Error.invalid("SCCP transfer account codecs do not match its exact domains")
        }
        for (codecField, valueField) in [
            ("asset_id_codec", "asset_id"), ("sender_codec", "sender"),
            ("recipient_codec", "recipient"), ("route_id_codec", "route_id"),
        ] {
            let codec = try SccpStrictJSON.uint32(item, codecField, minimum: 1, maximum: 7)
            guard let exactCodec = SccpCodecV1(rawValue: UInt8(codec)) else { throw SccpV1Error.invalid("SCCP transfer uses a retired codec") }
            _ = try exactCodec.validate(variableHex(item, valueField))
        }
    }

    private static func validateLineages(_ routes: [SccpGovernedRouteV1]) throws {
        let grouped = Dictionary(grouping: routes) { "\($0.routeId)\0\($0.assetKey)" }
        for revisions in grouped.values {
            let ordered = revisions.sorted { $0.revision < $1.revision }
            for (index, route) in ordered.enumerated() where route.revision != UInt32(index + 1) {
                throw SccpV1Error.invalid("SCCP route revisions must start at one and have no gaps")
            }
            guard ordered.filter({ $0.activation.allowsOutbound }).count <= 1 else {
                throw SccpV1Error.invalid("SCCP registry enables multiple revisions of one route")
            }
        }
    }

    static func destinationBindingHash(lane: SccpLaneIdV1, destination: SccpDestinationDeploymentV1) throws -> Data {
        if case let .ton(ton) = destination {
            let globalId: Int32
            switch lane.source {
            case .tonMainnet: globalId = -239
            default: throw SccpV1Error.invalid("TON binding requires a TON lane")
            }
            var payload = Data("iroha:sccp:ton-destination-binding:v1".utf8)
            payload.append(1)
            appendBytes(Data("ton-groth16-bls12381-v1".utf8), to: &payload)
            appendBytes(SccpV1.canonicalNetworkBytes(lane.source), to: &payload)
            appendInt32LE(globalId, to: &payload)
            appendUInt32LE(0, to: &payload)
            appendUInt32LE(4, to: &payload)
            payload.append(ton.jettonMasterCodeHash)
            payload.append(ton.jettonWalletCodeHash)
            payload.append(ton.routeCodeHash)
            payload.append(ton.embeddedVerifierCodeHash)
            payload.append(ton.verifierCircuitHash)
            payload.append(ton.verifierKeyHash)
            payload.append(ton.proofProfileCommitment)
            for guardian in ton.mintBreakerGuardianKeys.ordered {
                payload.append(guardian)
            }
            payload.append(ton.outboundProofPolicy.semanticProfile.profileHash)
            payload.append(ton.outboundProofPolicy.soraFinalityAnchor.anchorHash)
            return Data(SHA256.hash(data: payload))
        }

        let evmTron: SccpEvmTronDestinationDeploymentV1
        let isTron: Bool
        switch destination {
        case let .evm(value): evmTron = value; isTron = false
        case let .tron(value): evmTron = value; isTron = true
        case .ton: preconditionFailure("handled above")
        }
        let domain = lane.source.domainId
        let networkWord: Data
        if isTron {
            let networkId: UInt32
            switch lane.source {
            case .tronMainnet: networkId = 0x2b66_53dc
            default: throw SccpV1Error.invalid("TRON binding requires a TRON lane")
            }
            networkWord = abiWord(UInt64(networkId))
        } else {
            let chainId: UInt64
            switch lane.source {
            case .ethereumMainnet: chainId = 1
            case .bscMainnet: chainId = 56
            default: throw SccpV1Error.invalid("EVM binding requires an EVM lane")
            }
            networkWord = abiWord(chainId)
        }
        let bindingDomain = isTron ? "iroha:sccp:tron-destination-binding:v1" : "iroha:sccp:evm-destination-binding:v1"
        let backend = isTron ? "tron-groth16-bn254-v1" : "evm-groth16-bn254-v1"
        var payload = irohaKeccak256(Data(bindingDomain.utf8))
        payload.append(irohaKeccak256(Data(backend.utf8)))
        payload.append(networkWord)
        payload.append(abiWord(0))
        payload.append(abiWord(UInt64(domain)))
        payload.append(isTron ? abiTronAddress(evmTron.verifierAddress) : abiAddress(evmTron.verifierAddress))
        payload.append(isTron ? abiTronAddress(evmTron.routeAddress) : abiAddress(evmTron.routeAddress))
        payload.append(evmTron.verifierCodeHash)
        payload.append(evmTron.verifierKeyHash)
        payload.append(evmTron.outboundProofPolicy.semanticProfile.profileHash)
        payload.append(evmTron.outboundProofPolicy.soraFinalityAnchor.anchorHash)
        payload.append(isTron ? abiTronAddress(evmTron.replayVerifierAddress) : abiAddress(evmTron.replayVerifierAddress))
        payload.append(evmTron.replayVerifierCodeHash)
        payload.append(isTron ? abiTronAddress(evmTron.mintBreakerAddress) : abiAddress(evmTron.mintBreakerAddress))
        payload.append(evmTron.mintBreakerCodeHash)
        return irohaKeccak256(payload)
    }

    static func routeConfigurationHash(
        lane: SccpLaneIdV1,
        routeId: String,
        assetKey: String,
        revision: UInt32,
        destination: SccpDestinationDeploymentV1
    ) throws -> Data {
        guard assetKey == "xor" else { throw SccpV1Error.invalid("SCCP V1 route asset must be xor") }
        if case let .ton(ton) = destination {
            guard routeId == "taira_ton_xor" else {
                throw SccpV1Error.invalid("SCCP TON route id must be taira_ton_xor")
            }
            let globalId: Int32
            switch lane.source {
            case .tonMainnet: globalId = -239
            default: throw SccpV1Error.invalid("TON route requires a TON lane")
            }
            let sourceHash = SccpV1.laneHash(lane)
            let reverseHash = SccpV1.laneHash(try SccpLaneIdV1(source: lane.target, target: lane.source))
            let binding = ton.destinationBindingHash
            let semantic = ton.outboundProofPolicy.semanticProfile.profileHash
            let anchor = ton.outboundProofPolicy.soraFinalityAnchor.anchorHash
            let roles = [sourceHash, reverseHash, ton.jettonMasterCodeHash,
                         ton.jettonMasterInitialDataHash, ton.jettonWalletCodeHash,
                         ton.routeCodeHash, ton.routeInitialDataHash,
                         ton.embeddedVerifierCodeHash, ton.verifierCircuitHash,
                         ton.verifierKeyHash, ton.proofProfileCommitment, semantic, anchor, binding]
            guard Set(roles).count == roles.count else {
                throw SccpV1Error.invalid("SCCP TON route reuses a hash role")
            }
            var deployment = Data()
            deployment.append(ton.jettonMasterCodeHash)
            deployment.append(ton.jettonWalletCodeHash)
            deployment.append(ton.routeCodeHash)
            deployment.append(ton.embeddedVerifierCodeHash)
            deployment.append(ton.verifierCircuitHash)
            deployment.append(ton.verifierKeyHash)
            deployment.append(ton.proofProfileCommitment)
            for guardian in ton.mintBreakerGuardianKeys.ordered {
                deployment.append(guardian)
            }
            deployment.append(semantic)
            deployment.append(anchor)
            deployment.append(binding)
            let deploymentHash = Data(SHA256.hash(data: deployment))
            var assetRoute = Data()
            appendBytes(Data("xor".utf8), to: &assetRoute)
            appendBytes(Data("taira_ton_xor".utf8), to: &assetRoute)
            appendUInt32LE(revision, to: &assetRoute)
            appendUInt64LE(ton.tairaToTokenMultiplier, to: &assetRoute)
            try appendUInt128LE(ton.maxWrappedSupply, to: &assetRoute)
            let assetRouteHash = Data(SHA256.hash(data: assetRoute))
            var payload = Data("sccp:concrete-route-config:v1".utf8)
            payload.append(1)
            appendUInt32LE(4, to: &payload)
            appendBytes(SccpV1.canonicalNetworkBytes(lane.source), to: &payload)
            appendInt32LE(globalId, to: &payload)
            payload.append(sourceHash)
            payload.append(reverseHash)
            payload.append(deploymentHash)
            payload.append(assetRouteHash)
            return Data(SHA256.hash(data: payload))
        }

        let evmTron: SccpEvmTronDestinationDeploymentV1
        switch destination {
        case let .evm(value), let .tron(value): evmTron = value
        case .ton: preconditionFailure("handled above")
        }
        let expectedRoute: String
        let networkValue: UInt64
        switch lane.source {
        case .ethereumMainnet: expectedRoute = "taira_eth_xor"; networkValue = 1
        case .bscMainnet: expectedRoute = "taira_bsc_xor"; networkValue = 56
        case .tronMainnet: expectedRoute = "taira_tron_xor"; networkValue = 0x2b66_53dc
        default: throw SccpV1Error.invalid("SCCP route external profile is unsupported")
        }
        guard routeId == expectedRoute else { throw SccpV1Error.invalid("SCCP route id does not match its exact deployment") }
        let sourceHash = SccpV1.laneHash(lane)
        let reverseHash = SccpV1.laneHash(try SccpLaneIdV1(source: lane.target, target: lane.source))
        var roles = [sourceHash, reverseHash, evmTron.tokenCodeHash, evmTron.verifierCodeHash,
                     evmTron.verifierKeyHash, evmTron.routeCodeHash,
                     evmTron.replayVerifierCodeHash, evmTron.mintBreakerCodeHash,
                     evmTron.outboundProofPolicy.semanticProfile.profileHash,
                     evmTron.outboundProofPolicy.soraFinalityAnchor.anchorHash]
        if destination.family == .tronGroth16Bn254 {
            roles.append(evmTron.destinationBindingHash)
        }
        guard Set(roles).count == roles.count else { throw SccpV1Error.invalid("SCCP route reuses a hash role") }
        var deployment = abiAddress(evmTron.tokenAddress)
        deployment.append(evmTron.tokenCodeHash)
        deployment.append(abiAddress(evmTron.verifierAddress))
        deployment.append(evmTron.verifierCodeHash)
        deployment.append(evmTron.verifierKeyHash)
        deployment.append(evmTron.outboundProofPolicy.semanticProfile.profileHash)
        deployment.append(evmTron.outboundProofPolicy.soraFinalityAnchor.anchorHash)
        if destination.family == .tronGroth16Bn254 { deployment.append(evmTron.destinationBindingHash) }
        deployment.append(abiAddress(evmTron.replayVerifierAddress))
        deployment.append(evmTron.replayVerifierCodeHash)
        deployment.append(abiAddress(evmTron.mintBreakerAddress))
        deployment.append(evmTron.mintBreakerCodeHash)
        let deploymentHash = irohaKeccak256(deployment)
        var assetRoute = irohaKeccak256(Data("xor".utf8))
        assetRoute.append(irohaKeccak256(Data(routeId.utf8)))
        assetRoute.append(abiWord(UInt64(revision)))
        assetRoute.append(abiWord(evmTron.tairaToTokenMultiplier))
        assetRoute.append(try abiWord(evmTron.maxWrappedSupply))
        let assetRouteHash = irohaKeccak256(assetRoute)
        var payload = irohaKeccak256(Data("sccp:concrete-route-config:v1".utf8))
        payload.append(abiWord(UInt64(lane.source.domainId)))
        payload.append(abiWord(UInt64(lane.source.tag)))
        payload.append(abiWord(networkValue))
        payload.append(sourceHash)
        payload.append(reverseHash)
        payload.append(deploymentHash)
        payload.append(assetRouteHash)
        return irohaKeccak256(payload)
    }

    private static func publicSignalSchemaHash() -> Data {
        var canonical = Data([1])
        appendUInt32LE(UInt32(publicSignalLabels.count), to: &canonical)
        for label in publicSignalLabels {
            let bytes = Data(label.utf8)
            appendUInt32LE(UInt32(bytes.count), to: &canonical)
            canonical.append(bytes)
        }
        return irohaKeccak256(Data("sccp:groth16-bn254:public-signal-schema:v1".utf8) + canonical)
    }

    private static func bls12381PublicSignalSchemaHash() -> Data {
        var canonical = Data([1])
        appendUInt32LE(UInt32(bls12381PublicSignalLabels.count), to: &canonical)
        for label in bls12381PublicSignalLabels {
            appendBytes(Data(label.utf8), to: &canonical)
        }
        return Data(SHA256.hash(
            data: Data("sccp:groth16-bls12381:public-signal-schema:v1".utf8) + canonical
        ))
    }

    private static func tonProofProfileCommitment() -> Data {
        var preimage = Data("sccp:ton:groth16-bls12381:proof-profile:v1".utf8)
        preimage.append(1)
        preimage.append(Data("ietf-bls12381-compressed-g1-48-g2-96".utf8))
        preimage.append(Data("groth16-a-g1-b-g2-c-g1".utf8))
        preimage.append(Data("sha256-sha256-label-value-mod-r".utf8))
        preimage.append(bls12381ScalarField)
        preimage.append(bls12381PublicSignalSchemaHash())
        return Data(SHA256.hash(data: preimage))
    }

    private static func isBls12381G1Compressed(_ value: Data) -> Bool {
        guard value.count == 48, value[0] & 0x80 != 0, value[0] & 0x40 == 0 else {
            return false
        }
        var x = value
        x[0] &= 0x1f
        return x.lexicographicallyPrecedes(bls12381BaseField)
    }

    private static func isBls12381G2Compressed(_ value: Data) -> Bool {
        guard value.count == 96 else { return false }
        return isBls12381G1Compressed(Data(value.prefix(48)))
            && Data(value.suffix(48)).lexicographicallyPrecedes(bls12381BaseField)
    }

    private static func tonAddress(_ item: [String: Any], label: String) throws -> SccpTonAddressV1 {
        try SccpStrictJSON.exactFields(item, ["workchain", "account"], label: label)
        let address = try SccpTonAddressV1(
            workchain: signedInt32(item, "workchain"),
            account: upperFixed(item, "account", bytes: 32)
        )
        guard address.isSccpBasechainContract else {
            throw SccpV1Error.invalid("\(label) must be a TON basechain contract")
        }
        return address
    }

    private static func isBn254Field(_ value: Data) -> Bool {
        value != bn254BaseField && value.lexicographicallyPrecedes(bn254BaseField)
    }

    private static func abiAddress(_ value: Data) -> Data { Data(repeating: 0, count: 12) + value }
    private static func abiTronAddress(_ value: Data) -> Data { Data(repeating: 0, count: 11) + Data([0x41]) + value }
    private static func abiWord(_ value: UInt64) -> Data {
        var out = Data(repeating: 0, count: 24)
        var big = value.bigEndian
        withUnsafeBytes(of: &big) { out.append(contentsOf: $0) }
        return out
    }

    private static func abiWord(_ value: String) throws -> Data {
        let little = try uint128LittleEndian(value)
        return Data(repeating: 0, count: 16) + Data(little.reversed())
    }

    private static func appendUInt16LE(_ value: UInt16, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }

    private static func appendUInt32LE(_ value: UInt32, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }

    private static func appendInt32LE(_ value: Int32, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }

    private static func appendUInt64LE(_ value: UInt64, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }

    private static func appendUInt128LE(_ value: String, to out: inout Data) throws {
        out.append(contentsOf: try uint128LittleEndian(value))
    }

    private static func uint128LittleEndian(_ value: String) throws -> [UInt8] {
        guard !value.isEmpty,
              value.first != "0",
              value.utf8.allSatisfy({ (48...57).contains($0) }),
              value.count < maximumUInt128.count
                  || value.count == maximumUInt128.count && value <= maximumUInt128
        else { throw SccpV1Error.invalid("value must be a canonical positive UInt128") }
        var digits = value.utf8.map { Int($0 - 48) }
        var bytes: [UInt8] = []
        while !digits.isEmpty {
            var quotient: [Int] = []
            var remainder = 0
            for digit in digits {
                let current = remainder * 10 + digit
                let next = current / 256
                remainder = current % 256
                if !quotient.isEmpty || next != 0 { quotient.append(next) }
            }
            bytes.append(UInt8(remainder))
            digits = quotient
        }
        guard bytes.count <= 16 else { throw SccpV1Error.invalid("value exceeds UInt128") }
        bytes.append(contentsOf: repeatElement(0, count: 16 - bytes.count))
        return bytes
    }

    private static func appendBytes(_ value: Data, to out: inout Data) {
        appendUInt32LE(UInt32(value.count), to: &out)
        out.append(value)
    }

    private static func appendTonRegistryAddress(_ value: SccpTonAddressV1, to out: inout Data) {
        appendInt32LE(value.workchain, to: &out)
        out.append(value.account)
    }

    private static func signedInt32(_ item: [String: Any], _ field: String) throws -> Int32 {
        guard let number = item[field] as? NSNumber,
              CFGetTypeID(number) != CFBooleanGetTypeID(),
              let value = Int32(number.stringValue),
              String(value) == number.stringValue
        else { throw SccpV1Error.invalid("\(field) must be a canonical signed Int32") }
        return value
    }

    private static func routeKey(_ item: [String: Any], _ field: String) throws -> String {
        let value = try SccpStrictJSON.text(item, field)
        guard value.utf8.count <= 64, value.unicodeScalars.allSatisfy(routeKeyCharacters.contains),
              value.first?.isNumber == true || value.first?.isLowercase == true,
              value.last?.isNumber == true || value.last?.isLowercase == true
        else { throw SccpV1Error.invalid("\(field) must be canonical lowercase route text") }
        return value
    }

    private static func fixedPath(_ item: [String: Any], _ field: String) throws -> String {
        let value = try path(item, field)
        guard value == capabilityPaths[field] else { throw SccpV1Error.invalid("\(field) does not match the SCCP V1 endpoint") }
        return value
    }

    private static func optionalFixedPath(_ item: [String: Any], _ field: String) throws -> String? {
        guard let raw = item[field], !(raw is NSNull) else { return nil }
        guard raw is String else { throw SccpV1Error.invalid("\(field) must be a path or null") }
        return try fixedPath(item, field)
    }

    private static func path(_ item: [String: Any], _ field: String) throws -> String {
        let value = try SccpStrictJSON.text(item, field)
        guard value.first == "/", !value.contains("//"), !value.contains("?"), !value.contains("#"),
              !value.contains("%"), !value.contains("\\"), value.utf8.count <= 1024
        else { throw SccpV1Error.invalid("\(field) must be a canonical absolute Torii path") }
        return value
    }

    private static func prefixedHash(
        _ item: [String: Any],
        _ field: String,
        allowZero: Bool = false
    ) throws -> String {
        let value = try SccpStrictJSON.text(item, field)
        guard value.count == 66, value.hasPrefix("0x"),
              value.dropFirst(2).utf8.allSatisfy({
                  (48...57).contains($0) || (97...102).contains($0)
              }),
              allowZero || value.dropFirst(2).contains(where: { $0 != "0" })
        else {
            let qualifier = allowZero ? "" : " nonzero"
            throw SccpV1Error.invalid("\(field) must be canonical lowercase\(qualifier) 0x-prefixed hash")
        }
        return value
    }

    private static func unprefixedHash(_ item: [String: Any], _ field: String) throws -> String {
        let value = try SccpStrictJSON.text(item, field)
        _ = try SccpSubmitValidation.responseHash(value, field: field)
        return value
    }

    private static func distinctHexRoles(_ values: [String], label: String) throws {
        guard Set(values).count == values.count else { throw SccpV1Error.invalid("\(label) must be role-separated") }
    }

    private static func upperFixed(_ item: [String: Any], _ field: String, bytes: Int, allowZero: Bool = false) throws -> Data {
        let value = try SccpStrictJSON.text(item, field)
        guard value.count == bytes * 2,
              value.utf8.allSatisfy({ (48...57).contains($0) || (65...70).contains($0) }),
              (allowZero || value.contains(where: { $0 != "0" })),
              let data = Data(hexString: value)
        else { throw SccpV1Error.invalid("\(field) must be canonical uppercase \(bytes)-byte hex") }
        return data
    }

    private static func requireRuntimeCodeHash(_ value: Data, field: String) throws {
        guard value != keccak256EmptyBytes else {
            throw SccpV1Error.invalid("\(field) must not identify empty runtime bytecode")
        }
    }

    private static func unsignedIntegerText(
        _ item: [String: Any],
        _ field: String,
        maximum: String
    ) throws -> String {
        let value: String
        if let exact = item[field] as? SccpStrictJSON.ExactUnsignedInteger {
            value = exact.text
        } else if let number = item[field] as? NSNumber,
                  CFGetTypeID(number) != CFBooleanGetTypeID(),
                  !CFNumberIsFloatType(number)
        {
            value = number.stringValue
        } else {
            throw SccpV1Error.invalid("\(field) must be a canonical positive integer")
        }
        guard !value.isEmpty,
              value.first != "0",
              value.utf8.allSatisfy({ (48...57).contains($0) }),
              value.count < maximum.count || value.count == maximum.count && value <= maximum
        else { throw SccpV1Error.invalid("\(field) is outside its canonical positive range") }
        return value
    }

    private static func variableHex(_ item: [String: Any], _ field: String) throws -> Data {
        let value = try SccpStrictJSON.text(item, field)
        guard value.hasPrefix("0x"), value.count > 2, (value.count - 2).isMultiple(of: 2),
              value.dropFirst(2).allSatisfy({ $0.isNumber || ("a"..."f").contains(String($0)) }),
              (value.count - 2) / 2 <= 16 * 1024 * 1024,
              let data = Data(hexString: String(value.dropFirst(2))), !data.isEmpty
        else { throw SccpV1Error.invalid("\(field) must be canonical nonempty lowercase 0x-prefixed hex") }
        return data
    }

    private static func decimalText(_ item: [String: Any], _ field: String, minimum: UInt64) throws -> String {
        let value = try SccpStrictJSON.text(item, field)
        guard !value.isEmpty,
              value.utf8.allSatisfy({ (48...57).contains($0) }),
              value == "0" || value.first != "0",
              value != "0" || minimum == 0
        else { throw SccpV1Error.invalid("\(field) must be canonical unsigned decimal") }
        return value
    }

    private static func decimalUInt64(_ item: [String: Any], _ field: String, minimum: UInt64) throws -> UInt64 {
        let value = try decimalText(item, field, minimum: minimum)
        guard let parsed = UInt64(value), parsed >= minimum, String(parsed) == value else {
            throw SccpV1Error.invalid("\(field) must fit UInt64")
        }
        return parsed
    }

    private static func optionalText(_ item: [String: Any], _ field: String) throws -> String? {
        guard let raw = item[field], !(raw is NSNull) else { return nil }
        guard raw is String else { throw SccpV1Error.invalid("\(field) must be text or null") }
        return try SccpStrictJSON.text(item, field)
    }

    private static func object(_ item: [String: Any], _ field: String) throws -> [String: Any] {
        guard let value = item[field] as? [String: Any] else { throw SccpV1Error.invalid("\(field) must be an object") }
        return value
    }

    private static func payloadProjectionData(
        _ item: [String: Any],
        field: String,
        expectedDestinationDomain: UInt32,
        expectedAmount: String,
        label: String
    ) throws -> Data {
        let projection = try object(item, field)
        try SccpStrictJSON.exactFields(projection, ["Transfer"], label: label)
        let transfer = try object(projection, "Transfer")
        let fields: Set<String> = [
            "version", "source_domain", "dest_domain", "nonce", "route_revision",
            "asset_home_domain", "asset_id", "amount", "sender", "recipient", "route_id",
        ]
        try SccpStrictJSON.exactFields(transfer, fields, label: "\(label).Transfer")
        guard try SccpStrictJSON.uint64(transfer, "version", minimum: 1) == 1,
              try SccpStrictJSON.uint32(transfer, "source_domain", minimum: 0, maximum: 0) == 0,
              try SccpStrictJSON.uint32(transfer, "dest_domain", minimum: 1, maximum: 4) == expectedDestinationDomain,
              try SccpStrictJSON.uint32(transfer, "asset_home_domain", minimum: 0, maximum: 0) == 0
        else { throw SccpV1Error.invalid("\(label).Transfer domains or version do not match the recent message") }
        guard let nonceNumber = transfer["nonce"] as? NSNumber,
              CFGetTypeID(nonceNumber) != CFBooleanGetTypeID(),
              let nonce = UInt64(nonceNumber.stringValue),
              nonce.description == nonceNumber.stringValue
        else { throw SccpV1Error.invalid("\(label).Transfer.nonce must be a canonical UInt64 integer") }
        _ = try SccpStrictJSON.uint32(transfer, "route_revision", minimum: 1, maximum: UInt32.max)

        let maximumUInt128 = "340282366920938463463374607431768211455"
        let amount = try unsignedIntegerText(
            transfer,
            "amount",
            maximum: maximumUInt128
        )
        guard amount == expectedAmount
        else { throw SccpV1Error.invalid("\(label).Transfer.amount must equal the positive UInt128 readback amount") }

        try normalizedCanonicalText(transfer, field: "asset_id", expected: "xor", label: label)
        try normalizedCanonicalText(transfer, field: "sender", expected: nil, label: label)
        let expectedRoute: String
        switch expectedDestinationDomain {
        case 1: expectedRoute = "taira_eth_xor"
        case 2: expectedRoute = "taira_bsc_xor"
        case 4: expectedRoute = "taira_ton_xor"
        case 5: expectedRoute = "taira_tron_xor"
        default: throw SccpV1Error.invalid("\(label).Transfer destination domain is unsupported")
        }
        try normalizedCanonicalText(transfer, field: "route_id", expected: expectedRoute, label: label)
        try normalizedRecipient(
            transfer,
            field: "recipient",
            destinationDomain: expectedDestinationDomain,
            label: label
        )
        return try SccpStrictJSON.canonicalData(projection)
    }

    private static func normalizedCanonicalText(
        _ item: [String: Any],
        field: String,
        expected: String?,
        label: String
    ) throws {
        let tagged = try object(item, field)
        try SccpStrictJSON.exactFields(tagged, ["CanonicalText"], label: "\(label).Transfer.\(field)")
        let content = try object(tagged, "CanonicalText")
        try SccpStrictJSON.exactFields(content, ["value"], label: "\(label).Transfer.\(field).CanonicalText")
        let value = try SccpStrictJSON.text(content, "value")
        guard (try? SccpCodecV1.canonicalText.validate(Data(value.utf8))) != nil,
              expected.map({ value == $0 }) ?? true
        else { throw SccpV1Error.invalid("\(label).Transfer.\(field) is not canonical text") }
    }

    private static func normalizedRecipient(
        _ item: [String: Any],
        field: String,
        destinationDomain: UInt32,
        label: String
    ) throws {
        let tagged = try object(item, field)
        if destinationDomain == 4 {
            try SccpStrictJSON.exactFields(tagged, ["TonAccount36"], label: "\(label).Transfer.\(field)")
            let content = try object(tagged, "TonAccount36")
            try SccpStrictJSON.exactFields(
                content,
                ["workchain", "account"],
                label: "\(label).Transfer.\(field).TonAccount36"
            )
            guard try signedInt32(content, "workchain") == 0 else {
                throw SccpV1Error.invalid("\(label).Transfer.\(field) must use TON basechain workchain 0")
            }
            let account = try SccpStrictJSON.text(content, "account")
            guard account.count == 66, account.hasPrefix("0x"),
                  account.dropFirst(2).allSatisfy({ $0.isNumber || ("a"..."f").contains(String($0)) }),
                  account.dropFirst(2).contains(where: { $0 != "0" })
            else { throw SccpV1Error.invalid("\(label).Transfer.\(field) is not a canonical TON account") }
            return
        }
        let tag = destinationDomain == 3 ? "TronAddress21" : "EvmAddress20"
        try SccpStrictJSON.exactFields(tagged, [tag], label: "\(label).Transfer.\(field)")
        let content = try object(tagged, tag)
        try SccpStrictJSON.exactFields(content, ["bytes"], label: "\(label).Transfer.\(field).\(tag)")
        let bytes = try SccpStrictJSON.text(content, "bytes")
        let hex = bytes.dropFirst(2)
        let expectedHexCount = destinationDomain == 3 ? 42 : 40
        guard bytes.hasPrefix(destinationDomain == 3 ? "0x41" : "0x"),
              hex.count == expectedHexCount,
              hex.allSatisfy({ $0 >= "0" && $0 <= "9" || $0 >= "a" && $0 <= "f" }),
              (destinationDomain == 3 ? hex.dropFirst(2) : hex).contains(where: { $0 != "0" })
        else { throw SccpV1Error.invalid("\(label).Transfer.\(field) does not match its destination address codec") }
    }

    private static func object(_ raw: Any, label: String) throws -> [String: Any] {
        guard let value = raw as? [String: Any] else { throw SccpV1Error.invalid("\(label) must be an object") }
        return value
    }

    private static func array(_ item: [String: Any], _ field: String) throws -> [Any] {
        guard let value = item[field] as? [Any] else { throw SccpV1Error.invalid("\(field) must be an array") }
        return value
    }
}
