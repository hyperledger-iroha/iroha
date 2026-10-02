using System.Numerics;

namespace Hyperledger.Iroha.Petal;

/// <summary>CRC-32C (Castagnoli) that binds a reassembled Petal payload to its beacon.</summary>
public static class PetalCrc32C
{
    /// <summary>
    /// Computes CRC-32C: reflected polynomial <c>0x82F63B78</c>, initial value
    /// <c>0xFFFFFFFF</c>, final xor <c>0xFFFFFFFF</c>.
    /// </summary>
    /// <param name="bytes">Input bytes.</param>
    /// <returns>The checksum; <c>0</c> for empty input.</returns>
    /// <remarks>
    /// <see cref="BitOperations.Crc32C(uint, byte)"/> uses the CPU CRC instruction
    /// when present and an identical software table otherwise, so every host
    /// returns the same value.
    /// </remarks>
    public static uint Compute(ReadOnlySpan<byte> bytes)
    {
        var crc = 0xFFFF_FFFFu;
        foreach (var value in bytes)
            crc = BitOperations.Crc32C(crc, value);
        return ~crc;
    }
}

/// <summary>
/// Marsaglia xorshift32 with shifts 13, 17 and 5: the one generator shared by
/// lane whitening and the test vectors of the fountain code.
/// </summary>
/// <remarks>
/// The generator is part of the wire format: whitening sequences are derived
/// from it, so every implementation must match bit for bit.
/// </remarks>
public sealed class PetalXorshift32 : IEquatable<PetalXorshift32>
{
    /// <summary>Replacement for a zero seed; xorshift cannot leave the all-zero state.</summary>
    public const uint ZeroSeedReplacement = 0xDEAD_BEEF;

    /// <summary>Creates a generator; a zero seed is replaced by <see cref="ZeroSeedReplacement"/>.</summary>
    /// <param name="seed">Initial state.</param>
    public PetalXorshift32(uint seed)
    {
        State = seed == 0 ? ZeroSeedReplacement : seed;
    }

    /// <summary>Current internal state.</summary>
    public uint State { get; private set; }

    /// <summary>Advances the generator and returns the next 32-bit word.</summary>
    /// <returns>The next output word.</returns>
    public uint NextUInt32()
    {
        var x = State;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        State = x;
        return x;
    }

    /// <summary>Returns the top byte of the next word.</summary>
    /// <returns>The most significant byte of <see cref="NextUInt32"/>.</returns>
    public byte NextByte() => (byte)(NextUInt32() >> 24);

    /// <inheritdoc />
    public bool Equals(PetalXorshift32? other) => other is not null && State == other.State;

    /// <inheritdoc />
    public override bool Equals(object? obj) => Equals(obj as PetalXorshift32);

    /// <inheritdoc />
    public override int GetHashCode() => (int)State;
}
