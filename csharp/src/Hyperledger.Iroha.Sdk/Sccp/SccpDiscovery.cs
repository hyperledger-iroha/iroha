using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;

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
    uint MaxEd25519SignatureChecksPerTransaction,
    uint MaxEd25519SignatureChecksPerBlock,
    uint MaxEd25519ValidatorKeyChecksPerTransaction,
    uint MaxEd25519ValidatorKeyChecksPerBlock,
    uint MaxBn254PairingChecksPerTransaction,
    uint MaxBn254PairingChecksPerBlock,
    uint MaxBls12381PairingChecksPerTransaction,
    uint MaxBls12381PairingChecksPerBlock);

/// <summary>Stable first-release SCCP HTTP surface.</summary>
public sealed record SccpCapabilities(
    byte Version,
    string RegistryRevision,
    string RegistryPath,
    string MessageBundlePath,
    string ProofRequestPath,
    string RecentMessagesPath,
    string SoraOutboundMaterialPath,
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
    TonGroth16Bls12381,
}

/// <summary>Closed curve-specific semantic proof profiles admitted by SCCP V1.</summary>
public enum SccpSemanticProofProfileKindV1
{
    Groth16Bn254,
    Groth16Bls12381,
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
    byte[] ProfileHash)
{
    /// <summary>Curve and public-signal schema selected by this profile.</summary>
    public SccpSemanticProofProfileKindV1 Kind { get; init; } =
        SccpSemanticProofProfileKindV1.Groth16Bn254;
}

/// <summary>Canonical raw TON account identity retained by SCCP governance.</summary>
public sealed record SccpTonAddressV1
{
    private readonly byte[] account;

    public SccpTonAddressV1(int workchain, byte[] account)
    {
        ArgumentNullException.ThrowIfNull(account);
        if (account.Length != 32 || account.All(static value => value == 0))
        {
            throw new ArgumentException("TON account must be a nonzero 32-byte value.", nameof(account));
        }

        Workchain = workchain;
        this.account = [.. account];
    }

    public int Workchain { get; }

    public byte[] Account => [.. account];

    public bool IsSccpBasechainContract => Workchain == 0;

    internal byte[] RegistryBytes()
    {
        var result = new byte[36];
        BinaryPrimitives.WriteInt32LittleEndian(result, Workchain);
        account.CopyTo(result, 4);
        return result;
    }
}

/// <summary>Immutable TON basechain source-emitter identity.</summary>
public sealed class SccpTonSourceEmitterV1 : SccpSourceEmitterV1
{
    private readonly byte[] codeHash;
    private readonly byte[] routeConfigHash;

    public SccpTonSourceEmitterV1(
        SccpTonAddressV1 address,
        byte[] codeHash,
        byte[] routeConfigHash)
    {
        ArgumentNullException.ThrowIfNull(address);
        ArgumentNullException.ThrowIfNull(codeHash);
        ArgumentNullException.ThrowIfNull(routeConfigHash);
        if (!address.IsSccpBasechainContract
            || codeHash.Length != 32
            || routeConfigHash.Length != 32
            || codeHash.All(static value => value == 0)
            || routeConfigHash.All(static value => value == 0)
            || codeHash.AsSpan().SequenceEqual(routeConfigHash))
        {
            throw new ArgumentException("TON source-emitter roles are malformed or aliased.");
        }

        Address = address;
        this.codeHash = [.. codeHash];
        this.routeConfigHash = [.. routeConfigHash];
    }

    public SccpTonAddressV1 Address { get; }

    public byte[] CodeHash => [.. codeHash];

    public byte[] RouteConfigHash => [.. routeConfigHash];
}

/// <summary>Exact ordered five-key TON mint-breaker guardian set.</summary>
public sealed class SccpTonMintBreakerGuardianKeysV1
{
    private readonly byte[][] keys;

    public SccpTonMintBreakerGuardianKeysV1(
        byte[] guardian0,
        byte[] guardian1,
        byte[] guardian2,
        byte[] guardian3,
        byte[] guardian4)
    {
        keys = new[] { guardian0, guardian1, guardian2, guardian3, guardian4 }
            .Select(static key => key is null ? throw new ArgumentNullException(nameof(key)) : key.ToArray())
            .ToArray();
        if (keys.Any(static key => key.Length != 32 || key.All(static value => value == 0))
            || keys.Zip(keys.Skip(1)).Any(static pair => pair.First.AsSpan().SequenceCompareTo(pair.Second) >= 0))
        {
            throw new ArgumentException(
                "TON mint-breaker guardians must be five nonzero, strictly increasing 32-byte keys.");
        }
    }

    public byte[] Guardian0 => [.. keys[0]];

    public byte[] Guardian1 => [.. keys[1]];

    public byte[] Guardian2 => [.. keys[2]];

    public byte[] Guardian3 => [.. keys[3]];

    public byte[] Guardian4 => [.. keys[4]];

    /// <summary>Keys in canonical TON StateInit and SCCP hash-preimage order.</summary>
    public IReadOnlyList<byte[]> Ordered => keys.Select(static key => key.ToArray()).ToArray();
}

public sealed record SccpSoraFinalityAnchorV1(
    ushort ProtocolVersion,
    byte[] ChainIdHash,
    ulong CheckpointHeight,
    byte[] CheckpointBlockHash,
    byte[] CheckpointContextId,
    byte[] CheckpointFinalityArtifactHash,
    byte[] AnchorHash);

/// <summary>Exact destination proof policy bound into one governed deployment.</summary>
public sealed record SccpOutboundProofPolicyV1(
    byte Version,
    SccpSemanticProofProfileV1 SemanticProfile,
    SccpSoraFinalityAnchorV1 SoraFinalityAnchor);

/// <summary>Strict portable reference to one governance-registered IVM key.</summary>
public sealed record SccpPortableVerifyingKeyRefV1(
    string Backend,
    string Name,
    uint Version,
    byte[] Commitment);

/// <summary>Mandatory Taira-side execution policy for one outbound SCCP route.</summary>
public sealed record SccpSoraOutboundExecutionPolicyV1(
    byte Version,
    string Semantics,
    byte[] ContractArtifactSha256,
    SccpPortableVerifyingKeyRefV1 VerifyingKeyReference,
    ulong GasLimit);

public sealed record SccpDestinationDeploymentV1(
    SccpDestinationProofBackendV1 Family,
    byte[] TokenAddress,
    byte[] TokenCodeHash,
    byte[] VerifierAddress,
    byte[] VerifierCodeHash,
    byte[] VerifierKeyHash,
    SccpOutboundProofPolicyV1 OutboundProofPolicy,
    byte[] RouteAddress,
    byte[] RouteCodeHash,
    byte[] ReplayVerifierAddress,
    byte[] ReplayVerifierCodeHash,
    byte[] MintBreakerAddress,
    byte[] MintBreakerCodeHash,
    ulong TairaToTokenMultiplier,
    UInt128 MaxWrappedSupply,
    byte[] DestinationBindingHash)
{
    /// <summary>Audited semantic circuit selected by <see cref="OutboundProofPolicy"/>.</summary>
    public SccpSemanticProofProfileV1 SemanticProofProfile => OutboundProofPolicy.SemanticProfile;

    /// <summary>Taira finality anchor selected by <see cref="OutboundProofPolicy"/>.</summary>
    public SccpSoraFinalityAnchorV1 SoraFinalityAnchor => OutboundProofPolicy.SoraFinalityAnchor;

    /// <summary>TON Jetton master address; populated only for the TON family.</summary>
    public SccpTonAddressV1? TonJettonMasterAddress { get; init; }

    /// <summary>TON destination route address; populated only for the TON family.</summary>
    public SccpTonAddressV1? TonRouteAddress { get; init; }

    /// <summary>TON Jetton master StateInit data commitment; populated only for the TON family.</summary>
    public byte[]? TonJettonMasterInitialDataHash { get; init; }

    /// <summary>TON Jetton wallet code commitment; populated only for the TON family.</summary>
    public byte[]? TonJettonWalletCodeHash { get; init; }

    /// <summary>TON route StateInit data commitment; populated only for the TON family.</summary>
    public byte[]? TonRouteInitialDataHash { get; init; }

    /// <summary>TON governed BLS12-381 circuit commitment; populated only for the TON family.</summary>
    public byte[]? TonVerifierCircuitHash { get; init; }

    /// <summary>TON proof-format commitment; populated only for the TON family.</summary>
    public byte[]? TonProofProfileCommitment { get; init; }

    /// <summary>TON ordered mint-breaker guardians; populated only for the TON family.</summary>
    public SccpTonMintBreakerGuardianKeysV1? TonMintBreakerGuardianKeys { get; init; }

    /// <summary>Canonical curve-specific verifying-key bytes.</summary>
    public byte[]? VerifyingKeyBytes { get; init; }

    /// <summary>Validate one positive TON Jetton amount against this deployment's immutable cap.</summary>
    public UInt128 RequireTonAmountWithinCap(UInt128 amount)
    {
        var maximumTonCoins = (UInt128.One << 120) - 1;
        if (Family != SccpDestinationProofBackendV1.TonGroth16Bls12381)
        {
            throw new InvalidOperationException("TON amount validation requires a TON destination deployment.");
        }

        if (MaxWrappedSupply == 0 || MaxWrappedSupply > maximumTonCoins)
        {
            throw new InvalidOperationException("TON max_wrapped_supply must be in 1..2^120-1.");
        }

        if (amount == 0 || amount > MaxWrappedSupply)
        {
            throw new ArgumentOutOfRangeException(
                nameof(amount),
                "TON amount must be positive and no greater than max_wrapped_supply.");
        }

        return amount;
    }
}

