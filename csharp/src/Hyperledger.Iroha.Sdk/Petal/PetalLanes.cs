using System.Diagnostics.CodeAnalysis;

namespace Hyperledger.Iroha.Petal;

/// <summary>One of the five data lanes of a Petal frame, in decode order.</summary>
/// <remarks>
/// The reference names them <c>Lane::D</c> and <c>Lane::Field(sector)</c>;
/// <see cref="PetalLanes.Field"/> maps a sector to its lane.
/// </remarks>
public enum PetalLane
{
    /// <summary>Dots on the three rings (240 bits, the most robust lane; carries the beacon).</summary>
    D,

    /// <summary>The petals of field sector 0 (3 o'clock to 6 o'clock).</summary>
    F0,

    /// <summary>The petals of field sector 1 (6 o'clock to 9 o'clock).</summary>
    F1,

    /// <summary>The petals of field sector 2 (9 o'clock to 12 o'clock).</summary>
    F2,

    /// <summary>The petals of field sector 3 (12 o'clock to 3 o'clock).</summary>
    F3,
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
/// <item><term>D</term><description>240 ring slots × 1 bit / 30 B / 19 B / 11 B</description></item>
/// <item><term>F0–F3</term><description>840 field cells of one sector × 1 bit / 105 B / 83 B / 22 B</description></item>
/// </list>
/// <para>Bit order is most-significant-bit first.</para>
/// </remarks>
public static class PetalLanes
{
    /// <summary>Length of a fountain atom in bytes.</summary>
    public const int AtomLength = 16;

    /// <summary>Bytes of the per-lane header (<c>tag</c>, <c>frame</c> high, <c>frame</c> low).</summary>
    public const int LaneHeaderLength = 3;

    /// <summary>Atoms carried by lane <c>D</c> on frames that do not carry a beacon.</summary>
    public const int DAtoms = 1;

    /// <summary>Atoms carried by each field lane.</summary>
    public const int FieldAtoms = 5;

    /// <summary>Most atoms one frame can carry (the four field lanes and lane <c>D</c>).</summary>
    public const int AtomsPerFrame = PetalLayout.Sectors * FieldAtoms + DAtoms;

    /// <summary>Codeword length of lane <c>D</c> in bytes.</summary>
    public const int DWordLength = PetalLayout.DBits / 8;

    /// <summary>Parity bytes of lane <c>D</c>.</summary>
    public const int DParityLength = 11;

    /// <summary>Data bytes of lane <c>D</c>.</summary>
    public const int DDataLength = DWordLength - DParityLength;

    /// <summary>Codeword length of a field lane in bytes.</summary>
    public const int FieldWordLength = 105;

    /// <summary>Parity bytes of a field lane.</summary>
    public const int FieldParityLength = 22;

    /// <summary>Data bytes of a field lane.</summary>
    public const int FieldDataLength = FieldWordLength - FieldParityLength;

    /// <summary>Field cells of a sector that carry no bit (they stay dark).</summary>
    public const int SpareSectorCells = PetalLayout.SectorCells - FieldWordLength * 8;

    /// <summary>Cells of a sector that carry its codeword's bits.</summary>
    internal const int FieldBits = FieldWordLength * 8;

    private static readonly PetalLane[] DecodeOrderValues = [PetalLane.D, PetalLane.F0, PetalLane.F1, PetalLane.F2, PetalLane.F3];
    private static readonly PetalLane[] FieldValues = [PetalLane.F0, PetalLane.F1, PetalLane.F2, PetalLane.F3];
    private static readonly PetalReedSolomon DCode = new(DParityLength);
    private static readonly PetalReedSolomon FieldCode = new(FieldParityLength);
    private static readonly byte[][] Whitenings =
    [
        BuildWhitening(PetalLane.D),
        BuildWhitening(PetalLane.F0),
        BuildWhitening(PetalLane.F1),
        BuildWhitening(PetalLane.F2),
        BuildWhitening(PetalLane.F3),
    ];

    /// <summary>All lanes in decode order: <c>D</c>, then the field lanes in sector order.</summary>
    public static IReadOnlyList<PetalLane> DecodeOrder { get; } = Array.AsReadOnly(DecodeOrderValues);

