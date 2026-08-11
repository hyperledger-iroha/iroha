using System.Buffers.Binary;

namespace Hyperledger.Iroha.Norito;

internal ref struct CanonicalNoritoReader
{
    private readonly ReadOnlySpan<byte> input;
    private readonly string context;
    private readonly string parameterName;
    private int offset;

    internal CanonicalNoritoReader(
        ReadOnlySpan<byte> input,
        string context,
        string parameterName)
    {
        this.input = input;
        this.context = context;
        this.parameterName = parameterName;
        offset = 0;
    }

    internal readonly int Remaining => input.Length - offset;

    internal readonly bool IsFinished => offset == input.Length;

    internal byte ReadByte(string fieldName) => ReadExact(1, fieldName)[0];

    internal uint ReadUInt32LittleEndian(string fieldName) =>
        BinaryPrimitives.ReadUInt32LittleEndian(ReadExact(sizeof(uint), fieldName));

    internal ulong ReadUInt64LittleEndian(string fieldName) =>
        BinaryPrimitives.ReadUInt64LittleEndian(ReadExact(sizeof(ulong), fieldName));

    internal ulong ReadSequenceLength(string fieldName) =>
        ReadUInt64LittleEndian(fieldName);

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
