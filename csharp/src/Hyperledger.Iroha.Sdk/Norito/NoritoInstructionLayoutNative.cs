using System.Runtime.InteropServices;
using System.Text;

namespace Hyperledger.Iroha.Norito;

/// <summary>
/// The native bridge's typed instruction transcoder (<c>connect_norito_transcode_instruction_v1</c>).
/// </summary>
/// <remarks>
/// A compact instruction envelope carries the instruction's Norito frame; a dense envelope
/// (<c>norito.md</c>, "Dense layout") carries its bare dense payload with no nested header. The
/// bridge decodes the payload as the instruction registered under the wire ID, requires exactly
/// that instruction's canonical payload in the source layout and re-encodes it in the target
/// layout. Both layouts are the caller's explicit selection; nothing is inferred from bytes.
/// </remarks>
internal static class NoritoInstructionLayoutNative
{
    private const string Library = "connect_norito_bridge";
    private const string Export = "connect_norito_transcode_instruction_v1";
    private const byte LayoutCurrent = 0;
    private const byte LayoutDense = 1;
    private const int ErrInstructionUnregistered = -516;
    private const int ErrInstructionPayload = -517;
    private static readonly Encoding StrictUtf8 = new UTF8Encoding(false, true);
    private static readonly Lazy<bool> Available = new(DetectAvailability);
    private static bool Uses32BitUnsignedLong => OperatingSystem.IsWindows() || IntPtr.Size == 4;

    private static bool DetectAvailability()
    {
        IntPtr handle = IntPtr.Zero;
        try
        {
            return NativeLibrary.TryLoad(Library, typeof(NoritoInstructionLayoutNative).Assembly, null, out handle)
                && NativeLibrary.TryGetExport(handle, "connect_norito_bridge_abi_version", out _)
                && NativeLibrary.TryGetExport(handle, "connect_norito_free", out _)
                && NativeLibrary.TryGetExport(handle, Export, out _)
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

    /// <summary>Whether the ABI-33 native transcoder is loadable.</summary>
    internal static bool IsAvailable => Available.Value;

    /// <summary>
    /// Transcode one instruction envelope payload, or return <see langword="null"/> when the
    /// bridge registers no instruction under <paramref name="wireId"/>, so the caller keeps that
    /// envelope opaque in its own layout.
    /// </summary>
    /// <exception cref="ArgumentException">
    /// The payload is not the canonical payload of the registered instruction in
    /// <paramref name="from"/>.
    /// </exception>
    /// <exception cref="InvalidOperationException">The native transcoder is unavailable.</exception>
    internal static byte[]? Transcode(
        string wireId,
        ReadOnlySpan<byte> payload,
        NoritoWireLayout from,
        NoritoWireLayout to,
        string paramName)
    {
        ArgumentNullException.ThrowIfNull(wireId);
        var fromSelector = Selector(from, nameof(from));
        var toSelector = Selector(to, nameof(to));
        byte[] wireIdBytes;
        try { wireIdBytes = StrictUtf8.GetBytes(wireId); }
        catch (EncoderFallbackException) { throw new ArgumentException("Instruction wire id must be UTF-8 text.", paramName); }
        if (!Available.Value) throw Unavailable();
        var input = payload.ToArray();
        IntPtr output = IntPtr.Zero;
        try
        {
            int status;
            ulong outputLength;
            // C unsigned long is 32 bits on Windows (including x64) and on 32-bit Unix; it is
            // 64 bits on 64-bit Darwin/Linux.
            if (Uses32BitUnsignedLong)
            {
                status = Transcode32(wireIdBytes, checked((uint)wireIdBytes.Length), input, checked((uint)input.Length),
                    fromSelector, toSelector, out output, out var length);
                outputLength = length;
            }
            else
            {
                status = Transcode64(wireIdBytes, checked((ulong)wireIdBytes.Length), input, checked((ulong)input.Length),
                    fromSelector, toSelector, out output, out outputLength);
            }
            switch (status)
            {
                case 0:
                    break;
                case ErrInstructionUnregistered:
                    return null;
                case ErrInstructionPayload:
                    throw new ArgumentException(
                        $"Instruction `{wireId}` payload is not its canonical payload in the {from} layout.", paramName);
                default:
                    throw Unavailable();
            }
            if (outputLength == 0) return Array.Empty<byte>();
            if (output == IntPtr.Zero) throw Unavailable();
            var bytes = new byte[checked((int)outputLength)];
            Marshal.Copy(output, bytes, 0, bytes.Length);
            return bytes;
        }
        catch (Exception error) when (error is DllNotFoundException or EntryPointNotFoundException
            or BadImageFormatException or OverflowException)
        {
            throw Unavailable(error);
        }
        finally
        {
            if (output != IntPtr.Zero) Free(output);
        }
    }

    private static byte Selector(NoritoWireLayout layout, string paramName) => layout switch
    {
        NoritoWireLayout.Compact => LayoutCurrent,
        NoritoWireLayout.Dense => LayoutDense,
        _ => throw new ArgumentOutOfRangeException(
            paramName, layout, "Instruction envelopes are framed in the compact or dense layout."),
    };

    private static InvalidOperationException Unavailable(Exception? inner = null) => new(
        "Dense instruction envelopes require the ABI-33 instruction transcoder from connect_norito_bridge.", inner);

    [DllImport(Library, EntryPoint = "connect_norito_bridge_abi_version", CallingConvention = CallingConvention.Cdecl)]
    private static extern uint BridgeAbiVersion();
    [DllImport(Library, EntryPoint = "connect_norito_free", CallingConvention = CallingConvention.Cdecl)]
    private static extern void Free(IntPtr pointer);
    [DllImport(Library, EntryPoint = Export, CallingConvention = CallingConvention.Cdecl)]
    private static extern int Transcode32(byte[] wireId, uint wireIdLength, byte[] payload, uint payloadLength,
        byte fromLayout, byte toLayout, out IntPtr output, out uint outputLength);
    [DllImport(Library, EntryPoint = Export, CallingConvention = CallingConvention.Cdecl)]
    private static extern int Transcode64(byte[] wireId, ulong wireIdLength, byte[] payload, ulong payloadLength,
        byte fromLayout, byte toLayout, out IntPtr output, out ulong outputLength);
}
