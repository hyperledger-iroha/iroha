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

public sealed class KagemushaRecursiveSpendableNoteDescriptor
{
    private static readonly BigInteger DescriptorMaxU128 = (BigInteger.One << 128) - BigInteger.One;
    private readonly byte[] noteCommitment;
    private readonly byte[] spendNullifier;

    public KagemushaRecursiveSpendableNoteDescriptor(
        ReadOnlySpan<byte> noteCommitment,
        ReadOnlySpan<byte> spendNullifier,
        string amount)
    {
        if (noteCommitment.Length != 32)
        {
            throw new ArgumentException("noteCommitment must be exactly 32 bytes.", nameof(noteCommitment));
        }
        if (spendNullifier.Length != 32)
        {
            throw new ArgumentException("spendNullifier must be exactly 32 bytes.", nameof(spendNullifier));
        }
        if (AllZero(noteCommitment))
        {
            throw new ArgumentException("noteCommitment must be non-zero.", nameof(noteCommitment));
        }
        if (AllZero(spendNullifier))
        {
            throw new ArgumentException("spendNullifier must be non-zero.", nameof(spendNullifier));
        }
        if (noteCommitment.SequenceEqual(spendNullifier))
        {
            throw new ArgumentException("spendNullifier must differ from noteCommitment.", nameof(spendNullifier));
        }

        this.noteCommitment = noteCommitment.ToArray();
        this.spendNullifier = spendNullifier.ToArray();
        Amount = CanonicalU128Decimal(amount);
    }

    public byte[] NoteCommitment => noteCommitment.ToArray();

    public byte[] SpendNullifier => spendNullifier.ToArray();

    public string Amount { get; }

    private static bool AllZero(ReadOnlySpan<byte> bytes)
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

    private static string CanonicalU128Decimal(string? value)
    {
        if (value is null)
        {
            throw new ArgumentNullException(nameof(value));
        }
        if (value.Length == 0)
        {
            throw new ArgumentException("amount must be a decimal integer.", nameof(value));
        }
        foreach (var ch in value)
        {
            if (ch < '0' || ch > '9')
            {
                throw new ArgumentException("amount must be a decimal integer.", nameof(value));
            }
        }
        if (value.Length > 1 && value[0] == '0')
        {
            throw new ArgumentException("amount must be canonical.", nameof(value));
        }
        var parsed = BigInteger.Parse(value, CultureInfo.InvariantCulture);
        if (parsed <= BigInteger.Zero)
        {
            throw new ArgumentException("amount must be greater than zero.", nameof(value));
        }
        if (parsed > DescriptorMaxU128)
        {
            throw new ArgumentException("amount must fit in u128.", nameof(value));
        }
        return value;
    }
}

public sealed class KagemushaRecursiveSpendBundleSummary
{
    private readonly byte[] initialRoot;
    private readonly byte[] finalRoot;
    private readonly byte[][] topupAnchorNullifiers;

    internal KagemushaRecursiveSpendBundleSummary(
        uint hopCount,
        string proofCircuitId,
        string asset,
        string chainId,
        ReadOnlySpan<byte> initialRoot,
        ReadOnlySpan<byte> finalRoot,
        byte[][] topupAnchorNullifiers,
        KagemushaRecursiveSpendableNoteSummary currentNote)
    {
        HopCount = hopCount;
        ProofCircuitId = proofCircuitId;
        Asset = asset;
        ChainId = chainId;
        this.initialRoot = initialRoot.ToArray();
        this.finalRoot = finalRoot.ToArray();
        this.topupAnchorNullifiers = CopyByteArrays(topupAnchorNullifiers);
        CurrentNote = currentNote;
    }

    public uint HopCount { get; }

    public string ProofCircuitId { get; }

    public string Asset { get; }

    public string ChainId { get; }

    public byte[] InitialRoot => initialRoot.ToArray();

    public byte[] FinalRoot => finalRoot.ToArray();

    public IReadOnlyList<byte[]> TopupAnchorNullifiers => CopyByteArrays(topupAnchorNullifiers);

    public KagemushaRecursiveSpendableNoteSummary CurrentNote { get; }

    private static byte[][] CopyByteArrays(IReadOnlyList<byte[]> values)
    {
        var copies = new byte[values.Count][];
        for (var index = 0; index < values.Count; index++)
        {
            copies[index] = values[index].ToArray();
        }
        return copies;
    }
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

public sealed class KagemushaRecursiveSpendTransitionProfileSummary
{
    private readonly byte[][] previousTopupAnchorNullifiers;
    private readonly byte[][] currentHopOutputCommitments;

    internal KagemushaRecursiveSpendTransitionProfileSummary(
        uint hopIndex,
        uint hopCount,
        byte[][] previousTopupAnchorNullifiers,
        byte[][] currentHopOutputCommitments,
        KagemushaRecursiveSpendableNoteSummary currentNote)
    {
        HopIndex = hopIndex;
        HopCount = hopCount;
        this.previousTopupAnchorNullifiers = CopyByteArrays(previousTopupAnchorNullifiers);
        this.currentHopOutputCommitments = CopyByteArrays(currentHopOutputCommitments);
        CurrentNote = currentNote;
    }

    public uint HopIndex { get; }

    public uint HopCount { get; }

    public bool HasPriorState => HopIndex > 0;

    public IReadOnlyList<byte[]> PreviousTopupAnchorNullifiers => CopyByteArrays(previousTopupAnchorNullifiers);

    public IReadOnlyList<byte[]> CurrentHopOutputCommitments => CopyByteArrays(currentHopOutputCommitments);

    public KagemushaRecursiveSpendableNoteSummary CurrentNote { get; }

    private static byte[][] CopyByteArrays(IReadOnlyList<byte[]> values)
    {
        var copies = new byte[values.Count][];
        for (var index = 0; index < values.Count; index++)
        {
            copies[index] = values[index].ToArray();
        }
        return copies;
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
    public const string RecursiveSpendInitRequestWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendInitRequestV1";
    public const string RecursiveSpendAppendRequestWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendAppendRequestV1";
    public const string RecursiveSpendLineageWitnessWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendLineageWitnessV1";
    public const string RecursiveSpendTransitionProfileWireName =
        "iroha_data_model::offline::model::KagemushaRecursiveSpendTransitionProfileV1";
    public const string VerifiedFoldRecordBundleWireName =
        "iroha_data_model::offline::model::KagemushaVerifiedFoldRecordBundle";
    public const string VerifyingKeyRecordWireName =
        "iroha_data_model::proof::VerifyingKeyRecord";
    public const string RecursiveSpendAccumulatorDomain =
        "iroha:kagemusha:v1:recursive-spend-accumulator";

    public const uint RequiredNativeBridgeAbiVersion = 6;
    public const uint RecursiveCompactRequiredNativeBridgeAbiVersion = 7;
    public const uint CompactTokenMaxHops = 64;
    public const int FoldStepMaxInputs = 2;
    public const int FoldStepMaxOutputs = 2;
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
    private static readonly byte[] ZeroClearChunk = new byte[8192];
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
    private static readonly byte[] PallasOpenEnvelopeVectorSchemaHash = new byte[]
    {
        0xfe, 0x38, 0x26, 0x32, 0x8f, 0x08, 0x17, 0x71,
        0x75, 0x0f, 0x24, 0xfe, 0x11, 0x02, 0x60, 0xca,
    };
    private const ushort PallasCurveId = 1;
    private const int RecursivePallasOpenEnvelopeMaxK = 24;
    private const int RecursivePallasOpenEnvelopeMaxN = 1 << RecursivePallasOpenEnvelopeMaxK;
    private const string BundleTopupAnchorNullifiersField = "bundle.accumulator.topup_anchor_nullifiers";
    private const string BundleTopupAnchorNullifiersCountError =
        "bundle.accumulator.topup_anchor_nullifiers count is out of range";
    private const string BundleTopupAnchorNullifiersZeroError =
        "bundle.accumulator.topup_anchor_nullifiers must not contain zero values";
    private const string BundleTopupAnchorNullifiersOrderError =
        "bundle.accumulator.topup_anchor_nullifiers must be strictly sorted and unique";
    private const string BundleTopupAnchorNullifiersCurrentNoteReuseError =
        "bundle.accumulator.topup_anchor_nullifiers must not reuse current note material";
    private const string TransitionProfilePreviousTopupAnchorNullifiersField =
        "transition_profile.previous_topup_anchor_nullifiers";
    private const string TransitionProfilePreviousTopupAnchorNullifiersCountError =
        "transition_profile.previous_topup_anchor_nullifiers count is out of range";
    private const string TransitionProfilePreviousTopupAnchorNullifiersZeroError =
        "transition_profile.previous_topup_anchor_nullifiers must not contain zero values";
    private const string TransitionProfilePreviousTopupAnchorNullifiersOrderError =
        "transition_profile.previous_topup_anchor_nullifiers must be strictly sorted and unique";
    private const string TransitionProfilePreviousTopupAnchorNullifiersCurrentNoteReuseError =
        "transition_profile.previous_topup_anchor_nullifiers must not reuse current note material";
    private const string TransitionProfileOutputCommitmentsPreviousTopupAnchorReuseError =
        "transition_profile.output_commitments must not reuse previous top-up anchor nullifiers";
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
        byte[][] TopupAnchorNullifiers,
        uint HopCount,
        KagemushaRecursiveSpendableNoteSummary CurrentNote);

    private readonly record struct TransitionProfileStepSummary(
        byte[][] InputNullifiers,
        byte[][] OutputCommitments);

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
        if (recursiveCompactAvailable)
        {
            return KagemushaOfflineSpendMode.RecursiveCompactV1;
        }

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
        ValidateRedeemLineagePreflight(
            proofCircuitId,
            hopCount,
            hasLineageWitness,
            hasLineageVerifierRecord,
            lineageVerifierRecordsCount: 0,
            lineageWitnessHasReservedPreviousProofs: false);
    }

    public static void ValidateRedeemLineagePreflight(
        string? proofCircuitId,
        uint hopCount,
        bool hasLineageWitness,
        bool hasLineageVerifierRecord,
        int lineageVerifierRecordsCount)
    {
        ValidateRedeemLineagePreflight(
            proofCircuitId,
            hopCount,
            hasLineageWitness,
            hasLineageVerifierRecord,
            lineageVerifierRecordsCount,
            lineageWitnessHasReservedPreviousProofs: false);
    }

    public static void ValidateRedeemLineagePreflight(
        string? proofCircuitId,
        uint hopCount,
        bool hasLineageWitness,
        bool hasLineageVerifierRecord,
        int lineageVerifierRecordsCount,
        bool lineageWitnessHasReservedPreviousProofs)
    {
        if (RequiresLineageWitnessForRedeem(proofCircuitId, hopCount) && !hasLineageWitness)
        {
            throw new ArgumentException(
                "lineageWitness is required for this bundle",
                nameof(hasLineageWitness));
        }

        if (lineageVerifierRecordsCount < 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(lineageVerifierRecordsCount),
                "lineageVerifierRecordsCount must be non-negative.");
        }

