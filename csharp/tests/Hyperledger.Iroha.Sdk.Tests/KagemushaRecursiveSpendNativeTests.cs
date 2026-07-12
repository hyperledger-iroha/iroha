using System.Collections.Generic;
using System.Buffers.Binary;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using Hyperledger.Iroha.Norito;
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
        Assert.True(KagemushaRecursiveSpendNative.IsAvailable(() => 18u, () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(() => 17u, () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(() => 19u, () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(() => null, () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(() => 5u, () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(() => 18u, () => false));
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
            () => 18u,
            () => throw new EntryPointNotFoundException("missing recursive spend symbol")));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => 18u,
            () => throw new InvalidOperationException("symbol probe failed")));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => throw new ArgumentException("malformed ABI probe"),
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => throw new NullReferenceException("broken ABI probe"),
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => 18u,
            () => throw new ArgumentException("malformed symbol probe")));
        Assert.False(KagemushaRecursiveSpendNative.IsAvailable(
            () => 18u,
            () => throw new NullReferenceException("broken symbol probe")));
        Assert.True(KagemushaRecursiveSpendNative.IsCompactPaymentTokenProverAvailable(
            () => 18u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsCompactPaymentTokenProverAvailable(
            () => 7u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsCompactPaymentTokenProverAvailable(
            () => 5u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsCompactPaymentTokenProverAvailable(
            () => 18u,
            () => false));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveAggregationProofBundleProverAvailable(
            () => 19u,
            () => true));
        Assert.True(KagemushaRecursiveSpendNative.IsRecursiveAggregationProofBundleProverAvailable(
            () => 18u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveAggregationProofBundleProverAvailable(
            () => 5u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveAggregationProofBundleProverAvailable(
            () => 18u,
            () => false));
        Assert.True(KagemushaRecursiveSpendNative.IsPallasOpenEnvelopeBuilderAvailable(
            () => 7u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsPallasOpenEnvelopeBuilderAvailable(
            () => 6u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsPallasOpenEnvelopeBuilderAvailable(
            () => 7u,
            () => false));
        Assert.True(KagemushaRecursiveSpendNative.IsRecursiveCompactPaymentTokenProverAvailable(
            () => 7u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveCompactPaymentTokenProverAvailable(
            () => 6u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveCompactPaymentTokenProverAvailable(
            () => 7u,
            () => false));
        Assert.True(KagemushaRecursiveSpendNative.IsRecursiveCompactPaymentTokenVerifierAvailable(
            () => 7u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveCompactPaymentTokenVerifierAvailable(
            () => 6u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveCompactPaymentTokenVerifierAvailable(
            () => 7u,
            () => false));
        Assert.True(KagemushaRecursiveSpendNative.IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable(
            () => 7u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable(
            () => 6u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable(
            () => 7u,
            () => false));
        Assert.True(KagemushaRecursiveSpendNative.IsTopUpAvailable(
            () => 15u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsTopUpAvailable(
            () => 14u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsTopUpAvailable(
            () => 15u,
            () => false));
        Assert.False(KagemushaRecursiveSpendNative.IsTopUpAvailable(
            () => throw new EntryPointNotFoundException("missing ABI probe"),
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsTopUpAvailable(
            () => 15u,
            () => throw new EntryPointNotFoundException("missing top-up symbol")));
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
    public void RecursiveSpendNativeProbeResultClearsAndFreesUnexpectedNativeOutput()
    {
        var bytes = Encoding.UTF8.GetBytes("unexpected-kagemusha-probe-output-never-survives-free");
        var pointer = Marshal.AllocHGlobal(bytes.Length);
        var freed = false;
        Marshal.Copy(bytes, 0, pointer, bytes.Length);

        try
        {
            var accepted = KagemushaRecursiveSpendNative.ConsumeProbeResult(
                -311,
                pointer,
                (UIntPtr)bytes.Length,
                ptr =>
                {
                    Assert.Equal(pointer, ptr);
                    AssertPointerZeroed(ptr, bytes.Length);
                    Marshal.FreeHGlobal(ptr);
                    pointer = IntPtr.Zero;
                    freed = true;
                });

            Assert.False(accepted);
            Assert.True(freed);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    [Fact]
    public void RecursiveSpendNativePreferredModeSelectsOnlyFirstReleaseMode()
    {
        Assert.Equal(
            KagemushaOfflineSpendMode.RecursiveSpend,
            KagemushaRecursiveSpendNative.PreferredMode(true));
        Assert.Null(KagemushaRecursiveSpendNative.PreferredMode(false));
        Assert.Equal(
            "recursive_spend_v2",
            KagemushaOfflineSpendMode.RecursiveSpend.WireName());
        Assert.True(KagemushaRecursiveSpendNative.IsSpendAgainMode("recursive_spend_v2"));
        Assert.False(KagemushaRecursiveSpendNative.IsSpendAgainMode("recursive_spend_v1"));
        Assert.False(KagemushaRecursiveSpendNative.IsSpendAgainMode("recursive_compact_v1"));
        Assert.DoesNotContain(
            "checked_prefold_v1",
            Enum.GetValues<KagemushaOfflineSpendMode>().Select(mode => mode.WireName()));
        Assert.Equal(18u, KagemushaRecursiveSpendNative.RequiredNativeBridgeAbiVersion);
        Assert.Equal(7u, KagemushaRecursiveSpendNative.RecursiveCompactRequiredNativeBridgeAbiVersion);
        Assert.Equal(15u, KagemushaRecursiveSpendNative.TopUpRequiredNativeBridgeAbiVersion);
        Assert.Equal(
            "kagemusha-recursive-compact-v1",
            KagemushaRecursiveSpendNative.RecursiveCompactCircuitIdV1);
        Assert.Equal(
            "iroha_data_model::offline::model::KagemushaRecursiveSpendBundleV1",
            KagemushaRecursiveSpendNative.RecursiveSpendBundleWireName);
        Assert.Equal(
            "iroha_data_model::offline::model::KagemushaRecursiveAggregationProofPublicInputs",
            KagemushaRecursiveSpendNative.RecursiveAggregationProofPublicInputsWireName);
        Assert.Equal(
            "iroha:kagemusha:v1:recursive-spend-accumulator",
            KagemushaRecursiveSpendNative.RecursiveSpendAccumulatorDomain);
        var validRecursiveCompactVerifierKeys = KagemushaNoritoFrameWithPayload(0xe2);
        AssertArgumentDiagnostic(
            "Compact token archive must not be empty.",
            "compactTokenArchive",
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(
                Array.Empty<byte>(),
                validRecursiveCompactVerifierKeys));

        AssertArgumentDiagnostic(
            "Compact token archive must be a valid Norito archive.",
            "compactTokenArchive",
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(
                new byte[] { 0x01 },
                validRecursiveCompactVerifierKeys));

        AssertArgumentDiagnostic(
            "Compact token archive must contain a non-empty Norito payload.",
            "compactTokenArchive",
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(
                KagemushaNoritoFrame(0x4b),
                validRecursiveCompactVerifierKeys));

        AssertArgumentDiagnostic(
            "Recursive compact verifier keys archive must not be empty.",
            "recursiveCompactVerifierKeysArchive",
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(
                KagemushaNoritoFrameWithPayload(0x4b),
                Array.Empty<byte>()));

        AssertArgumentDiagnostic(
            "Recursive compact verifier keys archive must be a valid Norito archive.",
            "recursiveCompactVerifierKeysArchive",
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(
                KagemushaNoritoFrameWithPayload(0x4b),
                new byte[] { 0x01 }));

        AssertArgumentDiagnostic(
            "Recursive compact verifier keys archive must contain a non-empty Norito payload.",
            "recursiveCompactVerifierKeysArchive",
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(
                KagemushaNoritoFrameWithPayload(0x4b),
                KagemushaNoritoFrame(0xe2)));

        AssertArgumentDiagnostic(
            "Compact token archive must not be empty.",
            "compactTokenArchive",
            () => KagemushaRecursiveSpendNative.VerifyRecursiveSpendCompactPaymentTokenProjection(
                Array.Empty<byte>(),
                KagemushaNoritoFrameWithPayload(0x4b)));

        AssertArgumentDiagnostic(
            "Verifier record archive must be a valid Norito archive.",
            "verifierRecordArchive",
            () => KagemushaRecursiveSpendNative.VerifyRecursiveSpendCompactPaymentTokenProjection(
                KagemushaNoritoFrameWithPayload(0x4b),
                new byte[] { 0x01 }));

        AssertArgumentDiagnostic(
            "Verifier record archive must contain a non-empty Norito payload.",
            "verifierRecordArchive",
            () => KagemushaRecursiveSpendNative.VerifyRecursiveSpendCompactPaymentTokenProjection(
                KagemushaNoritoFrameWithPayload(0x4b),
                KagemushaNoritoFrame(0x4b)));
        Assert.Equal(
            "kagemusha-recursive-aggregation-v1",
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1);
        Assert.Equal(
            "kagemusha-recursive-spend-lineage-onehop-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1);
        Assert.Equal(
            "kagemusha-recursive-spend-lineage-append-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1);
        Assert.Equal("halo2/ipa", KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend);
        Assert.Equal(64u, KagemushaRecursiveSpendNative.CompactTokenMaxHops);
        Assert.Equal(64u, KagemushaRecursiveSpendNative.RecursiveSpendLineageWitnesslessMaxHopsV1);
        Assert.False(KagemushaRecursiveSpendNative.RecursiveSpendLineageTransitionCircuitWiredV1);
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
            256 * 1024 * 1024,
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
            string.Empty,
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(null));
        Assert.Equal(
            string.Empty,
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(""));
        Assert.Equal(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(
                "kagemusha-recursive-spend-lineage-v1"));
        Assert.Equal(
            "unknown-kagemusha-recursive-spend-circuit",
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit"));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(null));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(""));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            "kagemusha-recursive-spend-lineage-v1"));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            "unknown-kagemusha-recursive-spend-circuit"));
        Assert.False(KagemushaRecursiveSpendNative.IsLineageProofCircuitId(
            "kagemusha-recursive-spend-lineage-v1"));
        Assert.True(KagemushaRecursiveSpendNative.IsLineageProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsLineageProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsLineageAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsLineageAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.RequiresLineageKeyArtifactsForInit());
        foreach (var openingLen in new[] { 2, 4, 8, 16, 32, 64, 128 })
        {
            Assert.True(
                KagemushaRecursiveSpendNative.IsSupportedLineageKeyArtifactOpeningLen(openingLen));
        }
        foreach (var openingLen in new[] { 0, 1, 3, 65, 129, -2 })
        {
            Assert.False(
                KagemushaRecursiveSpendNative.IsSupportedLineageKeyArtifactOpeningLen(openingLen));
        }
        var initVerifierKey = KagemushaLineageVerifierKey(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            0xa1);
        var initProvingKeyArchive = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            initVerifierKey,
            0xa2);
        var appendVerifierKey = KagemushaLineageVerifierKey(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            0xa3);
        var appendProvingKeyArchive = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            appendVerifierKey,
            0xa4);
        var verifierKey = (byte[])initVerifierKey.Clone();
        var provingKey = (byte[])initProvingKeyArchive.Clone();
        var initArtifacts = KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
            128,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            verifierKey,
            provingKey);
        Array.Fill(verifierKey, (byte)0);
        Array.Fill(provingKey, (byte)0);
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            initArtifacts.ProofCircuitId);
        Assert.Equal(128, initArtifacts.VerifierOpeningLen);
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            initArtifacts.LineageVerifierKeyBackend);
        Assert.Equal(initVerifierKey, initArtifacts.LineageVerifierKey());
        Assert.Equal(initProvingKeyArchive, initArtifacts.LineageProvingKeyArchive());
        Assert.True(initArtifacts.IsInitArtifact);
        Assert.False(initArtifacts.IsAppendArtifact);
        var returnedVerifierKey = initArtifacts.LineageVerifierKey();
        returnedVerifierKey[0] = 9;
        Assert.Equal(initVerifierKey, initArtifacts.LineageVerifierKey());
        var returnedProvingKeyArchive = initArtifacts.LineageProvingKeyArchive();
        returnedProvingKeyArchive[0] = 9;
        Assert.Equal(initProvingKeyArchive, initArtifacts.LineageProvingKeyArchive());
        var appendArtifacts = KagemushaRecursiveSpendNative.LineageKeyArtifactsForAppend(
            64,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            appendVerifierKey,
            appendProvingKeyArchive);
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            appendArtifacts.ProofCircuitId);
        Assert.False(appendArtifacts.IsInitArtifact);
        Assert.True(appendArtifacts.IsAppendArtifact);
        Assert.Equal(
            2,
            KagemushaRecursiveSpendNative.LineageKeyArtifacts(
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                2,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                appendVerifierKey,
                appendProvingKeyArchive).VerifierOpeningLen);
        Assert.Equal(
            initArtifacts.ProofCircuitId,
            KagemushaRecursiveSpendNative.ValidateLineageKeyArtifacts(initArtifacts).ProofCircuitId);
        AssertExactLineageKeyArtifactError(
            "lineage_verifier_key",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                appendVerifierKey,
                appendProvingKeyArchive));
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                appendProvingKeyArchive));
        AssertExactLineageKeyArtifactError(
            "lineage_verifier_key",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                Encoding.ASCII.GetBytes("not-zk1"),
                initProvingKeyArchive));
        var duplicateCidVerifierKey = initVerifierKey
            .Concat(KagemushaZk1Tlv(
                "CID1",
                Encoding.UTF8.GetBytes(
                    KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1)))
            .ToArray();
        AssertExactLineageKeyArtifactError(
            "lineage_verifier_key",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                duplicateCidVerifierKey,
                initProvingKeyArchive));
        var whitespaceCidVerifierKey = KagemushaLineageVerifierKey(
            $" {KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1} ",
            0xb2);
        var whitespaceCidProvingKeyArchive = KagemushaLineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            KagemushaVerifierKeyCommitment(whitespaceCidVerifierKey),
            Enumerable.Repeat((byte)0xb3, 64).ToArray());
        AssertExactLineageKeyArtifactError(
            "lineage_verifier_key",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                whitespaceCidVerifierKey,
                whitespaceCidProvingKeyArchive));
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                Encoding.ASCII.GetBytes("not-norito")));
        var missingCircuitArchive = KagemushaLineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            KagemushaVerifierKeyCommitment(initVerifierKey),
            Enumerable.Repeat((byte)0xa5, 64).ToArray());
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                missingCircuitArchive));
        var smuggledCircuitArchive = KagemushaLineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            KagemushaVerifierKeyCommitment(initVerifierKey),
            Encoding.UTF8.GetBytes(KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1)
                .Concat(Enumerable.Repeat((byte)0xa6, 64))
                .ToArray());
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                smuggledCircuitArchive));
        var wrongCommitmentArchive = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            appendVerifierKey,
            0xa6);
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                wrongCommitmentArchive));
        var smuggledCommitmentArchive = KagemushaLineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            KagemushaVerifierKeyCommitment(appendVerifierKey),
            KagemushaVerifierKeyCommitment(initVerifierKey)
                .Concat(Enumerable.Repeat((byte)0xa7, 64))
                .ToArray());
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                smuggledCommitmentArchive));
        var wrongVersionArchive = KagemushaLineageProvingKeyArchiveRaw(
            2,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            KagemushaVerifierKeyCommitment(initVerifierKey),
            Enumerable.Repeat((byte)0xa8, 64).ToArray());
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                wrongVersionArchive));
        var emptyProvingKeyArchive = KagemushaLineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            KagemushaVerifierKeyCommitment(initVerifierKey),
            Array.Empty<byte>());
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                emptyProvingKeyArchive));
        var trailingPayloadArchive = KagemushaLineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            KagemushaVerifierKeyCommitment(initVerifierKey),
            Enumerable.Repeat((byte)0xa9, 64).ToArray(),
            trailingPayload: new byte[] { 0x7f });
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                trailingPayloadArchive));
        var oldSchemaArchive = KagemushaLineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            KagemushaVerifierKeyCommitment(initVerifierKey),
            Enumerable.Repeat((byte)0xaa, 64).ToArray(),
            schemaHash: OldKagemushaLineageProvingKeyArchiveSchemaHash);
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                oldSchemaArchive));
        var packedStructArchive = KagemushaLineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            KagemushaVerifierKeyCommitment(initVerifierKey),
            Enumerable.Repeat((byte)0xab, 64).ToArray(),
            flags: KagemushaNoritoCompactLenFlag | KagemushaNoritoPackedStructFlag);
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                packedStructArchive));
        var fieldBitsetArchive = KagemushaLineageProvingKeyArchiveRaw(
            1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            KagemushaVerifierKeyCommitment(initVerifierKey),
            Enumerable.Repeat((byte)0xac, 64).ToArray(),
            flags: KagemushaNoritoCompactLenFlag | PrivacyNoritoFieldBitsetFlag);
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                fieldBitsetArchive));
        var circuitIdBytes = Encoding.UTF8.GetBytes(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1);
        var overlongVersionLengthArchive = KagemushaNoritoFrameFromSchemaHash(
            KagemushaLineageProvingKeyArchiveSchemaHash,
            KagemushaOverlongCompactLength(2)
                .Concat(new byte[] { 1, 0 })
                .Concat(KagemushaNoritoField(KagemushaNoritoString(
                    KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1)))
                .Concat(KagemushaNoritoField(KagemushaVerifierKeyCommitment(initVerifierKey)))
                .Concat(KagemushaNoritoField(KagemushaNoritoByteVec(
                    Enumerable.Repeat((byte)0xad, 64).ToArray())))
                .ToArray(),
            KagemushaNoritoCompactLenFlag);
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                overlongVersionLengthArchive));
        var oversizedTerminalCompactLengthArchive = KagemushaNoritoFrameFromSchemaHash(
            KagemushaLineageProvingKeyArchiveSchemaHash,
            KagemushaOversizedTerminalCompactLength()
                .Concat(new byte[] { 1, 0 })
                .Concat(KagemushaNoritoField(KagemushaNoritoString(
                    KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1)))
                .Concat(KagemushaNoritoField(KagemushaVerifierKeyCommitment(initVerifierKey)))
                .Concat(KagemushaNoritoField(KagemushaNoritoByteVec(
                    Enumerable.Repeat((byte)0xb0, 64).ToArray())))
                .ToArray(),
            KagemushaNoritoCompactLenFlag);
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                oversizedTerminalCompactLengthArchive));
        var hugeCanonicalCompactLengthArchive = KagemushaNoritoFrameFromSchemaHash(
            KagemushaLineageProvingKeyArchiveSchemaHash,
            KagemushaHugeCanonicalCompactLength()
                .Concat(new byte[] { 1, 0 })
                .Concat(KagemushaNoritoField(KagemushaNoritoString(
                    KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1)))
                .Concat(KagemushaNoritoField(KagemushaVerifierKeyCommitment(initVerifierKey)))
                .Concat(KagemushaNoritoField(KagemushaNoritoByteVec(
                    Enumerable.Repeat((byte)0xb1, 64).ToArray())))
                .ToArray(),
            KagemushaNoritoCompactLenFlag);
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                hugeCanonicalCompactLengthArchive));
        var overlongCircuitStringArchive = KagemushaNoritoFrameFromSchemaHash(
            KagemushaLineageProvingKeyArchiveSchemaHash,
            KagemushaNoritoField(new byte[] { 1, 0 })
                .Concat(KagemushaNoritoField(
                    KagemushaOverlongCompactLength(circuitIdBytes.Length)
                        .Concat(circuitIdBytes)
                        .ToArray()))
                .Concat(KagemushaNoritoField(KagemushaVerifierKeyCommitment(initVerifierKey)))
                .Concat(KagemushaNoritoField(KagemushaNoritoByteVec(
                    Enumerable.Repeat((byte)0xae, 64).ToArray())))
                .ToArray(),
            KagemushaNoritoCompactLenFlag);
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                overlongCircuitStringArchive));
        var invalidUtf8CircuitArchive = KagemushaNoritoFrameFromSchemaHash(
            KagemushaLineageProvingKeyArchiveSchemaHash,
            KagemushaNoritoField(new byte[] { 1, 0 })
                .Concat(KagemushaNoritoField(
                    KagemushaNoritoLength(1, KagemushaNoritoCompactLenFlag)
                        .Concat(new byte[] { (byte)0xff })
                        .ToArray()))
                .Concat(KagemushaNoritoField(KagemushaVerifierKeyCommitment(initVerifierKey)))
                .Concat(KagemushaNoritoField(KagemushaNoritoByteVec(
                    circuitIdBytes
                        .Concat(Enumerable.Repeat((byte)0xaf, 64))
                        .ToArray())))
                .ToArray(),
            KagemushaNoritoCompactLenFlag);
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                invalidUtf8CircuitArchive));
        AssertExactLineageKeyArtifactError(
            "lineage_proving_key_archive",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                initVerifierKey,
                KagemushaNoritoFrame(0x9a)));
        AssertArgumentDiagnostic(
            "lineage_key_artifacts",
            "artifacts",
            () => KagemushaRecursiveSpendNative.ValidateLineageKeyArtifacts(null));
        AssertArgumentDiagnostic(
            "proof_circuit_id",
            "artifacts",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifacts(
                "kagemusha-recursive-spend-lineage-v1",
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                new byte[] { 1 },
                new byte[] { 2 }));
        AssertArgumentDiagnostic(
            "proof_circuit_id",
            "artifacts",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifacts(
                "unknown-kagemusha-recursive-spend-circuit",
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                new byte[] { 1 },
                new byte[] { 2 }));
        AssertArgumentDiagnostic(
            "verifier_opening_len",
            "artifacts",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                3,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                new byte[] { 1 },
                new byte[] { 2 }));
        AssertArgumentDiagnostic(
            "lineage_verifier_key",
            "artifacts",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                "halo2/kzg",
                new byte[] { 1 },
                new byte[] { 2 }));
        AssertArgumentDiagnostic(
            "lineage_verifier_key",
            "artifacts",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                Array.Empty<byte>(),
                new byte[] { 2 }));
        AssertArgumentDiagnostic(
            "lineage_proving_key_archive",
            "artifacts",
            () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                128,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                new byte[] { 1 },
                Array.Empty<byte>()));
        Assert.False(KagemushaRecursiveSpendNative.RequiresLineageKeyArtifactsForAppendOutput(
            "kagemusha-recursive-spend-lineage-v1"));
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
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            "kagemusha-recursive-spend-lineage-v1"));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            "unknown-kagemusha-recursive-spend-circuit"));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousLineageVerifierRecordForAppend(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousLineageVerifierRecordForAppend(
            "kagemusha-recursive-spend-lineage-v1"));
        Assert.True(KagemushaRecursiveSpendNative.RequiresPreviousLineageVerifierRecordForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.RequiresPreviousLineageVerifierRecordForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousLineageVerifierRecordForAppend(
            "unknown-kagemusha-recursive-spend-circuit"));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            ""));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            "kagemusha-recursive-spend-lineage-v1",
            "kagemusha-recursive-spend-lineage-v1"));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            "kagemusha-recursive-spend-lineage-v1"),
            "semantic previous proofs cannot select Reserved-lineage output");
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            "unknown-kagemusha-recursive-spend-circuit",
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            "kagemusha-recursive-spend-lineage-v1",
            "unknown-kagemusha-recursive-spend-circuit"));
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(1u));
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(63u));
        Assert.True(
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(64u)
                == KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            "the semantic append circuit remains preferred while lineage transition verification is unavailable");
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(0u));
        Assert.True(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(null, 1u));
        Assert.True(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.CompactTokenMaxHops - 1u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            0u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.CompactTokenMaxHops));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            "kagemusha-recursive-spend-lineage-v1",
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            "kagemusha-recursive-spend-lineage-v1",
            63u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            "kagemusha-recursive-spend-lineage-v1",
            64u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            "unknown-kagemusha-recursive-spend-circuit",
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            "unknown-kagemusha-recursive-spend-circuit",
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            "kagemusha-recursive-spend-lineage-v1",
            1u),
            "semantic previous proofs cannot select Reserved-lineage output");
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            "kagemusha-recursive-spend-lineage-v1",
            "kagemusha-recursive-spend-lineage-v1",
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
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
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            "kagemusha-recursive-spend-lineage-v1",
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            2u));
        Assert.True(KagemushaRecursiveSpendNative.RequiresLineageWitnessForRedeem(
            "kagemusha-recursive-spend-lineage-v1",
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageWitnesslessMaxHopsV1));
        Assert.True(KagemushaRecursiveSpendNative.RequiresLineageWitnessForRedeem(
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageWitnesslessMaxHopsV1));
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            "kagemusha-recursive-spend-lineage-v1",
            0u));
        Assert.True(KagemushaRecursiveSpendNative.RequiresLineageWitnessForRedeem(
            "kagemusha-recursive-spend-lineage-v1",
            2u));
        Assert.True(KagemushaRecursiveSpendNative.RequiresLineageWitnessForRedeem(null, 1u));
        foreach (var (circuitId, hopCount) in new (string?, uint)[]
        {
            ("kagemusha-recursive-spend-lineage-v1", uint.MaxValue),
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
        foreach (var circuitId in new[]
        {
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
        })
        {
            foreach (var hopCount in new[] { 1u, 2u, 63u, 64u })
            {
                Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(circuitId, hopCount));
                Assert.True(
                    KagemushaRecursiveSpendNative.RequiresLineageWitnessForRedeem(circuitId, hopCount),
                    $"{circuitId} hop {hopCount} must require a record-backed lineage witness");
            }
        }
        foreach (var hopCount in new[] { 0u, 1u, 2u, 63u, 64u, uint.MaxValue })
        {
            Assert.False(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(hopCount));
        }
        Assert.False(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(uint.MaxValue));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            "kagemusha-recursive-spend-lineage-v1",
            1u));
        Assert.True(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            "kagemusha-recursive-spend-lineage-v1",
            64u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            "kagemusha-recursive-spend-lineage-v1",
            0u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(null, 1u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend("", 1u));

        _ = KagemushaRecursiveSpendNative.PreferredMode();
    }

    [Fact]
    public void RecursiveSpendNativeVerifyLineagePreflightRejectsMissingOrDanglingRecordBeforeNativeBridge()
    {
        var malformedRequestArchive = new byte[] { 0xde, 0xad, 0xbe, 0xef };

        AssertArgumentDiagnostic(
            "lineageVerifierRecord is required for reserved-lineage bundles",
            "hasLineageVerifierRecord",
            () => KagemushaRecursiveSpendNative.Verify(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                hasLineageVerifierRecord: false));

        AssertArgumentDiagnostic(
            "lineageVerifierRecord is only valid for reserved-lineage bundles",
            "hasLineageVerifierRecord",
            () => KagemushaRecursiveSpendNative.Verify(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                hasLineageVerifierRecord: true));

        KagemushaRecursiveSpendNative.ValidateVerifyLineagePreflight(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            hasLineageVerifierRecord: false);
        KagemushaRecursiveSpendNative.ValidateVerifyLineagePreflight(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            hasLineageVerifierRecord: true);

        AssertArgumentDiagnostic(
            "Request archive must be a valid Norito archive.",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Verify(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                hasLineageVerifierRecord: false));

        AssertArgumentDiagnostic(
            "Request archive must be a valid Norito archive.",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Verify(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                hasLineageVerifierRecord: true));
    }

    [Fact]
    public void RecursiveSpendNativeRedeemLineagePreflightRejectsMissingMaterialBeforeNativeBridge()
    {
        var requestArchive = KagemushaNoritoFrameWithPayload(0x4b);

        AssertArgumentDiagnostic(
            "lineageWitness is required for this bundle",
            "hasLineageWitness",
            () => KagemushaRecursiveSpendNative.Redeem(
                requestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                1u,
                hasLineageWitness: false,
                hasLineageVerifierRecord: false));

        AssertArgumentDiagnostic(
            "lineageVerifierRecord is required for reserved-lineage bundles",
            "hasLineageVerifierRecord",
            () => KagemushaRecursiveSpendNative.Redeem(
                requestArchive,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false));

        AssertArgumentDiagnostic(
            "lineageVerifierRecord is required for reserved-lineage bundles",
            "lineageVerifierRecordCount",
            () => KagemushaRecursiveSpendNative.Redeem(
                requestArchive,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                lineageVerifierRecordCount: 0));

        AssertArgumentDiagnostic(
            "lineageVerifierRecords count must be non-negative",
            "lineageVerifierRecordCount",
            () => KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                lineageVerifierRecordCount: -1));

        var missingMultiProfileRecords = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                lineageVerifierRecordCount: 0,
                lineageWitnessHasReservedPreviousProofs: true));
        Assert.Equal("lineageVerifierRecordCount", missingMultiProfileRecords.ParamName);
        Assert.Contains(
            "lineageVerifierRecord is required for lineage witnesses with reserved-lineage proofs",
            missingMultiProfileRecords.Message);

        var danglingSingleRecord = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: true,
                lineageVerifierRecordCount: 0,
                lineageWitnessHasReservedPreviousProofs: false));
        Assert.Equal("hasLineageVerifierRecord", danglingSingleRecord.ParamName);
        Assert.Contains(
            "lineageVerifierRecord is only valid for reserved-lineage bundles or lineage witnesses with reserved-lineage proofs",
            danglingSingleRecord.Message);

        var danglingPluralRecords = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.Redeem(
                requestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                lineageVerifierRecordCount: 1,
                lineageWitnessHasReservedPreviousProofs: false));
        Assert.Equal("lineageVerifierRecordCount", danglingPluralRecords.ParamName);
        Assert.Contains(
            "lineageVerifierRecord is only valid for reserved-lineage bundles or lineage witnesses with reserved-lineage proofs",
            danglingPluralRecords.Message);

        var negativePluralCount = Assert.Throws<ArgumentOutOfRangeException>(() =>
            KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                lineageVerifierRecordCount: -1,
                lineageWitnessHasReservedPreviousProofs: false));
        Assert.Equal("lineageVerifierRecordCount", negativePluralCount.ParamName);

        var overLimitPluralCount = Assert.Throws<ArgumentOutOfRangeException>(() =>
            KagemushaRecursiveSpendNative.Redeem(
                requestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                lineageVerifierRecordCount: (int)KagemushaRecursiveSpendNative.CompactTokenMaxHops + 1,
                lineageWitnessHasReservedPreviousProofs: true));
        Assert.Equal("lineageVerifierRecordCount", overLimitPluralCount.ParamName);
        Assert.Contains("must not exceed", overLimitPluralCount.Message);

        var reservedPreviousWithoutWitness = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                1u,
                hasLineageWitness: false,
                hasLineageVerifierRecord: false,
                lineageVerifierRecordCount: 1,
                lineageWitnessHasReservedPreviousProofs: true));
        Assert.Equal("hasLineageWitness", reservedPreviousWithoutWitness.ParamName);

        KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u,
            hasLineageWitness: true,
            hasLineageVerifierRecord: false);
        KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u,
            hasLineageWitness: true,
            hasLineageVerifierRecord: false,
            lineageVerifierRecordCount: 2,
            lineageWitnessHasReservedPreviousProofs: true);
        KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            2u,
            hasLineageWitness: true,
            hasLineageVerifierRecord: true);
        KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            1u,
            hasLineageWitness: true,
            hasLineageVerifierRecord: false,
            lineageVerifierRecordCount: 1,
            lineageWitnessHasReservedPreviousProofs: false);
    }

    [Fact]
    public void RecursiveSpendNativeRedeemChangeOutputPreflightRejectsInvalidRelationshipsBeforeNativeBridge()
    {
        var requestArchive = KagemushaNoritoFrameWithPayload(0x4b);

        AssertArgumentDiagnostic(
            "changeOutput is required when publicAmount is less than current note amount",
            "hasChangeOutput",
            () => KagemushaRecursiveSpendNative.Redeem(
                requestArchive,
                publicAmount: "40",
                currentNoteAmount: "100",
                hasChangeOutput: false));

        AssertArgumentDiagnostic(
            "publicAmount must be less than current note amount when changeOutput is present",
            "publicAmount",
            () => KagemushaRecursiveSpendNative.Redeem(
                requestArchive,
                publicAmount: "100",
                currentNoteAmount: "100",
                hasChangeOutput: true));

        AssertArgumentDiagnostic(
            "publicAmount must not exceed current note amount",
            "publicAmount",
            () => KagemushaRecursiveSpendNative.Redeem(
                requestArchive,
                publicAmount: "101",
                currentNoteAmount: "100",
                hasChangeOutput: false));

        AssertArgumentDiagnostic(
            "publicAmount must be less than current note amount when changeOutput is present",
            "publicAmount",
            () => KagemushaRecursiveSpendNative.Redeem(
                requestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                publicAmount: "101",
                currentNoteAmount: "100",
                hasChangeOutput: true));

        AssertArgumentDiagnostic(
            "publicAmount must be canonical",
            "publicAmount",
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                publicAmount: "01",
                currentNoteAmount: "100",
                hasChangeOutput: true));

        AssertArgumentDiagnostic(
            "currentNoteAmount must fit in u128",
            "currentNoteAmount",
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                publicAmount: "1",
                currentNoteAmount: "340282366920938463463374607431768211456",
                hasChangeOutput: true));

        var nullPublicAmount = Assert.Throws<ArgumentNullException>(() =>
            KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                publicAmount: null,
                currentNoteAmount: "100",
                hasChangeOutput: true));
        Assert.Equal("publicAmount", nullPublicAmount.ParamName);

        var nullCurrentNoteAmount = Assert.Throws<ArgumentNullException>(() =>
            KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                publicAmount: "1",
                currentNoteAmount: null,
                hasChangeOutput: true));
        Assert.Equal("currentNoteAmount", nullCurrentNoteAmount.ParamName);

        AssertArgumentDiagnostic(
            "publicAmount must be a decimal integer",
            "publicAmount",
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                publicAmount: "",
                currentNoteAmount: "100",
                hasChangeOutput: true));

        AssertArgumentDiagnostic(
            "currentNoteAmount must be a decimal integer",
            "currentNoteAmount",
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                publicAmount: "1",
                currentNoteAmount: "100_000",
                hasChangeOutput: true));

        AssertArgumentDiagnostic(
            "publicAmount must be greater than zero",
            "publicAmount",
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                publicAmount: "0",
                currentNoteAmount: "100",
                hasChangeOutput: true));

        AssertArgumentDiagnostic(
            "currentNoteAmount must be greater than zero",
            "currentNoteAmount",
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                publicAmount: "1",
                currentNoteAmount: "0",
                hasChangeOutput: false));

        KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
            publicAmount: "40",
            currentNoteAmount: "100",
            hasChangeOutput: true);
        KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
            publicAmount: "100",
            currentNoteAmount: "100",
            hasChangeOutput: false);
        KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
            publicAmount: "340282366920938463463374607431768211455",
            currentNoteAmount: "340282366920938463463374607431768211455",
            hasChangeOutput: false);
        KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
            publicAmount: "340282366920938463463374607431768211454",
            currentNoteAmount: "340282366920938463463374607431768211455",
            hasChangeOutput: true);
    }

    [Fact]
    public void RecursiveSpendNativeRedeemChangeOutputPreflightRejectsInvalidFixed32BytesBeforeNativeBridge()
    {
        var malformedRequestArchive = new byte[] { 0xde, 0xad, 0xbe, 0xef };
        var shortChangeOutput = Enumerable.Repeat((byte)0x01, 31).ToArray();
        var zeroChangeOutput = new byte[32];
        var validChangeOutput = Enumerable.Repeat((byte)0x42, 32).ToArray();

        AssertArgumentDiagnostic(
            "changeOutput must be exactly 32 bytes",
            "changeOutput",
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputBytes(shortChangeOutput));

        AssertArgumentDiagnostic(
            "changeOutput must be non-zero",
            "changeOutput",
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputBytes(zeroChangeOutput));

        AssertArgumentDiagnostic(
            "changeOutput must be exactly 32 bytes",
            "changeOutput",
            () => KagemushaRecursiveSpendNative.Redeem(
                malformedRequestArchive,
                publicAmount: "40",
                currentNoteAmount: "100",
                changeOutput: shortChangeOutput));

        AssertArgumentDiagnostic(
            "changeOutput must be non-zero",
            "changeOutput",
            () => KagemushaRecursiveSpendNative.Redeem(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                publicAmount: "40",
                currentNoteAmount: "100",
                changeOutput: zeroChangeOutput));

        KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputBytes(validChangeOutput);

        AssertArgumentDiagnostic(
            "Request archive must be a valid Norito archive.",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Redeem(
                malformedRequestArchive,
                publicAmount: "40",
                currentNoteAmount: "100",
                changeOutput: validChangeOutput));
    }

    [Fact]
    public void RecursiveSpendNativeRedeemChangeOutputPreflightRejectsReservedMaterialBeforeNativeBridge()
    {
        var malformedRequestArchive = new byte[] { 0xde, 0xad, 0xbe, 0xef };
        var bundleSummary = KagemushaRecursiveSpendNative.DecodeBundleSummary(
            SharedRecursiveSpendArchive("init_bundle"));
        var reservedValues = new[]
        {
            bundleSummary.CurrentNote.NoteCommitment,
            bundleSummary.CurrentNote.SpendNullifier,
            bundleSummary.TopupAnchorNullifiers[0],
        };

        foreach (var reservedValue in reservedValues)
        {
            var directError = Assert.Throws<ArgumentException>(() =>
                KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputNotReserved(
                    reservedValue,
                    bundleSummary));
            Assert.Equal("changeOutput", directError.ParamName);
            Assert.Contains(
                "changeOutput must not reuse the current note commitment, redeem nullifier, or top-up anchor nullifier",
                directError.Message);

            var redeemError = Assert.Throws<ArgumentException>(() =>
                KagemushaRecursiveSpendNative.Redeem(
                    malformedRequestArchive,
                    publicAmount: "1",
                    bundleSummary,
                    reservedValue));
            Assert.Equal("changeOutput", redeemError.ParamName);
            Assert.Contains(
                "changeOutput must not reuse the current note commitment, redeem nullifier, or top-up anchor nullifier",
                redeemError.Message);
        }

        var nullSummary = Assert.Throws<ArgumentNullException>(() =>
            KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputNotReserved(
                NonReservedChangeOutput(bundleSummary),
                null!));
        Assert.Equal("bundleSummary", nullSummary.ParamName);

        var validChangeOutputThenArchiveValidation = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.Redeem(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
                bundleSummary.HopCount,
                hasLineageWitness: true,
                hasLineageVerifierRecord: true,
                publicAmount: "1",
                bundleSummary,
                NonReservedChangeOutput(bundleSummary)));
        Assert.Equal("requestArchive", validChangeOutputThenArchiveValidation.ParamName);
    }

    [Theory]
    [InlineData(" 40", "100", "publicAmount")]
    [InlineData("40 ", "100", "publicAmount")]
    [InlineData("+40", "100", "publicAmount")]
    [InlineData("-40", "100", "publicAmount")]
    [InlineData("40.0", "100", "publicAmount")]
    [InlineData("40", " 100", "currentNoteAmount")]
    [InlineData("40", "100 ", "currentNoteAmount")]
    [InlineData("40", "+100", "currentNoteAmount")]
    [InlineData("40", "-100", "currentNoteAmount")]
    [InlineData("40", "100.0", "currentNoteAmount")]
    public void RecursiveSpendNativeRedeemChangeOutputPreflightRejectsMalformedDecimalVectors(
        string publicAmount,
        string currentNoteAmount,
        string expectedParamName)
    {
        AssertArgumentDiagnostic(
            $"{expectedParamName} must be a decimal integer",
            expectedParamName,
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                publicAmount,
                currentNoteAmount,
                hasChangeOutput: true));
    }

    [Theory]
    [InlineData("", "must be a decimal integer")]
    [InlineData("0", "must be greater than zero")]
    [InlineData("00", "must be canonical")]
    [InlineData("01", "must be canonical")]
    [InlineData("0007", "must be canonical")]
    [InlineData("-1", "must be a decimal integer")]
    [InlineData("+1", "must be a decimal integer")]
    [InlineData("1.0", "must be a decimal integer")]
    [InlineData("1e3", "must be a decimal integer")]
    [InlineData("7 ", "must be a decimal integer")]
    [InlineData(" 7", "must be a decimal integer")]
    [InlineData("\t7", "must be a decimal integer")]
    [InlineData("7\n", "must be a decimal integer")]
    [InlineData("340282366920938463463374607431768211456", "must fit in u128")]
    [InlineData("9999999999999999999999999999999999999999", "must fit in u128")]
    public void RecursiveSpendNativeRedeemChangeOutputPreflightRejectsInvalidAmountVectorFamily(
        string amount,
        string expectedSuffix)
    {
        AssertArgumentDiagnostic(
            $"publicAmount {expectedSuffix}",
            "publicAmount",
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                amount,
                currentNoteAmount: "100",
                hasChangeOutput: true));

        AssertArgumentDiagnostic(
            $"currentNoteAmount {expectedSuffix}",
            "currentNoteAmount",
            () => KagemushaRecursiveSpendNative.ValidateRedeemChangeOutputPreflight(
                publicAmount: "1",
                currentNoteAmount: amount,
                hasChangeOutput: false));
    }

    [Fact]
    public void RecursiveCompactVerifierOutputRejectsInvalidNativeBoolean()
    {
        const string symbol = "connect_norito_kagemusha_verify_recursive_compact_payment_token";

        Assert.False(KagemushaRecursiveSpendNative.NormalizeRecursiveCompactVerifierOutput(
            symbol,
            0,
            0));
        Assert.True(KagemushaRecursiveSpendNative.NormalizeRecursiveCompactVerifierOutput(
            symbol,
            0,
            1));

        var invalidBoolean = Assert.Throws<InvalidOperationException>(() =>
            KagemushaRecursiveSpendNative.NormalizeRecursiveCompactVerifierOutput(symbol, 0, 2));
        Assert.Equal(
            "connect_norito_kagemusha_verify_recursive_compact_payment_token returned invalid boolean output 2.",
            invalidBoolean.Message);

        var bridgeError = Assert.Throws<InvalidOperationException>(() =>
            KagemushaRecursiveSpendNative.NormalizeRecursiveCompactVerifierOutput(symbol, -311, 0));
        Assert.Equal(
            "connect_norito_kagemusha_verify_recursive_compact_payment_token failed with bridge error code -311.",
            bridgeError.Message);

        var unavailable = Assert.Throws<InvalidOperationException>(() =>
            KagemushaRecursiveSpendNative.NormalizeRecursiveCompactVerifierOutput(
                symbol,
                KagemushaRecursiveSpendNative.RecursiveCompactUnavailableBridgeErrorCode,
                0));
        Assert.Equal(
            "connect_norito_kagemusha_verify_recursive_compact_payment_token is unavailable until ABI-7 recursive compact proof composition is enabled; bridge error code -312.",
            unavailable.Message);
    }

    [Fact]
    public void RecursiveSpendArchiveWrappersDefensivelyCopyNoritoBytes()
    {
        static void AssertDefensiveCopies(Func<byte[], KagemushaNativeArchive> factory)
        {
            var expected = KagemushaNoritoFrameWithPayload(0x4b);
            var source = KagemushaNoritoFrameWithPayload(0x4b);
            var archive = factory(source);
            source[0] = 0x7f;

            var firstRead = archive.NoritoBytes;
            Assert.Equal(expected, firstRead);
            firstRead[1] = 0x7f;
            Assert.Equal(expected, archive.NoritoBytes);
        }

        AssertDefensiveCopies(bytes => new KagemushaRecursiveSpendArchive(bytes));
        AssertDefensiveCopies(bytes => new KagemushaRecursiveSpendTransitionProfileArchive(bytes));
        AssertDefensiveCopies(bytes => new KagemushaRecursiveSpendLineageAppendBoundaryArchive(bytes));
        AssertDefensiveCopies(bytes => new KagemushaRecursiveSpendLineageWitnessArchive(bytes));
        AssertDefensiveCopies(bytes => new KagemushaRecursiveSpendVerifyArchive(bytes));
        AssertDefensiveCopies(bytes => new KagemushaRecursiveSpendRedeemInstructionArchive(bytes));
        AssertDefensiveCopies(bytes => new KagemushaCompactPaymentTokenArchive(bytes));
        AssertDefensiveCopies(bytes => new KagemushaRecursiveAggregationProofBundleArchive(bytes));
        AssertDefensiveCopies(bytes => new KagemushaRecursiveCompactPaymentTokenArchive(bytes));
        AssertDefensiveCopies(bytes => new KagemushaPallasOpenEnvelopesArchive(bytes));
        AssertDefensiveCopies(bytes => new KagemushaPreviousProofOpenEnvelopesArchive(bytes));
    }

    [Fact]
    public void RecursiveSpendArchiveWrappersRejectUnsafeNoritoBytes()
    {
        static void AssertRejectsUnsafeInputs(
            Func<byte[], KagemushaNativeArchive> factory,
            byte[] oversizedArchive)
        {
            var nullError = Assert.Throws<ArgumentNullException>(() => factory(null!));
            Assert.Equal("noritoBytes", nullError.ParamName);

            AssertArgumentDiagnostic(
                "Kagemusha Norito archive must not be empty.",
                "noritoBytes",
                () => factory(Array.Empty<byte>()));

            AssertArgumentDiagnostic(
                $"Kagemusha Norito archive must not exceed {KagemushaRecursiveSpendNative.NativeArchiveMaxBytes} bytes.",
                "noritoBytes",
                () => factory(oversizedArchive));

            AssertArgumentDiagnostic(
                "Kagemusha Norito archive must be a valid Norito V1 archive.",
                "noritoBytes",
                () => factory(new byte[] { 0x01 }));

            AssertArgumentDiagnostic(
                "Kagemusha Norito archive must contain a non-empty Norito payload.",
                "noritoBytes",
                () => factory(KagemushaNoritoFrame(0x4b)));

            var compressed = KagemushaNoritoFrameWithPayload(0x4b);
            compressed[22] = 1;
            AssertArgumentDiagnostic(
                "Kagemusha Norito archive must be a valid Norito V1 archive.",
                "noritoBytes",
                () => factory(compressed));

            var unsupportedFlags = KagemushaNoritoFrameWithPayload(0x4b);
            unsupportedFlags[39] = 0x08;
            AssertArgumentDiagnostic(
                "Kagemusha Norito archive must be a valid Norito V1 archive.",
                "noritoBytes",
                () => factory(unsupportedFlags));

            var invalidFieldBitset = KagemushaNoritoFrameWithPayload(0x4b);
            invalidFieldBitset[39] = 0x20;
            AssertArgumentDiagnostic(
                "Kagemusha Norito archive must be a valid Norito V1 archive.",
                "noritoBytes",
                () => factory(invalidFieldBitset));

            var nonZeroPadding = WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[] { 0x7f });
            AssertArgumentDiagnostic(
                "Kagemusha Norito archive must be a valid Norito V1 archive.",
                "noritoBytes",
                () => factory(nonZeroPadding));

            var excessivePadding = WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[65]);
            AssertArgumentDiagnostic(
                "Kagemusha Norito archive must be a valid Norito V1 archive.",
                "noritoBytes",
                () => factory(excessivePadding));
        }

        var oversizedArchive = new byte[KagemushaRecursiveSpendNative.NativeArchiveMaxBytes + 1];

        AssertRejectsUnsafeInputs(
            bytes => new KagemushaRecursiveSpendArchive(bytes),
            oversizedArchive);
        AssertRejectsUnsafeInputs(
            bytes => new KagemushaRecursiveSpendTransitionProfileArchive(bytes),
            oversizedArchive);
        AssertRejectsUnsafeInputs(
            bytes => new KagemushaRecursiveSpendLineageAppendBoundaryArchive(bytes),
            oversizedArchive);
        AssertRejectsUnsafeInputs(
            bytes => new KagemushaRecursiveSpendLineageWitnessArchive(bytes),
            oversizedArchive);
        AssertRejectsUnsafeInputs(
            bytes => new KagemushaRecursiveSpendVerifyArchive(bytes),
            oversizedArchive);
        AssertRejectsUnsafeInputs(
            bytes => new KagemushaRecursiveSpendRedeemInstructionArchive(bytes),
            oversizedArchive);
        AssertRejectsUnsafeInputs(
            bytes => new KagemushaCompactPaymentTokenArchive(bytes),
            oversizedArchive);
        AssertRejectsUnsafeInputs(
            bytes => new KagemushaRecursiveAggregationProofBundleArchive(bytes),
            oversizedArchive);
        AssertRejectsUnsafeInputs(
            bytes => new KagemushaRecursiveCompactPaymentTokenArchive(bytes),
            oversizedArchive);
        AssertRejectsUnsafeInputs(
            bytes => new KagemushaPallasOpenEnvelopesArchive(bytes),
            oversizedArchive);
        AssertRejectsUnsafeInputs(
            bytes => new KagemushaPreviousProofOpenEnvelopesArchive(bytes),
            oversizedArchive);
    }

    [Fact]
    public void RecursiveSpendSharedAbi6FixtureIsExplicitlyRejectedByFirstReleaseSurface()
    {
        using var manifest = LoadSharedRecursiveSpendManifest();
        var root = manifest.RootElement;

        Assert.Equal(
            "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1",
            root.GetProperty("schema").GetString());
        var fixtureAbiVersion = (uint)root.GetProperty("native_bridge_abi_version").GetInt32();
        Assert.Equal(6u, fixtureAbiVersion);
        Assert.NotEqual(KagemushaRecursiveSpendNative.RequiredNativeBridgeAbiVersion, fixtureAbiVersion);
        Assert.Equal(9, root.GetProperty("operation_count").GetInt32());

        var circuitIds = root.GetProperty("proof_circuit_ids");
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            circuitIds.GetProperty("recursive_aggregation").GetString());
        Assert.False(circuitIds.TryGetProperty("reserved_lineage", out _));
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
        Assert.Equal(2, KagemushaRecursiveSpendNative.FoldStepMaxInputs);
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
                "change_output",
                "lineage_verifier_record",
                "block_height",
                "lineage_verifier_records",
            },
            requestFieldsByType["KagemushaRecursiveSpendRedeemRequestV1"]);
        foreach (var (requestType, fields) in requestFieldRecordsByType)
        {
            foreach (var field in fields)
            {
                Assert.False(
                    field.GetProperty("norito_default").GetBoolean(),
                    $"{requestType}.{field.GetProperty("name").GetString()} must be encoded explicitly");
            }
        }
        var lineageVerifierRecords = requestFieldRecordsByType["KagemushaRecursiveSpendRedeemRequestV1"]
            .Single(field => field.GetProperty("name").GetString() == "lineage_verifier_records");
        Assert.Equal("Vec<VerifyingKeyRecord>", lineageVerifierRecords.GetProperty("type").GetString());
        Assert.False(lineageVerifierRecords.GetProperty("norito_default").GetBoolean());
        Assert.Equal(
            "additional_reserved_lineage_verifier_records",
            lineageVerifierRecords.GetProperty("semantics").GetString());
        foreach (var requestType in requestFieldsByType.Keys)
        {
            var blockHeight = requestFieldRecordsByType[requestType]
                .Single(field => field.GetProperty("name").GetString() == "block_height");
            Assert.Equal("Option<u64>", blockHeight.GetProperty("type").GetString());
            Assert.False(blockHeight.GetProperty("norito_default").GetBoolean());
            Assert.Equal(
                "verifier_record_activation_height",
                blockHeight.GetProperty("semantics").GetString());
        }
        Assert.Equal("redeem", redeemArchive.GetProperty("operation").GetString());
        Assert.Equal(
            "KagemushaRecursiveSpendRedeemRequestV1",
            redeemArchive.GetProperty("norito_type").GetString());
        Assert.Equal(
            "703128068fa36897c952640cb77006af29a8aa802d67da82c97e73c8e0ef1864",
            redeemArchive.GetProperty("sha256_hex").GetString());
        Assert.True(redeemArchive.GetProperty("byte_len").GetInt32() > 0);
        Assert.NotEmpty(Convert.FromBase64String(
            redeemArchive.GetProperty("bytes_base64").GetString()!));
        Assert.Equal("redeem", redeemInstructionArchive.GetProperty("operation").GetString());
        Assert.Equal(
            "RedeemKagemushaRecursive",
            redeemInstructionArchive.GetProperty("norito_type").GetString());
        Assert.Equal(
            "e05fb3ebb3a3e823f65403e09d1aa6e5deab0145f7aa0827f66a371ad633cc3e",
            redeemInstructionArchive.GetProperty("sha256_hex").GetString());
        Assert.True(redeemInstructionArchive.GetProperty("byte_len").GetInt32() > 0);
        Assert.NotEmpty(Convert.FromBase64String(
            redeemInstructionArchive.GetProperty("bytes_base64").GetString()!));

        Assert.Equal(
            circuitIds.GetProperty("recursive_aggregation").GetString(),
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(1u));
        Assert.Equal(
            circuitIds.GetProperty("recursive_aggregation").GetString(),
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(63u));
        Assert.Equal(
            circuitIds.GetProperty("recursive_aggregation").GetString(),
            KagemushaRecursiveSpendNative.PreferredAppendOutputCircuitId(64u));
        Assert.False(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(0u));
        Assert.False(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(63u));
        Assert.False(KagemushaRecursiveSpendNative.CanAppendWitnesslessLineage(64u));
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            2u));
        Assert.False(KagemushaRecursiveSpendNative.CanRedeemWitnessless(
            "kagemusha-recursive-spend-lineage-v1",
            65u));
    }

    [Fact]
    public void RecursiveSpendSharedAbi7FixturePinsCurrentProfileHashes()
    {
        using var archiveFixture = LoadSharedRecursiveSpendAbi7Archives();
        var root = archiveFixture.RootElement;
        Assert.Equal(
            "iroha.kagemusha.recursive_spend.abi7.archive_fixtures.v1",
            root.GetProperty("schema").GetString());

        var expectedArchives = new Dictionary<string, (string Operation, string NoritoType, int ByteLen, string Sha256Hex)>
        {
            ["append_bundle"] = (
                "append",
                "KagemushaRecursiveSpendBundleV1",
                13622,
                "42c7b1b0e2dc838a6660b3691e08474bb936fa001e446310930d387b00ba686b"),
            ["verify_request"] = (
                "verify",
                "KagemushaRecursiveSpendVerifyRequestV1",
                13628,
                "829be9daba04c4c928a34e1502ad2f1e467853ad2f02cdac0b6735f852fff44e"),
            ["verify_result"] = (
                "verify",
                "KagemushaRecursiveSpendVerifyResultV1",
                304,
                "67eb9b1f7c89bd842dbfb769bb802c60464fba510b4db0ac4c83bcfbd5626d15"),
            ["redeem_request"] = (
                "redeem",
                "KagemushaRecursiveSpendRedeemRequestV1",
                26275,
                "de704684a72f8e79264f62337327395c3ca426cbe26da57fc133aa97f4e240c0"),
            ["redeem_instruction"] = (
                "redeem",
                "RedeemKagemushaRecursive",
                26262,
                "bcd7306e54db93a09ffb013860adaae223655205d77651201cb49dd3a5d5d980"),
        };

        var seenArchives = new HashSet<string>();
        foreach (var archive in root.GetProperty("archives").EnumerateArray())
        {
            var name = archive.GetProperty("name").GetString()!;
            Assert.True(expectedArchives.TryGetValue(name, out var expected), $"unexpected ABI-7 archive {name}");
            seenArchives.Add(name);
            Assert.Equal(expected.Operation, archive.GetProperty("operation").GetString());
            Assert.Equal(expected.NoritoType, archive.GetProperty("norito_type").GetString());
            Assert.Equal(expected.ByteLen, archive.GetProperty("byte_len").GetInt32());
            Assert.Equal(expected.Sha256Hex, archive.GetProperty("sha256_hex").GetString());

            var bytes = Convert.FromBase64String(archive.GetProperty("bytes_base64").GetString()!);
            Assert.Equal(expected.ByteLen, bytes.Length);
            Assert.Equal(
                expected.Sha256Hex,
                Convert.ToHexString(SHA256.HashData(bytes)).ToLowerInvariant());
        }

        Assert.True(seenArchives.SetEquals(expectedArchives.Keys));
    }

    [Fact]
    public void RecursiveSpendBundleSummaryDecoderReadsSharedBundleArchives()
    {
        var initBundleArchive = SharedRecursiveSpendArchive("init_bundle");
        var initBundle = KagemushaRecursiveSpendNative.DecodeBundleSummary(initBundleArchive);

        Assert.Equal(1u, initBundle.HopCount);
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            initBundle.ProofCircuitId);
        Assert.StartsWith("hex:", initBundle.Asset);
        Assert.False(string.IsNullOrWhiteSpace(initBundle.ChainId));
        Assert.Equal(32, initBundle.InitialRoot.Length);
        Assert.Equal(32, initBundle.FinalRoot.Length);
        Assert.Equal(KagemushaRecursiveSpendNative.FoldStepMaxInputs, initBundle.TopupAnchorNullifiers.Count);
        foreach (var topupAnchorNullifier in initBundle.TopupAnchorNullifiers)
        {
            Assert.Equal(32, topupAnchorNullifier.Length);
            Assert.False(topupAnchorNullifier.All(value => value == 0));
        }
        Assert.Equal(32, initBundle.CurrentNote.NoteCommitment.Length);
        Assert.Equal(32, initBundle.CurrentNote.SpendNullifier.Length);
        Assert.Equal("7", initBundle.CurrentNote.Amount);

        var initialRoot = initBundle.InitialRoot;
        var initialRootByte = initialRoot[0];
        initialRoot[0] ^= 0xff;
        Assert.Equal(initialRootByte, initBundle.InitialRoot[0]);
        var noteCommitment = initBundle.CurrentNote.NoteCommitment;
        var noteCommitmentByte = noteCommitment[0];
        noteCommitment[0] ^= 0xff;
        Assert.Equal(noteCommitmentByte, initBundle.CurrentNote.NoteCommitment[0]);
        var copiedTopupAnchorNullifier = initBundle.TopupAnchorNullifiers[0];
        var copiedTopupAnchorNullifierByte = copiedTopupAnchorNullifier[0];
        copiedTopupAnchorNullifier[0] ^= 0xff;
        Assert.Equal(copiedTopupAnchorNullifierByte, initBundle.TopupAnchorNullifiers[0][0]);

        var appendBundle = KagemushaRecursiveSpendNative.DecodeBundleSummary(
            SharedRecursiveSpendArchive("append_bundle"));
        Assert.True(appendBundle.HopCount >= 1);
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            appendBundle.ProofCircuitId));
        Assert.Equal(32, appendBundle.InitialRoot.Length);
        Assert.Equal(32, appendBundle.FinalRoot.Length);
    }

    [Fact]
    public void RecursiveSpendTransitionProfileSummaryDecoderReadsSharedArchives()
    {
        var initProfile = KagemushaRecursiveSpendNative.DecodeTransitionProfileSummary(
            SharedRecursiveSpendArchive("transition_profile_init"));

        Assert.Equal(0u, initProfile.HopIndex);
        Assert.Equal(1u, initProfile.HopCount);
        Assert.False(initProfile.HasPriorState);
        Assert.Empty(initProfile.PreviousTopupAnchorNullifiers);
        Assert.Equal(
            KagemushaRecursiveSpendNative.FoldStepMaxOutputs,
            initProfile.CurrentHopOutputCommitments.Count);
        foreach (var outputCommitment in initProfile.CurrentHopOutputCommitments)
        {
            Assert.Equal(32, outputCommitment.Length);
            Assert.False(outputCommitment.All(value => value == 0));
        }

        var appendProfile = KagemushaRecursiveSpendNative.DecodeTransitionProfileSummary(
            SharedRecursiveSpendArchive("transition_profile_append"));

        Assert.Equal(1u, appendProfile.HopIndex);
        Assert.Equal(2u, appendProfile.HopCount);
        Assert.True(appendProfile.HasPriorState);
        Assert.Equal(
            KagemushaRecursiveSpendNative.FoldStepMaxInputs,
            appendProfile.PreviousTopupAnchorNullifiers.Count);
        Assert.Equal(
            KagemushaRecursiveSpendNative.FoldStepMaxOutputs,
            appendProfile.CurrentHopOutputCommitments.Count);
        foreach (var previousTopupAnchorNullifier in appendProfile.PreviousTopupAnchorNullifiers)
        {
            Assert.Equal(32, previousTopupAnchorNullifier.Length);
            Assert.False(previousTopupAnchorNullifier.All(value => value == 0));
        }

        var copiedPreviousTopupAnchorNullifier = appendProfile.PreviousTopupAnchorNullifiers[0];
        var copiedPreviousTopupAnchorNullifierByte = copiedPreviousTopupAnchorNullifier[0];
        copiedPreviousTopupAnchorNullifier[0] ^= 0xff;
        Assert.Equal(
            copiedPreviousTopupAnchorNullifierByte,
            appendProfile.PreviousTopupAnchorNullifiers[0][0]);
        var copiedOutputCommitment = appendProfile.CurrentHopOutputCommitments[0];
        var copiedOutputCommitmentByte = copiedOutputCommitment[0];
        copiedOutputCommitment[0] ^= 0xff;
        Assert.Equal(copiedOutputCommitmentByte, appendProfile.CurrentHopOutputCommitments[0][0]);
    }

    [Fact]
    public void RecursiveSpendTransitionProfileSummaryDecoderRejectsPreviousTopupAnchorDrift()
    {
        var initProfileArchive = SharedRecursiveSpendArchive("transition_profile_init");
        var appendProfileArchive = SharedRecursiveSpendArchive("transition_profile_append");
        var appendProfile = KagemushaRecursiveSpendNative.DecodeTransitionProfileSummary(appendProfileArchive);
        var carriedPreviousTopupAnchor = appendProfile.PreviousTopupAnchorNullifiers[0];
        var nonReservedOutput = NonReservedTransitionProfileOutput(appendProfile);

        foreach (var malformedPreviousAnchors in new[]
        {
            (
                Archive: RecursiveSpendTransitionProfileWithField(
                    appendProfileArchive,
                    7,
                    TopupAnchorNullifierCountPayload(0)),
                ExpectedField: "transition_profile.previous_topup_anchor_nullifiers count is out of range"
            ),
            (
                Archive: RecursiveSpendTransitionProfileWithField(
                    appendProfileArchive,
                    7,
                    TopupAnchorNullifiersPayload(new byte[32])),
                ExpectedField: "transition_profile.previous_topup_anchor_nullifiers must not contain zero values"
            ),
            (
                Archive: RecursiveSpendTransitionProfileWithField(
                    appendProfileArchive,
                    7,
                    TopupAnchorNullifiersPayload(Fixed32(0x34), Fixed32(0x34))),
                ExpectedField: "transition_profile.previous_topup_anchor_nullifiers must be strictly sorted and unique"
            ),
            (
                Archive: RecursiveSpendTransitionProfileWithField(
                    appendProfileArchive,
                    7,
                    TopupAnchorNullifiersPayload(Fixed32(0x35), Fixed32(0x34))),
                ExpectedField: "transition_profile.previous_topup_anchor_nullifiers must be strictly sorted and unique"
            ),
            (
                Archive: RecursiveSpendTransitionProfileWithField(
                    initProfileArchive,
                    7,
                    TopupAnchorNullifiersPayload(Fixed32(0x34))),
                ExpectedField: "transition_profile.previous_topup_anchor_nullifiers count is out of range"
            ),
            (
                Archive: RecursiveSpendTransitionProfileWithCurrentHopOutputCommitments(
                    appendProfileArchive,
                    SortedFixed32(carriedPreviousTopupAnchor, nonReservedOutput)),
                ExpectedField: "transition_profile.output_commitments must not reuse previous top-up anchor nullifiers"
            ),
            (
                Archive: RecursiveSpendTransitionProfileWithCurrentNoteField(
                    appendProfileArchive,
                    0,
                    carriedPreviousTopupAnchor),
                ExpectedField: "transition_profile.previous_topup_anchor_nullifiers must not reuse current note material"
            ),
            (
                Archive: RecursiveSpendTransitionProfileWithCurrentNoteField(
                    appendProfileArchive,
                    1,
                    carriedPreviousTopupAnchor),
                ExpectedField: "transition_profile.previous_topup_anchor_nullifiers must not reuse current note material"
            ),
        })
        {
            AssertTransitionProfileSummaryRejects(
                malformedPreviousAnchors.Archive,
                malformedPreviousAnchors.ExpectedField);
        }
    }

    [Fact]
    public void RecursiveSpendVerifyResultDecoderReadsSharedArchivesAndRejectsTrailingFields()
    {
        var abi6Result = KagemushaRecursiveSpendNative.DecodeVerifyResult(
            SharedRecursiveSpendArchive("verify_result"));
        Assert.False(abi6Result.Valid);
        Assert.Equal(2u, abi6Result.HopCount);
        Assert.Equal(4011u, abi6Result.EncodedBytes);
        Assert.False(abi6Result.ChainAdmissible);
        Assert.True(abi6Result.LineageWitnessRequiredForRedeem);

        var abi7Result = KagemushaRecursiveSpendNative.DecodeVerifyResult(
            SharedRecursiveSpendAbi7Archive("verify_result"));
        Assert.True(abi7Result.Valid);
        Assert.Equal(1u, abi7Result.HopCount);
        Assert.Equal(13622u, abi7Result.EncodedBytes);
        Assert.True(abi7Result.LineageWitnessRequiredForRedeem);

        AssertArgumentDiagnostic(
            "verifyResult has trailing bytes",
            "bundleArchive",
            () => KagemushaRecursiveSpendNative.DecodeVerifyResult(
                RecursiveSpendVerifyResultWithTrailingField()));
    }

    [Fact]
    public void RecursiveSpendLineageWitnessDecoderRejectsTrailingFields()
    {
        var validLineageWitness = (
            Archive: SharedRecursiveSpendArchive("lineage_witness_append_result"),
            ExpectedReservedPreviousProof: true);
        Assert.Equal(
            validLineageWitness.ExpectedReservedPreviousProof,
            KagemushaRecursiveSpendNative.LineageWitnessHasReservedPreviousProof(
                validLineageWitness.Archive));

        foreach (var malformedWitness in new[]
        {
            (
                Archive: RecursiveSpendLineageWitnessWithTrailingField(),
                ExpectedField: "lineageWitness has trailing bytes"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithTrailingPreviousProofsField(),
                ExpectedField: "lineageWitness.previousRecursiveProofs"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithOverLimitPreviousProofCountOnly(),
                ExpectedField: "lineageWitness.previousRecursiveProofs count must not exceed"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithPreviousProofCountPrefixOnly(
                    (ulong)KagemushaRecursiveSpendNative.CompactTokenMaxHops + 1UL),
                ExpectedField:
                    $"lineageWitness.previousRecursiveProofs count must not exceed {KagemushaRecursiveSpendNative.CompactTokenMaxHops}"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithTrailingPreviousProofField(),
                ExpectedField: "lineageWitness.previousRecursiveProofs"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithTrailingPreviousVerifierKeyIdField(),
                ExpectedField: "lineageWitness.previousRecursiveProofs.verifierKeyId"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithPreviousProofField(
                    1,
                    Array.Empty<byte>()),
                ExpectedField: "lineageWitness.previousRecursiveProofs.proof_public_inputs empty recursive proof inputs"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithPreviousProofField(
                    2,
                    new byte[32]),
                ExpectedField: "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash must be non-zero"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithPreviousProofField(
                    2,
                    Enumerable.Repeat((byte)0x7f, 32).ToArray()),
                ExpectedField: "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash mismatch"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithPreviousProofField(
                    2,
                    KagemushaFixedArrayPayload(0x44, 31)),
                ExpectedField: "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash must be exactly 32 bytes"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithPreviousProofField(
                    2,
                    KagemushaCountPrefixedFixedArrayPayload(0x44, 32)),
                ExpectedField: "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash byte field length must be 1"
            ),
            (
                Archive: RecursiveSpendLineageWitnessWithPreviousProofField(
                    2,
                    KagemushaFixedArrayPayload(0x44, 33)),
                ExpectedField: "lineageWitness.previousRecursiveProofs.proof_public_inputs_hash must be exactly 32 bytes"
            ),
        })
        {
            AssertArgumentDiagnostic(
                malformedWitness.ExpectedField,
                "bundleArchive",
                () => KagemushaRecursiveSpendNative.LineageWitnessHasReservedPreviousProof(
                    malformedWitness.Archive));
        }
    }

    [Fact]
    public void RecursiveSpendBundleSummaryDecoderRejectsAdversarialProofAttachmentAndHopCount()
    {
        var initBundleArchive = SharedRecursiveSpendArchive("init_bundle");

        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithPayloadTextReplaced(
                initBundleArchive,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
                "kagemusha-recursive-spend-lineage-badhop-v1"),
            "bundle.proof_circuit_id unsupported recursive proof circuit id: kagemusha-recursive-spend-lineage-badhop-v1");

        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithPayloadTextReplaced(
                initBundleArchive,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                "halo2/kzg"),
            "bundle.proof_backend unsupported recursive proof backend: halo2/kzg");

        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithEmptyProofBytes(initBundleArchive),
            "bundle.proof_bytes empty");

        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithRecursiveProofField(
                initBundleArchive,
                1,
                Array.Empty<byte>()),
            "bundle.proof_public_inputs empty");

        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithRecursiveProofField(
                initBundleArchive,
                2,
                new byte[32]),
            "bundle.proof_public_inputs_hash must be non-zero");

        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithRecursiveProofField(
                initBundleArchive,
                2,
                Enumerable.Repeat((byte)0x7f, 32).ToArray()),
            "bundle.proof_public_inputs_hash mismatch");

        foreach (var malformedProofPublicInputsHash in new[]
        {
            (
                Replacement: KagemushaFixedArrayPayload(0x44, 31),
                ExpectedField: "bundle.proof_public_inputs_hash must be exactly 32 bytes"
            ),
            (
                Replacement: KagemushaCountPrefixedFixedArrayPayload(0x44, 32),
                ExpectedField: "bundle.proof_public_inputs_hash byte field length must be 1"
            ),
            (
                Replacement: KagemushaFixedArrayPayload(0x44, 33),
                ExpectedField: "bundle.proof_public_inputs_hash must be exactly 32 bytes"
            ),
        })
        {
            AssertBundleSummaryRejects(
                RecursiveSpendBundleWithRecursiveProofField(
                    initBundleArchive,
                    2,
                    malformedProofPublicInputsHash.Replacement),
                malformedProofPublicInputsHash.ExpectedField);
        }

        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithAccumulatorField(
                initBundleArchive,
                0,
                KagemushaNoritoString(
                    "iroha:kagemusha:v1:recursive-spend-accumulator-digest")),
            "bundle.accumulator.domain expected iroha:kagemusha:v1:recursive-spend-accumulator");

        foreach (var malformedAccumulatorField in new[]
        {
            (FieldIndex: 2, Replacement: KagemushaFixedArrayPayload(0x01, 15), ExpectedField: "asset"),
            (FieldIndex: 2, Replacement: KagemushaFixedArrayPayload(0x01, 17), ExpectedField: "asset"),
            (FieldIndex: 3, Replacement: KagemushaFixedArrayPayload(0x02, 31), ExpectedField: "initialRoot"),
            (FieldIndex: 3, Replacement: KagemushaFixedArrayPayload(0x02, 33), ExpectedField: "initialRoot"),
            (FieldIndex: 4, Replacement: KagemushaFixedArrayPayload(0x03, 31), ExpectedField: "finalRoot"),
            (FieldIndex: 4, Replacement: KagemushaFixedArrayPayload(0x03, 33), ExpectedField: "finalRoot"),
            (FieldIndex: 5, Replacement: TopupAnchorNullifierCountPayload(0), ExpectedField: "bundle.accumulator.topup_anchor_nullifiers count is out of range"),
            (FieldIndex: 5, Replacement: TopupAnchorNullifierCountPayload((ulong)KagemushaRecursiveSpendNative.FoldStepMaxInputs + 1), ExpectedField: "bundle.accumulator.topup_anchor_nullifiers count is out of range"),
            (FieldIndex: 5, Replacement: TopupAnchorNullifiersPayload(new byte[32]), ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must not contain zero values"),
            (FieldIndex: 5, Replacement: TopupAnchorNullifiersPayload(Fixed32(0x34), Fixed32(0x34)), ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique"),
            (FieldIndex: 5, Replacement: TopupAnchorNullifiersPayload(Fixed32(0x35), Fixed32(0x34)), ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique"),
            (FieldIndex: 5, Replacement: TopupAnchorNullifiersPayload(KagemushaRecursiveSpendNative.DecodeBundleSummary(initBundleArchive).CurrentNote.NoteCommitment), ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material"),
            (FieldIndex: 5, Replacement: TopupAnchorNullifiersPayload(KagemushaRecursiveSpendNative.DecodeBundleSummary(initBundleArchive).CurrentNote.SpendNullifier), ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material"),
        })
        {
            AssertBundleSummaryRejects(
                RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    malformedAccumulatorField.FieldIndex,
                    malformedAccumulatorField.Replacement),
                malformedAccumulatorField.ExpectedField);
        }

        var bundleSummary = KagemushaRecursiveSpendNative.DecodeBundleSummary(initBundleArchive);
        foreach (var topupAnchorPrecedence in new[]
        {
            (
                Label: "malformed proof cannot mask invalid top-up anchor nullifiers",
                Archive: RecursiveSpendBundleWithEmptyProofBytes(
                    RecursiveSpendBundleWithAccumulatorField(
                        initBundleArchive,
                        5,
                        TopupAnchorNullifiersPayload(new byte[32]))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must not contain zero values"
            ),
            (
                Label: "trailing accumulator cannot mask invalid top-up anchor nullifiers",
                Archive: RecursiveSpendBundleWithAccumulatorTrailingField(
                    RecursiveSpendBundleWithAccumulatorField(
                        initBundleArchive,
                        5,
                        TopupAnchorNullifiersPayload(new byte[32]))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must not contain zero values"
            ),
            (
                Label: "malformed proof cannot mask current-note top-up anchor reuse",
                Archive: RecursiveSpendBundleWithEmptyProofBytes(
                    RecursiveSpendBundleWithAccumulatorField(
                        initBundleArchive,
                        5,
                        TopupAnchorNullifiersPayload(bundleSummary.CurrentNote.NoteCommitment))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material"
            ),
            (
                Label: "trailing accumulator cannot mask current-note top-up anchor reuse",
                Archive: RecursiveSpendBundleWithAccumulatorTrailingField(
                    RecursiveSpendBundleWithAccumulatorField(
                        initBundleArchive,
                        5,
                        TopupAnchorNullifiersPayload(bundleSummary.CurrentNote.NoteCommitment))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material"
            ),
            (
                Label: "malformed proof cannot mask duplicate top-up anchors",
                Archive: RecursiveSpendBundleWithEmptyProofBytes(
                    RecursiveSpendBundleWithAccumulatorField(
                        initBundleArchive,
                        5,
                        TopupAnchorNullifiersPayload(Fixed32(0x34), Fixed32(0x34)))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique"
            ),
            (
                Label: "trailing accumulator cannot mask descending top-up anchors",
                Archive: RecursiveSpendBundleWithAccumulatorTrailingField(
                    RecursiveSpendBundleWithAccumulatorField(
                        initBundleArchive,
                        5,
                        TopupAnchorNullifiersPayload(Fixed32(0x35), Fixed32(0x34)))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique"
            ),
        })
        {
            var error = Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.DecodeBundleSummary(topupAnchorPrecedence.Archive));
            Assert.Contains(topupAnchorPrecedence.ExpectedField, error.Message);
            Assert.False(string.IsNullOrWhiteSpace(topupAnchorPrecedence.Label));
        }

        foreach (var malformedHopCount in new[]
        {
            0u,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageWitnesslessMaxHopsV1 + 1u,
        })
        {
            var hopCountPayload = new byte[4];
            BinaryPrimitives.WriteUInt32LittleEndian(hopCountPayload, malformedHopCount);
            AssertBundleSummaryRejects(
                RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    6,
                    hopCountPayload),
                $"bundle.accumulator.hop_count must be in 1..{KagemushaRecursiveSpendNative.RecursiveSpendLineageWitnesslessMaxHopsV1}");
        }

        foreach (var malformedCurrentNote in new[]
        {
            (
                Archive: RecursiveSpendBundleWithCurrentNoteField(
                    initBundleArchive,
                    0,
                    new byte[32]),
                ExpectedField: "bundle.current_note.note_commitment must not be all-zero"
            ),
            (
                Archive: RecursiveSpendBundleWithCurrentNoteField(
                    initBundleArchive,
                    1,
                    new byte[32]),
                ExpectedField: "bundle.current_note.spend_nullifier must not be all-zero"
            ),
            (
                Archive: RecursiveSpendBundleWithEqualCurrentNoteNullifier(initBundleArchive),
                ExpectedField: "bundle.current_note note commitment and spend nullifier must differ"
            ),
            (
                Archive: RecursiveSpendBundleWithCurrentNoteField(
                    initBundleArchive,
                    2,
                    KagemushaNumericAmountPayload(0)),
                ExpectedField: "bundle.current_note.amount must fit in u128"
            ),
            (
                Archive: RecursiveSpendBundleWithCurrentNoteField(
                    initBundleArchive,
                    0,
                    KagemushaFixedArrayPayload(0x04, 31)),
                ExpectedField: "noteCommitment"
            ),
            (
                Archive: RecursiveSpendBundleWithCurrentNoteField(
                    initBundleArchive,
                    0,
                    KagemushaFixedArrayPayload(0x04, 33)),
                ExpectedField: "noteCommitment"
            ),
            (
                Archive: RecursiveSpendBundleWithCurrentNoteField(
                    initBundleArchive,
                    1,
                    KagemushaFixedArrayPayload(0x05, 31)),
                ExpectedField: "spendNullifier"
            ),
            (
                Archive: RecursiveSpendBundleWithCurrentNoteField(
                    initBundleArchive,
                    1,
                    KagemushaFixedArrayPayload(0x05, 33)),
                ExpectedField: "spendNullifier"
            ),
        })
        {
            AssertBundleSummaryRejects(
                malformedCurrentNote.Archive,
                malformedCurrentNote.ExpectedField);
        }

        AssertBundleSummaryRejects(
            RebuildKagemushaNoritoFrameLike(
                initBundleArchive,
                KagemushaNoritoPayload(initBundleArchive),
                flags: 0),
            "bundle must use compact Norito layout");
    }

    [Fact]
    public void RecursiveSpendBundleSummaryDecoderRejectsCanonicalShapeDriftAndTrailingFields()
    {
        var initBundleArchive = SharedRecursiveSpendArchive("init_bundle");

        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithAccumulatorField(
                initBundleArchive,
                1,
                KagemushaNoritoString("prod-chain-id")),
            "chainId");

        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithProofBoxField(
                initBundleArchive,
                0,
                KagemushaNoritoString("halo2/kzg")),
            "bundle.proof_backend unsupported recursive proof backend: halo2/kzg");

        foreach (var malformedBundle in new[]
        {
            (
                Archive: RecursiveSpendBundleWithTopLevelTrailingField(initBundleArchive),
                ExpectedField: "bundle has trailing bytes"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorTrailingField(initBundleArchive),
                ExpectedField: "accumulator has trailing bytes"
            ),
            (
                Archive: RecursiveSpendBundleWithCurrentNoteTrailingField(initBundleArchive),
                ExpectedField: "currentNote has trailing bytes"
            ),
            (
                Archive: RecursiveSpendBundleWithCurrentNoteAmountTrailingField(initBundleArchive),
                ExpectedField: "bundle.current_note.amount"
            ),
            (
                Archive: RecursiveSpendBundleWithRecursiveProofTrailingField(initBundleArchive),
                ExpectedField: "recursiveProof has trailing bytes"
            ),
            (
                Archive: RecursiveSpendBundleWithVerifierKeyIdTrailingField(initBundleArchive),
                ExpectedField: "verifierKeyId has trailing bytes"
            ),
            (
                Archive: RecursiveSpendBundleWithProofBoxTrailingField(initBundleArchive),
                ExpectedField: "proof has trailing bytes"
            ),
        })
        {
            AssertBundleSummaryRejects(malformedBundle.Archive, malformedBundle.ExpectedField);
        }
    }

    [Fact]
    public void RecursiveSpendNativeRejectsEmptyArchivesBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundle = RecordBundleWithStepCount();

        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            Array.Empty<byte>(),
            "must not be empty.");
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Init(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Append(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.TransitionProfileInit(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.TransitionProfileAppend(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageAppendBoundary(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
            Array.Empty<byte>(),
            validArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
            validArchive,
            Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            Array.Empty<byte>(),
            validArchive,
            validArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            validArchive,
            Array.Empty<byte>(),
            validArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            validArchive,
            validArchive,
            Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Verify(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Redeem(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    Array.Empty<byte>(),
                    validArchive));
        Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.BuildPallasOpenEnvelopesArchive(Array.Empty<byte>()));
        Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.BuildPreviousProofOpenEnvelopesArchive(Array.Empty<byte>()));
    }

    [Fact]
    public void CompactTokenProverRejectsMalformedInputsBeforeLoadingNativeBridge()
    {
        AssertArgumentDiagnostic(
            "Record bundle archive must be a valid Norito archive.",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(
                new byte[] { 0x01, 0x02 }));
    }

    [Fact]
    public void CompactTokenProverRejectsOversizedInputsBeforeLoadingNativeBridge()
    {
        var oversizedArchive = OversizedKagemushaArchive();
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(
                oversizedArchive),
            "Record bundle archive must not exceed",
            "recordBundleArchive");
    }

    [Fact]
    public void CompactTokenProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge()
    {
        AssertArgumentDiagnostic(
            "Record bundle archive must contain a non-empty Norito payload.",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(
                KagemushaNoritoFrame(0x4b)));
    }

    [Fact]
    public void RecursiveAggregationProverRejectsMalformedInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundle = RecordBundleWithStepCount();
        var validPallasOpenEnvelopesArchive = PallasOpenEnvelopesArchive();
        var recordBundle = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    new byte[] { 0x01, 0x02 },
                    validPallasOpenEnvelopesArchive));

        AssertArgumentDiagnostic(
            "Pallas open-envelopes archive must be a valid Norito archive.",
            "pallasOpenEnvelopesArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    new byte[] { 0x01, 0x02 }));
    }

    [Fact]
    public void RecursiveAggregationProverRejectsOversizedInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundle = RecordBundleWithStepCount();
        var validPallasOpenEnvelopesArchive = PallasOpenEnvelopesArchive();
        var oversizedArchive = OversizedKagemushaArchive();
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    oversizedArchive,
                    validPallasOpenEnvelopesArchive),
            "Record bundle archive must not exceed",
            "recordBundleArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    oversizedArchive),
            "Pallas open-envelopes archive must not exceed",
            "pallasOpenEnvelopesArchive");
    }

    [Fact]
    public void RecursiveAggregationProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundle = RecordBundleWithStepCount();
        var validPallasOpenEnvelopesArchive = PallasOpenEnvelopesArchive();
        var emptyPayloadArchive = KagemushaNoritoFrame(0x4b);
        AssertArgumentDiagnostic(
            "Record bundle archive must contain a non-empty Norito payload.",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    emptyPayloadArchive,
                    validPallasOpenEnvelopesArchive));

        AssertArgumentDiagnostic(
            "Pallas open-envelopes archive must contain a non-empty Norito payload.",
            "pallasOpenEnvelopesArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    emptyPayloadArchive));
    }

    [Fact]
    public void RecursiveSpendPallasOpenEnvelopePreflightRejectsMalformedVectorsBeforeLoadingNativeBridge()
    {
        var validRecordBundle = RecordBundleWithStepCount();
        var validKeyArtifacts = KagemushaNoritoFrameWithPayload(0x4b);
        var malformedArchives = new (byte[] Archive, string ExpectedMessage)[]
        {
            (
                KagemushaNoritoFrameFromSchemaHash(
                    NoritoCodec.SchemaHash("test.PallasOpenEnvelopes"),
                    new byte[] { 0x72 },
                    KagemushaNoritoCompactLenFlag),
                "pallasOpenEnvelopes must be a valid Vec<iroha_zkp_halo2::OpenVerifyEnvelope> Norito archive"
            ),
            (PallasOpenEnvelopesArchive(0), "pallasOpenEnvelopesArchive requires exactly 1 envelope(s)"),
            (PallasOpenEnvelopesArchive(2), "pallasOpenEnvelopesArchive requires exactly 1 envelope(s)"),
            (
                PallasOpenEnvelopesArchive(configure: spec => spec.IncludeDomainTag = false),
                "pallasOpenEnvelopesArchive[0].domain_tag is required"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec => spec.ParamsGSequencePayload = U64LE(5)),
                "pallasOpenEnvelopesArchive[0].params.g length must equal params.n"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec => spec.ParamsHSequencePayload = U64LE(5)),
                "pallasOpenEnvelopesArchive[0].params.h length must equal params.n"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec => spec.ProofLSequencePayload = U64LE(3)),
                "pallasOpenEnvelopesArchive[0].proof round count mismatch: expected 2, found count prefix"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec => spec.ProofRSequencePayload = U64LE(3)),
                "pallasOpenEnvelopesArchive[0].proof round count mismatch: expected 2, found count prefix"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.VkCommitmentPayload = KagemushaFixedArrayPayload(0x70, 32)),
                "pallasOpenEnvelopesArchive[0].vk_commitment must be exactly 32 bytes"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.VkCommitmentOptionPayload = OptionRawWithTrailingByte(SyntheticFixed32(0x70))),
                "pallasOpenEnvelopesArchive[0].vk_commitment"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.VkCommitmentOptionPayload = OptionRawWithUnknownTag()),
                "pallasOpenEnvelopesArchive[0].vk_commitment option tag must be 0 or 1"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.VkCommitmentOptionPayload = OptionRawWithDeclaredLengthTooLong(SyntheticFixed32(0x70))),
                "pallasOpenEnvelopesArchive[0].vk_commitment payload length mismatch"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.PublicInputsSchemaHashPayload = KagemushaFixedArrayPayload(0x71, 32)),
                "pallasOpenEnvelopesArchive[0].public_inputs_schema_hash must be exactly 32 bytes"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.PublicInputsSchemaHashOptionPayload = OptionRawWithTrailingByte(SyntheticFixed32(0x71))),
                "pallasOpenEnvelopesArchive[0].public_inputs_schema_hash"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.PublicInputsSchemaHashOptionPayload = OptionRawWithUnknownTag()),
                "pallasOpenEnvelopesArchive[0].public_inputs_schema_hash option tag must be 0 or 1"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.PublicInputsSchemaHashOptionPayload =
                        OptionRawWithDeclaredLengthTooLong(SyntheticFixed32(0x71))),
                "pallasOpenEnvelopesArchive[0].public_inputs_schema_hash payload length mismatch"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.DomainTagPayload = KagemushaFixedArrayPayload(0x72, 32)),
                "pallasOpenEnvelopesArchive[0].domain_tag must be exactly 32 bytes"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.DomainTagOptionPayload = OptionRawWithTrailingByte(SyntheticFixed32(0x72))),
                "pallasOpenEnvelopesArchive[0].domain_tag"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.DomainTagOptionPayload = OptionRawWithUnknownTag()),
                "pallasOpenEnvelopesArchive[0].domain_tag option tag must be 0 or 1"
            ),
            (
                PallasOpenEnvelopesArchive(configure: spec =>
                    spec.DomainTagOptionPayload = OptionRawWithDeclaredLengthTooLong(SyntheticFixed32(0x72))),
                "pallasOpenEnvelopesArchive[0].domain_tag payload length mismatch"
            ),
        };

        foreach (var (archive, expectedMessage) in malformedArchives)
        {
            AssertPallasArchiveRejected(
                () => KagemushaRecursiveSpendNative
                    .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                        validRecordBundle,
                        archive),
                expectedMessage);
            AssertPallasArchiveRejected(
                () => KagemushaRecursiveSpendNative
                    .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                        validRecordBundle,
                        archive,
                        validKeyArtifacts),
                expectedMessage);
        }

        var countMismatch = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    RecordBundleWithStepCount(2),
                    PallasOpenEnvelopesArchive()));
        Assert.Equal("pallasOpenEnvelopesArchive", countMismatch.ParamName);
        Assert.Contains("pallasOpenEnvelopes requires exactly 2 envelope(s)", countMismatch.Message);
    }

    [Fact]
    public void RecursiveSpendInitRequestEncoderRejectsMalformedLineageAndPallasInputsBeforeNativeBridge()
    {
        var recordBundle = RecordBundleWithStepCount();
        var pallasOpenEnvelopes = PallasOpenEnvelopesArchive();
        var currentNote = ValidSpendableNoteDescriptor();
        var initVerifierKey = KagemushaLineageVerifierKey(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            0xd1);
        var initProvingKey = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            initVerifierKey,
            0xd2);
        var appendVerifierKey = KagemushaLineageVerifierKey(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            0xd3);
        var appendProvingKey = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            appendVerifierKey,
            0xd4);
        var initArtifacts = KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
            2,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            initVerifierKey,
            initProvingKey);
        var appendArtifacts = KagemushaRecursiveSpendNative.LineageKeyArtifactsForAppend(
            2,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            appendVerifierKey,
            appendProvingKey);

        var request = KagemushaRecursiveSpendNative.EncodeInitRequest(
            recordBundle,
            pallasOpenEnvelopes,
            currentNote,
            initArtifacts,
            blockHeight: 42);
        Assert.Equal(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendInitRequestWireName),
            request.AsSpan(6, 16).ToArray());
        Assert.Equal(KagemushaNoritoCompactLenFlag, request[39]);
        var fields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(request),
            KagemushaNoritoCompactLenFlag);
        Assert.Equal(6, fields.Count);
        Assert.Equal(0x01, fields[3][0]);
        Assert.Equal(0x01, fields[4][0]);
        Assert.Equal(0x01, fields[5][0]);

        var semanticRequest = KagemushaRecursiveSpendNative.EncodeInitRequest(
            recordBundle,
            pallasOpenEnvelopes,
            currentNote,
            blockHeight: 43);
        var semanticFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(semanticRequest),
            KagemushaNoritoCompactLenFlag);
        Assert.Equal(6, semanticFields.Count);
        Assert.Equal(0x00, semanticFields[3][0]);
        Assert.Equal(0x00, semanticFields[4][0]);
        Assert.Equal(0x01, semanticFields[5][0]);

        var wrongArtifactProfile = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeInitRequest(
                recordBundle,
                pallasOpenEnvelopes,
                currentNote,
                appendArtifacts));
        Assert.Equal("lineageKeyArtifacts", wrongArtifactProfile.ParamName);
        Assert.Contains("lineage_key_artifacts must be init artifacts", wrongArtifactProfile.Message);

        Assert.Contains(
            "lineage_verifier_key",
            Assert.Throws<ArgumentException>(() =>
                KagemushaRecursiveSpendNative.EncodeInitRequestWithLineageMaterials(
                    recordBundle,
                    pallasOpenEnvelopes,
                    currentNote,
                    appendVerifierKey,
                    appendProvingKey)).Message);

        var pallasMismatch = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeInitRequest(
                recordBundle,
                PallasOpenEnvelopesArchive(2),
                currentNote,
                initArtifacts));
        Assert.Equal("pallasOpenEnvelopesArchive", pallasMismatch.ParamName);
        Assert.Contains("pallasOpenEnvelopesArchive requires exactly 1 envelope(s)", pallasMismatch.Message);

        Assert.Throws<ArgumentNullException>(() =>
            KagemushaRecursiveSpendNative.EncodeInitRequest(
                recordBundle,
                pallasOpenEnvelopes,
                null!,
                initArtifacts));
    }

    [Fact]
    public void RecursiveSpendAppendRequestEncoderFailsClosedForReservedOutputAndRejectsMisplacedLineageMaterial()
    {
        var previousBundle = SharedRecursiveSpendAbi7Archive("append_bundle");
        var recordBundle = RecordBundleWithStepCount();
        var pallasOpenEnvelopes = PallasOpenEnvelopesArchive();
        var previousProofOpenEnvelopes = PallasOpenEnvelopesArchive();
        var previousLineageRecord = VerifyingKeyRecordArchive();
        var currentNote = ValidSpendableNoteDescriptor();
        var appendVerifierKey = KagemushaLineageVerifierKey(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            0xd5);
        var appendProvingKey = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            appendVerifierKey,
            0xd6);
        var initVerifierKey = KagemushaLineageVerifierKey(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            0xd7);
        var initProvingKey = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            initVerifierKey,
            0xd8);
        var appendArtifacts = KagemushaRecursiveSpendNative.LineageKeyArtifactsForAppend(
            2,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            appendVerifierKey,
            appendProvingKey);
        var initArtifacts = KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
            2,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            initVerifierKey,
            initProvingKey);

        var reservedOutput = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequest(
                previousBundle,
                recordBundle,
                pallasOpenEnvelopes,
                currentNote,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                previousLineageRecord,
                previousProofOpenEnvelopes,
                appendArtifacts));
        Assert.Equal("outputProofCircuitId", reservedOutput.ParamName);
        Assert.Contains("outputProofCircuitId is not valid for the previous bundle", reservedOutput.Message);

        var request = KagemushaRecursiveSpendNative.EncodeAppendRequest(
            previousBundle,
            recordBundle,
            pallasOpenEnvelopes,
            currentNote,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            blockHeight: 43);
        Assert.Equal(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendAppendRequestWireName),
            request.AsSpan(6, 16).ToArray());
        Assert.Equal(KagemushaNoritoCompactLenFlag, request[39]);
        var fields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(request),
            KagemushaNoritoCompactLenFlag);
        Assert.Equal(10, fields.Count);
        Assert.Equal(0x00, fields[5][0]);
        Assert.Equal(0x00, fields[6][0]);
        Assert.Equal(0x00, fields[7][0]);
        Assert.Equal(0x00, fields[8][0]);
        Assert.Equal(0x01, fields[9][0]);

        var danglingPreviousRecord = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequest(
                previousBundle,
                recordBundle,
                pallasOpenEnvelopes,
                currentNote,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                previousLineageRecord,
                previousProofOpenEnvelopesArchive: null,
                lineageKeyArtifacts: null));
        Assert.Equal("previousLineageVerifierRecordArchive", danglingPreviousRecord.ParamName);
        Assert.Contains("previousLineageVerifierRecordArchive is only valid", danglingPreviousRecord.Message);

        var danglingPreviousOpenings = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequest(
                previousBundle,
                recordBundle,
                pallasOpenEnvelopes,
                currentNote,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                previousLineageVerifierRecordArchive: null,
                previousProofOpenEnvelopesArchive: previousProofOpenEnvelopes,
                lineageKeyArtifacts: null));
        Assert.Equal("previousProofOpenEnvelopesArchive", danglingPreviousOpenings.ParamName);
        Assert.Contains("previousProofOpenEnvelopesArchive is only valid", danglingPreviousOpenings.Message);

        var wrongAppendArtifact = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequest(
                previousBundle,
                recordBundle,
                pallasOpenEnvelopes,
                currentNote,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                previousLineageVerifierRecordArchive: null,
                previousProofOpenEnvelopesArchive: null,
                lineageKeyArtifacts: initArtifacts));
        Assert.Equal("lineageKeyArtifacts", wrongAppendArtifact.ParamName);
        Assert.Contains("lineage_key_artifacts must be append artifacts", wrongAppendArtifact.Message);

        var danglingLineageKeyMaterial = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequestWithLineageMaterials(
                previousBundle,
                recordBundle,
                pallasOpenEnvelopes,
                currentNote,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                previousLineageVerifierRecordArchive: null,
                previousProofOpenEnvelopesArchive: null,
                lineageVerifierKey: appendVerifierKey,
                lineageProvingKeyArchive: appendProvingKey));
        Assert.Equal("lineageVerifierKey", danglingLineageKeyMaterial.ParamName);
        Assert.Contains("lineageKeyArtifacts are only valid for lineage append output", danglingLineageKeyMaterial.Message);
    }

    [Fact]
    public void RecursiveSpendGeneratedPallasInitRequestHelperRejectsLineageBeforeNativeBuilder()
    {
        var malformedRecordBundle = new byte[] { 0x01, 0x02 };
        var currentNote = ValidSpendableNoteDescriptor();
        var initVerifierKey = KagemushaLineageVerifierKey(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            0xd9);
        var initProvingKey = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            initVerifierKey,
            0xda);
        var appendVerifierKey = KagemushaLineageVerifierKey(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            0xdb);
        var appendProvingKey = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            appendVerifierKey,
            0xdc);
        var initArtifacts = KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
            2,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            initVerifierKey,
            initProvingKey);
        var appendArtifacts = KagemushaRecursiveSpendNative.LineageKeyArtifactsForAppend(
            2,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            appendVerifierKey,
            appendProvingKey);

        var wrongProfile = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeInitRequestWithGeneratedPallas(
                malformedRecordBundle,
                currentNote,
                appendArtifacts));
        Assert.Equal("lineageKeyArtifacts", wrongProfile.ParamName);
        Assert.Contains("lineage_key_artifacts must be init artifacts", wrongProfile.Message);

        Assert.Contains(
            "lineage_verifier_key",
            Assert.Throws<ArgumentException>(() =>
                KagemushaRecursiveSpendNative.EncodeInitRequestWithGeneratedPallas(
                    malformedRecordBundle,
                    currentNote,
                    appendVerifierKey,
                    appendProvingKey)).Message);

        var nullNote = Assert.Throws<ArgumentNullException>(() =>
            KagemushaRecursiveSpendNative.EncodeInitRequestWithGeneratedPallas(
                malformedRecordBundle,
                null!,
                initArtifacts));
        Assert.Equal("currentNote", nullNote.ParamName);
    }

    [Fact]
    public void RecursiveSpendGeneratedPallasAppendRequestHelperRejectsLineageBeforeNativeBuilder()
    {
        var previousBundle = SharedRecursiveSpendAbi7Archive("append_bundle");
        var malformedRecordBundle = new byte[] { 0x01, 0x02 };
        var previousLineageRecord = VerifyingKeyRecordArchive();
        var currentNote = ValidSpendableNoteDescriptor();
        var initVerifierKey = KagemushaLineageVerifierKey(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            0xdd);
        var initProvingKey = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            initVerifierKey,
            0xde);
        var appendVerifierKey = KagemushaLineageVerifierKey(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            0xdf);
        var appendProvingKey = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            appendVerifierKey,
            0xe0);
        var initArtifacts = KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
            2,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            initVerifierKey,
            initProvingKey);
        var appendArtifacts = KagemushaRecursiveSpendNative.LineageKeyArtifactsForAppend(
            2,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
            appendVerifierKey,
            appendProvingKey);

        var reservedOutput = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequestWithGeneratedPallas(
                previousBundle,
                malformedRecordBundle,
                currentNote,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                previousLineageVerifierRecordArchive: null,
                lineageKeyArtifacts: appendArtifacts));
        Assert.Equal("outputProofCircuitId", reservedOutput.ParamName);
        Assert.Contains("outputProofCircuitId is not valid for the previous bundle", reservedOutput.Message);

        var wrongProfile = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequestWithGeneratedPallas(
                previousBundle,
                malformedRecordBundle,
                currentNote,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                previousLineageRecord,
                initArtifacts));
        Assert.Equal("lineageKeyArtifacts", wrongProfile.ParamName);
        Assert.Contains("lineage_key_artifacts must be append artifacts", wrongProfile.Message);

        var danglingPreviousRecord = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequestWithGeneratedPallas(
                previousBundle,
                malformedRecordBundle,
                currentNote,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                previousLineageVerifierRecordArchive: new byte[] { 0x01, 0x02 },
                lineageKeyArtifacts: null));
        Assert.Equal("previousLineageVerifierRecordArchive", danglingPreviousRecord.ParamName);
        Assert.Contains("previousLineageVerifierRecordArchive is only valid", danglingPreviousRecord.Message);

        var danglingLineageKeyMaterial = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequestWithGeneratedPallas(
                previousBundle,
                malformedRecordBundle,
                currentNote,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                previousLineageVerifierRecordArchive: null,
                lineageVerifierKey: appendVerifierKey,
                lineageProvingKeyArchive: appendProvingKey));
        Assert.Equal("lineageVerifierKey", danglingLineageKeyMaterial.ParamName);
        Assert.Contains("lineageKeyArtifacts are only valid for lineage append output", danglingLineageKeyMaterial.Message);

        var malformedRecordBundleError = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequestWithGeneratedPallas(
                previousBundle,
                malformedRecordBundle,
                currentNote,
                KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                lineageKeyArtifacts: null));
        Assert.Equal("recordBundleArchive", malformedRecordBundleError.ParamName);
        Assert.Contains("must be a valid Norito archive", malformedRecordBundleError.Message);

        var nullNote = Assert.Throws<ArgumentNullException>(() =>
            KagemushaRecursiveSpendNative.EncodeAppendRequestWithGeneratedPallas(
                previousBundle,
                malformedRecordBundle,
                null!,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
                previousLineageVerifierRecordArchive: previousLineageRecord,
                lineageKeyArtifacts: appendArtifacts));
        Assert.Equal("currentNote", nullNote.ParamName);
    }

    [Fact]
    public void PallasOpenEnvelopeBuildersRejectMalformedInputsBeforeLoadingNativeBridge()
    {
        AssertArgumentDiagnostic(
            "Record bundle archive must be a valid Norito archive.",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative.BuildPallasOpenEnvelopesArchive(new byte[] { 0x01, 0x02 }));

        AssertArgumentDiagnostic(
            "Previous recursive proof bundle archive must be a valid Norito archive.",
            "previousBundleArchive",
            () => KagemushaRecursiveSpendNative.BuildPreviousProofOpenEnvelopesArchive(new byte[] { 0x01, 0x02 }));
    }

    [Fact]
    public void PallasOpenEnvelopeBuildersRejectOversizedInputsBeforeLoadingNativeBridge()
    {
        var oversizedArchive = OversizedKagemushaArchive();
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.BuildPallasOpenEnvelopesArchive(oversizedArchive),
            "Record bundle archive must not exceed",
            "recordBundleArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.BuildPreviousProofOpenEnvelopesArchive(oversizedArchive),
            "Previous recursive proof bundle archive must not exceed",
            "previousBundleArchive");
    }

    [Fact]
    public void PallasOpenEnvelopeBuildersRejectEmptyPayloadInputsBeforeLoadingNativeBridge()
    {
        var emptyPayloadArchive = KagemushaNoritoFrame(0x4b);
        AssertArgumentDiagnostic(
            "Record bundle archive must contain a non-empty Norito payload.",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative.BuildPallasOpenEnvelopesArchive(emptyPayloadArchive));

        AssertArgumentDiagnostic(
            "Previous recursive proof bundle archive must contain a non-empty Norito payload.",
            "previousBundleArchive",
            () => KagemushaRecursiveSpendNative.BuildPreviousProofOpenEnvelopesArchive(emptyPayloadArchive));
    }

    [Fact]
    public void RecursiveSpendNativeInitAppendRejectOverLimitRecordBundleStepCountBeforeNativeBridge()
    {
        var recordBundlePayload = KagemushaRecordBundlePayloadWithStepsPayload(
            KagemushaUInt64Payload((ulong)KagemushaRecursiveSpendNative.CompactTokenMaxHops + 1UL));

        AssertRecordBundleStepCountPreflightRejects(
            () => KagemushaRecursiveSpendNative.Init(
                KagemushaInitRequestArchiveWithRecordBundle(recordBundlePayload)),
            "requestArchive");
        AssertRecordBundleStepCountPreflightRejects(
            () => KagemushaRecursiveSpendNative.TransitionProfileInit(
                KagemushaInitRequestArchiveWithRecordBundle(recordBundlePayload)),
            "requestArchive");
        AssertRecordBundleStepCountPreflightRejects(
            () => KagemushaRecursiveSpendNative.Append(
                KagemushaAppendRequestArchiveWithRecordBundle(recordBundlePayload)),
            "requestArchive");
        AssertRecordBundleStepCountPreflightRejects(
            () => KagemushaRecursiveSpendNative.TransitionProfileAppend(
                KagemushaAppendRequestArchiveWithRecordBundle(recordBundlePayload)),
            "requestArchive");
    }

    [Fact]
    public void RecursiveSpendNativeRecordBundleSurfacesRejectOverLimitStepCountBeforeNativeBridge()
    {
        var recordBundleArchive = KagemushaRecordBundleArchiveWithStepsPayload(
            KagemushaUInt64Payload((ulong)KagemushaRecursiveSpendNative.CompactTokenMaxHops + 1UL));
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);

        AssertRecordBundleStepCountPreflightRejects(
            () => KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(
                recordBundleArchive),
            "recordBundleArchive");
        AssertRecordBundleStepCountPreflightRejects(
            () => KagemushaRecursiveSpendNative.BuildPallasOpenEnvelopesArchive(
                recordBundleArchive),
            "recordBundleArchive");
        AssertRecordBundleStepCountPreflightRejects(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive,
                    validArchive),
            "recordBundleArchive");
        AssertRecordBundleStepCountPreflightRejects(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive,
                    validArchive,
                    validArchive),
            "recordBundleArchive");
    }

    [Fact]
    public void RecursiveSpendNativeInitAppendRejectPallasEnvelopeCountMismatchBeforeNativeBridge()
    {
        var recordBundlePayload = KagemushaRecordBundlePayloadWithStepCount(1);
        var mismatchedPallasOpenEnvelopesArchive = KagemushaPallasOpenEnvelopesArchiveWithCount(0);

        AssertPallasEnvelopeCountPreflightRejects(
            () => KagemushaRecursiveSpendNative.Init(
                KagemushaInitRequestArchiveWithRecordBundleAndPallas(
                    recordBundlePayload,
                    mismatchedPallasOpenEnvelopesArchive)),
            "requestArchive");
        AssertPallasEnvelopeCountPreflightRejects(
            () => KagemushaRecursiveSpendNative.TransitionProfileInit(
                KagemushaInitRequestArchiveWithRecordBundleAndPallas(
                    recordBundlePayload,
                    mismatchedPallasOpenEnvelopesArchive)),
            "requestArchive");
        AssertPallasEnvelopeCountPreflightRejects(
            () => KagemushaRecursiveSpendNative.Append(
                KagemushaAppendRequestArchiveWithRecordBundleAndPallas(
                    recordBundlePayload,
                    mismatchedPallasOpenEnvelopesArchive)),
            "requestArchive");
        AssertPallasEnvelopeCountPreflightRejects(
            () => KagemushaRecursiveSpendNative.TransitionProfileAppend(
                KagemushaAppendRequestArchiveWithRecordBundleAndPallas(
                    recordBundlePayload,
                    mismatchedPallasOpenEnvelopesArchive)),
            "requestArchive");
    }

    [Fact]
    public void RecursiveSpendNativeAppendRejectsInvalidOutputSelectionAndMisplacedLineageMaterialBeforeNativeBridge()
    {
        var aggregationPreviousBundleArchive = SharedRecursiveSpendAbi7Archive("append_bundle");
        var aggregationPreviousBundle = KagemushaRecursiveSpendNative.DecodeBundleSummary(
            aggregationPreviousBundleArchive);
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            aggregationPreviousBundle.ProofCircuitId);

        AssertArgumentDiagnostic(
            "outputProofCircuitId is not valid for the previous bundle",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Append(
                KagemushaFullAppendRequestArchive(
                    aggregationPreviousBundleArchive,
                    KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1)));
        AssertArgumentDiagnostic(
            "outputProofCircuitId is not valid for the previous bundle",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.TransitionProfileAppend(
                KagemushaFullAppendRequestArchive(
                    aggregationPreviousBundleArchive,
                    KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1)));

        AssertArgumentDiagnostic(
            "previousLineageVerifierRecord is only valid for lineage previous bundles",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Append(
                KagemushaFullAppendRequestArchive(
                    aggregationPreviousBundleArchive,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                    previousLineageVerifierRecordPayload: KagemushaFixed32(0x77))));
        AssertArgumentDiagnostic(
            "previousLineageVerifierRecord is only valid for lineage previous bundles",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.TransitionProfileAppend(
                KagemushaFullAppendRequestArchive(
                    aggregationPreviousBundleArchive,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                    previousLineageVerifierRecordPayload: KagemushaFixed32(0x77))));

        AssertArgumentDiagnostic(
            "previousProofOpenEnvelopes are only valid for lineage append output",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Append(
                KagemushaFullAppendRequestArchive(
                    aggregationPreviousBundleArchive,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                    previousProofOpenEnvelopesArchive: KagemushaPallasOpenEnvelopesArchiveWithCount(1))));
        AssertArgumentDiagnostic(
            "previousProofOpenEnvelopes are only valid for lineage append output",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.TransitionProfileAppend(
                KagemushaFullAppendRequestArchive(
                    aggregationPreviousBundleArchive,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                    previousProofOpenEnvelopesArchive: KagemushaPallasOpenEnvelopesArchiveWithCount(1))));

        AssertArgumentDiagnostic(
            "lineageKeyArtifacts are only valid for lineage append output",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Append(
                KagemushaFullAppendRequestArchive(
                    aggregationPreviousBundleArchive,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                    lineageVerifierKeyPayload: KagemushaFixed32(0x78))));
        AssertArgumentDiagnostic(
            "lineageKeyArtifacts are only valid for lineage append output",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.TransitionProfileAppend(
                KagemushaFullAppendRequestArchive(
                    aggregationPreviousBundleArchive,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
                    lineageProvingKeyArchivePayload: KagemushaFixed32(0x79))));
    }

    [Fact]
    public void RecursiveSpendNativeRecordBackedProversRejectPallasEnvelopeCountMismatchBeforeNativeBridge()
    {
        var recordBundleArchive = KagemushaRecordBundleArchiveWithStepCount(1);
        var mismatchedPallasOpenEnvelopesArchive = KagemushaPallasOpenEnvelopesArchiveWithCount(0);
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);

        AssertPallasEnvelopeCountPreflightRejects(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive,
                    mismatchedPallasOpenEnvelopesArchive),
            "pallasOpenEnvelopesArchive");
        AssertPallasEnvelopeCountPreflightRejects(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundleArchive,
                    mismatchedPallasOpenEnvelopesArchive,
                    validArchive),
            "pallasOpenEnvelopesArchive");
    }

    [Fact]
    public void RecursiveSpendNativeInitAppendRejectPallasInnerEnvelopeShapeBeforeNativeBridge()
    {
        var recordBundlePayload = KagemushaRecordBundlePayloadWithStepCount(1);
        var malformedPallasOpenEnvelopesArchives = new[]
        {
            (
                Archive: KagemushaPallasOpenEnvelopesArchiveWithEnvelope(
                    KagemushaPallasOpenEnvelopePayload(domainTag: KagemushaPallasMetadataOption(null))),
                Message: "pallasOpenEnvelopes[0].domain_tag is required"
            ),
            (
                Archive: KagemushaPallasOpenEnvelopesArchiveWithEnvelope(
                    KagemushaPallasOpenEnvelopePayload(domainTag: KagemushaFixed32(0x7f))),
                Message: "pallasOpenEnvelopes[0].domain_tag option tag must be 0 or 1"
            ),
            (
                Archive: KagemushaPallasOpenEnvelopesArchiveWithEnvelope(
                    KagemushaPallasOpenEnvelopePayload(
                        paramsPayload: KagemushaPallasIpaParamsPayload(
                            gPayload: KagemushaUInt64Payload(3)))),
                Message: "pallasOpenEnvelopes[0].params.g length must equal params.n"
            ),
        };

        foreach (var (malformedPallasOpenEnvelopesArchive, message) in malformedPallasOpenEnvelopesArchives)
        {
            AssertPallasInnerEnvelopePreflightRejects(
                () => KagemushaRecursiveSpendNative.Init(
                    KagemushaInitRequestArchiveWithRecordBundleAndPallas(
                        recordBundlePayload,
                        malformedPallasOpenEnvelopesArchive)),
                "requestArchive",
                message);
            AssertPallasInnerEnvelopePreflightRejects(
                () => KagemushaRecursiveSpendNative.TransitionProfileInit(
                    KagemushaInitRequestArchiveWithRecordBundleAndPallas(
                        recordBundlePayload,
                        malformedPallasOpenEnvelopesArchive)),
                "requestArchive",
                message);
            AssertPallasInnerEnvelopePreflightRejects(
                () => KagemushaRecursiveSpendNative.Append(
                    KagemushaAppendRequestArchiveWithRecordBundleAndPallas(
                        recordBundlePayload,
                        malformedPallasOpenEnvelopesArchive)),
                "requestArchive",
                message);
            AssertPallasInnerEnvelopePreflightRejects(
                () => KagemushaRecursiveSpendNative.TransitionProfileAppend(
                    KagemushaAppendRequestArchiveWithRecordBundleAndPallas(
                        recordBundlePayload,
                        malformedPallasOpenEnvelopesArchive)),
                "requestArchive",
                message);
        }
    }

    [Fact]
    public void RecursiveSpendNativeRecordBackedProversRejectPallasInnerEnvelopeShapeBeforeNativeBridge()
    {
        var recordBundleArchive = KagemushaRecordBundleArchiveWithStepCount(1);
        var cases = new[]
        {
            (
                Archive: KagemushaPallasOpenEnvelopesArchiveWithEnvelope(
                    KagemushaPallasOpenEnvelopePayload(domainTag: KagemushaPallasMetadataOption(null))),
                Message: "pallasOpenEnvelopes[0].domain_tag is required"
            ),
            (
                Archive: KagemushaPallasOpenEnvelopesArchiveWithEnvelope(
                    KagemushaPallasOpenEnvelopePayload(domainTag: KagemushaFixed32(0x7f))),
                Message: "pallasOpenEnvelopes[0].domain_tag option tag must be 0 or 1"
            ),
            (
                Archive: KagemushaPallasOpenEnvelopesArchiveWithEnvelope(
                    KagemushaPallasOpenEnvelopePayload(domainTag: new byte[] { 1 }
                        .Concat(KagemushaNoritoField(KagemushaFixed32(0x7f)))
                        .Concat(new byte[] { 0x99 })
                        .ToArray())),
                Message: "pallasOpenEnvelopes[0].domain_tag payload length mismatch"
            ),
            (
                Archive: KagemushaPallasOpenEnvelopesArchiveWithEnvelope(
                    KagemushaPallasOpenEnvelopePayload(domainTag: new byte[] { 1 }
                        .Concat(KagemushaNoritoLength(33))
                        .Concat(KagemushaFixed32(0x7f))
                        .ToArray())),
                Message: "pallasOpenEnvelopes[0].domain_tag payload length mismatch"
            ),
            (
                Archive: KagemushaPallasOpenEnvelopesArchiveWithEnvelope(
                    KagemushaPallasOpenEnvelopePayload(domainTag: new byte[] { 2 })),
                Message: "pallasOpenEnvelopes[0].domain_tag option tag must be 0 or 1"
            ),
            (
                Archive: KagemushaPallasOpenEnvelopesArchiveWithEnvelope(
                    KagemushaPallasOpenEnvelopePayload(
                        paramsPayload: KagemushaPallasIpaParamsPayload(
                            gPayload: KagemushaUInt64Payload(3)))),
                Message: "pallasOpenEnvelopes[0].params.g length must equal params.n"
            ),
            (
                Archive: KagemushaPallasOpenEnvelopesArchiveWithEnvelope(
                    KagemushaPallasOpenEnvelopePayload(
                        paramsPayload: KagemushaPallasIpaParamsPayload(n: 2),
                        publicPayload: KagemushaPallasPolyOpenPublicPayload(n: 2),
                        proofPayload: KagemushaPallasIpaProofPayload(
                            lPayload: KagemushaUInt64Payload(2)))),
                Message: "pallasOpenEnvelopes[0].proof round count mismatch: expected 1, found count prefix"
            ),
        };

        foreach (var (archive, message) in cases)
        {
            AssertPallasInnerEnvelopePreflightRejects(
                () => KagemushaRecursiveSpendNative
                    .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                        recordBundleArchive,
                        archive),
                "pallasOpenEnvelopesArchive",
                message);
        }
    }

    [Fact]
    public void RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundle = RecordBundleWithStepCount();
        var validPallasOpenEnvelopes = PallasOpenEnvelopesArchive();
        AssertArgumentDiagnostic(
            "Record bundle archive must be a valid Norito archive.",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    new byte[] { 0x01, 0x02 },
                    validPallasOpenEnvelopes,
                    validArchive));

        AssertArgumentDiagnostic(
            "Pallas open-envelopes archive must be a valid Norito archive.",
            "pallasOpenEnvelopesArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    new byte[] { 0x01, 0x02 },
                    validArchive));

        AssertArgumentDiagnostic(
            "Recursive compact key artifacts archive must be a valid Norito archive.",
            "recursiveCompactKeyArtifactsArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    validPallasOpenEnvelopes,
                    new byte[] { 0x01, 0x02 }));
    }

    [Fact]
    public void RecursiveCompactProverRejectsOversizedInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundle = RecordBundleWithStepCount();
        var validPallasOpenEnvelopes = PallasOpenEnvelopesArchive();
        var oversizedArchive = OversizedKagemushaArchive();
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    oversizedArchive,
                    validPallasOpenEnvelopes,
                    validArchive),
            "Record bundle archive must not exceed",
            "recordBundleArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    oversizedArchive,
                    validArchive),
            "Pallas open-envelopes archive must not exceed",
            "pallasOpenEnvelopesArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    validPallasOpenEnvelopes,
                    oversizedArchive),
            "Recursive compact key artifacts archive must not exceed",
            "recursiveCompactKeyArtifactsArchive");
    }

    [Fact]
    public void RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundle = RecordBundleWithStepCount();
        var validPallasOpenEnvelopes = PallasOpenEnvelopesArchive();
        var emptyPayloadArchive = KagemushaNoritoFrame(0x4b);
        AssertArgumentDiagnostic(
            "Record bundle archive must contain a non-empty Norito payload.",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    emptyPayloadArchive,
                    validPallasOpenEnvelopes,
                    validArchive));

        AssertArgumentDiagnostic(
            "Pallas open-envelopes archive must contain a non-empty Norito payload.",
            "pallasOpenEnvelopesArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    emptyPayloadArchive,
                    validArchive));

        AssertArgumentDiagnostic(
            "Recursive compact key artifacts archive must contain a non-empty Norito payload.",
            "recursiveCompactKeyArtifactsArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    validPallasOpenEnvelopes,
                    emptyPayloadArchive));
    }

    [Fact]
    public void RecursiveSpendCompactProjectionRejectsInvalidBundleBeforeLoadingNativeBridge()
    {
        AssertArgumentDiagnostic(
            "Recursive spend bundle archive must be a valid Norito archive.",
            "bundleArchive",
            () => KagemushaRecursiveSpendNative
                .RecursiveSpendCompactPaymentTokenFromBundle(new byte[] { 0x01, 0x02 }));

        AssertArgumentDiagnostic(
            "Recursive spend bundle archive must contain a non-empty Norito payload.",
            "bundleArchive",
            () => KagemushaRecursiveSpendNative
                .RecursiveSpendCompactPaymentTokenFromBundle(KagemushaNoritoFrame(0x4c)));

        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .RecursiveSpendCompactPaymentTokenFromBundle(OversizedKagemushaArchive()),
            "Recursive spend bundle archive must not exceed",
            "bundleArchive");
    }

    [Fact]
    public void RecursiveSpendCompactProjectionVerifierRejectsInvalidInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);

        AssertArgumentDiagnostic(
            "Compact token archive must not be empty.",
            "compactTokenArchive",
            () => KagemushaRecursiveSpendNative
                .VerifyRecursiveSpendCompactPaymentTokenProjection(Array.Empty<byte>(), validArchive));

        AssertArgumentDiagnostic(
            "Compact token archive must be a valid Norito archive.",
            "compactTokenArchive",
            () => KagemushaRecursiveSpendNative
                .VerifyRecursiveSpendCompactPaymentTokenProjection(new byte[] { 0x01, 0x02 }, validArchive));

        AssertArgumentDiagnostic(
            "Verifier record archive must be a valid Norito archive.",
            "verifierRecordArchive",
            () => KagemushaRecursiveSpendNative
                .VerifyRecursiveSpendCompactPaymentTokenProjection(validArchive, new byte[] { 0x01, 0x02 }));

        AssertArgumentDiagnostic(
            "Verifier record archive must contain a non-empty Norito payload.",
            "verifierRecordArchive",
            () => KagemushaRecursiveSpendNative
                .VerifyRecursiveSpendCompactPaymentTokenProjection(validArchive, KagemushaNoritoFrame(0x4b)));

        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .VerifyRecursiveSpendCompactPaymentTokenProjection(validArchive, OversizedKagemushaArchive()),
            "Verifier record archive must not exceed",
            "verifierRecordArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .VerifyRecursiveSpendCompactPaymentTokenProjection(OversizedKagemushaArchive(), validArchive),
            "Compact token archive must not exceed",
            "compactTokenArchive");
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

        Assert.Equal(
            "connect_norito_kagemusha_recursive_spend_redeem failed with bridge error code -311.",
            error.Message);
    }

    [Fact]
    public void RecursiveSpendNativeReadBridgeOutputClearsAndFreesNonNullPointerOnBridgeErrors()
    {
        var errorBytes = Encoding.UTF8.GetBytes("kagemusha-native-error-output-never-survives-free");
        var pointer = Marshal.AllocHGlobal(errorBytes.Length);
        var freed = false;
        Marshal.Copy(errorBytes, 0, pointer, errorBytes.Length);

        try
        {
            var error = Assert.Throws<InvalidOperationException>(() =>
                KagemushaRecursiveSpendNative.ReadBridgeOutput(
                    "connect_norito_kagemusha_recursive_spend_redeem",
                    -311,
                    pointer,
                    (UIntPtr)errorBytes.Length,
                    ptr =>
                    {
                        Assert.Equal(pointer, ptr);
                        AssertPointerZeroed(ptr, errorBytes.Length);
                        Marshal.FreeHGlobal(ptr);
                        pointer = IntPtr.Zero;
                        freed = true;
                    }));

            Assert.True(freed);
            Assert.Contains("-311", error.Message);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    [Fact]
    public void RecursiveSpendNativeReadBridgeOutputReportsRecursiveCompactUnavailable()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            KagemushaRecursiveSpendNative.ReadBridgeOutput(
                "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes",
                KagemushaRecursiveSpendNative.RecursiveCompactUnavailableBridgeErrorCode,
                IntPtr.Zero,
                UIntPtr.Zero));

        Assert.Equal(
            "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes is unavailable until ABI-7 recursive compact proof composition is enabled; bridge error code -312.",
            error.Message);
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

        Assert.Equal(
            "connect_norito_kagemusha_recursive_spend_redeem returned a null output pointer.",
            error.Message);
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

        Assert.Equal(
            "connect_norito_kagemusha_recursive_spend_redeem returned empty output.",
            error.Message);
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

        Assert.Equal(
            "connect_norito_kagemusha_recursive_spend_redeem returned oversized output.",
            error.Message);
    }

    [Fact]
    public void RecursiveSpendNativeReadBridgeOutputRejectsMalformedNoritoSuccessOutput()
    {
        static void AssertRejectsMalformedBridgeOutput(byte[] output)
        {
            var error = Assert.Throws<InvalidOperationException>(() =>
                ReadBridgeOutputWithBytes(output));

            Assert.Equal(
                "connect_norito_kagemusha_recursive_spend_redeem returned invalid Norito archive.",
                error.Message);
        }

        AssertRejectsMalformedBridgeOutput(new byte[] { 0x01 });

        var compressed = KagemushaNoritoFrameWithPayload(0x4b);
        compressed[22] = 1;
        AssertRejectsMalformedBridgeOutput(compressed);

        var unsupportedFlags = KagemushaNoritoFrameWithPayload(0x4b);
        unsupportedFlags[39] = 0x08;
        AssertRejectsMalformedBridgeOutput(unsupportedFlags);

        var invalidFieldBitset = KagemushaNoritoFrameWithPayload(0x4b);
        invalidFieldBitset[39] = 0x20;
        AssertRejectsMalformedBridgeOutput(invalidFieldBitset);

        AssertRejectsMalformedBridgeOutput(
            WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[] { 0x7f }));
        AssertRejectsMalformedBridgeOutput(
            WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[65]));
    }

    [Fact]
    public void RecursiveSpendNativeReadBridgeOutputClearsMalformedNoritoBeforeFree()
    {
        var output = new byte[] { 0x01, 0x02, 0x03 };
        var pointer = Marshal.AllocHGlobal(output.Length);
        var freed = false;
        Marshal.Copy(output, 0, pointer, output.Length);

        try
        {
            var error = Assert.Throws<InvalidOperationException>(() =>
                KagemushaRecursiveSpendNative.ReadBridgeOutput(
                    "connect_norito_kagemusha_recursive_spend_redeem",
                    0,
                    pointer,
                    (UIntPtr)output.Length,
                    ptr =>
                    {
                        Assert.Equal(pointer, ptr);
                        AssertPointerZeroed(ptr, output.Length);
                        Marshal.FreeHGlobal(ptr);
                        pointer = IntPtr.Zero;
                        freed = true;
                    }));

            Assert.True(freed);
            Assert.Contains("invalid Norito archive", error.Message);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    [Fact]
    public void RecursiveSpendNativeReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            ReadBridgeOutputWithBytes(KagemushaNoritoFrame(0x4b)));

        Assert.Equal(
            "connect_norito_kagemusha_recursive_spend_redeem returned empty Norito payload.",
            error.Message);
    }

    [Fact]
    public void PallasOpenEnvelopeBuilderReadBridgeOutputRejectsMalformedNoritoSuccessOutput()
    {
        var malformedCurrentHop = Assert.Throws<InvalidOperationException>(() =>
            ReadBridgeOutputWithBytes(
                new byte[] { 0x01 },
                "connect_norito_kagemusha_build_pallas_open_envelopes_archive"));
        Assert.Equal(
            "connect_norito_kagemusha_build_pallas_open_envelopes_archive returned invalid Norito archive.",
            malformedCurrentHop.Message);

        var malformedPreviousProof = Assert.Throws<InvalidOperationException>(() =>
            ReadBridgeOutputWithBytes(
                new byte[] { 0x01 },
                "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive"));
        Assert.Equal(
            "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive returned invalid Norito archive.",
            malformedPreviousProof.Message);
    }

    [Fact]
    public void PallasOpenEnvelopeBuilderReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput()
    {
        var emptyCurrentHop = Assert.Throws<InvalidOperationException>(() =>
            ReadBridgeOutputWithBytes(
                KagemushaNoritoFrame(0x4b),
                "connect_norito_kagemusha_build_pallas_open_envelopes_archive"));
        Assert.Equal(
            "connect_norito_kagemusha_build_pallas_open_envelopes_archive returned empty Norito payload.",
            emptyCurrentHop.Message);

        var emptyPreviousProof = Assert.Throws<InvalidOperationException>(() =>
            ReadBridgeOutputWithBytes(
                KagemushaNoritoFrame(0x4b),
                "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive"));
        Assert.Equal(
            "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive returned empty Norito payload.",
            emptyPreviousProof.Message);
    }

    [Fact]
    public void RecursiveSpendNativeReadBridgeOutputReturnsValidNoritoSuccessOutput()
    {
        var archive = KagemushaNoritoFrameWithPayload(0x4b);
        var pointer = Marshal.AllocHGlobal(archive.Length);
        var freed = false;
        Marshal.Copy(archive, 0, pointer, archive.Length);

        try
        {
            var output = KagemushaRecursiveSpendNative.ReadBridgeOutput(
                "connect_norito_kagemusha_recursive_spend_redeem",
                0,
                pointer,
                (UIntPtr)archive.Length,
                ptr =>
                {
                    Assert.Equal(pointer, ptr);
                    AssertPointerZeroed(ptr, archive.Length);
                    Marshal.FreeHGlobal(ptr);
                    pointer = IntPtr.Zero;
                    freed = true;
                });

            Assert.Equal(archive, output);
            Assert.True(freed);
        }
        finally
        {
            if (pointer != IntPtr.Zero)
            {
                Marshal.FreeHGlobal(pointer);
            }
        }
    }

    [Fact]
    public void RecursiveSpendNativeRejectsMalformedArchivesBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundle = RecordBundleWithStepCount();
        static void AssertRejectsMalformedEverywhere(
            byte[] malformed,
            byte[] validArchive,
            byte[] validRecordBundle)
        {
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Init(malformed));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.TopUp(malformed));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Append(malformed));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.TransitionProfileInit(malformed));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.TransitionProfileAppend(malformed));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageAppendBoundary(malformed));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
                malformed,
                validArchive));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
                validArchive,
                malformed));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
                malformed,
                validArchive,
                validArchive));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
                validArchive,
                malformed,
                validArchive));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
                validArchive,
                validArchive,
                malformed));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Verify(malformed));
            Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Redeem(malformed));
            Assert.Throws<ArgumentException>(() =>
                KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(malformed));
            Assert.Throws<ArgumentException>(() =>
                KagemushaRecursiveSpendNative
                    .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                        malformed,
                        validArchive));
            Assert.Throws<ArgumentException>(() =>
                KagemushaRecursiveSpendNative
                    .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                        validRecordBundle,
                        malformed));
        }

        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            new byte[] { 0x01, 0x02 },
            "must be a valid Norito archive.");
        AssertRejectsMalformedEverywhere(new byte[] { 0x01, 0x02 }, validArchive, validRecordBundle);

        var compressed = KagemushaNoritoFrameWithPayload(0x4b);
        compressed[22] = 1;
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            compressed,
            "must be a valid Norito archive.");
        AssertRejectsMalformedEverywhere(compressed, validArchive, validRecordBundle);

        var unsupportedFlags = KagemushaNoritoFrameWithPayload(0x4b);
        unsupportedFlags[39] = 0x08;
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            unsupportedFlags,
            "must be a valid Norito archive.");
        AssertRejectsMalformedEverywhere(unsupportedFlags, validArchive, validRecordBundle);

        var invalidFieldBitset = KagemushaNoritoFrameWithPayload(0x4b);
        invalidFieldBitset[39] = 0x20;
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            invalidFieldBitset,
            "must be a valid Norito archive.");
        AssertRejectsMalformedEverywhere(invalidFieldBitset, validArchive, validRecordBundle);

        AssertRejectsMalformedEverywhere(
            WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[] { 0x7f }),
            validArchive,
            validRecordBundle);
        AssertRejectsMalformedEverywhere(
            WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[65]),
            validArchive,
            validRecordBundle);
    }

    [Fact]
    public void RecursiveSpendNativeRejectsOversizedArchivesBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var oversizedArchive = OversizedKagemushaArchive();
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.Init(oversizedArchive),
            "Request archive must not exceed",
            "requestArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.Append(oversizedArchive),
            "Request archive must not exceed",
            "requestArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.TopUp(oversizedArchive),
            "Request archive must not exceed",
            "requestArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.TransitionProfileInit(oversizedArchive),
            "Request archive must not exceed",
            "requestArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.TransitionProfileAppend(oversizedArchive),
            "Request archive must not exceed",
            "requestArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.LineageAppendBoundary(oversizedArchive),
            "Request archive must not exceed",
            "requestArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
                oversizedArchive,
                validArchive),
            "Request archive must not exceed",
            "requestArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
                validArchive,
                oversizedArchive),
            "Bundle archive must not exceed",
            "bundleArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
                oversizedArchive,
                validArchive,
                validArchive),
            "Previous witness archive must not exceed",
            "previousWitnessArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
                validArchive,
                oversizedArchive,
                validArchive),
            "Request archive must not exceed",
            "requestArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
                validArchive,
                validArchive,
                oversizedArchive),
            "Bundle archive must not exceed",
            "bundleArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.Verify(oversizedArchive),
            "Request archive must not exceed",
            "requestArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.Redeem(oversizedArchive),
            "Request archive must not exceed",
            "requestArchive");
    }

    [Fact]
    public void RecursiveSpendNativeRejectsEmptyPayloadArchivesBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundle = RecordBundleWithStepCount();
        var emptyPayloadArchive = KagemushaNoritoFrame(0x4b);
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            KagemushaNoritoFrame(0x4b),
            "must contain a non-empty Norito payload.");
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Init(emptyPayloadArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Append(emptyPayloadArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.TransitionProfileInit(emptyPayloadArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.TransitionProfileAppend(emptyPayloadArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageAppendBoundary(emptyPayloadArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
            emptyPayloadArchive,
            validArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
            validArchive,
            emptyPayloadArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            emptyPayloadArchive,
            validArchive,
            validArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            validArchive,
            emptyPayloadArchive,
            validArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
            validArchive,
            validArchive,
            emptyPayloadArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Verify(emptyPayloadArchive));
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Redeem(emptyPayloadArchive));
        Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(emptyPayloadArchive));
        Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    emptyPayloadArchive,
                    validArchive));
        Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundle,
                    emptyPayloadArchive));
    }

    [Fact]
    public void RecursiveSpendRecordBundlePreflightRejectsOverLimitStepCountBeforeLoadingNativeBridge()
    {
        var recordBundle = RecordBundleWithOverLimitStepCountOnly();
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);

        void AssertRecordBundleRejected(Action action)
        {
            var error = Assert.Throws<ArgumentException>(action);
            Assert.Equal("recordBundleArchive", error.ParamName);
            Assert.Contains("recordBundle.steps fold step count must not exceed", error.Message);
        }

        AssertRecordBundleRejected(
            () => KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(recordBundle));
        AssertRecordBundleRejected(
            () => KagemushaRecursiveSpendNative.BuildPallasOpenEnvelopesArchive(recordBundle));
        AssertRecordBundleRejected(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    recordBundle,
                    validArchive));
        AssertRecordBundleRejected(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    recordBundle,
                    validArchive,
                    validArchive));
    }

    [Fact]
    public void RecursiveCompactVerifierRejectsOversizedInputBeforeLoadingNativeBridge()
    {
        var oversizedArchive = OversizedKagemushaArchive();
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(
                oversizedArchive,
                KagemushaNoritoFrameWithPayload(0xe2)),
            "Compact token archive must not exceed",
            "compactTokenArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(
                KagemushaNoritoFrameWithPayload(0x4b),
                oversizedArchive),
            "Recursive compact verifier keys archive must not exceed",
            "recursiveCompactVerifierKeysArchive");
    }

    private static JsonDocument LoadSharedRecursiveSpendManifest()
    {
        return LoadSharedRecursiveSpendFixture("manifest.json");
    }

    private static JsonDocument LoadSharedRecursiveSpendArchives()
    {
        return LoadSharedRecursiveSpendFixture("archives.json");
    }

    private static byte[] SharedRecursiveSpendArchive(string name)
    {
        using var archiveFixture = LoadSharedRecursiveSpendArchives();
        foreach (var archive in archiveFixture.RootElement.GetProperty("archives").EnumerateArray())
        {
            if (archive.GetProperty("name").GetString() == name)
            {
                return Convert.FromBase64String(archive.GetProperty("bytes_base64").GetString()!);
            }
        }

        throw new InvalidOperationException($"missing shared recursive spend archive {name}");
    }

    private static byte[] SharedRecursiveSpendAbi7Archive(string name)
    {
        using var archiveFixture = LoadSharedRecursiveSpendAbi7Archives();
        foreach (var archive in archiveFixture.RootElement.GetProperty("archives").EnumerateArray())
        {
            if (archive.GetProperty("name").GetString() == name)
            {
                return Convert.FromBase64String(archive.GetProperty("bytes_base64").GetString()!);
            }
        }

        throw new InvalidOperationException($"missing shared recursive spend ABI-7 archive {name}");
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

    private static JsonDocument LoadSharedRecursiveSpendAbi7Archives()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory != null)
        {
            var candidate = Path.Combine(
                directory.FullName,
                "fixtures",
                "kagemusha_recursive_spend_abi7",
                "archives.json");
            if (File.Exists(candidate))
            {
                return JsonDocument.Parse(File.ReadAllText(candidate));
            }

            directory = directory.Parent;
        }

        throw new FileNotFoundException("missing shared recursive spend ABI-7 archives fixture");
    }

    private const byte KagemushaNoritoCompactLenFlag = 0x02;
    private const byte KagemushaNoritoPackedStructFlag = 0x04;
    private const byte PrivacyNoritoFieldBitsetFlag = 0x20;

    private static readonly byte[] KagemushaLineageProvingKeyArchiveSchemaHash = new byte[]
    {
        0xc8, 0x84, 0x89, 0x61, 0x8a, 0x01, 0x2c, 0x28,
        0x3f, 0xf3, 0xbb, 0x2e, 0xba, 0xbc, 0x77, 0x75,
    };

    private static readonly byte[] OldKagemushaLineageProvingKeyArchiveSchemaHash = new byte[]
    {
        0x11, 0x9f, 0x4d, 0xf3, 0x8a, 0x98, 0xef, 0x58,
        0x48, 0xad, 0x0a, 0xad, 0xb9, 0x71, 0x57, 0x79,
    };
    private static readonly byte[] PallasOpenEnvelopeVectorSchemaHash = new byte[]
    {
        0xfe, 0x38, 0x26, 0x32, 0x8f, 0x08, 0x17, 0x71,
        0x75, 0x0f, 0x24, 0xfe, 0x11, 0x02, 0x60, 0xca,
    };

    private static readonly byte[] KagemushaPallasOpenEnvelopesSchemaHash = new byte[]
    {
        0xfe, 0x38, 0x26, 0x32, 0x8f, 0x08, 0x17, 0x71,
        0x75, 0x0f, 0x24, 0xfe, 0x11, 0x02, 0x60, 0xca,
    };

    private static byte[] KagemushaNoritoFrame(byte schemaByte)
    {
        var frame = new byte[40];
        frame[0] = (byte)'N';
        frame[1] = (byte)'R';
        frame[2] = (byte)'T';
        frame[3] = (byte)'0';
        Array.Fill(frame, schemaByte, 6, 16);
        return frame;
    }

    private static byte[] OversizedKagemushaArchive()
    {
        return new byte[KagemushaRecursiveSpendNative.NativeArchiveMaxBytes + 1];
    }

    private static void AssertOversizedArchive(
        Action action,
        string expectedMessage,
        string expectedParameterName)
    {
        AssertArgumentDiagnostic(
            $"{expectedMessage} {KagemushaRecursiveSpendNative.NativeArchiveMaxBytes} bytes.",
            expectedParameterName,
            action);
    }

    private static void AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
        byte[] rejectedArchive,
        string expectedPredicate)
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundleArchive = KagemushaRecordBundleArchiveWithStepCount(1);
        var validPallasOpenEnvelopesArchive = KagemushaPallasOpenEnvelopesArchiveWithCount(1);

        void AssertRejected(string displayName, string expectedParameterName, Action action)
        {
            AssertArgumentDiagnostic($"{displayName} {expectedPredicate}", expectedParameterName, action);
        }

        AssertRejected(
            "Request archive",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Init(rejectedArchive));
        AssertRejected(
            "Request archive",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.TopUp(rejectedArchive));
        AssertRejected(
            "Request archive",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Append(rejectedArchive));
        AssertRejected(
            "Request archive",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.TransitionProfileInit(rejectedArchive));
        AssertRejected(
            "Request archive",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.TransitionProfileAppend(rejectedArchive));
        AssertRejected(
            "Request archive",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.LineageAppendBoundary(rejectedArchive));
        AssertRejected(
            "Request archive",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
                rejectedArchive,
                validArchive));
        AssertRejected(
            "Bundle archive",
            "bundleArchive",
            () => KagemushaRecursiveSpendNative.LineageWitnessFromInitResult(
                validArchive,
                rejectedArchive));
        AssertRejected(
            "Previous witness archive",
            "previousWitnessArchive",
            () => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
                rejectedArchive,
                validArchive,
                validArchive));
        AssertRejected(
            "Request archive",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
                validArchive,
                rejectedArchive,
                validArchive));
        AssertRejected(
            "Bundle archive",
            "bundleArchive",
            () => KagemushaRecursiveSpendNative.LineageWitnessAppendResult(
                validArchive,
                validArchive,
                rejectedArchive));
        AssertRejected(
            "Request archive",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Verify(rejectedArchive));
        AssertRejected(
            "Request archive",
            "requestArchive",
            () => KagemushaRecursiveSpendNative.Redeem(rejectedArchive));
        AssertRejected(
            "Record bundle archive",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(
                rejectedArchive));
        AssertRejected(
            "Record bundle archive",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    rejectedArchive,
                    validPallasOpenEnvelopesArchive));
        AssertRejected(
            "Pallas open-envelopes archive",
            "pallasOpenEnvelopesArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundleArchive,
                    rejectedArchive));
        AssertRejected(
            "Record bundle archive",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative.BuildPallasOpenEnvelopesArchive(rejectedArchive));
        AssertRejected(
            "Previous recursive proof bundle archive",
            "previousBundleArchive",
            () => KagemushaRecursiveSpendNative.BuildPreviousProofOpenEnvelopesArchive(
                rejectedArchive));
    }

    private static void AssertRecordBundleStepCountPreflightRejects(
        Action action,
        string expectedParameterName)
    {
        AssertArgumentDiagnostic(
            $"recordBundle.steps fold step count must not exceed {KagemushaRecursiveSpendNative.CompactTokenMaxHops}",
            expectedParameterName,
            action);
    }

    private static void AssertPallasEnvelopeCountPreflightRejects(
        Action action,
        string expectedParameterName)
    {
        AssertArgumentDiagnostic(
            "pallasOpenEnvelopes requires exactly 1 envelope(s)",
            expectedParameterName,
            action);
    }

    private static void AssertPallasInnerEnvelopePreflightRejects(
        Action action,
        string expectedParameterName,
        string expectedMessage)
    {
        AssertArgumentDiagnostic(expectedMessage, expectedParameterName, action);
    }

    private static void AssertArgumentDiagnostic(
        string expectedMessage,
        string expectedParameterName,
        Action action)
    {
        var error = Assert.ThrowsAny<ArgumentException>(action);
        Assert.Equal(expectedParameterName, error.ParamName);
        Assert.True(
            error.Message.Length >= expectedMessage.Length
            && (error.Message.Length == expectedMessage.Length
                || error.Message[expectedMessage.Length] == ' '
                || error.Message[expectedMessage.Length] == '.'
                || error.Message[expectedMessage.Length] == ':'),
            $"unexpected diagnostic suffix: {error.Message}");
        Assert.Equal(expectedMessage, error.Message[..expectedMessage.Length]);
    }

    private static void AssertExactLineageKeyArtifactError(
        string expectedMessage,
        Action action)
    {
        var error = Assert.Throws<ArgumentException>(action);
        Assert.Equal(expectedMessage, error.Message);
    }

    private static byte[] KagemushaNoritoFrameWithPayload(byte schemaByte)
    {
        var frame = new byte[45];
        KagemushaNoritoFrame(schemaByte).CopyTo(frame, 0);
        frame[23] = 3;
        new byte[]
        {
            0xb9,
            0xd3,
            0xa8,
            0x0c,
            0xcd,
            0x5d,
            0x13,
            0x24,
        }.CopyTo(frame, 31);
        frame[42] = 0xa5;
        frame[43] = 0x5a;
        frame[44] = 0x11;
        return frame;
    }

    private static byte[] WithHeaderPadding(byte[] archive, byte[] padding)
    {
        var padded = new byte[archive.Length + padding.Length];
        Array.Copy(archive, 0, padded, 0, 40);
        Array.Copy(padding, 0, padded, 40, padding.Length);
        Array.Copy(archive, 40, padded, 40 + padding.Length, archive.Length - 40);
        return padded;
    }

    private static readonly ulong[] KagemushaTestCrc64Table = BuildKagemushaTestCrc64Table();

    private static ulong[] BuildKagemushaTestCrc64Table()
    {
        const ulong reflectedPoly = 0xC96C_5795_D787_0F42UL;
        var table = new ulong[256];
        for (var index = 0; index < table.Length; index++)
        {
            var crc = (ulong)index;
            for (var bit = 0; bit < 8; bit++)
            {
                crc = (crc & 1) != 0 ? (crc >> 1) ^ reflectedPoly : crc >> 1;
            }
            table[index] = crc;
        }
        return table;
    }

    private static ulong KagemushaTestCrc64(byte[] payload)
    {
        var crc = ulong.MaxValue;
        foreach (var value in payload)
        {
            var index = (byte)(crc ^ value);
            crc = KagemushaTestCrc64Table[index] ^ (crc >> 8);
        }
        return crc ^ ulong.MaxValue;
    }

    private static byte[] KagemushaNoritoFrameFromPayload(byte schemaByte, byte[] payload)
    {
        var frame = new byte[40 + payload.Length];
        KagemushaNoritoFrame(schemaByte).CopyTo(frame, 0);
        payload.CopyTo(frame, 40);
        BinaryPrimitives.WriteUInt64LittleEndian(frame.AsSpan(23, 8), (ulong)payload.Length);
        BinaryPrimitives.WriteUInt64LittleEndian(frame.AsSpan(31, 8), KagemushaTestCrc64(payload));
        return frame;
    }

    private static byte[] KagemushaNoritoFrameFromSchemaHash(
        byte[] schemaHash,
        byte[] payload,
        byte flags = 0)
    {
        var frame = new byte[40 + payload.Length];
        frame[0] = (byte)'N';
        frame[1] = (byte)'R';
        frame[2] = (byte)'T';
        frame[3] = (byte)'0';
        schemaHash.CopyTo(frame, 6);
        frame[39] = flags;
        payload.CopyTo(frame, 40);
        BinaryPrimitives.WriteUInt64LittleEndian(frame.AsSpan(23, 8), (ulong)payload.Length);
        BinaryPrimitives.WriteUInt64LittleEndian(frame.AsSpan(31, 8), KagemushaTestCrc64(payload));
        return frame;
    }

    private static byte[] KagemushaNoritoPayload(byte[] archive)
    {
        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(archive.AsSpan(23, 8));
        Assert.True(payloadLength <= int.MaxValue);
        Assert.True(payloadLength <= (ulong)(archive.Length - 40));
        return archive.AsSpan(archive.Length - (int)payloadLength, (int)payloadLength).ToArray();
    }

    private static byte[] RebuildKagemushaNoritoFrameLike(
        byte[] archive,
        byte[] payload,
        byte? flags = null)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            archive.AsSpan(6, 16).ToArray(),
            payload,
            flags ?? archive[39]);
    }

    private static byte[] RecordBundleWithStepCount(int hopCount = 1)
    {
        var stepPayload = KagemushaNoritoEncodeFields(
            Enumerable.Range(0, 6).Select(index => new byte[] { (byte)(0xa0 + index) }),
            KagemushaNoritoCompactLenFlag);
        var steps = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(steps, (ulong)hopCount);
        var stepsPayload = steps
            .Concat(Enumerable.Range(0, hopCount)
                .SelectMany(_ => KagemushaNoritoField(stepPayload)))
            .ToArray();
        return RecordBundleWithStepsPayload(stepsPayload);
    }

    private static byte[] RecordBundleWithOverLimitStepCountOnly()
    {
        var stepsPayload = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(
            stepsPayload,
            (ulong)KagemushaRecursiveSpendNative.CompactTokenMaxHops + 1);
        return RecordBundleWithStepsPayload(stepsPayload);
    }

    private static byte[] KagemushaRecordBundleArchiveWithStepCount(int hopCount)
    {
        return RecordBundleWithStepCount(hopCount);
    }

    private static byte[] KagemushaRecordBundleArchiveWithStepsPayload(byte[] stepsPayload)
    {
        return RecordBundleWithStepsPayload(stepsPayload);
    }

    private static byte[] KagemushaRecordBundlePayloadWithStepCount(int hopCount)
    {
        return KagemushaNoritoPayload(KagemushaRecordBundleArchiveWithStepCount(hopCount));
    }

    private static byte[] KagemushaRecordBundlePayloadWithStepsPayload(byte[] stepsPayload)
    {
        return KagemushaNoritoPayload(KagemushaRecordBundleArchiveWithStepsPayload(stepsPayload));
    }

    private static byte[] RecordBundleWithStepsPayload(byte[] stepsPayload)
    {
        var bundlePayload = KagemushaNoritoEncodeFields(
            new[]
            {
                new byte[] { 0x41 },
                new byte[] { 0x42 },
                stepsPayload,
            },
            KagemushaNoritoCompactLenFlag);
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.VerifiedFoldRecordBundleWireName),
            KagemushaNoritoEncodeFields(
                new[]
                {
                    bundlePayload,
                    Array.Empty<byte>(),
                },
                KagemushaNoritoCompactLenFlag),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaInitRequestArchiveWithRecordBundle(byte[] recordBundlePayload)
    {
        return KagemushaInitRequestArchiveWithRecordBundleAndPallas(
            recordBundlePayload,
            KagemushaPallasOpenEnvelopesArchiveWithCount(1));
    }

    private static byte[] KagemushaInitRequestArchiveWithRecordBundleAndPallas(
        byte[] recordBundlePayload,
        byte[] pallasOpenEnvelopesArchive)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendInitRequestWireName),
            KagemushaNoritoEncodeFields(
                new[]
                {
                    recordBundlePayload,
                    KagemushaNoritoByteVec(pallasOpenEnvelopesArchive),
                    KagemushaSpendableNotePayload(),
                    KagemushaNoritoOptionPayload(null),
                    KagemushaNoritoOptionPayload(null),
                    KagemushaNoritoOptionPayload(null),
                },
                KagemushaNoritoCompactLenFlag),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaAppendRequestArchiveWithRecordBundle(byte[] recordBundlePayload)
    {
        return KagemushaAppendRequestArchiveWithRecordBundleAndPallas(
            recordBundlePayload,
            KagemushaPallasOpenEnvelopesArchiveWithCount(1));
    }

    private static byte[] KagemushaAppendRequestArchiveWithRecordBundleAndPallas(
        byte[] recordBundlePayload,
        byte[] pallasOpenEnvelopesArchive)
    {
        return KagemushaFullAppendRequestArchive(
            SharedRecursiveSpendAbi7Archive("append_bundle"),
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            recordBundlePayload: recordBundlePayload,
            pallasOpenEnvelopesArchive: pallasOpenEnvelopesArchive);
    }

    private static byte[] KagemushaFullAppendRequestArchive(
        byte[] previousBundleArchive,
        string outputProofCircuitId,
        byte[]? previousLineageVerifierRecordPayload = null,
        byte[]? previousProofOpenEnvelopesArchive = null,
        byte[]? lineageVerifierKeyPayload = null,
        byte[]? lineageProvingKeyArchivePayload = null,
        byte[]? recordBundlePayload = null,
        byte[]? pallasOpenEnvelopesArchive = null)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendAppendRequestWireName),
            KagemushaNoritoEncodeFields(
                new[]
                {
                    KagemushaNoritoPayload(previousBundleArchive),
                    recordBundlePayload ?? KagemushaRecordBundlePayloadWithStepCount(1),
                    KagemushaNoritoByteVec(
                        pallasOpenEnvelopesArchive ?? KagemushaPallasOpenEnvelopesArchiveWithCount(1)),
                    KagemushaSpendableNotePayload(),
                    KagemushaNoritoString(outputProofCircuitId),
                    KagemushaNoritoOptionPayload(previousLineageVerifierRecordPayload),
                    KagemushaNoritoByteVec(previousProofOpenEnvelopesArchive ?? Array.Empty<byte>()),
                    KagemushaNoritoOptionPayload(lineageVerifierKeyPayload),
                    KagemushaNoritoOptionPayload(lineageProvingKeyArchivePayload),
                    KagemushaNoritoOptionPayload(null),
                },
                KagemushaNoritoCompactLenFlag),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaSpendableNotePayload()
    {
        return KagemushaNoritoEncodeFields(
            new[]
            {
                KagemushaFixedArrayPayload(0x31, 32),
                KagemushaFixedArrayPayload(0x32, 32),
                KagemushaNumericAmountPayload(7),
            },
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaNoritoOptionPayload(byte[]? payload)
    {
        if (payload is null)
        {
            return new byte[] { 0 };
        }
        return new byte[] { 1 }
            .Concat(KagemushaNoritoField(payload))
            .ToArray();
    }

    private static byte[] KagemushaPallasOpenEnvelopesArchiveWithCount(int count)
    {
        return PallasOpenEnvelopesArchive(count);
    }

    private static byte[] KagemushaPallasOpenEnvelopesArchiveWithEnvelope(byte[] envelope)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            KagemushaPallasOpenEnvelopesSchemaHash,
            U64LE(1)
                .Concat(KagemushaNoritoField(envelope))
                .ToArray(),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaPallasOpenEnvelopePayload(
        byte[]? paramsPayload = null,
        byte[]? publicPayload = null,
        byte[]? proofPayload = null,
        byte[]? domainTag = null)
    {
        return KagemushaNoritoEncodeFields(
            new[]
            {
                paramsPayload ?? KagemushaPallasIpaParamsPayload(),
                publicPayload ?? KagemushaPallasPolyOpenPublicPayload(),
                proofPayload ?? KagemushaPallasIpaProofPayload(),
                KagemushaNoritoString("pallas-open"),
                KagemushaPallasMetadataOption(SyntheticFixed32(0x70)),
                KagemushaPallasMetadataOption(SyntheticFixed32(0x71)),
                domainTag ?? KagemushaPallasMetadataOption(SyntheticFixed32(0x72)),
            },
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaPallasIpaParamsPayload(
        int n = 4,
        byte[]? gPayload = null,
        byte[]? hPayload = null)
    {
        return KagemushaNoritoEncodeFields(
            new[]
            {
                U16LE(1),
                U16LE(1),
                U32LE(n),
                gPayload ?? Fixed32Sequence(n, 0x10),
                hPayload ?? Fixed32Sequence(n, 0x20),
                SyntheticFixed32(0x30),
            },
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaPallasPolyOpenPublicPayload(int n = 4)
    {
        return KagemushaNoritoEncodeFields(
            new[]
            {
                U16LE(1),
                U16LE(1),
                U32LE(n),
                SyntheticFixed32(0x31),
                SyntheticFixed32(0x32),
                SyntheticFixed32(0x33),
            },
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaPallasIpaProofPayload(
        byte[]? lPayload = null,
        byte[]? rPayload = null)
    {
        return KagemushaNoritoEncodeFields(
            new[]
            {
                U16LE(1),
                lPayload ?? Fixed32Sequence(2, 0x40),
                rPayload ?? Fixed32Sequence(2, 0x50),
                SyntheticFixed32(0x60),
                SyntheticFixed32(0x61),
            },
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaPallasMetadataOption(byte[]? payload)
    {
        return OptionRaw(payload);
    }

    private static KagemushaRecursiveSpendableNoteDescriptor ValidSpendableNoteDescriptor()
    {
        return new KagemushaRecursiveSpendableNoteDescriptor(
            Enumerable.Repeat((byte)0x31, 32).ToArray(),
            Enumerable.Repeat((byte)0x32, 32).ToArray(),
            "7");
    }

    private static byte[] VerifyingKeyRecordArchive()
    {
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.VerifyingKeyRecordWireName),
            KagemushaNoritoEncodeFields(
                new[]
                {
                    KagemushaNoritoString(KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend),
                    KagemushaNoritoString("kagemusha-recursive-spend-test-key"),
                    KagemushaNoritoByteVec(Enumerable.Repeat((byte)0x55, 32).ToArray()),
                },
                KagemushaNoritoCompactLenFlag),
            KagemushaNoritoCompactLenFlag);
    }

    private static void AssertPallasArchiveRejected(Action action, string expectedMessage)
    {
        var error = Assert.Throws<ArgumentException>(action);
        Assert.Equal("pallasOpenEnvelopesArchive", error.ParamName);
        var directFieldMessage = expectedMessage.Replace(
            "pallasOpenEnvelopesArchive",
            "pallasOpenEnvelopes",
            StringComparison.Ordinal);
        Assert.True(
            error.Message.Contains(expectedMessage, StringComparison.Ordinal)
            || error.Message.Contains(directFieldMessage, StringComparison.Ordinal),
            error.Message);
    }

    private static byte[] PallasOpenEnvelopesArchive(
        int count = 1,
        Action<PallasOpenEnvelopeSpec>? configure = null)
    {
        var spec = new PallasOpenEnvelopeSpec();
        configure?.Invoke(spec);
        var envelope = PallasOpenEnvelopePayload(spec);
        var payload = U64LE((ulong)count)
            .Concat(Enumerable.Range(0, count)
                .SelectMany(_ => KagemushaNoritoField(envelope)))
            .ToArray();
        return KagemushaNoritoFrameFromSchemaHash(
            PallasOpenEnvelopeVectorSchemaHash,
            payload,
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] PallasOpenEnvelopePayload(PallasOpenEnvelopeSpec spec)
    {
        const int n = 4;
        var parameters = KagemushaNoritoEncodeFields(
            new[]
            {
                U16LE(1),
                U16LE(spec.ParamsCurveId),
                U32LE(n),
                spec.ParamsGSequencePayload ?? Fixed32Sequence(n, 0x10),
                spec.ParamsHSequencePayload ?? Fixed32Sequence(n, 0x20),
                SyntheticFixed32(0x30),
            },
            KagemushaNoritoCompactLenFlag);
        var publicValue = KagemushaNoritoEncodeFields(
            new[]
            {
                U16LE(1),
                U16LE(spec.PublicCurveId),
                U32LE(n),
                SyntheticFixed32(0x31),
                SyntheticFixed32(0x32),
                SyntheticFixed32(0x33),
            },
            KagemushaNoritoCompactLenFlag);
        var proof = KagemushaNoritoEncodeFields(
            new[]
            {
                U16LE(1),
                spec.ProofLSequencePayload ?? Fixed32Sequence(2, 0x40),
                spec.ProofRSequencePayload ?? Fixed32Sequence(2, 0x50),
                SyntheticFixed32(0x60),
                SyntheticFixed32(0x61),
            },
            KagemushaNoritoCompactLenFlag);
        return KagemushaNoritoEncodeFields(
            new[]
            {
                parameters,
                publicValue,
                proof,
                KagemushaNoritoString(spec.TranscriptLabel),
                spec.VkCommitmentOptionPayload ??
                    OptionRaw(spec.IncludeVkCommitment ? spec.VkCommitmentPayload ?? SyntheticFixed32(0x70) : null),
                spec.PublicInputsSchemaHashOptionPayload ??
                    OptionRaw(
                        spec.IncludePublicInputsSchemaHash
                            ? spec.PublicInputsSchemaHashPayload ?? SyntheticFixed32(0x71)
                            : null),
                spec.DomainTagOptionPayload ??
                    OptionRaw(spec.IncludeDomainTag ? spec.DomainTagPayload ?? SyntheticFixed32(0x72) : null),
            },
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] Fixed32Sequence(int count, byte seed)
    {
        return U64LE((ulong)count)
            .Concat(Enumerable.Range(0, count)
                .SelectMany(index => KagemushaNoritoField(SyntheticFixed32((byte)(seed + index)))))
            .ToArray();
    }

    private static byte[] SyntheticFixed32(byte seed)
    {
        return Enumerable.Range(0, 32)
            .Select(index => (byte)(seed + index))
            .ToArray();
    }

    private static byte[] OptionRaw(byte[]? payload)
    {
        if (payload is null)
        {
            return new byte[] { 0 };
        }
        return new byte[] { 1 }
            .Concat(KagemushaNoritoLength(payload.Length, KagemushaNoritoCompactLenFlag))
            .Concat(payload)
            .ToArray();
    }

    private static byte[] OptionRawWithTrailingByte(byte[] payload)
    {
        return OptionRaw(payload).Concat(new byte[] { 0x7f }).ToArray();
    }

    private static byte[] OptionRawWithUnknownTag()
    {
        return new byte[] { 0x02 };
    }

    private static byte[] OptionRawWithDeclaredLengthTooLong(byte[] payload)
    {
        return new byte[] { 1 }
            .Concat(KagemushaNoritoLength(payload.Length + 1, KagemushaNoritoCompactLenFlag))
            .Concat(payload)
            .ToArray();
    }

    private static byte[] U64LE(ulong value)
    {
        var output = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(output, value);
        return output;
    }

    private static byte[] KagemushaUInt64Payload(ulong value)
    {
        return U64LE(value);
    }

    private static byte[] U32LE(int value)
    {
        var output = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(output, checked((uint)value));
        return output;
    }

    private static byte[] U16LE(int value)
    {
        var output = new byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(output, checked((ushort)value));
        return output;
    }

    private sealed class PallasOpenEnvelopeSpec
    {
        internal int ParamsCurveId { get; set; } = 1;
        internal int PublicCurveId { get; set; } = 1;
        internal string TranscriptLabel { get; set; } = "pallas-open";
        internal byte[]? ParamsGSequencePayload { get; set; }
        internal byte[]? ParamsHSequencePayload { get; set; }
        internal byte[]? ProofLSequencePayload { get; set; }
        internal byte[]? ProofRSequencePayload { get; set; }
        internal bool IncludeVkCommitment { get; set; } = true;
        internal bool IncludePublicInputsSchemaHash { get; set; } = true;
        internal bool IncludeDomainTag { get; set; } = true;
        internal byte[]? VkCommitmentPayload { get; set; }
        internal byte[]? PublicInputsSchemaHashPayload { get; set; }
        internal byte[]? DomainTagPayload { get; set; }
        internal byte[]? VkCommitmentOptionPayload { get; set; }
        internal byte[]? PublicInputsSchemaHashOptionPayload { get; set; }
        internal byte[]? DomainTagOptionPayload { get; set; }
    }

    private static void AssertBundleSummaryRejects(byte[] archive, string expectedField)
    {
        AssertArgumentDiagnostic(
            expectedField,
            "bundleArchive",
            () => KagemushaRecursiveSpendNative.DecodeBundleSummary(archive));
    }

    private static void AssertTransitionProfileSummaryRejects(byte[] archive, string expectedField)
    {
        Assert.Contains(
            expectedField,
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.DecodeTransitionProfileSummary(archive)).Message);
    }

    private static byte[] RecursiveSpendBundleWithPayloadTextReplaced(
        byte[] archive,
        string existing,
        string replacement)
    {
        var existingBytes = Encoding.UTF8.GetBytes(existing);
        var replacementBytes = Encoding.UTF8.GetBytes(replacement);
        Assert.Equal(existingBytes.Length, replacementBytes.Length);
        var payload = KagemushaNoritoPayload(archive);
        var replacements = 0;
        var searchOffset = 0;
        while (searchOffset <= payload.Length - existingBytes.Length)
        {
            var relativeIndex = payload.AsSpan(searchOffset).IndexOf(existingBytes);
            if (relativeIndex < 0)
            {
                break;
            }

            var absoluteIndex = searchOffset + relativeIndex;
            replacementBytes.CopyTo(payload.AsSpan(absoluteIndex, replacementBytes.Length));
            replacements++;
            searchOffset = absoluteIndex + replacementBytes.Length;
        }

        Assert.True(replacements > 0);
        return RebuildKagemushaNoritoFrameLike(archive, payload);
    }

    private static byte[] RecursiveSpendVerifyResultWithTrailingField()
    {
        var archive = SharedRecursiveSpendAbi7Archive("verify_result");
        var flags = archive[39];
        var fields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        fields.Add(new byte[] { 0x01 });
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(fields, flags));
    }

    private static byte[] RecursiveSpendLineageWitnessWithTrailingField()
    {
        var archive = SharedRecursiveSpendArchive("lineage_witness_append_result");
        var flags = archive[39];
        var fields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        fields.Add(KagemushaNoritoString("ignored-extra-lineage-witness-field"));
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(fields, flags));
    }

    private static byte[] RecursiveSpendLineageWitnessWithTrailingPreviousProofsField()
    {
        var archive = SharedRecursiveSpendArchive("lineage_witness_append_result");
        var flags = archive[39];
        var fields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(fields.Count > 3);
        fields[3] = fields[3]
            .Concat(KagemushaNoritoField(
                KagemushaNoritoString("ignored-extra-previous-proofs-field"),
                flags))
            .ToArray();
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(fields, flags));
    }

    private static byte[] RecursiveSpendLineageWitnessWithOverLimitPreviousProofCountOnly()
    {
        var archive = SharedRecursiveSpendArchive("lineage_witness_append_result");
        var flags = archive[39];
        var fields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(fields.Count > 3);

        var previousProofsPayload = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(
            previousProofsPayload,
            (ulong)KagemushaRecursiveSpendNative.CompactTokenMaxHops + 1);
        fields[3] = previousProofsPayload;

        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(fields, flags));
    }

    private static byte[] RecursiveSpendLineageWitnessWithTrailingPreviousProofField()
    {
        var archive = SharedRecursiveSpendArchive("lineage_witness_append_result");
        var flags = archive[39];
        var fields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(fields.Count > 3);
        var previousProofsPrefix = fields[3].AsSpan(0, 8).ToArray();
        var previousProofFields = KagemushaNoritoReadFieldPayloads(
            fields[3].AsSpan(8).ToArray(),
            flags);
        Assert.True(previousProofFields.Count >= 1);
        previousProofFields[0] = previousProofFields[0]
            .Concat(KagemushaNoritoField(
                KagemushaNoritoString("ignored-extra-previous-proof-field"),
                flags))
            .ToArray();
        fields[3] = previousProofsPrefix
            .Concat(KagemushaNoritoEncodeFields(previousProofFields, flags))
            .ToArray();
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(fields, flags));
    }

    private static byte[] RecursiveSpendLineageWitnessWithPreviousProofCountPrefixOnly(ulong count)
    {
        var archive = SharedRecursiveSpendArchive("lineage_witness_append_result");
        var flags = archive[39];
        var fields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(fields.Count > 3);
        fields[3] = KagemushaUInt64Payload(count);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(fields, flags));
    }

    private static byte[] RecursiveSpendLineageWitnessWithPreviousProofField(
        int fieldIndex,
        byte[] replacementPayload)
    {
        var archive = SharedRecursiveSpendArchive("lineage_witness_append_result");
        var flags = archive[39];
        var fields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(fields.Count > 3);
        var previousProofsPrefix = fields[3].AsSpan(0, 8).ToArray();
        var previousProofFields = KagemushaNoritoReadFieldPayloads(
            fields[3].AsSpan(8).ToArray(),
            flags);
        Assert.True(previousProofFields.Count >= 1);
        var previousProofInnerFields = KagemushaNoritoReadFieldPayloads(
            previousProofFields[0],
            flags);
        Assert.True(fieldIndex >= 0 && fieldIndex < previousProofInnerFields.Count);
        previousProofInnerFields[fieldIndex] = replacementPayload;
        previousProofFields[0] = KagemushaNoritoEncodeFields(previousProofInnerFields, flags);
        fields[3] = previousProofsPrefix
            .Concat(KagemushaNoritoEncodeFields(previousProofFields, flags))
            .ToArray();
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(fields, flags));
    }

    private static byte[] RecursiveSpendLineageWitnessWithTrailingPreviousVerifierKeyIdField()
    {
        var archive = SharedRecursiveSpendArchive("lineage_witness_append_result");
        var flags = archive[39];
        var fields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(fields.Count > 3);
        var previousProofsPrefix = fields[3].AsSpan(0, 8).ToArray();
        var previousProofFields = KagemushaNoritoReadFieldPayloads(
            fields[3].AsSpan(8).ToArray(),
            flags);
        Assert.True(previousProofFields.Count >= 1);
        var previousProofInnerFields = KagemushaNoritoReadFieldPayloads(
            previousProofFields[0],
            flags);
        Assert.True(previousProofInnerFields.Count >= 1);
        var verifierKeyIdFields = KagemushaNoritoReadFieldPayloads(
            previousProofInnerFields[0],
            flags);
        Assert.True(verifierKeyIdFields.Count >= 2);
        verifierKeyIdFields.Add(KagemushaNoritoString("ignored-extra-verifier-key-id-field"));
        previousProofInnerFields[0] = KagemushaNoritoEncodeFields(verifierKeyIdFields, flags);
        previousProofFields[0] = KagemushaNoritoEncodeFields(previousProofInnerFields, flags);
        fields[3] = previousProofsPrefix
            .Concat(KagemushaNoritoEncodeFields(previousProofFields, flags))
            .ToArray();
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(fields, flags));
    }

    private static byte[] RecursiveSpendBundleWithEmptyProofBytes(byte[] archive)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 2);
        var recursiveProofFields = KagemushaNoritoReadFieldPayloads(topLevelFields[1], flags);
        Assert.True(recursiveProofFields.Count >= 4);
        var proofBoxFields = KagemushaNoritoReadFieldPayloads(recursiveProofFields[3], flags);
        Assert.True(proofBoxFields.Count >= 2);
        proofBoxFields[1] = KagemushaNoritoByteVec(Array.Empty<byte>());
        recursiveProofFields[3] = KagemushaNoritoEncodeFields(proofBoxFields, flags);
        topLevelFields[1] = KagemushaNoritoEncodeFields(recursiveProofFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithProofBoxField(
        byte[] archive,
        int fieldIndex,
        byte[] replacementPayload)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 2);
        var recursiveProofFields = KagemushaNoritoReadFieldPayloads(topLevelFields[1], flags);
        Assert.True(recursiveProofFields.Count >= 4);
        var proofBoxFields = KagemushaNoritoReadFieldPayloads(recursiveProofFields[3], flags);
        Assert.True(fieldIndex >= 0 && fieldIndex < proofBoxFields.Count);
        proofBoxFields[fieldIndex] = replacementPayload;
        recursiveProofFields[3] = KagemushaNoritoEncodeFields(proofBoxFields, flags);
        topLevelFields[1] = KagemushaNoritoEncodeFields(recursiveProofFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithRecursiveProofField(
        byte[] archive,
        int fieldIndex,
        byte[] replacementPayload)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 2);
        var recursiveProofFields = KagemushaNoritoReadFieldPayloads(topLevelFields[1], flags);
        Assert.True(fieldIndex >= 0 && fieldIndex < recursiveProofFields.Count);
        recursiveProofFields[fieldIndex] = replacementPayload;
        topLevelFields[1] = KagemushaNoritoEncodeFields(recursiveProofFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithAccumulatorField(
        byte[] archive,
        int fieldIndex,
        byte[] replacementPayload)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 2);
        var accumulatorFields = KagemushaNoritoReadFieldPayloads(topLevelFields[0], flags);
        Assert.True(fieldIndex >= 0 && fieldIndex < accumulatorFields.Count);
        accumulatorFields[fieldIndex] = replacementPayload;
        topLevelFields[0] = KagemushaNoritoEncodeFields(accumulatorFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendTransitionProfileWithField(
        byte[] archive,
        int fieldIndex,
        byte[] replacementPayload)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(fieldIndex >= 0 && fieldIndex < topLevelFields.Count);
        topLevelFields[fieldIndex] = replacementPayload;
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendTransitionProfileWithCurrentHopOutputCommitments(
        byte[] archive,
        byte[][] outputCommitments)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count > 20);
        var currentHopFields = KagemushaNoritoReadFieldPayloads(topLevelFields[20], flags);
        Assert.True(currentHopFields.Count > 3);
        currentHopFields[3] = TopupAnchorNullifiersPayload(outputCommitments);
        topLevelFields[20] = KagemushaNoritoEncodeFields(currentHopFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendTransitionProfileWithCurrentNoteField(
        byte[] archive,
        int fieldIndex,
        byte[] replacementPayload)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count > 21);
        var currentNoteFields = KagemushaNoritoReadFieldPayloads(topLevelFields[21], flags);
        Assert.True(fieldIndex >= 0 && fieldIndex < currentNoteFields.Count);
        currentNoteFields[fieldIndex] = replacementPayload;
        topLevelFields[21] = KagemushaNoritoEncodeFields(currentNoteFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithCurrentNoteField(
        byte[] archive,
        int fieldIndex,
        byte[] replacementPayload)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 1);
        var accumulatorFields = KagemushaNoritoReadFieldPayloads(topLevelFields[0], flags);
        Assert.True(accumulatorFields.Count > 22);
        var currentNoteFields = KagemushaNoritoReadFieldPayloads(accumulatorFields[22], flags);
        Assert.True(fieldIndex >= 0 && fieldIndex < currentNoteFields.Count);
        currentNoteFields[fieldIndex] = replacementPayload;
        accumulatorFields[22] = KagemushaNoritoEncodeFields(currentNoteFields, flags);
        topLevelFields[0] = KagemushaNoritoEncodeFields(accumulatorFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithEqualCurrentNoteNullifier(byte[] archive)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 1);
        var accumulatorFields = KagemushaNoritoReadFieldPayloads(topLevelFields[0], flags);
        Assert.True(accumulatorFields.Count > 22);
        var currentNoteFields = KagemushaNoritoReadFieldPayloads(accumulatorFields[22], flags);
        Assert.True(currentNoteFields.Count >= 2);
        currentNoteFields[1] = currentNoteFields[0].ToArray();
        accumulatorFields[22] = KagemushaNoritoEncodeFields(currentNoteFields, flags);
        topLevelFields[0] = KagemushaNoritoEncodeFields(accumulatorFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithTopLevelTrailingField(byte[] archive)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        topLevelFields.Add(KagemushaNoritoString("unexpected"));
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithAccumulatorTrailingField(byte[] archive)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 1);
        var accumulatorFields = KagemushaNoritoReadFieldPayloads(topLevelFields[0], flags);
        accumulatorFields.Add(KagemushaNoritoString("unexpected"));
        topLevelFields[0] = KagemushaNoritoEncodeFields(accumulatorFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithCurrentNoteTrailingField(byte[] archive)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 1);
        var accumulatorFields = KagemushaNoritoReadFieldPayloads(topLevelFields[0], flags);
        Assert.True(accumulatorFields.Count > 22);
        var currentNoteFields = KagemushaNoritoReadFieldPayloads(accumulatorFields[22], flags);
        currentNoteFields.Add(KagemushaNoritoString("unexpected"));
        accumulatorFields[22] = KagemushaNoritoEncodeFields(currentNoteFields, flags);
        topLevelFields[0] = KagemushaNoritoEncodeFields(accumulatorFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithCurrentNoteAmountTrailingField(byte[] archive)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 1);
        var accumulatorFields = KagemushaNoritoReadFieldPayloads(topLevelFields[0], flags);
        Assert.True(accumulatorFields.Count > 22);
        var currentNoteFields = KagemushaNoritoReadFieldPayloads(accumulatorFields[22], flags);
        Assert.True(currentNoteFields.Count >= 3);
        var amountFields = KagemushaNoritoReadFieldPayloads(currentNoteFields[2], flags);
        amountFields.Add(KagemushaNoritoString("unexpected"));
        currentNoteFields[2] = KagemushaNoritoEncodeFields(amountFields, flags);
        accumulatorFields[22] = KagemushaNoritoEncodeFields(currentNoteFields, flags);
        topLevelFields[0] = KagemushaNoritoEncodeFields(accumulatorFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithRecursiveProofTrailingField(byte[] archive)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 2);
        var recursiveProofFields = KagemushaNoritoReadFieldPayloads(topLevelFields[1], flags);
        recursiveProofFields.Add(KagemushaNoritoString("unexpected"));
        topLevelFields[1] = KagemushaNoritoEncodeFields(recursiveProofFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithVerifierKeyIdTrailingField(byte[] archive)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 2);
        var recursiveProofFields = KagemushaNoritoReadFieldPayloads(topLevelFields[1], flags);
        Assert.True(recursiveProofFields.Count >= 1);
        var verifierKeyIdFields = KagemushaNoritoReadFieldPayloads(recursiveProofFields[0], flags);
        verifierKeyIdFields.Add(KagemushaNoritoString("unexpected"));
        recursiveProofFields[0] = KagemushaNoritoEncodeFields(verifierKeyIdFields, flags);
        topLevelFields[1] = KagemushaNoritoEncodeFields(recursiveProofFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] RecursiveSpendBundleWithProofBoxTrailingField(byte[] archive)
    {
        var flags = archive[39];
        var topLevelFields = KagemushaNoritoReadFieldPayloads(
            KagemushaNoritoPayload(archive),
            flags);
        Assert.True(topLevelFields.Count >= 2);
        var recursiveProofFields = KagemushaNoritoReadFieldPayloads(topLevelFields[1], flags);
        Assert.True(recursiveProofFields.Count >= 4);
        var proofBoxFields = KagemushaNoritoReadFieldPayloads(recursiveProofFields[3], flags);
        proofBoxFields.Add(KagemushaNoritoString("unexpected"));
        recursiveProofFields[3] = KagemushaNoritoEncodeFields(proofBoxFields, flags);
        topLevelFields[1] = KagemushaNoritoEncodeFields(recursiveProofFields, flags);
        return RebuildKagemushaNoritoFrameLike(
            archive,
            KagemushaNoritoEncodeFields(topLevelFields, flags));
    }

    private static byte[] KagemushaNoritoEncodeFields(IEnumerable<byte[]> fields, byte flags)
    {
        return fields.SelectMany(field => KagemushaNoritoField(field, flags)).ToArray();
    }

    private static byte[] KagemushaFixedArrayPayload(byte value, int count)
    {
        return Enumerable.Range(0, count)
            .SelectMany(_ => KagemushaNoritoField(new byte[] { value }))
            .ToArray();
    }

    private static byte[] KagemushaCountPrefixedFixedArrayPayload(byte value, int count)
    {
        return U64LE((ulong)count)
            .Concat(KagemushaFixedArrayPayload(value, count))
            .ToArray();
    }

    private static byte[] Fixed32(byte value)
    {
        return Enumerable.Repeat(value, 32).ToArray();
    }

    private static byte[] KagemushaFixed32(byte value)
    {
        return Fixed32(value);
    }

    private static byte[] TopupAnchorNullifierCountPayload(ulong count)
    {
        var payload = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(payload, count);
        return payload;
    }

    private static byte[] TopupAnchorNullifiersPayload(params byte[][] nullifiers)
    {
        return TopupAnchorNullifierCountPayload((ulong)nullifiers.Length)
            .Concat(nullifiers.SelectMany(nullifier => KagemushaNoritoField(nullifier)))
            .ToArray();
    }

    private static byte[][] SortedFixed32(params byte[][] values)
    {
        var sorted = values.Select(value => value.ToArray()).ToArray();
        Array.Sort(sorted, CompareFixed32);
        return sorted;
    }

    private static int CompareFixed32(byte[] left, byte[] right)
    {
        Assert.Equal(32, left.Length);
        Assert.Equal(32, right.Length);
        for (var index = 0; index < 32; index++)
        {
            var comparison = left[index].CompareTo(right[index]);
            if (comparison != 0)
            {
                return comparison;
            }
        }
        return 0;
    }

    private static byte[] NonReservedTransitionProfileOutput(
        KagemushaRecursiveSpendTransitionProfileSummary transitionProfile)
    {
        for (var seed = 0x40; seed <= 0xff; seed++)
        {
            var candidate = Fixed32((byte)seed);
            if (!candidate.SequenceEqual(transitionProfile.CurrentNote.NoteCommitment)
                && !candidate.SequenceEqual(transitionProfile.CurrentNote.SpendNullifier)
                && !transitionProfile.PreviousTopupAnchorNullifiers.Any(
                    nullifier => candidate.SequenceEqual(nullifier))
                && !transitionProfile.CurrentHopOutputCommitments.Any(
                    outputCommitment => candidate.SequenceEqual(outputCommitment)))
            {
                return candidate;
            }
        }
        throw new InvalidOperationException("test helper could not find a non-reserved profile output");
    }

    private static byte[] NonReservedChangeOutput(KagemushaRecursiveSpendBundleSummary bundleSummary)
    {
        for (var seed = 0x40; seed <= 0xff; seed++)
        {
            var candidate = Fixed32((byte)seed);
            if (!candidate.SequenceEqual(bundleSummary.CurrentNote.NoteCommitment)
                && !candidate.SequenceEqual(bundleSummary.CurrentNote.SpendNullifier)
                && !bundleSummary.TopupAnchorNullifiers.Any(nullifier => candidate.SequenceEqual(nullifier)))
            {
                return candidate;
            }
        }
        throw new InvalidOperationException("test helper could not find a non-reserved change output");
    }

    private static List<byte[]> KagemushaNoritoReadFieldPayloads(byte[] payload, byte flags)
    {
        var fields = new List<byte[]>();
        var offset = 0;
        while (offset < payload.Length)
        {
            var length = KagemushaNoritoReadLength(payload, ref offset, flags);
            Assert.True(length <= payload.Length - offset);
            fields.Add(payload.AsSpan(offset, length).ToArray());
            offset += length;
        }
        return fields;
    }

    private static int KagemushaNoritoReadLength(byte[] payload, ref int offset, byte flags)
    {
        if ((flags & KagemushaNoritoCompactLenFlag) == 0)
        {
            Assert.True(offset + 8 <= payload.Length);
            var length = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(offset, 8));
            Assert.True(length <= int.MaxValue);
            offset += 8;
            return (int)length;
        }

        ulong value = 0;
        var shift = 0;
        var start = offset;
        for (var index = 0; index < 10; index++)
        {
            Assert.True(offset < payload.Length);
            var current = payload[offset++];
            var currentValue = current & 0x7f;
            Assert.False(shift >= 63 && currentValue > 1);
            value |= (ulong)currentValue << shift;
            if ((current & 0x80) == 0)
            {
                var encodedLength = offset - start;
                Assert.False(encodedLength > 1 && value < (1UL << (7 * (encodedLength - 1))));
                Assert.True(value <= int.MaxValue);
                return (int)value;
            }
            shift += 7;
        }

        throw new InvalidOperationException("test helper failed to read Norito length");
    }

    private static byte[] KagemushaNoritoLength(int value, byte flags = 0)
    {
        if ((flags & KagemushaNoritoCompactLenFlag) == 0)
        {
            var length = new byte[8];
            BinaryPrimitives.WriteUInt64LittleEndian(length, (ulong)value);
            return length;
        }

        var remaining = (uint)value;
        var bytes = new List<byte>();
        while (remaining >= 0x80)
        {
            bytes.Add((byte)((remaining & 0x7f) | 0x80));
            remaining >>= 7;
        }
        bytes.Add((byte)remaining);
        return bytes.ToArray();
    }

    private static byte[] KagemushaOverlongCompactLength(int value)
    {
        if (value < 0 || value >= 0x80)
        {
            throw new ArgumentException("test helper only encodes small overlong lengths", nameof(value));
        }

        return new byte[] { (byte)(value | 0x80), 0x00 };
    }

    private static byte[] KagemushaOversizedTerminalCompactLength()
    {
        return Enumerable.Repeat((byte)0x80, 9)
            .Concat(new byte[] { 0x02 })
            .ToArray();
    }

    private static byte[] KagemushaHugeCanonicalCompactLength()
    {
        return Enumerable.Repeat((byte)0x80, 9)
            .Concat(new byte[] { 0x01 })
            .ToArray();
    }

    private static byte[] KagemushaNoritoField(byte[] payload, byte flags = KagemushaNoritoCompactLenFlag)
    {
        return KagemushaNoritoLength(payload.Length, flags)
            .Concat(payload)
            .ToArray();
    }

    private static byte[] KagemushaNoritoString(string value, byte flags = KagemushaNoritoCompactLenFlag)
    {
        var bytes = Encoding.UTF8.GetBytes(value);
        return KagemushaNoritoLength(bytes.Length, flags)
            .Concat(bytes)
            .ToArray();
    }

    private static byte[] KagemushaNoritoByteVec(byte[] value)
    {
        var length = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(length, (ulong)value.Length);
        return length.Concat(value).ToArray();
    }

    private static byte[] KagemushaNumericAmountPayload(ulong value, byte flags = KagemushaNoritoCompactLenFlag)
    {
        var mantissaBytes = value == 0
            ? Array.Empty<byte>()
            : BitConverter.GetBytes(value).Reverse().SkipWhile(valueByte => valueByte == 0).Reverse().ToArray();
        var mantissa = new byte[4 + mantissaBytes.Length];
        BinaryPrimitives.WriteUInt32LittleEndian(mantissa.AsSpan(0, 4), (uint)mantissaBytes.Length);
        mantissaBytes.CopyTo(mantissa.AsSpan(4));
        var scale = new byte[4];
        return KagemushaNoritoEncodeFields(new[] { mantissa, scale }, flags);
    }

    private static byte[] KagemushaZk1Tlv(string tag, byte[] payload)
    {
        var tagBytes = Encoding.ASCII.GetBytes(tag);
        var encoded = new byte[8 + payload.Length];
        tagBytes.CopyTo(encoded, 0);
        BinaryPrimitives.WriteUInt32LittleEndian(encoded.AsSpan(4, 4), (uint)payload.Length);
        payload.CopyTo(encoded, 8);
        return encoded;
    }

    private static byte[] KagemushaLineageVerifierKey(string circuitId, byte seed)
    {
        return new byte[] { 0x5a, 0x4b, 0x31, 0x00 }
            .Concat(KagemushaZk1Tlv("IPAK", new byte[] { 8, 0, 0, 0 }))
            .Concat(KagemushaZk1Tlv("CID1", Encoding.UTF8.GetBytes(circuitId)))
            .Concat(KagemushaZk1Tlv("H2VK", Enumerable.Repeat(seed, 32).ToArray()))
            .ToArray();
    }

    private static byte[] KagemushaVerifierKeyCommitment(byte[] verifierKey)
    {
        var backend = Encoding.UTF8.GetBytes(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend);
        var domain = Encoding.ASCII.GetBytes("iroha:zk:v1:vk");
        var preimage = new byte[domain.Length + 8 + backend.Length + 8 + verifierKey.Length];
        var offset = 0;
        domain.AsSpan().CopyTo(preimage.AsSpan(offset));
        offset += domain.Length;
        BinaryPrimitives.WriteUInt64BigEndian(preimage.AsSpan(offset, 8), (ulong)backend.Length);
        offset += 8;
        backend.AsSpan().CopyTo(preimage.AsSpan(offset));
        offset += backend.Length;
        BinaryPrimitives.WriteUInt64BigEndian(preimage.AsSpan(offset, 8), (ulong)verifierKey.Length);
        offset += 8;
        verifierKey.AsSpan().CopyTo(preimage.AsSpan(offset));
        return SHA256.HashData(preimage);
    }

    private static byte[] KagemushaLineageProvingKeyArchive(
        string circuitId,
        byte[] verifierKey,
        byte seed)
    {
        return KagemushaLineageProvingKeyArchiveRaw(
            1,
            circuitId,
            KagemushaVerifierKeyCommitment(verifierKey),
            Enumerable.Repeat(seed, 64).ToArray());
    }

    private static byte[] KagemushaLineageProvingKeyArchiveRaw(
        ushort version,
        string circuitId,
        byte[] verifierKeyCommitment,
        byte[] provingKey,
        byte flags = KagemushaNoritoCompactLenFlag,
        byte[]? schemaHash = null,
        byte[]? trailingPayload = null)
    {
        var versionBytes = new byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(versionBytes, version);
        var payload = KagemushaNoritoField(versionBytes, flags)
            .Concat(KagemushaNoritoField(KagemushaNoritoString(circuitId, flags), flags))
            .Concat(KagemushaNoritoField(verifierKeyCommitment, flags))
            .Concat(KagemushaNoritoField(KagemushaNoritoByteVec(provingKey), flags))
            .Concat(trailingPayload ?? Array.Empty<byte>())
            .ToArray();
        return KagemushaNoritoFrameFromSchemaHash(
            schemaHash ?? KagemushaLineageProvingKeyArchiveSchemaHash,
            payload,
            flags);
    }

    private static byte[] ReadBridgeOutputWithBytes(
        byte[] bytes,
        string symbol = "connect_norito_kagemusha_recursive_spend_redeem")
    {
        var pointer = Marshal.AllocHGlobal(bytes.Length);
        Marshal.Copy(bytes, 0, pointer, bytes.Length);
        return KagemushaRecursiveSpendNative.ReadBridgeOutput(
            symbol,
            0,
            pointer,
            (UIntPtr)bytes.Length,
            Marshal.FreeHGlobal);
    }

    private static void AssertPointerZeroed(IntPtr pointer, int length)
    {
        var observed = new byte[length];
        Marshal.Copy(pointer, observed, 0, observed.Length);
        Assert.True(Array.TrueForAll(observed, value => value == 0));
    }
}
