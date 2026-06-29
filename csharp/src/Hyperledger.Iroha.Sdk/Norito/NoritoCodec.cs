using System.Buffers.Binary;
using System.Security.Cryptography;
using System.Text;

namespace Hyperledger.Iroha.Norito;

public static class NoritoCodec
{
    private const int MaxHeaderPaddingBytes = 64;
    private const byte SupportedLayoutFlags = 0x27;
    private const byte FieldBitsetFlag = 0x20;
    private const byte FieldBitsetRequiredFlags = 0x06;

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

        var checksum = Crc64Ecma.Compute(payload);
        var header = new NoritoHeader(schemaHash.ToArray(), NoritoCompression.None, (ulong)payload.Length, checksum, flags);

        var encodedHeader = header.Encode();
        var output = new byte[encodedHeader.Length + payload.Length];
        encodedHeader.CopyTo(output, 0);
        payload.CopyTo(output.AsSpan(encodedHeader.Length));
        return output;
    }

    public static (byte[] Payload, byte Flags) Decode(string typeName, ReadOnlySpan<byte> archive)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(typeName);

        return DecodeWithSchemaHash(SchemaHash(typeName), archive);
    }

    public static (byte[] Payload, byte Flags) DecodeWithSchemaHash(
        ReadOnlySpan<byte> schemaHash,
        ReadOnlySpan<byte> archive)
    {
        if (schemaHash.Length != 16)
        {
            throw new ArgumentException("Norito schema hash must be 16 bytes.", nameof(schemaHash));
        }

        if (archive.Length < NoritoHeader.EncodedLength)
        {
            throw new ArgumentException("Norito archive is shorter than the header.", nameof(archive));
        }

        if (!archive[..4].SequenceEqual(Magic) || archive[4] != 0 || archive[5] != 0)
        {
            throw new ArgumentException("Norito archive has an unsupported magic or version.", nameof(archive));
        }

        if (!archive.Slice(6, 16).SequenceEqual(schemaHash))
        {
            throw new ArgumentException("Norito archive schema hash does not match the expected type.", nameof(archive));
        }

        if (archive[22] != (byte)NoritoCompression.None)
        {
            throw new ArgumentException("Norito archive uses unsupported compression.", nameof(archive));
        }

        var flags = archive[39];
        if (!AreFlagsSupported(flags))
        {
            throw new ArgumentException("Norito archive uses unsupported or reserved layout flags.", nameof(archive));
        }

        var payloadLength = BinaryPrimitives.ReadUInt64LittleEndian(archive.Slice(23, sizeof(ulong)));
        var availableLength = archive.Length - NoritoHeader.EncodedLength;
        if (payloadLength > int.MaxValue || payloadLength > (ulong)availableLength)
        {
            throw new ArgumentException("Norito archive payload length is invalid.", nameof(archive));
        }

        var payloadLengthInt = (int)payloadLength;
        var paddingLength = availableLength - payloadLengthInt;
        if (paddingLength > MaxHeaderPaddingBytes)
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

        return (payload.ToArray(), flags);
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

    private static void RequireSupportedFlags(byte flags, string parameterName)
    {
        if (!AreFlagsSupported(flags))
        {
            throw new ArgumentOutOfRangeException(
                parameterName,
                "Norito flags contain unsupported or reserved v1 layout bits.");
        }
    }

    private static bool AreFlagsSupported(byte flags)
    {
        return (flags & ~SupportedLayoutFlags) == 0
            && ((flags & FieldBitsetFlag) == 0
                || (flags & FieldBitsetRequiredFlags) == FieldBitsetRequiredFlags);
    }
}