    /// <summary>The four field lanes in sector order.</summary>
    public static IReadOnlyList<PetalLane> Fields { get; } = Array.AsReadOnly(FieldValues);

    /// <summary>The field lane of sector <paramref name="sector"/> (the reference's <c>Lane::Field(sector)</c>).</summary>
    /// <param name="sector">Sector, 0 to 3.</param>
    /// <returns><see cref="PetalLane.F0"/> to <see cref="PetalLane.F3"/>.</returns>
    public static PetalLane Field(int sector)
    {
        if ((uint)sector >= PetalLayout.Sectors)
            throw new ArgumentOutOfRangeException(nameof(sector));
        return PetalLane.F0 + sector;
    }

    /// <summary>Whether <paramref name="lane"/> is a field lane (<c>F0</c>–<c>F3</c>).</summary>
    /// <param name="lane">The lane.</param>
    /// <returns><see langword="false"/> for lane <c>D</c>.</returns>
    public static bool IsField(PetalLane lane) => CheckLane(lane) != PetalLane.D;

    /// <summary>The field sector of a field lane.</summary>
    /// <param name="lane">A field lane.</param>
    /// <returns>0 to 3.</returns>
    /// <exception cref="ArgumentOutOfRangeException">The lane is <c>D</c> or not a lane.</exception>
    public static int Sector(PetalLane lane)
    {
        if (!IsField(lane))
            throw new ArgumentOutOfRangeException(nameof(lane), "Lane D has no sector.");
        return lane - PetalLane.F0;
    }

    /// <summary>The letter that names the lane in reports: <c>D</c>, or the sector digit <c>0</c>–<c>3</c>.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>The letter.</returns>
    public static char Letter(PetalLane lane) => CheckLane(lane) == PetalLane.D ? 'D' : (char)('0' + (lane - PetalLane.F0));

    /// <summary>Codeword length of <paramref name="lane"/> in bytes.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>30 for lane <c>D</c>, 105 for a field lane.</returns>
    public static int WordLength(PetalLane lane) => IsField(lane) ? FieldWordLength : DWordLength;

    /// <summary>Parity bytes of <paramref name="lane"/>.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>11 for lane <c>D</c>, 22 for a field lane.</returns>
    public static int ParityLength(PetalLane lane) => IsField(lane) ? FieldParityLength : DParityLength;

    /// <summary>Data bytes of <paramref name="lane"/>.</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>19 for lane <c>D</c>, 83 for a field lane.</returns>
    public static int DataLength(PetalLane lane) => WordLength(lane) - ParityLength(lane);

    /// <summary>Atoms carried by <paramref name="lane"/> (lane <c>D</c>: on frames without a beacon).</summary>
    /// <param name="lane">The lane.</param>
    /// <returns>1 for lane <c>D</c>, 5 for a field lane.</returns>
    public static int Atoms(PetalLane lane) => IsField(lane) ? FieldAtoms : DAtoms;

    /// <summary>The fixed whitening sequence of <paramref name="lane"/>.</summary>
    /// <remarks>
    /// The top bytes of xorshift32 seeded with <c>"DOTS"</c> for lane <c>D</c> and
    /// <c>"PET0"</c> … <c>"PET3"</c> for the field lanes (big-endian ASCII).
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

    private static PetalLane CheckLane(PetalLane lane)
    {
        if ((uint)lane > (uint)PetalLane.F3)
            throw new ArgumentOutOfRangeException(nameof(lane));
        return lane;
    }

    private static PetalReedSolomon CodeOf(PetalLane lane) => IsField(lane) ? FieldCode : DCode;

    private static byte[] WhiteningOf(PetalLane lane) => Whitenings[(int)CheckLane(lane)];

