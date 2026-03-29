using System.Buffers.Binary;

namespace Hyperledger.Iroha.Norito;

internal sealed class OfflineNoritoWriter
{
    private readonly List<byte> buffer = [];

    public int Length => buffer.Count;

    public void WriteByte(byte value) => buffer.Add(value);

    public void WriteUInt16LittleEndian(ushort value)
    {
        Span<byte> scratch = stackalloc byte[2];
        BinaryPrimitives.WriteUInt16LittleEndian(scratch, value);
        WriteBytes(scratch);
    }

    public void WriteUInt32LittleEndian(uint value)
    {
        Span<byte> scratch = stackalloc byte[4];
        BinaryPrimitives.WriteUInt32LittleEndian(scratch, value);
        WriteBytes(scratch);
    }

    public void WriteUInt64LittleEndian(ulong value)
    {
        Span<byte> scratch = stackalloc byte[8];
        BinaryPrimitives.WriteUInt64LittleEndian(scratch, value);
        WriteBytes(scratch);
    }

    public void WriteLength(ulong value) => WriteUInt64LittleEndian(value);

    public void WriteBytes(ReadOnlySpan<byte> bytes)
    {
        foreach (var value in bytes)
        {
            buffer.Add(value);
        }
    }

    public void WriteField(ReadOnlySpan<byte> payload)
    {
        WriteLength((ulong)payload.Length);
        WriteBytes(payload);
    }

    public byte[] ToArray() => [.. buffer];
}
