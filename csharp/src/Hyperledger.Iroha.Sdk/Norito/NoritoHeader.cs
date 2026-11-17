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

    /// <summary>
    /// Header minor version: <c>0</c> for the current layout (flags <c>0x00</c> or
    /// <c>0x02</c>), <c>1</c> for the dense layout (flags <c>0x00</c>).
    /// </summary>
    public byte Minor { get; init; }

    public bool Equals(NoritoHeader other) =>
        Compression == other.Compression
        && Length == other.Length
        && Checksum == other.Checksum
        && Flags == other.Flags
        && Minor == other.Minor
        && (schemaHash ?? []).AsSpan().SequenceEqual(other.schemaHash ?? []);

    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.Add(Compression);
        hash.Add(Length);
        hash.Add(Checksum);
        hash.Add(Flags);
        hash.Add(Minor);
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

        if (Minor is not (NoritoWireLayoutRules.CurrentMinor or NoritoWireLayoutRules.DenseMinor))
        {
            throw new ArgumentOutOfRangeException(
                nameof(Minor),
                "Norito v1 header minor must be 0 (current layout) or 1 (dense layout).");
        }

        if (!NoritoWireLayoutRules.TryFromHeader(Minor, Flags, out _))
        {
            throw new ArgumentOutOfRangeException(
                nameof(Flags),
                Minor == NoritoWireLayoutRules.DenseMinor
                    ? "Norito dense headers (minor 1) carry no layout flags."
                    : "Norito v1 header flags must be 0x00 or COMPACT_LEN (0x02).");
        }

        var buffer = new byte[EncodedLength];
        Magic.CopyTo(buffer);
        buffer[4] = 0;
        buffer[5] = Minor;
        schemaHash.CopyTo(buffer, 6);
        buffer[22] = (byte)Compression;
        BinaryPrimitives.WriteUInt64LittleEndian(buffer.AsSpan(23, 8), Length);
        BinaryPrimitives.WriteUInt64LittleEndian(buffer.AsSpan(31, 8), Checksum);
        buffer[39] = Flags;
        return buffer;
    }
}
