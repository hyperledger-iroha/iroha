using System;
using System.Buffers.Binary;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Offline;

internal static class KagemushaArchiveBytes
{
    internal static byte[] Copy(byte[] noritoBytes, string parameterName)
    {
        if (noritoBytes is null)
        {
            throw new ArgumentNullException(parameterName);
        }

        if (noritoBytes.Length == 0)
        {
            throw new ArgumentException("Kagemusha Norito archive must not be empty.", parameterName);
        }

        if (noritoBytes.Length > KagemushaRecursiveSpendNative.NativeArchiveMaxBytes)
        {
            throw new ArgumentException(
                $"Kagemusha Norito archive must not exceed {KagemushaRecursiveSpendNative.NativeArchiveMaxBytes} bytes.",
                parameterName);
        }

        if (!PrivacyNative.IsNoritoV1Archive(noritoBytes))
        {
            throw new ArgumentException(
                "Kagemusha Norito archive must be a valid Norito V1 archive.",
                parameterName);
        }

        if (!PrivacyNative.HasNonEmptyPrivacyNoritoPayload(noritoBytes))
        {
            throw new ArgumentException(
                "Kagemusha Norito archive must contain a non-empty Norito payload.",
                parameterName);
        }

        return (byte[])noritoBytes.Clone();
    }
}

public abstract class KagemushaNativeArchive
{
    private readonly byte[] noritoBytes;

    protected KagemushaNativeArchive(byte[] noritoBytes)
    {
        this.noritoBytes = KagemushaArchiveBytes.Copy(noritoBytes, nameof(noritoBytes));
    }

    public byte[] NoritoBytes => KagemushaArchiveBytes.Copy(noritoBytes, nameof(NoritoBytes));
}

