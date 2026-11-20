using System.Buffers.Binary;

namespace Hyperledger.Iroha.Norito;

/// <summary>
/// Reads typed Norito payload bytes in one explicit layout: the compact layout (the default) or
/// the dense layout (<c>norito.md</c>, "Dense layout"). The caller names the layout; the reader
/// never infers it from the bytes.
/// </summary>
internal ref struct CanonicalNoritoReader
{
    private readonly ReadOnlySpan<byte> input;
    private readonly string context;
    private readonly string parameterName;
    private readonly bool dense;
    private int offset;

    internal CanonicalNoritoReader(
        ReadOnlySpan<byte> input,
        string context,
        string parameterName)
        : this(input, context, parameterName, NoritoWireLayout.Compact)
    {
    }

    internal CanonicalNoritoReader(
        ReadOnlySpan<byte> input,
        string context,
        string parameterName,
        NoritoWireLayout layout)
    {
        this.input = input;
        this.context = context;
        this.parameterName = parameterName;
        dense = NoritoWireLayoutRules.RequireTyped(layout, nameof(layout)) == NoritoWireLayout.Dense;
        offset = 0;
    }

    /// <summary>The layout every count, discriminant and byte sequence is read in.</summary>
    internal readonly NoritoWireLayout Layout => dense ? NoritoWireLayout.Dense : NoritoWireLayout.Compact;

    internal readonly int Remaining => input.Length - offset;

    internal readonly bool IsFinished => offset == input.Length;

    internal byte ReadByte(string fieldName) => ReadExact(1, fieldName)[0];

    internal uint ReadUInt32LittleEndian(string fieldName) =>
        BinaryPrimitives.ReadUInt32LittleEndian(ReadExact(sizeof(uint), fieldName));

    internal ulong ReadUInt64LittleEndian(string fieldName) =>
        BinaryPrimitives.ReadUInt64LittleEndian(ReadExact(sizeof(ulong), fieldName));

    /// <summary>
    /// Reads a sequence, map or set count: a fixed <c>u64</c> in the compact layout, a
    /// shortest-form varint in the dense layout.
    /// </summary>
    internal ulong ReadSequenceLength(string fieldName) =>
        dense ? ReadCompactLength(fieldName) : ReadUInt64LittleEndian(fieldName);

    /// <summary>
    /// Reads a sequence count whose elements each occupy at least
    /// <paramref name="minimumElementBytes"/> bytes, refusing a count the remaining input
    /// cannot hold before anything is allocated for it.
    /// </summary>
    internal int ReadBoundedSequenceLength(string fieldName, int minimumElementBytes)
    {
        ArgumentOutOfRangeException.ThrowIfLessThan(minimumElementBytes, 1);
        var count = ReadSequenceLength(fieldName);
        if (count > (ulong)(Remaining / minimumElementBytes))
        {
            throw Invalid($"{fieldName} exceeds the remaining input.");
        }

        return (int)count;
    }

    /// <summary>
    /// Reads an enum discriminant: a <c>u32</c> word in the compact layout, a shortest-form
    /// varint that fits <c>u32</c> in the dense layout.
    /// </summary>
    internal uint ReadEnumTag(string fieldName)
    {
        if (!dense)
        {
            return ReadUInt32LittleEndian(fieldName);
        }

        var tag = ReadCompactLength(fieldName);
        if (tag > uint.MaxValue)
        {
            throw Invalid($"{fieldName} discriminant overflows UInt32.");
        }

        return (uint)tag;
    }

    /// <summary>
    /// Reads <paramref name="count"/> elements of a byte array or element sequence of
    /// <c>u8</c>: one <c>[len = 1][byte]</c> pair per byte in the compact layout, the raw bytes
    /// in the dense layout.
    /// </summary>
    internal byte[] ReadByteElements(int count, string fieldName)
    {
        if (dense)
        {
            return ReadExact(count, fieldName).ToArray();
        }

        if (count < 0 || count > Remaining / 2)
        {
            throw Invalid($"{fieldName} is truncated.");
        }

        var pairs = ReadExact(count * 2, fieldName);
        var values = new byte[count];
        for (var index = 0; index < count; index++)
        {
            if (pairs[index * 2] != 1)
            {
                throw Invalid($"{fieldName}[{index}] must be a one-byte element.");
            }

            values[index] = pairs[index * 2 + 1];
        }

        return values;
    }

    internal ulong ReadCompactLength(string fieldName)
    {
        ulong value = 0;
        var shift = 0;
        for (var index = 0; index < 10; index++)
        {
            if (offset >= input.Length)
            {
                throw Invalid($"{fieldName} compact length is truncated.");
            }

            var current = input[offset++];
            var chunk = (ulong)(current & 0x7f);
            if (shift == 63 && chunk > 1)
            {
                throw Invalid($"{fieldName} compact length overflows UInt64.");
            }

            value |= chunk << shift;
            if ((current & 0x80) == 0)
            {
                if (index > 0 && chunk == 0)
                {
                    throw Invalid($"{fieldName} compact length is overlong.");
                }

                return value;
            }

            shift += 7;
        }

        throw Invalid($"{fieldName} compact length overflows UInt64.");
    }

    internal ReadOnlySpan<byte> ReadField(string fieldName)
    {
        var length = ReadCompactLength($"{fieldName}.length");
        if (length > int.MaxValue)
        {
            throw Invalid($"{fieldName} exceeds the managed runtime bound.");
        }

        return ReadExact(checked((int)length), fieldName);
    }

    internal ReadOnlySpan<byte> ReadExact(int count, string fieldName)
    {
        if (count < 0 || offset > input.Length - count)
        {
            throw Invalid($"{fieldName} is truncated.");
        }

        var value = input.Slice(offset, count);
        offset += count;
        return value;
    }

    internal readonly void RequireEnd()
    {
        if (!IsFinished)
        {
            throw Invalid("contains trailing or unknown bytes.");
        }
    }

    private readonly ArgumentException Invalid(string message) =>
        new($"{context} {message}", parameterName);
}
