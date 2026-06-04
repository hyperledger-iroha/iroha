using Hyperledger.Iroha.Offline;

namespace Hyperledger.Iroha.Sdk.Tests;

public sealed class KagemushaRecursiveSpendNativeTests
{
    [Fact]
    public void RecursiveSpendNativeAvailabilityProbeDoesNotThrow()
    {
        _ = KagemushaRecursiveSpendNative.IsAvailable();
    }

    [Fact]
    public void RecursiveSpendNativeAvailabilityRequiresCompleteAbiSurface()
    {
        Assert.True(KagemushaRecursiveSpendNative.IsAvailable(() => 6u, () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(() => null, () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(() => 5u, () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(() => 6u, () => false));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => throw new DllNotFoundException("missing bridge"),
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => throw new EntryPointNotFoundException("missing ABI probe"),
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => throw new BadImageFormatException("wrong architecture"),
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => 6u,
            () => throw new EntryPointNotFoundException("missing recursive spend symbol")));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => 6u,
            () => throw new InvalidOperationException("symbol probe failed")));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => throw new ArgumentException("malformed ABI probe"),
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => throw new NullReferenceException("broken ABI probe"),
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => 6u,
            () => throw new ArgumentException("malformed symbol probe")));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => 6u,
            () => throw new NullReferenceException("broken symbol probe")));
    }

    [Fact]
    public void RecursiveSpendNativeAvailabilityRequiresExpectedMalformedArchiveProbeFailure()
    {
        Assert.True(KagemushaRecursiveSpendNative.IsExpectedMalformedArchiveProbeResult(
            -311,
            IntPtr.Zero,
            UIntPtr.Zero));
        Assert.False(KagemushaRecursiveSpendNative.IsExpectedMalformedArchiveProbeResult(
            0,
            IntPtr.Zero,
            UIntPtr.Zero));
        Assert.False(KagemushaRecursiveSpendNative.IsExpectedMalformedArchiveProbeResult(
            -1,
            IntPtr.Zero,
            UIntPtr.Zero));
        Assert.False(KagemushaRecursiveSpendNative.IsExpectedMalformedArchiveProbeResult(
            -311,
            IntPtr.Zero,
            (UIntPtr)1));
        Assert.False(KagemushaRecursiveSpendNative.IsExpectedMalformedArchiveProbeResult(
            -311,
            new IntPtr(1),
            UIntPtr.Zero));
    }

    [Fact]
    public void RecursiveSpendNativePreferredModeDefaultsToRecursiveWhenAvailable()
    {
        Assert.Equal(
            KagemushaOfflineSpendMode.RecursiveSpendV1,
            KagemushaRecursiveSpendNative.PreferredMode(true));
        Assert.Equal(
            KagemushaOfflineSpendMode.CheckedPrefoldV1,
            KagemushaRecursiveSpendNative.PreferredMode(false));
        Assert.Equal(
            "recursive_spend_v1",
            KagemushaOfflineSpendMode.RecursiveSpendV1.WireName());
        Assert.Equal(
            "checked_prefold_v1",
            KagemushaOfflineSpendMode.CheckedPrefoldV1.WireName());
        Assert.Equal(6u, KagemushaRecursiveSpendNative.RequiredBridgeAbiVersion);
        Assert.Equal(
            "kagemusha-recursive-aggregation-v1",
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1);
        Assert.Equal(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1);
        Assert.Equal(
            "kagemusha-recursive-spend-lineage-onehop-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1);
        Assert.Equal(
            "kagemusha-recursive-spend-lineage-append-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1);
        Assert.Equal(64u, KagemushaRecursiveSpendNative.CompactTokenMaxHops);
        Assert.Equal(64u, KagemushaRecursiveSpendNative.RecursiveSpendLineageWitnesslessMaxHopsV1);
        Assert.True(KagemushaRecursiveSpendNative.RecursiveSpendLineageTransitionCircuitWiredV1);
        Assert.Equal(
            1,
            KagemushaRecursiveSpendNative.RecursivePreviousProofOpenEnvelopesRequiredCountV1);
        Assert.Equal(
            8 * 1024 * 1024,
            KagemushaRecursiveSpendNative.RecursivePreviousProofOpenEnvelopesMaxBytes);
        Assert.Equal(
            128,
            KagemushaRecursiveSpendNative.RecursivePallasOpenEnvelopeMaxTranscriptLabelBytes);
        Assert.Equal(
            "iroha:kagemusha:v1:recursive-spend-transition-profile",
            KagemushaRecursiveSpendNative.RecursiveSpendTransitionProfileDomain);
        Assert.Equal(
            "iroha:kagemusha:v1:recursive-spend-transition-profile-digest",
            KagemushaRecursiveSpendNative.RecursiveSpendTransitionProfileDigestDomain);
        Assert.Equal(
            "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest",
            KagemushaRecursiveSpendNative.RecursiveSpendTransitionProfileBindingDigestDomain);
        Assert.Equal(
            "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendOpeningsPreflightDomainV1);
        Assert.Equal(
            "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendBoundaryDomainV1);
        Assert.Equal(
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1",
            KagemushaRecursiveSpendNative
                .RecursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1);
        Assert.Equal(
            "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1",
            KagemushaRecursiveSpendNative
                .RecursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1);
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(null));
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(""));
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(
                KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1));
        Assert.Equal(
            "unknown-kagemusha-recursive-spend-circuit",
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit"));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(null));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(""));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            "unknown-kagemusha-recursive-spend-circuit"));
        Assert.True(KagemushaRecursiveSpendNative.IsLineageProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsLineageProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsLineageProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsLineageAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsLineageAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            "unknown-kagemusha-recursive-spend-circuit"));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousLineageVerifierRecordForAppend(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.RequiresPreviousLineageVerifierRecordForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.RequiresPreviousLineageVerifierRecordForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.RequiresPreviousLineageVerifierRecordForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousLineageVerifierRecordForAppend(
            "unknown-kagemusha-recursive-spend-circuit"));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            ""));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1),
            "semantic previous proofs cannot select Reserved-lineage output");
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            "unknown-kagemusha-recursive-spend-circuit",
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            "unknown-kagemusha-recursive-spend-circuit"));
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(1u));
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(63u));
        Assert.True(
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(64u)
                == KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            "preferred append selector falls back at the witnessless hop cap");
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(0u));
        Assert.True(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(null, 1u));
        Assert.True(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.CompactTokenMaxHops - 1u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            0u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.CompactTokenMaxHops));
        Assert.True(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            63u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            64u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            "unknown-kagemusha-recursive-spend-circuit",
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            "unknown-kagemusha-recursive-spend-circuit",
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            1u),
            "semantic previous proofs cannot select Reserved-lineage output");
        Assert.True(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            "unknown-kagemusha-recursive-spend-circuit",
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            0u));
        Assert.True(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            2u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresLineageWitnessForRedeem(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageWitnesslessMaxHopsV1));
        Assert.False(KagemushaRecursiveSpendNative.RequiresLineageWitnessForRedeem(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageWitnesslessMaxHopsV1));
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            0u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresLineageWitnessForRedeem(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            2u));
        Assert.True(KagemushaRecursiveSpendNative.RequiresLineageWitnessForRedeem(null, 1u));
        foreach (var (circuitId, hopCount) in new (string?, uint)[]
        {
            (KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1, uint.MaxValue),
            (KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1, 0u),
            (KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1, uint.MaxValue),
            ("", 1u),
            ("unknown-kagemusha-recursive-spend-circuit", uint.MaxValue),
            (null, uint.MaxValue),
        })
        {
            Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(circuitId, hopCount));
            Assert.True(KagemushaRecursiveSpendNative.RequiresLineageWitnessForRedeem(circuitId, hopCount));
        }
        Assert.False(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(0u));
        Assert.True(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(1u));
        Assert.True(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(63u));
        Assert.False(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(64u));
        Assert.False(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(uint.MaxValue));
        Assert.True(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            64u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            0u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(null, 1u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend("", 1u));

        _ = KagemushaRecursiveSpendNative.PreferredMode();
    }

    [Fact]
    public void RecursiveSpendNativeRejectsEmptyArchivesBeforeLoadingNativeBridge()
    {
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Init(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Append(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.TransitionProfileInit(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.TransitionProfileAppend(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageAppendBoundary(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
            Array.Empty<byte>(),
            new byte[] { 0x01 }));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
            new byte[] { 0x01 },
            Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            Array.Empty<byte>(),
            new byte[] { 0x01 },
            new byte[] { 0x02 }));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            new byte[] { 0x01 },
            Array.Empty<byte>(),
            new byte[] { 0x02 }));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            new byte[] { 0x01 },
            new byte[] { 0x02 },
            Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Verify(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Redeem(Array.Empty<byte>()));
    }

    [Fact]
    public void RecursiveSpendNativeReadBridgeOutputPropagatesNativeRedeemRejection()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            KagemushaRecursiveSpendNative.ReadBridgeOutput(
                "connect_norito_kagemusha_recursive_spend_redeem",
                -311,
                IntPtr.Zero,
                UIntPtr.Zero));

        Assert.Contains("connect_norito_kagemusha_recursive_spend_redeem", error.Message);
        Assert.Contains("-311", error.Message);
    }

    [Fact]
    public void RecursiveSpendNativeReadBridgeOutputRejectsNullSuccessPointer()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            KagemushaRecursiveSpendNative.ReadBridgeOutput(
                "connect_norito_kagemusha_recursive_spend_redeem",
                0,
                IntPtr.Zero,
                (UIntPtr)1));

        Assert.Contains("null output pointer", error.Message);
    }

    [Fact]
    public void RecursiveSpendNativeReadBridgeOutputRejectsEmptySuccessOutput()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            KagemushaRecursiveSpendNative.ReadBridgeOutput(
                "connect_norito_kagemusha_recursive_spend_redeem",
                0,
                IntPtr.Zero,
                UIntPtr.Zero));

        Assert.Contains("empty output", error.Message);
    }

    [Fact]
    public void RecursiveSpendNativeRejectsMalformedArchivesWhenBridgeIsAvailable()
    {
        if (!KagemushaRecursiveSpendNative.IsAvailable())
        {
            return;
        }

        var malformed = new byte[] { 0x01, 0x02 };
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.Init(malformed));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.Append(malformed));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.TransitionProfileInit(malformed));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.TransitionProfileAppend(malformed));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.LineageAppendBoundary(malformed));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
            malformed,
            new byte[] { 0x03, 0x04 }));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            malformed,
            new byte[] { 0x03, 0x04 },
            new byte[] { 0x05, 0x06 }));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.Verify(malformed));
        Assert.Throws<InvalidOperationException>(() => KagemushaRecursiveSpendNative.Redeem(malformed));
    }
}