        if ((uint)lineageVerifierRecordsCount > CompactTokenMaxHops)
        {
            throw new ArgumentOutOfRangeException(
                nameof(lineageVerifierRecordsCount),
                $"lineageVerifierRecordsCount must not exceed {CompactTokenMaxHops}.");
        }

        if (lineageWitnessHasReservedPreviousProofs && !hasLineageWitness)
        {
            throw new ArgumentException(
                "lineageWitnessHasReservedPreviousProofs requires lineageWitness",
                nameof(lineageWitnessHasReservedPreviousProofs));
        }

        var hasLineageVerifierRecords = hasLineageVerifierRecord || lineageVerifierRecordsCount > 0;
        if (IsLineageProofCircuitId(proofCircuitId))
        {
            if (!hasLineageVerifierRecords)
            {
                throw new ArgumentException(
                    "lineageVerifierRecord is required for reserved-lineage bundles",
                    nameof(hasLineageVerifierRecord));
            }
        }
        else if (lineageWitnessHasReservedPreviousProofs)
        {
            if (!hasLineageVerifierRecords)
            {
                throw new ArgumentException(
                    "lineageVerifierRecord is required for lineage witnesses with reserved-lineage proofs",
                    nameof(hasLineageVerifierRecord));
            }
        }
        else if (hasLineageVerifierRecords)
        {
            throw new ArgumentException(
                "lineageVerifierRecord is only valid for reserved-lineage bundles or lineage witnesses with reserved-lineage proofs",
                hasLineageVerifierRecord ? nameof(hasLineageVerifierRecord) : nameof(lineageVerifierRecordsCount));
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

    public static void ValidateRedeemChangeOutputNotReserved(
        ReadOnlySpan<byte> changeOutput,
        KagemushaRecursiveSpendBundleSummary bundleSummary)
    {
        ArgumentNullException.ThrowIfNull(bundleSummary);
        ValidateRedeemChangeOutputBytes(changeOutput);

        var currentNote = bundleSummary.CurrentNote;
        if (changeOutput.SequenceEqual(currentNote.NoteCommitment)
            || changeOutput.SequenceEqual(currentNote.SpendNullifier))
        {
            throw new ArgumentException(
                "changeOutput must not reuse the current note commitment, redeem nullifier, or top-up anchor nullifier",
                nameof(changeOutput));
        }

        foreach (var nullifier in bundleSummary.TopupAnchorNullifiers)
        {
            if (changeOutput.SequenceEqual(nullifier))
            {
                throw new ArgumentException(
                    "changeOutput must not reuse the current note commitment, redeem nullifier, or top-up anchor nullifier",
                    nameof(changeOutput));
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
            : outputCircuitId;
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
        if (count > CompactTokenMaxHops)
        {
            throw BundleDecodeError(
                "lineageWitness.previousRecursiveProofs",
                $"lineageWitness.previousRecursiveProofs count must not exceed {CompactTokenMaxHops}");
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
            accumulator.TopupAnchorNullifiers,
            accumulator.CurrentNote);
    }

    public static KagemushaRecursiveSpendTransitionProfileSummary DecodeTransitionProfileSummary(
        byte[] transitionProfileArchive)
    {
        var (payload, flags) = KagemushaNoritoArchivePayload(
            transitionProfileArchive,
            RecursiveSpendTransitionProfileWireName,
            "transitionProfile",
            nameof(transitionProfileArchive));
        if (flags != KagemushaNoritoCompactLenFlag)
        {
            throw new ArgumentException(
                "transitionProfile must use compact Norito layout",
                nameof(transitionProfileArchive));
        }

        var offset = 0;
        var domain = DecodeBundleString(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.domain"),
            flags,
            "transition_profile.domain");
        if (domain != RecursiveSpendTransitionProfileDomain)
        {
            throw TransitionProfileDecodeError(
                "transition_profile.domain",
                $"transition_profile.domain expected {RecursiveSpendTransitionProfileDomain}");
        }

        offset = SkipBundleFields(payload, offset, flags, 2, "transitionProfile");
        var priorStateDigestPayload = ReadBundleOptionPayload(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.previousStateDigest"),
            flags,
            "transition_profile.previous_state_digest");
        offset = SkipBundleFields(payload, offset, flags, 3, "transitionProfile");
        var previousTopupAnchorNullifiers = ReadTransitionProfileTopupAnchorNullifiersPayload(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.previousTopupAnchorNullifiers"),
            flags);
        offset = SkipBundleFields(payload, offset, flags, 10, "transitionProfile");
        var hopIndex = ReadBundleU32Payload(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.hopIndex"),
            "transition_profile.hop_index");
        var hopCount = ReadBundleU32Payload(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.hopCount"),
            "transition_profile.hop_count");
        var currentHopStatement = ReadTransitionProfileStepStatement(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.currentHopStatement"),
            flags);
        var currentNote = ReadBundleSpendableNotePayload(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.currentNote"),
            flags);
        offset = SkipBundleFields(payload, offset, flags, 18, "transitionProfile");
        if (offset != payload.Length)
        {
            throw TransitionProfileDecodeError("transition_profile", "transition_profile has trailing bytes");
        }

        RequireTransitionProfilePreviousTopupAnchors(
            priorStateDigestPayload is not null,
            hopIndex,
            hopCount,
            previousTopupAnchorNullifiers,
            currentNote,
            currentHopStatement.OutputCommitments);

        return new KagemushaRecursiveSpendTransitionProfileSummary(
            hopIndex,
            hopCount,
            previousTopupAnchorNullifiers,
            currentHopStatement.OutputCommitments,
            currentNote);
    }

    private static byte[]? ReadBundleOptionPayload(byte[] payload, byte flags, string field)
    {
        if (payload.Length == 0)
        {
            throw TransitionProfileDecodeError(field);
        }
        var tag = payload[0];
        if (tag == 0)
        {
            if (payload.Length != 1)
            {
                throw TransitionProfileDecodeError(field);
            }
            return null;
        }
        if (tag != 1)
        {
            throw TransitionProfileDecodeError(field);
        }

        var offset = 1;
        var length = ReadBundleLength(payload, ref offset, flags, field);
        if (length > payload.Length - offset)
        {
            throw TransitionProfileDecodeError(field);
        }
        var value = payload.AsSpan(offset, length).ToArray();
        offset += length;
        if (offset != payload.Length)
        {
            throw TransitionProfileDecodeError(field);
        }
        return value;
    }

    private static byte[][] ReadTransitionProfileTopupAnchorNullifiersPayload(byte[] payload, byte flags)
    {
        return ReadTransitionProfileFixed32SequencePayload(
            payload,
            flags,
            TransitionProfilePreviousTopupAnchorNullifiersField,
            FoldStepMaxInputs,
            allowEmpty: true);
    }

    private static TransitionProfileStepSummary ReadTransitionProfileStepStatement(byte[] payload, byte flags)
    {
        var offset = 0;
        _ = ReadBundleU32Payload(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.currentHopStatement.hopIndex"),
            "transition_profile.current_hop_statement.hop_index");
        _ = ReadBundleFixedBytesFlexible(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.currentHopStatement.rootBefore"),
            flags,
            32,
            "transition_profile.current_hop_statement.root_before");
        var inputNullifiers = ReadTransitionProfileFixed32SequencePayload(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.currentHopStatement.inputNullifiers"),
            flags,
            "transition_profile.input_nullifiers",
            FoldStepMaxInputs,
            allowEmpty: false);
        var outputCommitments = ReadTransitionProfileFixed32SequencePayload(
            ReadBundleField(payload, ref offset, flags, "transitionProfile.currentHopStatement.outputCommitments"),
            flags,
            "transition_profile.output_commitments",
            FoldStepMaxOutputs,
            allowEmpty: false);
        offset = SkipBundleFields(payload, offset, flags, 6, "transitionProfile.currentHopStatement");
        if (offset != payload.Length)
        {
            throw TransitionProfileDecodeError(
                "transition_profile.current_hop_statement",
                "transition_profile.current_hop_statement has trailing bytes");
        }
        return new TransitionProfileStepSummary(inputNullifiers, outputCommitments);
    }

    private static byte[][] ReadTransitionProfileFixed32SequencePayload(
        byte[] payload,
        byte flags,
        string field,
        int maxCount,
        bool allowEmpty)
    {
        if (payload.Length < 8)
        {
            throw TransitionProfileDecodeError(field);
        }
        var count = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(0, 8));
        if ((!allowEmpty && count == 0) || count > (ulong)maxCount)
        {
            throw TransitionProfileDecodeError(field, $"{field} count is out of range");
        }
        if (count > int.MaxValue)
        {
            throw TransitionProfileDecodeError(field);
        }

        var values = new byte[(int)count][];
        var offset = 8;
        for (var index = 0; index < values.Length; index++)
        {
            values[index] = ReadBundleFixedBytesFlexible(
                ReadBundleField(payload, ref offset, flags, $"{field}[{index}]"),
                flags,
                32,
                $"{field}[{index}]");
            if (IsZeroBytes(values[index]))
            {
                throw TransitionProfileDecodeError(field, $"{field} must not contain zero values");
            }
            if (index > 0 && CompareFixedBytes(values[index - 1], values[index]) >= 0)
            {
                throw TransitionProfileDecodeError(field, $"{field} must be strictly sorted and unique");
            }
        }
        if (offset != payload.Length)
        {
            throw TransitionProfileDecodeError(field, $"{field} has trailing bytes");
        }
        return values;
    }

    private static void RequireTransitionProfilePreviousTopupAnchors(
        bool priorStateDigestPresent,
        uint hopIndex,
        uint hopCount,
        byte[][] previousTopupAnchorNullifiers,
        KagemushaRecursiveSpendableNoteSummary currentNote,
        byte[][] currentHopOutputCommitments)
    {
        var hasPrevious = hopIndex > 0;
        if (hopCount == 0 || hopCount > CompactTokenMaxHops || hopIndex == uint.MaxValue || hopCount != hopIndex + 1)
        {
            throw TransitionProfileDecodeError(
                "transition_profile.hop_count",
                "transition_profile.hop_count must equal hop_index + 1 and stay within CompactTokenMaxHops");
        }
        if (priorStateDigestPresent != hasPrevious)
        {
            throw TransitionProfileDecodeError(
                "transition_profile.previous_state_digest",
                "transition_profile.previous_state_digest presence must match hop_index");
        }
        if (!hasPrevious)
        {
            if (previousTopupAnchorNullifiers.Length != 0)
            {
                throw TransitionProfileDecodeError(
                    TransitionProfilePreviousTopupAnchorNullifiersField,
                    TransitionProfilePreviousTopupAnchorNullifiersCountError);
            }
            return;
        }

        if (previousTopupAnchorNullifiers.Length == 0 || previousTopupAnchorNullifiers.Length > FoldStepMaxInputs)
        {
            throw TransitionProfileDecodeError(
                TransitionProfilePreviousTopupAnchorNullifiersField,
                TransitionProfilePreviousTopupAnchorNullifiersCountError);
        }

        byte[]? previous = null;
        foreach (var nullifier in previousTopupAnchorNullifiers)
        {
            if (IsZeroBytes(nullifier))
            {
                throw TransitionProfileDecodeError(
                    TransitionProfilePreviousTopupAnchorNullifiersField,
                    TransitionProfilePreviousTopupAnchorNullifiersZeroError);
            }
            if (previous is not null && CompareFixedBytes(previous, nullifier) >= 0)
            {
                throw TransitionProfileDecodeError(
                    TransitionProfilePreviousTopupAnchorNullifiersField,
                    TransitionProfilePreviousTopupAnchorNullifiersOrderError);
            }
            previous = nullifier;
        }

        foreach (var outputCommitment in currentHopOutputCommitments)
        {
            if (previousTopupAnchorNullifiers.Any(nullifier => outputCommitment.AsSpan().SequenceEqual(nullifier)))
            {
                throw TransitionProfileDecodeError(
                    "transition_profile.output_commitments",
                    TransitionProfileOutputCommitmentsPreviousTopupAnchorReuseError);
            }
        }

        var noteCommitment = currentNote.NoteCommitment;
        var spendNullifier = currentNote.SpendNullifier;
        foreach (var nullifier in previousTopupAnchorNullifiers)
        {
            if (nullifier.AsSpan().SequenceEqual(noteCommitment)
                || nullifier.AsSpan().SequenceEqual(spendNullifier))
            {
                throw TransitionProfileDecodeError(
                    TransitionProfilePreviousTopupAnchorNullifiersField,
                    TransitionProfilePreviousTopupAnchorNullifiersCurrentNoteReuseError);
            }
        }
    }

    private static ArgumentException TransitionProfileDecodeError(string field, string? message = null)
    {
        return new ArgumentException(message ?? field, "transitionProfileArchive");
    }

    private static (byte[] Payload, byte Flags) KagemushaNoritoArchivePayload(
        byte[] archive,
        string schema,
        string field,
        string parameterName)
    {
        var expectedSchemaHash = NoritoCodec.SchemaHash(schema);
        return KagemushaNoritoArchivePayload(
            archive,
            expectedSchemaHash,
            field,
            parameterName,
            $"{field} must use {schema}");
    }

    private static (byte[] Payload, byte Flags) KagemushaNoritoArchivePayload(
        byte[] archive,
        ReadOnlySpan<byte> expectedSchemaHash,
        string field,
        string parameterName,
        string schemaMessage)
    {
        var copy = KagemushaArchiveBytes.Copy(archive, parameterName);
        if (!copy.AsSpan(6, 16).SequenceEqual(expectedSchemaHash))
        {
            throw new ArgumentException(schemaMessage, parameterName);
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

    private static int ReadVerifiedFoldRecordBundleHopCount(
        byte[] recordBundleArchive,
        string parameterName)
    {
        var (payload, flags) = KagemushaNoritoArchivePayload(
            recordBundleArchive,
            VerifiedFoldRecordBundleWireName,
            "recordBundle",
            parameterName);

        var offset = 0;
        var bundlePayload = ReadRecordBundleField(
            payload,
            ref offset,
            flags,
            "recordBundle.bundle",
            parameterName);
        _ = ReadRecordBundleField(
            payload,
            ref offset,
            flags,
            "recordBundle.verifierRecords",
            parameterName);
        if (offset != payload.Length)
        {
            throw RecordBundleDecodeError(
                "recordBundle",
                "recordBundle has trailing bytes",
                parameterName);
        }

        var bundleOffset = 0;
        bundleOffset = SkipRecordBundleFields(
            bundlePayload,
            bundleOffset,
            flags,
            2,
            "recordBundle.bundle",
            parameterName);
        var stepsPayload = ReadRecordBundleField(
            bundlePayload,
            ref bundleOffset,
            flags,
            "recordBundle.steps",
            parameterName);
        if (bundleOffset != bundlePayload.Length)
        {
            throw RecordBundleDecodeError(
                "recordBundle.bundle",
                "recordBundle.bundle has trailing bytes",
                parameterName);
        }

        var hopCount = ReadVerifiedFoldStepCount(
            stepsPayload,
            flags,
            "recordBundle.steps",
            parameterName);
        if (hopCount < 1)
        {
            throw RecordBundleDecodeError(
                "recordBundle",
                "recordBundle must contain at least one fold step",
                parameterName);
        }
        return hopCount;
    }

    private static int ReadVerifiedFoldStepCount(
        byte[] payload,
        byte flags,
        string field,
        string parameterName)
    {
        if (payload.Length < 8)
        {
            throw RecordBundleDecodeError(
                field,
                $"{field} count is truncated",
                parameterName);
        }

        var count = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(0, 8));
        if (count > CompactTokenMaxHops)
        {
            throw RecordBundleDecodeError(
                field,
                $"{field} fold step count must not exceed {CompactTokenMaxHops}",
                parameterName);
        }

        var offset = 8;
        for (var index = 0; index < (int)count; index++)
        {
            var itemPayload = ReadRecordBundleField(
                payload,
                ref offset,
                flags,
                $"{field}[{index}]",
                parameterName);
            var itemOffset = 0;
            itemOffset = SkipRecordBundleFields(
                itemPayload,
                itemOffset,
                flags,
                6,
                $"{field}[{index}]",
                parameterName);
            if (itemOffset != itemPayload.Length)
            {
                throw RecordBundleDecodeError(
                    $"{field}[{index}]",
                    $"Trailing bytes after {field}[{index}]",
                    parameterName);
            }
        }
        if (offset != payload.Length)
        {
            throw RecordBundleDecodeError(
                field,
                $"Trailing bytes after {field}",
                parameterName);
        }
        return (int)count;
    }

    private static int SkipRecordBundleFields(
        byte[] payload,
        int offset,
        byte flags,
        int count,
        string field,
        string parameterName)
    {
        var cursor = offset;
        for (var index = 0; index < count; index++)
        {
            _ = ReadRecordBundleField(payload, ref cursor, flags, field, parameterName);
        }
        return cursor;
    }

    private static byte[] ReadRecordBundleField(
        byte[] buffer,
        ref int offset,
        byte flags,
        string field,
        string parameterName)
    {
        var length = ReadRecordBundleLength(buffer, ref offset, flags, field, parameterName);
        if (length > buffer.Length - offset)
        {
            throw RecordBundleDecodeError(field, parameterName: parameterName);
        }
        var result = buffer.AsSpan(offset, length).ToArray();
        offset += length;
        return result;
    }

    private static int ReadRecordBundleLength(
        byte[] buffer,
        ref int offset,
        byte flags,
        string field,
        string parameterName)
    {
        if ((flags & KagemushaNoritoCompactLenFlag) == 0)
        {
            if (offset + 8 > buffer.Length)
            {
                throw RecordBundleDecodeError(field, parameterName: parameterName);
            }
            var fixedLength = BinaryPrimitives.ReadUInt64LittleEndian(buffer.AsSpan(offset, 8));
            if (fixedLength > int.MaxValue)
            {
                throw RecordBundleDecodeError(field, parameterName: parameterName);
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
                throw RecordBundleDecodeError(field, parameterName: parameterName);
            }
            var current = buffer[offset++];
            var currentValue = current & 0x7f;
            if (shift >= 63 && currentValue > 1)
            {
                throw RecordBundleDecodeError(field, parameterName: parameterName);
            }
            value |= (ulong)currentValue << shift;
            if ((current & 0x80) == 0)
            {
                var encodedLength = offset - startOffset;
                if (encodedLength > 1 && value < (1UL << (7 * (encodedLength - 1))))
                {
                    throw RecordBundleDecodeError(field, parameterName: parameterName);
                }
                if (value > int.MaxValue)
                {
                    throw RecordBundleDecodeError(field, parameterName: parameterName);
                }
                return (int)value;
            }
            shift += 7;
        }
        throw RecordBundleDecodeError(field, parameterName: parameterName);
    }

    private static ArgumentException RecordBundleDecodeError(
        string field,
        string? message = null,
        string parameterName = "recordBundleArchive")
    {
        return new ArgumentException(message ?? field, parameterName);
    }

    private static byte[] RequireValidPallasOpenEnvelopesArchive(
        ReadOnlySpan<byte> archive,
        string parameterName,
        int expectedEnvelopeCount)
    {
        var bytes = RequireValidInputArchive(
            archive,
            parameterName,
            "Pallas open-envelopes archive");
        ValidatePallasOpenEnvelopesArchive(
            bytes,
            parameterName,
            "pallasOpenEnvelopesArchive",
            expectedEnvelopeCount);
        return bytes;
    }

    private static void ValidatePallasOpenEnvelopesArchive(
        byte[] archive,
        string parameterName,
        string field,
        int expectedEnvelopeCount)
    {
        var (payload, flags) = KagemushaNoritoArchivePayload(
            archive,
            PallasOpenEnvelopeVectorSchemaHash,
            field,
            parameterName,
            $"{field} must be a valid Vec<iroha_zkp_halo2::OpenVerifyEnvelope> Norito archive");
        if (flags != KagemushaNoritoCompactLenFlag)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} must use compact Norito layout",
                parameterName);
        }
        if (payload.Length < 8)
        {
            throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
        }

        var count = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(0, 8));
        if (count != (ulong)expectedEnvelopeCount)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} requires exactly {expectedEnvelopeCount} envelope(s)",
                parameterName);
        }
        if (count > int.MaxValue)
        {
            throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
        }

        var offset = 8;
        for (var index = 0; index < (int)count; index++)
        {
            var itemPayload = ReadPallasField(
                payload,
                ref offset,
                flags,
                $"{field}[{index}]",
                parameterName);
            ValidatePallasOpenEnvelopePayload(
                itemPayload,
                flags,
                $"{field}[{index}]",
                parameterName);
        }
        if (offset != payload.Length)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} has trailing bytes",
                parameterName);
        }
    }

    private static void ValidatePallasOpenEnvelopePayload(
        byte[] payload,
        byte flags,
        string field,
        string parameterName)
    {
        var offset = 0;
        var paramsN = ReadPallasIpaParams(
            ReadPallasField(payload, ref offset, flags, $"{field}.params", parameterName),
            flags,
            $"{field}.params",
            parameterName);
        var publicN = ReadPallasPolyOpenPublic(
            ReadPallasField(payload, ref offset, flags, $"{field}.public", parameterName),
            flags,
            $"{field}.public",
            parameterName);
        if (publicN != paramsN)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} public opening length mismatch",
                parameterName);
        }
        ReadPallasIpaProof(
            ReadPallasField(payload, ref offset, flags, $"{field}.proof", parameterName),
            flags,
            paramsN,
            $"{field}.proof",
            parameterName);
        var transcriptLabel = DecodePallasString(
            ReadPallasField(payload, ref offset, flags, $"{field}.transcript_label", parameterName),
            flags,
            $"{field}.transcript_label",
            parameterName);
        if (transcriptLabel.Length == 0)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} transcript_label must be non-empty",
                parameterName);
        }
        if (StrictUtf8.GetByteCount(transcriptLabel) > RecursivePallasOpenEnvelopeMaxTranscriptLabelBytes)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} transcript_label exceeds {RecursivePallasOpenEnvelopeMaxTranscriptLabelBytes} bytes",
                parameterName);
        }
        ReadRequiredPallasMetadataOption(
            ReadPallasField(payload, ref offset, flags, $"{field}.vk_commitment", parameterName),
            flags,
            $"{field}.vk_commitment",
            parameterName);
        ReadRequiredPallasMetadataOption(
            ReadPallasField(payload, ref offset, flags, $"{field}.public_inputs_schema_hash", parameterName),
            flags,
            $"{field}.public_inputs_schema_hash",
            parameterName);
        ReadRequiredPallasMetadataOption(
            ReadPallasField(payload, ref offset, flags, $"{field}.domain_tag", parameterName),
            flags,
            $"{field}.domain_tag",
            parameterName);
        if (offset != payload.Length)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"Trailing bytes after {field}",
                parameterName);
        }
    }

    private static int ReadPallasIpaParams(
        byte[] payload,
        byte flags,
        string field,
        string parameterName)
    {
        var offset = 0;
        var version = ReadPallasU16Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.version", parameterName),
            $"{field}.version",
            parameterName);
        if (version != 1)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.version",
                $"{field}.version must be 1",
                parameterName);
        }
        var curveId = ReadPallasU16Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.curve_id", parameterName),
            $"{field}.curve_id",
            parameterName);
        if (curveId != PallasCurveId)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.curve_id",
                $"{field}.curve_id must be Pallas",
                parameterName);
        }
        var rawN = ReadPallasU32Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.n", parameterName),
            $"{field}.n",
            parameterName);
        if (rawN > int.MaxValue)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.n",
                $"{field}.n exceeds max 2^{RecursivePallasOpenEnvelopeMaxK}",
                parameterName);
        }
        var n = (int)rawN;
        if (n < 2 || (n & (n - 1)) != 0)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.n",
                $"{field}.n must be a power of two >= 2",
                parameterName);
        }
        if (n > RecursivePallasOpenEnvelopeMaxN)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.n",
                $"{field}.n exceeds max 2^{RecursivePallasOpenEnvelopeMaxK}",
                parameterName);
        }
        var gCount = ReadPallasFixed32SequenceCount(
            ReadPallasField(payload, ref offset, flags, $"{field}.g", parameterName),
            flags,
            $"{field}.g",
            parameterName,
            n,
            $"{field}.g length must equal params.n");
        if (gCount != n)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.g",
                $"{field}.g length must equal params.n",
                parameterName);
        }
        var hCount = ReadPallasFixed32SequenceCount(
            ReadPallasField(payload, ref offset, flags, $"{field}.h", parameterName),
            flags,
            $"{field}.h",
            parameterName,
            n,
            $"{field}.h length must equal params.n");
        if (hCount != n)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.h",
                $"{field}.h length must equal params.n",
                parameterName);
        }
        ReadPallasFixed32Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.u", parameterName),
            $"{field}.u",
            parameterName);
        if (offset != payload.Length)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"Trailing bytes after {field}",
                parameterName);
        }
        return n;
    }

    private static int ReadPallasPolyOpenPublic(
        byte[] payload,
        byte flags,
        string field,
        string parameterName)
    {
        var offset = 0;
        var version = ReadPallasU16Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.version", parameterName),
            $"{field}.version",
            parameterName);
        if (version != 1)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.version",
                $"{field}.version must be 1",
                parameterName);
        }
        var curveId = ReadPallasU16Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.curve_id", parameterName),
            $"{field}.curve_id",
            parameterName);
        if (curveId != PallasCurveId)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.curve_id",
                $"{field}.curve_id must be Pallas",
                parameterName);
        }
        var rawN = ReadPallasU32Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.n", parameterName),
            $"{field}.n",
            parameterName);
        if (rawN > int.MaxValue)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.n",
                $"{field}.n exceeds max 2^{RecursivePallasOpenEnvelopeMaxK}",
                parameterName);
        }
        var n = (int)rawN;
        ReadPallasFixed32Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.z", parameterName),
            $"{field}.z",
            parameterName);
        ReadPallasFixed32Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.t", parameterName),
            $"{field}.t",
            parameterName);
        ReadPallasFixed32Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.p_g", parameterName),
            $"{field}.p_g",
            parameterName);
        if (offset != payload.Length)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"Trailing bytes after {field}",
                parameterName);
        }
        return n;
    }

    private static void ReadPallasIpaProof(
        byte[] payload,
        byte flags,
        int n,
        string field,
        string parameterName)
    {
        var offset = 0;
        var version = ReadPallasU16Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.version", parameterName),
            $"{field}.version",
            parameterName);
        if (version != 1)
        {
            throw PallasOpenEnvelopeDecodeError(
                $"{field}.version",
                $"{field}.version must be 1",
                parameterName);
        }

        var expectedRounds = BitOperations.TrailingZeroCount((uint)n);
        var lCount = ReadPallasFixed32SequenceCount(
            ReadPallasField(payload, ref offset, flags, $"{field}.l", parameterName),
            flags,
            $"{field}.l",
            parameterName,
            expectedRounds,
            $"{field} round count mismatch: expected {expectedRounds}, found count prefix");
        var rCount = ReadPallasFixed32SequenceCount(
            ReadPallasField(payload, ref offset, flags, $"{field}.r", parameterName),
            flags,
            $"{field}.r",
            parameterName,
            expectedRounds,
            $"{field} round count mismatch: expected {expectedRounds}, found count prefix");
        if (lCount != rCount)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} L/R round count mismatch",
                parameterName);
        }
        if (lCount != expectedRounds)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} round count mismatch: expected {expectedRounds}, found {lCount}",
                parameterName);
        }
        ReadPallasFixed32Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.a_final", parameterName),
            $"{field}.a_final",
            parameterName);
        ReadPallasFixed32Payload(
            ReadPallasField(payload, ref offset, flags, $"{field}.b_final", parameterName),
            $"{field}.b_final",
            parameterName);
        if (offset != payload.Length)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"Trailing bytes after {field}",
                parameterName);
        }
    }

    private static int ReadPallasFixed32SequenceCount(
        byte[] payload,
        byte flags,
        string field,
        string parameterName,
        int expectedCount,
        string mismatchMessage)
    {
        if (payload.Length < 8)
        {
            throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
        }
        var count = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(0, 8));
        if (count != (ulong)expectedCount)
        {
            throw PallasOpenEnvelopeDecodeError(field, mismatchMessage, parameterName);
        }
        if (count > int.MaxValue)
        {
            throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
        }

        var offset = 8;
        for (var index = 0; index < (int)count; index++)
        {
            ReadPallasFixed32Payload(
                ReadPallasField(payload, ref offset, flags, $"{field}[{index}]", parameterName),
                $"{field}[{index}]",
                parameterName);
        }
        if (offset != payload.Length)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"Trailing bytes after {field}",
                parameterName);
        }
        return (int)count;
    }

    private static byte[] ReadRequiredPallasMetadataOption(
        byte[] payload,
        byte flags,
        string field,
        string parameterName)
    {
        if (payload.Length == 0)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} option tag must be 0 or 1",
                parameterName);
        }
        var tag = payload[0];
        if (tag != 0 && tag != 1)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} option tag must be 0 or 1",
                parameterName);
        }
        if (tag == 0)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} is required",
                parameterName);
        }

        var offset = 1;
        var length = ReadPallasLength(payload, ref offset, flags, field, parameterName);
        if (length > payload.Length - offset)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} payload length mismatch",
                parameterName);
        }
        var value = payload.AsSpan(offset, length).ToArray();
        offset += length;
        if (offset != payload.Length)
        {
            throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
        }
        if (value.Length != 32)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} must be exactly 32 bytes",
                parameterName);
        }
        if (IsZeroBytes(value))
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} must be non-zero",
                parameterName);
        }
        return value;
    }

    private static byte[] ReadPallasFixed32Payload(
        byte[] payload,
        string field,
        string parameterName)
    {
        if (payload.Length != 32)
        {
            throw PallasOpenEnvelopeDecodeError(
                field,
                $"{field} must be exactly 32 bytes",
                parameterName);
        }
        return payload.ToArray();
    }

    private static ushort ReadPallasU16Payload(byte[] payload, string field, string parameterName)
    {
        if (payload.Length != 2)
        {
            throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
        }
        return BinaryPrimitives.ReadUInt16LittleEndian(payload);
    }

    private static uint ReadPallasU32Payload(byte[] payload, string field, string parameterName)
    {
        if (payload.Length != 4)
        {
            throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
        }
        return BinaryPrimitives.ReadUInt32LittleEndian(payload);
    }

    private static string DecodePallasString(
        byte[] payload,
        byte flags,
        string field,
        string parameterName)
    {
        try
        {
            var offset = 0;
            var length = ReadPallasLength(payload, ref offset, flags, field, parameterName);
            if (length != payload.Length - offset)
            {
                throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
            }
            return StrictUtf8.GetString(payload, offset, length);
        }
        catch (DecoderFallbackException)
        {
            throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
        }
    }

    private static byte[] ReadPallasField(
        byte[] buffer,
        ref int offset,
        byte flags,
        string field,
        string parameterName)
    {
        var length = ReadPallasLength(buffer, ref offset, flags, field, parameterName);
        if (length > buffer.Length - offset)
        {
            throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
        }
        var result = buffer.AsSpan(offset, length).ToArray();
        offset += length;
        return result;
    }

    private static int ReadPallasLength(
        byte[] buffer,
        ref int offset,
        byte flags,
        string field,
        string parameterName)
    {
        if ((flags & KagemushaNoritoCompactLenFlag) == 0)
        {
            if (offset + 8 > buffer.Length)
            {
                throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
            }
            var fixedLength = BinaryPrimitives.ReadUInt64LittleEndian(buffer.AsSpan(offset, 8));
            if (fixedLength > int.MaxValue)
            {
                throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
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
                throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
            }
            var current = buffer[offset++];
            var currentValue = current & 0x7f;
            if (shift >= 63 && currentValue > 1)
            {
                throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
            }
            value |= (ulong)currentValue << shift;
            if ((current & 0x80) == 0)
            {
                var encodedLength = offset - startOffset;
                if (encodedLength > 1 && value < (1UL << (7 * (encodedLength - 1))))
                {
                    throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
                }
                if (value > int.MaxValue)
                {
                    throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
                }
                return (int)value;
            }
            shift += 7;
        }
        throw PallasOpenEnvelopeDecodeError(field, parameterName: parameterName);
    }

    private static ArgumentException PallasOpenEnvelopeDecodeError(
        string field,
        string? message = null,
        string parameterName = "pallasOpenEnvelopesArchive")
    {
        return new ArgumentException(message ?? field, parameterName);
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
        var topupAnchorNullifiers = ReadBundleTopupAnchorNullifiersPayload(
            ReadBundleField(payload, ref offset, flags, "accumulator.topupAnchorNullifiers"),
            flags);
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
        RequireBundleTopupAnchorNullifiers(topupAnchorNullifiers, currentNote);
        if (offset != payload.Length)
        {
            throw BundleDecodeError("bundle", "accumulator has trailing bytes");
        }

        return new BundleAccumulatorSummary(
            chainId,
            asset,
            initialRoot,
            finalRoot,
            topupAnchorNullifiers,
            hopCount,
            currentNote);
    }

    private static byte[][] ReadBundleTopupAnchorNullifiersPayload(byte[] payload, byte flags)
    {
        const string field = BundleTopupAnchorNullifiersField;
        if (payload.Length < 8)
        {
            throw BundleDecodeError(field, $"{field} count is truncated");
        }
        var count = BinaryPrimitives.ReadUInt64LittleEndian(payload.AsSpan(0, 8));
        if (count == 0 || count > FoldStepMaxInputs)
        {
            throw BundleDecodeError(field, BundleTopupAnchorNullifiersCountError);
        }

        var values = new byte[(int)count][];
        var offset = 8;
        for (var index = 0; index < values.Length; index++)
        {
            values[index] = ReadBundleFixedBytesFlexible(
                ReadBundleField(payload, ref offset, flags, $"{field}[{index}]"),
                flags,
                32,
                $"{field}[{index}]");
        }
        if (offset != payload.Length)
        {
            throw BundleDecodeError(field, $"{field} has trailing bytes");
        }
        return values;
    }

    private static void RequireBundleTopupAnchorNullifiers(
        byte[][] topupAnchorNullifiers,
        KagemushaRecursiveSpendableNoteSummary currentNote)
    {
        const string field = BundleTopupAnchorNullifiersField;
        if (topupAnchorNullifiers.Length == 0 || topupAnchorNullifiers.Length > FoldStepMaxInputs)
        {
            throw BundleDecodeError(field, BundleTopupAnchorNullifiersCountError);
        }

        byte[]? previous = null;
        foreach (var nullifier in topupAnchorNullifiers)
        {
            if (IsZeroBytes(nullifier))
            {
                throw BundleDecodeError(field, BundleTopupAnchorNullifiersZeroError);
            }
            if (previous is not null && CompareFixedBytes(previous, nullifier) >= 0)
            {
                throw BundleDecodeError(field, BundleTopupAnchorNullifiersOrderError);
            }
            previous = nullifier;
        }

        var noteCommitment = currentNote.NoteCommitment;
        var spendNullifier = currentNote.SpendNullifier;
        foreach (var nullifier in topupAnchorNullifiers)
        {
            if (nullifier.AsSpan().SequenceEqual(noteCommitment)
                || nullifier.AsSpan().SequenceEqual(spendNullifier))
            {
                throw BundleDecodeError(field, BundleTopupAnchorNullifiersCurrentNoteReuseError);
            }
        }
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

    private static int CompareFixedBytes(ReadOnlySpan<byte> left, ReadOnlySpan<byte> right)
    {
        var count = Math.Min(left.Length, right.Length);
        for (var index = 0; index < count; index++)
        {
            var comparison = left[index].CompareTo(right[index]);
            if (comparison != 0)
            {
                return comparison;
            }
        }
        return left.Length.CompareTo(right.Length);
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

    public static byte[] EncodeInitRequest(
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        KagemushaRecursiveSpendLineageKeyArtifacts lineageKeyArtifacts,
        ulong? blockHeight = null)
    {
        var artifacts = ValidateLineageKeyArtifacts(lineageKeyArtifacts);
        if (!artifacts.IsInitArtifact)
        {
            throw new ArgumentException("lineage_key_artifacts must be init artifacts", nameof(lineageKeyArtifacts));
        }
        return EncodeInitRequestCore(
            recordBundleArchive,
            pallasOpenEnvelopesArchive,
            currentNote,
            artifacts.LineageVerifierKey(),
            artifacts.LineageProvingKeyArchive(),
            blockHeight);
    }

    public static byte[] EncodeInitRequestWithLineageMaterials(
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        ReadOnlySpan<byte> lineageVerifierKey,
        ReadOnlySpan<byte> lineageProvingKeyArchive,
        ulong? blockHeight = null)
    {
        var artifacts = LineageKeyArtifactsForInit(
            verifierOpeningLen: 2,
            RecursiveAggregationProofBackend,
            lineageVerifierKey,
            lineageProvingKeyArchive);
        return EncodeInitRequestCore(
            recordBundleArchive,
            pallasOpenEnvelopesArchive,
            currentNote,
            artifacts.LineageVerifierKey(),
            artifacts.LineageProvingKeyArchive(),
            blockHeight);
    }

    public static byte[] EncodeInitRequestWithGeneratedPallas(
        ReadOnlySpan<byte> recordBundleArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        KagemushaRecursiveSpendLineageKeyArtifacts lineageKeyArtifacts,
        ulong? blockHeight = null)
    {
        ArgumentNullException.ThrowIfNull(currentNote);
        var artifacts = ValidateLineageKeyArtifacts(lineageKeyArtifacts);
        if (!artifacts.IsInitArtifact)
        {
            throw new ArgumentException("lineage_key_artifacts must be init artifacts", nameof(lineageKeyArtifacts));
        }
        var pallasOpenEnvelopes = BuildPallasOpenEnvelopesArchive(recordBundleArchive).NoritoBytes;
        return EncodeInitRequestCore(
            recordBundleArchive,
            pallasOpenEnvelopes,
            currentNote,
            artifacts.LineageVerifierKey(),
            artifacts.LineageProvingKeyArchive(),
            blockHeight);
    }

    public static byte[] EncodeInitRequestWithGeneratedPallas(
        ReadOnlySpan<byte> recordBundleArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        ReadOnlySpan<byte> lineageVerifierKey,
        ReadOnlySpan<byte> lineageProvingKeyArchive,
        ulong? blockHeight = null)
    {
        ArgumentNullException.ThrowIfNull(currentNote);
        var artifacts = LineageKeyArtifactsForInit(
            verifierOpeningLen: 2,
            RecursiveAggregationProofBackend,
            lineageVerifierKey,
            lineageProvingKeyArchive);
        var pallasOpenEnvelopes = BuildPallasOpenEnvelopesArchive(recordBundleArchive).NoritoBytes;
        return EncodeInitRequestCore(
            recordBundleArchive,
            pallasOpenEnvelopes,
            currentNote,
            artifacts.LineageVerifierKey(),
            artifacts.LineageProvingKeyArchive(),
            blockHeight);
    }

    public static byte[] EncodeAppendRequest(
        ReadOnlySpan<byte> previousBundleArchive,
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        string? outputProofCircuitId = null,
        byte[]? previousLineageVerifierRecordArchive = null,
        byte[]? previousProofOpenEnvelopesArchive = null,
        KagemushaRecursiveSpendLineageKeyArtifacts? lineageKeyArtifacts = null,
        ulong? blockHeight = null)
    {
        byte[]? lineageVerifierKey = null;
        byte[]? lineageProvingKeyArchive = null;
        if (lineageKeyArtifacts is not null)
        {
            var artifacts = ValidateLineageKeyArtifacts(lineageKeyArtifacts);
            if (!artifacts.IsAppendArtifact)
            {
                throw new ArgumentException(
                    "lineage_key_artifacts must be append artifacts",
                    nameof(lineageKeyArtifacts));
            }
            lineageVerifierKey = artifacts.LineageVerifierKey();
            lineageProvingKeyArchive = artifacts.LineageProvingKeyArchive();
        }

        return EncodeAppendRequestCore(
            previousBundleArchive,
            recordBundleArchive,
            pallasOpenEnvelopesArchive,
            currentNote,
            outputProofCircuitId,
            previousLineageVerifierRecordArchive,
            previousProofOpenEnvelopesArchive,
            lineageVerifierKey,
            lineageProvingKeyArchive,
            blockHeight);
    }

    public static byte[] EncodeAppendRequestWithLineageMaterials(
        ReadOnlySpan<byte> previousBundleArchive,
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        string? outputProofCircuitId = null,
        byte[]? previousLineageVerifierRecordArchive = null,
        byte[]? previousProofOpenEnvelopesArchive = null,
        byte[]? lineageVerifierKey = null,
        byte[]? lineageProvingKeyArchive = null,
        ulong? blockHeight = null)
    {
        return EncodeAppendRequestCore(
            previousBundleArchive,
            recordBundleArchive,
            pallasOpenEnvelopesArchive,
            currentNote,
            outputProofCircuitId,
            previousLineageVerifierRecordArchive,
            previousProofOpenEnvelopesArchive,
            lineageVerifierKey,
            lineageProvingKeyArchive,
            blockHeight);
    }

    public static byte[] EncodeAppendRequestWithGeneratedPallas(
        ReadOnlySpan<byte> previousBundleArchive,
        ReadOnlySpan<byte> recordBundleArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        string? outputProofCircuitId = null,
        byte[]? previousLineageVerifierRecordArchive = null,
        KagemushaRecursiveSpendLineageKeyArtifacts? lineageKeyArtifacts = null,
        ulong? blockHeight = null)
    {
        ArgumentNullException.ThrowIfNull(currentNote);
        var (lineageVerifierKey, lineageProvingKeyArchive) = PrepareAppendGeneratedPallasPreflight(
            previousBundleArchive,
            outputProofCircuitId,
            previousLineageVerifierRecordArchive,
            lineageKeyArtifacts);
        return EncodeAppendRequestWithGeneratedPallasCore(
            previousBundleArchive,
            recordBundleArchive,
            currentNote,
            outputProofCircuitId,
            previousLineageVerifierRecordArchive,
            lineageVerifierKey,
            lineageProvingKeyArchive,
            blockHeight);
    }

    public static byte[] EncodeAppendRequestWithGeneratedPallas(
        ReadOnlySpan<byte> previousBundleArchive,
        ReadOnlySpan<byte> recordBundleArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        string? outputProofCircuitId = null,
        byte[]? previousLineageVerifierRecordArchive = null,
        byte[]? lineageVerifierKey = null,
        byte[]? lineageProvingKeyArchive = null,
        ulong? blockHeight = null)
    {
        ArgumentNullException.ThrowIfNull(currentNote);
        var (preparedVerifierKey, preparedProvingKeyArchive) = PrepareAppendGeneratedPallasPreflight(
            previousBundleArchive,
            outputProofCircuitId,
            previousLineageVerifierRecordArchive,
            lineageVerifierKey,
            lineageProvingKeyArchive);
        return EncodeAppendRequestWithGeneratedPallasCore(
            previousBundleArchive,
            recordBundleArchive,
            currentNote,
            outputProofCircuitId,
            previousLineageVerifierRecordArchive,
            preparedVerifierKey,
            preparedProvingKeyArchive,
            blockHeight);
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
        string? publicAmount,
        KagemushaRecursiveSpendBundleSummary bundleSummary,
        ReadOnlySpan<byte> changeOutput)
    {
        ArgumentNullException.ThrowIfNull(bundleSummary);
        ValidateRedeemChangeOutputPreflight(
            publicAmount,
            bundleSummary.CurrentNote.Amount,
            hasChangeOutput: true);
        ValidateRedeemChangeOutputNotReserved(changeOutput, bundleSummary);
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
        int lineageVerifierRecordsCount,
        bool lineageWitnessHasReservedPreviousProofs)
    {
        ValidateRedeemLineagePreflight(
            proofCircuitId,
            hopCount,
            hasLineageWitness,
            hasLineageVerifierRecord,
            lineageVerifierRecordsCount,
            lineageWitnessHasReservedPreviousProofs);
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
        int lineageVerifierRecordsCount,
        bool lineageWitnessHasReservedPreviousProofs,
        string? publicAmount,
        string? currentNoteAmount,
        bool hasChangeOutput)
    {
        ValidateRedeemLineagePreflight(
            proofCircuitId,
            hopCount,
            hasLineageWitness,
            hasLineageVerifierRecord,
            lineageVerifierRecordsCount,
            lineageWitnessHasReservedPreviousProofs);
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

    public static KagemushaRecursiveSpendRedeemInstructionArchive Redeem(
        ReadOnlySpan<byte> requestArchive,
        string? proofCircuitId,
        uint hopCount,
        bool hasLineageWitness,
        bool hasLineageVerifierRecord,
        string? publicAmount,
        KagemushaRecursiveSpendBundleSummary bundleSummary,
        ReadOnlySpan<byte> changeOutput)
    {
        ArgumentNullException.ThrowIfNull(bundleSummary);
        ValidateRedeemLineagePreflight(
            proofCircuitId,
            hopCount,
            hasLineageWitness,
            hasLineageVerifierRecord);
        ValidateRedeemChangeOutputPreflight(
            publicAmount,
            bundleSummary.CurrentNote.Amount,
            hasChangeOutput: true);
        ValidateRedeemChangeOutputNotReserved(changeOutput, bundleSummary);
        return Redeem(requestArchive);
    }

    public static KagemushaRecursiveSpendRedeemInstructionArchive Redeem(
        ReadOnlySpan<byte> requestArchive,
        string? proofCircuitId,
        uint hopCount,
        bool hasLineageWitness,
        bool hasLineageVerifierRecord,
        int lineageVerifierRecordsCount,
        bool lineageWitnessHasReservedPreviousProofs,
        string? publicAmount,
        string? currentNoteAmount,
        ReadOnlySpan<byte> changeOutput)
    {
        ValidateRedeemLineagePreflight(
            proofCircuitId,
            hopCount,
            hasLineageWitness,
            hasLineageVerifierRecord,
            lineageVerifierRecordsCount,
            lineageWitnessHasReservedPreviousProofs);
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
        var recordBundle = RequireValidRecordBundleArchive(
            recordBundleArchive,
            nameof(recordBundleArchive));
        try
        {
            if (!IsCompactPaymentTokenProverAvailable())
            {
                throw new InvalidOperationException(
                    "Kagemusha compact payment-token prover requires native bridge ABI 6 with the compact-token prover symbol.");
            }
            const string symbol = "connect_norito_kagemusha_prove_verified_compact_payment_token_with_records";
            var code = NativeCompactPaymentToken(
                recordBundle,
                (UIntPtr)recordBundle.Length,
                out var outPtr,
                out var outLen);
            return new KagemushaCompactPaymentTokenArchive(ReadBridgeOutput(symbol, code, outPtr, outLen));
        }
        finally
        {
            Clear(recordBundle);
        }
    }

    public static KagemushaPallasOpenEnvelopesArchive BuildPallasOpenEnvelopesArchive(
        ReadOnlySpan<byte> recordBundleArchive)
    {
        var recordBundle = RequireValidRecordBundleArchive(
            recordBundleArchive,
            nameof(recordBundleArchive));
        try
        {
            if (!IsPallasOpenEnvelopeBuilderAvailable())
            {
                throw new InvalidOperationException(
                    "Kagemusha Pallas open-envelope builders require native bridge ABI 7 with current-hop and previous-proof builder symbols.");
            }
            const string symbol = "connect_norito_kagemusha_build_pallas_open_envelopes_archive";
            var code = NativeBuildPallasOpenEnvelopesArchive(
                recordBundle,
                (UIntPtr)recordBundle.Length,
                out var outPtr,
                out var outLen);
            return new KagemushaPallasOpenEnvelopesArchive(ReadBridgeOutput(symbol, code, outPtr, outLen));
        }
        finally
        {
            Clear(recordBundle);
        }
    }

    public static KagemushaPreviousProofOpenEnvelopesArchive BuildPreviousProofOpenEnvelopesArchive(
        ReadOnlySpan<byte> previousBundleArchive)
    {
        var previousBundle = RequireValidInputArchive(
            previousBundleArchive,
            nameof(previousBundleArchive),
            "Previous recursive proof bundle archive");
        try
        {
            if (!IsPallasOpenEnvelopeBuilderAvailable())
            {
                throw new InvalidOperationException(
                    "Kagemusha Pallas open-envelope builders require native bridge ABI 7 with current-hop and previous-proof builder symbols.");
            }
            const string symbol = "connect_norito_kagemusha_build_previous_proof_open_envelopes_archive";
            var code = NativeBuildPreviousProofOpenEnvelopesArchive(
                previousBundle,
                (UIntPtr)previousBundle.Length,
                out var outPtr,
                out var outLen);
            return new KagemushaPreviousProofOpenEnvelopesArchive(ReadBridgeOutput(symbol, code, outPtr, outLen));
        }
        finally
        {
            Clear(previousBundle);
        }
    }

    public static KagemushaRecursiveAggregationProofBundleArchive ProveVerifiedRecursiveAggregationProofBundleWithRecordsAndPallasOpenEnvelopes(
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive)
    {
        var (recordBundle, hopCount) = RequireValidRecordBundleArchiveWithHopCount(
            recordBundleArchive,
            nameof(recordBundleArchive));
        var pallasOpenEnvelopes = RequireValidPallasOpenEnvelopesArchive(
            pallasOpenEnvelopesArchive,
            nameof(pallasOpenEnvelopesArchive),
            hopCount);
        try
        {
            if (!IsRecursiveAggregationProofBundleProverAvailable())
            {
                throw new InvalidOperationException(
                    "Kagemusha recursive aggregation proof-bundle prover requires native bridge ABI 6 with the recursive aggregation prover symbol.");
            }
            const string symbol = "connect_norito_kagemusha_prove_verified_recursive_aggregation_proof_bundle_with_records_and_pallas_open_envelopes";
            var code = NativeRecursiveAggregationProofBundle(
                recordBundle,
                (UIntPtr)recordBundle.Length,
                pallasOpenEnvelopes,
                (UIntPtr)pallasOpenEnvelopes.Length,
                out var outPtr,
                out var outLen);
            return new KagemushaRecursiveAggregationProofBundleArchive(ReadBridgeOutput(symbol, code, outPtr, outLen));
        }
        finally
        {
            Clear(recordBundle);
            Clear(pallasOpenEnvelopes);
        }
    }

    public static KagemushaRecursiveCompactPaymentTokenArchive ProveVerifiedRecursiveCompactPaymentTokenWithRecordsAndPallasOpenEnvelopes(
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive,
        ReadOnlySpan<byte> recursiveCompactKeyArtifactsArchive)
    {
        var (recordBundle, hopCount) = RequireValidRecordBundleArchiveWithHopCount(
            recordBundleArchive,
            nameof(recordBundleArchive));
        var pallasOpenEnvelopes = RequireValidPallasOpenEnvelopesArchive(
            pallasOpenEnvelopesArchive,
            nameof(pallasOpenEnvelopesArchive),
            hopCount);
        var recursiveCompactKeyArtifacts = RequireValidInputArchive(
            recursiveCompactKeyArtifactsArchive,
            nameof(recursiveCompactKeyArtifactsArchive),
            "Recursive compact key artifacts archive");
        try
        {
            if (!IsRecursiveCompactPaymentTokenProverAvailable())
            {
                throw new InvalidOperationException(
                    "Recursive compact Kagemusha payment-token prover requires native bridge ABI 7 with compact prover and verifier symbols.");
            }
            const string symbol = "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes";
            var code = NativeRecursiveCompactPaymentToken(
                recordBundle,
                (UIntPtr)recordBundle.Length,
                pallasOpenEnvelopes,
                (UIntPtr)pallasOpenEnvelopes.Length,
                recursiveCompactKeyArtifacts,
                (UIntPtr)recursiveCompactKeyArtifacts.Length,
                out var outPtr,
                out var outLen);
            return new KagemushaRecursiveCompactPaymentTokenArchive(ReadBridgeOutput(symbol, code, outPtr, outLen));
        }
        finally
        {
            Clear(recordBundle);
            Clear(pallasOpenEnvelopes);
            Clear(recursiveCompactKeyArtifacts);
        }
    }

    public static KagemushaRecursiveCompactPaymentTokenArchive RecursiveSpendCompactPaymentTokenFromBundle(
        ReadOnlySpan<byte> bundleArchive)
    {
        var bundle = RequireValidInputArchive(
            bundleArchive,
            nameof(bundleArchive),
            "Recursive spend bundle archive");
        try
        {
            if (!IsRecursiveCompactPaymentTokenProverAvailable())
            {
                throw new InvalidOperationException(
                    "Recursive spend compact Kagemusha payment-token projection requires native bridge ABI 7 with the compact projection symbol.");
            }
            const string symbol = "connect_norito_kagemusha_recursive_spend_compact_payment_token_from_bundle";
            var code = NativeRecursiveSpendCompactPaymentTokenFromBundle(
                bundle,
                (UIntPtr)bundle.Length,
                out var outPtr,
                out var outLen);
            return new KagemushaRecursiveCompactPaymentTokenArchive(ReadBridgeOutput(symbol, code, outPtr, outLen));
        }
        finally
        {
            Clear(bundle);
        }
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
        byte[]? recursiveCompactVerifierKeys = null;
        try
        {
            RequireValidRecursiveCompactTokenArchive(compactToken);
            recursiveCompactVerifierKeys = RequireValidInputArchive(
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
        finally
        {
            Clear(compactToken);
            Clear(recursiveCompactVerifierKeys);
        }
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
        byte[]? verifierRecord = null;
        try
        {
            RequireValidRecursiveCompactTokenArchive(compactToken);
            verifierRecord = RequireValidInputArchive(
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
        finally
        {
            Clear(compactToken);
            Clear(verifierRecord);
        }
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

    private static byte[] EncodeInitRequestCore(
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        byte[] lineageVerifierKey,
        byte[] lineageProvingKeyArchive,
        ulong? blockHeight)
    {
        ArgumentNullException.ThrowIfNull(currentNote);
        var (recordBundle, hopCount) = RequireValidRecordBundleArchiveWithHopCount(
            recordBundleArchive,
            nameof(recordBundleArchive));
        var pallasOpenEnvelopes = RequireValidPallasOpenEnvelopesArchive(
            pallasOpenEnvelopesArchive,
            nameof(pallasOpenEnvelopesArchive),
            hopCount);
        var recordBundlePayload = CompactArchivePayloadForRequest(
            recordBundle,
            VerifiedFoldRecordBundleWireName,
            "recordBundle",
            nameof(recordBundleArchive));
        var lineageVerifierKeyPayload = EncodeVerifyingKeyBoxPayload(lineageVerifierKey);

        return NoritoCodec.Encode(
            RecursiveSpendInitRequestWireName,
            EncodeFields(
                recordBundlePayload,
                EncodeByteVec(pallasOpenEnvelopes),
                EncodeSpendableNotePayload(currentNote),
                EncodeOptionRaw(lineageVerifierKeyPayload),
                EncodeOptionBytesVec(lineageProvingKeyArchive),
                EncodeOptionU64(blockHeight)),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] EncodeAppendRequestCore(
        ReadOnlySpan<byte> previousBundleArchive,
        ReadOnlySpan<byte> recordBundleArchive,
        ReadOnlySpan<byte> pallasOpenEnvelopesArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        string? outputProofCircuitId,
        byte[]? previousLineageVerifierRecordArchive,
        byte[]? previousProofOpenEnvelopesArchive,
        byte[]? lineageVerifierKey,
        byte[]? lineageProvingKeyArchive,
        ulong? blockHeight)
    {
        ArgumentNullException.ThrowIfNull(currentNote);
        var previousBundle = RequireValidInputArchive(
            previousBundleArchive,
            nameof(previousBundleArchive),
            "Previous bundle archive");
        var previousSummary = DecodeBundleSummary(previousBundle);
        var (recordBundle, hopCount) = RequireValidRecordBundleArchiveWithHopCount(
            recordBundleArchive,
            nameof(recordBundleArchive));
        var pallasOpenEnvelopes = RequireValidPallasOpenEnvelopesArchive(
            pallasOpenEnvelopesArchive,
            nameof(pallasOpenEnvelopesArchive),
            hopCount);
        var normalizedOutput = NormalizeAppendOutputCircuitId(outputProofCircuitId);
        if (!CanSelectAppendOutputCircuitId(
                previousSummary.ProofCircuitId,
                normalizedOutput,
                previousSummary.HopCount))
        {
            throw new ArgumentException(
                "outputProofCircuitId is not valid for the previous bundle",
                nameof(outputProofCircuitId));
        }

        var appendNeedsPreviousLineageRecord =
            RequiresPreviousLineageVerifierRecordForAppend(previousSummary.ProofCircuitId);
        if (appendNeedsPreviousLineageRecord && previousLineageVerifierRecordArchive is null)
        {
            throw new ArgumentException(
                "previousLineageVerifierRecordArchive is required for lineage previous bundles",
                nameof(previousLineageVerifierRecordArchive));
        }
        if (!appendNeedsPreviousLineageRecord && previousLineageVerifierRecordArchive is not null)
        {
            throw new ArgumentException(
                "previousLineageVerifierRecordArchive is only valid for lineage previous bundles",
                nameof(previousLineageVerifierRecordArchive));
        }

        byte[]? previousLineageRecordPayload = null;
        if (previousLineageVerifierRecordArchive is not null)
        {
            previousLineageRecordPayload = CompactArchivePayloadForRequest(
                previousLineageVerifierRecordArchive,
                VerifyingKeyRecordWireName,
                "previousLineageVerifierRecordArchive",
                nameof(previousLineageVerifierRecordArchive));
        }

        var appendNeedsPreviousOpenings =
            RequiresPreviousProofOpenEnvelopesForAppend(normalizedOutput, previousSummary.HopCount);
        if (previousProofOpenEnvelopesArchive is not null && !appendNeedsPreviousOpenings)
        {
            throw new ArgumentException(
                "previousProofOpenEnvelopesArchive is only valid for lineage append output",
                nameof(previousProofOpenEnvelopesArchive));
        }

        byte[]? previousOpenings = null;
        if (previousProofOpenEnvelopesArchive is not null)
        {
            previousOpenings = RequireValidPreviousProofOpenEnvelopesArchive(
                previousProofOpenEnvelopesArchive,
                nameof(previousProofOpenEnvelopesArchive));
        }
        if (appendNeedsPreviousOpenings && previousOpenings is null)
        {
            throw new ArgumentException(
                "previousProofOpenEnvelopesArchive is required for lineage append output",
                nameof(previousProofOpenEnvelopesArchive));
        }

        var appendNeedsLineageKeyArtifacts =
            RequiresLineageKeyArtifactsForAppendOutput(normalizedOutput);
        var suppliedLineageKeyMaterial =
            lineageVerifierKey is not null || lineageProvingKeyArchive is not null;
        if (suppliedLineageKeyMaterial && !appendNeedsLineageKeyArtifacts)
        {
            throw new ArgumentException(
                "lineageKeyArtifacts are only valid for lineage append output",
                nameof(lineageVerifierKey));
        }

        byte[]? appendVerifierKey = null;
        byte[]? appendProvingKeyArchive = null;
        if (appendNeedsLineageKeyArtifacts)
        {
            if (lineageVerifierKey is null || lineageVerifierKey.Length == 0)
            {
                throw new ArgumentException(
                    "lineageVerifierKey is required for lineage append output",
                    nameof(lineageVerifierKey));
            }
            if (lineageProvingKeyArchive is null || lineageProvingKeyArchive.Length == 0)
            {
                throw new ArgumentException(
                    "lineageProvingKeyArchive is required for lineage append output",
                    nameof(lineageProvingKeyArchive));
            }
            var artifacts = LineageKeyArtifactsForAppend(
                verifierOpeningLen: 2,
                RecursiveAggregationProofBackend,
                lineageVerifierKey,
                lineageProvingKeyArchive);
            appendVerifierKey = artifacts.LineageVerifierKey();
            appendProvingKeyArchive = artifacts.LineageProvingKeyArchive();
        }

        var previousBundlePayload = CompactArchivePayloadForRequest(
            previousBundle,
            RecursiveSpendBundleWireName,
            "previousBundle",
            nameof(previousBundleArchive));
        var recordBundlePayload = CompactArchivePayloadForRequest(
            recordBundle,
            VerifiedFoldRecordBundleWireName,
            "recordBundle",
            nameof(recordBundleArchive));
        var outputWire = normalizedOutput == RecursiveAggregationProofCircuitIdV1
            ? string.Empty
            : normalizedOutput;

        return NoritoCodec.Encode(
            RecursiveSpendAppendRequestWireName,
            EncodeFields(
                previousBundlePayload,
                recordBundlePayload,
                EncodeByteVec(pallasOpenEnvelopes),
                EncodeSpendableNotePayload(currentNote),
                EncodeString(outputWire),
                EncodeOptionRaw(previousLineageRecordPayload),
                EncodeByteVec(previousOpenings ?? Array.Empty<byte>()),
                EncodeOptionRaw(appendVerifierKey is null ? null : EncodeVerifyingKeyBoxPayload(appendVerifierKey)),
                EncodeOptionBytesVec(appendProvingKeyArchive),
                EncodeOptionU64(blockHeight)),
            KagemushaNoritoCompactLenFlag);
    }

    private static byte[] EncodeAppendRequestWithGeneratedPallasCore(
        ReadOnlySpan<byte> previousBundleArchive,
        ReadOnlySpan<byte> recordBundleArchive,
        KagemushaRecursiveSpendableNoteDescriptor currentNote,
        string? outputProofCircuitId,
        byte[]? previousLineageVerifierRecordArchive,
        byte[]? lineageVerifierKey,
        byte[]? lineageProvingKeyArchive,
        ulong? blockHeight)
    {
        var previousSummary = DecodeBundleSummary(
            RequireValidInputArchive(
                previousBundleArchive,
                nameof(previousBundleArchive),
                "Previous bundle archive"));
        var normalizedOutput = NormalizeAppendOutputCircuitId(outputProofCircuitId);
        var pallasOpenEnvelopes = BuildPallasOpenEnvelopesArchive(recordBundleArchive).NoritoBytes;
        var previousProofOpenEnvelopes =
            RequiresPreviousProofOpenEnvelopesForAppend(normalizedOutput, previousSummary.HopCount)
                ? BuildPreviousProofOpenEnvelopesArchive(previousBundleArchive).NoritoBytes
                : null;
        return EncodeAppendRequestCore(
            previousBundleArchive,
            recordBundleArchive,
            pallasOpenEnvelopes,
            currentNote,
            outputProofCircuitId,
            previousLineageVerifierRecordArchive,
            previousProofOpenEnvelopes,
            lineageVerifierKey,
            lineageProvingKeyArchive,
            blockHeight);
    }

    private static (byte[]? LineageVerifierKey, byte[]? LineageProvingKeyArchive)
        PrepareAppendGeneratedPallasPreflight(
            ReadOnlySpan<byte> previousBundleArchive,
            string? outputProofCircuitId,
            byte[]? previousLineageVerifierRecordArchive,
            KagemushaRecursiveSpendLineageKeyArtifacts? lineageKeyArtifacts)
    {
        byte[]? lineageVerifierKey = null;
        byte[]? lineageProvingKeyArchive = null;
        if (lineageKeyArtifacts is not null)
        {
            var artifacts = ValidateLineageKeyArtifacts(lineageKeyArtifacts);
            if (!artifacts.IsAppendArtifact)
            {
                throw new ArgumentException(
                    "lineage_key_artifacts must be append artifacts",
                    nameof(lineageKeyArtifacts));
            }
            lineageVerifierKey = artifacts.LineageVerifierKey();
            lineageProvingKeyArchive = artifacts.LineageProvingKeyArchive();
        }
        return PrepareAppendGeneratedPallasPreflight(
            previousBundleArchive,
            outputProofCircuitId,
            previousLineageVerifierRecordArchive,
            lineageVerifierKey,
            lineageProvingKeyArchive);
    }

    private static (byte[]? LineageVerifierKey, byte[]? LineageProvingKeyArchive)
        PrepareAppendGeneratedPallasPreflight(
            ReadOnlySpan<byte> previousBundleArchive,
            string? outputProofCircuitId,
            byte[]? previousLineageVerifierRecordArchive,
            byte[]? lineageVerifierKey,
            byte[]? lineageProvingKeyArchive)
    {
        var previousBundle = RequireValidInputArchive(
            previousBundleArchive,
            nameof(previousBundleArchive),
            "Previous bundle archive");
        var previousSummary = DecodeBundleSummary(previousBundle);
        var normalizedOutput = NormalizeAppendOutputCircuitId(outputProofCircuitId);
        if (!CanSelectAppendOutputCircuitId(
                previousSummary.ProofCircuitId,
                normalizedOutput,
                previousSummary.HopCount))
        {
            throw new ArgumentException(
                "outputProofCircuitId is not valid for the previous bundle",
                nameof(outputProofCircuitId));
        }

        var appendNeedsPreviousLineageRecord =
            RequiresPreviousLineageVerifierRecordForAppend(previousSummary.ProofCircuitId);
        if (appendNeedsPreviousLineageRecord && previousLineageVerifierRecordArchive is null)
        {
            throw new ArgumentException(
                "previousLineageVerifierRecordArchive is required for lineage previous bundles",
                nameof(previousLineageVerifierRecordArchive));
        }
        if (!appendNeedsPreviousLineageRecord && previousLineageVerifierRecordArchive is not null)
        {
            throw new ArgumentException(
                "previousLineageVerifierRecordArchive is only valid for lineage previous bundles",
                nameof(previousLineageVerifierRecordArchive));
        }
        if (previousLineageVerifierRecordArchive is not null)
        {
            _ = CompactArchivePayloadForRequest(
                previousLineageVerifierRecordArchive,
                VerifyingKeyRecordWireName,
                "previousLineageVerifierRecordArchive",
                nameof(previousLineageVerifierRecordArchive));
        }

        var appendNeedsLineageKeyArtifacts =
            RequiresLineageKeyArtifactsForAppendOutput(normalizedOutput);
        var suppliedLineageKeyMaterial =
            lineageVerifierKey is not null || lineageProvingKeyArchive is not null;
        if (suppliedLineageKeyMaterial && !appendNeedsLineageKeyArtifacts)
        {
            throw new ArgumentException(
                "lineageKeyArtifacts are only valid for lineage append output",
                nameof(lineageVerifierKey));
        }

        if (!appendNeedsLineageKeyArtifacts)
        {
            return (null, null);
        }
        if (lineageVerifierKey is null || lineageVerifierKey.Length == 0)
        {
            throw new ArgumentException(
                "lineageVerifierKey is required for lineage append output",
                nameof(lineageVerifierKey));
        }
        if (lineageProvingKeyArchive is null || lineageProvingKeyArchive.Length == 0)
        {
            throw new ArgumentException(
                "lineageProvingKeyArchive is required for lineage append output",
                nameof(lineageProvingKeyArchive));
        }
        var appendArtifacts = LineageKeyArtifactsForAppend(
            verifierOpeningLen: 2,
            RecursiveAggregationProofBackend,
            lineageVerifierKey,
            lineageProvingKeyArchive);
        return (appendArtifacts.LineageVerifierKey(), appendArtifacts.LineageProvingKeyArchive());
    }

    private static byte[] RequireValidPreviousProofOpenEnvelopesArchive(
        ReadOnlySpan<byte> archive,
        string parameterName)
    {
        if (archive.Length == 0)
        {
            throw new ArgumentException(
                "Previous proof open-envelopes archive must not be empty.",
                parameterName);
        }
        if (archive.Length > RecursivePreviousProofOpenEnvelopesMaxBytes)
        {
            throw new ArgumentException(
                $"Previous proof open-envelopes archive must not exceed {RecursivePreviousProofOpenEnvelopesMaxBytes} bytes.",
                parameterName);
        }

        var bytes = RequireValidInputArchive(
            archive,
            parameterName,
            "Previous proof open-envelopes archive");
        ValidatePallasOpenEnvelopesArchive(
            bytes,
            parameterName,
            "previousProofOpenEnvelopesArchive",
            RecursivePreviousProofOpenEnvelopesRequiredCountV1);
        return bytes;
    }

    private static byte[] CompactArchivePayloadForRequest(
        byte[] archive,
        string schema,
        string field,
        string parameterName)
    {
        var (payload, flags) = KagemushaNoritoArchivePayload(
            archive,
            schema,
            field,
            parameterName);
        if (flags != KagemushaNoritoCompactLenFlag)
        {
            throw new ArgumentException($"{field} must use compact Norito layout", parameterName);
        }
        return payload;
    }

    private static byte[] EncodeFields(params byte[][] payloads)
    {
        var output = new List<byte>();
        foreach (var payload in payloads)
        {
            WriteCompactLength(output, (ulong)payload.Length);
            output.AddRange(payload);
        }
        return [.. output];
    }

    private static byte[] EncodeString(string value)
    {
        var bytes = StrictUtf8.GetBytes(value);
        var output = new List<byte>();
        WriteCompactLength(output, (ulong)bytes.Length);
        output.AddRange(bytes);
        return [.. output];
    }

    private static byte[] EncodeByteVec(byte[] value)
    {
        var output = new byte[8 + value.Length];
        BinaryPrimitives.WriteUInt64LittleEndian(output.AsSpan(0, 8), (ulong)value.Length);
        value.CopyTo(output.AsSpan(8));
        return output;
    }

    private static byte[] EncodeOptionRaw(byte[]? payload)
    {
        if (payload is null)
        {
            return new byte[] { 0x00 };
        }
        var output = new List<byte> { 0x01 };
        WriteCompactLength(output, (ulong)payload.Length);
        output.AddRange(payload);
        return [.. output];
    }

    private static byte[] EncodeOptionBytesVec(byte[]? value)
    {
        if (value is null)
        {
            return new byte[] { 0x00 };
        }
        return new byte[] { 0x01 }
            .Concat(EncodeFields(EncodeByteVec(value)))
            .ToArray();
    }

    private static byte[] EncodeOptionU64(ulong? value)
    {
        if (!value.HasValue)
        {
            return new byte[] { 0x00 };
        }
        var payload = new byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(payload, value.Value);
        return new byte[] { 0x01 }
            .Concat(EncodeFields(payload))
            .ToArray();
    }

    private static byte[] EncodeSpendableNotePayload(KagemushaRecursiveSpendableNoteDescriptor note)
    {
        return EncodeFields(
            EncodeConstVec(note.NoteCommitment),
            EncodeConstVec(note.SpendNullifier),
            EncodeNumeric(note.Amount));
    }

    private static byte[] EncodeVerifyingKeyBoxPayload(byte[] lineageVerifierKey)
    {
        if (lineageVerifierKey.Length == 0)
        {
            throw new ArgumentException("lineageVerifierKey must not be empty", nameof(lineageVerifierKey));
        }
        return EncodeFields(
            EncodeString(RecursiveAggregationProofBackend),
            EncodeByteVec(lineageVerifierKey));
    }

    private static byte[] EncodeConstVec(byte[] value)
    {
        var output = new List<byte>();
        foreach (var item in value)
        {
            WriteCompactLength(output, 1);
            output.Add(item);
        }
        return [.. output];
    }

    private static byte[] EncodeNumeric(string amount)
    {
        var value = BigInteger.Parse(amount, CultureInfo.InvariantCulture);
        var mantissaBytes = value.ToByteArray();
        var mantissaPayload = new byte[4 + mantissaBytes.Length];
        BinaryPrimitives.WriteUInt32LittleEndian(mantissaPayload.AsSpan(0, 4), (uint)mantissaBytes.Length);
        mantissaBytes.CopyTo(mantissaPayload.AsSpan(4));
        return EncodeFields(mantissaPayload, new byte[4]);
    }

    private static void WriteCompactLength(List<byte> output, ulong value)
    {
        do
        {
            var current = (byte)(value & 0x7f);
            value >>= 7;
            if (value != 0)
            {
                current |= 0x80;
            }
            output.Add(current);
        }
        while (value != 0);
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
        byte[]? request = null;
        try
        {
            request = RequireValidInputArchive(
                requestArchive,
                nameof(requestArchive),
                "Request archive");

            RequireAbi();

            int code;
            IntPtr outPtr;
            UIntPtr outLen;
            try
            {
                code = nativeCall(request, (UIntPtr)request.Length, out outPtr, out outLen);
            }
            catch (Exception)
            {
                throw new InvalidOperationException($"{symbol} failed.");
            }
            return ReadBridgeOutput(symbol, code, outPtr, outLen);
        }
        finally
        {
            Clear(request);
        }
    }

    private static byte[] Call(
        ReadOnlySpan<byte> requestArchive,
        ReadOnlySpan<byte> bundleArchive,
        string symbol,
        NativeArchivePairCall nativeCall)
    {
        byte[]? request = null;
        byte[]? bundle = null;
        try
        {
            request = RequireValidInputArchive(
                requestArchive,
                nameof(requestArchive),
                "Request archive");
            bundle = RequireValidInputArchive(
                bundleArchive,
                nameof(bundleArchive),
                "Bundle archive");

            RequireAbi();

            int code;
            IntPtr outPtr;
            UIntPtr outLen;
            try
            {
                code = nativeCall(
                    request,
                    (UIntPtr)request.Length,
                    bundle,
                    (UIntPtr)bundle.Length,
                    out outPtr,
                    out outLen);
            }
            catch (Exception)
            {
                throw new InvalidOperationException($"{symbol} failed.");
            }
            return ReadBridgeOutput(symbol, code, outPtr, outLen);
        }
        finally
        {
            Clear(request);
            Clear(bundle);
        }
    }

    private static byte[] Call(
        ReadOnlySpan<byte> previousWitnessArchive,
        ReadOnlySpan<byte> requestArchive,
        ReadOnlySpan<byte> bundleArchive,
        string symbol,
        NativeArchiveTripleCall nativeCall)
    {
        byte[]? witness = null;
        byte[]? request = null;
        byte[]? bundle = null;
        try
        {
            witness = RequireValidInputArchive(
                previousWitnessArchive,
                nameof(previousWitnessArchive),
                "Previous witness archive");
            request = RequireValidInputArchive(
                requestArchive,
                nameof(requestArchive),
                "Request archive");
            bundle = RequireValidInputArchive(
                bundleArchive,
                nameof(bundleArchive),
                "Bundle archive");

            RequireAbi();

            int code;
            IntPtr outPtr;
            UIntPtr outLen;
            try
            {
                code = nativeCall(
                    witness,
                    (UIntPtr)witness.Length,
                    request,
                    (UIntPtr)request.Length,
                    bundle,
                    (UIntPtr)bundle.Length,
                    out outPtr,
                    out outLen);
            }
            catch (Exception)
            {
                throw new InvalidOperationException($"{symbol} failed.");
            }
            return ReadBridgeOutput(symbol, code, outPtr, outLen);
        }
        finally
        {
            Clear(witness);
            Clear(request);
            Clear(bundle);
        }
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

    private static byte[] RequireValidRecordBundleArchive(
        ReadOnlySpan<byte> archive,
        string parameterName)
    {
        return RequireValidRecordBundleArchiveWithHopCount(archive, parameterName).Bytes;
    }

    private static (byte[] Bytes, int HopCount) RequireValidRecordBundleArchiveWithHopCount(
        ReadOnlySpan<byte> archive,
        string parameterName)
    {
        var bytes = RequireValidInputArchive(
            archive,
            parameterName,
            "Record bundle archive");
        var hopCount = ReadVerifiedFoldRecordBundleHopCount(bytes, parameterName);
        return (bytes, hopCount);
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
        try
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
            try
            {
                Marshal.Copy(outPtr, result, 0, length);
                RequireValidNativeOutput(symbol, result);
                return result;
            }
            catch
            {
                Clear(result);
                throw;
            }
        }
        finally
        {
            if (outPtr != IntPtr.Zero)
            {
                ClearNativeBuffer(outPtr, outLen);
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

    private static void Clear(byte[]? buffer)
    {
        if (buffer is not null)
        {
            CryptographicOperations.ZeroMemory(buffer);
        }
    }

    private static void ClearNativeBuffer(IntPtr ptr, UIntPtr outLen)
    {
        if (ptr == IntPtr.Zero)
        {
            return;
        }

        var length = outLen.ToUInt64();
        if (length == 0 || length > NativeArchiveMaxBytes)
        {
            return;
        }

        var remaining = (int)length;
        var offset = 0;
        while (remaining > 0)
        {
            var chunk = Math.Min(remaining, ZeroClearChunk.Length);
            Marshal.Copy(ZeroClearChunk, 0, IntPtr.Add(ptr, offset), chunk);
            remaining -= chunk;
            offset += chunk;
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

    internal static bool ConsumeProbeResult(
        int code,
        IntPtr outPtr,
        UIntPtr outLen,
        Action<IntPtr>? free = null)
    {
        var expected = IsExpectedMalformedArchiveProbeResult(code, outPtr, outLen);
        if (outPtr != IntPtr.Zero)
        {
            ClearNativeBuffer(outPtr, outLen);
            (free ?? NativeFree)(outPtr);
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
