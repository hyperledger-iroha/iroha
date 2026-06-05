using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
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
            64 * 1024 * 1024,
            KagemushaRecursiveSpendNative.NativeArchiveMaxBytes);
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
        Assert.True(KagemushaRecursiveSpendNative.RequiresLineageKeyArtifactsForInit());
        Assert.True(KagemushaRecursiveSpendNative.RequiresLineageKeyArtifactsForAppendOutput(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.RequiresLineageKeyArtifactsForAppendOutput(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.RequiresLineageKeyArtifactsForAppendOutput(null));
        Assert.False(KagemushaRecursiveSpendNative.RequiresLineageKeyArtifactsForAppendOutput(""));
        Assert.False(KagemushaRecursiveSpendNative.RequiresLineageKeyArtifactsForAppendOutput(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.RequiresLineageKeyArtifactsForAppendOutput(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.RequiresLineageKeyArtifactsForAppendOutput(
            "unknown-kagemusha-recursive-spend-circuit"));
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
    public void RecursiveSpendSharedAbi6FixtureMatchesSdkSurface()
    {
        using var manifest = LoadSharedRecursiveSpendManifest();
        var root = manifest.RootElement;

        Assert.Equal(
            "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1",
            root.GetProperty("schema").GetString());
        Assert.Equal(
            KagemushaRecursiveSpendNative.RequiredBridgeAbiVersion,
            (uint)root.GetProperty("bridge_abi_version").GetInt32());
        Assert.Equal(9, root.GetProperty("operation_count").GetInt32());

        var circuitIds = root.GetProperty("proof_circuit_ids");
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            circuitIds.GetProperty("recursive_aggregation").GetString());
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            circuitIds.GetProperty("reserved_lineage").GetString());
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            circuitIds.GetProperty("reserved_lineage_one_hop").GetString());
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            circuitIds.GetProperty("reserved_lineage_append").GetString());

        var limits = root.GetProperty("limits");
        Assert.Equal(
            KagemushaRecursiveSpendNative.CompactTokenMaxHops,
            (uint)limits.GetProperty("compact_token_max_hops").GetInt32());
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageWitnesslessMaxHopsV1,
            (uint)limits.GetProperty("reserved_lineage_witnessless_max_hops").GetInt32());
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursivePreviousProofOpenEnvelopesRequiredCountV1,
            limits.GetProperty("previous_proof_open_envelopes_required_count").GetInt32());
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursivePreviousProofOpenEnvelopesMaxBytes,
            limits.GetProperty("previous_proof_open_envelopes_max_bytes").GetInt32());
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursivePallasOpenEnvelopeMaxTranscriptLabelBytes,
            limits.GetProperty("pallas_open_envelope_max_transcript_label_bytes").GetInt32());
        Assert.Equal(
            KagemushaRecursiveSpendNative.NativeArchiveMaxBytes,
            limits.GetProperty("native_archive_max_bytes").GetInt32());

        var domains = root.GetProperty("domains");
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendTransitionProfileDomain,
            domains.GetProperty("transition_profile").GetString());
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1,
            domains.GetProperty("lineage_append_boundary_final_note_binding").GetString());

        var symbols = new HashSet<string>();
        JsonElement appendWitness = default;
        var operationCount = 0;
        foreach (var operation in root.GetProperty("operations").EnumerateArray())
        {
            operationCount++;
            symbols.Add(operation.GetProperty("symbol").GetString()!);
            if (operation.GetProperty("name").GetString() == "lineage_witness_append_result")
            {
                appendWitness = operation;
            }
        }

        Assert.Equal(root.GetProperty("operation_count").GetInt32(), operationCount);
        Assert.True(symbols.SetEquals(new[]
        {
            "connect_norito_kagemusha_recursive_spend_init",
            "connect_norito_kagemusha_recursive_spend_append",
            "connect_norito_kagemusha_recursive_spend_transition_profile_init",
            "connect_norito_kagemusha_recursive_spend_transition_profile_append",
            "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
            "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
            "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
            "connect_norito_kagemusha_recursive_spend_verify",
            "connect_norito_kagemusha_recursive_spend_redeem",
        }));
        Assert.Equal(
            3,
            appendWitness.GetProperty("input_archives").GetArrayLength());
        Assert.Equal(
            "KagemushaRecursiveSpendLineageWitnessV1",
            appendWitness.GetProperty("output_archive").GetString());

        var payloadBenchmarks = root.GetProperty("payload_benchmarks");
        Assert.Equal(1751, payloadBenchmarks.GetProperty("semantic_payload_bytes").GetInt32());
        Assert.Equal(3847, payloadBenchmarks.GetProperty("reserved_lineage_payload_bytes").GetInt32());
        Assert.Equal(
            2817,
            payloadBenchmarks.GetProperty("reserved_lineage_transition_profile_bytes").GetInt32());

        using var archiveFixture = LoadSharedRecursiveSpendArchives();
        var archiveRoot = archiveFixture.RootElement;
        Assert.Equal(
            "iroha.kagemusha.recursive_spend.abi6.archive_fixtures.v1",
            archiveRoot.GetProperty("schema").GetString());
        var archiveNames = new HashSet<string>();
        JsonElement redeemArchive = default;
        JsonElement redeemInstructionArchive = default;
        foreach (var archive in archiveRoot.GetProperty("archives").EnumerateArray())
        {
            archiveNames.Add(archive.GetProperty("name").GetString()!);
            if (archive.GetProperty("name").GetString() == "redeem_request")
            {
                redeemArchive = archive;
            }
            if (archive.GetProperty("name").GetString() == "redeem_instruction")
            {
                redeemInstructionArchive = archive;
            }
        }

        Assert.True(archiveNames.SetEquals(new[]
        {
            "init_request",
            "init_bundle",
            "transition_profile_init",
            "append_request",
            "append_bundle",
            "transition_profile_append",
            "lineage_append_boundary",
            "lineage_witness_from_init_result",
            "lineage_witness_append_result",
            "verify_request",
            "verify_result",
            "redeem_request",
            "redeem_instruction",
        }));
        var requestFieldRecordsByType = new Dictionary<string, JsonElement[]>();
        var requestFieldsByType = new Dictionary<string, string[]>();
        foreach (var entry in archiveRoot.GetProperty("request_archive_fields").EnumerateArray())
        {
            var requestType = entry.GetProperty("norito_type").GetString()!;
            var fields = entry.GetProperty("fields").EnumerateArray().ToArray();
            requestFieldRecordsByType.Add(requestType, fields);
            requestFieldsByType.Add(
                requestType,
                fields.Select(field => field.GetProperty("name").GetString()!).ToArray());
        }

        Assert.True(requestFieldsByType.Keys.ToHashSet().SetEquals(new[]
        {
            "KagemushaRecursiveSpendInitRequestV1",
            "KagemushaRecursiveSpendAppendRequestV1",
            "KagemushaRecursiveSpendVerifyRequestV1",
            "KagemushaRecursiveSpendRedeemRequestV1",
        }));
        Assert.Equal(
            new[]
            {
                "record_bundle",
                "pallas_open_envelopes_archive",
                "current_note",
                "lineage_verifier_key",
                "lineage_proving_key_archive",
                "block_height",
            },
            requestFieldsByType["KagemushaRecursiveSpendInitRequestV1"]);
        Assert.Equal(
            new[]
            {
                "previous_bundle",
                "record_bundle",
                "pallas_open_envelopes_archive",
                "current_note",
                "output_proof_circuit_id",
                "previous_lineage_verifier_record",
                "previous_recursive_proof_open_envelopes_archive",
                "lineage_verifier_key",
                "lineage_proving_key_archive",
                "block_height",
            },
            requestFieldsByType["KagemushaRecursiveSpendAppendRequestV1"]);
        Assert.Equal(
            new[] { "bundle", "lineage_verifier_record", "block_height" },
            requestFieldsByType["KagemushaRecursiveSpendVerifyRequestV1"]);
        Assert.Equal(
            new[]
            {
                "bundle",
                "recipient",
                "public_amount",
                "redeem_proof",
                "lineage_witness",
                "lineage_verifier_record",
                "block_height",
            },
            requestFieldsByType["KagemushaRecursiveSpendRedeemRequestV1"]);
        foreach (var requestType in requestFieldsByType.Keys)
        {
            var blockHeight = requestFieldRecordsByType[requestType]
                .Single(field => field.GetProperty("name").GetString() == "block_height");
            Assert.Equal("Option<u64>", blockHeight.GetProperty("type").GetString());
            Assert.True(blockHeight.GetProperty("norito_default").GetBoolean());
            Assert.Equal(
                "verifier_record_activation_height",
                blockHeight.GetProperty("semantics").GetString());
        }
        Assert.Equal("redeem", redeemArchive.GetProperty("operation").GetString());
        Assert.Equal(
            "KagemushaRecursiveSpendRedeemRequestV1",
            redeemArchive.GetProperty("norito_type").GetString());
        Assert.Equal(
            "b83b33541f50ab893ae356c1f42da60aaf81da95bc4daf871511509fc8eea5b2",
            redeemArchive.GetProperty("sha256_hex").GetString());
        Assert.True(redeemArchive.GetProperty("byte_len").GetInt32() > 0);
        Assert.NotEmpty(Convert.FromBase64String(
            redeemArchive.GetProperty("bytes_base64").GetString()!));
        Assert.Equal("redeem", redeemInstructionArchive.GetProperty("operation").GetString());
        Assert.Equal(
            "RedeemKagemushaRecursive",
            redeemInstructionArchive.GetProperty("norito_type").GetString());
        Assert.Equal(
            "a598660cbfe91a207b64a69b7a9dbdc985fd901c60fe886aecb4dead4115169e",
            redeemInstructionArchive.GetProperty("sha256_hex").GetString());
        Assert.True(redeemInstructionArchive.GetProperty("byte_len").GetInt32() > 0);
        Assert.NotEmpty(Convert.FromBase64String(
            redeemInstructionArchive.GetProperty("bytes_base64").GetString()!));

        Assert.Equal(
            circuitIds.GetProperty("reserved_lineage_append").GetString(),
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(1u));
        Assert.Equal(
            circuitIds.GetProperty("reserved_lineage_append").GetString(),
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(63u));
        Assert.Equal(
            circuitIds.GetProperty("recursive_aggregation").GetString(),
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(64u));
        Assert.False(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(0u));
        Assert.True(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(63u));
        Assert.False(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(64u));
        Assert.True(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            2u));
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            65u));
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
    public void RecursiveSpendNativeReadBridgeOutputRejectsOversizedSuccessOutput()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            KagemushaRecursiveSpendNative.ReadBridgeOutput(
                "connect_norito_kagemusha_recursive_spend_redeem",
                0,
                IntPtr.Zero,
                (UIntPtr)((ulong)KagemushaRecursiveSpendNative.NativeArchiveMaxBytes + 1UL)));

        Assert.Contains("oversized output", error.Message);
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

    private static JsonDocument LoadSharedRecursiveSpendManifest()
    {
        return LoadSharedRecursiveSpendFixture("manifest.json");
    }

    private static JsonDocument LoadSharedRecursiveSpendArchives()
    {
        return LoadSharedRecursiveSpendFixture("archives.json");
    }

    private static JsonDocument LoadSharedRecursiveSpendFixture(string fileName)
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory != null)
        {
            var candidate = Path.Combine(
                directory.FullName,
                "fixtures",
                "kagemusha_recursive_spend_abi6",
                fileName);
            if (File.Exists(candidate))
            {
                return JsonDocument.Parse(File.ReadAllText(candidate));
            }

            directory = directory.Parent;
        }

        throw new FileNotFoundException($"missing shared recursive spend ABI-6 fixture {fileName}");
    }
}
