using System;
using System.Buffers.Binary;
using System.Globalization;
using System.Numerics;
using System.Runtime.InteropServices;
using System.Security.Cryptography;
using System.Text;
using Hyperledger.Iroha.Norito;
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

public sealed class KagemushaRecursiveSpendableNoteSummary
{
    private readonly byte[] noteCommitment;
    private readonly byte[] spendNullifier;

    internal KagemushaRecursiveSpendableNoteSummary(
        ReadOnlySpan<byte> noteCommitment,
        ReadOnlySpan<byte> spendNullifier,
        string amount)
    {
        this.noteCommitment = noteCommitment.ToArray();
        this.spendNullifier = spendNullifier.ToArray();
        Amount = amount;
    }

    public byte[] NoteCommitment => noteCommitment.ToArray();

    public byte[] SpendNullifier => spendNullifier.ToArray();

    public string Amount { get; }
}

public sealed class KagemushaRecursiveSpendBundleSummary
{
    private readonly byte[] initialRoot;
    private readonly byte[] finalRoot;

    internal KagemushaRecursiveSpendBundleSummary(
        uint hopCount,
        string proofCircuitId,
        string asset,
        string chainId,
        ReadOnlySpan<byte> initialRoot,
        ReadOnlySpan<byte> finalRoot,
        KagemushaRecursiveSpendableNoteSummary currentNote)
    {
        HopCount = hopCount;
        ProofCircuitId = proofCircuitId;
        Asset = asset;
        ChainId = chainId;
        this.initialRoot = initialRoot.ToArray();
        this.finalRoot = finalRoot.ToArray();
        CurrentNote = currentNote;
    }

    public uint HopCount { get; }

    public string ProofCircuitId { get; }

    public string Asset { get; }

    public string ChainId { get; }

    public byte[] InitialRoot => initialRoot.ToArray();

    public byte[] FinalRoot => finalRoot.ToArray();

    public KagemushaRecursiveSpendableNoteSummary CurrentNote { get; }
}

public sealed class KagemushaRecursiveSpendVerifyResult
{
    internal KagemushaRecursiveSpendVerifyResult(
        bool valid,
        uint hopCount,
        uint encodedBytes,
        string reason,
        bool chainAdmissible,
        string chainAdmissionReason,
        bool witnesslessRedeemSupported,
        bool lineageWitnessRequired)
    {
        Valid = valid;
        HopCount = hopCount;
        EncodedBytes = encodedBytes;
        Reason = reason;
        ChainAdmissible = chainAdmissible;
        ChainAdmissionReason = chainAdmissionReason;
        WitnesslessRedeemSupported = witnesslessRedeemSupported;
        LineageWitnessRequired = lineageWitnessRequired;
    }

    public bool Valid { get; }

    public uint HopCount { get; }

    public uint EncodedBytes { get; }

    public string Reason { get; }

    public bool ChainAdmissible { get; }

    public string ChainAdmissionReason { get; }

    public bool WitnesslessRedeemSupported { get; }

    public bool LineageWitnessRequired { get; }
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
    public const string RecursiveSpendBundleWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendBundleV1";
    public const string RecursiveAggregationProofPublicInputsWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveAggregationProofPublicInputs";
    public const string RecursiveSpendVerifyResultWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendVerifyResultV1";
    public const string RecursiveSpendLineageWitnessWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendLineageWitnessV1";
    public const string RecursiveSpendAccumulatorDomain =
        "iroha:kagemusha:v1:recursive-spend-accumulator";

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
    private static readonly BigInteger MaxU128 = (BigInteger.One << 128) - BigInteger.One;
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

    private readonly record struct BundleAccumulatorSummary(
        string ChainId,
        string Asset,
        byte[] InitialRoot,
        byte[] FinalRoot,
        uint HopCount,
        KagemushaRecursiveSpendableNoteSummary CurrentNote);

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

