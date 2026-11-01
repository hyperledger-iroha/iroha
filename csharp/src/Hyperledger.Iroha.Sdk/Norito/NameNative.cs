using System.Runtime.InteropServices;
using System.Text;

namespace Hyperledger.Iroha.Norito;

/// <summary>Native pinned NFC and syntax admission for already canonical Name bytes.</summary>
internal static class NameNative
{
    private const string Library = "connect_norito_bridge";
    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);
    private static readonly Lazy<bool> Available = new(DetectAvailability);
    private static bool Uses32BitUnsignedLong => OperatingSystem.IsWindows() || IntPtr.Size == 4;

    private static bool DetectAvailability()
    {
        IntPtr handle = IntPtr.Zero;
        try
        {
            return NativeLibrary.TryLoad(Library, typeof(NameNative).Assembly, null, out handle)
                && NativeLibrary.TryGetExport(handle, "connect_norito_bridge_abi_version", out _)
                && NativeLibrary.TryGetExport(handle, "connect_norito_name_validate_v1", out _)
                && BridgeAbiVersion() == 33;
        }
        catch (Exception error) when (error is DllNotFoundException or EntryPointNotFoundException or BadImageFormatException)
        {
            return false;
        }
        finally
        {
            if (handle != IntPtr.Zero) NativeLibrary.Free(handle);
        }
    }

    internal static void ValidateCanonical(string value, string paramName)
    {
        ArgumentNullException.ThrowIfNull(value, paramName);
        // UTF-16 length bounds work before encoding; Rust owns syntax and NFC.
        if (value.Length is < 1 or > 255) throw Invalid(paramName);
        int byteCount;
        try { byteCount = StrictUtf8.GetByteCount(value); }
        catch (EncoderFallbackException) { throw Invalid(paramName); }
        if (byteCount > 255) throw Invalid(paramName);
        if (!Available.Value) throw Unavailable();
        var bytes = StrictUtf8.GetBytes(value);
        try
        {
            var status = Uses32BitUnsignedLong
                ? Validate32(bytes, checked((uint)bytes.Length))
                : Validate64(bytes, checked((ulong)bytes.Length));
            if (status == 1 || status == -2) throw Invalid(paramName);
            if (status != 0) throw Unavailable();
        }
        catch (Exception error) when (error is DllNotFoundException or EntryPointNotFoundException or BadImageFormatException)
        {
            throw Unavailable(error);
        }
    }

    private static ArgumentException Invalid(string paramName) => new(
        "Name must use exact native-canonical NFC text within 255 UTF-8 bytes.", paramName);

    private static InvalidOperationException Unavailable(Exception? inner = null) => new(
        "Names require the ABI-33 Rust Name validator from connect_norito_bridge.", inner);

    [DllImport(Library, EntryPoint = "connect_norito_bridge_abi_version", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint BridgeAbiVersion();
    [DllImport(Library, EntryPoint = "connect_norito_name_validate_v1", CallingConvention = CallingConvention.Cdecl)]
    private static extern int Validate32(byte[] input, uint length);
    [DllImport(Library, EntryPoint = "connect_norito_name_validate_v1", CallingConvention = CallingConvention.Cdecl)]
    private static extern int Validate64(byte[] input, ulong length);
}
