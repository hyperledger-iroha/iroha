using System.Buffers.Binary;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Address;

namespace Hyperledger.Iroha.Sccp;

/// <summary>Fixed SCCP V1 route-registry capacity limits.</summary>
public sealed record SccpRegistryLimits(
    uint MaxGovernedLanes,
    uint MaxLiveGovernedRoutes,
    uint MaxLiveRoutesPerLane,
    uint MaxRetainedRoutesPerLane,
    uint MaxRetainedNativeTrustAnchorsPerLane);

/// <summary>Consensus-critical SCCP proof and deterministic verifier-work limits.</summary>
public sealed record SccpResourceLimits(
    uint MaxOutboundMessagesPerBlock,
    ulong MaxOutboundMessagePayloadBytes,
    ulong MaxPendingOutboundMessages,
    ulong MaxPendingOutboundPayloadBytes,
    uint MaxProofsPerTransaction,
    uint MaxProofsPerBlock,
    ulong MaxProofBytesPerProof,
    ulong MaxProofBytesPerTransaction,
    ulong MaxProofBytesPerBlock,
    uint MaxNativeHeadersPerTransaction,
    uint MaxNativeHeadersPerBlock,
    uint MaxEthereumLightClientUpdatesPerTransaction,
    uint MaxEthereumLightClientUpdatesPerBlock,
    ulong MaxNativeHeaderBytesPerTransaction,
    ulong MaxNativeHeaderBytesPerBlock,
    uint MaxSecp256k1RecoveriesPerTransaction,
    uint MaxSecp256k1RecoveriesPerBlock,
    uint MaxBlsAggregateChecksPerTransaction,
    uint MaxBlsAggregateChecksPerBlock,
    uint MaxBlsSignerContributionsPerTransaction,
    uint MaxBlsSignerContributionsPerBlock,
    uint MaxBn254PairingChecksPerTransaction,
    uint MaxBn254PairingChecksPerBlock);

/// <summary>Stable first-release SCCP HTTP surface.</summary>
public sealed record SccpCapabilities(
    byte Version,
    string RegistryRevision,
    string RegistryPath,
    string MessageBundlePath,
    string ProofRequestPath,
    string RecentMessagesPath,
    SccpRegistryLimits RegistryLimits,
    SccpResourceLimits ResourceLimits,
    string? ProofSubmitPath,
    string? NativeMessageSubmitPath)
{
    public static SccpCapabilities Parse(ReadOnlyMemory<byte> json) => SccpExactParser.ParseCapabilities(json);
}

public enum SccpDestinationProofBackendV1
{
    EvmGroth16Bn254,
    TronGroth16Bn254,
}

public enum SccpRouteActivationV1
{
    Staged,
    Bidirectional,
    InboundOnly,
    Paused,
    Retired,
}

public static class SccpRouteActivationV1Extensions
{
    public static bool AllowsOutbound(this SccpRouteActivationV1 value) => value == SccpRouteActivationV1.Bidirectional;

    public static bool AllowsInbound(this SccpRouteActivationV1 value) =>
        value is SccpRouteActivationV1.Bidirectional or SccpRouteActivationV1.InboundOnly;
}

public sealed record SccpNativeTrustAnchorV1(
    SccpNativeBackendV1 Backend,
    byte[] AnchorHash,
    ulong CheckpointHeight);

public sealed record SccpInboundFinalityCutoffV1(
    byte[] TrustAnchorHash,
    ulong MaxAnchorIntervalHeight);

public sealed record SccpSemanticProofProfileV1(
    byte[] CircuitCommitment,
    byte[] WitnessGeneratorCommitment,
    byte[] PublicSignalSchemaHash,
    byte[] ProfileHash);

public sealed record SccpSoraFinalityAnchorV1(
    ushort ProtocolVersion,
    byte[] ChainIdHash,
    ulong CheckpointHeight,
    byte[] CheckpointBlockHash,
    byte[] CheckpointContextId,
    byte[] CheckpointFinalityArtifactHash,
    byte[] AnchorHash);

public sealed record SccpDestinationDeploymentV1(
    SccpDestinationProofBackendV1 Family,
    byte[] TokenAddress,
    byte[] TokenCodeHash,
    byte[] VerifierAddress,
    byte[] VerifierCodeHash,
    byte[] VerifierKeyHash,
    SccpSemanticProofProfileV1 SemanticProofProfile,
    SccpSoraFinalityAnchorV1 SoraFinalityAnchor,
    byte[] RouteAddress,
    byte[] RouteCodeHash,
    ulong TairaToTokenMultiplier,
    byte[] DestinationBindingHash);

public sealed record SccpGovernedRouteV1(
    SccpLaneIdV1 Lane,
    string RouteId,
    string AssetKey,
    uint Revision,
    SccpRouteActivationV1 Activation,
    SccpInboundFinalityCutoffV1? InboundFinalityCutoff,
    SccpSourceEmitterV1 SourceEmitter,
    SccpDestinationDeploymentV1 Destination,
    string AssetDefinitionId,
    string CustodyAccountId,
    uint PayloadAmountScale,
    byte[] RouteConfigurationHash);

public sealed record SccpGovernedLaneV1(
    SccpLaneIdV1 Lane,
    IReadOnlyList<SccpNativeTrustAnchorV1> NativeTrustAnchors,
    byte[]? CurrentNativeTrustAnchorHash,
    IReadOnlyList<SccpGovernedRouteV1> Routes);

/// <summary>Authoritative typed SCCP registry returned by <c>GET /v1/sccp/registry</c>.</summary>
public sealed record SccpRegistryV1(byte Version, IReadOnlyList<SccpGovernedLaneV1> Lanes, byte[] RawJson)
{
    public static SccpRegistryV1 Parse(ReadOnlyMemory<byte> json) => SccpExactParser.ParseRegistry(json);
}

public sealed record SccpMessageBundleV1(
    byte Version,
    string CommitmentRoot,
    string MessageId,
    uint TargetDomain,
    SccpTransferPayloadV1 Payload,
    SccpHubCommitmentV1 Commitment,
    IReadOnlyList<SccpMerkleStepV1> MerkleProof,
    byte[] FinalityProof,
    byte[] RawJson)
{
    public static SccpMessageBundleV1 Parse(ReadOnlyMemory<byte> json) => SccpExactParser.ParseBundle(json);
}

public sealed record SccpGroth16ProofRequestV1(
    byte Version,
    SccpDestinationProofBackendV1 Backend,
    SccpNetworkV1 SourceNetwork,
    SccpNetworkV1 TargetNetwork,
    string MessageId,
    string PayloadHash,
    uint TargetDomain,
    string CommitmentRoot,
    ulong FinalityHeight,
    string FinalityBlockHash,
    string VerifierKeyHash,
    SccpSemanticProofProfileV1 SemanticProofProfile,
    SccpSoraFinalityAnchorV1 SoraFinalityAnchor,
    string StatementHash,
    string DestinationBindingHash,
    string RouteConfigurationHash,
    string RequestHash,
    byte[] BundleBytes,
    byte[] RawJson)
{
    public static SccpGroth16ProofRequestV1 Parse(ReadOnlyMemory<byte> json) =>
        SccpExactParser.ParseProofRequest(json);
}

public sealed record SccpRecentMessageLinks(string BundlePath, string ProofRequestPath);

public sealed record SccpRecentMessage(
    ulong Height,
    uint CommitmentIndex,
    string MessageIdHex,
    SccpPayloadKindV1 Kind,
    SccpLaneIdV1 Lane,
    string DestinationBindingHash,
    string RouteConfigurationHash,
    string? AssetId,
    string? RouteId,
    string? Recipient,
    string Amount,
    string PayloadProjectionJson,
    SccpRecentMessageLinks Links);

public sealed record SccpRecentCursor(ulong From, uint AfterIndex);

public sealed record SccpRecentMessages(
    IReadOnlyList<SccpRecentMessage> Items,
    SccpRecentCursor? Next)
{
    public static SccpRecentMessages Parse(ReadOnlyMemory<byte> json) => SccpExactParser.ParseRecent(json);
}