    public static void ValidateVerifyLineagePreflight(
        string? proofCircuitId,
        bool hasLineageVerifierRecord)
    {
        if (IsLineageProofCircuitId(proofCircuitId))
        {
            if (!hasLineageVerifierRecord)
            {
                throw new ArgumentException(
                    "lineageVerifierRecord is required for reserved-lineage bundles",
                    nameof(hasLineageVerifierRecord));
            }
        }
        else if (hasLineageVerifierRecord)
        {
            throw new ArgumentException(
                "lineageVerifierRecord is only valid for reserved-lineage bundles",
                nameof(hasLineageVerifierRecord));
        }
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

    public static void ValidateRedeemChangeOutputBytes(ReadOnlySpan<byte> changeOutput)
    {
        if (changeOutput.Length != 32)
        {
            throw new ArgumentException(
                "changeOutput must be exactly 32 bytes",
                nameof(changeOutput));
        }
        if (IsZeroBytes(changeOutput))
        {
            throw new ArgumentException(
                "changeOutput must be non-zero",
                nameof(changeOutput));
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

    public static KagemushaRecursiveSpendVerifyResult DecodeVerifyResult(byte[] verifyResultArchive)
    {
        var (payload, flags) = KagemushaNoritoArchivePayload(
            verifyResultArchive,
            RecursiveSpendVerifyResultWireName,
            "verifyResult",
            nameof(verifyResultArchive));
        if (flags != KagemushaNoritoCompactLenFlag)
        {
            throw new ArgumentException("verifyResult must use compact Norito layout", nameof(verifyResultArchive));
        }

        var offset = 0;
        var valid = ReadBundleBoolPayload(
            ReadBundleField(payload, ref offset, flags, "verifyResult.valid"),
            "verifyResult.valid");
        var hopCount = ReadBundleU32Payload(
            ReadBundleField(payload, ref offset, flags, "verifyResult.hopCount"),
            "verifyResult.hopCount");
        var encodedBytes = ReadBundleU32Payload(
            ReadBundleField(payload, ref offset, flags, "verifyResult.encodedBytes"),
            "verifyResult.encodedBytes");
        var reason = DecodeBundleString(
            ReadBundleField(payload, ref offset, flags, "verifyResult.reason"),
            flags,
            "verifyResult.reason");
        var chainAdmissible = ReadBundleBoolPayload(
            ReadBundleField(payload, ref offset, flags, "verifyResult.chainAdmissible"),
            "verifyResult.chainAdmissible");
        var chainAdmissionReason = DecodeBundleString(
            ReadBundleField(payload, ref offset, flags, "verifyResult.chainAdmissionReason"),
            flags,
            "verifyResult.chainAdmissionReason");

        var witnesslessRedeemSupported = false;
        var lineageWitnessRequired = false;
        if (offset < payload.Length)
        {
            witnesslessRedeemSupported = ReadBundleBoolPayload(
                ReadBundleField(payload, ref offset, flags, "verifyResult.witnesslessRedeemSupported"),
                "verifyResult.witnesslessRedeemSupported");
        }
        if (offset < payload.Length)
        {
            lineageWitnessRequired = ReadBundleBoolPayload(
                ReadBundleField(payload, ref offset, flags, "verifyResult.lineageWitnessRequired"),
                "verifyResult.lineageWitnessRequired");
        }
        if (offset != payload.Length)
        {
            throw BundleDecodeError("verifyResult", "verifyResult has trailing bytes");
        }

        return new KagemushaRecursiveSpendVerifyResult(
            valid,
            hopCount,
            encodedBytes,
            reason,
            chainAdmissible,
            chainAdmissionReason,
            witnesslessRedeemSupported,
            lineageWitnessRequired);
    }

    public static bool LineageWitnessHasReservedPreviousProof(byte[] lineageWitnessArchive)
    {
        var (payload, flags) = KagemushaNoritoArchivePayload(
            lineageWitnessArchive,
            RecursiveSpendLineageWitnessWireName,
            "lineageWitness",
            nameof(lineageWitnessArchive));
        if (flags != KagemushaNoritoCompactLenFlag)
        {
            throw new ArgumentException(
                "lineageWitness must use compact Norito layout",
                nameof(lineageWitnessArchive));
        }

        var offset = SkipBundleFields(payload, 0, flags, 3, "lineageWitness");
        var previousProofsPayload = ReadBundleField(
            payload,
            ref offset,
            flags,
            "lineageWitness.previousRecursiveProofs");
        if (offset != payload.Length)
        {
            throw BundleDecodeError("lineageWitness", "lineageWitness has trailing bytes");
        }
        if (previousProofsPayload.Length < 8)
        {
            throw BundleDecodeError("lineageWitness.previousRecursiveProofs");
        }
        var count = BinaryPrimitives.ReadUInt64LittleEndian(previousProofsPayload.AsSpan(0, 8));
        if (count > int.MaxValue)
        {
            throw BundleDecodeError("lineageWitness.previousRecursiveProofs");
        }

        var proofOffset = 8;
        var hasReserved = false;
        for (var index = 0; index < (int)count; index++)
        {
            var proofPayload = ReadBundleField(
                previousProofsPayload,
                ref proofOffset,
                flags,
                $"lineageWitness.previousRecursiveProofs[{index}]");
            var circuitId = ReadLineagePreviousRecursiveProofCircuitId(proofPayload, flags);
            hasReserved = hasReserved || IsLineageProofCircuitId(circuitId);
        }
        if (proofOffset != previousProofsPayload.Length)
        {
            throw BundleDecodeError("lineageWitness.previousRecursiveProofs");
        }
        return hasReserved;
    }

    public static KagemushaRecursiveSpendBundleSummary DecodeBundleSummary(byte[] bundleArchive)
    {
        var (payload, flags) = KagemushaNoritoArchivePayload(
            bundleArchive,
            RecursiveSpendBundleWireName,
            "bundle",
            nameof(bundleArchive));
        if (flags != KagemushaNoritoCompactLenFlag)
        {
            throw new ArgumentException("bundle must use compact Norito layout", nameof(bundleArchive));
        }

        var offset = 0;
        var accumulatorPayload = ReadBundleField(payload, ref offset, flags, "bundle.accumulator");
        var proofPayload = ReadBundleField(payload, ref offset, flags, "bundle.proof");
        if (offset != payload.Length)
        {
            throw BundleDecodeError("bundle", "bundle has trailing bytes");
        }

        var accumulator = ReadBundleAccumulatorSummary(accumulatorPayload, flags);
        var proofCircuitId = ReadBundleRecursiveProofCircuitId(proofPayload, flags);
        if (!IsSupportedPreviousProofCircuitId(proofCircuitId))
        {
            throw BundleDecodeError(
                "bundle.proof_circuit_id",
                $"bundle.proof_circuit_id unsupported recursive proof circuit id: {proofCircuitId}");
        }

        return new KagemushaRecursiveSpendBundleSummary(
            accumulator.HopCount,
            proofCircuitId,
            accumulator.Asset,
            accumulator.ChainId,
            accumulator.InitialRoot,
            accumulator.FinalRoot,
            accumulator.CurrentNote);
    }

    private static (byte[] Payload, byte Flags) KagemushaNoritoArchivePayload(
        byte[] archive,
        string schema,
        string field,
        string parameterName)
    {
        var copy = KagemushaArchiveBytes.Copy(archive, parameterName);
        var expectedSchemaHash = NoritoCodec.SchemaHash(schema);
        if (!copy.AsSpan(6, 16).SequenceEqual(expectedSchemaHash))
        {
            throw new ArgumentException($"{field} must use {schema}", parameterName);
        }

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(copy.AsSpan(23, 8));
        if (payloadLength == 0
            || payloadLength > int.MaxValue
            || payloadLength > (ulong)(copy.Length - NoritoHeader.EncodedLength))
        {
            throw new ArgumentException($"{field} payload length is invalid", parameterName);
        }

        var minimumLength = NoritoHeader.EncodedLength + (int)payloadLength;
        if (copy.Length < minimumLength)
        {
            throw new ArgumentException($"{field} payload is truncated", parameterName);
        }

        var paddingLength = copy.Length - minimumLength;
        var payloadOffset = NoritoHeader.EncodedLength + paddingLength;
        return (copy.AsSpan(payloadOffset, (int)payloadLength).ToArray(), copy[39]);
    }

    private static BundleAccumulatorSummary ReadBundleAccumulatorSummary(byte[] payload, byte flags)
    {
        var offset = 0;
        var domain = DecodeBundleString(
            ReadBundleField(payload, ref offset, flags, "accumulator.domain"),
            flags,
            "bundle.accumulator.domain");
        if (domain != RecursiveSpendAccumulatorDomain)
        {
            throw BundleDecodeError(
                "bundle.accumulator.domain",
                $"bundle.accumulator.domain expected {RecursiveSpendAccumulatorDomain}");
        }

        var chainId = ReadBundleChainIdPayload(
            ReadBundleField(payload, ref offset, flags, "accumulator.chainId"),
            flags);
        var assetBytes = ReadBundleFixedBytesFlexible(
            ReadBundleField(payload, ref offset, flags, "accumulator.asset"),
            flags,
            16,
            "asset");
        var asset = "hex:" + Convert.ToHexString(assetBytes).ToLowerInvariant();
        var initialRoot = ReadBundleFixedBytesFlexible(
            ReadBundleField(payload, ref offset, flags, "accumulator.initialRoot"),
            flags,
            32,
            "initialRoot");
        var finalRoot = ReadBundleFixedBytesFlexible(
            ReadBundleField(payload, ref offset, flags, "accumulator.finalRoot"),
            flags,
            32,
            "finalRoot");
        offset = SkipBundleFields(payload, offset, flags, 1, "accumulator");
        var hopCount = ReadBundleU32Payload(
            ReadBundleField(payload, ref offset, flags, "accumulator.hopCount"),
            "bundle.accumulator.hop_count");
        if (hopCount < 1 || hopCount > RecursiveSpendLineageWitnesslessMaxHopsV1)
        {
            throw BundleDecodeError(
                "bundle.accumulator.hop_count",
                $"bundle.accumulator.hop_count must be in 1..{RecursiveSpendLineageWitnesslessMaxHopsV1}");
        }

        offset = SkipBundleFields(payload, offset, flags, 15, "accumulator");
        var currentNote = ReadBundleSpendableNotePayload(
            ReadBundleField(payload, ref offset, flags, "accumulator.currentNote"),
            flags);
        if (offset != payload.Length)
        {
            throw BundleDecodeError("bundle", "accumulator has trailing bytes");
        }

        return new BundleAccumulatorSummary(
            chainId,
            asset,
            initialRoot,
            finalRoot,
            hopCount,
            currentNote);
    }

    private static string ReadBundleChainIdPayload(byte[] payload, byte flags)
    {
        var offset = 0;
        var field = ReadBundleField(payload, ref offset, flags, "chainId");
        if (offset != payload.Length)
        {
            throw BundleDecodeError("bundle", "chainId has trailing bytes");
        }
        return DecodeBundleString(field, flags, "chainId");
    }

    private static string ReadBundleRecursiveProofCircuitId(byte[] payload, byte flags)
    {
        var offset = 0;
        var verifierPayload = ReadBundleField(
            payload,
            ref offset,
            flags,
            "recursiveProof.verifierKeyId");
        var publicInputsPayload = ReadBundleField(
            payload,
            ref offset,
            flags,
            "recursiveProof.publicInputs");
        if (publicInputsPayload.Length == 0)
        {
            throw BundleDecodeError("bundle.proof_public_inputs", "bundle.proof_public_inputs empty");
        }
        var publicInputsHash = ReadBundleFixedBytesFlexible(
            ReadBundleField(payload, ref offset, flags, "recursiveProof.publicInputsHash"),
            flags,
            32,
            "proof.publicInputsHash");
        if (IsZeroBytes(publicInputsHash))
        {
            throw BundleDecodeError(
                "bundle.proof_public_inputs_hash",
                "bundle.proof_public_inputs_hash empty");
        }
        var publicInputsArchive = NoritoCodec.Encode(
            RecursiveAggregationProofPublicInputsWireName,
            publicInputsPayload,
            KagemushaNoritoCompactLenFlag);
        if (!publicInputsHash.AsSpan().SequenceEqual(IrohaHash.Hash(publicInputsArchive)))
        {
            throw BundleDecodeError(
                "bundle.proof_public_inputs_hash",
                "bundle.proof_public_inputs_hash mismatch");
        }
        var proofBackend = ReadBundleProofBoxBackend(
            ReadBundleField(payload, ref offset, flags, "recursiveProof.proof"),
            flags);
        if (offset != payload.Length)
        {
            throw BundleDecodeError("bundle", "recursiveProof has trailing bytes");
        }
        if (proofBackend != RecursiveAggregationProofBackend)
        {
            throw BundleDecodeError(
                "bundle.proof_backend",
                $"bundle.proof_backend unsupported recursive proof backend: {proofBackend}");
        }

        var verifierOffset = 0;
        var backend = DecodeBundleString(
            ReadBundleField(verifierPayload, ref verifierOffset, flags, "verifierKeyId.backend"),
            flags,
            "verifierKeyId.backend");
        var name = DecodeBundleString(
            ReadBundleField(verifierPayload, ref verifierOffset, flags, "verifierKeyId.name"),
            flags,
            "verifierKeyId.name");
        if (verifierOffset != verifierPayload.Length)
        {
            throw BundleDecodeError("bundle", "verifierKeyId has trailing bytes");
        }

        RequireBundlePortableId(backend, "verifierKeyId.backend");
        if (backend != RecursiveAggregationProofBackend)
        {
            throw BundleDecodeError(
                "bundle.proof_backend",
                $"bundle.proof_backend unsupported recursive proof backend: {backend}");
        }
        if (proofBackend != backend)
        {
            throw BundleDecodeError(
                "bundle.proof_backend",
                $"bundle.proof_backend recursive proof backend mismatch: {proofBackend}");
        }
        RequireBundlePortableId(name, "verifierKeyId");
        return name;
    }

    private static bool IsZeroBytes(ReadOnlySpan<byte> bytes)
    {
        foreach (var value in bytes)
        {
            if (value != 0)
            {
                return false;
            }
        }
        return true;
    }

    private static string ReadBundleProofBoxBackend(byte[] payload, byte flags)
    {
        var offset = 0;
        var backend = DecodeBundleString(
            ReadBundleField(payload, ref offset, flags, "proof.backend"),
            flags,
            "proof.backend");
        var proofBytes = DecodeBundleByteVec(
            ReadBundleField(payload, ref offset, flags, "proof.bytes"),
            "proof.bytes");
        if (offset != payload.Length)
        {
            throw BundleDecodeError("bundle", "proof has trailing bytes");
        }
        RequireBundlePortableId(backend, "proof.backend");
        if (proofBytes.Length == 0)
        {
            throw BundleDecodeError("bundle.proof_bytes", "bundle.proof_bytes empty");
        }
        return backend;
    }

    private static string ReadLineagePreviousRecursiveProofCircuitId(byte[] payload, byte flags)
    {
        var offset = 0;
        var verifierPayload = ReadBundleField(
            payload,
            ref offset,
            flags,
            "lineageWitness.previousRecursiveProofs.verifierKeyId");
        offset = SkipBundleFields(
            payload,
            offset,
            flags,
            3,
            "lineageWitness.previousRecursiveProofs");
        if (offset != payload.Length)
        {
            throw BundleDecodeError("lineageWitness.previousRecursiveProofs");
        }

        var verifierOffset = 0;
        var backend = DecodeBundleString(
            ReadBundleField(
                verifierPayload,
                ref verifierOffset,
                flags,
                "lineageWitness.previousRecursiveProofs.verifierKeyId.backend"),
            flags,
            "lineageWitness.previousRecursiveProofs.verifierKeyId.backend");
        var name = DecodeBundleString(
            ReadBundleField(
                verifierPayload,
                ref verifierOffset,
                flags,
                "lineageWitness.previousRecursiveProofs.verifierKeyId.name"),
            flags,
            "lineageWitness.previousRecursiveProofs.verifierKeyId.name");
        if (verifierOffset != verifierPayload.Length)
        {
            throw BundleDecodeError("lineageWitness.previousRecursiveProofs.verifierKeyId");
        }

        RequireBundlePortableId(backend, "lineageWitness.previousRecursiveProofs.verifierKeyId.backend");
        if (backend != RecursiveAggregationProofBackend)
        {
            throw BundleDecodeError("lineageWitness.previousRecursiveProofs.verifierKeyId.backend");
        }
        RequireBundlePortableId(name, "lineageWitness.previousRecursiveProofs.verifierKeyId.name");
        if (!IsSupportedPreviousProofCircuitId(name))
        {
            throw BundleDecodeError("lineageWitness.previousRecursiveProofs.verifierKeyId.name");
        }
        return name;
    }

    private static KagemushaRecursiveSpendableNoteSummary ReadBundleSpendableNotePayload(
        byte[] payload,
        byte flags)
    {
        var offset = 0;
        var noteCommitment = ReadBundleFixedBytesFlexible(
            ReadBundleField(payload, ref offset, flags, "currentNote.noteCommitment"),
            flags,
            32,
            "noteCommitment");
        var spendNullifier = ReadBundleFixedBytesFlexible(
            ReadBundleField(payload, ref offset, flags, "currentNote.spendNullifier"),
            flags,
            32,
            "spendNullifier");
        var amount = DecodeBundleNumericAmount(
            ReadBundleField(payload, ref offset, flags, "currentNote.amount"),
            flags,
            "bundle.current_note.amount");
        if (offset != payload.Length)
        {
            throw BundleDecodeError("bundle", "currentNote has trailing bytes");
        }
        if (IsZeroBytes(noteCommitment))
        {
            throw BundleDecodeError(
                "bundle.current_note.note_commitment",
                "bundle.current_note.note_commitment must not be all-zero");
        }
        if (IsZeroBytes(spendNullifier))
        {
            throw BundleDecodeError(
                "bundle.current_note.spend_nullifier",
                "bundle.current_note.spend_nullifier must not be all-zero");
        }
        if (noteCommitment.AsSpan().SequenceEqual(spendNullifier))
        {
            throw BundleDecodeError(
                "bundle.current_note",
                "bundle.current_note note commitment and spend nullifier must differ");
        }
        return new KagemushaRecursiveSpendableNoteSummary(noteCommitment, spendNullifier, amount);
    }

    private static byte[] ReadBundleFixedBytesFlexible(
        byte[] payload,
        byte flags,
        int expectedSize,
        string field)
    {
        if (payload.Length == expectedSize)
        {
            return payload.ToArray();
        }

        try
        {
            return ReadBundleConstVecPayload(payload, flags, 0, expectedSize, field);
        }
        catch (ArgumentException)
        {
            if (payload.Length < 8)
            {
                throw BundleDecodeError(field);
            }
            var count = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(0, 8));
            if (count != (ulong)expectedSize)
            {
                throw BundleDecodeError(field);
            }
            return ReadBundleConstVecPayload(payload, flags, 8, expectedSize, field);
        }
    }

    private static byte[] ReadBundleConstVecPayload(
        byte[] payload,
        byte flags,
        int start,
        int expectedSize,
        string field)
    {
        var offset = start;
        var output = new byte[expectedSize];
        var index = 0;
        while (offset < payload.Length)
        {
            var fieldPayload = ReadBundleField(payload, ref offset, flags, field);
            if (fieldPayload.Length != 1 || index >= expectedSize)
            {
                throw BundleDecodeError(field);
            }
            output[index++] = fieldPayload[0];
        }
        if (index != expectedSize)
        {
            throw BundleDecodeError(field);
        }
        return output;
    }

    private static string DecodeBundleNumericAmount(byte[] payload, byte flags, string field)
    {
        var offset = 0;
        var mantissaPayload = ReadBundleField(payload, ref offset, flags, "amount.mantissa");
        var scalePayload = ReadBundleField(payload, ref offset, flags, "amount.scale");
        if (offset != payload.Length || mantissaPayload.Length < 4)
        {
            throw BundleDecodeError(field);
        }

        var mantissaLength = BinaryPrimitives.ReadUInt32LittleEndian(mantissaPayload.AsSpan(0, 4));
        if (mantissaLength > int.MaxValue
            || 4 + (int)mantissaLength != mantissaPayload.Length)
        {
            throw BundleDecodeError(field);
        }
        if (scalePayload.Length != 4
            || BinaryPrimitives.ReadUInt32LittleEndian(scalePayload) != 0)
        {
            throw BundleDecodeError(field, $"{field} numeric scale must be zero");
        }

        var integer = new BigInteger(
            mantissaPayload.AsSpan(4, (int)mantissaLength),
            isUnsigned: false,
            isBigEndian: false);
        if (integer <= BigInteger.Zero || integer > MaxU128)
        {
            throw BundleDecodeError(field, $"{field} must fit in u128");
        }
        return integer.ToString(CultureInfo.InvariantCulture);
    }

    private static byte[] DecodeBundleByteVec(byte[] payload, string field)
    {
        if (payload.Length < 8)
        {
            throw BundleDecodeError(field);
        }
        var length = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(0, 8));
        if (length > int.MaxValue || length != (ulong)(payload.Length - 8))
        {
            throw BundleDecodeError(field);
        }
        return payload.AsSpan(8, (int)length).ToArray();
    }

    private static uint ReadBundleU32Payload(byte[] payload, string field)
    {
        if (payload.Length != 4)
        {
            throw BundleDecodeError(field);
        }
        return BinaryPrimitives.ReadUInt32LittleEndian(payload);
    }

    private static bool ReadBundleBoolPayload(byte[] payload, string field)
    {
        if (payload.Length != 1 || payload[0] > 1)
        {
            throw BundleDecodeError(field);
        }
        return payload[0] == 1;
    }

    private static string DecodeBundleString(byte[] payload, byte flags, string field)
    {
        try
        {
            var offset = 0;
            var length = ReadBundleLength(payload, ref offset, flags, field);
            if (length != payload.Length - offset)
            {
                throw BundleDecodeError(field);
            }
            return StrictUtf8.GetString(payload, offset, length);
        }
        catch (DecoderFallbackException)
        {
            throw BundleDecodeError(field);
        }
    }

    private static int SkipBundleFields(
        byte[] payload,
        int offset,
        byte flags,
        int count,
        string field)
    {
        var cursor = offset;
        for (var index = 0; index < count; index++)
        {
            _ = ReadBundleField(payload, ref cursor, flags, field);
        }
        return cursor;
    }

    private static byte[] ReadBundleField(byte[] buffer, ref int offset, byte flags, string field)
    {
        var length = ReadBundleLength(buffer, ref offset, flags, field);
        if (length > buffer.Length - offset)
        {
            throw BundleDecodeError(field);
        }
        var result = buffer.AsSpan(offset, length).ToArray();
        offset += length;
        return result;
    }

    private static int ReadBundleLength(byte[] buffer, ref int offset, byte flags, string field)
    {
        if ((flags & KagemushaNoritoCompactLenFlag) == 0)
        {
            if (offset + 8 > buffer.Length)
            {
                throw BundleDecodeError(field);
            }
            var fixedLength = BinaryPrimitives.ReadUInt64LittleEndian(buffer.AsSpan(offset, 8));
            if (fixedLength > int.MaxValue)
            {
                throw BundleDecodeError(field);
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
                throw BundleDecodeError(field);
            }
            var current = buffer[offset++];
            var currentValue = current & 0x7f;
            if (shift >= 63 && currentValue > 1)
            {
                throw BundleDecodeError(field);
            }
            value |= (ulong)currentValue << shift;
            if ((current & 0x80) == 0)
            {
                var encodedLength = offset - startOffset;
                if (encodedLength > 1 && value < (1UL << (7 * (encodedLength - 1))))
                {
                    throw BundleDecodeError(field);
                }
                if (value > int.MaxValue)
                {
                    throw BundleDecodeError(field);
                }
                return (int)value;
            }
            shift += 7;
        }
        throw BundleDecodeError(field);
    }

    private static void RequireBundlePortableId(string value, string field)
    {
        if (string.IsNullOrWhiteSpace(value) || value.Trim() != value)
        {
            throw BundleDecodeError(field, $"{field} must be a non-empty unpadded string");
        }
        if (value.Length > 256)
        {
            throw BundleDecodeError(field, $"{field} must use portable registry syntax");
        }
        foreach (var ch in value)
        {
            if (ch >= 'A' && ch <= 'Z'
                || ch >= 'a' && ch <= 'z'
                || ch >= '0' && ch <= '9'
                || ch == '.'
                || ch == '_'
                || ch == '-'
                || ch == '/'
                || ch == ':'
                || ch == '@'
                || ch == '+'
                || ch == '=')
            {
                continue;
            }
            throw BundleDecodeError(field, $"{field} must use portable registry syntax");
        }
    }

    private static ArgumentException BundleDecodeError(string field, string? message = null)
    {
        return new ArgumentException(message ?? field, "bundleArchive");
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

    public static KagemushaRecursiveSpendVerifyArchive Verify(
        ReadOnlySpan<byte> requestArchive,
        string? proofCircuitId,
        bool hasLineageVerifierRecord)
    {
        ValidateVerifyLineagePreflight(proofCircuitId, hasLineageVerifierRecord);
        return Verify(requestArchive);
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
        string? publicAmount,
        string? currentNoteAmount,
        ReadOnlySpan<byte> changeOutput)
    {
        ValidateRedeemChangeOutputPreflight(
            publicAmount,
            currentNoteAmount,
            hasChangeOutput: true);
        ValidateRedeemChangeOutputBytes(changeOutput);
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

    public static KagemushaRecursiveSpendRedeemInstructionArchive Redeem(
        ReadOnlySpan<byte> requestArchive,
        string? proofCircuitId,
        uint hopCount,
        bool hasLineageWitness,
        bool hasLineageVerifierRecord,
        string? publicAmount,
        string? currentNoteAmount,
        ReadOnlySpan<byte> changeOutput)
    {
        ValidateRedeemLineagePreflight(
            proofCircuitId,
            hopCount,
            hasLineageWitness,
            hasLineageVerifierRecord);
        ValidateRedeemChangeOutputPreflight(
            publicAmount,
            currentNoteAmount,
            hasChangeOutput: true);
        ValidateRedeemChangeOutputBytes(changeOutput);
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
