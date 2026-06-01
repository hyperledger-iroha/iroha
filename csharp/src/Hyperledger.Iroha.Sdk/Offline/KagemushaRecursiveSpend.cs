using System;
using System.Runtime.InteropServices;

namespace Hyperledger.Iroha.Offline;

public sealed record KagemushaRecursiveSpendArchive(byte[] NoritoBytes);

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
    private const uint RequiredAbiVersion = 6;
    private const string LibraryName = "connect_norito_bridge";

    public static bool IsAvailable()
    {
        return TryGetAbiVersion(out var version) && version >= RequiredAbiVersion;
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

    private static byte[] Call(
        ReadOnlySpan<byte> requestArchive,
        string symbol,
        NativeArchiveCall nativeCall)
    {
        if (requestArchive.IsEmpty)
        {
            throw new ArgumentException("Request archive must not be empty.", nameof(requestArchive));
        }

        if (!TryGetAbiVersion(out var version))
        {
            throw new InvalidOperationException(
                $"{LibraryName} is unavailable; install the native bridge before using recursive Kagemusha.");
        }

        if (version < RequiredAbiVersion)
        {
            throw new InvalidOperationException(
                $"{LibraryName} ABI v{RequiredAbiVersion} is required for recursive Kagemusha, found v{version}.");
        }

        var request = requestArchive.ToArray();
        var code = nativeCall(request, (UIntPtr)request.Length, out var outPtr, out var outLen);
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

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_verify", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeVerify(byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_kagemusha_recursive_spend_redeem", CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeRedeem(byte[] requestPtr, UIntPtr requestLen, out IntPtr outPtr, out UIntPtr outLen);

    [DllImport(LibraryName, EntryPoint = "connect_norito_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void NativeFree(IntPtr ptr);
}