internal static class SccpExactParser
{
    private const int MaximumWireBytes = 16 * 1024 * 1024;
    private const ulong JsonSafeIntegerMaximum = (1UL << 53) - 1;
    private static readonly byte[] Bn254BaseField = Convert.FromHexString(
        "30644E72E131A029B85045B68181585D97816A916871CA8D3C208C16D87CFD47");
    private static readonly byte[] TairaChainId = Convert.FromHexString("FC56984B2BE7431D840E21514D1883F0");
    private static readonly string[] PublicSignalLabels =
    [
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
    ];
    private static readonly IReadOnlyDictionary<string, string> CapabilityPaths =
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["registry_path"] = "/v1/sccp/registry",
            ["message_bundle_path"] = "/v1/sccp/proofs/message/{message_id}",
            ["proof_request_path"] = "/v1/sccp/proof-requests/{message_id}",
            ["recent_messages_path"] = "/v1/sccp/messages/recent",
            ["proof_submit_path"] = "/v1/bridge/proofs/submit",
            ["native_message_submit_path"] = "/v1/bridge/messages",
        };

    internal static SccpCapabilities ParseCapabilities(ReadOnlyMemory<byte> json)
    {
        using var document = SccpJson.Parse(json, "SCCP capabilities");
        var root = document.RootElement;
        HashSet<string> required =
        [
            "version", "registry_revision", "registry_path", "message_bundle_path",
            "proof_request_path", "recent_messages_path", "registry_limits", "resource_limits",
        ];
        var allowed = new HashSet<string>(required, StringComparer.Ordinal)
        {
            "proof_submit_path", "native_message_submit_path",
        };
        SccpJson.ExactFields(root, allowed, required, "SCCP capabilities");
        RequireVersion(root, "SCCP capability");
        var proofSubmitPath = OptionalFixedPath(root, "proof_submit_path");
        var nativeMessageSubmitPath = OptionalFixedPath(root, "native_message_submit_path");
        if ((proofSubmitPath is null) != (nativeMessageSubmitPath is null))
        {
            throw new ArgumentException("SCCP submission capability paths must be advertised together.");
        }

        return new SccpCapabilities(
            1,
            PrefixedHash(root, "registry_revision"),
            FixedPath(root, "registry_path"),
            FixedPath(root, "message_bundle_path"),
            FixedPath(root, "proof_request_path"),
            FixedPath(root, "recent_messages_path"),
            ParseRegistryLimits(Object(root, "registry_limits")),
            ParseResourceLimits(Object(root, "resource_limits")),
            proofSubmitPath,
            nativeMessageSubmitPath);
    }

    private static SccpRegistryLimits ParseRegistryLimits(JsonElement item)
    {
        SccpJson.ExactFields(
            item,
            [
                "max_governed_lanes", "max_live_governed_routes",
                "max_live_routes_per_lane", "max_retained_routes_per_lane",
                "max_retained_native_trust_anchors_per_lane",
            ],
            "SCCP registry limits");
        var limits = new SccpRegistryLimits(
            SccpJson.UInt32(item, "max_governed_lanes", 1, uint.MaxValue),
            SccpJson.UInt32(item, "max_live_governed_routes", 1, uint.MaxValue),
            SccpJson.UInt32(item, "max_live_routes_per_lane", 1, uint.MaxValue),
            SccpJson.UInt32(item, "max_retained_routes_per_lane", 1, uint.MaxValue),
            SccpJson.UInt32(
                item,
                "max_retained_native_trust_anchors_per_lane",
                1,
                uint.MaxValue));
        if (limits != new SccpRegistryLimits(16, 64, 8, 64, 4_096))
        {
            throw new ArgumentException(
                "SCCP registry limits must equal the fixed V1 capacities.");
        }

        return limits;
    }

    private static SccpResourceLimits ParseResourceLimits(JsonElement item)
    {
        SccpJson.ExactFields(
            item,
            [
                "max_outbound_messages_per_block",
                "max_outbound_message_payload_bytes",
                "max_pending_outbound_messages",
                "max_pending_outbound_payload_bytes",
                "max_proofs_per_transaction", "max_proofs_per_block",
                "max_proof_bytes_per_proof", "max_proof_bytes_per_transaction",
                "max_proof_bytes_per_block", "max_native_headers_per_transaction",
                "max_native_headers_per_block",
                "max_ethereum_light_client_updates_per_transaction",
                "max_ethereum_light_client_updates_per_block",
                "max_native_header_bytes_per_transaction",
                "max_native_header_bytes_per_block",
                "max_secp256k1_recoveries_per_transaction",
                "max_secp256k1_recoveries_per_block",
                "max_bls_aggregate_checks_per_transaction",
                "max_bls_aggregate_checks_per_block",
                "max_bls_signer_contributions_per_transaction",
                "max_bls_signer_contributions_per_block",
                "max_bn254_pairing_checks_per_transaction",
                "max_bn254_pairing_checks_per_block",
            ],
            "SCCP resource limits");
        var limits = new SccpResourceLimits(
            SccpJson.UInt32(item, "max_outbound_messages_per_block", 512, 512),
            SccpJson.UInt64(item, "max_outbound_message_payload_bytes", 4_096, 4_096),
            SccpJson.UInt64(
                item,
                "max_pending_outbound_messages",
                1,
                JsonSafeIntegerMaximum),
            SccpJson.UInt64(
                item,
                "max_pending_outbound_payload_bytes",
                1,
                JsonSafeIntegerMaximum),
            SccpJson.UInt32(item, "max_proofs_per_transaction", 1, uint.MaxValue),
            SccpJson.UInt32(item, "max_proofs_per_block", 1, uint.MaxValue),
            SccpJson.UInt64(item, "max_proof_bytes_per_proof", 1, JsonSafeIntegerMaximum),
            SccpJson.UInt64(item, "max_proof_bytes_per_transaction", 1, JsonSafeIntegerMaximum),
            SccpJson.UInt64(item, "max_proof_bytes_per_block", 1, JsonSafeIntegerMaximum),
            SccpJson.UInt32(item, "max_native_headers_per_transaction", 1, uint.MaxValue),
            SccpJson.UInt32(item, "max_native_headers_per_block", 1, uint.MaxValue),
            SccpJson.UInt32(
                item,
                "max_ethereum_light_client_updates_per_transaction",
                1,
                uint.MaxValue),
            SccpJson.UInt32(
                item,
                "max_ethereum_light_client_updates_per_block",
                1,
                uint.MaxValue),
            SccpJson.UInt64(
                item,
                "max_native_header_bytes_per_transaction",
                1,
                JsonSafeIntegerMaximum),
            SccpJson.UInt64(
                item,
                "max_native_header_bytes_per_block",
                1,
                JsonSafeIntegerMaximum),
            SccpJson.UInt32(
                item,
                "max_secp256k1_recoveries_per_transaction",
                1,
                uint.MaxValue),
            SccpJson.UInt32(
                item, "max_secp256k1_recoveries_per_block", 1, uint.MaxValue),
            SccpJson.UInt32(
                item, "max_bls_aggregate_checks_per_transaction", 1, uint.MaxValue),
            SccpJson.UInt32(
                item, "max_bls_aggregate_checks_per_block", 1, uint.MaxValue),
            SccpJson.UInt32(
                item,
                "max_bls_signer_contributions_per_transaction",
                1,
                uint.MaxValue),
            SccpJson.UInt32(
                item,
                "max_bls_signer_contributions_per_block",
                1,
                uint.MaxValue),
            SccpJson.UInt32(
                item, "max_bn254_pairing_checks_per_transaction", 1, uint.MaxValue),
            SccpJson.UInt32(
                item, "max_bn254_pairing_checks_per_block", 1, uint.MaxValue));
        if (limits.MaxProofBytesPerProof > limits.MaxProofBytesPerTransaction)
        {
            throw new ArgumentException(
                "SCCP per-proof byte limit exceeds its transaction limit.");
        }

        (ulong Transaction, ulong Block)[] orderedPairs =
        [
            (limits.MaxProofsPerTransaction, limits.MaxProofsPerBlock),
            (limits.MaxProofBytesPerTransaction, limits.MaxProofBytesPerBlock),
            (limits.MaxNativeHeadersPerTransaction, limits.MaxNativeHeadersPerBlock),
            (
                limits.MaxEthereumLightClientUpdatesPerTransaction,
                limits.MaxEthereumLightClientUpdatesPerBlock
            ),
            (limits.MaxNativeHeaderBytesPerTransaction, limits.MaxNativeHeaderBytesPerBlock),
            (
                limits.MaxSecp256k1RecoveriesPerTransaction,
                limits.MaxSecp256k1RecoveriesPerBlock
            ),
            (limits.MaxBlsAggregateChecksPerTransaction, limits.MaxBlsAggregateChecksPerBlock),
            (
                limits.MaxBlsSignerContributionsPerTransaction,
                limits.MaxBlsSignerContributionsPerBlock
            ),
            (limits.MaxBn254PairingChecksPerTransaction, limits.MaxBn254PairingChecksPerBlock),
        ];
        if (orderedPairs.Any(static pair => pair.Transaction > pair.Block))
        {
            throw new ArgumentException(
                "SCCP transaction resource limits must not exceed block limits.");
        }

        return limits;
    }

    internal static SccpRegistryV1 ParseRegistry(ReadOnlyMemory<byte> json)
    {
        using var document = SccpJson.Parse(json, "SCCP registry");
        var root = document.RootElement;
        SccpJson.ExactFields(root, ["version", "lanes"], "SCCP registry");
        RequireVersion(root, "SCCP registry");
        var laneValues = Array(root, "lanes");
        if (laneValues.Length > 16)
        {
            throw new ArgumentException("SCCP registry exceeds 16 lanes.");
        }

        var lanes = new List<SccpGovernedLaneV1>();
        var laneKeys = new HashSet<string>(StringComparer.Ordinal);
        var routeKeys = new HashSet<string>(StringComparer.Ordinal);
        var bindings = new HashSet<string>(StringComparer.Ordinal);
        var configurations = new HashSet<string>(StringComparer.Ordinal);
        var liveRouteCount = 0;
        for (var index = 0; index < laneValues.Length; index++)
        {
            var item = laneValues[index];
            var label = $"lanes[{index}]";
            SccpJson.ExactFields(
                item,
                ["lane_id", "native_trust_anchors", "current_native_trust_anchor_hash", "routes"],
                label);
            var lane = ParseInboundLane(Object(item, "lane_id"), $"lanes[{index}].lane_id");
            var laneKey = $"{lane.Source.ProfileKey()}>{lane.Target.ProfileKey()}";
            if (!laneKeys.Add(laneKey))
            {
                throw new ArgumentException("SCCP registry contains a duplicate lane.");
            }

            var anchorValues = Array(item, "native_trust_anchors");
            if (anchorValues.Length > 4_096)
            {
                throw new ArgumentException(
                    $"{label} contains more than 4,096 retained native trust anchors.");
            }
            var anchors = new List<SccpNativeTrustAnchorV1>(anchorValues.Length);
            var anchorHashes = new HashSet<string>(StringComparer.Ordinal);
            SccpNativeTrustAnchorV1? previousAnchor = null;
            for (var anchorIndex = 0; anchorIndex < anchorValues.Length; anchorIndex++)
            {
                var anchorLabel = $"{label}.native_trust_anchors[{anchorIndex}]";
                var anchor = ParseNativeAnchor(anchorValues[anchorIndex], lane, anchorLabel)
                    ?? throw new ArgumentException($"{anchorLabel} must not be null.");
                if (!anchorHashes.Add(Convert.ToHexString(anchor.AnchorHash)))
                {
                    throw new ArgumentException($"{label} contains a duplicate native trust-anchor hash.");
                }

                if (previousAnchor is not null
                    && (anchor.Backend != previousAnchor.Backend
                        || anchor.CheckpointHeight <= previousAnchor.CheckpointHeight))
                {
                    throw new ArgumentException(
                        $"{label}.native_trust_anchors must advance monotonically within one backend.");
                }

                anchors.Add(anchor);
                previousAnchor = anchor;
            }

            var currentHashElement = item.GetProperty("current_native_trust_anchor_hash");
            var currentAnchorHash = currentHashElement.ValueKind switch
            {
                JsonValueKind.Null => null,
                JsonValueKind.String => UpperHex(item, "current_native_trust_anchor_hash", 32),
                _ => throw new ArgumentException(
                    $"{label}.current_native_trust_anchor_hash must be canonical uppercase hex or null."),
            };
            var expectedCurrentAnchorHash = previousAnchor?.AnchorHash;
            var currentAnchorMatches = currentAnchorHash is null
                ? expectedCurrentAnchorHash is null
                : expectedCurrentAnchorHash is not null
                    && currentAnchorHash.AsSpan().SequenceEqual(expectedCurrentAnchorHash);
            if (!currentAnchorMatches)
            {
                throw new ArgumentException(
                    $"{label}.current_native_trust_anchor_hash must name the last retained anchor.");
            }
            var currentNativeTrustAnchor = previousAnchor;

            var routeValues = Array(item, "routes");
            if (routeValues.Length < 1)
            {
                throw new ArgumentException("SCCP registry route bounds are invalid.");
            }
            if (routeValues.Length > 64)
            {
                throw new ArgumentException(
                    $"{label} contains more than 64 retained route revisions.");
            }

            var routes = new List<SccpGovernedRouteV1>();
            var laneLiveRouteCount = 0;
            for (var routeIndex = 0; routeIndex < routeValues.Length; routeIndex++)
            {
                var route = ParseGovernedRoute(
                    routeValues[routeIndex],
                    lane,
                    currentNativeTrustAnchor,
                    $"lanes[{index}].routes[{routeIndex}]");
                var routeKey = $"{laneKey}\0{route.RouteId}\0{route.AssetKey}\0{route.Revision}";
                if (!routeKeys.Add(routeKey))
                {
                    throw new ArgumentException("SCCP registry contains a duplicate route key.");
                }

                if (!bindings.Add(Convert.ToHexString(route.Destination.DestinationBindingHash))
                    || !configurations.Add(Convert.ToHexString(route.RouteConfigurationHash)))
                {
                    throw new ArgumentException("SCCP registry reuses a destination or route-configuration hash.");
                }

                if (route.InboundFinalityCutoff is { } cutoff)
                {
                    var anchorIndex = anchors.FindIndex(anchor =>
                        anchor.AnchorHash.AsSpan().SequenceEqual(cutoff.TrustAnchorHash));
                    if (anchorIndex < 0
                        || anchorIndex + 1 >= anchors.Count
                        || anchors[anchorIndex + 1].CheckpointHeight != cutoff.MaxAnchorIntervalHeight)
                    {
                        throw new ArgumentException(
                            $"{label}.routes[{routeIndex}].inbound_finality_cutoff must close one complete retained anchor interval.");
                    }
                }

                if (route.Activation != SccpRouteActivationV1.Retired)
                {
                    laneLiveRouteCount++;
                    liveRouteCount++;
                    if (laneLiveRouteCount > 8 || liveRouteCount > 64)
                    {
                        throw new ArgumentException("SCCP registry route bounds are invalid.");
                    }
                }

                routes.Add(route);
            }

            ValidateLineages(routes);
            lanes.Add(new SccpGovernedLaneV1(lane, anchors, currentAnchorHash, routes));
        }

        return new SccpRegistryV1(1, lanes, json.ToArray());
    }

    internal static SccpMessageBundleV1 ParseBundle(ReadOnlyMemory<byte> json)
    {
        using var document = SccpJson.Parse(json, "SCCP message bundle");
        var root = document.RootElement;
        SccpJson.ExactFields(root,
            ["version", "commitment_root", "commitment", "merkle_proof", "payload", "finality_proof"],
            "SCCP message bundle");
        RequireVersion(root, "SCCP message bundle");
        var rootHash = PrefixedHash(root, "commitment_root");
        var commitment = Object(root, "commitment");
        SccpJson.ExactFields(commitment,
            ["version", "kind", "context", "message_id", "payload_hash"],
            "SCCP message commitment");
        RequireVersion(commitment, "SCCP message commitment");
        if (SccpJson.Text(commitment, "kind") != "Transfer")
        {
            throw new ArgumentException("SCCP message commitment kind is unsupported or retired.");
        }

        var messageId = PrefixedHash(commitment, "message_id");
        var payloadHash = PrefixedHash(commitment, "payload_hash");
        var context = Object(commitment, "context");
        SccpJson.ExactFields(context,
            ["lane", "destination_binding_hash", "route_configuration_hash"],
            "SCCP message context");
        var lane = ParseOutboundLane(Object(context, "lane"), "SCCP message context lane");
        var destinationBindingHash = PrefixedHash(context, "destination_binding_hash");
        var routeConfigurationHash = PrefixedHash(context, "route_configuration_hash");
        DistinctHexRoles(
            [messageId, payloadHash, rootHash, destinationBindingHash, routeConfigurationHash],
            "SCCP bundle hash roles");
        var merkle = Object(root, "merkle_proof");
        SccpJson.ExactFields(merkle, ["steps"], "SCCP Merkle proof");
        var stepValues = Array(merkle, "steps");
        if (stepValues.Length > 64)
        {
            throw new ArgumentException("SCCP Merkle proof is too deep.");
        }

        var steps = new List<SccpMerkleStepV1>(stepValues.Length);
        for (var index = 0; index < stepValues.Length; index++)
        {
            var step = stepValues[index];
            SccpJson.ExactFields(
                step,
                ["sibling_hash", "sibling_is_left"],
                $"SCCP Merkle proof.steps[{index}]");
            steps.Add(new SccpMerkleStepV1(
                PrefixedHashBytes(step, "sibling_hash"),
                SccpJson.Boolean(step, "sibling_is_left")));
        }

        var finalityProof = VariableHex(root, "finality_proof");
        SccpV1.RequireCanonicalFinalityFrame(finalityProof);
        var payload = Object(root, "payload");
        SccpJson.ExactFields(payload, ["Transfer"], "SCCP payload");
        var transfer = ParseTransferPayload(Object(payload, "Transfer"), lane);
        var exactContext = new SccpOutboundMessageContextV1(
            lane,
            DecodePrefixedHash(destinationBindingHash),
            DecodePrefixedHash(routeConfigurationHash));
        var exactCommitment = SccpV1.Commitment(exactContext, transfer);
        if ("0x" + SccpV1.LowerHex(exactCommitment.MessageId) != messageId
            || "0x" + SccpV1.LowerHex(exactCommitment.PayloadHash) != payloadHash)
        {
            throw new ArgumentException("SCCP bundle commitment does not match its canonical payload.");
        }

        var computedRoot = SccpV1.MerkleRootFromCommitment(exactCommitment, steps);
        if ("0x" + SccpV1.LowerHex(computedRoot) != rootHash)
        {
            throw new ArgumentException("SCCP bundle commitment root does not match its Merkle proof.");
        }

        return new SccpMessageBundleV1(
            1,
            rootHash,
            messageId,
            lane.Target.DomainId(),
            transfer,
            exactCommitment,
            steps.AsReadOnly(),
            finalityProof,
            json.ToArray());
    }

    internal static SccpGroth16ProofRequestV1 ParseProofRequest(ReadOnlyMemory<byte> json)
    {
        using var document = SccpJson.Parse(json, "SCCP proof request");
        var root = document.RootElement;
        SccpJson.ExactFields(root,
        [
            "version", "backend", "source_network", "target_network", "public_inputs", "verifying_key",
            "verifier_key_hash", "semantic_proof_profile", "semantic_proof_profile_hash",
            "sora_finality_anchor", "sora_finality_anchor_hash", "bundle_bytes", "statement_hash",
            "destination_binding_hash", "route_configuration_hash", "request_hash",
        ], "SCCP proof request");
        RequireVersion(root, "SCCP proof request");
        var backend = ParseDestinationBackend(Object(root, "backend"));
        var source = ParseNetwork(Object(root, "source_network"), "source_network");
        var target = ParseNetwork(Object(root, "target_network"), "target_network");
        var targetIsTron = target.ProfileKey().StartsWith("tron-", StringComparison.Ordinal);
        if (source != SccpNetworkV1.SoraTaira || !target.IsExternal()
            || (backend == SccpDestinationProofBackendV1.TronGroth16Bn254) != targetIsTron)
        {
            throw new ArgumentException("SCCP proof backend does not match an exact Taira-to-external lane.");
        }

        var inputs = Object(root, "public_inputs");
        SccpJson.ExactFields(inputs,
            ["version", "message_id", "payload_hash", "target_domain", "commitment_root", "finality_height", "finality_block_hash"],
            "SCCP proof public inputs");
        RequireVersion(inputs, "SCCP proof public inputs");
        var targetDomain = SccpJson.UInt32(inputs, "target_domain", 1, 5);
        if (targetDomain != target.DomainId())
        {
            throw new ArgumentException("SCCP target profile/domain mismatch.");
        }

        var messageId = PrefixedHash(inputs, "message_id");
        var payloadHash = PrefixedHash(inputs, "payload_hash");
        var commitmentRoot = PrefixedHash(inputs, "commitment_root");
        var finalityHash = PrefixedHash(inputs, "finality_block_hash");
        var finalityHeight = DecimalUInt64(inputs, "finality_height", 1);
        var keyBytes = ParseVerifyingKey(Object(root, "verifying_key"), "SCCP proof verifying key");
        var keyHash = PrefixedHash(root, "verifier_key_hash");
        if ("0x" + SccpV1.LowerHex(SccpV1.Keccak256(keyBytes)) != keyHash)
        {
            throw new ArgumentException("verifier_key_hash does not match the exact 38-word verifying key.");
        }

        var semantic = ParseSemanticProfile(Object(root, "semantic_proof_profile"), "semantic_proof_profile");
        var semanticHash = PrefixedHash(root, "semantic_proof_profile_hash");
        if ("0x" + SccpV1.LowerHex(semantic.ProfileHash) != semanticHash)
        {
            throw new ArgumentException("semantic_proof_profile_hash does not match its typed profile.");
        }

        var anchor = ParseFinalityAnchor(Object(root, "sora_finality_anchor"), "sora_finality_anchor");
        var anchorHash = PrefixedHash(root, "sora_finality_anchor_hash");
        if ("0x" + SccpV1.LowerHex(anchor.AnchorHash) != anchorHash)
        {
            throw new ArgumentException("sora_finality_anchor_hash does not match its typed anchor.");
        }

        var statement = PrefixedHash(root, "statement_hash");
        var binding = PrefixedHash(root, "destination_binding_hash");
        var configuration = PrefixedHash(root, "route_configuration_hash");
        var request = PrefixedHash(root, "request_hash");
        DistinctHexRoles(
            [messageId, payloadHash, commitmentRoot, finalityHash, keyHash, semanticHash, anchorHash, statement, binding, configuration, request],
            "SCCP proof-request hash roles");
        var bundleBytes = VariableHex(root, "bundle_bytes");
        var bundle = SccpV1.DecodeCanonicalMessageBundle(bundleBytes);
        if (bundle.Commitment.Context.Lane.Source != source
            || bundle.Commitment.Context.Lane.Target != target
            || "0x" + SccpV1.LowerHex(bundle.Commitment.MessageId) != messageId
            || "0x" + SccpV1.LowerHex(bundle.Commitment.PayloadHash) != payloadHash
            || "0x" + SccpV1.LowerHex(bundle.CommitmentRoot) != commitmentRoot
            || "0x" + SccpV1.LowerHex(bundle.Commitment.Context.DestinationBindingHash) != binding
            || "0x" + SccpV1.LowerHex(bundle.Commitment.Context.RouteConfigurationHash) != configuration
            || bundle.Payload.Destination != targetDomain)
        {
            throw new ArgumentException("SCCP proof request does not match its canonical message bundle.");
        }

        var hashes = SccpV1.CanonicalProofRequestHashes(
            backend,
            source,
            target,
            DecodePrefixedHash(messageId),
            DecodePrefixedHash(payloadHash),
            targetDomain,
            DecodePrefixedHash(commitmentRoot),
            finalityHeight,
            DecodePrefixedHash(finalityHash),
            bundleBytes,
            keyBytes,
            DecodePrefixedHash(keyHash),
            semantic,
            DecodePrefixedHash(semanticHash),
            anchor,
            DecodePrefixedHash(anchorHash),
            DecodePrefixedHash(binding),
            DecodePrefixedHash(configuration));
        if ("0x" + SccpV1.LowerHex(hashes.StatementHash) != statement
            || "0x" + SccpV1.LowerHex(hashes.RequestHash) != request)
        {
            throw new ArgumentException("SCCP proof-request statement or request hash is not canonical.");
        }

        return new SccpGroth16ProofRequestV1(
            1, backend, source, target, messageId, payloadHash, targetDomain, commitmentRoot,
            finalityHeight, finalityHash, keyHash, semantic, anchor, statement, binding,
            configuration, request, bundleBytes, json.ToArray());
    }

    internal static SccpRecentMessages ParseRecent(ReadOnlyMemory<byte> json)
    {
        using var document = SccpJson.Parse(json, "SCCP recent messages");
        var root = document.RootElement;
        SccpJson.ExactFields(
            root,
            ["items", "next"],
            ["items"],
            "SCCP recent messages");
        var values = Array(root, "items");
        if (values.Length > 50)
        {
            throw new ArgumentException("SCCP recent response exceeds 50 items.");
        }

        var messages = new List<SccpRecentMessage>();
        var ids = new HashSet<string>(StringComparer.Ordinal);
        for (var index = 0; index < values.Length; index++)
        {
            var item = values[index];
            HashSet<string> required =
            [
                "height", "commitment_index", "message_id_hex", "kind", "source_profile", "target_profile",
                "destination_binding_hash", "route_configuration_hash", "target_domain", "amount",
                "payload_projection", "links",
            ];
            var allowed = new HashSet<string>(required, StringComparer.Ordinal)
            {
                "asset_id", "route_id", "recipient",
            };
            SccpJson.ExactFields(item, allowed, required, $"items[{index}]");
            if (SccpJson.Text(item, "kind") != "transfer")
            {
                throw new ArgumentException("Recent SCCP message kind is unsupported or retired.");
            }

            var source = SccpNetworkV1Extensions.ParseProfileKey(SccpJson.Text(item, "source_profile"));
            var target = SccpNetworkV1Extensions.ParseProfileKey(SccpJson.Text(item, "target_profile"));
            var lane = new SccpLaneIdV1(source, target);
            var targetDomain = SccpJson.UInt32(item, "target_domain", 1, 5);
            if (!lane.IsOutbound || source != SccpNetworkV1.SoraTaira
                || targetDomain != target.DomainId())
            {
                throw new ArgumentException("Recent SCCP lane/profile/domain is invalid.");
            }

            var id = UnprefixedHash(item, "message_id_hex");
            if (!ids.Add(id))
            {
                throw new ArgumentException("Recent SCCP message ids must be unique.");
            }

            var binding = PrefixedHash(item, "destination_binding_hash");
            var configuration = PrefixedHash(item, "route_configuration_hash");
            DistinctHexRoles(["0x" + id, binding, configuration], "Recent SCCP hash roles");
            var amount = DecimalText(item, "amount", 1);
            var links = Object(item, "links");
            SccpJson.ExactFields(links, ["bundle_path", "proof_request_path"], "Recent SCCP links");
            var bundlePath = $"/v1/sccp/proofs/message/{id}";
            var requestPath = $"/v1/sccp/proof-requests/{id}";
            if (Path(links, "bundle_path") != bundlePath || Path(links, "proof_request_path") != requestPath)
            {
                throw new ArgumentException("Recent SCCP links must name the exact message.");
            }

            var rawProjection = item.GetProperty("payload_projection");
            if (rawProjection.ValueKind != JsonValueKind.Object)
            {
                throw new ArgumentException("payload_projection must be an object.");
            }
            var projection = ExactProjection(
                rawProjection,
                $"items[{index}].payload_projection",
                targetDomain,
                amount);
            if (!TryParseUInt128(amount, out _))
            {
                throw new ArgumentException("Recent SCCP amount must fit UInt128.");
            }

            var assetId = OptionalText(item, "asset_id");
            var routeId = OptionalText(item, "route_id");
            var recipient = OptionalText(item, "recipient");
            var expectedRoute = targetDomain switch
            {
                1 => "taira_eth_xor",
                2 => "taira_bsc_xor",
                5 => "taira_tron_xor",
                _ => throw new ArgumentException("Recent SCCP target domain is unsupported."),
            };
            if (assetId is not null && assetId != "xor"
                || routeId is not null && routeId != expectedRoute
                || recipient is not null)
            {
                throw new ArgumentException("Recent SCCP summary fields disagree with payload_projection.");
            }

            messages.Add(new SccpRecentMessage(
                SccpJson.UInt64(item, "height", 1),
                SccpJson.UInt32(item, "commitment_index", 0, 511),
                id, SccpPayloadKindV1.Transfer, lane,
                binding, configuration, assetId, routeId, recipient, amount, projection,
                new SccpRecentMessageLinks(bundlePath, requestPath)));
        }

        for (var index = 1; index < messages.Count; index++)
        {
            var previous = messages[index - 1];
            var current = messages[index];
            if (current.Height > previous.Height)
            {
                throw new ArgumentException("Recent SCCP messages must be newest first.");
            }

            if (current.Height == previous.Height
                && current.CommitmentIndex != previous.CommitmentIndex + 1)
            {
                throw new ArgumentException(
                    "Recent SCCP messages at one height must have contiguous ascending commitment indices.");
            }

            if (current.Height < previous.Height && current.CommitmentIndex != 0)
            {
                throw new ArgumentException(
                    "Recent SCCP messages at an older height must begin at commitment index zero.");
            }
        }

        SccpRecentCursor? next = null;
        if (root.TryGetProperty("next", out var nextElement))
        {
            SccpJson.ExactFields(nextElement, ["from", "after_index"], "SCCP recent cursor");
            next = new SccpRecentCursor(
                SccpJson.UInt64(nextElement, "from", 1),
                SccpJson.UInt32(nextElement, "after_index", 0, 511));
            var last = messages.LastOrDefault();
            if (last is null
                || next.From != last.Height
                || next.AfterIndex != last.CommitmentIndex)
            {
                throw new ArgumentException(
                    "Recent SCCP continuation must identify the last returned item.");
            }
        }

        return new SccpRecentMessages(messages, next);
    }

    private static SccpGovernedRouteV1 ParseGovernedRoute(
        JsonElement item,
        SccpLaneIdV1 expectedLane,
        SccpNativeTrustAnchorV1? currentNativeAnchor,
        string label)
    {
        SccpJson.ExactFields(item,
            [
                "lane_id", "route_id", "asset_key", "revision", "activation",
                "inbound_finality_cutoff", "source_identity", "destination", "settlement",
            ],
            label);
        var lane = ParseInboundLane(Object(item, "lane_id"), $"{label}.lane_id");
        if (lane != expectedLane)
        {
            throw new ArgumentException($"{label}.lane_id does not match its parent lane.");
        }

        var routeId = RouteKey(item, "route_id");
        var assetKey = RouteKey(item, "asset_key");
        var revision = SccpJson.UInt32(item, "revision", 1, uint.MaxValue);
        var activation = ParseActivation(Object(item, "activation"), $"{label}.activation");
        SccpInboundFinalityCutoffV1? cutoff = null;
        var cutoffElement = item.GetProperty("inbound_finality_cutoff");
        if (cutoffElement.ValueKind != JsonValueKind.Null)
        {
            if (cutoffElement.ValueKind != JsonValueKind.Object)
            {
                throw new ArgumentException($"{label}.inbound_finality_cutoff must be an object or null.");
            }

            SccpJson.ExactFields(
                cutoffElement,
                ["trust_anchor_hash", "max_anchor_interval_height"],
                $"{label}.inbound_finality_cutoff");
            cutoff = new SccpInboundFinalityCutoffV1(
                UpperHex(cutoffElement, "trust_anchor_hash", 32),
                SccpJson.UInt64(cutoffElement, "max_anchor_interval_height", 1));
        }

        if ((activation == SccpRouteActivationV1.Retired) != (cutoff is not null))
        {
            throw new ArgumentException(
                $"{label}.inbound_finality_cutoff must be present exactly for a retired route.");
        }
        var source = ParseSourceIdentity(Object(item, "source_identity"), lane, $"{label}.source_identity");
        var destination = ParseDestination(Object(item, "destination"), lane, $"{label}.destination");
        var sourceParts = SourceParts(source);
        if (sourceParts.Family != destination.Family
            || !sourceParts.Address.AsSpan().SequenceEqual(destination.RouteAddress)
            || !sourceParts.Runtime.AsSpan().SequenceEqual(destination.RouteCodeHash))
        {
            throw new ArgumentException($"{label} source identity does not name its destination route deployment.");
        }

        var settlement = Object(item, "settlement");
        SccpJson.ExactFields(settlement,
            ["asset_definition_id", "custody_account_id", "payload_amount_scale"],
            $"{label}.settlement");
        var assetDefinition = SccpJson.Text(settlement, "asset_definition_id");
        if (assetDefinition != "6TEAJqbb8oEPmLncoNiMRbLEK6tw")
        {
            throw new ArgumentException($"{label} must settle canonical Taira XOR.");
        }

        var custody = SccpJson.Text(settlement, "custody_account_id");
        AccountAddress custodyAddress;
        try
        {
            custodyAddress = AccountAddress.Parse(custody, AccountAddress.DefaultChainDiscriminant);
        }
        catch (Exception error) when (error is ArgumentException or FormatException)
        {
            throw new ArgumentException($"{label}.custody_account_id must be canonical.", error);
        }

        if (custodyAddress.ToI105() != custody)
        {
            throw new ArgumentException($"{label}.custody_account_id must be canonical.");
        }

        var scale = SccpJson.UInt32(settlement, "payload_amount_scale", 9, 9);
        var configuration = RouteConfigurationHash(lane, routeId, assetKey, revision, destination);
        if (!sourceParts.Configuration.AsSpan().SequenceEqual(configuration))
        {
            throw new ArgumentException($"{label} source route_config_hash does not match the immutable deployment.");
        }

        if (activation.AllowsInbound()
            && (currentNativeAnchor is null || !currentNativeAnchor.Backend.Supports(lane.Source)))
        {
            throw new ArgumentException($"{label} enables inbound settlement without a matching native trust anchor.");
        }

        return new SccpGovernedRouteV1(
            lane, routeId, assetKey, revision, activation, cutoff, source, destination,
            assetDefinition, custody, scale, configuration);
    }

    private static SccpDestinationDeploymentV1 ParseDestination(JsonElement item, SccpLaneIdV1 lane, string label)
    {
        SccpJson.ExactFields(item, ["family", "deployment"], label);
        var family = SccpJson.Text(item, "family") switch
        {
            "evm" => SccpDestinationProofBackendV1.EvmGroth16Bn254,
            "tron" => SccpDestinationProofBackendV1.TronGroth16Bn254,
            _ => throw new ArgumentException($"{label} family is unsupported or retired."),
        };
        var laneIsTron = lane.Source.ProfileKey().StartsWith("tron-", StringComparison.Ordinal);
        if ((family == SccpDestinationProofBackendV1.TronGroth16Bn254) != laneIsTron)
        {
            throw new ArgumentException($"{label} family does not match its lane.");
        }

        var deployment = Object(item, "deployment");
        SccpJson.ExactFields(deployment,
        [
            "token_address", "token_code_hash", "verifier_address", "verifier_code_hash",
            "verifying_key", "verifier_key_hash", "outbound_proof_policy", "route_address",
            "route_code_hash", "taira_to_token_multiplier",
        ], $"{label}.deployment");
        var addresses = new[]
        {
            UpperHex(deployment, "token_address", 20),
            UpperHex(deployment, "verifier_address", 20),
            UpperHex(deployment, "route_address", 20),
        };
        var hashes = new[]
        {
            UpperHex(deployment, "token_code_hash", 32),
            UpperHex(deployment, "verifier_code_hash", 32),
            UpperHex(deployment, "verifier_key_hash", 32),
            UpperHex(deployment, "route_code_hash", 32),
        };
        RequireDistinctBytes(addresses, $"{label}.deployment addresses");
        RequireDistinctBytes(hashes, $"{label}.deployment hashes");
        var key = ParseVerifyingKey(Object(deployment, "verifying_key"), $"{label}.deployment.verifying_key");
        if (!SccpV1.Keccak256(key).AsSpan().SequenceEqual(hashes[2]))
        {
            throw new ArgumentException($"{label}.deployment.verifier_key_hash does not match verifying_key.");
        }

        var (semantic, anchor) = ParseOutboundPolicy(
            Object(deployment, "outbound_proof_policy"),
            $"{label}.deployment.outbound_proof_policy");
        RequireDistinctBytes(
            hashes.Concat([semantic.ProfileHash, anchor.AnchorHash]),
            $"{label}.deployment proof-policy and deployment hashes");
        if (SccpJson.UInt64(deployment, "taira_to_token_multiplier", 1_000_000_000) != 1_000_000_000)
        {
            throw new ArgumentException($"{label}.deployment has the wrong Taira/token multiplier.");
        }

        var partial = new SccpDestinationDeploymentV1(
            family, addresses[0], hashes[0], addresses[1], hashes[1], hashes[2], semantic, anchor,
            addresses[2], hashes[3], 1_000_000_000, []);
        return partial with { DestinationBindingHash = DestinationBindingHash(lane, partial) };
    }

    private static (SccpSemanticProofProfileV1 Semantic, SccpSoraFinalityAnchorV1 Anchor) ParseOutboundPolicy(
        JsonElement item,
        string label)
    {
        SccpJson.ExactFields(item, ["version", "semantic_profile", "sora_finality_anchor"], label);
        RequireVersion(item, label);
        var semantic = ParseSemanticProfile(Object(item, "semantic_profile"), $"{label}.semantic_profile");
        var anchor = ParseFinalityAnchor(Object(item, "sora_finality_anchor"), $"{label}.sora_finality_anchor");
        RequireDistinctBytes(
        [
            semantic.CircuitCommitment, semantic.WitnessGeneratorCommitment, semantic.PublicSignalSchemaHash,
            semantic.ProfileHash, anchor.ChainIdHash, anchor.CheckpointBlockHash,
            anchor.CheckpointContextId, anchor.CheckpointFinalityArtifactHash, anchor.AnchorHash,
        ], $"{label} hashes");
        return (semantic, anchor);
    }

    private static SccpSemanticProofProfileV1 ParseSemanticProfile(JsonElement item, string label)
    {
        SccpJson.ExactFields(item, ["profile", "commitments"], label);
        if (SccpJson.Text(item, "profile") != "sora_taira_finality_inclusion_groth16_bn254")
        {
            throw new ArgumentException($"{label} is unsupported or retired.");
        }

        var commitments = Object(item, "commitments");
        SccpJson.ExactFields(commitments,
            ["version", "circuit_commitment", "witness_generator_commitment", "public_signal_schema_hash"],
            $"{label}.commitments");
        RequireVersion(commitments, $"{label}.commitments");
        var circuit = UpperHex(commitments, "circuit_commitment", 32);
        var witness = UpperHex(commitments, "witness_generator_commitment", 32);
        var schema = UpperHex(commitments, "public_signal_schema_hash", 32);
        if (!schema.AsSpan().SequenceEqual(PublicSignalSchemaHash()))
        {
            throw new ArgumentException($"{label} does not commit the exact eleven-signal schema.");
        }

        RequireDistinctBytes([circuit, witness, schema], $"{label} commitments");
        var canonical = Concat([1, 0, 1], circuit, witness, schema);
        var hash = SccpV1.Keccak256(Concat("sccp:semantic-proof-profile:v1"u8.ToArray(), canonical));
        return new SccpSemanticProofProfileV1(circuit, witness, schema, hash);
    }

    private static SccpSoraFinalityAnchorV1 ParseFinalityAnchor(JsonElement item, string label)
    {
        SccpJson.ExactFields(item,
        [
            "version", "source_network", "protocol_version", "chain_id_hash", "checkpoint_height",
            "checkpoint_block_hash", "checkpoint_context_id", "checkpoint_finality_artifact_hash",
        ], label);
        RequireVersion(item, label);
        if (ParseNetwork(Object(item, "source_network"), $"{label}.source_network") != SccpNetworkV1.SoraTaira)
        {
            throw new ArgumentException($"{label} source network must be Taira.");
        }

        var protocolVersion = checked((ushort)SccpJson.UInt32(item, "protocol_version", 3, 4));
        var chainHash = UpperHex(item, "chain_id_hash", 32);
        if (!chainHash.AsSpan().SequenceEqual(SccpV1.Keccak256(TairaChainId)))
        {
            throw new ArgumentException($"{label}.chain_id_hash is not Taira.");
        }

        var checkpointHeight = SccpJson.UInt64(item, "checkpoint_height", 1);
        var checkpointHash = UpperHex(item, "checkpoint_block_hash", 32);
        var contextId = UpperHex(item, "checkpoint_context_id", 32);
        var artifactHash = UpperHex(item, "checkpoint_finality_artifact_hash", 32);
        RequireDistinctBytes([chainHash, checkpointHash, contextId, artifactHash], $"{label} finality hashes");
        using var canonical = new MemoryStream();
        canonical.WriteByte(1);
        canonical.WriteByte((byte)SccpNetworkV1.SoraTaira);
        WriteUInt16LittleEndian(canonical, protocolVersion);
        canonical.Write(chainHash);
        WriteUInt64LittleEndian(canonical, checkpointHeight);
        canonical.Write(checkpointHash);
        canonical.Write(contextId);
        canonical.Write(artifactHash);
        var hash = SccpV1.Keccak256(Concat("sccp:sora-finality-anchor:v1"u8.ToArray(), canonical.ToArray()));
        return new SccpSoraFinalityAnchorV1(
            protocolVersion, chainHash, checkpointHeight, checkpointHash, contextId, artifactHash, hash);
    }

    private static byte[] ParseVerifyingKey(JsonElement item, string label)
    {
        SccpJson.ExactFields(item, ["version", "alpha1", "beta2", "gamma2", "delta2", "ic"], label);
        RequireVersion(item, label);
        var words = new List<byte[]>();
        words.AddRange(ParseG1(Object(item, "alpha1"), $"{label}.alpha1"));
        foreach (var field in new[] { "beta2", "gamma2", "delta2" })
        {
            words.AddRange(ParseG2(Object(item, field), $"{label}.{field}"));
        }

        var ic = Object(item, "ic");
        var icFields = new[] { "constant" }.Concat(Enumerable.Range(0, 11).Select(static index => $"signal_{index}")).ToArray();
        SccpJson.ExactFields(ic, new HashSet<string>(icFields, StringComparer.Ordinal), $"{label}.ic");
        foreach (var field in icFields)
        {
            words.AddRange(ParseG1(Object(ic, field), $"{label}.ic.{field}"));
        }

        if (words.Count != 38)
        {
            throw new ArgumentException($"{label} must contain exactly 38 ABI words.");
        }

        return words.SelectMany(static value => value).ToArray();
    }

    private static byte[][] ParseG1(JsonElement item, string label)
    {
        SccpJson.ExactFields(item, ["x", "y"], label);
        var result = new[] { UpperHex(item, "x", 32, true), UpperHex(item, "y", 32, true) };
        if (result.All(IsZero) || result.Any(static value => value.AsSpan().SequenceCompareTo(Bn254BaseField) >= 0))
        {
            throw new ArgumentException($"{label} is not a canonical non-infinity BN254 G1 point.");
        }

        return result;
    }

    private static byte[][] ParseG2(JsonElement item, string label)
    {
        string[] fields = ["x_c0", "x_c1", "y_c0", "y_c1"];
        SccpJson.ExactFields(item, new HashSet<string>(fields, StringComparer.Ordinal), label);
        var result = fields.Select(field => UpperHex(item, field, 32, true)).ToArray();
        if (result.All(IsZero) || result.Any(static value => value.AsSpan().SequenceCompareTo(Bn254BaseField) >= 0))
        {
            throw new ArgumentException($"{label} is not a canonical non-infinity BN254 G2 point.");
        }

        return result;
    }

    private static SccpNativeTrustAnchorV1? ParseNativeAnchor(
        JsonElement item,
        SccpLaneIdV1 lane,
        string label)
    {
        if (item.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        if (item.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException($"{label} must be an object or null.");
        }

        SccpJson.ExactFields(item, ["backend", "anchor_hash", "checkpoint_height"], label);
        var backendObject = Object(item, "backend");
        SccpJson.ExactFields(backendObject, ["backend", "protocol"], $"{label}.backend");
        if (backendObject.GetProperty("protocol").ValueKind != JsonValueKind.Null)
        {
            throw new ArgumentException($"{label}.backend.protocol must be null.");
        }

        var backend = SccpNativeBackendV1Extensions.ParseWireKey(SccpJson.Text(backendObject, "backend"));
        if (!backend.Supports(lane.Source))
        {
            throw new ArgumentException($"{label} backend does not match its lane.");
        }

        return new SccpNativeTrustAnchorV1(
            backend,
            UpperHex(item, "anchor_hash", 32),
            SccpJson.UInt64(item, "checkpoint_height", 1));
    }

    private static SccpSourceEmitterV1 ParseSourceIdentity(JsonElement item, SccpLaneIdV1 expectedLane, string label)
    {
        SccpJson.ExactFields(item, ["lane", "emitter"], label);
        if (ParseInboundLane(Object(item, "lane"), $"{label}.lane") != expectedLane)
        {
            throw new ArgumentException($"{label}.lane does not match its route.");
        }

        var emitter = Object(item, "emitter");
        SccpJson.ExactFields(emitter, ["emitter", "identity"], $"{label}.emitter");
        var family = SccpJson.Text(emitter, "emitter");
        var identity = Object(emitter, "identity");
        SccpJson.ExactFields(identity,
            ["address", "runtime_code_hash", "route_config_hash"],
            $"{label}.emitter.identity");
        var address = UpperHex(identity, "address", 20);
        var runtime = UpperHex(identity, "runtime_code_hash", 32);
        var configuration = UpperHex(identity, "route_config_hash", 32);
        var laneIsTron = expectedLane.Source.ProfileKey().StartsWith("tron-", StringComparison.Ordinal);
        return (family, laneIsTron) switch
        {
            ("evm", false) => new SccpSourceEmitterV1.Evm(address, runtime, configuration),
            ("tron", true) => new SccpSourceEmitterV1.Tron(address, runtime, configuration),
            _ => throw new ArgumentException($"{label} emitter family does not match its lane."),
        };
    }

    private static (SccpDestinationProofBackendV1 Family, byte[] Address, byte[] Runtime, byte[] Configuration)
        SourceParts(SccpSourceEmitterV1 source) => source switch
        {
            SccpSourceEmitterV1.Evm evm =>
                (SccpDestinationProofBackendV1.EvmGroth16Bn254, evm.Address, evm.RuntimeCodeHash, evm.RouteConfigHash),
            SccpSourceEmitterV1.Tron tron =>
                (SccpDestinationProofBackendV1.TronGroth16Bn254, tron.Address, tron.RuntimeCodeHash, tron.RouteConfigHash),
            _ => throw new ArgumentException("Unsupported SCCP source emitter."),
        };

    private static SccpDestinationProofBackendV1 ParseDestinationBackend(JsonElement item)
    {
        SccpJson.ExactFields(item, ["backend", "family"], "SCCP proof backend");
        if (item.GetProperty("family").ValueKind != JsonValueKind.Null)
        {
            throw new ArgumentException("SCCP proof backend family must be null.");
        }

        return SccpJson.Text(item, "backend") switch
        {
            "evm_groth16_bn254_v1" => SccpDestinationProofBackendV1.EvmGroth16Bn254,
            "tron_groth16_bn254_v1" => SccpDestinationProofBackendV1.TronGroth16Bn254,
            _ => throw new ArgumentException("SCCP proof backend is unsupported or retired."),
        };
    }

    private static SccpRouteActivationV1 ParseActivation(JsonElement item, string label)
    {
        SccpJson.ExactFields(item, ["activation", "direction"], label);
        if (item.GetProperty("direction").ValueKind != JsonValueKind.Null)
        {
            throw new ArgumentException($"{label}.direction must be null.");
        }

        return SccpJson.Text(item, "activation") switch
        {
            "staged" => SccpRouteActivationV1.Staged,
            "bidirectional" => SccpRouteActivationV1.Bidirectional,
            "inbound_only" => SccpRouteActivationV1.InboundOnly,
            "paused" => SccpRouteActivationV1.Paused,
            "retired" => SccpRouteActivationV1.Retired,
            _ => throw new ArgumentException($"{label} is unsupported."),
        };
    }

    private static SccpNetworkV1 ParseNetwork(JsonElement item, string label)
    {
        SccpJson.ExactFields(item, ["network", "profile"], label);
        if (item.GetProperty("profile").ValueKind != JsonValueKind.Null)
        {
            throw new ArgumentException($"{label}.profile must be null.");
        }

        var wire = SccpJson.Text(item, "network");
        if (wire.Any(static value => value is not (>= 'a' and <= 'z') and not '_'))
        {
            throw new ArgumentException($"{label} is unsupported or retired.");
        }

        return SccpNetworkV1Extensions.ParseProfileKey(wire.Replace('_', '-'));
    }

    private static SccpLaneIdV1 ParseInboundLane(JsonElement item, string label)
    {
        var lane = ParseLane(item, label);
        if (!lane.IsInbound || lane.Target != SccpNetworkV1.SoraTaira)
        {
            throw new ArgumentException($"{label} must be external-to-Taira.");
        }

        return lane;
    }

    private static SccpLaneIdV1 ParseOutboundLane(JsonElement item, string label)
    {
        var lane = ParseLane(item, label);
        if (!lane.IsOutbound || lane.Source != SccpNetworkV1.SoraTaira)
        {
            throw new ArgumentException($"{label} must be Taira-to-external.");
        }

        return lane;
    }

    private static SccpLaneIdV1 ParseLane(JsonElement item, string label)
    {
        SccpJson.ExactFields(item, ["source", "target"], label);
        return new SccpLaneIdV1(
            ParseNetwork(Object(item, "source"), $"{label}.source"),
            ParseNetwork(Object(item, "target"), $"{label}.target"));
    }

    private static SccpTransferPayloadV1 ParseTransferPayload(JsonElement item, SccpLaneIdV1 lane)
    {
        SccpJson.ExactFields(item,
        [
            "version", "source_domain", "dest_domain", "nonce", "route_revision", "asset_home_domain",
            "asset_id_codec", "asset_id", "amount", "sender_codec", "sender", "recipient_codec",
            "recipient", "route_id_codec", "route_id",
        ], "SCCP transfer payload");
        RequireVersion(item, "SCCP transfer payload");
        if (SccpJson.UInt32(item, "source_domain", 0, 5) != lane.Source.DomainId()
            || SccpJson.UInt32(item, "dest_domain", 0, 5) != lane.Target.DomainId())
        {
            throw new ArgumentException("SCCP transfer payload does not match its exact lane.");
        }

        var nonce = DecimalUInt64(item, "nonce", 0);
        var routeRevision = SccpJson.UInt32(item, "route_revision", 1, uint.MaxValue);
        var assetHomeDomain = SccpJson.UInt32(item, "asset_home_domain", 0, 5);
        if (assetHomeDomain is not (0 or 1 or 2 or 5))
        {
            throw new ArgumentException("SCCP transfer asset_home_domain is unsupported or retired.");
        }

        var amount = DecimalText(item, "amount", 1);
        const string maximumUInt128 = "340282366920938463463374607431768211455";
        if (amount.Length > maximumUInt128.Length
            || amount.Length == maximumUInt128.Length
                && string.CompareOrdinal(amount, maximumUInt128) > 0)
        {
            throw new ArgumentException("SCCP transfer amount must fit UInt128.");
        }

        if (!TryParseUInt128(amount, out var parsedAmount))
        {
            throw new ArgumentException("SCCP transfer amount must fit UInt128.");
        }

        var senderCodec = SccpJson.UInt32(item, "sender_codec", 1, 5);
        var recipientCodec = SccpJson.UInt32(item, "recipient_codec", 1, 5);
        var expectedRecipientCodec = lane.Target.DomainId() == 5
            ? (uint)SccpCodecV1.TronAddress21
            : (uint)SccpCodecV1.EvmAddress20;
        if (senderCodec != (uint)SccpCodecV1.CanonicalText || recipientCodec != expectedRecipientCodec)
        {
            throw new ArgumentException("SCCP transfer account codecs do not match its exact domains.");
        }

        var values = new Dictionary<string, (SccpCodecV1 Codec, byte[] Value)>(StringComparer.Ordinal);
        foreach (var (codecField, valueField) in new[]
        {
            ("asset_id_codec", "asset_id"), ("sender_codec", "sender"),
            ("recipient_codec", "recipient"), ("route_id_codec", "route_id"),
        })
        {
            var tag = SccpJson.UInt32(item, codecField, 1, 5);
            if (!Enum.IsDefined((SccpCodecV1)tag))
            {
                throw new ArgumentException("SCCP transfer uses a retired codec.");
            }

            var codec = (SccpCodecV1)tag;
            var value = codec.Validate(VariableHex(item, valueField));
            values.Add(valueField, (codec, value));
        }


        return new SccpTransferPayloadV1(
            lane.Source.DomainId(),
            lane.Target.DomainId(),
            nonce,
            routeRevision,
            assetHomeDomain,
            values["asset_id"].Codec,
            values["asset_id"].Value,
            parsedAmount,
            values["sender"].Codec,
            values["sender"].Value,
            values["recipient"].Codec,
            values["recipient"].Value,
            values["route_id"].Codec,
            values["route_id"].Value);
    }

    private static string ExactProjection(
        JsonElement projection,
        string label,
        uint expectedDestinationDomain,
        string expectedAmount)
    {
        SccpJson.ExactFields(projection, ["Transfer"], label);
        var transfer = projection.GetProperty("Transfer");
        if (transfer.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException($"{label}.Transfer must be an object.");
        }

        SccpJson.ExactFields(
            transfer,
            [
                "version", "source_domain", "dest_domain", "nonce", "route_revision",
                "asset_home_domain", "asset_id", "amount", "sender", "recipient", "route_id",
            ],
            $"{label}.Transfer");
        if (SccpJson.UInt64(transfer, "version", 1) != 1
            || SccpJson.UInt32(transfer, "source_domain", 0, 0) != 0
            || SccpJson.UInt32(transfer, "dest_domain", 1, 5) != expectedDestinationDomain
            || SccpJson.UInt32(transfer, "asset_home_domain", 0, 0) != 0)
        {
            throw new ArgumentException($"{label}.Transfer domains or version do not match the recent message.");
        }
        _ = SccpJson.UInt64(transfer, "nonce", 0);
        _ = SccpJson.UInt32(transfer, "route_revision", 1, uint.MaxValue);

        var amountElement = transfer.GetProperty("amount");
        var amount = amountElement.GetRawText();
        if (amountElement.ValueKind != JsonValueKind.Number
            || amount.Length == 0
            || amount[0] == '0'
            || amount.Any(static character => !char.IsAsciiDigit(character))
            || !TryParseUInt128(amount, out var parsedAmount)
            || parsedAmount == 0
            || amount != expectedAmount)
        {
            throw new ArgumentException($"{label}.Transfer.amount must equal the positive UInt128 readback amount.");
        }

        ExactCanonicalText(transfer, "asset_id", "xor", label);
        ExactCanonicalText(transfer, "sender", null, label);
        var expectedRoute = expectedDestinationDomain switch
        {
            1 => "taira_eth_xor",
            2 => "taira_bsc_xor",
            5 => "taira_tron_xor",
            _ => throw new ArgumentException($"{label}.Transfer destination domain is unsupported."),
        };
        ExactCanonicalText(transfer, "route_id", expectedRoute, label);
        ExactRecipient(transfer, "recipient", expectedDestinationDomain, label);

        return projection.GetRawText();
    }

    private static void ExactCanonicalText(
        JsonElement transfer,
        string field,
        string? expected,
        string label)
    {
        var tagged = Object(transfer, field);
        SccpJson.ExactFields(tagged, ["CanonicalText"], $"{label}.Transfer.{field}");
        var content = Object(tagged, "CanonicalText");
        SccpJson.ExactFields(content, ["value"], $"{label}.Transfer.{field}.CanonicalText");
        var value = SccpJson.Text(content, "value");
        try
        {
            _ = SccpCodecV1.CanonicalText.Validate(Encoding.UTF8.GetBytes(value));
        }
        catch (ArgumentException exception)
        {
            throw new ArgumentException($"{label}.Transfer.{field} is not canonical text.", exception);
        }
        if (expected is not null && value != expected)
        {
            throw new ArgumentException($"{label}.Transfer.{field} is not canonical text.");
        }
    }

    private static void ExactRecipient(
        JsonElement transfer,
        string field,
        uint destinationDomain,
        string label)
    {
        var tagged = Object(transfer, field);
        var tag = destinationDomain == 5 ? "TronAddress21" : "EvmAddress20";
        SccpJson.ExactFields(tagged, [tag], $"{label}.Transfer.{field}");
        var content = Object(tagged, tag);
        SccpJson.ExactFields(content, ["bytes"], $"{label}.Transfer.{field}.{tag}");
        var encoded = SccpJson.Text(content, "bytes");
        var expectedLength = destinationDomain == 5 ? 44 : 42;
        var bodyOffset = destinationDomain == 5 ? 4 : 2;
        if (encoded.Length != expectedLength
            || !encoded.StartsWith(destinationDomain == 5 ? "0x41" : "0x", StringComparison.Ordinal)
            || encoded.Skip(2).Any(static character =>
                !(character is >= '0' and <= '9') && !(character is >= 'a' and <= 'f'))
            || !encoded.Skip(bodyOffset).Any(static character => character != '0'))
        {
            throw new ArgumentException($"{label}.Transfer.{field} does not match its destination address codec.");
        }
    }

    private static void ValidateLineages(IEnumerable<SccpGovernedRouteV1> routes)
    {
        foreach (var lineage in routes.GroupBy(static route => (route.RouteId, route.AssetKey)))
        {
            var ordered = lineage.OrderBy(static route => route.Revision).ToArray();
            for (var index = 0; index < ordered.Length; index++)
            {
                if (ordered[index].Revision != index + 1)
                {
                    throw new ArgumentException("SCCP route revisions must start at one and have no gaps.");
                }
            }

            if (ordered.Count(static route => route.Activation.AllowsOutbound()) > 1)
            {
                throw new ArgumentException("SCCP registry enables multiple revisions of one route.");
            }
        }
    }

    private static byte[] DestinationBindingHash(SccpLaneIdV1 lane, SccpDestinationDeploymentV1 destination)
    {
        var tron = destination.Family == SccpDestinationProofBackendV1.TronGroth16Bn254;
        ulong network = lane.Source switch
        {
            SccpNetworkV1.EthereumMainnet => 1,
            SccpNetworkV1.EthereumSepolia => 11_155_111,
            SccpNetworkV1.BscMainnet => 56,
            SccpNetworkV1.BscTestnet => 97,
            SccpNetworkV1.TronMainnet => 0x2b66_53dc,
            SccpNetworkV1.TronNile => 0xcd86_90dc,
            SccpNetworkV1.TronShasta => 0x94a9_059e,
            _ => throw new ArgumentException("SCCP destination binding lane is unsupported."),
        };
        var bindingDomain = tron
            ? "iroha:sccp:tron-destination-binding:v1"
            : "iroha:sccp:evm-destination-binding:v1";
        var backend = tron ? "tron-groth16-bn254-v1" : "evm-groth16-bn254-v1";
        return SccpV1.Keccak256(Concat(
            SccpV1.Keccak256(Encoding.UTF8.GetBytes(bindingDomain)),
            SccpV1.Keccak256(Encoding.UTF8.GetBytes(backend)),
            AbiWord(network),
            AbiWord(0),
            AbiWord(lane.Source.DomainId()),
            tron ? AbiTronAddress(destination.VerifierAddress) : AbiAddress(destination.VerifierAddress),
            tron ? AbiTronAddress(destination.RouteAddress) : AbiAddress(destination.RouteAddress),
            destination.VerifierCodeHash,
            destination.VerifierKeyHash,
            destination.SemanticProofProfile.ProfileHash,
            destination.SoraFinalityAnchor.AnchorHash));
    }

    private static byte[] RouteConfigurationHash(
        SccpLaneIdV1 lane,
        string routeId,
        string assetKey,
        uint revision,
        SccpDestinationDeploymentV1 destination)
    {
        if (assetKey != "xor")
        {
            throw new ArgumentException("SCCP V1 route asset must be xor.");
        }

        var (expectedRoute, network) = lane.Source switch
        {
            SccpNetworkV1.EthereumMainnet => ("taira_eth_xor", 1UL),
            SccpNetworkV1.EthereumSepolia => ("taira_eth_xor", 11_155_111UL),
            SccpNetworkV1.BscMainnet => ("taira_bsc_xor", 56UL),
            SccpNetworkV1.BscTestnet => ("taira_bsc_xor", 97UL),
            SccpNetworkV1.TronMainnet => ("taira_tron_xor", 0x2b66_53dcUL),
            SccpNetworkV1.TronNile => ("taira_tron_xor", 0xcd86_90dcUL),
            SccpNetworkV1.TronShasta => ("taira_tron_xor", 0x94a9_059eUL),
            _ => throw new ArgumentException("SCCP route external profile is unsupported."),
        };
        if (routeId != expectedRoute)
        {
            throw new ArgumentException("SCCP route id does not match its exact deployment.");
        }

        var sourceHash = SccpV1.LaneHash(lane);
        var reverseHash = SccpV1.LaneHash(new SccpLaneIdV1(lane.Target, lane.Source));
        var routeHashRoles = new List<byte[]>
        {
            sourceHash, reverseHash, destination.TokenCodeHash, destination.VerifierCodeHash,
            destination.VerifierKeyHash, destination.SemanticProofProfile.ProfileHash,
            destination.SoraFinalityAnchor.AnchorHash,
        };
        if (destination.Family == SccpDestinationProofBackendV1.TronGroth16Bn254)
        {
            routeHashRoles.Add(destination.DestinationBindingHash);
        }
        RequireDistinctBytes(routeHashRoles, "SCCP route hash roles");
        var deploymentParts = new List<byte[]>
        {
            AbiAddress(destination.TokenAddress), destination.TokenCodeHash,
            AbiAddress(destination.VerifierAddress), destination.VerifierCodeHash,
            destination.VerifierKeyHash, destination.SemanticProofProfile.ProfileHash,
            destination.SoraFinalityAnchor.AnchorHash,
        };
        if (destination.Family == SccpDestinationProofBackendV1.TronGroth16Bn254)
        {
            deploymentParts.Add(destination.DestinationBindingHash);
        }

        var deploymentHash = SccpV1.Keccak256(Concat(deploymentParts.ToArray()));
        var assetRouteHash = SccpV1.Keccak256(Concat(
            SccpV1.Keccak256("xor"u8),
            SccpV1.Keccak256(Encoding.UTF8.GetBytes(routeId)),
            AbiWord(revision),
            AbiWord(destination.TairaToTokenMultiplier)));
        return SccpV1.Keccak256(Concat(
            SccpV1.Keccak256("sccp:concrete-route-config:v1"u8),
            AbiWord(lane.Source.DomainId()),
            AbiWord((byte)lane.Source),
            AbiWord(network),
            sourceHash,
            reverseHash,
            deploymentHash,
            assetRouteHash));
    }

    private static byte[] PublicSignalSchemaHash()
    {
        using var canonical = new MemoryStream();
        canonical.WriteByte(1);
        WriteUInt32LittleEndian(canonical, checked((uint)PublicSignalLabels.Length));
        foreach (var label in PublicSignalLabels)
        {
            var bytes = Encoding.UTF8.GetBytes(label);
            WriteUInt32LittleEndian(canonical, checked((uint)bytes.Length));
            canonical.Write(bytes);
        }

        return SccpV1.Keccak256(Concat(
            "sccp:groth16-bn254:public-signal-schema:v1"u8.ToArray(),
            canonical.ToArray()));
    }

    private static byte[] AbiAddress(byte[] value) => Concat(new byte[12], value);

    private static byte[] AbiTronAddress(byte[] value) => Concat(new byte[11], [0x41], value);

    private static byte[] AbiWord(ulong value)
    {
        var result = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(result.AsSpan(24), value);
        return result;
    }

    private static void WriteUInt16LittleEndian(Stream stream, ushort value)
    {
        Span<byte> bytes = stackalloc byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(bytes, value);
        stream.Write(bytes);
    }

    private static void WriteUInt32LittleEndian(Stream stream, uint value)
    {
        Span<byte> bytes = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(bytes, value);
        stream.Write(bytes);
    }

    private static void WriteUInt64LittleEndian(Stream stream, ulong value)
    {
        Span<byte> bytes = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, value);
        stream.Write(bytes);
    }

    private static void RequireVersion(JsonElement item, string label)
    {
        if (SccpJson.UInt64(item, "version", 1) != 1)
        {
            throw new ArgumentException($"{label} version must be exactly 1.");
        }
    }

    private static string RouteKey(JsonElement item, string field)
    {
        var value = SccpJson.Text(item, field);
        if (value.Length > 64
            || value.Any(static character => character is not (>= 'a' and <= 'z')
                && !char.IsAsciiDigit(character) && character is not ('_' or '-'))
            || value[0] is not (>= 'a' and <= 'z') && !char.IsAsciiDigit(value[0])
            || value[^1] is not (>= 'a' and <= 'z') && !char.IsAsciiDigit(value[^1]))
        {
            throw new ArgumentException($"{field} must be canonical lowercase route text.");
        }

        return value;
    }

    private static string FixedPath(JsonElement item, string field)
    {
        var value = Path(item, field);
        if (value != CapabilityPaths[field])
        {
            throw new ArgumentException($"{field} does not match the SCCP V1 endpoint.");
        }

        return value;
    }

    private static string? OptionalFixedPath(JsonElement item, string field)
    {
        if (!item.TryGetProperty(field, out var property) || property.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        return FixedPath(item, field);
    }

    private static string Path(JsonElement item, string field)
    {
        var value = SccpJson.Text(item, field);
        if (!value.StartsWith("/", StringComparison.Ordinal)
            || value.Contains("//", StringComparison.Ordinal)
            || value.Contains('?')
            || value.Contains('#')
            || value.Contains('%')
            || value.Contains('\\')
            || value.Length > 1024)
        {
            throw new ArgumentException($"{field} must be a canonical absolute Torii path.");
        }

        return value;
    }

    private static string PrefixedHash(JsonElement item, string field)
    {
        var value = SccpJson.Text(item, field);
        if (value.Length != 66 || !value.StartsWith("0x", StringComparison.Ordinal))
        {
            throw new ArgumentException($"{field} must be canonical lowercase nonzero 0x-prefixed hash.");
        }

        _ = SccpSubmitValidation.ResponseHash(value[2..], field);
        return value;
    }

    private static byte[] PrefixedHashBytes(JsonElement item, string field) =>
        DecodePrefixedHash(PrefixedHash(item, field));

    private static byte[] DecodePrefixedHash(string value) => Convert.FromHexString(value[2..]);

    private static string UnprefixedHash(JsonElement item, string field) =>
        SccpSubmitValidation.ResponseHash(SccpJson.Text(item, field), field);

    private static void DistinctHexRoles(IEnumerable<string> values, string label)
    {
        var observed = new HashSet<string>(StringComparer.Ordinal);
        if (values.Any(value => !observed.Add(value)))
        {
            throw new ArgumentException($"{label} must be role-separated.");
        }
    }

    private static byte[] UpperHex(JsonElement item, string field, int bytes, bool allowZero = false)
    {
        var value = SccpJson.Text(item, field);
        if (value.Length != bytes * 2
            || value.Any(static character => !char.IsAsciiDigit(character) && character is not (>= 'A' and <= 'F'))
            || (!allowZero && value.All(static character => character == '0')))
        {
            throw new ArgumentException($"{field} must be canonical uppercase {bytes}-byte hex.");
        }

        return Convert.FromHexString(value);
    }

    private static byte[] VariableHex(JsonElement item, string field)
    {
        var value = SccpJson.Text(item, field);
        if (!value.StartsWith("0x", StringComparison.Ordinal) || value.Length <= 2 || value.Length % 2 != 0
            || value.AsSpan(2).ContainsAnyExcept("0123456789abcdef")
            || (value.Length - 2) / 2 > MaximumWireBytes)
        {
            throw new ArgumentException($"{field} must be canonical nonempty lowercase 0x-prefixed hex.");
        }

        return Convert.FromHexString(value[2..]);
    }

    private static string DecimalText(JsonElement item, string field, ulong minimum)
    {
        var value = SccpJson.Text(item, field);
        if (value.Any(static character => !char.IsAsciiDigit(character))
            || value.Length > 1 && value[0] == '0'
            || value == "0" && minimum > 0)
        {
            throw new ArgumentException($"{field} must be canonical unsigned decimal.");
        }

        return value;
    }

    private static bool TryParseUInt128(string value, out UInt128 result)
    {
        result = 0;
        foreach (var character in value)
        {
            var digit = (uint)(character - '0');
            if (result > (UInt128.MaxValue - digit) / 10)
            {
                result = 0;
                return false;
            }

            result = result * 10 + digit;
        }

        return value.Length != 0;
    }

    private static ulong DecimalUInt64(JsonElement item, string field, ulong minimum)
    {
        var value = DecimalText(item, field, minimum);
        if (!ulong.TryParse(value, System.Globalization.NumberStyles.None, System.Globalization.CultureInfo.InvariantCulture, out var result)
            || result < minimum || result.ToString(System.Globalization.CultureInfo.InvariantCulture) != value)
        {
            throw new ArgumentException($"{field} must fit UInt64.");
        }

        return result;
    }

    private static string? OptionalText(JsonElement item, string field)
    {
        if (!item.TryGetProperty(field, out var value) || value.ValueKind == JsonValueKind.Null)
        {
            return null;
        }

        return SccpJson.Text(item, field);
    }

    private static JsonElement Object(JsonElement item, string field)
    {
        var value = item.GetProperty(field);
        if (value.ValueKind != JsonValueKind.Object)
        {
            throw new ArgumentException($"{field} must be an object.");
        }

        return value;
    }

    private static JsonElement[] Array(JsonElement item, string field)
    {
        var value = item.GetProperty(field);
        if (value.ValueKind != JsonValueKind.Array)
        {
            throw new ArgumentException($"{field} must be an array.");
        }

        return value.EnumerateArray().ToArray();
    }

    private static void RequireDistinctBytes(IEnumerable<byte[]> values, string label)
    {
        var observed = new HashSet<string>(StringComparer.Ordinal);
        foreach (var value in values)
        {
            if (IsZero(value) || !observed.Add(Convert.ToHexString(value)))
            {
                throw new ArgumentException($"{label} must be nonzero and role-separated.");
            }
        }
    }

    private static bool IsZero(byte[] value) => value.All(static item => item == 0);

    private static byte[] Concat(params byte[][] values)
    {
        var length = values.Sum(static value => value.Length);
        var result = new byte[length];
        var offset = 0;
        foreach (var value in values)
        {
            value.CopyTo(result, offset);
            offset += value.Length;
        }

        return result;
    }
}
