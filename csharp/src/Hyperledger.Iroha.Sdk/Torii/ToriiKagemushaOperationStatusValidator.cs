using System.Runtime.InteropServices;

namespace Hyperledger.Iroha.Torii;

/// <summary>
/// Native structural validator for the exact JSON operation-status bytes
/// returned by Torii.
/// </summary>
/// <remarks>
/// A successful result proves that the response is an exact
/// <c>OfflineOperationStatus</c> and that its portable structural and mutual
/// bindings hold. It does not authenticate the embedded Commit-QC signature;
/// the prover must verify that signature against a separately trusted,
/// release-pinned validator roster.
/// </remarks>
internal static class KagemushaOperationStatusNative
{
    internal const uint RequiredBridgeAbiVersion =
        (uint)ToriiKagemushaTransport.BridgeAbiVersion;
    internal const uint RequiredNativeContractRevision = 1;

    private const string LibraryName = "connect_norito_bridge";
    private const string NativeContractRevisionSymbol =
        "connect_norito_kagemusha_native_contract_revision";
    private const string JsonValidatorSymbol =
        "connect_norito_kagemusha_offline_operation_status_json_validate_v2";

    internal static void ValidateJsonV2(byte[] statusJson)
    {
        ArgumentNullException.ThrowIfNull(statusJson);
        if (statusJson.Length == 0
            || statusJson.Length > ToriiKagemushaTransport.MaxOperationStatusJsonResponseBytes)
        {
            throw new ArgumentOutOfRangeException(
                nameof(statusJson),
                $"statusJson must contain 1..{ToriiKagemushaTransport.MaxOperationStatusJsonResponseBytes} bytes.");
        }

        EnsureAvailable();
        var status = NativeValidateJsonV2(
            statusJson,
            new UIntPtr(checked((uint)statusJson.Length)));
        if (status != 0)
        {
            throw new InvalidDataException(
                $"native Kagemusha operation-status JSON validator failed closed (status {status}).");
        }
    }

    private static void EnsureAvailable()
    {
        IntPtr handle = IntPtr.Zero;
        try
        {
            if (!NativeLibrary.TryLoad(
                    LibraryName,
                    typeof(KagemushaOperationStatusNative).Assembly,
                    null,
                    out handle)
                || !NativeLibrary.TryGetExport(
                    handle,
                    "connect_norito_bridge_abi_version",
                    out _)
                || !NativeLibrary.TryGetExport(
                    handle,
                    NativeContractRevisionSymbol,
                    out _)
                || !NativeLibrary.TryGetExport(handle, JsonValidatorSymbol, out _)
                || !HasRequiredNativeVersions(
                    NativeBridgeAbiVersion(),
                    NativeKagemushaContractRevision()))
            {
                throw new InvalidOperationException(
                    "ABI-23 connect_norito_bridge with Kagemusha native contract revision 1 and the V2 operation-status JSON validator is required.");
            }
        }
        catch (Exception error) when (
            error is DllNotFoundException
            or EntryPointNotFoundException
            or BadImageFormatException)
        {
            throw new InvalidOperationException(
                "ABI-23 connect_norito_bridge with Kagemusha native contract revision 1 and the V2 operation-status JSON validator is required.",
                error);
        }
        finally
        {
            if (handle != IntPtr.Zero)
            {
                NativeLibrary.Free(handle);
            }
        }
    }

    internal static bool HasRequiredNativeVersions(
        uint bridgeAbiVersion,
        uint nativeContractRevision) =>
        bridgeAbiVersion == RequiredBridgeAbiVersion
        && nativeContractRevision == RequiredNativeContractRevision;

    private static int NativeValidateJsonV2(byte[] statusJson, UIntPtr statusJsonLength)
    {
        if (!OperatingSystem.IsWindows())
        {
            return NativeValidateJsonV2Unix(statusJson, statusJsonLength);
        }
        return NativeValidateJsonV2Windows(
            statusJson,
            checked((uint)statusJsonLength.ToUInt64()));
    }

    [DllImport(
        LibraryName,
        EntryPoint = "connect_norito_bridge_abi_version",
        CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeBridgeAbiVersion();

    [DllImport(
        LibraryName,
        EntryPoint = NativeContractRevisionSymbol,
        CallingConvention = CallingConvention.Cdecl)]
    private static extern uint NativeKagemushaContractRevision();

    [DllImport(
        LibraryName,
        EntryPoint = JsonValidatorSymbol,
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateJsonV2Unix(
        [In] byte[] statusJson,
        UIntPtr statusJsonLength);

    [DllImport(
        LibraryName,
        EntryPoint = JsonValidatorSymbol,
        CallingConvention = CallingConvention.Cdecl)]
    private static extern int NativeValidateJsonV2Windows(
        [In] byte[] statusJson,
        uint statusJsonLength);
}

internal interface IKagemushaOperationStatusValidator
{
    void Validate(byte[] statusJson);
}

internal sealed class NativeKagemushaOperationStatusValidator
    : IKagemushaOperationStatusValidator
{
    internal static readonly NativeKagemushaOperationStatusValidator Instance = new();

    private NativeKagemushaOperationStatusValidator()
    {
    }

    public void Validate(byte[] statusJson) =>
        KagemushaOperationStatusNative.ValidateJsonV2(statusJson);
}
