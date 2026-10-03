using System.Numerics;

namespace Hyperledger.Iroha.Petal;

/// <summary>
/// Rateless fountain code over GF(2).
/// </summary>
/// <remarks>
/// A payload is cut into <c>k</c> source atoms of <see cref="PetalLanes.AtomLength"/>
/// bytes (the last one zero-padded). Encoded atom <c>id</c> is source atom
/// <c>id</c> for <c>id &lt; k</c> (systematic), and otherwise the XOR of a
/// pseudo-random half of the source atoms chosen by <see cref="MaskWords"/>.
/// A receiver that holds any <c>k + 2</c> or so independent atoms, in any
/// order, recovers the payload by Gaussian elimination; lost frames cost
/// nothing but time.
/// </remarks>
public static class PetalFountain
{
    /// <summary>Splits a payload into zero-padded 16-byte source atoms.</summary>
    /// <param name="payload">Payload bytes.</param>
    /// <returns>One array per source atom.</returns>
    public static byte[][] SplitPayload(ReadOnlySpan<byte> payload)
    {
        var count = (payload.Length + PetalLanes.AtomLength - 1) / PetalLanes.AtomLength;
        var atoms = new byte[count][];
        for (var i = 0; i < count; i++)
        {
            var atom = new byte[PetalLanes.AtomLength];
            var start = i * PetalLanes.AtomLength;
            payload.Slice(start, Math.Min(PetalLanes.AtomLength, payload.Length - start)).CopyTo(atom);
            atoms[i] = atom;
        }

        return atoms;
    }

    /// <summary>Number of 32-bit words needed for a mask over <paramref name="k"/> source atoms.</summary>
    /// <param name="k">Source atom count.</param>
    /// <returns><c>ceil(k / 32)</c>.</returns>
    public static int MaskLength(int k)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(k);
        return (int)(((long)k + 31) / 32);
    }

    /// <summary>The 32-bit finalizer of MurmurHash3 (<c>fmix32</c>).</summary>
    /// <remarks>
    /// Masks must not come from a GF(2)-linear generator such as xorshift: every
    /// mask would then lie in a subspace of dimension at most 32 and repair atoms
    /// could never raise the decoder rank past 32. The multiplications make this
    /// mixer nonlinear over GF(2).
    /// </remarks>
    /// <param name="x">Input word.</param>
    /// <returns>The mixed word.</returns>
    public static uint Mix32(uint x)
    {
        unchecked
        {
            x ^= x >> 16;
            x *= 0x85EB_CA6B;
            x ^= x >> 13;
            x *= 0xC2B2_AE35;
            x ^= x >> 16;
            return x;
        }
    }

    /// <summary>The combination mask of encoded atom <paramref name="id"/>, as little-endian bit words.</summary>
    /// <remarks>
    /// <para>
    /// <paramref name="crc"/> is the payload CRC-32C and only diversifies masks
    /// between streams. Atoms with <c>id &lt; k</c> are systematic (a unit
    /// vector); every other atom combines a pseudo-random half of the sources
    /// (all arithmetic modulo 2^32):
    /// </para>
    /// <code>
    /// seed    = mix32((id * 0x9E3779B1) ^ crc ^ 0xA5A5A5A5)
    /// word[w] = mix32(seed + (w + 1) * 0x9E3779B9)
    /// </code>
    /// <para>
    /// Bits at or above <c>k</c> are cleared, and an all-zero mask is replaced by
    /// the single bit <c>id mod k</c>.
    /// </para>
    /// </remarks>
    /// <param name="k">Source atom count, at least one.</param>
    /// <param name="crc">Payload CRC-32C.</param>
    /// <param name="id">Encoded atom id.</param>
    /// <returns><see cref="MaskLength"/> words.</returns>
    /// <exception cref="ArgumentOutOfRangeException"><paramref name="k"/> is not positive.</exception>
    public static uint[] MaskWords(int k, uint crc, uint id)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(k);
        var mask = new uint[MaskLength(k)];
        if (id < (uint)k)
        {
            mask[id / 32] = 1u << (int)(id % 32);
            return mask;
        }

        unchecked
        {
            var seed = Mix32((id * 0x9E37_79B1u) ^ crc ^ 0xA5A5_A5A5u);
            for (var w = 0; w < mask.Length; w++)
                mask[w] = Mix32(seed + ((uint)w + 1) * 0x9E37_79B9u);
        }

        var tail = k % 32;
        if (tail != 0)
            mask[^1] &= (1u << tail) - 1;
        if (!mask.AsSpan().ContainsAnyExcept(0u))
        {
            var bit = (int)(id % (uint)k);
            mask[bit / 32] |= 1u << (bit % 32);
        }

        return mask;
    }

    /// <summary>Encodes atom <paramref name="id"/> from the source atoms.</summary>
    /// <param name="source">Source atoms, each 16 bytes.</param>
    /// <param name="crc">Payload CRC-32C.</param>
    /// <param name="id">Encoded atom id.</param>
    /// <returns>The 16-byte encoded atom.</returns>
    public static byte[] EncodeAtom(IReadOnlyList<byte[]> source, uint crc, uint id)
    {
        ArgumentNullException.ThrowIfNull(source);
        var output = new byte[PetalLanes.AtomLength];
        EncodeAtom(source, crc, id, output);
        return output;
    }

    /// <summary>Encodes atom <paramref name="id"/> into <paramref name="output"/>.</summary>
    internal static void EncodeAtom(IReadOnlyList<byte[]> source, uint crc, uint id, Span<byte> output)
    {
        var mask = MaskWords(source.Count, crc, id);
        output[..PetalLanes.AtomLength].Clear();
        for (var index = 0; index < source.Count; index++)
        {
            if (((mask[index / 32] >> (index % 32)) & 1) == 0)
                continue;
            var atom = source[index];
            for (var i = 0; i < PetalLanes.AtomLength; i++)
                output[i] ^= atom[i];
        }
    }
}

