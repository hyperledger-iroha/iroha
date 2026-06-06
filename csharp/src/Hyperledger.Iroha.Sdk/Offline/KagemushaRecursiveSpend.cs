using System;
using System.Runtime.InteropServices;
using Hyperledger.Iroha.Privacy;

namespace Hyperledger.Iroha.Offline;

public sealed record KagemushaRecursiveSpendArchive(byte[] NoritoBytes);

public sealed record KagemushaRecursiveSpendTransitionProfileArchive(byte[] NoritoBytes);

public sealed record KagemushaRecursiveSpendLineageAppendBoundaryArchive(byte[] NoritoBytes);

public sealed record KagemushaRecursiveSpendLineageWitnessArchive(byte[] NoritoBytes);

public sealed record KagemushaRecursiveSpendVerifyArchive(byte[] NoritoBytes);

public sealed record KagemushaRecursiveSpendRedeemInstructionArchive(byte[] NoritoBytes);

public sealed record KagemushaCompactPaymentTokenArchive(byte[] NoritoBytes);

public sealed record KagemushaRecursiveAggregationProofBundleArchive(byte[] NoritoBytes);

public sealed record KagemushaRecursiveCompactPaymentTokenArchive(byte[] NoritoBytes);

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
    public const string RecursiveAggregationProofCircuitIdV1 = "kagemusha-recursive-aggregation-v1";
    public const string RecursiveSpendLineageProofCircuitIdV1 = "kagemusha-recursive-spend-lineage-v1";
    public const string RecursiveSpendLineageOneHopProofCircuitIdV1 = "kagemusha-recursive-spend-lineage-onehop-v1";
    public const string RecursiveSpendLineageAppendProofCircuitIdV1 = "kagemusha-recursive-spend-lineage-append-v1";
    public const string RecursiveCompactCircuitIdV1 = "kagemusha-recursive-compact-v1";

    public const uint RequiredBridgeAbiVersion = 6;
    public const uint RecursiveCompactRequiredBridgeAbiVersion = 7;
    public const uint CompactTokenMaxHops = 64;
    public const uint RecursiveSpendLineageWitnesslessMaxHopsV1 = 64;
    public const bool RecursiveSpendLineageTransitionCircuitWiredV1 = true;
    public const int RecursivePreviousProofOpenEnvelopesRequiredCountV1 = 1;
    public const int RecursivePreviousProofOpenEnvelopesMaxBytes = 8 * 1024 * 1024;
    public const int RecursivePallasOpenEnvelopeMaxTranscriptLabelBytes = 128;
    public const int NativeArchiveMaxBytes = 64 * 1024 * 1024;
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
                && version.Value >= RequiredBridgeAbiVersion
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
                && version.Value >= RecursiveCompactRequiredBridgeAbiVersion
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
                && version.Value >= RequiredBridgeAbiVersion
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
                && version.Value >= RequiredBridgeAbiVersion
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
                && version.Value >= RecursiveCompactRequiredBridgeAbiVersion
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

    public static bool RequiresLineageKeyArtifactsForInit()
    {
        return true;
    }

    public static bool RequiresLineageWitnessForRedeem(string? circuitId, uint hopCount)
    {
        return !CanRedeemWitnessless(circuitId, hopCount);
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
            out var outPtr,
            out var outLen);
        return new KagemushaRecursiveCompactPaymentTokenArchive(ReadBridgeOutput(
            "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes",
            code,
            outPtr,
            outLen));
    }

    public static bool VerifyRecursiveCompactPaymentToken(ReadOnlySpan<byte> compactTokenArchive)
    {
        if (compactTokenArchive.IsEmpty)
        {
            throw new ArgumentException(
                "Compact token archive must not be empty.",
                nameof(compactTokenArchive));
        }
        var compactToken = compactTokenArchive.ToArray();
        RequireValidRecursiveCompactTokenArchive(compactToken);
        if (!IsRecursiveCompactPaymentTokenVerifierAvailable())
        {
            throw new InvalidOperationException(
                "Recursive compact Kagemusha payment-token verifier requires native bridge ABI 7 with the compact verifier symbol.");
        }
        var code = NativeVerifyRecursiveCompactPaymentToken(
            compactToken,
            (UIntPtr)compactToken.Length,
            out var valid);
        if (code != 0)
        {
            throw new InvalidOperationException(
                "connect_norito_kagemusha_verify_recursive_compact_payment_token failed with bridge error code "
                    + code
                    + ".");
        }
        return valid != 0;
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
        var bytes = archive.ToArray();
        if (bytes.Length > NativeArchiveMaxBytes)
        {
            throw new ArgumentException(
                $"{displayName} must not exceed {NativeArchiveMaxBytes} bytes.",
                parameterName);
        }
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

        if (version < RequiredBridgeAbiVersion)
        {
            throw new InvalidOperationException(
                $"{LibraryName} ABI v{RequiredBridgeAbiVersion} is required for recursive Kagemusha, found v{version}.");
        }

        if (!TryProbeRequiredSymbols())
        {
            throw new InvalidOperationException(
                $"{LibraryName} ABI v{RequiredBridgeAbiVersion} recursive Kagemusha surface is incomplete.");
        }
    }

    internal static byte[] ReadBridgeOutput(string symbol, int code, IntPtr outPtr, UIntPtr outLen)
    {
        if (code != 0)
        {
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
                NativeFree(outPtr);
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
            var ok = Probe((NativeArchivePairCall)NativeRecursiveCompactPaymentToken);
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

    private static bool TryProbeRecursiveCompactPaymentTokenVerifierSymbol()
    {
        try
        {
            var code = NativeVerifyRecursiveCompactPaymentToken(
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

    private static bool TryProbeRecursiveCompactPaymentTokenSurface()
    {
        return TryProbeRecursiveCompactPaymentTokenSymbol()
            && TryProbeRecursiveCompactPaymentTokenVerifierSymbol();
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

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_prove_verified_recursive_compact_payment_token_with_records_and_pallas_open_envelopes", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeRecursiveCompactPaymentToken(
        byte[] recordBundlePtr,
        UIntPtr recordBundleLen,
        byte[] pallasOpenEnvelopesPtr,
        UIntPtr pallasOpenEnvelopesLen,
        out IntPtr outPtr,
        out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_verify_recursive_compact_payment_token", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeVerifyRecursiveCompactPaymentToken(
        byte[] compactTokenPtr,
        UIntPtr compactTokenLen,
        out byte valid);

    [DllImport(LibraryName, EntryPoint = "connect_norito_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr ptr);
}
