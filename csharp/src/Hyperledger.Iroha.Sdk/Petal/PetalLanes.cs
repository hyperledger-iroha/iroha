using System.Diagnostics.CodeAnalysis;

namespace Hyperledger.Iroha.Petal;

/// <summary>One of the three data lanes of a Petal frame.</summary>
public enum PetalLane
{
    /// <summary>Light/dark polarity of the 256 tiles (1 bit each, robust).</summary>
    P,

    /// <summary>Katakana glyph of each tile (4 bits each, the high-rate lane).</summary>
    K,

    /// <summary>Dots on the three rings (240 bits, the most robust lane).</summary>
    D,
}

/// <summary>
/// Lane codecs: bytes ⇄ transmitted cell states.
/// </summary>
/// <remarks>
/// <para>
/// Each lane is exactly one Reed–Solomon codeword, XOR-whitened with a fixed
/// xorshift32 sequence so the picture is statistically balanced whatever the
/// payload is:
/// </para>
/// <list type="table">
/// <listheader><term>lane</term><description>cells / codeword / data / parity</description></listheader>
/// <item><term>P</term><description>256 tiles × 1 bit / 32 B / 19 B / 13 B</description></item>
/// <item><term>K</term><description>256 tiles × 4 bits / 128 B / 83 B / 45 B</description></item>
/// <item><term>D</term><description>240 ring slots × 1 bit / 30 B / 19 B / 11 B</description></item>
/// </list>
/// <para>
/// Bit order is most-significant-bit first; lane <c>K</c> packs the first tile
/// of a pair into the high nibble.
/// </para>
/// </remarks>
public static class PetalLanes
{
    /// <summary>Length of a fountain atom in bytes.</summary>
    public const int AtomLength = 16;

    /// <summary>Bytes of the per-lane header (<c>tag</c>, <c>frame</c> high, <c>frame</c> low).</summary>
    public const int LaneHeaderLength = 3;

    /// <summary>Atoms carried by lane <c>P</c>.</summary>
    public const int PAtoms = 1;

    /// <summary>Atoms carried by lane <c>D</c> on frames that do not carry a beacon.</summary>
    public const int DAtoms = 1;

    /// <summary>Atoms carried by lane <c>K</c>.</summary>
    public const int KAtoms = 5;

    /// <summary>Most atoms one frame can carry (lanes <c>P</c>, <c>D</c> and <c>K</c>).</summary>
    public const int AtomsPerFrame = PAtoms + DAtoms + KAtoms;

    /// <summary>Codeword length of lane <c>P</c> in bytes.</summary>
    public const int PWordLength = PetalLayout.TileCount / 8;

    /// <summary>Codeword length of lane <c>K</c> in bytes.</summary>
    public const int KWordLength = PetalLayout.TileCount / 2;

    /// <summary>Codeword length of lane <c>D</c> in bytes.</summary>
    public const int DWordLength = PetalLayout.DBits / 8;

    /// <summary>Parity bytes of lane <c>P</c>.</summary>
    public const int PParityLength = 13;

    /// <summary>Parity bytes of lane <c>K</c>.</summary>
    public const int KParityLength = 45;

    /// <summary>Parity bytes of lane <c>D</c>.</summary>
    public const int DParityLength = 11;

    /// <summary>Data bytes of lane <c>P</c>.</summary>
    public const int PDataLength = PWordLength - PParityLength;

    /// <summary>Data bytes of lane <c>K</c>.</summary>
    public const int KDataLength = KWordLength - KParityLength;

    /// <summary>Data bytes of lane <c>D</c>.</summary>
    public const int DDataLength = DWordLength - DParityLength;

