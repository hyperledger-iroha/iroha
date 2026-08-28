using System.Numerics;

namespace Hyperledger.Iroha.Norito;

/// <summary>Minimal deterministic BLAKE3 helper for protocol receipt digests.</summary>
internal static class Blake3
{
    private const int BlockLength = 64;
    private const int ChunkLength = 1024;
    private const int OutputLength = 32;
    private const uint ChunkStart = 1;
    private const uint ChunkEnd = 2;
    private const uint Parent = 4;
    private const uint Root = 8;

    private static readonly uint[] Iv =
    [
        0x6A09E667,
        0xBB67AE85,
        0x3C6EF372,
        0xA54FF53A,
        0x510E527F,
        0x9B05688C,
        0x1F83D9AB,
        0x5BE0CD19,
    ];

    private static readonly int[] MessagePermutation =
    [
        2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8,
    ];

    public static byte[] Hash(ReadOnlySpan<byte> input)
    {
        var rootOutput = RootOutput(input);
        var output = new byte[OutputLength];
        var words = rootOutput.RootWords(0);
        for (var index = 0; index < OutputLength / sizeof(uint); index++)
        {
            var word = words[index];
            var offset = index * sizeof(uint);
            output[offset] = (byte)word;
            output[offset + 1] = (byte)(word >> 8);
            output[offset + 2] = (byte)(word >> 16);
            output[offset + 3] = (byte)(word >> 24);
        }

        return output;
    }

    private static Output RootOutput(ReadOnlySpan<byte> input)
    {
        var chunkCount = Math.Max(1, (input.Length + ChunkLength - 1) / ChunkLength);
        return SubtreeOutput(input, 0, chunkCount);
    }

    private static Output SubtreeOutput(
        ReadOnlySpan<byte> input,
        int chunkIndex,
        int chunkCount)
    {
        if (chunkCount == 1)
        {
            var offset = chunkIndex * ChunkLength;
            var length = Math.Min(ChunkLength, Math.Max(0, input.Length - offset));
            return ChunkOutput(input, offset, length, (ulong)chunkIndex);
        }

        var leftCount = LeftSubtreeChunkCount(chunkCount);
        var left = SubtreeOutput(input, chunkIndex, leftCount).ChainingValue();
        var right = SubtreeOutput(
            input,
            chunkIndex + leftCount,
            chunkCount - leftCount).ChainingValue();
        return ParentOutput(left, right);
    }

    private static int LeftSubtreeChunkCount(int chunkCount)
    {
        var power = 1;
        while (power * 2 < chunkCount)
        {
            power *= 2;
        }

        return power;
    }

    private static Output ChunkOutput(
        ReadOnlySpan<byte> input,
        int offset,
        int length,
        ulong chunkCounter)
    {
        var chainingValue = (uint[])Iv.Clone();
        var blockCount = Math.Max(1, (length + BlockLength - 1) / BlockLength);
        for (var index = 0; index < blockCount; index++)
        {
            var blockStart = offset + index * BlockLength;
            var blockLength = Math.Min(BlockLength, Math.Max(0, length - index * BlockLength));
            var blockWords = ParseBlockWords(input, blockStart);
            var flags = index == 0 ? ChunkStart : 0;
            if (index == blockCount - 1)
            {
                return new Output(
                    (uint[])chainingValue.Clone(),
                    blockWords,
                    chunkCounter,
                    (uint)blockLength,
                    flags | ChunkEnd);
            }

            var state = Compress(
                chainingValue,
                blockWords,
                chunkCounter,
                (uint)blockLength,
                flags);
            for (var word = 0; word < 8; word++)
            {
                chainingValue[word] = state[word] ^ state[word + 8];
            }
        }

        throw new InvalidOperationException("BLAKE3 chunk output is unreachable.");
    }

    private static Output ParentOutput(uint[] left, uint[] right)
    {
        var blockWords = new uint[16];
        Array.Copy(left, 0, blockWords, 0, 8);
        Array.Copy(right, 0, blockWords, 8, 8);
        return new Output((uint[])Iv.Clone(), blockWords, 0, BlockLength, Parent);
    }