    private static byte[] BuildWhitening(PetalLane lane)
    {
        var seed = lane == PetalLane.D
            ? 0x444F_5453u // "DOTS"
            : 0x5045_5430u + (uint)(lane - PetalLane.F0); // "PET0" … "PET3"
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
    /// <summary>Creates an all-dark frame: unlit dots and petals.</summary>
    public PetalFrameCells()
    {
        Dots = new bool[PetalLayout.TotalSlots];
        Petals = new bool[PetalLayout.FieldCells];
    }

    /// <summary>Lit state of every ring slot in flat order, gate dots included.</summary>
    public bool[] Dots { get; }

    /// <summary>Lit state of every field cell in cell order (spare cells are dark).</summary>
    public bool[] Petals { get; }

    /// <summary>Builds the cells from the transmitted codewords of a frame.</summary>
    /// <param name="words">The codewords (see <see cref="PetalStreamEncoder.Words"/>).</param>
    /// <returns>The frame cells.</returns>
    /// <exception cref="ArgumentException">A codeword has the wrong length.</exception>
    public static PetalFrameCells FromWords(PetalLaneData words)
    {
        ArgumentNullException.ThrowIfNull(words);
        return FromWords(words.D, words.Fields);
    }

    /// <summary>Builds the cells from the transmitted codewords of lane <c>D</c> and the four field lanes.</summary>
    /// <param name="d">Lane <c>D</c> codeword (30 bytes).</param>
    /// <param name="fields">The four field codewords (105 bytes each), sector 0 first.</param>
    /// <returns>The frame cells.</returns>
    /// <exception cref="ArgumentException">A codeword has the wrong length or there are not four field words.</exception>
    public static PetalFrameCells FromWords(ReadOnlySpan<byte> d, IReadOnlyList<byte[]> fields)
    {
        ArgumentNullException.ThrowIfNull(fields);
        if (d.Length != PetalLanes.DWordLength)
            throw new ArgumentException("Lane D codeword must be 30 bytes.", nameof(d));
        if (fields.Count != PetalLayout.Sectors)
            throw new ArgumentException("A frame has four field codewords.", nameof(fields));
        var cells = new PetalFrameCells();
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

        var order = PetalLayout.SectorOrderTable;
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            var word = fields[sector];
            if (word is null || word.Length != PetalLanes.FieldWordLength)
                throw new ArgumentException("A field codeword must be 105 bytes.", nameof(fields));
            var cellsOfSector = order.Slice(sector * PetalLayout.SectorCells, PetalLanes.FieldBits);
            for (var bit = 0; bit < cellsOfSector.Length; bit++)
                cells.Petals[cellsOfSector[bit]] = ((word[bit / 8] >> (7 - bit % 8)) & 1) == 1;
        }

        return cells;
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

    /// <summary>Packs the petals of sector <paramref name="sector"/> into its field codeword.</summary>
    /// <param name="sector">Sector, 0 to 3.</param>
    /// <returns>105 bytes.</returns>
    public byte[] FieldWord(int sector)
    {
        if ((uint)sector >= PetalLayout.Sectors)
            throw new ArgumentOutOfRangeException(nameof(sector));
        var word = new byte[PetalLanes.FieldWordLength];
        var cells = PetalLayout.SectorOrderTable.Slice(sector * PetalLayout.SectorCells, PetalLanes.FieldBits);
        for (var bit = 0; bit < cells.Length; bit++)
        {
            if (Petals[cells[bit]])
                word[bit / 8] |= (byte)(1 << (7 - bit % 8));
        }

        return word;
    }

    /// <summary>Deep copy of the cells.</summary>
    /// <returns>An independent copy.</returns>
    public PetalFrameCells Clone()
    {
        var copy = new PetalFrameCells();
        Dots.CopyTo(copy.Dots, 0);
        Petals.CopyTo(copy.Petals, 0);
        return copy;
    }

    /// <inheritdoc />
    public bool Equals(PetalFrameCells? other) => other is not null
        && Dots.AsSpan().SequenceEqual(other.Dots)
        && Petals.AsSpan().SequenceEqual(other.Petals);

    /// <inheritdoc />
    public override bool Equals(object? obj) => Equals(obj as PetalFrameCells);

    /// <inheritdoc />
    public override int GetHashCode()
    {
        var hash = new HashCode();
        foreach (var dot in Dots)
            hash.Add(dot);
        foreach (var petal in Petals)
            hash.Add(petal);
        return hash.ToHashCode();
    }
}