    private static readonly PetalLane[] DecodeOrderValues = [PetalLane.P, PetalLane.D, PetalLane.K];
    private static readonly PetalReedSolomon PCode = new(PParityLength);
    private static readonly PetalReedSolomon KCode = new(KParityLength);
    private static readonly PetalReedSolomon DCode = new(DParityLength);
    private static readonly byte[] PWhitening = BuildWhitening(PetalLane.P);
    private static readonly byte[] KWhitening = BuildWhitening(PetalLane.K);
    private static readonly byte[] DWhitening = BuildWhitening(PetalLane.D);

    /// <summary>All lanes in decode order (<c>P</c>, <c>D</c>, <c>K</c>).</summary>
    public static IReadOnlyList<PetalLane> DecodeOrder { get; } = Array.AsReadOnly(DecodeOrderValues);

    /// <summary>Codeword length of <paramref name="lane"/> in bytes.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>32, 128 or 30.</returns>
    public static int WordLength(PetalLane lane) => lane switch
    {
        PetalLane.P => PWordLength,
        PetalLane.K => KWordLength,
        PetalLane.D => DWordLength,
        _ => throw new ArgumentOutOfRangeException(nameof(lane)),
    };

    /// <summary>Parity bytes of <paramref name="lane"/>.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>13, 45 or 11.</returns>
    public static int ParityLength(PetalLane lane) => lane switch
    {
        PetalLane.P => PParityLength,
        PetalLane.K => KParityLength,
        PetalLane.D => DParityLength,
        _ => throw new ArgumentOutOfRangeException(nameof(lane)),
    };

    /// <summary>Data bytes of <paramref name="lane"/>.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>19, 83 or 19.</returns>
    public static int DataLength(PetalLane lane) => WordLength(lane) - ParityLength(lane);

    /// <summary>The fixed whitening sequence of <paramref name="lane"/>.</summary>
    /// <remarks>
    /// The top bytes of xorshift32 seeded with <c>"PETA"</c>, <c>"KANA"</c> or
    /// <c>"DOTS"</c> (big-endian ASCII).
    /// </remarks>
    /// <param name="lane">The lane.</param>
    /// <returns>A fresh copy of the <see cref="WordLength"/>-byte sequence.</returns>
    public static byte[] Whitening(PetalLane lane) => (byte[])WhiteningOf(lane).Clone();

    /// <summary>Encodes lane data into the transmitted (whitened) codeword.</summary>
    /// <param name="lane">The lane.</param>
    /// <param name="data">Exactly <see cref="DataLength"/> bytes.</param>
    /// <returns>The transmitted codeword.</returns>
    /// <exception cref="ArgumentException">The data length does not match the lane.</exception>
    public static byte[] EncodeLane(PetalLane lane, ReadOnlySpan<byte> data)
    {
        if (data.Length != DataLength(lane))
            throw new ArgumentException("Lane data length mismatch.", nameof(data));
        var word = new byte[WordLength(lane)];
        data.CopyTo(word);
        CodeOf(lane).EncodeParity(data, word.AsSpan(data.Length));
        var whitening = WhiteningOf(lane);
        for (var i = 0; i < word.Length; i++)
            word[i] ^= whitening[i];
        return word;
    }

    /// <summary>Decodes a transmitted codeword, returning the lane data bytes.</summary>
    /// <param name="lane">The lane.</param>
    /// <param name="transmitted">The received, still whitened codeword.</param>
    /// <param name="erasures">Byte positions the caller distrusts.</param>
    /// <param name="data">The lane data on success.</param>
    /// <returns>
    /// <see langword="false"/> when the word has the wrong length, the erasure
    /// list is invalid, or the word is uncorrectable.
    /// </returns>
    public static bool TryDecodeLane(
        PetalLane lane,
        ReadOnlySpan<byte> transmitted,
        ReadOnlySpan<int> erasures,
        [NotNullWhen(true)] out byte[]? data) =>
        TryDecodeLane(lane, transmitted, erasures, out data, out _);