    private static uint[] ParseBlockWords(ReadOnlySpan<byte> input, int blockStart)
    {
        var words = new uint[16];
        for (var index = 0; index < words.Length; index++)
        {
            var offset = blockStart + index * sizeof(uint);
            var byte0 = offset < input.Length ? input[offset] : 0U;
            var byte1 = offset + 1 < input.Length ? input[offset + 1] : 0U;
            var byte2 = offset + 2 < input.Length ? input[offset + 2] : 0U;
            var byte3 = offset + 3 < input.Length ? input[offset + 3] : 0U;
            words[index] = byte0 | byte1 << 8 | byte2 << 16 | byte3 << 24;
        }

        return words;
    }

    private static uint[] Compress(
        uint[] chainingValue,
        uint[] blockWords,
        ulong counter,
        uint blockLength,
        uint flags)
    {
        var state = new uint[]
        {
            chainingValue[0], chainingValue[1], chainingValue[2], chainingValue[3],
            chainingValue[4], chainingValue[5], chainingValue[6], chainingValue[7],
            Iv[0], Iv[1], Iv[2], Iv[3],
            (uint)counter, (uint)(counter >> 32), blockLength, flags,
        };
        var message = (uint[])blockWords.Clone();

        for (var round = 0; round < 7; round++)
        {
            Mix(state, 0, 4, 8, 12, message[0], message[1]);
            Mix(state, 1, 5, 9, 13, message[2], message[3]);
            Mix(state, 2, 6, 10, 14, message[4], message[5]);
            Mix(state, 3, 7, 11, 15, message[6], message[7]);
            Mix(state, 0, 5, 10, 15, message[8], message[9]);
            Mix(state, 1, 6, 11, 12, message[10], message[11]);
            Mix(state, 2, 7, 8, 13, message[12], message[13]);
            Mix(state, 3, 4, 9, 14, message[14], message[15]);
            if (round < 6)
            {
                message = Permute(message);
            }
        }

        return state;
    }

    private static void Mix(
        uint[] state,
        int a,
        int b,
        int c,
        int d,
        uint messageX,
        uint messageY)
    {
        state[a] = unchecked(state[a] + state[b] + messageX);
        state[d] = BitOperations.RotateRight(state[d] ^ state[a], 16);
        state[c] = unchecked(state[c] + state[d]);
        state[b] = BitOperations.RotateRight(state[b] ^ state[c], 12);
        state[a] = unchecked(state[a] + state[b] + messageY);
        state[d] = BitOperations.RotateRight(state[d] ^ state[a], 8);
        state[c] = unchecked(state[c] + state[d]);
        state[b] = BitOperations.RotateRight(state[b] ^ state[c], 7);
    }

    private static uint[] Permute(uint[] message)
    {
        var permuted = new uint[16];
        for (var index = 0; index < permuted.Length; index++)
        {
            permuted[index] = message[MessagePermutation[index]];
        }

        return permuted;
    }

    private sealed class Output(
        uint[] inputChainingValue,
        uint[] blockWords,
        ulong counter,
        uint blockLength,
        uint flags)
    {
        public uint[] ChainingValue()
        {
            var state = Compress(
                inputChainingValue,
                blockWords,
                counter,
                blockLength,
                flags);
            var output = new uint[8];
            for (var index = 0; index < output.Length; index++)
            {
                output[index] = state[index] ^ state[index + 8];
            }

            return output;
        }

        public uint[] RootWords(ulong outputBlockCounter)
        {
            var state = Compress(
                inputChainingValue,
                blockWords,
                outputBlockCounter,
                blockLength,
                flags | Root);
            var output = new uint[16];
            for (var index = 0; index < 8; index++)
            {
                output[index] = state[index] ^ state[index + 8];
                output[index + 8] = state[index + 8] ^ inputChainingValue[index];
            }

            return output;
        }
    }
}
