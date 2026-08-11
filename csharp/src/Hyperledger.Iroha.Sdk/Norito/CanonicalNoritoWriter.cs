using System.Buffers;
using System.Buffers.Binary;

namespace Hyperledger.Iroha.Norito;

internal sealed class CanonicalNoritoWriter
{
    private readonly ArrayBufferWriter<byte> buffer = new();

    internal int Length => buffer.WrittenCount;

    internal void WriteByte(byte value)
    {
        buffer.GetSpan(1)[0] = value;
        buffer.Advance(1);
    }

    internal void WriteUInt16LittleEndian(ushort value)
    {
        Span<byte> encoded = stackalloc byte[sizeof(ushort)];
        BinaryPrimitives.WriteUInt16LittleEndian(encoded, value);
        WriteBytes(encoded);
    }

    internal void WriteUInt32LittleEndian(uint value)
    {
        Span<byte> encoded = stackalloc byte[sizeof(uint)];
        BinaryPrimitives.WriteUInt32LittleEndian(encoded, value);
        WriteBytes(encoded);
    }

    internal void WriteUInt64LittleEndian(ulong value)
    {
        Span<byte> encoded = stackalloc byte[sizeof(ulong)];
        BinaryPrimitives.WriteUInt64LittleEndian(encoded, value);
        WriteBytes(encoded);
    }

    internal void WriteCompactLength(ulong value)
    {
        Span<byte> encoded = stackalloc byte[10];
        var count = 0;
        do
        {
            var next = (byte)(value & 0x7f);
            value >>= 7;
            if (value != 0)
            {
                next |= 0x80;
            }

            encoded[count++] = next;
        }
        while (value != 0);

        WriteBytes(encoded[..count]);
    }

    internal void WriteSequenceLength(ulong value) => WriteUInt64LittleEndian(value);

    internal void WriteField(ReadOnlySpan<byte> payload)
    {
        WriteCompactLength(checked((ulong)payload.Length));
        WriteBytes(payload);
    }

    internal void WriteBytes(ReadOnlySpan<byte> value)
    {
        value.CopyTo(buffer.GetSpan(value.Length));
        buffer.Advance(value.Length);
    }

    internal byte[] ToArray() => buffer.WrittenSpan.ToArray();
}
