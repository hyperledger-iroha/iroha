import Foundation

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
    public let maxBn254PairingChecksPerTransaction: UInt32
    public let maxBn254PairingChecksPerBlock: UInt32
}

/// Stable first-release SCCP HTTP surface. Every path is fixed and query-free except recent-message pagination.
public struct SccpCapabilities: Equatable, Sendable {
    public let version: UInt8
    public let registryRevision: String
    public let registryPath: String
    public let messageBundlePath: String
    public let proofRequestPath: String
    public let recentMessagesPath: String
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

/// Immutable commitments identifying the only semantic circuit accepted by SCCP V1.
public struct SccpSemanticProofProfileV1: Equatable, Sendable {
    public let circuitCommitment: Data
    public let witnessGeneratorCommitment: Data
    public let publicSignalSchemaHash: Data
    public let profileHash: Data
}

/// Immutable Taira finality checkpoint exposed as Groth16 public signal 10.
public struct SccpSoraFinalityAnchorV1: Equatable, Sendable {
    public let chainIdHash: Data
    public let checkpointHeight: UInt64
    public let checkpointBlockHash: Data
    public let validatorSetEpoch: UInt64
    public let validatorSetHash: Data
    public let validatorSetHashVersion: UInt16
    public let anchorHash: Data
}

/// Exact governed destination deployment summary. The full 38-word key was validated before construction.
public struct SccpDestinationDeploymentV1: Equatable, Sendable {
    public let family: SccpDestinationProofBackendV1
    public let tokenAddress: Data
    public let tokenCodeHash: Data
    public let verifierAddress: Data
    public let verifierCodeHash: Data
    public let verifierKeyHash: Data
    public let semanticProofProfile: SccpSemanticProofProfileV1
    public let soraFinalityAnchor: SccpSoraFinalityAnchorV1
    public let routeAddress: Data
    public let routeCodeHash: Data
    public let tairaToTokenMultiplier: UInt64
    public let destinationBindingHash: Data
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
    public let assetDefinitionId: String
    public let custodyAccountId: String
    public let payloadAmountScale: UInt32
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
    public let verifierKeyHash: String
    public let semanticProofProfile: SccpSemanticProofProfileV1
    public let soraFinalityAnchor: SccpSoraFinalityAnchorV1
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

public struct SccpRecentMessages: Equatable, Sendable {
    public let items: [SccpRecentMessage]

    public static func parse(_ data: Data) throws -> Self {
        try SccpExactParser.recent(data)
    }
}

private enum SccpExactParser {
    private static let jsonSafeIntegerMaximum: UInt64 = (1 << 53) - 1

