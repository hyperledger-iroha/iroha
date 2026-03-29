using System.Buffers.Binary;

namespace Hyperledger.Iroha.Norito;

internal static class Blake2b
{
    private static readonly ulong[] Iv =
    [
        0x6a09e667f3bcc908UL,
        0xbb67ae8584caa73bUL,
        0x3c6ef372fe94f82bUL,
        0xa54ff53a5f1d36f1UL,
        0x510e527fade682d1UL,
        0x9b05688c2b3e6c1fUL,
        0x1f83d9abfb41bd6bUL,
        0x5be0cd19137e2179UL,
    ];

    private static readonly int[,] Sigma =
    {
        { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15 },
        { 14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3 },
        { 11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4 },
        { 7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8 },
        { 9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13 },
        { 2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9 },
        { 12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11 },
        { 13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10 },
        { 6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5 },
        { 10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0 },
        { 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15 },
        { 14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3 },
    };

    public static byte[] Hash256(ReadOnlySpan<byte> data)
    {
        var state = (ulong[])Iv.Clone();
        state[0] ^= 0x01010020UL;

        var block = new byte[128];
        ulong t0 = 0;
        ulong t1 = 0;

        if (data.Length == 0)
        {
            Compress(state, block, 0, 0, ulong.MaxValue);
            return Finalize(state, 32);
        }

        var offset = 0;
        while (offset + 128 <= data.Length)
        {
            data.Slice(offset, 128).CopyTo(block);
            offset += 128;
            t0 += 128;
            if (t0 < 128)
            {
                t1++;
            }

            var finalBlock = offset == data.Length;
            Compress(state, block, t0, t1, finalBlock ? ulong.MaxValue : 0UL);
        }

        var remainder = data.Length - offset;
        if (remainder > 0)
        {
            Array.Clear(block);
            data[offset..].CopyTo(block);
            t0 += (ulong)remainder;
            if (t0 < (ulong)remainder)
            {
                t1++;
            }

            Compress(state, block, t0, t1, ulong.MaxValue);
        }

        return Finalize(state, 32);
    }

    private static byte[] Finalize(ulong[] state, int outputLength)
    {
        var output = new byte[outputLength];
        var written = 0;
        Span<byte> scratch = stackalloc byte[8];
        foreach (var value in state)
        {
            BinaryPrimitives.WriteUInt64LittleEndian(scratch, value);
            var remaining = Math.Min(8, outputLength - written);
            if (remaining <= 0)
            {
                break;
            }

            scratch[..remaining].CopyTo(output.AsSpan(written));
            written += remaining;
        }

        return output;
    }

    private static void Compress(ulong[] state, byte[] block, ulong t0, ulong t1, ulong f0)
    {
        Span<ulong> message = stackalloc ulong[16];
        for (var index = 0; index < 16; index++)
        {
            message[index] = BinaryPrimitives.ReadUInt64LittleEndian(block.AsSpan(index * 8, 8));
        }

        Span<ulong> work = stackalloc ulong[16];
        for (var index = 0; index < 8; index++)
        {
            work[index] = state[index];
            work[index + 8] = Iv[index];
        }

        work[12] ^= t0;
        work[13] ^= t1;
        work[14] ^= f0;

        for (var round = 0; round < 12; round++)
        {
            G(work, 0, 4, 8, 12, message[Sigma[round, 0]], message[Sigma[round, 1]]);
            G(work, 1, 5, 9, 13, message[Sigma[round, 2]], message[Sigma[round, 3]]);
            G(work, 2, 6, 10, 14, message[Sigma[round, 4]], message[Sigma[round, 5]]);
            G(work, 3, 7, 11, 15, message[Sigma[round, 6]], message[Sigma[round, 7]]);
            G(work, 0, 5, 10, 15, message[Sigma[round, 8]], message[Sigma[round, 9]]);
            G(work, 1, 6, 11, 12, message[Sigma[round, 10]], message[Sigma[round, 11]]);
            G(work, 2, 7, 8, 13, message[Sigma[round, 12]], message[Sigma[round, 13]]);
            G(work, 3, 4, 9, 14, message[Sigma[round, 14]], message[Sigma[round, 15]]);
        }

        for (var index = 0; index < 8; index++)
        {
            state[index] ^= work[index] ^ work[index + 8];
        }
    }

    private static void G(Span<ulong> state, int a, int b, int c, int d, ulong x, ulong y)
    {
        state[a] = unchecked(state[a] + state[b] + x);
        state[d] = RotateRight(state[d] ^ state[a], 32);
        state[c] = unchecked(state[c] + state[d]);
        state[b] = RotateRight(state[b] ^ state[c], 24);
        state[a] = unchecked(state[a] + state[b] + y);
        state[d] = RotateRight(state[d] ^ state[a], 16);
        state[c] = unchecked(state[c] + state[d]);
        state[b] = RotateRight(state[b] ^ state[c], 63);
    }

    private static ulong RotateRight(ulong value, int amount)
    {
        return (value >> amount) | (value << (64 - amount));
    }
}
