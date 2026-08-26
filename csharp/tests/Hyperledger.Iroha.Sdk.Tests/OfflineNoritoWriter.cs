using System.Buffers.Binary;

namespace Hyperledger.Iroha.Norito;

/// <summary>Builds deliberately retired fixed-length Norito fixtures for rejection tests.</summary>
internal sealed class OfflineNoritoWriter
{
    private readonly List<byte> buffer = [];

    public void WriteByte(byte value) => buffer.Add(value);

    public void WriteUInt32LittleEndian(uint value)
    {
        Span<byte> scratch = stackalloc byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32LittleEndian(scratch, value);
        WriteBytes(scratch);
    }

    public void WriteUInt64LittleEndian(ulong value)
    {
        Span<byte> scratch = stackalloc byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(scratch, value);
        WriteBytes(scratch);
    }

    public void WriteBytes(ReadOnlySpan<byte> bytes)
    {
        foreach (var value in bytes)
        {
            buffer.Add(value);
        }
    }

    public void WriteField(ReadOnlySpan<byte> payload)
    {
        WriteUInt64LittleEndian((ulong)payload.Length);
        WriteBytes(payload);
    }

    public byte[] ToArray() => [.. buffer];
}