    /// <summary>
    /// Decodes a transmitted codeword like <see cref="TryDecodeLane(PetalLane, ReadOnlySpan{byte}, ReadOnlySpan{int}, out byte[])"/>,
    /// also reporting how many byte positions the Reed–Solomon decoder rewrote.
    /// </summary>
    /// <param name="lane">The lane.</param>
    /// <param name="transmitted">The received, still whitened codeword.</param>
    /// <param name="erasures">Byte positions the caller distrusts.</param>
    /// <param name="data">The lane data on success.</param>
    /// <param name="corrected">
    /// The errata count on success: the erased positions plus the unflagged
    /// errors the decoder found (0 for a clean word).
    /// </param>
    /// <returns>
    /// <see langword="false"/> when the word has the wrong length, the erasure
    /// list is invalid, or the word is uncorrectable.
    /// </returns>
    public static bool TryDecodeLane(
        PetalLane lane,
        ReadOnlySpan<byte> transmitted,
        ReadOnlySpan<int> erasures,
        [NotNullWhen(true)] out byte[]? data,
        out int corrected)
    {
        data = null;
        corrected = 0;
        if (transmitted.Length != WordLength(lane))
            return false;
        Span<byte> word = stackalloc byte[transmitted.Length];
        var whitening = WhiteningOf(lane);
        for (var i = 0; i < word.Length; i++)
            word[i] = (byte)(transmitted[i] ^ whitening[i]);
        if (!CodeOf(lane).TryDecode(word, erasures, out corrected))
            return false;
        data = word[..DataLength(lane)].ToArray();
        return true;
    }

    private static PetalReedSolomon CodeOf(PetalLane lane) => lane switch
    {
        PetalLane.P => PCode,
        PetalLane.K => KCode,
        PetalLane.D => DCode,
        _ => throw new ArgumentOutOfRangeException(nameof(lane)),
    };

    private static byte[] WhiteningOf(PetalLane lane) => lane switch
    {
        PetalLane.P => PWhitening,
        PetalLane.K => KWhitening,
        PetalLane.D => DWhitening,
        _ => throw new ArgumentOutOfRangeException(nameof(lane)),
    };

    private static byte[] BuildWhitening(PetalLane lane)
    {
        var seed = lane switch
        {
            PetalLane.P => 0x5045_5441u, // "PETA"
            PetalLane.K => 0x4B41_4E41u, // "KANA"
            _ => 0x444F_5453u, // "DOTS"
        };
        var rng = new PetalXorshift32(seed);
        var sequence = new byte[WordLength(lane)];
        for (var i = 0; i < sequence.Length; i++)
            sequence[i] = rng.NextByte();
        return sequence;
    }
}

/// <summary>
/// Every cell of one frame: what a renderer draws and a decoder samples.
/// </summary>
public sealed class PetalFrameCells : IEquatable<PetalFrameCells>
{
    /// <summary>Creates an all-dark frame: dark tiles, glyph 0, unlit dots.</summary>
    public PetalFrameCells()
    {
        Light = new bool[PetalLayout.TileCount];
        Glyph = new byte[PetalLayout.TileCount];
        Dots = new bool[PetalLayout.TotalSlots];
    }

    /// <summary>Polarity of each tile; <see langword="true"/> is a light tile.</summary>
    public bool[] Light { get; }

    /// <summary>Glyph symbol (<c>0..16</c>) of each tile.</summary>
    public byte[] Glyph { get; }

    /// <summary>Lit state of every ring slot in flat order, gate dots included.</summary>
    public bool[] Dots { get; }

