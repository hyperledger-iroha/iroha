using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;

namespace Hyperledger.Iroha.Norito;

public static class NoritoCodec
{
    internal const byte CanonicalLayoutFlags = NoritoWireLayoutRules.CompactLengthFlag;

    private const int MaxHeaderPaddingBytes = 64;
    private const byte SupportedLayoutFlags = CanonicalLayoutFlags; // 0x00 or COMPACT_LEN only

    private static readonly byte[] TypeNameSchemaHashDomain = Encoding.UTF8.GetBytes("norito:v1:type-name\0");
    private static ReadOnlySpan<byte> Magic => "NRT0"u8;

    public static byte[] Encode(string typeName, ReadOnlySpan<byte> payload, byte flags = 0)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(typeName);

        var schemaHash = SchemaHash(typeName);
        return EncodeWithSchemaHash(schemaHash, payload, flags);
    }

    public static byte[] EncodeWithSchemaHash(ReadOnlySpan<byte> schemaHash, ReadOnlySpan<byte> payload, byte flags = 0)
    {
        if (schemaHash.Length != 16)
        {
            throw new ArgumentException("Norito schema hash must be 16 bytes.", nameof(schemaHash));
        }

        RequireSupportedFlags(flags, nameof(flags));

        return EncodeFrame(schemaHash, payload, NoritoWireLayoutRules.CurrentMinor, flags, paddingLength: 0);
    }

    /// <summary>
    /// Frames <paramref name="payload"/> in an explicit layout. The header declares the layout:
    /// minor <c>0</c> with flags <c>0x02</c> or <c>0x00</c>, or minor <c>1</c> with flags
    /// <c>0x00</c> for <see cref="NoritoWireLayout.Dense"/>.
    /// </summary>
    /// <remarks>
    /// The payload must already be encoded in <paramref name="layout"/>; framing never
    /// transcodes it. <paramref name="payloadAlignment"/> is the archived alignment of the
    /// framed type (a power of two up to 64): a minor-0 frame carries exactly the zero padding
    /// that aligns the payload after the 40-byte header, and a dense frame never carries any.
    /// </remarks>
    public static byte[] EncodeWithLayout(
        string typeName,
        ReadOnlySpan<byte> payload,
        NoritoWireLayout layout,
        int payloadAlignment = 1)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(typeName);

        return EncodeFrame(
            SchemaHash(typeName),
            payload,
            layout.HeaderMinor(),
            layout.HeaderFlags(),
            PayloadPadding(layout, payloadAlignment));
    }

    /// <summary>
    /// Decodes a current-layout (header minor <c>0</c>) frame and returns its flag byte. A dense
    /// frame (minor <c>1</c>) is refused: read it with <see cref="DecodeWithLayout"/>.
    /// </summary>
    public static (byte[] Payload, byte Flags) Decode(string typeName, ReadOnlySpan<byte> archive)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(typeName);

        return DecodeWithSchemaHash(SchemaHash(typeName), archive);
    }

    /// <summary>
    /// Decodes a current-layout (header minor <c>0</c>) frame and returns its flag byte. A dense
    /// frame (minor <c>1</c>) is refused.
    /// </summary>
    public static (byte[] Payload, byte Flags) DecodeWithSchemaHash(
        ReadOnlySpan<byte> schemaHash,
        ReadOnlySpan<byte> archive)
    {
        var payload = ReadFrame(schemaHash, archive, requiredLayout: null, exactPaddingLength: null, out var layout);
        if (layout == NoritoWireLayout.Dense)
        {
            throw new ArgumentException(
                "Norito archive declares the dense layout (header minor 1) where the current layout (minor 0) is required.",
                nameof(archive));
        }

        return (payload.ToArray(), layout.HeaderFlags());
    }

    /// <summary>
    /// Decodes a frame whose header must declare exactly <paramref name="layout"/>. A frame of
    /// any other layout is refused, never reinterpreted.
    /// </summary>
    /// <remarks>
    /// The frame must carry exactly the padding <see cref="EncodeWithLayout"/> writes for
    /// <paramref name="payloadAlignment"/>: none in a dense frame, the type's alignment padding
    /// in a minor-0 frame. Trailing bytes are refused.
    /// </remarks>
    public static byte[] DecodeWithLayout(
        string typeName,
        ReadOnlySpan<byte> archive,
        NoritoWireLayout layout,
        int payloadAlignment = 1)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(typeName);

        var padding = PayloadPadding(layout, payloadAlignment);
        return ReadFrame(SchemaHash(typeName), archive, layout, padding, out _).ToArray();
    }

    /// <summary>
    /// Reads the layout a frame declares from its header minor version and flag byte alone.
    /// </summary>
    public static NoritoWireLayout ReadLayout(ReadOnlySpan<byte> archive)
    {
        RequireHeaderPrefix(archive);
        return RequireHeaderLayout(archive);
    }

    public static byte[] SchemaHash(string typeName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(typeName);

        var typeNameBytes = Encoding.UTF8.GetBytes(typeName);
        var input = new byte[TypeNameSchemaHashDomain.Length + typeNameBytes.Length];
        TypeNameSchemaHashDomain.CopyTo(input, 0);
        typeNameBytes.CopyTo(input, TypeNameSchemaHashDomain.Length);
        return SHA256.HashData(input)[..16];
    }

    /// <summary>
    /// Zero padding between the header and the payload of a frame in <paramref name="layout"/>
    /// whose archived type has <paramref name="payloadAlignment"/>: the alignment padding for
    /// minor-0 frames, none for dense frames.
    /// </summary>
    internal static int PayloadPadding(NoritoWireLayout layout, int payloadAlignment)
    {
        if (payloadAlignment is < 1 or > MaxHeaderPaddingBytes
            || (payloadAlignment & (payloadAlignment - 1)) != 0)
        {
            throw new ArgumentOutOfRangeException(
                nameof(payloadAlignment),
                payloadAlignment,
                "Norito payload alignment must be a power of two between 1 and 64.");
        }

        if (layout.HeaderMinor() == NoritoWireLayoutRules.DenseMinor)
        {
            return 0;
        }

        var remainder = NoritoHeader.EncodedLength % payloadAlignment;
        return remainder == 0 ? 0 : payloadAlignment - remainder;
    }

    private static byte[] EncodeFrame(
        ReadOnlySpan<byte> schemaHash,
        ReadOnlySpan<byte> payload,
        byte minor,
        byte flags,
        int paddingLength)
    {
        var checksum = Crc64Ecma.Compute(payload);
        var header = new NoritoHeader(schemaHash.ToArray(), NoritoCompression.None, (ulong)payload.Length, checksum, flags)
        {
            Minor = minor,
        };

        var encodedHeader = header.Encode();
        var output = new byte[checked(encodedHeader.Length + paddingLength + payload.Length)];
        encodedHeader.CopyTo(output, 0);
        payload.CopyTo(output.AsSpan(encodedHeader.Length + paddingLength));
        return output;
    }

    // `requiredLayout` is the layout a typed caller names; the header must declare exactly it.
    // `exactPaddingLength` is the padding that caller requires. Without it a minor-0 frame may
    // carry up to 64 zero bytes; a dense frame never carries padding.
    private static ReadOnlySpan<byte> ReadFrame(
        ReadOnlySpan<byte> schemaHash,
        ReadOnlySpan<byte> archive,
        NoritoWireLayout? requiredLayout,
        int? exactPaddingLength,
        out NoritoWireLayout layout)
    {
        if (schemaHash.Length != 16)
        {
            throw new ArgumentException("Norito schema hash must be 16 bytes.", nameof(schemaHash));
        }

        RequireHeaderPrefix(archive);

        if (!archive.Slice(6, 16).SequenceEqual(schemaHash))
        {
            throw new ArgumentException("Norito archive schema hash does not match the expected type.", nameof(archive));
        }

        if (archive[22] != (byte)NoritoCompression.None)
        {
            throw new ArgumentException("Norito archive uses unsupported compression.", nameof(archive));
        }

        layout = RequireHeaderLayout(archive);
        if (requiredLayout is { } required && layout != required)
        {
            throw new ArgumentException(
                $"Norito archive declares the {layout} layout where the {required} layout is required.",
                nameof(archive));
        }

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(archive.Slice(23, sizeof(ulong)));
        var availableLength = archive.Length - NoritoHeader.EncodedLength;
        if (payloadLength > int.MaxValue || payloadLength > (ulong)availableLength)
        {
            throw new ArgumentException("Norito archive payload length is invalid.", nameof(archive));
        }

        var payloadLengthInt = (int)payloadLength;
        var paddingLength = availableLength - payloadLengthInt;
        // A dense frame has no alignment padding: its payload starts at byte 40 and ends the frame.
        var maxPaddingLength = layout == NoritoWireLayout.Dense ? 0 : MaxHeaderPaddingBytes;
        if (paddingLength > maxPaddingLength
            || (exactPaddingLength is { } exact && paddingLength != exact))
        {
            throw new ArgumentException("Norito archive header padding is invalid.", nameof(archive));
        }

        var padding = archive.Slice(NoritoHeader.EncodedLength, paddingLength);
        if (padding.IndexOfAnyExcept((byte)0) >= 0)
        {
            throw new ArgumentException("Norito archive header padding is invalid.", nameof(archive));
        }

        var payload = archive.Slice(NoritoHeader.EncodedLength + paddingLength, payloadLengthInt);
        var expectedChecksum = BinaryPrimitives.ReadUInt64LittleEndian(archive.Slice(31, sizeof(ulong)));
        if (Crc64Ecma.Compute(payload) != expectedChecksum)
        {
            throw new ArgumentException("Norito archive checksum does not match the payload.", nameof(archive));
        }

        return payload;
    }

    private static void RequireHeaderPrefix(ReadOnlySpan<byte> archive)
    {
        if (archive.Length < NoritoHeader.EncodedLength)
        {
            throw new ArgumentException("Norito archive is shorter than the header.", nameof(archive));
        }

        if (!archive[..4].SequenceEqual(Magic) || archive[4] != 0)
        {
            throw new ArgumentException("Norito archive has an unsupported magic or version.", nameof(archive));
        }

        if (archive[5] is not (NoritoWireLayoutRules.CurrentMinor or NoritoWireLayoutRules.DenseMinor))
        {
            throw new ArgumentException("Norito archive has an unsupported minor version.", nameof(archive));
        }
    }

    // The header minor and flag byte are the only source of a frame's layout.
    private static NoritoWireLayout RequireHeaderLayout(ReadOnlySpan<byte> archive)
    {
        if (!NoritoWireLayoutRules.TryFromHeader(archive[5], archive[39], out var layout))
        {
            throw new ArgumentException("Norito archive uses unsupported or reserved layout flags.", nameof(archive));
        }

        return layout;
    }

    private static void RequireSupportedFlags(byte flags, string parameterName)
    {
        if (!AreFlagsSupported(flags))
        {
            throw new ArgumentOutOfRangeException(
                parameterName,
                "Norito flags contain unsupported or reserved v1 layout bits.");
        }
    }

    internal static bool AreFlagsSupported(byte flags) => (flags & ~SupportedLayoutFlags) == 0;
}