public sealed record SccpGovernedRouteV1(
    SccpLaneIdV1 Lane,
    string RouteId,
    string AssetKey,
    uint Revision,
    SccpRouteActivationV1 Activation,
    SccpInboundFinalityCutoffV1? InboundFinalityCutoff,
    SccpSourceEmitterV1 SourceEmitter,
    SccpDestinationDeploymentV1 Destination,
    SccpSoraOutboundExecutionPolicyV1 SoraOutboundExecutionPolicy,
    string AssetDefinitionId,
    uint PayloadAmountScale,
    UInt128 MaxOutstandingLiability,
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
    /// <summary>TON BLS12-381 signal words in exact verifier order.</summary>
    public IReadOnlyList<byte[]>? TonPublicSignals { get; init; }

    /// <summary>TON governed verifier-circuit commitment.</summary>
    public string? TonVerifierCircuitHash { get; init; }

    /// <summary>TON proof-format commitment.</summary>
    public string? TonProofProfileCommitment { get; init; }

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
    private static readonly byte[] Bls12381BaseField = Convert.FromHexString(
        "1A0111EA397FE69A4B1BA7B6434BACD764774B84F38512BF6730D2A0F6B0F6241EABFFFEB153FFFFB9FEFFFFFFFFAAAB");
    private static readonly byte[] Bls12381ScalarField = Convert.FromHexString(
        "73EDA753299D7D483339D80809A1D80553BDA402FFFE5BFEFFFFFFFF00000001");
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
    private static readonly string[] Bls12381PublicSignalLabels =
    [
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
    ];
    private static readonly IReadOnlyDictionary<string, string> CapabilityPaths =
        new Dictionary<string, string>(StringComparer.Ordinal)
        {
            ["registry_path"] = "/v1/sccp/registry",
            ["message_bundle_path"] = "/v1/sccp/proofs/message/{message_id}",
            ["proof_request_path"] = "/v1/sccp/proof-requests/{message_id}",
            ["recent_messages_path"] = "/v1/sccp/messages/recent",
            ["sora_outbound_material_path"] = "/v1/sccp/routes/{source_profile}/{route_id}/{asset_key}/{revision}/sora-outbound-material",
            ["proof_submit_path"] = "/v1/bridge/proofs/submit",
            ["native_message_submit_path"] = "/v1/bridge/messages",
        };

    internal static SccpCapabilities ParseCapabilities(ReadOnlyMemory<byte> json)
    {
        using var document = SccpJson.Parse(json, "SCCP capabilities");
        var root = document.RootElement;
        HashSet<string> required =
        [
            "version",
            "registry_revision",
            "registry_path",
            "message_bundle_path",
            "proof_request_path",
            "recent_messages_path",
            "sora_outbound_material_path",
            "registry_limits",
            "resource_limits",
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
            FixedPath(root, "sora_outbound_material_path"),
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
                "max_governed_lanes",
                "max_live_governed_routes",
                "max_live_routes_per_lane",
                "max_retained_routes_per_lane",
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
                "max_proofs_per_transaction",
                "max_proofs_per_block",
                "max_proof_bytes_per_proof",
                "max_proof_bytes_per_transaction",
                "max_proof_bytes_per_block",
                "max_native_headers_per_transaction",
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
                "max_ed25519_signature_checks_per_transaction",
                "max_ed25519_signature_checks_per_block",
                "max_ed25519_validator_key_checks_per_transaction",
                "max_ed25519_validator_key_checks_per_block",
                "max_bn254_pairing_checks_per_transaction",
                "max_bn254_pairing_checks_per_block",
                "max_bls12_381_pairing_checks_per_transaction",
                "max_bls12_381_pairing_checks_per_block",
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
                item, "max_ed25519_signature_checks_per_transaction", 1, uint.MaxValue),
            SccpJson.UInt32(
                item, "max_ed25519_signature_checks_per_block", 1, uint.MaxValue),
            SccpJson.UInt32(
                item, "max_ed25519_validator_key_checks_per_transaction", 1, uint.MaxValue),
            SccpJson.UInt32(
                item, "max_ed25519_validator_key_checks_per_block", 1, uint.MaxValue),
            SccpJson.UInt32(
                item, "max_bn254_pairing_checks_per_transaction", 1, uint.MaxValue),
            SccpJson.UInt32(
                item, "max_bn254_pairing_checks_per_block", 1, uint.MaxValue),
            SccpJson.UInt32(
                item, "max_bls12_381_pairing_checks_per_transaction", 1, uint.MaxValue),
            SccpJson.UInt32(
                item, "max_bls12_381_pairing_checks_per_block", 1, uint.MaxValue));
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
            (
                limits.MaxEd25519SignatureChecksPerTransaction,
                limits.MaxEd25519SignatureChecksPerBlock
            ),
            (
                limits.MaxEd25519ValidatorKeyChecksPerTransaction,
                limits.MaxEd25519ValidatorKeyChecksPerBlock
            ),
            (limits.MaxBn254PairingChecksPerTransaction, limits.MaxBn254PairingChecksPerBlock),
            (
                limits.MaxBls12381PairingChecksPerTransaction,
                limits.MaxBls12381PairingChecksPerBlock
            ),
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
        var backend = ParseDestinationBackend(Object(root, "backend"));
        var exactFields = new HashSet<string>(
        [
            "version",
            "backend",
            "source_network",
            "target_network",
            "public_inputs",
            "verifying_key",
            "verifier_key_hash",
            "semantic_proof_profile",
            "semantic_proof_profile_hash",
            "sora_finality_anchor",
            "sora_finality_anchor_hash",
            "bundle_bytes",
            "statement_hash",
            "destination_binding_hash",
            "route_configuration_hash",
            "request_hash",
        ], StringComparer.Ordinal);
        if (backend == SccpDestinationProofBackendV1.TonGroth16Bls12381)
        {
            exactFields.UnionWith(["public_signals", "verifier_circuit_hash", "proof_profile_commitment"]);
        }
        SccpJson.ExactFields(root, exactFields, "SCCP proof request");
        RequireVersion(root, "SCCP proof request");
        var source = ParseNetwork(Object(root, "source_network"), "source_network");
        var target = ParseNetwork(Object(root, "target_network"), "target_network");
        var targetIsTron = target.ProfileKey().StartsWith("tron-", StringComparison.Ordinal);
        var targetIsTon = target.ProfileKey().StartsWith("ton-", StringComparison.Ordinal);
        if (source != SccpNetworkV1.SoraTaira || !target.IsExternal()
            || (backend == SccpDestinationProofBackendV1.TonGroth16Bls12381) != targetIsTon
            || (backend == SccpDestinationProofBackendV1.TronGroth16Bn254) != targetIsTron)
        {
            throw new ArgumentException("SCCP proof backend does not match an exact Taira-to-external lane.");
        }

        var inputs = Object(root, "public_inputs");
        SccpJson.ExactFields(inputs,
            ["version", "message_id", "payload_hash", "target_domain", "commitment_root", "finality_height", "finality_block_hash"],
            "SCCP proof public inputs");
        RequireVersion(inputs, "SCCP proof public inputs");
        var targetDomain = SccpJson.UInt32(inputs, "target_domain", 1, 4);
        if (targetDomain != target.DomainId())
        {
            throw new ArgumentException("SCCP target profile/domain mismatch.");
        }

        var messageId = PrefixedHash(inputs, "message_id");
        var payloadHash = PrefixedHash(inputs, "payload_hash");
        var commitmentRoot = PrefixedHash(inputs, "commitment_root");
        var finalityHash = PrefixedHash(inputs, "finality_block_hash");
        var finalityHeight = DecimalUInt64(inputs, "finality_height", 1);
        var keyBytes = backend == SccpDestinationProofBackendV1.TonGroth16Bls12381
            ? ParseBls12381VerifyingKey(Object(root, "verifying_key"), "SCCP proof verifying key")
            : ParseVerifyingKey(Object(root, "verifying_key"), "SCCP proof verifying key");
        var keyHash = PrefixedHash(root, "verifier_key_hash");
        var computedKeyHash = backend == SccpDestinationProofBackendV1.TonGroth16Bls12381
            ? SHA256.HashData(keyBytes)
            : SccpV1.Keccak256(keyBytes);
        if ("0x" + SccpV1.LowerHex(computedKeyHash) != keyHash)
        {
            throw new ArgumentException("verifier_key_hash does not match the exact curve-specific verifying key.");
        }

        var semantic = ParseSemanticProfile(Object(root, "semantic_proof_profile"), "semantic_proof_profile");
        var semanticHash = PrefixedHash(root, "semantic_proof_profile_hash");
        if ("0x" + SccpV1.LowerHex(semantic.ProfileHash) != semanticHash)
        {
            throw new ArgumentException("semantic_proof_profile_hash does not match its typed profile.");
        }
        var expectedSemanticKind = backend == SccpDestinationProofBackendV1.TonGroth16Bls12381
            ? SccpSemanticProofProfileKindV1.Groth16Bls12381
            : SccpSemanticProofProfileKindV1.Groth16Bn254;
        if (semantic.Kind != expectedSemanticKind)
        {
            throw new ArgumentException("SCCP proof backend and semantic profile curve do not match.");
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
        string? verifierCircuitHash = null;
        string? proofProfileCommitment = null;
        IReadOnlyList<byte[]>? publicSignals = null;
        var roleHashes = new List<string>
        {
            messageId, payloadHash, commitmentRoot, finalityHash, keyHash, semanticHash,
            anchorHash, statement, binding, configuration, request,
        };
        if (backend == SccpDestinationProofBackendV1.TonGroth16Bls12381)
        {
            verifierCircuitHash = PrefixedHash(root, "verifier_circuit_hash");
            proofProfileCommitment = PrefixedHash(root, "proof_profile_commitment");
            if (verifierCircuitHash != "0x" + SccpV1.LowerHex(semantic.CircuitCommitment)
                || proofProfileCommitment != "0x" + SccpV1.LowerHex(TonProofProfileCommitment()))
            {
                throw new ArgumentException("SCCP TON proof request does not bind its exact circuit and proof profile.");
            }
            roleHashes.Add(verifierCircuitHash);
            roleHashes.Add(proofProfileCommitment);
        }
        DistinctHexRoles(roleHashes, "SCCP proof-request hash roles");
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

        (byte[] StatementHash, byte[] RequestHash) hashes;
        if (backend == SccpDestinationProofBackendV1.TonGroth16Bls12381)
        {
            var tonHashes = TonProofRequestHashes(
                source,
                target,
                DecodePrefixedHash(messageId),
                DecodePrefixedHash(payloadHash),
                targetDomain,
                DecodePrefixedHash(commitmentRoot),
                finalityHeight,
                DecodePrefixedHash(finalityHash),
                bundle.Payload.CanonicalBytes(),
                bundleBytes,
                keyBytes,
                DecodePrefixedHash(keyHash),
                semantic,
                DecodePrefixedHash(semanticHash),
                anchor,
                DecodePrefixedHash(anchorHash),
                DecodePrefixedHash(binding),
                DecodePrefixedHash(configuration),
                DecodePrefixedHash(verifierCircuitHash!),
                DecodePrefixedHash(proofProfileCommitment!));
            hashes = (tonHashes.StatementHash, tonHashes.RequestHash);
            publicSignals = ParseTonPublicSignals(
                Object(root, "public_signals"),
                tonHashes.PublicSignals,
                "SCCP TON public signals");
        }
        else
        {
            hashes = SccpV1.CanonicalProofRequestHashes(
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
        }
        if ("0x" + SccpV1.LowerHex(hashes.StatementHash) != statement
            || "0x" + SccpV1.LowerHex(hashes.RequestHash) != request)
        {
            throw new ArgumentException("SCCP proof-request statement or request hash is not canonical.");
        }

        return new SccpGroth16ProofRequestV1(
            1, backend, source, target, messageId, payloadHash, targetDomain, commitmentRoot,
            finalityHeight, finalityHash, keyHash, semantic, anchor, statement, binding,
            configuration, request, bundleBytes, json.ToArray())
        {
            TonPublicSignals = publicSignals,
            TonVerifierCircuitHash = verifierCircuitHash,
            TonProofProfileCommitment = proofProfileCommitment,
        };
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
                "height",
                "commitment_index",
                "message_id_hex",
                "kind",
                "source_profile",
                "target_profile",
                "destination_binding_hash",
                "route_configuration_hash",
                "target_domain",
                "amount",
                "payload_projection",
                "links",
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
            var targetDomain = SccpJson.UInt32(item, "target_domain", 1, 4);
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
                4 => "taira_ton_xor",
                3 => "taira_tron_xor",
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
                "lane_id",
                "route_id",
                "asset_key",
                "revision",
                "activation",
                "inbound_finality_cutoff",
                "source_identity",
                "destination",
                "sora_outbound_execution_policy",
                "settlement",
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
        var executionPolicy = ParseSoraOutboundExecutionPolicy(
            Object(item, "sora_outbound_execution_policy"),
            $"{label}.sora_outbound_execution_policy");
        var sourceParts = SourceParts(source);
        if (sourceParts.Family != destination.Family)
        {
            throw new ArgumentException($"{label} source identity does not name its destination route deployment.");
        }
        if (source is SccpTonSourceEmitterV1 tonSource)
        {
            if (!tonSource.Address.RegistryBytes().AsSpan().SequenceEqual(destination.RouteAddress)
                || !tonSource.CodeHash.AsSpan().SequenceEqual(destination.RouteCodeHash))
            {
                throw new ArgumentException($"{label} source identity does not name its destination route deployment.");
            }
        }
        else if (!sourceParts.Address.AsSpan().SequenceEqual(destination.RouteAddress)
                 || !sourceParts.Runtime.AsSpan().SequenceEqual(destination.RouteCodeHash))
        {
            throw new ArgumentException($"{label} source identity does not name its destination route deployment.");
        }

        var settlement = Object(item, "settlement");
        SccpJson.ExactFields(settlement,
            ["asset_definition_id", "payload_amount_scale", "max_outstanding_liability"],
            $"{label}.settlement");
        var assetDefinition = SccpJson.Text(settlement, "asset_definition_id");
        if (assetDefinition != "6TEAJqbb8oEPmLncoNiMRbLEK6tw")
        {
            throw new ArgumentException($"{label} must settle canonical Taira XOR.");
        }

        var scale = SccpJson.UInt32(settlement, "payload_amount_scale", 9, 9);
        var maxOutstandingLiability = DecimalUInt128(
            settlement,
            "max_outstanding_liability",
            1);
        var multiplier = (UInt128)destination.TairaToTokenMultiplier;
        if (maxOutstandingLiability > UInt128.MaxValue / multiplier
            || maxOutstandingLiability * multiplier != destination.MaxWrappedSupply)
        {
            throw new ArgumentException(
                $"{label} destination wrapped-supply cap does not match its liability cap.");
        }
        var configuration = RouteConfigurationHash(lane, routeId, assetKey, revision, destination);
        var executionPolicyRoles = new List<byte[]>
        {
            executionPolicy.ContractArtifactSha256,
            executionPolicy.VerifyingKeyReference.Commitment,
            configuration,
            destination.DestinationBindingHash,
            destination.VerifierKeyHash,
            destination.SemanticProofProfile.ProfileHash,
            destination.SoraFinalityAnchor.AnchorHash,
        };
        if (destination.Family == SccpDestinationProofBackendV1.TonGroth16Bls12381)
        {
            executionPolicyRoles.Add(destination.TonJettonMasterInitialDataHash
                ?? throw new ArgumentException("TON Jetton master initial-data hash is missing."));
            executionPolicyRoles.Add(destination.TonRouteInitialDataHash
                ?? throw new ArgumentException("TON route initial-data hash is missing."));
        }
        RequireDistinctBytes(executionPolicyRoles, $"{label}.sora_outbound_execution_policy hash roles");
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
            executionPolicy, assetDefinition, scale, maxOutstandingLiability, configuration);
    }

    private static SccpDestinationDeploymentV1 ParseDestination(JsonElement item, SccpLaneIdV1 lane, string label)
    {
        SccpJson.ExactFields(item, ["family", "deployment"], label);
        var family = SccpJson.Text(item, "family") switch
        {
            "evm" => SccpDestinationProofBackendV1.EvmGroth16Bn254,
            "tron" => SccpDestinationProofBackendV1.TronGroth16Bn254,
            "ton" => SccpDestinationProofBackendV1.TonGroth16Bls12381,
            _ => throw new ArgumentException($"{label} family is unsupported or retired."),
        };
        var laneIsTron = lane.Source.ProfileKey().StartsWith("tron-", StringComparison.Ordinal);
        var laneIsTon = lane.Source.ProfileKey().StartsWith("ton-", StringComparison.Ordinal);
        if (family == SccpDestinationProofBackendV1.TonGroth16Bls12381)
        {
            if (!laneIsTon)
            {
                throw new ArgumentException($"{label} family does not match its lane.");
            }

            return ParseTonDestination(Object(item, "deployment"), lane, $"{label}.deployment");
        }

        if (laneIsTon || (family == SccpDestinationProofBackendV1.TronGroth16Bn254) != laneIsTron)
        {
            throw new ArgumentException($"{label} family does not match its lane.");
        }

        var deployment = Object(item, "deployment");
        SccpJson.ExactFields(deployment,
        [
            "token_address",
            "token_code_hash",
            "verifier_address",
            "verifier_code_hash",
            "verifying_key",
            "verifier_key_hash",
            "outbound_proof_policy",
            "route_address",
            "route_code_hash",
            "replay_verifier_address",
            "replay_verifier_code_hash",
            "mint_breaker_address",
            "mint_breaker_code_hash",
            "taira_to_token_multiplier",
            "max_wrapped_supply",
        ], $"{label}.deployment");
        var addresses = new[]
        {
            UpperHex(deployment, "token_address", 20),
            UpperHex(deployment, "verifier_address", 20),
            UpperHex(deployment, "route_address", 20),
            UpperHex(deployment, "replay_verifier_address", 20),
            UpperHex(deployment, "mint_breaker_address", 20),
        };
        var hashes = new[]
        {
            UpperHex(deployment, "token_code_hash", 32),
            UpperHex(deployment, "verifier_code_hash", 32),
            UpperHex(deployment, "verifier_key_hash", 32),
            UpperHex(deployment, "route_code_hash", 32),
            UpperHex(deployment, "replay_verifier_code_hash", 32),
            UpperHex(deployment, "mint_breaker_code_hash", 32),
        };
        RequireDistinctBytes(addresses, $"{label}.deployment addresses");
        RequireDistinctBytes(hashes, $"{label}.deployment hashes");
        var emptyRuntimeHash = SccpV1.Keccak256([]);
        foreach (var index in new[] { 0, 1, 3, 4, 5 })
        {
            if (hashes[index].AsSpan().SequenceEqual(emptyRuntimeHash))
            {
                throw new ArgumentException(
                    $"{label}.deployment runtime code hash must not identify empty bytecode.");
            }
        }
        var key = ParseVerifyingKey(Object(deployment, "verifying_key"), $"{label}.deployment.verifying_key");
        if (!SccpV1.Keccak256(key).AsSpan().SequenceEqual(hashes[2]))
        {
            throw new ArgumentException($"{label}.deployment.verifier_key_hash does not match verifying_key.");
        }

        var proofPolicy = ParseOutboundPolicy(
            Object(deployment, "outbound_proof_policy"),
            $"{label}.deployment.outbound_proof_policy");
        var semantic = proofPolicy.SemanticProfile;
        var anchor = proofPolicy.SoraFinalityAnchor;
        if (semantic.Kind != SccpSemanticProofProfileKindV1.Groth16Bn254)
        {
            throw new ArgumentException($"{label}.deployment requires the BN254 semantic profile.");
        }
        RequireDistinctBytes(
            hashes.Concat([semantic.ProfileHash, anchor.AnchorHash]),
            $"{label}.deployment proof-policy and deployment hashes");
        if (SccpJson.UInt64(deployment, "taira_to_token_multiplier", 1_000_000_000) != 1_000_000_000)
        {
            throw new ArgumentException($"{label}.deployment has the wrong Taira/token multiplier.");
        }
        var maxWrappedSupply = DecimalUInt128(deployment, "max_wrapped_supply", 1);

        var partial = new SccpDestinationDeploymentV1(
            family, addresses[0], hashes[0], addresses[1], hashes[1], hashes[2], proofPolicy,
            addresses[2], hashes[3], addresses[3], hashes[4], addresses[4], hashes[5],
            1_000_000_000, maxWrappedSupply, [])
        {
            VerifyingKeyBytes = key,
        };
        return partial with { DestinationBindingHash = DestinationBindingHash(lane, partial) };
    }

    private static SccpDestinationDeploymentV1 ParseTonDestination(
        JsonElement deployment,
        SccpLaneIdV1 lane,
        string label)
    {
        SccpJson.ExactFields(deployment,
        [
            "jetton_master_address",
            "jetton_master_code_hash",
            "jetton_master_initial_data_hash",
            "jetton_wallet_code_hash",
            "route_address",
            "route_code_hash",
            "route_initial_data_hash",
            "embedded_verifier_code_hash",
            "verifier_circuit_hash",
            "verifying_key",
            "verifier_key_hash",
            "proof_profile_commitment",
            "mint_breaker_guardian_keys",
            "outbound_proof_policy",
            "taira_to_token_multiplier",
            "max_wrapped_supply",
        ], label);
        var master = ParseTonAddress(Object(deployment, "jetton_master_address"), $"{label}.jetton_master_address");
        var route = ParseTonAddress(Object(deployment, "route_address"), $"{label}.route_address");
        if (!master.IsSccpBasechainContract
            || !route.IsSccpBasechainContract
            || (master.Workchain == route.Workchain
                && master.Account.AsSpan().SequenceEqual(route.Account)))
        {
            throw new ArgumentException($"{label} TON addresses must be distinct nonzero basechain contracts.");
        }

        var masterCode = UpperHex(deployment, "jetton_master_code_hash", 32);
        var masterInitialData = UpperHex(deployment, "jetton_master_initial_data_hash", 32);
        var walletCode = UpperHex(deployment, "jetton_wallet_code_hash", 32);
        var routeCode = UpperHex(deployment, "route_code_hash", 32);
        var routeInitialData = UpperHex(deployment, "route_initial_data_hash", 32);
        var verifierCode = UpperHex(deployment, "embedded_verifier_code_hash", 32);
        var circuit = UpperHex(deployment, "verifier_circuit_hash", 32);
        var keyHash = UpperHex(deployment, "verifier_key_hash", 32);
        var proofProfile = UpperHex(deployment, "proof_profile_commitment", 32);
        var guardians = ParseTonGuardianKeys(
            Object(deployment, "mint_breaker_guardian_keys"),
            $"{label}.mint_breaker_guardian_keys");
        var key = ParseBls12381VerifyingKey(
            Object(deployment, "verifying_key"),
            $"{label}.verifying_key");
        if (!SHA256.HashData(key).AsSpan().SequenceEqual(keyHash))
        {
            throw new ArgumentException($"{label}.verifier_key_hash does not match verifying_key.");
        }

        var proofPolicy = ParseOutboundPolicy(
            Object(deployment, "outbound_proof_policy"),
            $"{label}.outbound_proof_policy");
        var semantic = proofPolicy.SemanticProfile;
        var anchor = proofPolicy.SoraFinalityAnchor;
        if (semantic.Kind != SccpSemanticProofProfileKindV1.Groth16Bls12381
            || !semantic.CircuitCommitment.AsSpan().SequenceEqual(circuit)
            || !proofProfile.AsSpan().SequenceEqual(TonProofProfileCommitment()))
        {
            throw new ArgumentException($"{label} does not bind the exact TON proof profile and circuit.");
        }

        RequireDistinctBytes(
        [
            masterCode,
            masterInitialData,
            walletCode,
            routeCode,
            routeInitialData,
            verifierCode,
            circuit,
            keyHash,
            proofProfile,
            semantic.ProfileHash,
            anchor.AnchorHash,
        ], $"{label} hashes");
        if (SccpJson.UInt64(deployment, "taira_to_token_multiplier", 1, 1) != 1)
        {
            throw new ArgumentException($"{label} has the wrong Taira/Jetton multiplier.");
        }
        var maxWrappedSupply = DecimalUInt128(deployment, "max_wrapped_supply", 1);
        var maximumTonCoins = (UInt128.One << 120) - 1;
        if (maxWrappedSupply > maximumTonCoins)
        {
            throw new ArgumentException($"{label}.max_wrapped_supply exceeds the TON 120-bit coin range.");
        }

        var partial = new SccpDestinationDeploymentV1(
            SccpDestinationProofBackendV1.TonGroth16Bls12381,
            master.RegistryBytes(),
            masterCode,
            [],
            verifierCode,
            keyHash,
            proofPolicy,
            route.RegistryBytes(),
            routeCode,
            [],
            [],
            [],
            [],
            1,
            maxWrappedSupply,
            [])
        {
            TonJettonMasterAddress = master,
            TonRouteAddress = route,
            TonJettonMasterInitialDataHash = masterInitialData,
            TonJettonWalletCodeHash = walletCode,
            TonRouteInitialDataHash = routeInitialData,
            TonVerifierCircuitHash = circuit,
            TonProofProfileCommitment = proofProfile,
            TonMintBreakerGuardianKeys = guardians,
            VerifyingKeyBytes = key,
        };
        return partial with { DestinationBindingHash = DestinationBindingHash(lane, partial) };
    }

    private static SccpSoraOutboundExecutionPolicyV1 ParseSoraOutboundExecutionPolicy(
        JsonElement item,
        string label)
    {
        SccpJson.ExactFields(
            item,
            ["version", "semantics", "contract_artifact_sha256", "vk_ref", "gas_limit"],
            label);
        if (SccpJson.UInt32(item, "version", 1, 1) != 1)
        {
            throw new ArgumentException($"{label}.version must equal 1.");
        }
        var semantics = SccpJson.Text(item, "semantics");
        if (semantics != "ivm_proved_record_sccp_message_v1")
        {
            throw new ArgumentException($"{label}.semantics is unsupported or retired.");
        }
        var artifact = UpperHex(item, "contract_artifact_sha256", 32);
        var referenceItem = Object(item, "vk_ref");
        SccpJson.ExactFields(
            referenceItem,
            ["backend", "name", "version", "commitment"],
            $"{label}.vk_ref");
        var reference = new SccpPortableVerifyingKeyRefV1(
            PortableVerifyingKeyIdField(
                SccpJson.Text(referenceItem, "backend"),
                $"{label}.vk_ref.backend"),
            PortableVerifyingKeyIdField(
                SccpJson.Text(referenceItem, "name"),
                $"{label}.vk_ref.name"),
            SccpJson.UInt32(referenceItem, "version", 1, uint.MaxValue),
            UpperHex(referenceItem, "commitment", 32));
        var gasLimit = SccpJson.UInt64(item, "gas_limit", 1, 1_000_000_000);
        RequireDistinctBytes(
            [artifact, reference.Commitment],
            $"{label} artifact and verification-key commitments");
        return new SccpSoraOutboundExecutionPolicyV1(
            1,
            semantics,
            artifact,
            reference,
            gasLimit);
    }

    private static string PortableVerifyingKeyIdField(string value, string label)
    {
        static bool IsEdge(char value) => value is >= 'a' and <= 'z' or >= '0' and <= '9';
        static bool IsPortable(char value) => IsEdge(value) || value is '-' or '_' or '/' or ':' or '.';

        if (Encoding.UTF8.GetByteCount(value) > 256
            || !IsEdge(value[0])
            || !IsEdge(value[^1])
            || value.Any(static character => !IsPortable(character))
            || new[] { "..", "//", ":::", "/:", ":/", "/.", "./", ":.", ".:" }
                .Any(value.Contains))
        {
            throw new ArgumentException(
                $"{label} must use portable verification-key registry syntax.");
        }
        return value;
    }

    private static SccpOutboundProofPolicyV1 ParseOutboundPolicy(
        JsonElement item,
        string label)
    {
        SccpJson.ExactFields(item, ["version", "semantic_profile", "sora_finality_anchor"], label);
        RequireVersion(item, label);
        var semantic = ParseSemanticProfile(Object(item, "semantic_profile"), $"{label}.semantic_profile");
        var anchor = ParseFinalityAnchor(Object(item, "sora_finality_anchor"), $"{label}.sora_finality_anchor");
        RequireDistinctBytes(
        [
            semantic.CircuitCommitment,
            semantic.WitnessGeneratorCommitment,
            semantic.PublicSignalSchemaHash,
            semantic.ProfileHash,
            anchor.ChainIdHash,
            anchor.CheckpointBlockHash,
            anchor.CheckpointContextId,
            anchor.CheckpointFinalityArtifactHash,
            anchor.AnchorHash,
        ], $"{label} hashes");
        return new SccpOutboundProofPolicyV1(1, semantic, anchor);
    }

    private static SccpSemanticProofProfileV1 ParseSemanticProfile(JsonElement item, string label)
    {
        SccpJson.ExactFields(item, ["profile", "commitments"], label);
        var kind = SccpJson.Text(item, "profile") switch
        {
            "sora_taira_finality_inclusion_groth16_bn254" =>
                SccpSemanticProofProfileKindV1.Groth16Bn254,
            "sora_taira_finality_inclusion_groth16_bls12381" =>
                SccpSemanticProofProfileKindV1.Groth16Bls12381,
            _ => throw new ArgumentException($"{label} is unsupported or retired."),
        };

        var commitments = Object(item, "commitments");
        SccpJson.ExactFields(commitments,
            ["version", "circuit_commitment", "witness_generator_commitment", "public_signal_schema_hash"],
            $"{label}.commitments");
        RequireVersion(commitments, $"{label}.commitments");
        var circuit = UpperHex(commitments, "circuit_commitment", 32);
        var witness = UpperHex(commitments, "witness_generator_commitment", 32);
        var schema = UpperHex(commitments, "public_signal_schema_hash", 32);
        var expectedSchema = kind == SccpSemanticProofProfileKindV1.Groth16Bls12381
            ? Bls12381PublicSignalSchemaHash()
            : PublicSignalSchemaHash();
        if (!schema.AsSpan().SequenceEqual(expectedSchema))
        {
            throw new ArgumentException($"{label} does not commit the exact eleven-signal schema.");
        }

        RequireDistinctBytes([circuit, witness, schema], $"{label} commitments");
        var profileTag = kind == SccpSemanticProofProfileKindV1.Groth16Bls12381
            ? (byte)1
            : (byte)0;
        var canonical = Concat([1, profileTag, 1], circuit, witness, schema);
        var hash = SccpV1.Keccak256(Concat("sccp:semantic-proof-profile:v1"u8.ToArray(), canonical));
        return new SccpSemanticProofProfileV1(circuit, witness, schema, hash) { Kind = kind };
    }

    private static SccpSoraFinalityAnchorV1 ParseFinalityAnchor(JsonElement item, string label)
    {
        SccpJson.ExactFields(item,
        [
            "version",
            "source_network",
            "protocol_version",
            "chain_id_hash",
            "checkpoint_height",
            "checkpoint_block_hash",
            "checkpoint_context_id",
            "checkpoint_finality_artifact_hash",
        ], label);
        RequireVersion(item, label);
        if (ParseNetwork(Object(item, "source_network"), $"{label}.source_network") != SccpNetworkV1.SoraTaira)
        {
            throw new ArgumentException($"{label} source network must be Taira.");
        }

        var protocolVersion = checked((ushort)SccpJson.UInt32(item, "protocol_version", 4, 4));
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

    private static SccpTonAddressV1 ParseTonAddress(JsonElement item, string label)
    {
        SccpJson.ExactFields(item, ["workchain", "account"], label);
        var address = new SccpTonAddressV1(
            SignedInt32(item, "workchain"),
            UpperHex(item, "account", 32));
        if (!address.IsSccpBasechainContract)
        {
            throw new ArgumentException($"{label} must be a TON basechain contract.");
        }

        return address;
    }

    private static SccpTonMintBreakerGuardianKeysV1 ParseTonGuardianKeys(
        JsonElement item,
        string label)
    {
        SccpJson.ExactFields(
            item,
            ["guardian_0", "guardian_1", "guardian_2", "guardian_3", "guardian_4"],
            label);
        return new SccpTonMintBreakerGuardianKeysV1(
            UpperHex(item, "guardian_0", 32),
            UpperHex(item, "guardian_1", 32),
            UpperHex(item, "guardian_2", 32),
            UpperHex(item, "guardian_3", 32),
            UpperHex(item, "guardian_4", 32));
    }

    private static byte[] ParseBls12381VerifyingKey(JsonElement item, string label)
    {
        SccpJson.ExactFields(item, ["version", "alpha1", "beta2", "gamma2", "delta2", "ic"], label);
        RequireVersion(item, label);
        var key = new List<byte>(1 + 48 + 3 * 96 + 12 * 48) { 1 };
        key.AddRange(Bls12381G1(item, "alpha1", $"{label}.alpha1"));
        key.AddRange(Bls12381G2(item, "beta2", $"{label}.beta2"));
        key.AddRange(Bls12381G2(item, "gamma2", $"{label}.gamma2"));
        key.AddRange(Bls12381G2(item, "delta2", $"{label}.delta2"));
        var ic = Object(item, "ic");
        var fields = new[] { "constant" }
            .Concat(Enumerable.Range(0, 11).Select(static index => $"signal_{index}"))
            .ToArray();
        SccpJson.ExactFields(ic, new HashSet<string>(fields, StringComparer.Ordinal), $"{label}.ic");
        foreach (var field in fields)
        {
            key.AddRange(Bls12381G1(ic, field, $"{label}.ic.{field}"));
        }

        if (key.Count != 913)
        {
            throw new ArgumentException($"{label} must contain the exact fixed BLS12-381 key shape.");
        }

        return key.ToArray();
    }

    private static byte[] Bls12381G1(JsonElement item, string field, string label)
    {
        var point = UpperHex(item, field, 48, true);
        var x = point.ToArray();
        if ((x[0] & 0x80) == 0 || (x[0] & 0x40) != 0)
        {
            throw new ArgumentException($"{label} is not a compressed non-infinity BLS12-381 G1 point.");
        }
        x[0] &= 0x1f;
        if (x.AsSpan().SequenceCompareTo(Bls12381BaseField) >= 0)
        {
            throw new ArgumentException($"{label} is not a canonical BLS12-381 G1 point.");
        }

        return point;
    }

    private static byte[] Bls12381G2(JsonElement item, string field, string label)
    {
        var point = UpperHex(item, field, 96, true);
        var first = point[..48].ToArray();
        if ((first[0] & 0x80) == 0 || (first[0] & 0x40) != 0)
        {
            throw new ArgumentException($"{label} is not a compressed non-infinity BLS12-381 G2 point.");
        }
        first[0] &= 0x1f;
        if (first.AsSpan().SequenceCompareTo(Bls12381BaseField) >= 0
            || point.AsSpan(48, 48).SequenceCompareTo(Bls12381BaseField) >= 0)
        {
            throw new ArgumentException($"{label} is not a canonical BLS12-381 G2 point.");
        }

        return point;
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
        var laneIsTon = expectedLane.Source.ProfileKey().StartsWith("ton-", StringComparison.Ordinal);
        if (family == "ton")
        {
            if (!laneIsTon)
            {
                throw new ArgumentException($"{label} emitter family does not match its lane.");
            }
            SccpJson.ExactFields(
                identity,
                ["address", "code_hash", "route_config_hash"],
                $"{label}.emitter.identity");
            return new SccpTonSourceEmitterV1(
                ParseTonAddress(Object(identity, "address"), $"{label}.emitter.identity.address"),
                UpperHex(identity, "code_hash", 32),
                UpperHex(identity, "route_config_hash", 32));
        }

        if (laneIsTon)
        {
            throw new ArgumentException($"{label} emitter family does not match its lane.");
        }
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
            SccpTonSourceEmitterV1 ton =>
                (SccpDestinationProofBackendV1.TonGroth16Bls12381, ton.Address.RegistryBytes(), ton.CodeHash, ton.RouteConfigHash),
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
            "ton_groth16_bls12381_v1" => SccpDestinationProofBackendV1.TonGroth16Bls12381,
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
            "version",
            "source_domain",
            "dest_domain",
            "nonce",
            "route_revision",
            "asset_home_domain",
            "asset_id_codec",
            "asset_id",
            "amount",
            "sender_codec",
            "sender",
            "recipient_codec",
            "recipient",
            "route_id_codec",
            "route_id",
        ], "SCCP transfer payload");
        RequireVersion(item, "SCCP transfer payload");
        if (SccpJson.UInt32(item, "source_domain", 0, 4) != lane.Source.DomainId()
            || SccpJson.UInt32(item, "dest_domain", 0, 4) != lane.Target.DomainId())
        {
            throw new ArgumentException("SCCP transfer payload does not match its exact lane.");
        }

        var nonce = DecimalUInt64(item, "nonce", 0);
        var routeRevision = SccpJson.UInt32(item, "route_revision", 1, uint.MaxValue);
        var assetHomeDomain = SccpJson.UInt32(item, "asset_home_domain", 0, 4);
        if (assetHomeDomain is not (0 or 1 or 2 or 3 or 4))
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

        var senderCodec = SccpJson.UInt32(item, "sender_codec", 0, 3);
        var recipientCodec = SccpJson.UInt32(item, "recipient_codec", 0, 3);
        var expectedRecipientCodec = lane.Target.DomainId() switch
        {
            4 => (uint)SccpCodecV1.TonAccount36,
            3 => (uint)SccpCodecV1.TronAddress21,
            _ => (uint)SccpCodecV1.EvmAddress20,
        };
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
            var tag = SccpJson.UInt32(item, codecField, 0, 3);
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
                "version",
                "source_domain",
                "dest_domain",
                "nonce",
                "route_revision",
                "asset_home_domain",
                "asset_id",
                "amount",
                "sender",
                "recipient",
                "route_id",
            ],
            $"{label}.Transfer");
        if (SccpJson.UInt64(transfer, "version", 1) != 1
            || SccpJson.UInt32(transfer, "source_domain", 0, 0) != 0
            || SccpJson.UInt32(transfer, "dest_domain", 1, 4) != expectedDestinationDomain
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
            4 => "taira_ton_xor",
            3 => "taira_tron_xor",
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
        var tag = destinationDomain switch
        {
            4 => "TonAccount36",
            3 => "TronAddress21",
            _ => "EvmAddress20",
        };
        SccpJson.ExactFields(tagged, [tag], $"{label}.Transfer.{field}");
        var content = Object(tagged, tag);
        if (destinationDomain == 4)
        {
            SccpJson.ExactFields(content, ["workchain", "account"], $"{label}.Transfer.{field}.{tag}");
            if (SignedInt32(content, "workchain") != 0)
            {
                throw new ArgumentException($"{label}.Transfer.{field} must be a TON basechain account.");
            }
            var account = SccpJson.Text(content, "account");
            if (account.Length != 66
                || !account.StartsWith("0x", StringComparison.Ordinal)
                || account.AsSpan(2).ContainsAnyExcept("0123456789abcdef")
                || account.AsSpan(2).IndexOfAnyExcept('0') < 0)
            {
                throw new ArgumentException($"{label}.Transfer.{field} does not match TON account36.");
            }
            return;
        }

        SccpJson.ExactFields(content, ["bytes"], $"{label}.Transfer.{field}.{tag}");
        var encoded = SccpJson.Text(content, "bytes");
        var expectedLength = destinationDomain == 3 ? 44 : 42;
        var bodyOffset = destinationDomain == 3 ? 4 : 2;
        if (encoded.Length != expectedLength
            || !encoded.StartsWith(destinationDomain == 3 ? "0x41" : "0x", StringComparison.Ordinal)
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
        if (destination.Family == SccpDestinationProofBackendV1.TonGroth16Bls12381)
        {
            return TonDestinationBindingHash(lane, destination);
        }

        var tron = destination.Family == SccpDestinationProofBackendV1.TronGroth16Bn254;
        ulong network = lane.Source switch
        {
            SccpNetworkV1.EthereumMainnet => 1,
            SccpNetworkV1.BscMainnet => 56,
            SccpNetworkV1.TronMainnet => 0x2b66_53dc,
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
            destination.SoraFinalityAnchor.AnchorHash,
            tron ? AbiTronAddress(destination.ReplayVerifierAddress) : AbiAddress(destination.ReplayVerifierAddress),
            destination.ReplayVerifierCodeHash,
            tron ? AbiTronAddress(destination.MintBreakerAddress) : AbiAddress(destination.MintBreakerAddress),
            destination.MintBreakerCodeHash));
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

        if (destination.Family == SccpDestinationProofBackendV1.TonGroth16Bls12381)
        {
            if (routeId != "taira_ton_xor")
            {
                throw new ArgumentException("SCCP TON route id does not match its exact deployment.");
            }
            return TonRouteConfigurationHash(lane, revision, destination);
        }

        var (expectedRoute, network) = lane.Source switch
        {
            SccpNetworkV1.EthereumMainnet => ("taira_eth_xor", 1UL),
            SccpNetworkV1.BscMainnet => ("taira_bsc_xor", 56UL),
            SccpNetworkV1.TronMainnet => ("taira_tron_xor", 0x2b66_53dcUL),
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
            destination.RouteCodeHash, destination.ReplayVerifierCodeHash,
            destination.MintBreakerCodeHash, destination.VerifierKeyHash,
            destination.SemanticProofProfile.ProfileHash,
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
        deploymentParts.AddRange([
            AbiAddress(destination.ReplayVerifierAddress), destination.ReplayVerifierCodeHash,
            AbiAddress(destination.MintBreakerAddress), destination.MintBreakerCodeHash,
        ]);

        var deploymentHash = SccpV1.Keccak256(Concat(deploymentParts.ToArray()));
        var assetRouteHash = SccpV1.Keccak256(Concat(
            SccpV1.Keccak256("xor"u8),
            SccpV1.Keccak256(Encoding.UTF8.GetBytes(routeId)),
            AbiWord(revision),
            AbiWord(destination.TairaToTokenMultiplier),
            AbiWord(destination.MaxWrappedSupply)));
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

    private static byte[] TonDestinationBindingHash(
        SccpLaneIdV1 lane,
        SccpDestinationDeploymentV1 destination)
    {
        var walletCode = destination.TonJettonWalletCodeHash
            ?? throw new ArgumentException("TON Jetton wallet code hash is missing.");
        var circuit = destination.TonVerifierCircuitHash
            ?? throw new ArgumentException("TON verifier circuit hash is missing.");
        var proofProfile = destination.TonProofProfileCommitment
            ?? throw new ArgumentException("TON proof-profile commitment is missing.");
        var globalId = lane.Source switch
        {
            SccpNetworkV1.TonMainnet => -239,
            _ => throw new ArgumentException("SCCP TON destination binding lane is unsupported."),
        };
        using var payload = new MemoryStream();
        payload.Write("iroha:sccp:ton-destination-binding:v1"u8);
        payload.WriteByte(1);
        WriteVector(payload, "ton-groth16-bls12381-v1"u8);
        WriteVector(payload, SccpV1.CanonicalNetworkBytes(lane.Source));
        WriteInt32LittleEndian(payload, globalId);
        WriteUInt32LittleEndian(payload, 0);
        WriteUInt32LittleEndian(payload, 4);
        payload.Write(destination.TokenCodeHash);
        payload.Write(walletCode);
        payload.Write(destination.RouteCodeHash);
        payload.Write(destination.VerifierCodeHash);
        payload.Write(circuit);
        payload.Write(destination.VerifierKeyHash);
        payload.Write(proofProfile);
        foreach (var guardian in destination.TonMintBreakerGuardianKeys
                     ?.Ordered ?? throw new ArgumentException("TON mint-breaker guardians are missing."))
        {
            payload.Write(guardian);
        }
        payload.Write(destination.SemanticProofProfile.ProfileHash);
        payload.Write(destination.SoraFinalityAnchor.AnchorHash);
        return SHA256.HashData(payload.ToArray());
    }

    private static byte[] TonRouteConfigurationHash(
        SccpLaneIdV1 lane,
        uint revision,
        SccpDestinationDeploymentV1 destination)
    {
        var walletCode = destination.TonJettonWalletCodeHash
            ?? throw new ArgumentException("TON Jetton wallet code hash is missing.");
        var masterInitialData = destination.TonJettonMasterInitialDataHash
            ?? throw new ArgumentException("TON Jetton master initial-data hash is missing.");
        var routeInitialData = destination.TonRouteInitialDataHash
            ?? throw new ArgumentException("TON route initial-data hash is missing.");
        var circuit = destination.TonVerifierCircuitHash
            ?? throw new ArgumentException("TON verifier circuit hash is missing.");
        var proofProfile = destination.TonProofProfileCommitment
            ?? throw new ArgumentException("TON proof-profile commitment is missing.");
        var globalId = lane.Source switch
        {
            SccpNetworkV1.TonMainnet => -239,
            _ => throw new ArgumentException("SCCP TON route lane is unsupported."),
        };
        var sourceHash = SccpV1.LaneHash(lane);
        var reverseHash = SccpV1.LaneHash(new SccpLaneIdV1(lane.Target, lane.Source));
        RequireDistinctBytes(
        [
            sourceHash,
            reverseHash,
            destination.TokenCodeHash,
            masterInitialData,
            walletCode,
            destination.RouteCodeHash,
            routeInitialData,
            destination.VerifierCodeHash,
            circuit,
            destination.VerifierKeyHash,
            proofProfile,
            destination.SemanticProofProfile.ProfileHash,
            destination.SoraFinalityAnchor.AnchorHash,
            destination.DestinationBindingHash,
        ], "SCCP TON route hash roles");

        using var deployment = new MemoryStream();
        deployment.Write(destination.TokenCodeHash);
        deployment.Write(walletCode);
        deployment.Write(destination.RouteCodeHash);
        deployment.Write(destination.VerifierCodeHash);
        deployment.Write(circuit);
        deployment.Write(destination.VerifierKeyHash);
        deployment.Write(proofProfile);
        foreach (var guardian in destination.TonMintBreakerGuardianKeys
                     ?.Ordered ?? throw new ArgumentException("TON mint-breaker guardians are missing."))
        {
            deployment.Write(guardian);
        }
        deployment.Write(destination.SemanticProofProfile.ProfileHash);
        deployment.Write(destination.SoraFinalityAnchor.AnchorHash);
        deployment.Write(destination.DestinationBindingHash);
        var deploymentHash = SHA256.HashData(deployment.ToArray());

        using var assetRoute = new MemoryStream();
        WriteVector(assetRoute, "xor"u8);
        WriteVector(assetRoute, "taira_ton_xor"u8);
        WriteUInt32LittleEndian(assetRoute, revision);
        WriteUInt64LittleEndian(assetRoute, destination.TairaToTokenMultiplier);
        WriteUInt128LittleEndian(assetRoute, destination.MaxWrappedSupply);
        var assetRouteHash = SHA256.HashData(assetRoute.ToArray());

        using var payload = new MemoryStream();
        payload.Write("sccp:concrete-route-config:v1"u8);
        payload.WriteByte(1);
        WriteUInt32LittleEndian(payload, 4);
        WriteVector(payload, SccpV1.CanonicalNetworkBytes(lane.Source));
        WriteInt32LittleEndian(payload, globalId);
        payload.Write(sourceHash);
        payload.Write(reverseHash);
        payload.Write(deploymentHash);
        payload.Write(assetRouteHash);
        return SHA256.HashData(payload.ToArray());
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

    private static byte[] Bls12381PublicSignalSchemaHash()
    {
        using var canonical = new MemoryStream();
        canonical.WriteByte(1);
        WriteUInt32LittleEndian(canonical, checked((uint)Bls12381PublicSignalLabels.Length));
        foreach (var label in Bls12381PublicSignalLabels)
        {
            WriteVector(canonical, Encoding.UTF8.GetBytes(label));
        }

        return SHA256.HashData(Concat(
            "sccp:groth16-bls12381:public-signal-schema:v1"u8.ToArray(),
            canonical.ToArray()));
    }

    private static byte[] TonProofProfileCommitment() => SHA256.HashData(Concat(
        "sccp:ton:groth16-bls12381:proof-profile:v1"u8.ToArray(),
        [1],
        "ietf-bls12381-compressed-g1-48-g2-96"u8.ToArray(),
        "groth16-a-g1-b-g2-c-g1"u8.ToArray(),
        "sha256-sha256-label-value-mod-r"u8.ToArray(),
        Bls12381ScalarField,
        Bls12381PublicSignalSchemaHash()));

    private static (
        byte[] StatementHash,
        byte[] RequestHash,
        IReadOnlyList<byte[]> PublicSignals) TonProofRequestHashes(
        SccpNetworkV1 source,
        SccpNetworkV1 target,
        byte[] messageId,
        byte[] payloadHash,
        uint targetDomain,
        byte[] commitmentRoot,
        ulong finalityHeight,
        byte[] finalityBlockHash,
        byte[] canonicalPayload,
        byte[] bundleBytes,
        byte[] verifyingKey,
        byte[] verifierKeyHash,
        SccpSemanticProofProfileV1 semantic,
        byte[] semanticHash,
        SccpSoraFinalityAnchorV1 anchor,
        byte[] anchorHash,
        byte[] destinationBindingHash,
        byte[] routeConfigurationHash,
        byte[] verifierCircuitHash,
        byte[] proofProfileCommitment)
    {
        if (source != SccpNetworkV1.SoraTaira
            || target != SccpNetworkV1.TonMainnet
            || targetDomain != 4
            || semantic.Kind != SccpSemanticProofProfileKindV1.Groth16Bls12381)
        {
            throw new ArgumentException("SCCP TON proof-request lane and profile are invalid.");
        }
        var publicInputs = CanonicalPublicInputs(
            messageId,
            payloadHash,
            targetDomain,
            commitmentRoot,
            finalityHeight,
            finalityBlockHash);
        var semanticBytes = CanonicalSemanticProfileBytes(semantic);
        var anchorBytes = CanonicalFinalityAnchorBytes(anchor);

        using var statementPreimage = new MemoryStream();
        statementPreimage.WriteByte(1);
        statementPreimage.WriteByte(3);
        WriteVector(statementPreimage, SccpV1.CanonicalNetworkBytes(source));
        WriteVector(statementPreimage, SccpV1.CanonicalNetworkBytes(target));
        statementPreimage.Write(destinationBindingHash);
        statementPreimage.Write(routeConfigurationHash);
        statementPreimage.Write(verifierCircuitHash);
        statementPreimage.Write(verifierKeyHash);
        statementPreimage.Write(proofProfileCommitment);
        statementPreimage.Write(semanticHash);
        statementPreimage.Write(anchorHash);
        WriteVector(statementPreimage, semanticBytes);
        WriteVector(statementPreimage, anchorBytes);
        statementPreimage.Write(publicInputs);
        WriteVector(statementPreimage, canonicalPayload);
        WriteVector(statementPreimage, bundleBytes);
        var statementHash = SHA256.HashData(Concat(
            "sccp:groth16-bls12381:statement:v1"u8.ToArray(),
            statementPreimage.ToArray()));

        var signalValues = new byte[][]
        {
            messageId,
            payloadHash,
            AbiWord(targetDomain),
            commitmentRoot,
            AbiWord(finalityHeight),
            finalityBlockHash,
            AbiWord(source.DomainId()),
            statementHash,
            destinationBindingHash,
            routeConfigurationHash,
            anchorHash,
        };
        var signals = Bls12381PublicSignalLabels
            .Select((label, index) => Bls12381SignalWord(label, signalValues[index]))
            .ToArray();

        using var requestPreimage = new MemoryStream();
        requestPreimage.WriteByte(1);
        requestPreimage.WriteByte(3);
        WriteVector(requestPreimage, SccpV1.CanonicalNetworkBytes(source));
        WriteVector(requestPreimage, SccpV1.CanonicalNetworkBytes(target));
        requestPreimage.Write(publicInputs);
        foreach (var signal in signals)
        {
            requestPreimage.Write(signal);
        }
        WriteVector(requestPreimage, verifyingKey);
        WriteVector(requestPreimage, semanticBytes);
        WriteVector(requestPreimage, anchorBytes);
        WriteVector(requestPreimage, canonicalPayload);
        WriteVector(requestPreimage, bundleBytes);
        requestPreimage.Write(statementHash);
        requestPreimage.Write(destinationBindingHash);
        requestPreimage.Write(routeConfigurationHash);
        requestPreimage.Write(verifierCircuitHash);
        requestPreimage.Write(verifierKeyHash);
        requestPreimage.Write(proofProfileCommitment);
        requestPreimage.Write(semanticHash);
        requestPreimage.Write(anchorHash);
        var requestHash = SHA256.HashData(Concat(
            "sccp:groth16-bls12381:proof-request:v1"u8.ToArray(),
            requestPreimage.ToArray()));
        return (statementHash, requestHash, signals);
    }

    private static IReadOnlyList<byte[]> ParseTonPublicSignals(
        JsonElement item,
        IReadOnlyList<byte[]> expected,
        string label)
    {
        string[] fields =
        [
            "message_id",
            "payload_hash",
            "target_domain",
            "commitment_root",
            "finality_height",
            "finality_block_hash",
            "source_domain",
            "statement_hash",
            "destination_binding_hash",
            "route_configuration_hash",
            "sora_finality_anchor_hash",
        ];
        SccpJson.ExactFields(item, new HashSet<string>(fields, StringComparer.Ordinal), label);
        var result = fields.Select(field => LowerPrefixedHex32(item, field, allowZero: true)).ToArray();
        if (result.Length != expected.Count
            || result.Where((value, index) => !value.AsSpan().SequenceEqual(expected[index])).Any())
        {
            throw new ArgumentException($"{label} do not match the exact curve-specific signal derivation.");
        }

        return result;
    }

    private static byte[] Bls12381SignalWord(string label, byte[] value)
    {
        var labelHash = SHA256.HashData(Encoding.UTF8.GetBytes(label));
        var word = SHA256.HashData(Concat(labelHash, value));
        while (word.AsSpan().SequenceCompareTo(Bls12381ScalarField) >= 0)
        {
            SubtractBigEndian(word, Bls12381ScalarField);
        }
        return word;
    }

    private static void SubtractBigEndian(byte[] value, byte[] subtrahend)
    {
        var borrow = 0;
        for (var index = value.Length - 1; index >= 0; index--)
        {
            var difference = value[index] - subtrahend[index] - borrow;
            if (difference < 0)
            {
                difference += 256;
                borrow = 1;
            }
            else
            {
                borrow = 0;
            }
            value[index] = (byte)difference;
        }
    }

    private static byte[] CanonicalPublicInputs(
        byte[] messageId,
        byte[] payloadHash,
        uint targetDomain,
        byte[] commitmentRoot,
        ulong finalityHeight,
        byte[] finalityBlockHash)
    {
        using var output = new MemoryStream();
        output.WriteByte(1);
        output.Write(messageId);
        output.Write(payloadHash);
        WriteUInt32LittleEndian(output, targetDomain);
        output.Write(commitmentRoot);
        WriteUInt64LittleEndian(output, finalityHeight);
        output.Write(finalityBlockHash);
        return output.ToArray();
    }

    private static byte[] CanonicalSemanticProfileBytes(SccpSemanticProofProfileV1 profile) =>
        Concat(
            [1, profile.Kind == SccpSemanticProofProfileKindV1.Groth16Bls12381 ? (byte)1 : (byte)0, 1],
            profile.CircuitCommitment,
            profile.WitnessGeneratorCommitment,
            profile.PublicSignalSchemaHash);

    private static byte[] CanonicalFinalityAnchorBytes(SccpSoraFinalityAnchorV1 anchor)
    {
        using var output = new MemoryStream();
        output.WriteByte(1);
        output.WriteByte((byte)SccpNetworkV1.SoraTaira);
        WriteUInt16LittleEndian(output, anchor.ProtocolVersion);
        output.Write(anchor.ChainIdHash);
        WriteUInt64LittleEndian(output, anchor.CheckpointHeight);
        output.Write(anchor.CheckpointBlockHash);
        output.Write(anchor.CheckpointContextId);
        output.Write(anchor.CheckpointFinalityArtifactHash);
        return output.ToArray();
    }

    private static byte[] AbiAddress(byte[] value) => Concat(new byte[12], value);

    private static byte[] AbiTronAddress(byte[] value) => Concat(new byte[11], [0x41], value);

    private static byte[] AbiWord(ulong value)
    {
        var result = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(result.AsSpan(24), value);
        return result;
    }

    private static byte[] AbiWord(UInt128 value)
    {
        var result = new byte[32];
        BinaryPrimitives.WriteUInt64BigEndian(result.AsSpan(16), (ulong)(value >> 64));
        BinaryPrimitives.WriteUInt64BigEndian(result.AsSpan(24), (ulong)value);
        return result;
    }

    private static void WriteUInt16LittleEndian(Stream stream, ushort value)
    {
        Span<byte> bytes = stackalloc byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(bytes, value);
        stream.Write(bytes);
    }

    private static void WriteInt32LittleEndian(Stream stream, int value)
    {
        Span<byte> bytes = stackalloc byte[4];
        BinaryPrimitives.WriteInt32LittleEndian(bytes, value);
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

    private static void WriteUInt128LittleEndian(Stream stream, UInt128 value)
    {
        Span<byte> bytes = stackalloc byte[16];
        BinaryPrimitives.WriteUInt64LittleEndian(bytes, (ulong)value);
        BinaryPrimitives.WriteUInt64LittleEndian(bytes[8..], (ulong)(value >> 64));
        stream.Write(bytes);
    }

    private static void WriteVector(Stream stream, ReadOnlySpan<byte> value)
    {
        WriteUInt32LittleEndian(stream, checked((uint)value.Length));
        stream.Write(value);
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

    private static byte[] LowerPrefixedHex32(JsonElement item, string field, bool allowZero)
    {
        var value = SccpJson.Text(item, field);
        if (value.Length != 66
            || !value.StartsWith("0x", StringComparison.Ordinal)
            || value.AsSpan(2).ContainsAnyExcept("0123456789abcdef")
            || !allowZero && value.AsSpan(2).IndexOfAnyExcept('0') < 0)
        {
            throw new ArgumentException($"{field} must be canonical lowercase 0x-prefixed 32-byte hex.");
        }

        return Convert.FromHexString(value[2..]);
    }

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

    private static UInt128 DecimalUInt128(JsonElement item, string field, UInt128 minimum)
    {
        var value = DecimalText(item, field, minimum == 0 ? 0UL : 1UL);
        if (!TryParseUInt128(value, out var result) || result < minimum)
        {
            throw new ArgumentException($"{field} must be a canonical UInt128 decimal.");
        }

        return result;
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

    private static int SignedInt32(JsonElement item, string field)
    {
        var element = item.GetProperty(field);
        var text = element.GetRawText();
        if (element.ValueKind != JsonValueKind.Number
            || !int.TryParse(
                text,
                System.Globalization.NumberStyles.AllowLeadingSign,
                System.Globalization.CultureInfo.InvariantCulture,
                out var result)
            || result.ToString(System.Globalization.CultureInfo.InvariantCulture) != text)
        {
            throw new ArgumentException($"{field} must be a canonical signed Int32.");
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
