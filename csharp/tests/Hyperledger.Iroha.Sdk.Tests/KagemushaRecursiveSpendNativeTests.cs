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
        Assert.True(KagemushaRecursiveSpendNative.IsAvailable(() => 6u, () => true));
        Assert.True(KagemushaRecursiveSpendNative.IsAvailable(() => 7u, () => true));
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
        Assert.True(KagemushaRecursiveSpendNative.IsCompactPaymentTokenProverAvailable(
            () => 6u,
            () => true));
        Assert.True(KagemushaRecursiveSpendNative.IsCompactPaymentTokenProverAvailable(
            () => 7u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsCompactPaymentTokenProverAvailable(
            () => 5u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsCompactPaymentTokenProverAvailable(
            () => 6u,
            () => false));
        Assert.True(KagemushaRecursiveSpendNative.IsRecursiveAggregationProofBundleProverAvailable(
            () => 6u,
            () => true));
        Assert.True(KagemushaRecursiveSpendNative.IsRecursiveAggregationProofBundleProverAvailable(
            () => 7u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveAggregationProofBundleProverAvailable(
            () => 5u,
            () => true));
        Assert.False(KagemushaRecursiveSpendNative.IsRecursiveAggregationProofBundleProverAvailable(
            () => 6u,
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
    public void RecursiveSpendNativePreferredModePrefersCompactWhenAvailable()
    {
        Assert.Equal(
            KagemushaOfflineSpendMode.RecursiveCompactV1,
            KagemushaRecursiveSpendNative.PreferredMode(true, true));
        Assert.Equal(
            KagemushaOfflineSpendMode.RecursiveCompactV1,
            KagemushaRecursiveSpendNative.PreferredMode(true, false));
        Assert.Equal(
            KagemushaOfflineSpendMode.RecursiveSpendV1,
            KagemushaRecursiveSpendNative.PreferredMode(false, true));
        Assert.Null(KagemushaRecursiveSpendNative.PreferredMode(false, false));
        Assert.Equal(
            "recursive_compact_v1",
            KagemushaOfflineSpendMode.RecursiveCompactV1.WireName());
        Assert.Equal(
            "recursive_spend_v1",
            KagemushaOfflineSpendMode.RecursiveSpendV1.WireName());
        Assert.DoesNotContain(
            "checked_prefold_v1",
            Enum.GetValues<KagemushaOfflineSpendMode>().Select(mode => mode.WireName()));
        Assert.Equal(6u, KagemushaRecursiveSpendNative.RequiredNativeBridgeAbiVersion);
        Assert.Equal(7u, KagemushaRecursiveSpendNative.RecursiveCompactRequiredNativeBridgeAbiVersion);
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
            "kagemusha-recursive-spend-lineage-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1);
        Assert.Equal(
            "kagemusha-recursive-spend-lineage-onehop-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1);
        Assert.Equal(
            "kagemusha-recursive-spend-lineage-append-v1",
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1);
        Assert.Equal("halo2/ipa", KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend);
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
            string.Empty,
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(null));
        Assert.Equal(
            string.Empty,
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(""));
        Assert.Equal(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(
                KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1));
        Assert.Equal(
            "unknown-kagemusha-recursive-spend-circuit",
            KagemushaRecursiveSpendNative.NormalizeAppendOutputCircuitId(
                "unknown-kagemusha-recursive-spend-circuit"));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(null));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(""));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendOutputCircuitId(
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
                KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
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
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            ""));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1));
        Assert.False(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1));
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedAppendProofTransition(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1));
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
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.CanProveAppendOutputCircuitId(
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
        Assert.False(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.CanSelectAppendOutputCircuitId(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
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
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
            1u));
        Assert.True(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            1u));
        Assert.False(KagemushaRecursiveSpendNative.RequiresPreviousProofOpenEnvelopesForAppend(
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
    public void RecursiveSpendNativeVerifyLineagePreflightRejectsMissingOrDanglingRecordBeforeNativeBridge()
    {
        var malformedRequestArchive = new byte[] { 0xde, 0xad, 0xbe, 0xef };

        AssertArgumentDiagnostic(
            "lineageVerifierRecord is required for reserved-lineage bundles",
            "hasLineageVerifierRecord",
            () => KagemushaRecursiveSpendNative.Verify(
                malformedRequestArchive,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
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
                KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false));

        AssertArgumentDiagnostic(
            "lineageVerifierRecord is required for reserved-lineage bundles",
            "lineageVerifierRecordCount",
            () => KagemushaRecursiveSpendNative.Redeem(
                requestArchive,
                KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                lineageVerifierRecordCount: 0));

        AssertArgumentDiagnostic(
            "lineageVerifierRecords count must be non-negative",
            "lineageVerifierRecordCount",
            () => KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
                KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
                1u,
                hasLineageWitness: true,
                hasLineageVerifierRecord: false,
                lineageVerifierRecordCount: -1));

        KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
            KagemushaRecursiveSpendNative.RecursiveAggregationProofCircuitIdV1,
            1u,
            hasLineageWitness: true,
            hasLineageVerifierRecord: false);
        KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            2u,
            hasLineageWitness: false,
            hasLineageVerifierRecord: true);
        KagemushaRecursiveSpendNative.ValidateRedeemLineagePreflight(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1,
            2u,
            hasLineageWitness: false,
            hasLineageVerifierRecord: false,
            lineageVerifierRecordCount: 2);
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
    public void RecursiveSpendSharedAbi6FixtureMatchesSdkSurface()
    {
        using var manifest = LoadSharedRecursiveSpendManifest();
        var root = manifest.RootElement;

        Assert.Equal(
            "iroha.kagemusha.recursive_spend.abi6.fixture_manifest.v1",
            root.GetProperty("schema").GetString());
        Assert.Equal(
            KagemushaRecursiveSpendNative.RequiredNativeBridgeAbiVersion,
            (uint)root.GetProperty("native_bridge_abi_version").GetInt32());
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
            "1fe949217c8bbe26957cf2a2510d79894e15b20fc5143dee2c3a1ff8678d3a5d",
            redeemArchive.GetProperty("sha256_hex").GetString());
        Assert.True(redeemArchive.GetProperty("byte_len").GetInt32() > 0);
        Assert.NotEmpty(Convert.FromBase64String(
            redeemArchive.GetProperty("bytes_base64").GetString()!));
        Assert.Equal("redeem", redeemInstructionArchive.GetProperty("operation").GetString());
        Assert.Equal(
            "RedeemKagemushaRecursive",
            redeemInstructionArchive.GetProperty("norito_type").GetString());
        Assert.Equal(
            "dd7bcb5ab602696be67028e03578933a93e9396057a5decefe8cc9058662bf85",
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
        Assert.NotEmpty(initBundle.TopupAnchorNullifiers);
        Assert.True(initBundle.TopupAnchorNullifiers.Count <= KagemushaRecursiveSpendNative.FoldStepMaxInputs);
        Assert.All(initBundle.TopupAnchorNullifiers, anchor => Assert.Equal(32, anchor.Length));
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
        var topupAnchor = initBundle.TopupAnchorNullifiers[0];
        var topupAnchorByte = topupAnchor[0];
        topupAnchor[0] ^= 0xff;
        Assert.Equal(topupAnchorByte, initBundle.TopupAnchorNullifiers[0][0]);

        var appendBundle = KagemushaRecursiveSpendNative.DecodeBundleSummary(
            SharedRecursiveSpendArchive("append_bundle"));
        Assert.True(appendBundle.HopCount >= 1);
        Assert.True(KagemushaRecursiveSpendNative.IsSupportedPreviousProofCircuitId(
            appendBundle.ProofCircuitId));
        Assert.Equal(32, appendBundle.InitialRoot.Length);
        Assert.Equal(32, appendBundle.FinalRoot.Length);
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
        Assert.True(KagemushaRecursiveSpendNative.LineageWitnessHasReservedPreviousProof(
            SharedRecursiveSpendArchive("lineage_witness_append_result")));

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
                Archive: RecursiveSpendLineageWitnessWithPreviousProofCountPrefixOnly(
                    (ulong)KagemushaRecursiveSpendNative.CompactTokenMaxHops + 1UL),
                ExpectedField: $"lineageWitness.previousRecursiveProofs count must not exceed {KagemushaRecursiveSpendNative.CompactTokenMaxHops}"
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
        })
        {
            AssertBundleSummaryRejects(
                RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    malformedAccumulatorField.FieldIndex,
                    malformedAccumulatorField.Replacement),
                malformedAccumulatorField.ExpectedField);
        }

        var currentNoteCommitment = RecursiveSpendBundleCurrentNoteFieldPayload(initBundleArchive, 0);
        var currentNoteSpendNullifier = RecursiveSpendBundleCurrentNoteFieldPayload(initBundleArchive, 1);
        foreach (var malformedTopupAnchorNullifiers in new[]
        {
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    KagemushaUInt64Payload(0)),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers count is out of range"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    KagemushaUInt64Payload((ulong)KagemushaRecursiveSpendNative.FoldStepMaxInputs + 1UL)),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers count is out of range"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    KagemushaTopupAnchorNullifiersPayload(new byte[32])),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must not contain zero values"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    KagemushaTopupAnchorNullifiersPayload(
                        KagemushaFixed32(0x21),
                        KagemushaFixed32(0x21))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    KagemushaTopupAnchorNullifiersPayload(
                        KagemushaFixed32(0x31),
                        KagemushaFixed32(0x21))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    KagemushaTopupAnchorNullifiersPayload(currentNoteCommitment)),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    KagemushaTopupAnchorNullifiersPayload(currentNoteSpendNullifier)),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    new byte[] { 0x01, 0x02 }),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers count is truncated"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    KagemushaTopupAnchorNullifiersPayload(KagemushaFixedArrayPayload(0x21, 31))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers[0] must be exactly 32 bytes"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    KagemushaTopupAnchorNullifiersPayload(
                        KagemushaCountPrefixedFixedArrayPayload(0x21, 32))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers[0] byte field length must be 1"
            ),
            (
                Archive: RecursiveSpendBundleWithAccumulatorField(
                    initBundleArchive,
                    5,
                    KagemushaTopupAnchorNullifiersPayload(KagemushaFixedArrayPayload(0x21, 33))),
                ExpectedField: "bundle.accumulator.topup_anchor_nullifiers[0] must be exactly 32 bytes"
            ),
        })
        {
            AssertBundleSummaryRejects(
                malformedTopupAnchorNullifiers.Archive,
                malformedTopupAnchorNullifiers.ExpectedField);
        }

        var zeroTopupAnchorBundle = RecursiveSpendBundleWithAccumulatorField(
            initBundleArchive,
            5,
            KagemushaTopupAnchorNullifiersPayload(new byte[32]));
        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithEmptyProofBytes(zeroTopupAnchorBundle),
            "bundle.accumulator.topup_anchor_nullifiers must not contain zero values");
        AssertBundleSummaryRejects(
            RecursiveSpendBundleWithAccumulatorTrailingField(zeroTopupAnchorBundle),
            "bundle.accumulator.topup_anchor_nullifiers must not contain zero values");

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
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            Array.Empty<byte>(),
            "must not be empty.");
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
        var validRecordBundleArchive = KagemushaRecordBundleArchiveWithStepCount(1);
        var validPallasOpenEnvelopesArchive = KagemushaPallasOpenEnvelopesArchiveWithCount(1);
        AssertArgumentDiagnostic(
            "Record bundle archive must be a valid Norito archive.",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    new byte[] { 0x01, 0x02 },
                    validPallasOpenEnvelopesArchive));

        AssertArgumentDiagnostic(
            "Pallas open-envelopes archive must be a valid Norito archive.",
            "pallasOpenEnvelopesArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundleArchive,
                    new byte[] { 0x01, 0x02 }));
    }

    [Fact]
    public void RecursiveAggregationProverRejectsOversizedInputsBeforeLoadingNativeBridge()
    {
        var validRecordBundleArchive = KagemushaRecordBundleArchiveWithStepCount(1);
        var validPallasOpenEnvelopesArchive = KagemushaPallasOpenEnvelopesArchiveWithCount(1);
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
                    validRecordBundleArchive,
                    oversizedArchive),
            "Pallas open-envelopes archive must not exceed",
            "pallasOpenEnvelopesArchive");
    }

    [Fact]
    public void RecursiveAggregationProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge()
    {
        var validRecordBundleArchive = KagemushaRecordBundleArchiveWithStepCount(1);
        var validPallasOpenEnvelopesArchive = KagemushaPallasOpenEnvelopesArchiveWithCount(1);
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
                    validRecordBundleArchive,
                    emptyPayloadArchive));
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
        var validRecordBundleArchive = KagemushaRecordBundleArchiveWithStepCount(1);
        var validPallasOpenEnvelopesArchive = KagemushaPallasOpenEnvelopesArchiveWithCount(1);
        AssertArgumentDiagnostic(
            "Record bundle archive must be a valid Norito archive.",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    new byte[] { 0x01, 0x02 },
                    validPallasOpenEnvelopesArchive,
                    validArchive));

        AssertArgumentDiagnostic(
            "Pallas open-envelopes archive must be a valid Norito archive.",
            "pallasOpenEnvelopesArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundleArchive,
                    new byte[] { 0x01, 0x02 },
                    validArchive));

        AssertArgumentDiagnostic(
            "Recursive compact key artifacts archive must be a valid Norito archive.",
            "recursiveCompactKeyArtifactsArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundleArchive,
                    validPallasOpenEnvelopesArchive,
                    new byte[] { 0x01, 0x02 }));
    }

    [Fact]
    public void RecursiveCompactProverRejectsOversizedInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundleArchive = KagemushaRecordBundleArchiveWithStepCount(1);
        var validPallasOpenEnvelopesArchive = KagemushaPallasOpenEnvelopesArchiveWithCount(1);
        var oversizedArchive = OversizedKagemushaArchive();
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    oversizedArchive,
                    validPallasOpenEnvelopesArchive,
                    validArchive),
            "Record bundle archive must not exceed",
            "recordBundleArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundleArchive,
                    oversizedArchive,
                    validArchive),
            "Pallas open-envelopes archive must not exceed",
            "pallasOpenEnvelopesArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundleArchive,
                    validPallasOpenEnvelopesArchive,
                    oversizedArchive),
            "Recursive compact key artifacts archive must not exceed",
            "recursiveCompactKeyArtifactsArchive");
    }

    [Fact]
    public void RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var validRecordBundleArchive = KagemushaRecordBundleArchiveWithStepCount(1);
        var validPallasOpenEnvelopesArchive = KagemushaPallasOpenEnvelopesArchiveWithCount(1);
        var emptyPayloadArchive = KagemushaNoritoFrame(0x4b);
        AssertArgumentDiagnostic(
            "Record bundle archive must contain a non-empty Norito payload.",
            "recordBundleArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    emptyPayloadArchive,
                    validPallasOpenEnvelopesArchive,
                    validArchive));

        AssertArgumentDiagnostic(
            "Pallas open-envelopes archive must contain a non-empty Norito payload.",
            "pallasOpenEnvelopesArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundleArchive,
                    emptyPayloadArchive,
                    validArchive));

        AssertArgumentDiagnostic(
            "Recursive compact key artifacts archive must contain a non-empty Norito payload.",
            "recursiveCompactKeyArtifactsArchive",
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validRecordBundleArchive,
                    validPallasOpenEnvelopesArchive,
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
        var output = ReadBridgeOutputWithBytes(archive);

        Assert.Equal(archive, output);
    }

    [Fact]
    public void RecursiveSpendNativeRejectsMalformedArchivesBeforeLoadingNativeBridge()
    {
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            new byte[] { 0x01, 0x02 },
            "must be a valid Norito archive.");

        var compressed = KagemushaNoritoFrameWithPayload(0x4b);
        compressed[22] = 1;
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            compressed,
            "must be a valid Norito archive.");

        var unsupportedFlags = KagemushaNoritoFrameWithPayload(0x4b);
        unsupportedFlags[39] = 0x08;
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            unsupportedFlags,
            "must be a valid Norito archive.");

        var invalidFieldBitset = KagemushaNoritoFrameWithPayload(0x4b);
        invalidFieldBitset[39] = 0x20;
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            invalidFieldBitset,
            "must be a valid Norito archive.");

        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[] { 0x7f }),
            "must be a valid Norito archive.");
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            WithHeaderPadding(KagemushaNoritoFrameWithPayload(0x4b), new byte[65]),
            "must be a valid Norito archive.");
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
        AssertRecursiveSpendNativeArchivePreflightRejectsEverywhere(
            KagemushaNoritoFrame(0x4b),
            "must contain a non-empty Norito payload.");
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
                || error.Message[expectedMessage.Length] == ' '),
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

    private static byte[] KagemushaInitRequestArchiveWithRecordBundle(byte[] recordBundlePayload)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendInitRequestWireName),
            KagemushaNoritoField(recordBundlePayload),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaInitRequestArchiveWithRecordBundleAndPallas(
        byte[] recordBundlePayload,
        byte[] pallasOpenEnvelopesArchive)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendInitRequestWireName),
            KagemushaNoritoField(recordBundlePayload)
                .Concat(KagemushaNoritoField(KagemushaNoritoByteVec(pallasOpenEnvelopesArchive)))
                .ToArray(),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaAppendRequestArchiveWithRecordBundle(byte[] recordBundlePayload)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendAppendRequestWireName),
            KagemushaNoritoField(new byte[] { 0x01 })
                .Concat(KagemushaNoritoField(recordBundlePayload))
                .ToArray(),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaAppendRequestArchiveWithRecordBundleAndPallas(
        byte[] recordBundlePayload,
        byte[] pallasOpenEnvelopesArchive)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendAppendRequestWireName),
            KagemushaNoritoField(new byte[] { 0x01 })
                .Concat(KagemushaNoritoField(recordBundlePayload))
                .Concat(KagemushaNoritoField(KagemushaNoritoByteVec(pallasOpenEnvelopesArchive)))
                .ToArray(),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaFullAppendRequestArchive(
        byte[] previousBundleArchive,
        string outputProofCircuitId,
        byte[]? previousLineageVerifierRecordPayload = null,
        byte[]? previousProofOpenEnvelopesArchive = null,
        byte[]? lineageVerifierKeyPayload = null,
        byte[]? lineageProvingKeyArchivePayload = null)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendAppendRequestWireName),
            KagemushaNoritoField(KagemushaNoritoPayload(previousBundleArchive))
                .Concat(KagemushaNoritoField(KagemushaRecordBundlePayloadWithStepCount(1)))
                .Concat(KagemushaNoritoField(
                    KagemushaNoritoByteVec(KagemushaPallasOpenEnvelopesArchiveWithCount(1))))
                .Concat(KagemushaNoritoField(KagemushaSpendableNotePayload()))
                .Concat(KagemushaNoritoField(KagemushaNoritoString(outputProofCircuitId)))
                .Concat(KagemushaNoritoField(KagemushaNoritoOptionPayload(
                    previousLineageVerifierRecordPayload)))
                .Concat(KagemushaNoritoField(KagemushaNoritoByteVec(
                    previousProofOpenEnvelopesArchive ?? Array.Empty<byte>())))
                .Concat(KagemushaNoritoField(KagemushaNoritoOptionPayload(
                    lineageVerifierKeyPayload)))
                .Concat(KagemushaNoritoField(KagemushaNoritoOptionPayload(
                    lineageProvingKeyArchivePayload)))
                .Concat(KagemushaNoritoField(KagemushaNoritoOptionPayload(null)))
                .ToArray(),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaSpendableNotePayload(
        byte noteCommitmentSeed = 0x31,
        byte spendNullifierSeed = 0x32,
        ulong amount = 7,
        byte flags = KagemushaNoritoCompactLenFlag)
    {
        return KagemushaNoritoEncodeFields(
            new[]
            {
                KagemushaFixed32(noteCommitmentSeed),
                KagemushaFixed32(spendNullifierSeed),
                KagemushaNumericAmountPayload(amount),
            },
            flags);
    }

    private static byte[] KagemushaRecordBundleArchiveWithStepsPayload(byte[] stepsPayload)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendRecordBundleWireName),
            KagemushaRecordBundlePayloadWithStepsPayload(stepsPayload),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaRecordBundleArchiveWithStepCount(int stepCount)
    {
        return KagemushaNoritoFrameFromSchemaHash(
            NoritoCodec.SchemaHash(KagemushaRecursiveSpendNative.RecursiveSpendRecordBundleWireName),
            KagemushaRecordBundlePayloadWithStepCount(stepCount),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaRecordBundlePayloadWithStepsPayload(byte[] stepsPayload)
    {
        var bundlePayload = KagemushaNoritoEncodeFields(
            new[]
            {
                new byte[] { 0x41 },
                new byte[] { 0x42 },
                stepsPayload,
            },
            KagemushaNoritoCompactLenFlag);
        return KagemushaNoritoEncodeFields(
            new[]
            {
                bundlePayload,
                Array.Empty<byte>(),
            },
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaRecordBundlePayloadWithStepCount(int stepCount)
    {
        var stepPayload = KagemushaNoritoEncodeFields(
            Enumerable.Repeat(new byte[] { 0x51 }, 6),
            KagemushaNoritoCompactLenFlag);
        var stepsPayload = KagemushaUInt64Payload((ulong)stepCount)
            .Concat(Enumerable.Range(0, stepCount).SelectMany(_ => KagemushaNoritoField(stepPayload)))
            .ToArray();
        return KagemushaRecordBundlePayloadWithStepsPayload(stepsPayload);
    }

    private static byte[] KagemushaPallasOpenEnvelopesArchiveWithCount(int envelopeCount)
    {
        var payload = KagemushaUInt64Payload((ulong)envelopeCount)
            .Concat(Enumerable.Range(0, envelopeCount)
                .SelectMany(_ => KagemushaNoritoField(KagemushaPallasOpenEnvelopePayload())))
            .ToArray();
        return KagemushaNoritoFrameFromSchemaHash(
            KagemushaPallasOpenEnvelopesSchemaHash,
            payload,
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaPallasOpenEnvelopesArchiveWithEnvelope(byte[] envelopePayload)
    {
        var payload = KagemushaUInt64Payload(1)
            .Concat(KagemushaNoritoField(envelopePayload))
            .ToArray();
        return KagemushaNoritoFrameFromSchemaHash(
            KagemushaPallasOpenEnvelopesSchemaHash,
            payload,
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] KagemushaPallasOpenEnvelopePayload(
        byte[]? paramsPayload = null,
        byte[]? publicPayload = null,
        byte[]? proofPayload = null,
        byte[]? transcriptLabel = null,
        byte[]? vkCommitment = null,
        byte[]? publicInputsSchemaHash = null,
        byte[]? domainTag = null)
    {
        return KagemushaNoritoField(paramsPayload ?? KagemushaPallasIpaParamsPayload())
            .Concat(KagemushaNoritoField(publicPayload ?? KagemushaPallasPolyOpenPublicPayload()))
            .Concat(KagemushaNoritoField(proofPayload ?? KagemushaPallasIpaProofPayload()))
            .Concat(KagemushaNoritoField(transcriptLabel ?? KagemushaNoritoString("pallas-open")))
            .Concat(KagemushaNoritoField(vkCommitment ?? KagemushaPallasMetadataOption(KagemushaFixed32(0x21))))
            .Concat(KagemushaNoritoField(publicInputsSchemaHash ?? KagemushaPallasMetadataOption(KagemushaFixed32(0x22))))
            .Concat(KagemushaNoritoField(domainTag ?? KagemushaPallasMetadataOption(KagemushaFixed32(0x23))))
            .ToArray();
    }

    private static byte[] KagemushaPallasIpaParamsPayload(
        byte[]? gPayload = null,
        byte[]? hPayload = null)
    {
        return KagemushaNoritoField(KagemushaUInt16Payload(1))
            .Concat(KagemushaNoritoField(KagemushaUInt16Payload(1)))
            .Concat(KagemushaNoritoField(KagemushaUInt32Payload(2)))
            .Concat(KagemushaNoritoField(gPayload ?? KagemushaFixed32SequencePayload(2, 0x31)))
            .Concat(KagemushaNoritoField(hPayload ?? KagemushaFixed32SequencePayload(2, 0x41)))
            .Concat(KagemushaNoritoField(KagemushaFixed32(0x51)))
            .ToArray();
    }

    private static byte[] KagemushaPallasPolyOpenPublicPayload()
    {
        return KagemushaNoritoField(KagemushaUInt16Payload(1))
            .Concat(KagemushaNoritoField(KagemushaUInt16Payload(1)))
            .Concat(KagemushaNoritoField(KagemushaUInt32Payload(2)))
            .Concat(KagemushaNoritoField(KagemushaFixed32(0x61)))
            .Concat(KagemushaNoritoField(KagemushaFixed32(0x62)))
            .Concat(KagemushaNoritoField(KagemushaFixed32(0x63)))
            .ToArray();
    }

    private static byte[] KagemushaPallasIpaProofPayload(
        byte[]? lPayload = null,
        byte[]? rPayload = null)
    {
        return KagemushaNoritoField(KagemushaUInt16Payload(1))
            .Concat(KagemushaNoritoField(lPayload ?? KagemushaFixed32SequencePayload(1, 0x71)))
            .Concat(KagemushaNoritoField(rPayload ?? KagemushaFixed32SequencePayload(1, 0x81)))
            .Concat(KagemushaNoritoField(KagemushaFixed32(0x91)))
            .Concat(KagemushaNoritoField(KagemushaFixed32(0x92)))
            .ToArray();
    }

    private static byte[] KagemushaFixed32SequencePayload(int count, byte seed)
    {
        return KagemushaUInt64Payload((ulong)count)
            .Concat(Enumerable.Range(0, count).Select(index => (byte)(seed + index))
                .SelectMany(value => KagemushaNoritoField(KagemushaFixed32(value))))
            .ToArray();
    }

    private static byte[] KagemushaPallasMetadataOption(byte[]? value)
    {
        return KagemushaNoritoOptionPayload(value);
    }

    private static byte[] KagemushaNoritoOptionPayload(byte[]? value)
    {
        return value is null
            ? new byte[] { 0 }
            : new byte[] { 1 }.Concat(KagemushaNoritoField(value)).ToArray();
    }

    private static byte[] KagemushaFixed32(byte value)
    {
        return Enumerable.Repeat(value, 32).ToArray();
    }

    private static byte[] KagemushaUInt16Payload(ushort value)
    {
        var payload = new byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(payload, value);
        return payload;
    }

    private static byte[] KagemushaUInt32Payload(uint value)
    {
        var payload = new byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(payload, value);
        return payload;
    }

    private static byte[] KagemushaUInt64Payload(ulong value)
    {
        var payload = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(payload, value);
        return payload;
    }

    private static void AssertBundleSummaryRejects(byte[] archive, string expectedField)
    {
        AssertArgumentDiagnostic(
            expectedField,
            "bundleArchive",
            () => KagemushaRecursiveSpendNative.DecodeBundleSummary(archive));
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

    private static byte[] RecursiveSpendBundleCurrentNoteFieldPayload(byte[] archive, int fieldIndex)
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
        return currentNoteFields[fieldIndex].ToArray();
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
        return KagemushaUInt64Payload((ulong)count)
            .Concat(KagemushaFixedArrayPayload(value, count))
            .ToArray();
    }

    private static byte[] KagemushaTopupAnchorNullifiersPayload(params byte[][] anchorPayloads)
    {
        return KagemushaUInt64Payload((ulong)anchorPayloads.Length)
            .Concat(anchorPayloads.SelectMany(anchorPayload => KagemushaNoritoField(anchorPayload)))
            .ToArray();
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
}