/// <summary>Incremental Gaussian-elimination decoder for the Petal fountain code.</summary>
public sealed class PetalFountainDecoder
{
    private readonly int k;
    private readonly int maskLength;
    private readonly int[] pivot;
    private readonly List<uint[]> rowMasks = [];
    private readonly List<byte[]> rowData = [];

    /// <summary>Creates a decoder for <paramref name="sourceAtoms"/> source atoms.</summary>
    /// <param name="sourceAtoms">Number of source atoms, at least one.</param>
    /// <exception cref="ArgumentOutOfRangeException"><paramref name="sourceAtoms"/> is not positive.</exception>
    public PetalFountainDecoder(int sourceAtoms)
    {
        ArgumentOutOfRangeException.ThrowIfNegativeOrZero(sourceAtoms);
        k = sourceAtoms;
        maskLength = PetalFountain.MaskLength(sourceAtoms);
        pivot = new int[sourceAtoms];
        Array.Fill(pivot, -1);
    }

    /// <summary>Number of source atoms.</summary>
    public int SourceAtoms => k;

    /// <summary>Number of linearly independent atoms received so far.</summary>
    public int Rank => rowMasks.Count;

    /// <summary>Whether enough independent atoms arrived to recover the payload.</summary>
    public bool IsComplete => rowMasks.Count == k;

    /// <summary>Adds encoded atom <paramref name="id"/>; returns whether it increased the rank.</summary>
    /// <param name="crc">Payload CRC-32C.</param>
    /// <param name="id">Encoded atom id.</param>
    /// <param name="atom">The 16-byte atom.</param>
    /// <returns><see langword="true"/> when the atom was independent of those already held.</returns>
    public bool AddEncoded(uint crc, uint id, ReadOnlySpan<byte> atom) =>
        Add(PetalFountain.MaskWords(k, crc, id), atom);

    /// <summary>Adds a received combination; returns whether it increased the rank.</summary>
    /// <param name="mask">Combination mask (<see cref="PetalFountain.MaskLength"/> words).</param>
    /// <param name="atom">The 16-byte combined atom.</param>
    /// <returns><see langword="true"/> when the combination was independent of those already held.</returns>
    /// <exception cref="ArgumentException">The atom is not 16 bytes.</exception>
    public bool Add(ReadOnlySpan<uint> mask, ReadOnlySpan<byte> atom)
    {
        if (atom.Length != PetalLanes.AtomLength)
            throw new ArgumentException("A fountain atom is 16 bytes.", nameof(atom));
        if (mask.Length != maskLength)
            return false;
        var reduced = mask.ToArray();
        var data = atom.ToArray();
        var word = 0;
        while (true)
        {
            while (word < reduced.Length && reduced[word] == 0)
                word++;
            if (word == reduced.Length)
                return false;
            var column = word * 32 + BitOperations.TrailingZeroCount(reduced[word]);
            if (column >= k)
                return false;
            var row = pivot[column];
            if (row < 0)
            {
                pivot[column] = rowMasks.Count;
                rowMasks.Add(reduced);
                rowData.Add(data);
                return true;
            }

            var pivotMask = rowMasks[row];
            for (var w = word; w < reduced.Length; w++)
                reduced[w] ^= pivotMask[w];
            var pivotData = rowData[row];
            for (var i = 0; i < data.Length; i++)
                data[i] ^= pivotData[i];
        }
    }

    /// <summary>Returns the source atoms once the decoder is complete.</summary>
    /// <returns>The <see cref="SourceAtoms"/> atoms, or <see langword="null"/> while incomplete.</returns>
    public byte[][]? Solve()
    {
        if (!IsComplete)
            return null;
        var solution = new byte[k][];
        for (var column = k - 1; column >= 0; column--)
        {
            var row = pivot[column];
            if (row < 0)
                return null;
            var value = (byte[])rowData[row].Clone();
            var mask = rowMasks[row];
            var firstWord = column / 32;
            for (var word = firstWord; word < mask.Length; word++)
            {
                var bits = mask[word];
                if (word == firstWord)
                {
                    // keep only columns strictly above the pivot
                    var shift = column % 32 + 1;
                    bits = shift >= 32 ? 0 : bits >> shift << shift;
                }

                while (bits != 0)
                {
                    var bit = BitOperations.TrailingZeroCount(bits);
                    bits &= bits - 1;
                    var other = solution[word * 32 + bit];
                    for (var i = 0; i < value.Length; i++)
                        value[i] ^= other[i];
                }
            }

            solution[column] = value;
        }

        return solution;
    }
}
