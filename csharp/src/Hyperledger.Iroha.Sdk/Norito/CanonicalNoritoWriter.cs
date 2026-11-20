using System.Buffers;
using System.Buffers.Binary;

namespace Hyperledger.Iroha.Norito;

/// <summary>
/// Writes typed Norito payload bytes in one explicit layout: the compact layout (the default)
/// or the dense layout (<c>norito.md</c>, "Dense layout").
/// </summary>
internal sealed class CanonicalNoritoWriter
{
    private readonly ArrayBufferWriter<byte> buffer = new();

    internal CanonicalNoritoWriter()
        : this(NoritoWireLayout.Compact)
    {
    }

    internal CanonicalNoritoWriter(NoritoWireLayout layout)
    {
        Layout = NoritoWireLayoutRules.RequireTyped(layout, nameof(layout));
    }

    /// <summary>The layout every count, discriminant and byte sequence is written in.</summary>
    internal NoritoWireLayout Layout { get; }

    private bool IsDense => Layout == NoritoWireLayout.Dense;

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

    /// <summary>
    /// Writes a sequence, map or set count: a fixed <c>u64</c> in the compact layout, a
    /// shortest-form varint in the dense layout.
    /// </summary>
    internal void WriteSequenceLength(ulong value)
    {
        if (IsDense)
        {
            WriteCompactLength(value);
            return;
        }

        WriteUInt64LittleEndian(value);
    }

    /// <summary>
    /// Writes an enum discriminant: a <c>u32</c> word in the compact layout, a shortest-form
    /// varint in the dense layout. <c>Option</c>, <c>Result</c> and <c>bool</c> tags are single
    /// bytes in both layouts and use <see cref="WriteByte"/>.
    /// </summary>
    internal void WriteEnumTag(uint tag)
    {
        if (IsDense)
        {
            WriteCompactLength(tag);
            return;
        }

        WriteUInt32LittleEndian(tag);
    }

    internal void WriteField(ReadOnlySpan<byte> payload)
    {
        WriteCompactLength(checked((ulong)payload.Length));
        WriteBytes(payload);
    }

    /// <summary>
    /// Writes the elements of a byte array or element sequence of <c>u8</c> without their count:
    /// one <c>[len = 1][byte]</c> pair per byte in the compact layout, the raw bytes in the dense
    /// layout.
    /// </summary>
    internal void WriteByteElements(ReadOnlySpan<byte> values)
    {
        if (IsDense)
        {
            WriteBytes(values);
            return;
        }

        var encodedLength = checked(values.Length * 2);
        var encoded = buffer.GetSpan(encodedLength)[..encodedLength];
        for (var index = 0; index < values.Length; index++)
        {
            encoded[index * 2] = 1;
            encoded[index * 2 + 1] = values[index];
        }

        buffer.Advance(encodedLength);
    }

    internal void WriteBytes(ReadOnlySpan<byte> value)
    {
        value.CopyTo(buffer.GetSpan(value.Length));
        buffer.Advance(value.Length);
    }

    internal byte[] ToArray() => buffer.WrittenSpan.ToArray();
}