public sealed class KagemushaRecursiveSpendArchive : KagemushaNativeArchive
{
    public KagemushaRecursiveSpendArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaRecursiveSpendTransitionProfileArchive : KagemushaNativeArchive
{
    public KagemushaRecursiveSpendTransitionProfileArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaRecursiveSpendLineageAppendBoundaryArchive : KagemushaNativeArchive
{
    public KagemushaRecursiveSpendLineageAppendBoundaryArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaRecursiveSpendLineageWitnessArchive : KagemushaNativeArchive
{
    public KagemushaRecursiveSpendLineageWitnessArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaRecursiveSpendVerifyArchive : KagemushaNativeArchive
{
    public KagemushaRecursiveSpendVerifyArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaRecursiveSpendRedeemInstructionArchive : KagemushaNativeArchive
{
    public KagemushaRecursiveSpendRedeemInstructionArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaCompactPaymentTokenArchive : KagemushaNativeArchive
{
    public KagemushaCompactPaymentTokenArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaRecursiveAggregationProofBundleArchive : KagemushaNativeArchive
{
    public KagemushaRecursiveAggregationProofBundleArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaRecursiveCompactPaymentTokenArchive : KagemushaNativeArchive
{
    public KagemushaRecursiveCompactPaymentTokenArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaPallasOpenEnvelopesArchive : KagemushaNativeArchive
{
    public KagemushaPallasOpenEnvelopesArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaPreviousProofOpenEnvelopesArchive : KagemushaNativeArchive
{
    public KagemushaPreviousProofOpenEnvelopesArchive(byte[] noritoBytes) : base(noritoBytes)
    {
    }
}

public sealed class KagemushaRecursiveSpendLineageKeyArtifacts
{
    private readonly byte[] lineageVerifierKey;
    private readonly byte[] lineageProvingKeyArchive;

    internal KagemushaRecursiveSpendLineageKeyArtifacts(
        string proofCircuitId,
        int verifierOpeningLen,
        string lineageVerifierKeyBackend,
        ReadOnlySpan<byte> lineageVerifierKey,
        ReadOnlySpan<byte> lineageProvingKeyArchive)
    {
        ProofCircuitId = proofCircuitId;
        VerifierOpeningLen = verifierOpeningLen;
        LineageVerifierKeyBackend = lineageVerifierKeyBackend;
        this.lineageVerifierKey = lineageVerifierKey.ToArray();
        this.lineageProvingKeyArchive = lineageProvingKeyArchive.ToArray();
    }

    public string ProofCircuitId { get; }

    public int VerifierOpeningLen { get; }

    public string LineageVerifierKeyBackend { get; }

    public bool IsInitArtifact =>
        ProofCircuitId == KagemushaRecursiveSpendNative.RecursiveSpendLineageOneHopProofCircuitIdV1;

    public bool IsAppendArtifact =>
        ProofCircuitId == KagemushaRecursiveSpendNative.RecursiveSpendLineageAppendProofCircuitIdV1;

    public byte[] LineageVerifierKey() => lineageVerifierKey.ToArray();

    public byte[] LineageProvingKeyArchive() => lineageProvingKeyArchive.ToArray();
}

public enum KagemushaOfflineSpendMode
{
    RecursiveSpendV1 = 0,
    CheckedPrefoldV1 = 1,
    RecursiveCompactV1 = 2,
}

public static class KagemushaOfflineSpendModeExtensions
{
    public const string RecursiveCompactV1WireName = "recursive_compact_v1";
    public const string RecursiveSpendV1WireName = "recursive_spend_v1";
    public const string CheckedPrefoldV1WireName = "checked_prefold_v1";

    public static string WireName(this KagemushaOfflineSpendMode mode)
    {
        return mode switch
        {
            KagemushaOfflineSpendMode.RecursiveCompactV1 => RecursiveCompactV1WireName,
            KagemushaOfflineSpendMode.RecursiveSpendV1 => RecursiveSpendV1WireName,
            KagemushaOfflineSpendMode.CheckedPrefoldV1 => CheckedPrefoldV1WireName,
            _ => throw new ArgumentOutOfRangeException(nameof(mode), mode, "Unknown Kagemusha offline spend mode."),
        };
    }
}

public static class KagemushaRecursiveSpendNative
{
    public const string RecursiveAggregationProofBackend = "halo2/ipa";
    public const string RecursiveAggregationProofCircuitIdV1 = "kagemusha-recursive-aggregation-v1";
    public const string RecursiveSpendLineageProofCircuitIdV1 = "kagemusha-recursive-spend-lineage-v1";
    public const string RecursiveSpendLineageOneHopProofCircuitIdV1 = "kagemusha-recursive-spend-lineage-onehop-v1";
    public const string RecursiveSpendLineageAppendProofCircuitIdV1 = "kagemusha-recursive-spend-lineage-append-v1";
    public const string RecursiveCompactCircuitIdV1 = "kagemusha-recursive-compact-v1";

    public const uint RequiredNativeBridgeAbiVersion = 6;
    public const uint RecursiveCompactRequiredNativeBridgeAbiVersion = 7;
    public const uint CompactTokenMaxHops = 64;
    public const uint RecursiveSpendLineageWitnesslessMaxHopsV1 = 64;
    public const bool RecursiveSpendLineageTransitionCircuitWiredV1 = true;
    public const int RecursivePreviousProofOpenEnvelopesRequiredCountV1 = 1;
    public const int RecursivePreviousProofOpenEnvelopesMaxBytes = 8 * 1024 * 1024;
    public const int RecursivePallasOpenEnvelopeMaxTranscriptLabelBytes = 128;
    public const int NativeArchiveMaxBytes = 64 * 1024 * 1024;
    private const string MaxU128Decimal = "340282366920938463463374607431768211455";
    internal const int RecursiveCompactUnavailableBridgeErrorCode = -312;
    public const string RecursiveSpendTransitionProfileDomain =
        "iroha:kagemusha:v1:recursive-spend-transition-profile";
    public const string RecursiveSpendTransitionProfileDigestDomain =
        "iroha:kagemusha:v1:recursive-spend-transition-profile-digest";
    public const string RecursiveSpendTransitionProfileBindingDigestDomain =
        "iroha:kagemusha:v1:recursive-spend-transition-profile-binding-digest";
    public const string RecursiveSpendLineageAppendOpeningsPreflightDomainV1 =
        "iroha:kagemusha:recursive-spend-lineage-append-openings-preflight:v1";
    public const string RecursiveSpendLineageAppendBoundaryDomainV1 =
        "iroha:kagemusha:recursive-spend-lineage-append-boundary:v1";
    public const string RecursiveSpendLineageAppendBoundaryChainAssetBindingDomainV1 =
        "iroha:kagemusha:recursive-spend-lineage-append-boundary-chain-asset:v1";
    public const string RecursiveSpendLineageAppendBoundaryFinalNoteBindingDomainV1 =
        "iroha:kagemusha:recursive-spend-lineage-append-boundary-final-note:v1";
    private const string LibraryName = "connect_norito_bridge";
    private const int ExpectedMalformedArchiveProbeErrorCode = -311;
    private static readonly byte[] MalformedArchiveProbe = new byte[] { 0x00 };
    private static readonly byte[] KagemushaZk1Magic = new byte[] { 0x5a, 0x4b, 0x31, 0x00 };
    private static readonly byte[] KagemushaZk1TlvCid1 = Encoding.ASCII.GetBytes("CID1");
    private static readonly byte[] KagemushaZk1TlvIpaK = Encoding.ASCII.GetBytes("IPAK");
    private static readonly byte[] KagemushaZk1TlvH2Vk = Encoding.ASCII.GetBytes("H2VK");
    private const byte KagemushaNoritoCompactLenFlag = 0x02;
    private const byte KagemushaNoritoPackedStructFlag = 0x04;
    private const byte PrivacyNoritoFieldBitsetFlag = 0x20;
    private const ushort KagemushaLineageProvingKeyArchiveVersionV1 = 1;
    private static readonly byte[] KagemushaLineageProvingKeyArchiveSchemaHash = new byte[]
    {
        0xc8, 0x84, 0x89, 0x61, 0x8a, 0x01, 0x2c, 0x28,
        0x3f, 0xf3, 0xbb, 0x2e, 0xba, 0xbc, 0x77, 0x75,
    };
    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);

    private readonly record struct LineageProvingKeyArchive(
        ushort Version,
        string CircuitFamily,
        byte[] VerifierKeyCommitment,
        byte[] ProvingKey);

    public static bool IsAvailable()
    {
        return IsAvailable(
            () => TryGetAbiVersion(out var version) ? version : null,
            TryProbeRequiredSymbols);
    }

    internal static bool IsAvailable(Func<uint?> abiVersionProbe, Func<bool> requiredSymbolsProbe)
    {
        try
        {
            var version = abiVersionProbe();
            return version is not null
                && version.Value >= RequiredNativeBridgeAbiVersion
                && requiredSymbolsProbe();
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    public static bool IsRecursiveCompactPaymentTokenProverAvailable()
    {
        return IsRecursiveCompactPaymentTokenProverAvailable(
            () => TryGetAbiVersion(out var version) ? version : null,
            TryProbeRecursiveCompactPaymentTokenSurface);
    }

    internal static bool IsRecursiveCompactPaymentTokenProverAvailable(
        Func<uint?> abiVersionProbe,
        Func<bool> compactSurfaceProbe)
    {
        try
        {
            var version = abiVersionProbe();
            return version is not null
                && version.Value >= RecursiveCompactRequiredNativeBridgeAbiVersion
                && compactSurfaceProbe();
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    public static bool IsCompactPaymentTokenProverAvailable()
    {
        return IsCompactPaymentTokenProverAvailable(
            () => TryGetAbiVersion(out var version) ? version : null,
            TryProbeCompactPaymentTokenSymbol);
    }

    internal static bool IsCompactPaymentTokenProverAvailable(
        Func<uint?> abiVersionProbe,
        Func<bool> compactTokenSymbolProbe)
    {
        try
        {
            var version = abiVersionProbe();
            return version is not null
                && version.Value >= RequiredNativeBridgeAbiVersion
                && compactTokenSymbolProbe();
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    public static bool IsRecursiveAggregationProofBundleProverAvailable()
    {
        return IsRecursiveAggregationProofBundleProverAvailable(
            () => TryGetAbiVersion(out var version) ? version : null,
            TryProbeRecursiveAggregationProofBundleSymbol);
    }

    internal static bool IsRecursiveAggregationProofBundleProverAvailable(
        Func<uint?> abiVersionProbe,
        Func<bool> recursiveAggregationSymbolProbe)
    {
        try
        {
            var version = abiVersionProbe();
            return version is not null
                && version.Value >= RequiredNativeBridgeAbiVersion
                && recursiveAggregationSymbolProbe();
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    public static bool IsPallasOpenEnvelopeBuilderAvailable()
    {
        return IsPallasOpenEnvelopeBuilderAvailable(
            () => TryGetAbiVersion(out var version) ? version : null,
            TryProbePallasOpenEnvelopeBuilderSymbols);
    }

    internal static bool IsPallasOpenEnvelopeBuilderAvailable(
        Func<uint?> abiVersionProbe,
        Func<bool> builderSymbolProbe)
    {
        try
        {
            var version = abiVersionProbe();
            return version is not null
                && version.Value >= RecursiveCompactRequiredNativeBridgeAbiVersion
                && builderSymbolProbe();
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    public static bool IsRecursiveCompactPaymentTokenVerifierAvailable()
    {
        return IsRecursiveCompactPaymentTokenVerifierAvailable(
            () => TryGetAbiVersion(out var version) ? version : null,
            TryProbeRecursiveCompactPaymentTokenVerifierSymbol);
    }

    internal static bool IsRecursiveCompactPaymentTokenVerifierAvailable(
        Func<uint?> abiVersionProbe,
        Func<bool> verifierSymbolProbe)
    {
        try
        {
            var version = abiVersionProbe();
            return version is not null
                && version.Value >= RecursiveCompactRequiredNativeBridgeAbiVersion
                && verifierSymbolProbe();
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    public static bool IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable()
    {
        return IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable(
            () => TryGetAbiVersion(out var version) ? version : null,
            TryProbeRecursiveSpendCompactPaymentTokenProjectionVerifierSymbol);
    }

    internal static bool IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable(
        Func<uint?> abiVersionProbe,
        Func<bool> verifierSymbolProbe)
    {
        try
        {
            var version = abiVersionProbe();
            return version is not null
                && version.Value >= RecursiveCompactRequiredNativeBridgeAbiVersion
                && verifierSymbolProbe();
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    public static KagemushaOfflineSpendMode PreferredMode()
    {
        return PreferredMode(IsRecursiveCompactPaymentTokenProverAvailable(), IsAvailable());
    }

    public static KagemushaOfflineSpendMode PreferredMode(bool recursiveSpendAvailable)
    {
        return PreferredMode(false, recursiveSpendAvailable);
    }

    public static KagemushaOfflineSpendMode PreferredMode(
        bool recursiveCompactAvailable,
        bool recursiveSpendAvailable)
    {
        _ = recursiveCompactAvailable;
        return recursiveSpendAvailable
            ? KagemushaOfflineSpendMode.RecursiveSpendV1
            : KagemushaOfflineSpendMode.CheckedPrefoldV1;
    }

    public static bool CanRedeemWitnessless(string? circuitId, uint hopCount)
    {
        return RecursiveSpendLineageTransitionCircuitWiredV1
            && IsLineageProofCircuitId(circuitId)
            && hopCount >= 1
            && hopCount <= RecursiveSpendLineageWitnesslessMaxHopsV1;
    }

    public static bool IsLineageProofCircuitId(string? circuitId)
    {
        return circuitId == RecursiveSpendLineageProofCircuitIdV1
            || circuitId == RecursiveSpendLineageOneHopProofCircuitIdV1
            || circuitId == RecursiveSpendLineageAppendProofCircuitIdV1;
    }

    public static bool IsLineageAppendOutputCircuitId(string? outputCircuitId)
    {
        return outputCircuitId == RecursiveSpendLineageProofCircuitIdV1
            || outputCircuitId == RecursiveSpendLineageAppendProofCircuitIdV1;
    }

    public static bool IsSupportedLineageKeyArtifactOpeningLen(int verifierOpeningLen)
    {
        return verifierOpeningLen switch
        {
            2 or 4 or 8 or 16 or 32 or 64 or 128 => true,
            _ => false,
        };
    }

    public static KagemushaRecursiveSpendLineageKeyArtifacts LineageKeyArtifactsForInit(
        int verifierOpeningLen,
        string lineageVerifierKeyBackend,
        ReadOnlySpan<byte> lineageVerifierKey,
        ReadOnlySpan<byte> lineageProvingKeyArchive)
    {
        return LineageKeyArtifacts(
            RecursiveSpendLineageOneHopProofCircuitIdV1,
            verifierOpeningLen,
            lineageVerifierKeyBackend,
            lineageVerifierKey,
            lineageProvingKeyArchive);
    }

    public static KagemushaRecursiveSpendLineageKeyArtifacts LineageKeyArtifactsForAppend(
        int verifierOpeningLen,
        string lineageVerifierKeyBackend,
        ReadOnlySpan<byte> lineageVerifierKey,
        ReadOnlySpan<byte> lineageProvingKeyArchive)
    {
        return LineageKeyArtifacts(
            RecursiveSpendLineageAppendProofCircuitIdV1,
            verifierOpeningLen,
            lineageVerifierKeyBackend,
            lineageVerifierKey,
            lineageProvingKeyArchive);
    }

    public static KagemushaRecursiveSpendLineageKeyArtifacts LineageKeyArtifacts(
        string proofCircuitId,
        int verifierOpeningLen,
        string lineageVerifierKeyBackend,
        ReadOnlySpan<byte> lineageVerifierKey,
        ReadOnlySpan<byte> lineageProvingKeyArchive)
    {
        return ValidateLineageKeyArtifacts(new KagemushaRecursiveSpendLineageKeyArtifacts(
            proofCircuitId,
            verifierOpeningLen,
            lineageVerifierKeyBackend,
            lineageVerifierKey,
            lineageProvingKeyArchive));
    }

    public static KagemushaRecursiveSpendLineageKeyArtifacts ValidateLineageKeyArtifacts(
        KagemushaRecursiveSpendLineageKeyArtifacts? artifacts)
    {
        if (artifacts is null)
        {
            throw new ArgumentException("lineage_key_artifacts", nameof(artifacts));
        }
        if (artifacts.ProofCircuitId != RecursiveSpendLineageOneHopProofCircuitIdV1
            && artifacts.ProofCircuitId != RecursiveSpendLineageAppendProofCircuitIdV1)
        {
            throw new ArgumentException("proof_circuit_id", nameof(artifacts));
        }
        if (!IsSupportedLineageKeyArtifactOpeningLen(artifacts.VerifierOpeningLen))
        {
            throw new ArgumentException("verifier_opening_len", nameof(artifacts));
        }
        var lineageVerifierKey = artifacts.LineageVerifierKey();
        var lineageProvingKeyArchive = artifacts.LineageProvingKeyArchive();
        if (artifacts.LineageVerifierKeyBackend != RecursiveAggregationProofBackend
            || lineageVerifierKey.Length == 0)
        {
            throw new ArgumentException("lineage_verifier_key", nameof(artifacts));
        }
        if (lineageProvingKeyArchive.Length == 0)
        {
            throw new ArgumentException("lineage_proving_key_archive", nameof(artifacts));
        }
        ValidateLineageKeyArtifactPackageBinding(
            artifacts.ProofCircuitId,
            artifacts.LineageVerifierKeyBackend,
            lineageVerifierKey,
            lineageProvingKeyArchive);
        return new KagemushaRecursiveSpendLineageKeyArtifacts(
            artifacts.ProofCircuitId,
            artifacts.VerifierOpeningLen,
            artifacts.LineageVerifierKeyBackend,
            lineageVerifierKey,
            lineageProvingKeyArchive);
    }

    private static void ValidateLineageKeyArtifactPackageBinding(
        string proofCircuitId,
        string lineageVerifierKeyBackend,
        byte[] lineageVerifierKey,
        byte[] lineageProvingKeyArchive)
    {
        var verifierCircuitId = LineageVerifierKeyEnvelopeCircuitId(lineageVerifierKey);
        if (verifierCircuitId != proofCircuitId)
        {
            throw new ArgumentException("lineage_verifier_key");
        }

        var archivePayload = LineageProvingKeyArchivePayload(lineageProvingKeyArchive);
        var circuitIdBytes = Encoding.UTF8.GetBytes(proofCircuitId);
        var verifierKeyCommitment = VerifyingKeyCommitment(
            lineageVerifierKeyBackend,
            lineageVerifierKey);
        if (archivePayload.AsSpan().IndexOf(circuitIdBytes) < 0
            || archivePayload.AsSpan().IndexOf(verifierKeyCommitment) < 0)
        {
            throw new ArgumentException("lineage_proving_key_archive");
        }
        var archive = DecodeLineageProvingKeyArchivePayload(
            archivePayload,
            lineageProvingKeyArchive[39]);
        if (archive.Version != KagemushaLineageProvingKeyArchiveVersionV1
            || archive.CircuitFamily != proofCircuitId
            || !archive.VerifierKeyCommitment.AsSpan().SequenceEqual(verifierKeyCommitment)
            || archive.ProvingKey.Length == 0)
        {
            throw new ArgumentException("lineage_proving_key_archive");
        }
    }

    private static string LineageVerifierKeyEnvelopeCircuitId(byte[] lineageVerifierKey)
    {
        var envelope = lineageVerifierKey.AsSpan();
        if (envelope.Length < KagemushaZk1Magic.Length
            || !envelope[..KagemushaZk1Magic.Length].SequenceEqual(KagemushaZk1Magic))
        {
            throw new ArgumentException("lineage_verifier_key");
        }

        var offset = KagemushaZk1Magic.Length;
        string? circuitId = null;
        var sawIpaK = false;
        var sawH2Vk = false;
        while (offset < envelope.Length)
        {
            if (offset + 8 > envelope.Length)
            {
                throw new ArgumentException("lineage_verifier_key");
            }

            var tag = envelope.Slice(offset, 4);
            var payloadLength = BinaryPrimitives.ReadUInt32LittleEndian(
                envelope.Slice(offset + 4, 4));
            var payloadStart = offset + 8;
            if (payloadLength > int.MaxValue || payloadStart + (int)payloadLength > envelope.Length)
            {
                throw new ArgumentException("lineage_verifier_key");
            }

            var payload = envelope.Slice(payloadStart, (int)payloadLength);
            if (tag.SequenceEqual(KagemushaZk1TlvCid1))
            {
                if (circuitId is not null || payload.Length == 0)
                {
                    throw new ArgumentException("lineage_verifier_key");
                }

                foreach (var value in payload)
                {
                    if (value < 0x20 || value > 0x7e)
                    {
                        throw new ArgumentException("lineage_verifier_key");
                    }
                }

                circuitId = Encoding.UTF8.GetString(payload);
                if (circuitId.Length == 0)
                {
                    throw new ArgumentException("lineage_verifier_key");
                }
            }
            else if (tag.SequenceEqual(KagemushaZk1TlvIpaK))
            {
                if (sawIpaK || payload.Length != 4)
                {
                    throw new ArgumentException("lineage_verifier_key");
                }
                sawIpaK = true;
            }
            else if (tag.SequenceEqual(KagemushaZk1TlvH2Vk))
            {
                if (sawH2Vk || payload.Length == 0)
                {
                    throw new ArgumentException("lineage_verifier_key");
                }
                sawH2Vk = true;
            }
            else
            {
                throw new ArgumentException("lineage_verifier_key");
            }

            offset = payloadStart + (int)payloadLength;
        }

        if (circuitId is null || !sawIpaK || !sawH2Vk)
        {
            throw new ArgumentException("lineage_verifier_key");
        }
        return circuitId;
    }

    private static byte[] LineageProvingKeyArchivePayload(byte[] lineageProvingKeyArchive)
    {
        if (!PrivacyNative.IsNoritoV1Archive(lineageProvingKeyArchive)
            || !PrivacyNative.HasNonEmptyPrivacyNoritoPayload(lineageProvingKeyArchive))
        {
            throw new ArgumentException("lineage_proving_key_archive");
        }
        if (!lineageProvingKeyArchive.AsSpan(6, 16)
                .SequenceEqual(KagemushaLineageProvingKeyArchiveSchemaHash)
            || (lineageProvingKeyArchive[39] & KagemushaNoritoPackedStructFlag) != 0
            || (lineageProvingKeyArchive[39] & PrivacyNoritoFieldBitsetFlag) != 0)
        {
            throw new ArgumentException("lineage_proving_key_archive");
        }

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(
            lineageProvingKeyArchive.AsSpan(23, 8));
        if (payloadLength == 0
            || payloadLength > int.MaxValue
            || payloadLength > (ulong)(lineageProvingKeyArchive.Length - 40))
        {
            throw new ArgumentException("lineage_proving_key_archive");
        }

        var payloadOffset = lineageProvingKeyArchive.Length - (int)payloadLength;
        return lineageProvingKeyArchive.AsSpan(payloadOffset, (int)payloadLength).ToArray();
    }

    private static LineageProvingKeyArchive DecodeLineageProvingKeyArchivePayload(
        byte[] payload,
        byte flags)
    {
        try
        {
            var offset = 0;
            var versionPayload = ReadNoritoField(payload, ref offset, flags);
            if (versionPayload.Length != 2)
            {
                throw new ArgumentException("lineage_proving_key_archive");
            }
            var version = BinaryPrimitives.ReadUInt16LittleEndian(versionPayload);

            var circuitFamily = DecodeNoritoString(
                ReadNoritoField(payload, ref offset, flags),
                flags);

            var verifierKeyCommitment = ReadNoritoField(payload, ref offset, flags);
            if (verifierKeyCommitment.Length != 32)
            {
                throw new ArgumentException("lineage_proving_key_archive");
            }

            var provingKey = DecodeNoritoByteVec(
                ReadNoritoField(payload, ref offset, flags));
            if (offset != payload.Length)
            {
                throw new ArgumentException("lineage_proving_key_archive");
            }

            return new LineageProvingKeyArchive(
                version,
                circuitFamily,
                verifierKeyCommitment,
                provingKey);
        }
        catch (ArgumentException)
        {
            throw new ArgumentException("lineage_proving_key_archive");
        }
    }

    private static byte[] ReadNoritoField(byte[] buffer, ref int offset, byte flags)
    {
        var length = ReadNoritoLength(buffer, ref offset, flags);
        if (length > buffer.Length - offset)
        {
            throw new ArgumentException("lineage_proving_key_archive");
        }
        var field = buffer.AsSpan(offset, length).ToArray();
        offset += length;
        return field;
    }

    private static int ReadNoritoLength(byte[] buffer, ref int offset, byte flags)
    {
        if ((flags & KagemushaNoritoCompactLenFlag) == 0)
        {
            if (offset + 8 > buffer.Length)
            {
                throw new ArgumentException("lineage_proving_key_archive");
            }
            var fixedLength = BinaryPrimitives.ReadUInt64LittleEndian(
                buffer.AsSpan(offset, 8));
            if (fixedLength > int.MaxValue)
            {
                throw new ArgumentException("lineage_proving_key_archive");
            }
            offset += 8;
            return (int)fixedLength;
        }

        ulong value = 0;
        var shift = 0;
        var startOffset = offset;
        for (var index = 0; index < 10; index++)
        {
            if (offset >= buffer.Length)
            {
                throw new ArgumentException("lineage_proving_key_archive");
            }
            var current = buffer[offset++];
            var currentValue = current & 0x7f;
            if (shift >= 63 && currentValue > 1)
            {
                throw new ArgumentException("lineage_proving_key_archive");
            }
            value |= (ulong)currentValue << shift;
            if ((current & 0x80) == 0)
            {
                var encodedLength = offset - startOffset;
                if (encodedLength > 1 && value < (1UL << (7 * (encodedLength - 1))))
                {
                    throw new ArgumentException("lineage_proving_key_archive");
                }
                if (value > int.MaxValue)
                {
                    throw new ArgumentException("lineage_proving_key_archive");
                }
                return (int)value;
            }
            shift += 7;
        }
        throw new ArgumentException("lineage_proving_key_archive");
    }

    private static string DecodeNoritoString(byte[] payload, byte flags)
    {
        var offset = 0;
        var length = ReadNoritoLength(payload, ref offset, flags);
        if (length != payload.Length - offset)
        {
            throw new ArgumentException("lineage_proving_key_archive");
        }
        return StrictUtf8.GetString(payload, offset, length);
    }

    private static byte[] DecodeNoritoByteVec(byte[] payload)
    {
        if (payload.Length < 8)
        {
            throw new ArgumentException("lineage_proving_key_archive");
        }
        var length = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(0, 8));
        if (length > int.MaxValue || length != (ulong)(payload.Length - 8))
        {
            throw new ArgumentException("lineage_proving_key_archive");
        }
        return payload.AsSpan(8, (int)length).ToArray();
    }

    private static byte[] VerifyingKeyCommitment(
        string lineageVerifierKeyBackend,
        byte[] lineageVerifierKey)
    {
        var domain = Encoding.ASCII.GetBytes("iroha:zk:v1:vk");
        var backend = Encoding.UTF8.GetBytes(lineageVerifierKeyBackend);
        var preimage = new byte[domain.Length + 8 + backend.Length + 8 + lineageVerifierKey.Length];
        var offset = 0;
        domain.AsSpan().CopyTo(preimage.AsSpan(offset));
        offset += domain.Length;
        BinaryPrimitives.WriteUInt64BigEndian(preimage.AsSpan(offset, 8), (ulong)backend.Length);
        offset += 8;
        backend.AsSpan().CopyTo(preimage.AsSpan(offset));
        offset += backend.Length;
        BinaryPrimitives.WriteUInt64BigEndian(preimage.AsSpan(offset, 8), (ulong)lineageVerifierKey.Length);
        offset += 8;
        lineageVerifierKey.AsSpan().CopyTo(preimage.AsSpan(offset));
        return SHA256.HashData(preimage);
    }

    public static bool RequiresLineageKeyArtifactsForInit()
    {
        return true;
    }

    public static bool RequiresLineageWitnessForRedeem(string? circuitId, uint hopCount)
    {
        return !CanRedeemWitnessless(circuitId, hopCount);
    }

    public static void ValidateRedeemLineagePreflight(
        string? proofCircuitId,
        uint hopCount,
        bool hasLineageWitness,
        bool hasLineageVerifierRecord)
    {
        if (RequiresLineageWitnessForRedeem(proofCircuitId, hopCount) && !hasLineageWitness)
        {
            throw new ArgumentException(
                "lineageWitness is required for this bundle",
                nameof(hasLineageWitness));
        }

        if (IsLineageProofCircuitId(proofCircuitId) && !hasLineageVerifierRecord)
        {
            throw new ArgumentException(
                "lineageVerifierRecord is required for reserved-lineage bundles",
                nameof(hasLineageVerifierRecord));
        }
    }

    public static void ValidateRedeemChangeOutputPreflight(
        string? publicAmount,
        string? currentNoteAmount,
        bool hasChangeOutput)
    {
        var normalizedPublicAmount = CanonicalU128Decimal(
            publicAmount,
            nameof(publicAmount),
            nameof(publicAmount));
        var normalizedCurrentNoteAmount = CanonicalU128Decimal(
            currentNoteAmount,
            nameof(currentNoteAmount),
            nameof(currentNoteAmount));
        var comparison = CompareCanonicalDecimal(
            normalizedPublicAmount,
            normalizedCurrentNoteAmount);
        if (hasChangeOutput)
        {
            if (comparison >= 0)
            {
                throw new ArgumentException(
                    "publicAmount must be less than current note amount when changeOutput is present",
                    nameof(publicAmount));
            }
        }
        else
        {
            if (comparison < 0)
            {
                throw new ArgumentException(
                    "changeOutput is required when publicAmount is less than current note amount",
                    nameof(hasChangeOutput));
            }
            if (comparison > 0)
            {
                throw new ArgumentException(
                    "publicAmount must not exceed current note amount",
                    nameof(publicAmount));
            }
        }
    }

    private static string CanonicalU128Decimal(
        string? value,
        string fieldName,
        string parameterName)
    {
        if (value is null)
        {
            throw new ArgumentNullException(parameterName);
        }
        if (value.Length == 0)
        {
            throw new ArgumentException($"{fieldName} must be a decimal integer", parameterName);
        }
        foreach (var ch in value)
        {
            if (ch < '0' || ch > '9')
            {
                throw new ArgumentException($"{fieldName} must be a decimal integer", parameterName);
            }
        }
        if (value.Length > 1 && value[0] == '0')
        {
            throw new ArgumentException($"{fieldName} must be canonical", parameterName);
        }
        if (value == "0")
        {
            throw new ArgumentException($"{fieldName} must be greater than zero", parameterName);
        }
        if (CompareCanonicalDecimal(value, MaxU128Decimal) > 0)
        {
            throw new ArgumentException($"{fieldName} must fit in u128", parameterName);
        }
        return value;
    }

    private static int CompareCanonicalDecimal(string left, string right)
    {
        if (left.Length != right.Length)
        {
            return left.Length < right.Length ? -1 : 1;
        }
        return string.CompareOrdinal(left, right);
    }

    public static bool CanAppendWitnesslessLineage(uint previousHopCount)
    {
        return RecursiveSpendLineageTransitionCircuitWiredV1
            && previousHopCount >= 1
            && previousHopCount < RecursiveSpendLineageWitnesslessMaxHopsV1;
    }

    public static string NormalizeAppendOutputCircuitId(string? outputCircuitId)
    {
        if (string.IsNullOrEmpty(outputCircuitId))
        {
            return RecursiveAggregationProofCircuitIdV1;
        }
        return outputCircuitId == RecursiveSpendLineageProofCircuitIdV1
            ? RecursiveSpendLineageAppendProofCircuitIdV1
            : outputCircuitId!;
    }

    public static bool IsSupportedAppendOutputCircuitId(string? outputCircuitId)
    {
        var normalized = NormalizeAppendOutputCircuitId(outputCircuitId);
        return normalized == RecursiveAggregationProofCircuitIdV1
            || normalized == RecursiveSpendLineageAppendProofCircuitIdV1;
    }

    public static bool RequiresLineageKeyArtifactsForAppendOutput(string? outputCircuitId)
    {
        return IsLineageAppendOutputCircuitId(NormalizeAppendOutputCircuitId(outputCircuitId));
    }

    public static bool IsSupportedPreviousProofCircuitId(string? previousProofCircuitId)
    {
        return previousProofCircuitId == RecursiveAggregationProofCircuitIdV1
            || IsLineageProofCircuitId(previousProofCircuitId);
    }

    public static bool RequiresPreviousLineageVerifierRecordForAppend(string? previousProofCircuitId)
    {
        return IsLineageProofCircuitId(previousProofCircuitId);
    }

    public static bool IsSupportedAppendProofTransition(
        string? previousProofCircuitId,
        string? outputCircuitId)
    {
        var normalizedOutput = NormalizeAppendOutputCircuitId(outputCircuitId);
        return previousProofCircuitId == RecursiveAggregationProofCircuitIdV1
            && normalizedOutput == RecursiveAggregationProofCircuitIdV1
            || IsLineageProofCircuitId(previousProofCircuitId)
            && (
                normalizedOutput == RecursiveAggregationProofCircuitIdV1
                || normalizedOutput == RecursiveSpendLineageAppendProofCircuitIdV1);
    }

    public static string PreferredAppendOutputCircuitId(uint previousHopCount)
    {
        return CanAppendWitnesslessLineage(previousHopCount)
            ? RecursiveSpendLineageAppendProofCircuitIdV1
            : RecursiveAggregationProofCircuitIdV1;
    }

    public static bool CanProveAppendOutputCircuitId(string? outputCircuitId, uint previousHopCount)
    {
        if (previousHopCount < 1)
        {
            return false;
        }
        var normalized = NormalizeAppendOutputCircuitId(outputCircuitId);
        return normalized switch
        {
            RecursiveAggregationProofCircuitIdV1 => previousHopCount < CompactTokenMaxHops,
            RecursiveSpendLineageAppendProofCircuitIdV1 => CanAppendWitnesslessLineage(previousHopCount),
            _ => false,
        };
    }

    public static bool CanSelectAppendOutputCircuitId(
        string? previousProofCircuitId,
        string? outputCircuitId,
        uint previousHopCount)
    {
        if (!CanProveAppendOutputCircuitId(outputCircuitId, previousHopCount))
        {
            return false;
        }
        if (!IsSupportedPreviousProofCircuitId(previousProofCircuitId))
        {
            return false;
        }
        return IsSupportedAppendProofTransition(previousProofCircuitId, outputCircuitId);
    }

    public static bool RequiresPreviousProofOpenEnvelopesForAppend(string? outputCircuitId, uint previousHopCount)
    {
        return IsLineageAppendOutputCircuitId(NormalizeAppendOutputCircuitId(outputCircuitId))
            && previousHopCount >= 1;
    }

    public static KagemushaRecursiveSpendArchive Init(ReadOnlySpan<byte> requestArchive)
    {
        return new KagemushaRecursiveSpendArchive(Call(
            requestArchive,
            "connect_norito_kagemusha_recursive_spend_init",
            NativeInit));
    }

    public static KagemushaRecursiveSpendArchive Append(ReadOnlySpan<byte> requestArchive)
    {
        return new KagemushaRecursiveSpendArchive(Call(
            requestArchive,
            "connect_norito_kagemusha_recursive_spend_append",
            NativeAppend));
    }

    public static KagemushaRecursiveSpendTransitionProfileArchive TransitionProfileInit(
        ReadOnlySpan<byte> requestArchive)
    {
        return new KagemushaRecursiveSpendTransitionProfileArchive(Call(
            requestArchive,
            "connect_norito_kagemusha_recursive_spend_transition_profile_init",
            NativeTransitionProfileInit));
    }

    public static KagemushaRecursiveSpendTransitionProfileArchive TransitionProfileAppend(
        ReadOnlySpan<byte> requestArchive)
    {
        return new KagemushaRecursiveSpendTransitionProfileArchive(Call(
            requestArchive,
            "connect_norito_kagemusha_recursive_spend_transition_profile_append",
            NativeTransitionProfileAppend));
    }

    public static KagemushaRecursiveSpendLineageAppendBoundaryArchive LineageAppendBoundary(
        ReadOnlySpan<byte> profileArchive)
    {
        return new KagemushaRecursiveSpendLineageAppendBoundaryArchive(Call(
            profileArchive,
            "connect_norito_kagemusha_recursive_spend_lineage_append_boundary",
            NativeLineageAppendBoundary));
    }

    public static KagemushaRecursiveSpendLineageWitnessArchive LineageWitnessFromInitResult(
        ReadOnlySpan<byte> requestArchive,
        ReadOnlySpan<byte> bundleArchive)
    {
        return new KagemushaRecursiveSpendLineageWitnessArchive(Call(
            requestArchive,
            bundleArchive,
            "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result",
            NativeLineageWitnessFromInitResult));
    }

    public static KagemushaRecursiveSpendLineageWitnessArchive LineageWitnessAppendResult(
        ReadOnlySpan<byte> previousWitnessArchive,
        ReadOnlySpan<byte> requestArchive,
        ReadOnlySpan<byte> bundleArchive)
    {
        return new KagemushaRecursiveSpendLineageWitnessArchive(Call(
            previousWitnessArchive,
            requestArchive,
            bundleArchive,
            "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result",
            NativeLineageWitnessAppendResult));
    }

    public static KagemushaRecursiveSpendVerifyArchive Verify(ReadOnlySpan<byte> requestArchive)
    {
        return new KagemushaRecursiveSpendVerifyArchive(Call(
            requestArchive,
            "connect_norito_kagemusha_recursive_spend_verify",
            NativeVerify));
    }

    public static KagemushaRecursiveSpendRedeemInstructionArchive Redeem(ReadOnlySpan<byte> requestArchive)
    {
        return new KagemushaRecursiveSpendRedeemInstructionArchive(Call(
            requestArchive,
            "connect_norito_kagemusha_recursive_spend_redeem",
            NativeRedeem));
    }

    public static KagemushaRecursiveSpendRedeemInstructionArchive Redeem(
        ReadOnlySpan<byte> requestArchive,
        string? publicAmount,
        string? currentNoteAmount,
        bool hasChangeOutput)
    {
        ValidateRedeemChangeOutputPreflight(
            publicAmount,
            currentNoteAmount,
            hasChangeOutput);
        return Redeem(requestArchive);
    }

    public static KagemushaRecursiveSpendRedeemInstructionArchive Redeem(
        ReadOnlySpan<byte> requestArchive,
        string? proofCircuitId,
        uint hopCount,
        bool hasLineageWitness,
        bool hasLineageVerifierRecord)
    {
        ValidateRedeemLineagePreflight(
            proofCircuitId,
            hopCount,
            hasLineageWitness,
            hasLineageVerifierRecord);
        return Redeem(requestArchive);
    }

    public static KagemushaRecursiveSpendRedeemInstructionArchive Redeem(
        ReadOnlySpan<byte> requestArchive,
        string? proofCircuitId,
        uint hopCount,
        bool hasLineageWitness,
        bool hasLineageVerifierRecord,
        string? publicAmount,
        string? currentNoteAmount,
        bool hasChangeOutput)
    {
        ValidateRedeemLineagePreflight(
            proofCircuitId,
            hopCount,
            hasLineageWitness,
            hasLineageVerifierRecord);
        ValidateRedeemChangeOutputPreflight(
            publicAmount,
            currentNoteAmount,
            hasChangeOutput);
        return Redeem(requestArchive);
    }

    public static KagemushaCompactPaymentTokenArchive ProveVerifiedCompactPaymentTokenWithRecords(
        ReadOnlySpan<byte> recordBundleArchive)
    {
        var recordBundle = RequireValidInputArchive(
            recordBundleArchive,
            nameof(recordBundleArchive),
            "Record bundle archive");
        if (!IsCompactPaymentTokenProverAvailable())
        {
            throw new InvalidOperationException(
                "Kagemusha compact payment-token prover requires native bridge ABI 6 with the compact-token prover symbol.");
        }
        var code = NativeCompactPaymentToken(
            recordBundle,
            (UIntPtr)recordBundle.Length,
            out var outPtr,
            out var outLen);
        return new KagemushaCompactPaymentTokenArchive(ReadBridgeOutput(
            "connect_norito_kagemusha_prove_verified_compact_payment_token_with_records",
            code,
            outPtr,
            outLen));
    }

    public static KagemushaPallasOpenEnvelopesArchive BuildPallasOpenEnvelopesArchive(
        ReadOnlySpan<byte> recordBundleArchive)
    {
        var recordBundle = RequireValidInputArchive(
            recordBundleArchive,
            nameof(recordBundleArchive),
            "Record bundle archive");
        if (!IsPallasOpenEnvelopeBuilderAvailable())
        {
            throw new InvalidOperationException(
                "Kagemusha Pallas open-envelope builders require native bridge ABI 7 with current-hop and previous-proof builder symbols.");
        }
        var code = NativeBuildPallasOpenEnvelopesArchive(
            recordBundle,
            (UIntPtr)recordBundle.Length,
            out var outPtr,
            out var outLen);
        return new KagemushaPallasOpenEnvelopesArchive(ReadBridgeOutput(
            "connect_norito_kagemusha_build_pallas_open_envelopes_archive",
            code,
            outPtr,
            outLen));
    }

    public static KagemushaPreviousProofOpenEnvelopesArchive BuildPreviousProofOpenEnvelopesArchive(
        ReadOnlySpan<byte> previousBundleArchive)
    {
        var previousBundle = RequireValidInputArchive(
            previousBundleArchive,
            nameof(previousBundleArchive),
            "Previous recursive proof bundle archive");
        if (!IsPallasOpenEnvelopeBuilderAvailable())
        {
            throw new InvalidOperationException(
                "Kagemusha Pallas open-envelope builders require native bridge ABI 7 with current-hop and previous-proof builder symbols.");
        }
        var code = NativeBuildPreviousProofOpenEnvelopesArchive(
            previousBundle,
            (UIntPtr)previousBundle.Length,
            out var outPtr,
            out var outLen);
        return new KagemushaPreviousProofOpenEnvelopesArchive(ReadBridgeOutput(
            "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive",
            code,
            outPtr,
            outLen));
    }

    public static KagemushaRecursiveAggregationProofBundleArchive ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive)
    {
        var recordBundle = RequireValidInputArchive(
            recordBundleArchive,
            nameof(recordBundleArchive),
            "Record bundle archive");
        var pallasOpenEnvelopes = RequireValidInputArchive(
            pallasOpenEnvelopesArchive,
            nameof(pallasOpenEnvelopesArchive),
            "Pallas open-envelopes archive");
        if (!IsRecursiveAggregationProofBundleProverAvailable())
        {
            throw new InvalidOperationException(
                "Kagemusha recursive aggregation proof-bundle prover requires native bridge ABI 6 with the recursive aggregation prover symbol.");
        }
        var code = NativeRecursiveAggregationProofBundle(
            recordBundle,
            (UIntPtr)recordBundle.Length,
            pallasOpenEnvelopes,
            (UIntPtr)pallasOpenEnvelopes.Length,
            out var outPtr,
            out var outLen);
        return new KagemushaRecursiveAggregationProofBundleArchive(ReadBridgeOutput(
            "connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes",
            code,
            outPtr,
            outLen));
    }

    public static KagemushaRecursiveCompactPaymentTokenArchive ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive,
        ReadOnlySpan<byte> recursiveCompactKeyArtifactsArchive)
    {
        var recordBundle = RequireValidInputArchive(
            recordBundleArchive,
            nameof(recordBundleArchive),
            "Record bundle archive");
        var pallasOpenEnvelopes = RequireValidInputArchive(
            pallasOpenEnvelopesArchive,
            nameof(pallasOpenEnvelopesArchive),
            "Pallas open-envelopes archive");
        var recursiveCompactKeyArtifacts = RequireValidInputArchive(
            recursiveCompactKeyArtifactsArchive,
            nameof(recursiveCompactKeyArtifactsArchive),
            "Recursive compact key artifacts archive");
        if (!IsRecursiveCompactPaymentTokenProverAvailable())
        {
            throw new InvalidOperationException(
                "Recursive compact Kagemusha payment-token prover requires native bridge ABI 7 with compact prover and verifier symbols.");
        }
        var code = NativeRecursiveCompactPaymentToken(
            recordBundle,
            (UIntPtr)recordBundle.Length,
            pallasOpenEnvelopes,
            (UIntPtr)pallasOpenEnvelopes.Length,
            recursiveCompactKeyArtifacts,
            (UIntPtr)recursiveCompactKeyArtifacts.Length,
            out var outPtr,
            out var outLen);
        return new KagemushaRecursiveCompactPaymentTokenArchive(ReadBridgeOutput(
            "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes",
            code,
            outPtr,
            outLen));
    }

    public static KagemushaRecursiveCompactPaymentTokenArchive RecursiveSpendCompactPaymentTokenFromBundle(
        ReadOnlySpan<byte> bundleArchive)
    {
        var bundle = RequireValidInputArchive(
            bundleArchive,
            nameof(bundleArchive),
            "Recursive spend bundle archive");
        if (!IsRecursiveCompactPaymentTokenProverAvailable())
        {
            throw new InvalidOperationException(
                "Recursive spend compact Kagemusha payment-token projection requires native bridge ABI 7 with the compact projection symbol.");
        }
        var code = NativeRecursiveSpendCompactPaymentTokenFromBundle(
            bundle,
            (UIntPtr)bundle.Length,
            out var outPtr,
            out var outLen);
        return new KagemushaRecursiveCompactPaymentTokenArchive(ReadBridgeOutput(
            "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle",
            code,
            outPtr,
            outLen));
    }

    public static bool VerifyRecursiveCompactPaymentToken(
        ReadOnlySpan<byte> compactTokenArchive,
        ReadOnlySpan<byte> recursiveCompactVerifierKeysArchive)
    {
        if (compactTokenArchive.IsEmpty)
        {
            throw new ArgumentException(
                "Compact token archive must not be empty.",
                nameof(compactTokenArchive));
        }
        if (compactTokenArchive.Length > NativeArchiveMaxBytes)
        {
            throw new ArgumentException(
                $"Compact token archive must not exceed {NativeArchiveMaxBytes} bytes.",
                nameof(compactTokenArchive));
        }
        var compactToken = compactTokenArchive.ToArray();
        RequireValidRecursiveCompactTokenArchive(compactToken);
        var recursiveCompactVerifierKeys = RequireValidInputArchive(
            recursiveCompactVerifierKeysArchive,
            nameof(recursiveCompactVerifierKeysArchive),
            "Recursive compact verifier keys archive");
        if (!IsRecursiveCompactPaymentTokenVerifierAvailable())
        {
            throw new InvalidOperationException(
                "Recursive compact Kagemusha payment-token verifier requires native bridge ABI 7 with the compact verifier symbol.");
        }
        var code = NativeVerifyRecursiveCompactPaymentToken(
            compactToken,
            (UIntPtr)compactToken.Length,
            recursiveCompactVerifierKeys,
            (UIntPtr)recursiveCompactVerifierKeys.Length,
            out var valid);
        return NormalizeRecursiveCompactVerifierOutput(
            "connect_norito_kagemusha_verify_recursive_compact_payment_token",
            code,
            valid);
    }

    public static bool VerifyRecursiveSpendCompactPaymentTokenProjection(
        ReadOnlySpan<byte> compactTokenArchive,
        ReadOnlySpan<byte> verifierRecordArchive,
        ulong? blockHeight = null)
    {
        if (compactTokenArchive.IsEmpty)
        {
            throw new ArgumentException(
                "Compact token archive must not be empty.",
                nameof(compactTokenArchive));
        }
        var compactToken = compactTokenArchive.ToArray();
        RequireValidRecursiveCompactTokenArchive(compactToken);
        var verifierRecord = RequireValidInputArchive(
            verifierRecordArchive,
            nameof(verifierRecordArchive),
            "Verifier record archive");
        if (!IsRecursiveSpendCompactPaymentTokenProjectionVerifierAvailable())
        {
            throw new InvalidOperationException(
                "Recursive spend compact Kagemusha payment-token projection verifier requires native bridge ABI 7 with the compact projection verifier symbols.");
        }
        byte valid;
        int code;
        if (blockHeight.HasValue)
        {
            code = NativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                compactToken,
                (UIntPtr)compactToken.Length,
                verifierRecord,
                (UIntPtr)verifierRecord.Length,
                blockHeight.Value,
                out valid);
        }
        else
        {
            code = NativeVerifyRecursiveSpendCompactPaymentTokenProjection(
                compactToken,
                (UIntPtr)compactToken.Length,
                verifierRecord,
                (UIntPtr)verifierRecord.Length,
                out valid);
        }
        return NormalizeRecursiveCompactVerifierOutput(
            blockHeight.HasValue
                ? "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height"
                : "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection",
            code,
            valid);
    }

    internal static bool NormalizeRecursiveCompactVerifierOutput(string symbol, int code, byte valid)
    {
        if (code != 0)
        {
            if (code == RecursiveCompactUnavailableBridgeErrorCode)
            {
                throw new InvalidOperationException(
                    $"{symbol} is unavailable until ABI-7 recursive compact proof composition is enabled; bridge error code {code}.");
            }
            throw new InvalidOperationException($"{symbol} failed with bridge error code {code}.");
        }
        if (valid == 0)
        {
            return false;
        }
        if (valid == 1)
        {
            return true;
        }
        throw new InvalidOperationException($"{symbol} returned invalid boolean output {valid}.");
    }

    private static void RequireValidRecursiveCompactTokenArchive(byte[] compactTokenArchive)
    {
        if (compactTokenArchive.Length > NativeArchiveMaxBytes)
        {
            throw new ArgumentException(
                $"Compact token archive must not exceed {NativeArchiveMaxBytes} bytes.",
                nameof(compactTokenArchive));
        }
        if (!PrivacyNative.IsNoritoV1Archive(compactTokenArchive))
        {
            throw new ArgumentException(
                "Compact token archive must be a valid Norito archive.",
                nameof(compactTokenArchive));
        }
        if (!PrivacyNative.HasNonEmptyPrivacyNoritoPayload(compactTokenArchive))
        {
            throw new ArgumentException(
                "Compact token archive must contain a non-empty Norito payload.",
                nameof(compactTokenArchive));
        }
    }

    private delegate int NativeArchiveCall(
        byte[] requestPtr,
        UIntPtr requestLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    private delegate int NativeArchivePairCall(
        byte[] requestPtr,
        UIntPtr requestLen,
        byte[] bundlePtr,
        UIntPtr bundleLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    private delegate int NativeArchiveTripleCall(
        byte[] witnessPtr,
        UIntPtr witnessLen,
        byte[] requestPtr,
        UIntPtr requestLen,
        byte[] bundlePtr,
        UIntPtr bundleLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    private static byte[] Call(
        ReadOnlySpan<byte> requestArchive,
        string symbol,
        NativeArchiveCall nativeCall)
    {
        var request = RequireValidInputArchive(
            requestArchive,
            nameof(requestArchive),
            "Request archive");

        RequireAbi();

        var code = nativeCall(request, (UIntPtr)request.Length, out var outPtr, out var outLen);
        return ReadBridgeOutput(symbol, code, outPtr, outLen);
    }

    private static byte[] Call(
        ReadOnlySpan<byte> requestArchive,
        ReadOnlySpan<byte> bundleArchive,
        string symbol,
        NativeArchivePairCall nativeCall)
    {
        var request = RequireValidInputArchive(
            requestArchive,
            nameof(requestArchive),
            "Request archive");
        var bundle = RequireValidInputArchive(
            bundleArchive,
            nameof(bundleArchive),
            "Bundle archive");

        RequireAbi();

        var code = nativeCall(
            request,
            (UIntPtr)request.Length,
            bundle,
            (UIntPtr)bundle.Length,
            out var outPtr,
            out var outLen);
        return ReadBridgeOutput(symbol, code, outPtr, outLen);
    }

    private static byte[] Call(
        ReadOnlySpan<byte> previousWitnessArchive,
        ReadOnlySpan<byte> requestArchive,
        ReadOnlySpan<byte> bundleArchive,
        string symbol,
        NativeArchiveTripleCall nativeCall)
    {
        var witness = RequireValidInputArchive(
            previousWitnessArchive,
            nameof(previousWitnessArchive),
            "Previous witness archive");
        var request = RequireValidInputArchive(
            requestArchive,
            nameof(requestArchive),
            "Request archive");
        var bundle = RequireValidInputArchive(
            bundleArchive,
            nameof(bundleArchive),
            "Bundle archive");

        RequireAbi();

        var code = nativeCall(
            witness,
            (UIntPtr)witness.Length,
            request,
            (UIntPtr)request.Length,
            bundle,
            (UIntPtr)bundle.Length,
            out var outPtr,
            out var outLen);
        return ReadBridgeOutput(symbol, code, outPtr, outLen);
    }

    private static byte[] RequireValidInputArchive(
        ReadOnlySpan<byte> archive,
        string parameterName,
        string displayName)
    {
        if (archive.IsEmpty)
        {
            throw new ArgumentException($"{displayName} must not be empty.", parameterName);
        }
        if (archive.Length > NativeArchiveMaxBytes)
        {
            throw new ArgumentException(
                $"{displayName} must not exceed {NativeArchiveMaxBytes} bytes.",
                parameterName);
        }
        var bytes = archive.ToArray();
        if (!PrivacyNative.IsNoritoV1Archive(bytes))
        {
            throw new ArgumentException(
                $"{displayName} must be a valid Norito archive.",
                parameterName);
        }
        if (!PrivacyNative.HasNonEmptyPrivacyNoritoPayload(bytes))
        {
            throw new ArgumentException(
                $"{displayName} must contain a non-empty Norito payload.",
                parameterName);
        }
        return bytes;
    }

    private static void RequireAbi()
    {
        if (!TryGetAbiVersion(out var version))
        {
            throw new InvalidOperationException(
                $"{LibraryName} is unavailable; install the native bridge before using recursive Kagemusha.");
        }

        if (version < RequiredNativeBridgeAbiVersion)
        {
            throw new InvalidOperationException(
                $"{LibraryName} ABI v{RequiredNativeBridgeAbiVersion} is required for recursive Kagemusha, found v{version}.");
        }

        if (!TryProbeRequiredSymbols())
        {
            throw new InvalidOperationException(
                $"{LibraryName} ABI v{RequiredNativeBridgeAbiVersion} recursive Kagemusha surface is incomplete.");
        }
    }

    internal static byte[] ReadBridgeOutput(string symbol, int code, IntPtr outPtr, UIntPtr outLen)
    {
        return ReadBridgeOutput(symbol, code, outPtr, outLen, NativeFree);
    }

    internal static byte[] ReadBridgeOutput(
        string symbol,
        int code,
        IntPtr outPtr,
        UIntPtr outLen,
        Action<IntPtr> free)
    {
        if (code != 0)
        {
            if (code == RecursiveCompactUnavailableBridgeErrorCode)
            {
                throw new InvalidOperationException(
                    $"{symbol} is unavailable until ABI-7 recursive compact proof composition is enabled; bridge error code {code}.");
            }
            throw new InvalidOperationException($"{symbol} failed with bridge error code {code}.");
        }

        var shouldFree = outPtr != IntPtr.Zero;
        try
        {
            var rawLength = outLen.ToUInt64();
            if (rawLength > NativeArchiveMaxBytes)
            {
                throw new InvalidOperationException($"{symbol} returned oversized output.");
            }
            var length = (int)rawLength;
            if (length == 0)
            {
                throw new InvalidOperationException($"{symbol} returned empty output.");
            }
            if (outPtr == IntPtr.Zero)
            {
                throw new InvalidOperationException($"{symbol} returned a null output pointer.");
            }
            var result = new byte[length];
            Marshal.Copy(outPtr, result, 0, length);
            RequireValidNativeOutput(symbol, result);
            return result;
        }
        finally
        {
            if (shouldFree)
            {
                free(outPtr);
            }
        }
    }

    private static void RequireValidNativeOutput(string symbol, byte[] output)
    {
        if (!PrivacyNative.IsNoritoV1Archive(output))
        {
            throw new InvalidOperationException($"{symbol} returned invalid Norito archive.");
        }
        if (!PrivacyNative.HasNonEmptyPrivacyNoritoPayload(output))
        {
            throw new InvalidOperationException($"{symbol} returned empty Norito payload.");
        }
    }

    private static bool TryProbeRequiredSymbols()
    {
        try
        {
            if (!Probe(NativeInit)
                || !Probe(NativeAppend)
                || !Probe(NativeTransitionProfileInit)
                || !Probe(NativeTransitionProfileAppend)
                || !Probe(NativeLineageAppendBoundary)
                || !Probe(NativeVerify)
                || !Probe((NativeArchivePairCall)NativeLineageWitnessFromInitResult)
                || !Probe((NativeArchiveTripleCall)NativeLineageWitnessAppendResult)
                || !Probe(NativeRedeem))
            {
                return false;
            }
            NativeFree(IntPtr.Zero);
            return true;
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    private static bool TryProbeRecursiveCompactPaymentTokenSymbol()
    {
        try
        {
            var ok = Probe((NativeArchiveTripleCall)NativeRecursiveCompactPaymentToken);
            NativeFree(IntPtr.Zero);
            return ok;
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    private static bool TryProbeCompactPaymentTokenSymbol()
    {
        try
        {
            var ok = Probe((NativeArchiveCall)NativeCompactPaymentToken);
            NativeFree(IntPtr.Zero);
            return ok;
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    private static bool TryProbeRecursiveAggregationProofBundleSymbol()
    {
        try
        {
            var ok = Probe((NativeArchivePairCall)NativeRecursiveAggregationProofBundle);
            NativeFree(IntPtr.Zero);
            return ok;
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    private static bool TryProbePallasOpenEnvelopeBuilderSymbols()
    {
        try
        {
            var currentHopOk = Probe((NativeArchiveCall)NativeBuildPallasOpenEnvelopesArchive);
            var previousProofOk = Probe((NativeArchiveCall)NativeBuildPreviousProofOpenEnvelopesArchive);
            NativeFree(IntPtr.Zero);
            return currentHopOk && previousProofOk;
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    private static bool TryProbeRecursiveCompactPaymentTokenVerifierSymbol()
    {
        try
        {
            var code = NativeVerifyRecursiveCompactPaymentToken(
                MalformedArchiveProbe,
                (UIntPtr)MalformedArchiveProbe.Length,
                MalformedArchiveProbe,
                (UIntPtr)MalformedArchiveProbe.Length,
                out var valid);
            return code == ExpectedMalformedArchiveProbeErrorCode && valid == 0;
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    private static bool TryProbeRecursiveSpendCompactPaymentTokenProjectionVerifierSymbol()
    {
        try
        {
            var noHeightCode = NativeVerifyRecursiveSpendCompactPaymentTokenProjection(
                MalformedArchiveProbe,
                (UIntPtr)MalformedArchiveProbe.Length,
                MalformedArchiveProbe,
                (UIntPtr)MalformedArchiveProbe.Length,
                out var noHeightValid);
            if (noHeightCode != ExpectedMalformedArchiveProbeErrorCode || noHeightValid != 0)
            {
                return false;
            }
            var heightCode = NativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
                MalformedArchiveProbe,
                (UIntPtr)MalformedArchiveProbe.Length,
                MalformedArchiveProbe,
                (UIntPtr)MalformedArchiveProbe.Length,
                0,
                out var heightValid);
            return heightCode == ExpectedMalformedArchiveProbeErrorCode && heightValid == 0;
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    private static bool TryProbeRecursiveCompactPaymentTokenSurface()
    {
        return TryProbeRecursiveCompactPaymentTokenSymbol()
            && TryProbeRecursiveCompactPaymentTokenVerifierSymbol()
            && TryProbeRecursiveSpendCompactPaymentTokenProjectionSymbol();
    }

    private static bool TryProbeRecursiveSpendCompactPaymentTokenProjectionSymbol()
    {
        try
        {
            var ok = Probe((NativeArchiveCall)NativeRecursiveSpendCompactPaymentTokenFromBundle);
            NativeFree(IntPtr.Zero);
            return ok;
        }
        catch (DllNotFoundException)
        {
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            return false;
        }
        catch (BadImageFormatException)
        {
            return false;
        }
        catch (ArgumentException)
        {
            return false;
        }
        catch (InvalidOperationException)
        {
            return false;
        }
        catch (SystemException)
        {
            return false;
        }
    }

    private static bool Probe(NativeArchiveCall nativeCall)
    {
        var code = nativeCall(
            MalformedArchiveProbe,
            (UIntPtr)MalformedArchiveProbe.Length,
            out var outPtr,
            out var outLen);
        return ConsumeProbeResult(code, outPtr, outLen);
    }

    private static bool Probe(NativeArchivePairCall nativeCall)
    {
        var code = nativeCall(
            MalformedArchiveProbe,
            (UIntPtr)MalformedArchiveProbe.Length,
            MalformedArchiveProbe,
            (UIntPtr)MalformedArchiveProbe.Length,
            out var outPtr,
            out var outLen);
        return ConsumeProbeResult(code, outPtr, outLen);
    }

    private static bool Probe(NativeArchiveTripleCall nativeCall)
    {
        var code = nativeCall(
            MalformedArchiveProbe,
            (UIntPtr)MalformedArchiveProbe.Length,
            MalformedArchiveProbe,
            (UIntPtr)MalformedArchiveProbe.Length,
            MalformedArchiveProbe,
            (UIntPtr)MalformedArchiveProbe.Length,
            out var outPtr,
            out var outLen);
        return ConsumeProbeResult(code, outPtr, outLen);
    }

    private static bool ConsumeProbeResult(int code, IntPtr outPtr, UIntPtr outLen)
    {
        var expected = IsExpectedMalformedArchiveProbeResult(code, outPtr, outLen);
        if (outPtr != IntPtr.Zero)
        {
            NativeFree(outPtr);
        }
        return expected;
    }

    internal static bool IsExpectedMalformedArchiveProbeResult(int code, IntPtr outPtr, UIntPtr outLen)
    {
        return code == ExpectedMalformedArchiveProbeErrorCode
            && outPtr == IntPtr.Zero
            && outLen == UIntPtr.Zero;
    }

    private static bool TryGetAbiVersion(out uint version)
    {
        try
        {
            version = NativeAbiVersion();
            return true;
        }
        catch (DllNotFoundException)
        {
            version = 0;
            return false;
        }
        catch (EntryPointNotFoundException)
        {
            version = 0;
            return false;
        }
        catch (BadImageFormatException)
        {
            version = 0;
            return false;
        }
    }

    [DllImport(LibraryName, EntryPoint = "connect_norito_bridge_abi_version", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeAbiVersion();

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_init", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeInit(byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_append", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeAppend(byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_transition_profile_init", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeTransitionProfileInit(byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_transition_profile_append", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeTransitionProfileAppend(byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_lineage_append_boundary", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeLineageAppendBoundary(byte[] profilePtr, UIntPtr profileLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_lineage_witness_from_init_result", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeLineageWitnessFromInitResult(
        byte[] requestPtr,
        UIntPtr requestLen,
        byte[] bundlePtr,
        UIntPtr bundleLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_lineage_witness_append_result", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeLineageWitnessAppendResult(
        byte[] witnessPtr,
        UIntPtr witnessLen,
        byte[] requestPtr,
        UIntPtr requestLen,
        byte[] bundlePtr,
        UIntPtr bundleLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_verify", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeVerify(byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_redeem", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeRedeem(byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_prove_verified_compact_payment_token_with_records", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeCompactPaymentToken(
        byte[] recordBundlePtr,
        UIntPtr recordBundleLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeRecursiveAggregationProofBundle(
        byte[] recordBundlePtr,
        UIntPtr recordBundleLen,
        byte[] pallasOpenEnvelopesPtr,
        UIntPtr pallasOpenEnvelopesLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_build_pallas_open_envelopes_archive", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeBuildPallasOpenEnvelopesArchive(
        byte[] recordBundlePtr,
        UIntPtr recordBundleLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeBuildPreviousProofOpenEnvelopesArchive(
        byte[] previousBundlePtr,
        UIntPtr previousBundleLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeRecursiveCompactPaymentToken(
        byte[] recordBundlePtr,
        UIntPtr recordBundleLen,
        byte[] pallasOpenEnvelopesPtr,
        UIntPtr pallasOpenEnvelopesLen,
        byte[] recursiveCompactKeyArtifactsPtr,
        UIntPtr recursiveCompactKeyArtifactsLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_verify_recursive_compact_payment_token", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeVerifyRecursiveCompactPaymentToken(
        byte[] compactTokenPtr,
        UIntPtr compactTokenLen,
        byte[] recursiveCompactVerifierKeysPtr,
        UIntPtr recursiveCompactVerifierKeysLen,
        out byte valid);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeRecursiveSpendCompactPaymentTokenFromBundle(
        byte[] bundlePtr,
        UIntPtr bundleLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeVerifyRecursiveSpendCompactPaymentTokenProjection(
        byte[] compactTokenPtr,
        UIntPtr compactTokenLen,
        byte[] verifierRecordPtr,
        UIntPtr verifierRecordLen,
        out byte valid);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_verify_recursive_spend_compact_payment_token_projection_at_height", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeVerifyRecursiveSpendCompactPaymentTokenProjectionAtHeight(
        byte[] compactTokenPtr,
        UIntPtr compactTokenLen,
        byte[] verifierRecordPtr,
        UIntPtr verifierRecordLen,
        ulong blockHeight,
        out byte valid);

    [DllImport(LibraryName, EntryPoint = "connect_norito_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr ptr);
}