    private static let capabilityPaths: [String: String] = [
        "registry_path": "/v1/sccp/registry",
        "message_bundle_path": "/v1/sccp/proofs/message/{message_id}",
        "proof_request_path": "/v1/sccp/proof-requests/{message_id}",
        "recent_messages_path": "/v1/sccp/messages/recent",
        "proof_submit_path": "/v1/bridge/proofs/submit",
        "native_message_submit_path": "/v1/bridge/messages",
    ]
    private static let routeKeyCharacters = CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyz0123456789_-")
    private static let bn254BaseField = Data([
        0x30, 0x64, 0x4e, 0x72, 0xe1, 0x31, 0xa0, 0x29, 0xb8, 0x50, 0x45, 0xb6, 0x81, 0x81, 0x58, 0x5d,
        0x97, 0x81, 0x6a, 0x91, 0x68, 0x71, 0xca, 0x8d, 0x3c, 0x20, 0x8c, 0x16, 0xd8, 0x7c, 0xfd, 0x47,
    ])
    private static let tairaChainId = Data([
        0x80, 0x95, 0x74, 0xf5, 0xfe, 0xe7, 0x5e, 0x69, 0xbf, 0xcf, 0x52, 0x45, 0x1e, 0x42, 0xd5, 0x0f,
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

    static func capabilities(_ data: Data) throws -> SccpCapabilities {
        let root = try SccpStrictJSON.object(data, label: "SCCP capabilities")
        let required: Set<String> = [
            "version", "registry_revision", "registry_path", "message_bundle_path",
            "proof_request_path", "recent_messages_path", "registry_limits", "resource_limits",
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
            "max_bn254_pairing_checks_per_transaction", "max_bn254_pairing_checks_per_block",
        ]
        try SccpStrictJSON.exactFields(value, fields, label: "SCCP resource limits")
        let result = SccpResourceLimits(
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
            )
        )
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
                UInt64(result.maxBn254PairingChecksPerTransaction),
                UInt64(result.maxBn254PairingChecksPerBlock)
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
        try SccpStrictJSON.exactFields(root, [
            "version", "backend", "source_network", "target_network", "public_inputs", "verifying_key",
            "verifier_key_hash", "semantic_proof_profile", "semantic_proof_profile_hash",
            "sora_finality_anchor", "sora_finality_anchor_hash", "bundle_bytes", "statement_hash",
            "destination_binding_hash", "route_configuration_hash", "request_hash",
        ], label: "SCCP proof request")
        guard try SccpStrictJSON.uint64(root, "version", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("SCCP proof request version must be exactly 1")
        }
        let backend = try destinationBackend(object(root, "backend"), label: "SCCP proof request backend")
        let source = try network(object(root, "source_network"), label: "source_network")
        let target = try network(object(root, "target_network"), label: "target_network")
        guard source == .soraTaira, target.isExternal,
              (backend == .tronGroth16Bn254) == target.rawValue.hasPrefix("tron-")
        else { throw SccpV1Error.invalid("SCCP proof backend does not match an exact Taira-to-external lane") }
        let inputs = try object(root, "public_inputs")
        try SccpStrictJSON.exactFields(inputs, [
            "version", "message_id", "payload_hash", "target_domain", "commitment_root",
            "finality_height", "finality_block_hash",
        ], label: "SCCP proof public inputs")
        guard try SccpStrictJSON.uint64(inputs, "version", minimum: 1) == 1 else {
            throw SccpV1Error.invalid("SCCP public-input version must be exactly 1")
        }
        let targetDomain = try SccpStrictJSON.uint32(inputs, "target_domain", minimum: 1, maximum: 5)
        guard targetDomain == target.domainId else { throw SccpV1Error.invalid("SCCP target profile/domain mismatch") }
        let messageId = try prefixedHash(inputs, "message_id")
        let payloadHash = try prefixedHash(inputs, "payload_hash")
        let commitmentRoot = try prefixedHash(inputs, "commitment_root")
        let finalityBlockHash = try prefixedHash(inputs, "finality_block_hash")
        let finalityHeight = try decimalUInt64(inputs, "finality_height", minimum: 1)
        let keyBytes = try verifyingKey(object(root, "verifying_key"), label: "SCCP proof verifying key")
        let keyHash = try prefixedHash(root, "verifier_key_hash")
        guard "0x" + SccpV1.encodeLowerHex(irohaKeccak256(keyBytes)) == keyHash else {
            throw SccpV1Error.invalid("verifier_key_hash does not match the exact 38-word verifying key")
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
        let statement = try prefixedHash(root, "statement_hash")
        let binding = try prefixedHash(root, "destination_binding_hash")
        let configuration = try prefixedHash(root, "route_configuration_hash")
        let request = try prefixedHash(root, "request_hash")
        try distinctHexRoles([
            messageId, payloadHash, commitmentRoot, finalityBlockHash, keyHash, semanticHash,
            anchorHash, statement, binding, configuration, request,
        ], label: "SCCP proof-request hash roles")
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
            verifierKeyHash: keyHash,
            semanticProofProfile: semantic,
            soraFinalityAnchor: finalityAnchor,
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
        try SccpStrictJSON.exactFields(root, ["items"], label: "SCCP recent messages")
        let values = try array(root, "items")
        guard values.count <= 50 else { throw SccpV1Error.invalid("SCCP recent response exceeds 50 items") }
        var items: [SccpRecentMessage] = []
        var ids = Set<String>()
        for (index, raw) in values.enumerated() {
            let item = try object(raw, label: "items[\(index)]")
            let required: Set<String> = [
                "height", "message_id_hex", "kind", "source_profile", "target_profile",
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
            let domain = try SccpStrictJSON.uint32(item, "target_domain", minimum: 1, maximum: 5)
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
            case 5: expectedRoute = "taira_tron_xor"
            default: throw SccpV1Error.invalid("recent SCCP target domain is unsupported")
            }
            guard (assetId == nil || assetId == "xor"),
                  (routeId == nil || routeId == expectedRoute),
                  recipient == nil
            else { throw SccpV1Error.invalid("recent SCCP summary fields disagree with payload_projection") }
            items.append(SccpRecentMessage(
                height: try SccpStrictJSON.uint64(item, "height", minimum: 1),
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
            for index in 1..<items.count where items[index].height > items[index - 1].height {
                throw SccpV1Error.invalid("recent SCCP messages must be newest first")
            }
        }
        return SccpRecentMessages(items: items)
    }

    private static func governedRoute(
        _ item: [String: Any],
        expectedLane: SccpLaneIdV1,
        nativeAnchor: SccpNativeTrustAnchorV1?,
        label: String
    ) throws -> SccpGovernedRouteV1 {
        try SccpStrictJSON.exactFields(item, [
            "lane_id", "route_id", "asset_key", "revision", "activation",
            "inbound_finality_cutoff", "source_identity", "destination", "settlement",
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
        let sourceParts = emitterParts(source)
        guard sourceParts.family == destination.family,
              sourceParts.address == destination.routeAddress,
              sourceParts.runtimeCodeHash == destination.routeCodeHash
        else { throw SccpV1Error.invalid("\(label) source identity does not name its destination route deployment") }
        let settlement = try object(item, "settlement")
        try SccpStrictJSON.exactFields(settlement, ["asset_definition_id", "custody_account_id", "payload_amount_scale"], label: "\(label).settlement")
        let assetDefinition = try SccpStrictJSON.text(settlement, "asset_definition_id")
        guard assetDefinition == "6TEAJqbb8oEPmLncoNiMRbLEK6tw" else { throw SccpV1Error.invalid("\(label) must settle canonical Taira XOR") }
        let custody = try SccpStrictJSON.text(settlement, "custody_account_id")
        guard let address = try? AccountAddress.parseEncoded(custody),
              let canonical = try? address.toI105(
                  networkPrefix: SccpV1.tairaI105DiscriminantV1
              ), canonical == custody
        else {
            throw SccpV1Error.invalid(
                "\(label).custody_account_id must be a canonical Taira I105 AccountId"
            )
        }
        let scale = try SccpStrictJSON.uint32(settlement, "payload_amount_scale", minimum: 9, maximum: 9)
        let configuration = try routeConfigurationHash(
            lane: lane,
            routeId: routeId,
            assetKey: assetKey,
            revision: revision,
            destination: destination
        )
        guard sourceParts.routeConfigHash == configuration else {
            throw SccpV1Error.invalid("\(label) source route_config_hash does not match the immutable deployment")
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
            assetDefinitionId: assetDefinition,
            custodyAccountId: custody,
            payloadAmountScale: scale,
            routeConfigurationHash: configuration
        )
    }

    private static func destination(_ item: [String: Any], lane: SccpLaneIdV1, label: String) throws -> SccpDestinationDeploymentV1 {
        try SccpStrictJSON.exactFields(item, ["family", "deployment"], label: label)
        let familyText = try SccpStrictJSON.text(item, "family")
        let family: SccpDestinationProofBackendV1
        switch familyText {
        case "evm": family = .evmGroth16Bn254
        case "tron": family = .tronGroth16Bn254
        default: throw SccpV1Error.invalid("\(label) family is unsupported or retired")
        }
        guard (family == .tronGroth16Bn254) == lane.source.rawValue.hasPrefix("tron-") else {
            throw SccpV1Error.invalid("\(label) family does not match its lane")
        }
        let deployment = try object(item, "deployment")
        try SccpStrictJSON.exactFields(deployment, [
            "token_address", "token_code_hash", "verifier_address", "verifier_code_hash",
            "verifying_key", "verifier_key_hash", "outbound_proof_policy", "route_address",
            "route_code_hash", "taira_to_token_multiplier",
        ], label: "\(label).deployment")
        let addresses = try ["token_address", "verifier_address", "route_address"].map {
            try upperFixed(deployment, $0, bytes: 20)
        }
        let hashes = try ["token_code_hash", "verifier_code_hash", "verifier_key_hash", "route_code_hash"].map {
            try upperFixed(deployment, $0, bytes: 32)
        }
        guard Set(addresses).count == addresses.count, Set(hashes).count == hashes.count else {
            throw SccpV1Error.invalid("\(label).deployment reuses a role-separated address or hash")
        }
        let keyBytes = try verifyingKey(object(deployment, "verifying_key"), label: "\(label).deployment.verifying_key")
        guard irohaKeccak256(keyBytes) == hashes[2] else { throw SccpV1Error.invalid("\(label).deployment.verifier_key_hash does not match verifying_key") }
        let policy = try outboundPolicy(object(deployment, "outbound_proof_policy"), label: "\(label).deployment.outbound_proof_policy")
        let deploymentHashRoles = hashes + [policy.0.profileHash, policy.1.anchorHash]
        guard Set(deploymentHashRoles).count == deploymentHashRoles.count else {
            throw SccpV1Error.invalid("\(label).deployment reuses a proof-policy or deployment hash role")
        }
        guard try SccpStrictJSON.uint64(deployment, "taira_to_token_multiplier", minimum: 1_000_000_000) == 1_000_000_000 else {
            throw SccpV1Error.invalid("\(label).deployment has the wrong Taira/token multiplier")
        }
        let partial = SccpDestinationDeploymentV1(
            family: family,
            tokenAddress: addresses[0],
            tokenCodeHash: hashes[0],
            verifierAddress: addresses[1],
            verifierCodeHash: hashes[1],
            verifierKeyHash: hashes[2],
            semanticProofProfile: policy.0,
            soraFinalityAnchor: policy.1,
            routeAddress: addresses[2],
            routeCodeHash: hashes[3],
            tairaToTokenMultiplier: 1_000_000_000,
            destinationBindingHash: Data()
        )
        let binding = try destinationBindingHash(lane: lane, destination: partial)
        return SccpDestinationDeploymentV1(
            family: partial.family,
            tokenAddress: partial.tokenAddress,
            tokenCodeHash: partial.tokenCodeHash,
            verifierAddress: partial.verifierAddress,
            verifierCodeHash: partial.verifierCodeHash,
            verifierKeyHash: partial.verifierKeyHash,
            semanticProofProfile: partial.semanticProofProfile,
            soraFinalityAnchor: partial.soraFinalityAnchor,
            routeAddress: partial.routeAddress,
            routeCodeHash: partial.routeCodeHash,
            tairaToTokenMultiplier: partial.tairaToTokenMultiplier,
            destinationBindingHash: binding
        )
    }

    private static func outboundPolicy(_ item: [String: Any], label: String) throws -> (SccpSemanticProofProfileV1, SccpSoraFinalityAnchorV1) {
        try SccpStrictJSON.exactFields(item, ["version", "semantic_profile", "sora_finality_anchor"], label: label)
        guard try SccpStrictJSON.uint64(item, "version", minimum: 1) == 1 else { throw SccpV1Error.invalid("\(label).version must be 1") }
        let semantic = try semanticProfile(object(item, "semantic_profile"), label: "\(label).semantic_profile")
        let anchor = try finalityAnchor(object(item, "sora_finality_anchor"), label: "\(label).sora_finality_anchor")
        let roles = [semantic.circuitCommitment, semantic.witnessGeneratorCommitment, semantic.publicSignalSchemaHash,
                     semantic.profileHash, anchor.chainIdHash, anchor.checkpointBlockHash, anchor.validatorSetHash, anchor.anchorHash]
        guard roles.allSatisfy({ !$0.allSatisfy { $0 == 0 } }), Set(roles).count == roles.count else {
            throw SccpV1Error.invalid("\(label) reuses a proof-policy hash role")
        }
        return (semantic, anchor)
    }

    private static func semanticProfile(_ item: [String: Any], label: String) throws -> SccpSemanticProofProfileV1 {
        try SccpStrictJSON.exactFields(item, ["profile", "commitments"], label: label)
        guard try SccpStrictJSON.text(item, "profile") == "sora_taira_finality_inclusion_groth16_bn254" else {
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
        guard schema == publicSignalSchemaHash(), Set([circuit, witness, schema]).count == 3 else {
            throw SccpV1Error.invalid("\(label) does not commit the exact eleven-signal schema")
        }
        let canonical = Data([1, 0, 1]) + circuit + witness + schema
        let hash = irohaKeccak256(Data("sccp:semantic-proof-profile:v1".utf8) + canonical)
        return SccpSemanticProofProfileV1(
            circuitCommitment: circuit,
            witnessGeneratorCommitment: witness,
            publicSignalSchemaHash: schema,
            profileHash: hash
        )
    }

    private static func finalityAnchor(_ item: [String: Any], label: String) throws -> SccpSoraFinalityAnchorV1 {
        try SccpStrictJSON.exactFields(item, [
            "version", "source_network", "chain_id_hash", "checkpoint_height", "checkpoint_block_hash",
            "validator_set_epoch", "validator_set_hash", "validator_set_hash_version",
        ], label: label)
        guard try SccpStrictJSON.uint64(item, "version", minimum: 1) == 1,
              try network(object(item, "source_network"), label: "\(label).source_network") == .soraTaira
        else { throw SccpV1Error.invalid("\(label) must be a V1 Taira anchor") }
        let chainHash = try upperFixed(item, "chain_id_hash", bytes: 32)
        guard chainHash == irohaKeccak256(tairaChainId) else { throw SccpV1Error.invalid("\(label).chain_id_hash is not Taira") }
        let checkpointHeight = try SccpStrictJSON.uint64(item, "checkpoint_height", minimum: 1)
        let checkpointHash = try upperFixed(item, "checkpoint_block_hash", bytes: 32)
        let epoch = try SccpStrictJSON.uint64(item, "validator_set_epoch", minimum: 0)
        let validatorHash = try upperFixed(item, "validator_set_hash", bytes: 32)
        let hashVersion = try SccpStrictJSON.uint32(item, "validator_set_hash_version", minimum: 1, maximum: 1)
        guard Set([chainHash, checkpointHash, validatorHash]).count == 3 else { throw SccpV1Error.invalid("\(label) reuses a consensus hash role") }
        var canonical = Data([1, SccpNetworkV1.soraTaira.tag])
        canonical.append(chainHash)
        appendUInt64LE(checkpointHeight, to: &canonical)
        canonical.append(checkpointHash)
        appendUInt64LE(epoch, to: &canonical)
        canonical.append(validatorHash)
        appendUInt16LE(UInt16(hashVersion), to: &canonical)
        let hash = irohaKeccak256(Data("sccp:sora-finality-anchor:v1".utf8) + canonical)
        return SccpSoraFinalityAnchorV1(
            chainIdHash: chainHash,
            checkpointHeight: checkpointHeight,
            checkpointBlockHash: checkpointHash,
            validatorSetEpoch: epoch,
            validatorSetHash: validatorHash,
            validatorSetHashVersion: UInt16(hashVersion),
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
        try SccpStrictJSON.exactFields(identity, ["address", "runtime_code_hash", "route_config_hash"], label: "\(label).emitter.identity")
        let address = try upperFixed(identity, "address", bytes: 20)
        let runtime = try upperFixed(identity, "runtime_code_hash", bytes: 32)
        let configuration = try upperFixed(identity, "route_config_hash", bytes: 32)
        guard runtime != configuration else { throw SccpV1Error.invalid("\(label) runtime and route hashes must be distinct") }
        switch family {
        case "evm" where !expectedLane.source.rawValue.hasPrefix("tron-"):
            return try .validatedEvm(address: address, runtimeCodeHash: runtime, routeConfigHash: configuration)
        case "tron" where expectedLane.source.rawValue.hasPrefix("tron-"):
            return try .validatedTron(address: address, runtimeCodeHash: runtime, routeConfigHash: configuration)
        default: throw SccpV1Error.invalid("\(label) emitter family does not match its lane")
        }
    }

    private static func emitterParts(_ emitter: SccpSourceEmitterV1) -> (
        family: SccpDestinationProofBackendV1, address: Data, runtimeCodeHash: Data, routeConfigHash: Data
    ) {
        switch emitter {
        case let .evm(address, runtime, configuration): return (.evmGroth16Bn254, address, runtime, configuration)
        case let .tron(address, runtime, configuration): return (.tronGroth16Bn254, address, runtime, configuration)
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
              try SccpStrictJSON.uint32(item, "source_domain", minimum: 0, maximum: 5) == lane.source.domainId,
              try SccpStrictJSON.uint32(item, "dest_domain", minimum: 0, maximum: 5) == lane.target.domainId,
              try SccpStrictJSON.uint32(item, "route_revision", minimum: 1, maximum: UInt32.max) > 0
        else { throw SccpV1Error.invalid("SCCP transfer payload does not match its exact lane") }
        _ = try decimalUInt64(item, "nonce", minimum: 0)
        let assetHomeDomain = try SccpStrictJSON.uint32(item, "asset_home_domain", minimum: 0, maximum: 5)
        guard [UInt32(0), 1, 2, 5].contains(assetHomeDomain) else {
            throw SccpV1Error.invalid("SCCP transfer asset_home_domain is unsupported or retired")
        }
        let amount = try decimalText(item, "amount", minimum: 1)
        let maximumUInt128 = "340282366920938463463374607431768211455"
        guard amount.count < maximumUInt128.count || amount.count == maximumUInt128.count && amount <= maximumUInt128 else {
            throw SccpV1Error.invalid("SCCP transfer amount must fit UInt128")
        }
        let senderCodec = try SccpStrictJSON.uint32(item, "sender_codec", minimum: 1, maximum: 5)
        let recipientCodec = try SccpStrictJSON.uint32(item, "recipient_codec", minimum: 1, maximum: 5)
        let expectedRecipientCodec: UInt32 = lane.target.domainId == 5 ? 5 : 2
        guard senderCodec == 1, recipientCodec == expectedRecipientCodec else {
            throw SccpV1Error.invalid("SCCP transfer account codecs do not match its exact domains")
        }
        for (codecField, valueField) in [
            ("asset_id_codec", "asset_id"), ("sender_codec", "sender"),
            ("recipient_codec", "recipient"), ("route_id_codec", "route_id"),
        ] {
            let codec = try SccpStrictJSON.uint32(item, codecField, minimum: 1, maximum: 5)
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

    private static func destinationBindingHash(lane: SccpLaneIdV1, destination: SccpDestinationDeploymentV1) throws -> Data {
        let isTron = destination.family == .tronGroth16Bn254
        let domain = lane.source.domainId
        let networkWord: Data
        if isTron {
            let networkId: UInt32
            switch lane.source {
            case .tronMainnet: networkId = 0x2b66_53dc
            case .tronNile: networkId = 0xcd86_90dc
            case .tronShasta: networkId = 0x94a9_059e
            default: throw SccpV1Error.invalid("TRON binding requires a TRON lane")
            }
            networkWord = abiWord(UInt64(networkId))
        } else {
            let chainId: UInt64
            switch lane.source {
            case .ethereumMainnet: chainId = 1
            case .ethereumSepolia: chainId = 11_155_111
            case .bscMainnet: chainId = 56
            case .bscTestnet: chainId = 97
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
        payload.append(isTron ? abiTronAddress(destination.verifierAddress) : abiAddress(destination.verifierAddress))
        payload.append(isTron ? abiTronAddress(destination.routeAddress) : abiAddress(destination.routeAddress))
        payload.append(destination.verifierCodeHash)
        payload.append(destination.verifierKeyHash)
        payload.append(destination.semanticProofProfile.profileHash)
        payload.append(destination.soraFinalityAnchor.anchorHash)
        return irohaKeccak256(payload)
    }

    private static func routeConfigurationHash(
        lane: SccpLaneIdV1,
        routeId: String,
        assetKey: String,
        revision: UInt32,
        destination: SccpDestinationDeploymentV1
    ) throws -> Data {
        guard assetKey == "xor" else { throw SccpV1Error.invalid("SCCP V1 route asset must be xor") }
        let expectedRoute: String
        let networkValue: UInt64
        switch lane.source {
        case .ethereumMainnet: expectedRoute = "taira_eth_xor"; networkValue = 1
        case .ethereumSepolia: expectedRoute = "taira_eth_xor"; networkValue = 11_155_111
        case .bscMainnet: expectedRoute = "taira_bsc_xor"; networkValue = 56
        case .bscTestnet: expectedRoute = "taira_bsc_xor"; networkValue = 97
        case .tronMainnet: expectedRoute = "taira_tron_xor"; networkValue = 0x2b66_53dc
        case .tronNile: expectedRoute = "taira_tron_xor"; networkValue = 0xcd86_90dc
        case .tronShasta: expectedRoute = "taira_tron_xor"; networkValue = 0x94a9_059e
        default: throw SccpV1Error.invalid("SCCP route external profile is unsupported")
        }
        guard routeId == expectedRoute else { throw SccpV1Error.invalid("SCCP route id does not match its exact deployment") }
        let sourceHash = SccpV1.laneHash(lane)
        let reverseHash = SccpV1.laneHash(try SccpLaneIdV1(source: lane.target, target: lane.source))
        var roles = [sourceHash, reverseHash, destination.tokenCodeHash, destination.verifierCodeHash,
                     destination.verifierKeyHash, destination.semanticProofProfile.profileHash,
                     destination.soraFinalityAnchor.anchorHash]
        if destination.family == .tronGroth16Bn254 {
            roles.append(destination.destinationBindingHash)
        }
        guard Set(roles).count == roles.count else { throw SccpV1Error.invalid("SCCP route reuses a hash role") }
        var deployment = abiAddress(destination.tokenAddress)
        deployment.append(destination.tokenCodeHash)
        deployment.append(abiAddress(destination.verifierAddress))
        deployment.append(destination.verifierCodeHash)
        deployment.append(destination.verifierKeyHash)
        deployment.append(destination.semanticProofProfile.profileHash)
        deployment.append(destination.soraFinalityAnchor.anchorHash)
        if destination.family == .tronGroth16Bn254 { deployment.append(destination.destinationBindingHash) }
        let deploymentHash = irohaKeccak256(deployment)
        var assetRoute = irohaKeccak256(Data("xor".utf8))
        assetRoute.append(irohaKeccak256(Data(routeId.utf8)))
        assetRoute.append(abiWord(UInt64(revision)))
        assetRoute.append(abiWord(destination.tairaToTokenMultiplier))
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

    private static func appendUInt16LE(_ value: UInt16, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }

    private static func appendUInt32LE(_ value: UInt32, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
    }

    private static func appendUInt64LE(_ value: UInt64, to out: inout Data) {
        var little = value.littleEndian
        withUnsafeBytes(of: &little) { out.append(contentsOf: $0) }
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

    private static func prefixedHash(_ item: [String: Any], _ field: String) throws -> String {
        let value = try SccpStrictJSON.text(item, field)
        guard value.count == 66, value.hasPrefix("0x"),
              value.dropFirst(2).allSatisfy({ $0.isNumber || ("a"..."f").contains(String($0)) }),
              value.dropFirst(2).contains(where: { $0 != "0" })
        else { throw SccpV1Error.invalid("\(field) must be canonical lowercase nonzero 0x-prefixed hash") }
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
        guard value.allSatisfy(\.isNumber), value == "0" || value.first != "0",
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
              try SccpStrictJSON.uint32(transfer, "dest_domain", minimum: 1, maximum: 5) == expectedDestinationDomain,
              try SccpStrictJSON.uint32(transfer, "asset_home_domain", minimum: 0, maximum: 0) == 0
        else { throw SccpV1Error.invalid("\(label).Transfer domains or version do not match the recent message") }
        guard let nonceNumber = transfer["nonce"] as? NSNumber,
              CFGetTypeID(nonceNumber) != CFBooleanGetTypeID(),
              let nonce = UInt64(nonceNumber.stringValue),
              nonce.description == nonceNumber.stringValue
        else { throw SccpV1Error.invalid("\(label).Transfer.nonce must be a canonical UInt64 integer") }
        _ = try SccpStrictJSON.uint32(transfer, "route_revision", minimum: 1, maximum: UInt32.max)

        guard let amountNumber = transfer["amount"] as? NSNumber,
              CFGetTypeID(amountNumber) != CFBooleanGetTypeID()
        else { throw SccpV1Error.invalid("\(label).Transfer.amount must be a positive UInt128 integer") }
        let amount = amountNumber.stringValue
        let maximumUInt128 = "340282366920938463463374607431768211455"
        guard amount.first.map({ $0 != "0" }) == true,
              amount.allSatisfy(\.isNumber),
              amount.count < maximumUInt128.count || amount.count == maximumUInt128.count && amount <= maximumUInt128,
              amount == expectedAmount
        else { throw SccpV1Error.invalid("\(label).Transfer.amount must equal the positive UInt128 readback amount") }

        try normalizedCanonicalText(transfer, field: "asset_id", expected: "xor", label: label)
        try normalizedCanonicalText(transfer, field: "sender", expected: nil, label: label)
        let expectedRoute: String
        switch expectedDestinationDomain {
        case 1: expectedRoute = "taira_eth_xor"
        case 2: expectedRoute = "taira_bsc_xor"
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
        return try JSONSerialization.data(withJSONObject: projection, options: [.sortedKeys])
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
        let tag = destinationDomain == 5 ? "TronAddress21" : "EvmAddress20"
        try SccpStrictJSON.exactFields(tagged, [tag], label: "\(label).Transfer.\(field)")
        let content = try object(tagged, tag)
        try SccpStrictJSON.exactFields(content, ["bytes"], label: "\(label).Transfer.\(field).\(tag)")
        let bytes = try SccpStrictJSON.text(content, "bytes")
        let hex = bytes.dropFirst(2)
        let expectedHexCount = destinationDomain == 5 ? 42 : 40
        guard bytes.hasPrefix(destinationDomain == 5 ? "0x41" : "0x"),
              hex.count == expectedHexCount,
              hex.allSatisfy({ $0 >= "0" && $0 <= "9" || $0 >= "a" && $0 <= "f" }),
              (destinationDomain == 5 ? hex.dropFirst(2) : hex).contains(where: { $0 != "0" })
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
