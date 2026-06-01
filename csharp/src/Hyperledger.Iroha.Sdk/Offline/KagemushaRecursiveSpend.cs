using System;
using System.Runtime.InteropServices;

namespace Hyperledger.Iroha.Offline;

public sealed record KagemushaRecursiveSpendArchive(byte[] NoritoBytes);

public sealed record KagemushaRecursiveSpendLineageWitnessArchive(byte[] NoritoBytes);

public sealed record KagemushaRecursiveSpendVerifyArchive(byte[] NoritoBytes);

public sealed record KagemushaRecursiveSpendRedeemInstructionArchive(byte[] NoritoBytes);

public enum KagemushaOfflineSpendMode
{
    RecursiveSpendV1,
    CheckedPrefoldV1,
}

public static class KagemushaOfflineSpendModeExtensions
{
    public const string RecursiveSpendV1WireName = "recursive_spend_v1";
    public const string CheckedPrefoldV1WireName = "checked_prefold_v1";

    public static string WireName(this KagemushaOfflineSpendMode mode)
    {
        return mode switch
        {
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

    public const uint RequiredBridgeAbiVersion = 6;
    private const string LibraryName = "connect_norito_bridge";
    private const int ExpectedEmptyArchiveProbeErrorCode = -311;

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
        catch (InvalidOperationException)
        {
            return false;
        }
    }

    public static KagemushaOfflineSpendMode PreferredMode()
    {
        return PreferredMode(IsAvailable());
    }

    public static KagemushaOfflineSpendMode PreferredMode(bool recursiveSpendAvailable)
    {
        return recursiveSpendAvailable
            ? KagemushaOfflineSpendMode.RecursiveSpendV1
            : KagemushaOfflineSpendMode.CheckedPrefoldV1;
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
        if (requestArchive.IsEmpty)
        {
            throw new ArgumentException("Request archive must not be empty.", nameof(requestArchive));
        }

        RequireAbi();

        var request = requestArchive.ToArray();
        var code = nativeCall(request, (UIntPtr)request.Length, out var outPtr, out var outLen);
        return ReadBridgeOutput(symbol, code, outPtr, outLen);
    }

    private static byte[] Call(
        ReadOnlySpan<byte> requestArchive,
        ReadOnlySpan<byte> bundleArchive,
        string symbol,
        NativeArchivePairCall nativeCall)
    {
        if (requestArchive.IsEmpty)
        {
            throw new ArgumentException("Request archive must not be empty.", nameof(requestArchive));
        }

        if (bundleArchive.IsEmpty)
        {
            throw new ArgumentException("Bundle archive must not be empty.", nameof(bundleArchive));
        }

        RequireAbi();

        var request = requestArchive.ToArray();
        var bundle = bundleArchive.ToArray();
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
        if (previousWitnessArchive.IsEmpty)
        {
            throw new ArgumentException(
                "Previous witness archive must not be empty.",
                nameof(previousWitnessArchive));
        }

        if (requestArchive.IsEmpty)
        {
            throw new ArgumentException("Request archive must not be empty.", nameof(requestArchive));
        }

        if (bundleArchive.IsEmpty)
        {
            throw new ArgumentException("Bundle archive must not be empty.", nameof(bundleArchive));
        }

        RequireAbi();

        var witness = previousWitnessArchive.ToArray();
        var request = requestArchive.ToArray();
        var bundle = bundleArchive.ToArray();
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

    private static byte[] ReadBridgeOutput(string symbol, int code, IntPtr outPtr, UIntPtr outLen)
    {
        if (code != 0)
        {
            throw new InvalidOperationException($"{symbol} failed with bridge error code {code}.");
        }

        if (outPtr == IntPtr.Zero)
        {
            throw new InvalidOperationException($"{symbol} returned a null output pointer.");
        }

        try
        {
            var length = checked((int)outLen.ToUInt64());
            if (length == 0)
            {
                throw new InvalidOperationException($"{symbol} returned empty output.");
            }
            var result = new byte[length];
            Marshal.Copy(outPtr, result, 0, length);
            return result;
        }
        finally
        {
            NativeFree(outPtr);
        }
    }

    private static bool TryProbeRequiredSymbols()
    {
        try
        {
            if (!Probe(NativeInit)
                || !Probe(NativeAppend)
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
    }

    private static bool Probe(NativeArchiveCall nativeCall)
    {
        var code = nativeCall(Array.Empty<byte>(), UIntPtr.Zero, out var outPtr, out var outLen);
        return IsExpectedEmptyArchiveProbeResult(code, outPtr, outLen);
    }

    private static bool Probe(NativeArchivePairCall nativeCall)
    {
        var code = nativeCall(
            Array.Empty<byte>(),
            UIntPtr.Zero,
            Array.Empty<byte>(),
            UIntPtr.Zero,
            out var outPtr,
            out var outLen);
        return IsExpectedEmptyArchiveProbeResult(code, outPtr, outLen);
    }

    private static bool Probe(NativeArchiveTripleCall nativeCall)
    {
        var code = nativeCall(
            Array.Empty<byte>(),
            UIntPtr.Zero,
            Array.Empty<byte>(),
            UIntPtr.Zero,
            Array.Empty<byte>(),
            UIntPtr.Zero,
            out var outPtr,
            out var outLen);
        return IsExpectedEmptyArchiveProbeResult(code, outPtr, outLen);
    }

    internal static bool IsExpectedEmptyArchiveProbeResult(int code, IntPtr outPtr, UIntPtr outLen)
    {
        if (outPtr != IntPtr.Zero)
        {
            NativeFree(outPtr);
            return false;
        }

        return code == ExpectedEmptyArchiveProbeErrorCode && outLen == UIntPtr.Zero;
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

    [DllImport(LibraryName, EntryPoint = "connect_norito_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr ptr);
}
