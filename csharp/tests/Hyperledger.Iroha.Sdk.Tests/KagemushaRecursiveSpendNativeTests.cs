using System.Collections.Generic;
using System.Buffers.Binary;
using System.IO;
using System.Linq;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
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
    public void RecursiveSpendNativePreferredModeDefaultsToRecursiveWhenAvailable()
    {
        Assert.Equal(
            KagemushaOfflineSpendMode.RecursiveSpendV1,
            KagemushaRecursiveSpendNative.PreferredMode(true, true));
        Assert.Equal(
            KagemushaOfflineSpendMode.CheckedPrefoldV1,
            KagemushaRecursiveSpendNative.PreferredMode(true, false));
        Assert.Equal(
            KagemushaOfflineSpendMode.RecursiveSpendV1,
            KagemushaRecursiveSpendNative.PreferredMode(false, true));
        Assert.Equal(
            KagemushaOfflineSpendMode.RecursiveSpendV1,
            KagemushaRecursiveSpendNative.PreferredMode(true));
        Assert.Equal(
            KagemushaOfflineSpendMode.CheckedPrefoldV1,
            KagemushaRecursiveSpendNative.PreferredMode(false));
        Assert.Equal(
            "recursive_compact_v1",
            KagemushaOfflineSpendMode.RecursiveCompactV1.WireName());
        Assert.Equal(
            "recursive_spend_v1",
            KagemushaOfflineSpendMode.RecursiveSpendV1.WireName());
        Assert.Equal(
            "checked_prefold_v1",
            KagemushaOfflineSpendMode.CheckedPrefoldV1.WireName());
        Assert.Equal(6u, KagemushaRecursiveSpendNative.RequiredBridgeAbiVersion);
        Assert.Equal(7u, KagemushaRecursiveSpendNative.RecursiveCompactRequiredBridgeAbiVersion);
        Assert.Equal(
            "kagemusha-recursive-compact-v1",
            KagemushaRecursiveSpendNative.RecursiveCompactCircuitIdV1);
        Assert.Throws<ArgumentException>(
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(Array.Empty<byte>()));
        var malformedCompactToken = Assert.Throws<ArgumentException>(
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(new byte[] { 0x01 }));
        Assert.Contains("valid Norito archive", malformedCompactToken.Message);
        var emptyPayloadCompactToken = Assert.Throws<ArgumentException>(
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(
                KagemushaNoritoFrame(0x4b)));
        Assert.Contains("non-empty Norito payload", emptyPayloadCompactToken.Message);
        Assert.Throws<ArgumentException>(
            () => KagemushaRecursiveSpendNative.VerifyRecursiveSpendCompactPaymentTokenProjection(
                Array.Empty<byte>(),
                KagemushaNoritoFrameWithPayload(0x4b)));
        var malformedVerifierRecord = Assert.Throws<ArgumentException>(
            () => KagemushaRecursiveSpendNative.VerifyRecursiveSpendCompactPaymentTokenProjection(
                KagemushaNoritoFrameWithPayload(0x4b),
                new byte[] { 0x01 }));
        Assert.Contains("Verifier record archive must be a valid Norito archive", malformedVerifierRecord.Message);
        var emptyPayloadVerifierRecord = Assert.Throws<ArgumentException>(
            () => KagemushaRecursiveSpendNative.VerifyRecursiveSpendCompactPaymentTokenProjection(
                KagemushaNoritoFrameWithPayload(0x4b),
                KagemushaNoritoFrame(0x4b)));
        Assert.Contains("Verifier record archive must contain a non-empty Norito payload", emptyPayloadVerifierRecord.Message);
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
        Assert.Contains(
            "lineage_verifier_key",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    appendVerifierKey,
                    appendProvingKeyArchive)).Message);
        Assert.Contains(
            "lineage_proving_key_archive",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    initVerifierKey,
                    appendProvingKeyArchive)).Message);
        Assert.Contains(
            "lineage_verifier_key",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    Encoding.ASCII.GetBytes("not-zk1"),
                    initProvingKeyArchive)).Message);
        var duplicateCidVerifierKey = initVerifierKey
            .Concat(KagemushaZk1Tlv(
                "CID1",
                Encoding.UTF8.GetBytes(
                    KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1)))
            .ToArray();
        Assert.Contains(
            "lineage_verifier_key",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    duplicateCidVerifierKey,
                    initProvingKeyArchive)).Message);
        Assert.Contains(
            "lineage_proving_key_archive",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    initVerifierKey,
                    Encoding.ASCII.GetBytes("not-norito"))).Message);
        var missingCircuitArchive = KagemushaNoritoFrameFromPayload(
            0x9a,
            Encoding.ASCII.GetBytes("package")
                .Concat(KagemushaVerifierKeyCommitment(initVerifierKey))
                .Concat(Enumerable.Repeat((byte)0xa5, 64))
                .ToArray());
        Assert.Contains(
            "lineage_proving_key_archive",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    initVerifierKey,
                    missingCircuitArchive)).Message);
        var wrongCommitmentArchive = KagemushaLineageProvingKeyArchive(
            KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1,
            appendVerifierKey,
            0xa6);
        Assert.Contains(
            "lineage_proving_key_archive",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    initVerifierKey,
                    wrongCommitmentArchive)).Message);
        Assert.Contains(
            "lineage_proving_key_archive",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    initVerifierKey,
                    KagemushaNoritoFrame(0x9a))).Message);
        Assert.Contains(
            "lineage_key_artifacts",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.ValidateLineageKeyArtifacts(null)).Message);
        Assert.Contains(
            "proof_circuit_id",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifacts(
                    KagemushaRecursiveSpendNative.RecursiveSpendLineageProofCircuitIdV1,
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    new byte[] { 1 },
                    new byte[] { 2 })).Message);
        Assert.Contains(
            "proof_circuit_id",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifacts(
                    "unknown-kagemusha-recursive-spend-circuit",
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    new byte[] { 1 },
                    new byte[] { 2 })).Message);
        Assert.Contains(
            "verifier_opening_len",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    3,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    new byte[] { 1 },
                    new byte[] { 2 })).Message);
        Assert.Contains(
            "lineage_verifier_key",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    "halo2/kzg",
                    new byte[] { 1 },
                    new byte[] { 2 })).Message);
        Assert.Contains(
            "lineage_verifier_key",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    Array.Empty<byte>(),
                    new byte[] { 2 })).Message);
        Assert.Contains(
            "lineage_proving_key_archive",
            Assert.Throws<ArgumentException>(
                () => KagemushaRecursiveSpendNative.LineageKeyArtifactsForInit(
                    128,
                    KagemushaRecursiveSpendNative.RecursiveAggregationProofBackend,
                    new byte[] { 1 },
                    Array.Empty<byte>())).Message);
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
        Assert.Contains("invalid boolean output 2", invalidBoolean.Message);

        var bridgeError = Assert.Throws<InvalidOperationException>(() =>
            KagemushaRecursiveSpendNative.NormalizeRecursiveCompactVerifierOutput(symbol, -311, 0));
        Assert.Contains("bridge error code -311", bridgeError.Message);

        var unavailable = Assert.Throws<InvalidOperationException>(() =>
            KagemushaRecursiveSpendNative.NormalizeRecursiveCompactVerifierOutput(
                symbol,
                KagemushaRecursiveSpendNative.RecursiveCompactUnavailableBridgeErrorCode,
                0));
        Assert.Contains("recursive compact proof composition", unavailable.Message);
        Assert.Contains("-312", unavailable.Message);
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

            var emptyError = Assert.Throws<ArgumentException>(() => factory(Array.Empty<byte>()));
            Assert.Contains("must not be empty", emptyError.Message);
            Assert.Equal("noritoBytes", emptyError.ParamName);

            var oversizedError = Assert.Throws<ArgumentException>(() => factory(oversizedArchive));
            Assert.Contains("must not exceed", oversizedError.Message);
            Assert.Equal("noritoBytes", oversizedError.ParamName);

            var invalidError = Assert.Throws<ArgumentException>(() => factory(new byte[] { 0x01 }));
            Assert.Contains("valid Norito V1 archive", invalidError.Message);
            Assert.Equal("noritoBytes", invalidError.ParamName);

            var emptyPayloadError =
                Assert.Throws<ArgumentException>(() => factory(KagemushaNoritoFrame(0x4b)));
            Assert.Contains("non-empty Norito payload", emptyPayloadError.Message);
            Assert.Equal("noritoBytes", emptyPayloadError.ParamName);
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
            "f5a4a6a25fd9bfd8a121893ddb0c977753c16d8b9dfd835477d2965957c7c03e",
            redeemArchive.GetProperty("sha256_hex").GetString());
        Assert.True(redeemArchive.GetProperty("byte_len").GetInt32() > 0);
        Assert.NotEmpty(Convert.FromBase64String(
            redeemArchive.GetProperty("bytes_base64").GetString()!));
        Assert.Equal("redeem", redeemInstructionArchive.GetProperty("operation").GetString());
        Assert.Equal(
            "RedeemKagemushaRecursive",
            redeemInstructionArchive.GetProperty("norito_type").GetString());
        Assert.Equal(
            "88f293dccb455b6fbcd85d7c06426ce45f02a42fc330e68afda490d504903c03",
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
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);

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
                    validArchive,
                    Array.Empty<byte>()));
    }

    [Fact]
    public void CompactTokenProverRejectsMalformedInputsBeforeLoadingNativeBridge()
    {
        var malformed = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(
                new byte[] { 0x01, 0x02 }));
        Assert.Contains("Record bundle archive must be a valid Norito archive", malformed.Message);
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
        var emptyPayload = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative.ProveVerifiedCompactPaymentTokenWithRecords(
                KagemushaNoritoFrame(0x4b)));
        Assert.Contains("Record bundle archive must contain a non-empty Norito payload", emptyPayload.Message);
    }

    [Fact]
    public void RecursiveAggregationProverRejectsMalformedInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var recordBundle = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    new byte[] { 0x01, 0x02 },
                    validArchive));
        Assert.Contains("Record bundle archive must be a valid Norito archive", recordBundle.Message);

        var pallasOpenEnvelopes = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    new byte[] { 0x01, 0x02 }));
        Assert.Contains("Pallas open-envelopes archive must be a valid Norito archive", pallasOpenEnvelopes.Message);
    }

    [Fact]
    public void RecursiveAggregationProverRejectsOversizedInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var oversizedArchive = OversizedKagemushaArchive();
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    oversizedArchive,
                    validArchive),
            "Record bundle archive must not exceed",
            "recordBundleArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    oversizedArchive),
            "Pallas open-envelopes archive must not exceed",
            "pallasOpenEnvelopesArchive");
    }

    [Fact]
    public void RecursiveAggregationProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var emptyPayloadArchive = KagemushaNoritoFrame(0x4b);
        var recordBundle = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    emptyPayloadArchive,
                    validArchive));
        Assert.Contains("Record bundle archive must contain a non-empty Norito payload", recordBundle.Message);

        var pallasOpenEnvelopes = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    emptyPayloadArchive));
        Assert.Contains(
            "Pallas open-envelopes archive must contain a non-empty Norito payload",
            pallasOpenEnvelopes.Message);
    }

    [Fact]
    public void RecursiveCompactProverRejectsMalformedInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var recordBundle = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    new byte[] { 0x01, 0x02 },
                    validArchive));
        Assert.Contains("Record bundle archive must be a valid Norito archive", recordBundle.Message);

        var pallasOpenEnvelopes = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    new byte[] { 0x01, 0x02 }));
        Assert.Contains("Pallas open-envelopes archive must be a valid Norito archive", pallasOpenEnvelopes.Message);
    }

    [Fact]
    public void RecursiveCompactProverRejectsOversizedInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var oversizedArchive = OversizedKagemushaArchive();
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    oversizedArchive,
                    validArchive),
            "Record bundle archive must not exceed",
            "recordBundleArchive");
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    oversizedArchive),
            "Pallas open-envelopes archive must not exceed",
            "pallasOpenEnvelopesArchive");
    }

    [Fact]
    public void RecursiveCompactProverRejectsEmptyPayloadInputsBeforeLoadingNativeBridge()
    {
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var emptyPayloadArchive = KagemushaNoritoFrame(0x4b);
        var recordBundle = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    emptyPayloadArchive,
                    validArchive));
        Assert.Contains("Record bundle archive must contain a non-empty Norito payload", recordBundle.Message);

        var pallasOpenEnvelopes = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
                    validArchive,
                    emptyPayloadArchive));
        Assert.Contains(
            "Pallas open-envelopes archive must contain a non-empty Norito payload",
            pallasOpenEnvelopes.Message);
    }

    [Fact]
    public void RecursiveSpendCompactProjectionRejectsInvalidBundleBeforeLoadingNativeBridge()
    {
        var malformed = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .RecursiveSpendCompactPaymentTokenFromBundle(new byte[] { 0x01, 0x02 }));
        Assert.Contains("Recursive spend bundle archive must be a valid Norito archive", malformed.Message);

        var emptyPayload = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .RecursiveSpendCompactPaymentTokenFromBundle(KagemushaNoritoFrame(0x4c)));
        Assert.Contains(
            "Recursive spend bundle archive must contain a non-empty Norito payload",
            emptyPayload.Message);

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

        var malformedCompactToken = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .VerifyRecursiveSpendCompactPaymentTokenProjection(new byte[] { 0x01, 0x02 }, validArchive));
        Assert.Contains("Compact token archive must be a valid Norito archive", malformedCompactToken.Message);

        var malformedVerifierRecord = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .VerifyRecursiveSpendCompactPaymentTokenProjection(validArchive, new byte[] { 0x01, 0x02 }));
        Assert.Contains("Verifier record archive must be a valid Norito archive", malformedVerifierRecord.Message);

        var emptyPayloadVerifierRecord = Assert.Throws<ArgumentException>(() =>
            KagemushaRecursiveSpendNative
                .VerifyRecursiveSpendCompactPaymentTokenProjection(validArchive, KagemushaNoritoFrame(0x4b)));
        Assert.Contains(
            "Verifier record archive must contain a non-empty Norito payload",
            emptyPayloadVerifierRecord.Message);

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

        Assert.Contains("connect_norito_kagemusha_recursive_spend_redeem", error.Message);
        Assert.Contains("-311", error.Message);
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

        Assert.Contains("recursive compact proof composition", error.Message);
        Assert.Contains("-312", error.Message);
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
    public void RecursiveSpendNativeReadBridgeOutputRejectsMalformedNoritoSuccessOutput()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            ReadBridgeOutputWithBytes(new byte[] { 0x01 }));

        Assert.Contains("invalid Norito archive", error.Message);
    }

    [Fact]
    public void RecursiveSpendNativeReadBridgeOutputRejectsEmptyPayloadNoritoSuccessOutput()
    {
        var error = Assert.Throws<InvalidOperationException>(() =>
            ReadBridgeOutputWithBytes(KagemushaNoritoFrame(0x4b)));

        Assert.Contains("empty Norito payload", error.Message);
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
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var malformed = new byte[] { 0x01, 0x02 };
        Assert.Throws<ArgumentException>(() => KagemushaRecursiveSpendNative.Init(malformed));
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
                    validArchive,
                    malformed));
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
        var validArchive = KagemushaNoritoFrameWithPayload(0x4b);
        var emptyPayloadArchive = KagemushaNoritoFrame(0x4b);
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
                    validArchive,
                    emptyPayloadArchive));
    }

    [Fact]
    public void RecursiveCompactVerifierRejectsOversizedInputBeforeLoadingNativeBridge()
    {
        var oversizedArchive = OversizedKagemushaArchive();
        AssertOversizedArchive(
            () => KagemushaRecursiveSpendNative.VerifyRecursiveCompactPaymentToken(oversizedArchive),
            "Compact token archive must not exceed",
            "compactTokenArchive");
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
        var error = Assert.Throws<ArgumentException>(action);
        Assert.Contains(expectedMessage, error.Message);
        Assert.Contains(
            KagemushaRecursiveSpendNative.NativeArchiveMaxBytes.ToString(),
            error.Message);
        Assert.Equal(expectedParameterName, error.ParamName);
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
        return KagemushaNoritoFrameFromPayload(
            0x9a,
            new byte[] { 1, 0 }
                .Concat(Encoding.UTF8.GetBytes(circuitId))
                .Concat(KagemushaVerifierKeyCommitment(verifierKey))
                .Concat(Enumerable.Repeat(seed, 64))
                .ToArray());
    }

    private static byte[] ReadBridgeOutputWithBytes(byte[] bytes)
    {
        var pointer = Marshal.AllocHGlobal(bytes.Length);
        Marshal.Copy(bytes, 0, pointer, bytes.Length);
        return KagemushaRecursiveSpendNative.ReadBridgeOutput(
            "connect_norito_kagemusha_recursive_spend_redeem",
            0,
            pointer,
            (UIntPtr)bytes.Length);
    }
}
