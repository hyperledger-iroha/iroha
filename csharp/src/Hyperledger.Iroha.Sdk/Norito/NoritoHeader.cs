using System.Buffers.Binary;

namespace Hyperledger.Iroha.Norito;

public readonly record struct NoritoHeader(
    byte[] SchemaHash,
    NoritoCompression Compression,
    ulong Length,
    ulong Checksum,
    byte Flags)
{
    public const int EncodedLength = 40;

    private static ReadOnlySpan<byte> Magic => "NRT0"u8;

    public byte[] Encode()
    {
        if (SchemaHash.Length != 16)
        {
            throw new ArgumentException("Norito schema hash must be 16 bytes.", nameof(SchemaHash));
        }

        var buffer = new byte[EncodedLength];
        Magic.CopyTo(buffer);
        buffer[4] = 0;
        buffer[5] = 0;
        SchemaHash.CopyTo(buffer, 6);
        buffer[22] = (byte)Compression;
        BinaryPrimitives.WriteUInt64LittleEndian(buffer.AsSpan(23, 8), Length);
        BinaryPrimitives.WriteUInt64LittleEndian(buffer.AsSpan(31, 8), Checksum);
        buffer[39] = Flags;
        return buffer;
    }
}
