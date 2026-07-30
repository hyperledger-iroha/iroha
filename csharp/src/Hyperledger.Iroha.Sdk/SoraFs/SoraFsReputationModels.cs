namespace Hyperledger.Iroha.SoraFs;

/// <summary>Canonical V1 degradation flag names in their required wire order.</summary>
public enum SoraFsReputationDegradationFlagNameV1
{
    ReserveWarning,
    ReserveGrace,
    ReserveDelinquent,
    ReserveDefault,
    ProofSuccessBelow90,
    ProofSuccessBelow80,
    ActiveDispute,
    SlashingEvent,
    LowScore,
}

/// <summary>One unit-valued provider degradation flag.</summary>
public sealed record class SoraFsReputationDegradationFlagV1(
    SoraFsReputationDegradationFlagNameV1 Flag);

/// <summary>The seven canonical V1 reputation scoring weights.</summary>
public sealed record class SoraFsReputationWeightsV1(
    byte Version,
    ushort PorSuccessBps,
    ushort PdpSuccessBps,
    ushort PotrSuccessBps,
    ushort LatencyBps,
    ushort DisputeBps,
    ushort TokenViolationBps,
    ushort RepairBreachBps);

/// <summary>Raw bounded metrics committed for one provider.</summary>
public sealed record class SoraFsReputationProviderMetricsV1(
    byte Version,
    ushort PorSuccessBps,
    ushort PdpSuccessBps,
    ushort PotrSuccessBps,
    ushort LatencyHealthBps,
    ushort DisputeRateBps,
    ushort TokenViolationRateBps,
    ushort RepairBreachRateBps);

/// <summary>One provider row in a committed reputation snapshot.</summary>
public sealed class SoraFsReputationProviderV1
{
    internal SoraFsReputationProviderV1(
        string providerId,
        ushort scoreBps,
        IEnumerable<SoraFsReputationDegradationFlagV1> degradationFlags,
        SoraFsReputationProviderMetricsV1 rawMetrics,
        string rawMetricsHashHex)
    {
        ProviderId = providerId;
        ScoreBps = scoreBps;
        DegradationFlags = Array.AsReadOnly(degradationFlags.ToArray());
        RawMetrics = rawMetrics;
        RawMetricsHashHex = rawMetricsHashHex;
    }

    public string ProviderId { get; }

    public ushort ScoreBps { get; }

    public IReadOnlyList<SoraFsReputationDegradationFlagV1> DegradationFlags { get; }

    public SoraFsReputationProviderMetricsV1 RawMetrics { get; }

    public string RawMetricsHashHex { get; }
}

/// <summary>A bounded, immutable reputation snapshot projection.</summary>
public sealed class SoraFsReputationSnapshotSummaryV1
{
    internal SoraFsReputationSnapshotSummaryV1(
        string snapshotIdHex,
        ulong generatedAtUnix,
        string? previousSnapshotIdHex,
        string merkleRootHex,
        ulong providerCount,
        ulong returnedProviderCount,
        ulong limit,
        bool truncatedProviders,
        ushort alphaBps,
        ushort currentScoreWeightBps,
        SoraFsReputationWeightsV1 weights,
        IEnumerable<SoraFsReputationProviderV1> providers)
    {
        SnapshotIdHex = snapshotIdHex;
        GeneratedAtUnix = generatedAtUnix;
        PreviousSnapshotIdHex = previousSnapshotIdHex;
        MerkleRootHex = merkleRootHex;
        ProviderCount = providerCount;
        ReturnedProviderCount = returnedProviderCount;
        Limit = limit;
        TruncatedProviders = truncatedProviders;
        AlphaBps = alphaBps;
        CurrentScoreWeightBps = currentScoreWeightBps;
        Weights = weights;
        Providers = Array.AsReadOnly(providers.ToArray());
    }

    public string SnapshotIdHex { get; }

    public ulong GeneratedAtUnix { get; }

    public string? PreviousSnapshotIdHex { get; }

    public string MerkleRootHex { get; }

    public ulong ProviderCount { get; }

    public ulong ReturnedProviderCount { get; }

    public ulong Limit { get; }

    public bool TruncatedProviders { get; }

    public ushort AlphaBps { get; }

    public ushort CurrentScoreWeightBps { get; }

    public SoraFsReputationWeightsV1 Weights { get; }

    public IReadOnlyList<SoraFsReputationProviderV1> Providers { get; }
}

/// <summary>Complete structural inclusion proof for one provider row.</summary>
public sealed class SoraFsReputationMerkleProofV1
{
    internal SoraFsReputationMerkleProofV1(
        string providerId,
        uint leafIndex,
        uint leafCount,
        IEnumerable<string> siblingsHex)
    {
        ProviderId = providerId;
        LeafIndex = leafIndex;
        LeafCount = leafCount;
        SiblingsHex = Array.AsReadOnly(siblingsHex.ToArray());
    }

    public string ProviderId { get; }

    public uint LeafIndex { get; }

    public uint LeafCount { get; }

    public IReadOnlyList<string> SiblingsHex { get; }
}

/// <summary>Provider row and inclusion proof returned by the provider route.</summary>
public sealed record class SoraFsReputationProviderResponseV1(
    string SnapshotIdHex,
    ulong GeneratedAtUnix,
    string MerkleRootHex,
    SoraFsReputationProviderV1 Provider,
    SoraFsReputationMerkleProofV1 Proof);

/// <summary>Active scoring weights bound to the latest committed snapshot.</summary>
public sealed record class SoraFsReputationWeightsResponseV1(
    string SnapshotIdHex,
    ulong GeneratedAtUnix,
    ushort AlphaBps,
    ushort CurrentScoreWeightBps,
    SoraFsReputationWeightsV1 Weights);

/// <summary>One committed reputation snapshot publication event.</summary>
public sealed record class SoraFsReputationSnapshotEventV1(
    byte Version,
    ulong Sequence,
    string SnapshotIdHex,
    ulong GeneratedAtUnix,
    string MerkleRootHex,
    uint ProviderCount,
    string? PreviousSnapshotIdHex);

/// <summary>One bounded page of committed reputation events.</summary>
public sealed class SoraFsReputationEventsResponseV1
{
    internal SoraFsReputationEventsResponseV1(
        ulong? since,
        ulong limit,
        ulong count,
        ulong? nextSince,
        IEnumerable<SoraFsReputationSnapshotEventV1> events)
    {
        Since = since;
        Limit = limit;
        Count = count;
        NextSince = nextSince;
        Events = Array.AsReadOnly(events.ToArray());
    }

    public ulong? Since { get; }

    public ulong Limit { get; }

    public ulong Count { get; }

    public ulong? NextSince { get; }

    public IReadOnlyList<SoraFsReputationSnapshotEventV1> Events { get; }
}

/// <summary>Base type for a validated reputation SSE frame.</summary>
public abstract record class SoraFsReputationSseFrameV1;

/// <summary>A committed snapshot frame from the reputation SSE stream.</summary>
public sealed record class SoraFsReputationSnapshotSseFrameV1(
    ulong Id,
    SoraFsReputationSnapshotEventV1 Event) : SoraFsReputationSseFrameV1;

/// <summary>A positive retained-journal gap reported by the reputation SSE stream.</summary>
public sealed record class SoraFsReputationLaggedSseFrameV1(
    ulong Skipped) : SoraFsReputationSseFrameV1;
