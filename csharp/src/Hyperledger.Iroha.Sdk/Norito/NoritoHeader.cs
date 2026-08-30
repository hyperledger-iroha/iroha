using System.Buffers.Binary;

namespace Hyperledger.Iroha.Norito;

public readonly record struct NoritoHeader
{
    public const int EncodedLength = 40;

    private readonly byte[] schemaHash;

    public NoritoHeader(
        byte[] schemaHash,
        NoritoCompression compression,
        ulong length,
        ulong checksum,
        byte flags)
    {
        ArgumentNullException.ThrowIfNull(schemaHash);

        this.schemaHash = [.. schemaHash];
        Compression = compression;
        Length = length;
        Checksum = checksum;
        Flags = flags;
    }

    private static ReadOnlySpan<byte> Magic => "NRT0"u8;

    public byte[] SchemaHash
    {
        get => schemaHash is null ? [] : [.. schemaHash];
        init
        {
            ArgumentNullException.ThrowIfNull(value);
            schemaHash = [.. value];
        }
    }

    public NoritoCompression Compression { get; init; }

    public ulong Length { get; init; }

    public ulong Checksum { get; init; }

    public byte Flags { get; init; }

    public bool Equals(NoritoHeader other) =>
        Compression == other.Compression
        && Length == other.Length
        && Checksum == other.Checksum
        && Flags == other.Flags
        && (schemaHash ?? []).AsSpan().SequenceEqual(other.schemaHash ?? []);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Compression);
        hash.Add(Length);
        hash.Add(Checksum);
        hash.Add(Flags);
        foreach (var value in schemaHash ?? [])
        {
            hash.Add(value);
        }
        return hash.ToHashCode();
    }

    public byte[] Encode()
    {
        if (schemaHash is null || schemaHash.Length != 16)
        {
            throw new ArgumentException("Norito schema hash must be 16 bytes.", nameof(SchemaHash));
        }

        var buffer = new byte[EncodedLength];
        Magic.CopyTo(buffer);
        buffer[4] = 0;
        buffer[5] = 0;
        schemaHash.CopyTo(buffer, 6);
        buffer[22] = (byte)Compression;
        BinaryPrimitives.WriteUInt64LittleEndian(buffer.AsSpan(23, 8), Length);
        BinaryPrimitives.WriteUInt64LittleEndian(buffer.AsSpan(31, 8), Checksum);
        buffer[39] = Flags;
        return buffer;
    }
}