    /// <summary>Builds the cells from the three transmitted codewords.</summary>
    /// <param name="p">Lane <c>P</c> codeword (32 bytes).</param>
    /// <param name="k">Lane <c>K</c> codeword (128 bytes).</param>
    /// <param name="d">Lane <c>D</c> codeword (30 bytes).</param>
    /// <returns>The frame cells.</returns>
    /// <exception cref="ArgumentException">A codeword has the wrong length.</exception>
    public static PetalFrameCells FromWords(ReadOnlySpan<byte> p, ReadOnlySpan<byte> k, ReadOnlySpan<byte> d)
    {
        if (p.Length != PetalLanes.PWordLength)
            throw new ArgumentException("Lane P codeword must be 32 bytes.", nameof(p));
        if (k.Length != PetalLanes.KWordLength)
            throw new ArgumentException("Lane K codeword must be 128 bytes.", nameof(k));
        if (d.Length != PetalLanes.DWordLength)
            throw new ArgumentException("Lane D codeword must be 30 bytes.", nameof(d));
        var cells = new PetalFrameCells();
        for (var tile = 0; tile < PetalLayout.TileCount; tile++)
        {
            cells.Light[tile] = ((p[tile / 8] >> (7 - tile % 8)) & 1) == 1;
            var value = k[tile / 2];
            cells.Glyph[tile] = tile % 2 == 0 ? (byte)(value >> 4) : (byte)(value & 0x0F);
        }

        var roles = PetalLayout.SlotRoleTable;
        for (var slot = 0; slot < roles.Length; slot++)
        {
            var role = roles[slot];
            cells.Dots[slot] = role.Kind switch
            {
                PetalSlotKind.Gate => true,
                PetalSlotKind.Data => ((d[role.DataBit / 8] >> (7 - role.DataBit % 8)) & 1) == 1,
                _ => false,
            };
        }

        return cells;
    }

    /// <summary>Packs the polarity cells into a lane <c>P</c> codeword.</summary>
    /// <returns>32 bytes.</returns>
    public byte[] PWord()
    {
        var word = new byte[PetalLanes.PWordLength];
        for (var tile = 0; tile < PetalLayout.TileCount; tile++)
        {
            if (Light[tile])
                word[tile / 8] |= (byte)(1 << (7 - tile % 8));
        }

        return word;
    }

    /// <summary>Packs the glyph cells into a lane <c>K</c> codeword.</summary>
    /// <returns>128 bytes.</returns>
    public byte[] KWord()
    {
        var word = new byte[PetalLanes.KWordLength];
        for (var tile = 0; tile < PetalLayout.TileCount; tile++)
        {
            var nibble = Glyph[tile] & 0x0F;
            word[tile / 2] |= (byte)(tile % 2 == 0 ? nibble << 4 : nibble);
        }

        return word;
    }

    /// <summary>Packs the data dots into a lane <c>D</c> codeword.</summary>
    /// <returns>30 bytes.</returns>
    public byte[] DWord()
    {
        var word = new byte[PetalLanes.DWordLength];
        var slots = PetalLayout.DataSlotTable;
        for (var bit = 0; bit < slots.Length; bit++)
        {
            if (Dots[slots[bit]])
                word[bit / 8] |= (byte)(1 << (7 - bit % 8));
        }

        return word;
    }

    /// <summary>Deep copy of the cells.</summary>
    /// <returns>An independent copy.</returns>
    public PetalFrameCells Clone()
    {
        var copy = new PetalFrameCells();
        Light.CopyTo(copy.Light, 0);
        Glyph.CopyTo(copy.Glyph, 0);
        Dots.CopyTo(copy.Dots, 0);
        return copy;
    }

    /// <inheritdoc />
    public bool Equals(PetalFrameCells? other) => other is not null
        && Light.AsSpan().SequenceEqual(other.Light)
        && Glyph.AsSpan().SequenceEqual(other.Glyph)
        && Dots.AsSpan().SequenceEqual(other.Dots);

    /// <inheritdoc />
    public override bool Equals(object? obj) => Equals(obj as PetalFrameCells);

    /// <inheritdoc />
    public override int GetHashCode()
    {
        var hash = new HashCode();
        hash.AddBytes(Glyph);
        foreach (var light in Light)
            hash.Add(light);
        foreach (var dot in Dots)
            hash.Add(dot);
        return hash.ToHashCode();
    }
}
