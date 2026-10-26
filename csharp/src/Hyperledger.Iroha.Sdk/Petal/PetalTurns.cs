using System.Buffers.Binary;
using System.Diagnostics.CodeAnalysis;

namespace Hyperledger.Iroha.Petal;

/// <summary>One carrier petal and the turn a reader decided for it (see <see cref="PetalTurns.ObservedTurns"/>).</summary>
/// <param name="Cell">Field cell index, 0 to 3375.</param>
/// <param name="Position">The turn position decided for it, 0 to 3.</param>
/// <param name="Lead">How far that position's score led the runner-up's.</param>
public readonly record struct PetalObservedCarrier(int Cell, byte Position, double Lead);

/// <summary>The turns a reader decided for a capture, for diagnostics (see <see cref="PetalTurns.ObservedTurns"/>).</summary>
public sealed class PetalObservedTurns
{
    internal PetalObservedTurns(double blur, PetalObservedCarrier[] carriers)
    {
        Blur = blur;
        Carriers = Array.AsReadOnly(carriers);
    }

    /// <summary>The blur (Gaussian σ in canvas units) that fits the carriers best.</summary>
    public double Blur { get; }

    /// <summary>
    /// Every carrier of the decoded sectors, sector by sector in carrier order: its cell, the turn
    /// position decided for it, and how far that position's score led the runner-up's.
    /// </summary>
    public IReadOnlyList<PetalObservedCarrier> Carriers { get; }
}

/// <summary>
/// The turn layer: data in the way the lit petals point.
/// </summary>
/// <remarks>
/// <para>
/// A field lane tells the decoder exactly which petals of its sector are lit. The first
/// <see cref="TurnCarriers"/> lit petals of the sector, in the fixed pseudo-random
/// <see cref="CarrierOrder"/>, carry two more bits each: the petal is turned about its own centre
/// by 0°, 45°, 90° or 135° from its outward direction (<see cref="TurnAngle"/>). A turned petal
/// stays inside its own cell, so it moves no light toward its neighbours and the field lanes read
/// as before; a camera that resolves the petals' shapes also reads the sector's turn lane.
/// </para>
/// <para>
/// The turn lane of a sector is one Reed–Solomon codeword, RS(90, 68), XOR-whitened: four
/// fountain atoms from the repair range (ids from <see cref="TurnIdBase"/>, see
/// <see cref="TurnFirstId"/>) and a CRC-32C check that binds them to the stream tag, frame and
/// sector of the field lane they ride on (<see cref="TurnCheck"/>). Carrier <c>j</c> shows bits
/// <c>2j</c> and <c>2j + 1</c> of the transmitted codeword (most significant bit first) as a
/// Gray-coded position (<see cref="TurnPosition"/>). A sector with fewer than
/// <see cref="TurnCarriers"/> lit petals turns none (about one sector in 60,000).
/// </para>
/// <para>
/// <see cref="ReadTurns"/> reads the turn lanes of a decoded capture: every carrier's
/// neighbourhood is sampled in the petal's own outward frame, following a smooth correction of
/// the pose fitted to the carriers' centroids; the blur is the one under which the carriers and
/// their lit neighbours explain the image best; and each carrier takes the turn whose (slightly
/// wider) template matches best once its lit neighbours, at their own decided turns, are
/// subtracted. The reader repeats the reference's IEEE double operations in the same order; its
/// tables are built once, on first use, and its buffers are reused, so a read allocates next to
/// nothing.
/// </para>
/// </remarks>
public static class PetalTurns
{
    /// <summary>Turn positions a carrier petal can take.</summary>
    public const int TurnPositions = 4;

    /// <summary>Bits one carrier shows.</summary>
    public const int TurnBits = 2;

    /// <summary>Carrier petals of a sector.</summary>
    public const int TurnCarriers = 360;

    /// <summary>Codeword length of a turn lane in bytes.</summary>
    public const int TurnWordLength = TurnCarriers * TurnBits / 8;

    /// <summary>Parity bytes of a turn lane.</summary>
    public const int TurnParityLength = 22;

    /// <summary>Data bytes of a turn lane: the atoms, then their check.</summary>
    public const int TurnDataLength = TurnWordLength - TurnParityLength;

    /// <summary>Atoms carried by each turn lane.</summary>
    public const int TurnAtoms = 4;

    /// <summary>Bytes of the check that closes a turn lane's data.</summary>
    public const int TurnCheckLength = 4;

    /// <summary>
    /// Fountain id of the first turn atom of frame 0. The field lanes and lane <c>D</c> number their
    /// atoms from 0 and never come near it.
    /// </summary>
    public const uint TurnIdBase = 1u << 31;

    /// <summary>Bytes of the atoms of a turn lane.</summary>
    private const int AtomBytes = TurnAtoms * PetalLanes.AtomLength;

    /// <summary>Half the side, in canvas units, of the square a petal template is tabulated over.</summary>
    private const double TemplateHalf = 9.0;

    /// <summary>Template table step in canvas units.</summary>
    private const double TemplateStep = 0.25;

    /// <summary>Template table side in steps.</summary>
    private const int TemplateSide = 73;

    /// <summary>Cells of one template table.</summary>
    private const int TemplateCells = TemplateSide * TemplateSide;

    /// <summary>From this fitted blur on (3.2 units), the camera cannot resolve the turns and the reader stops.</summary>
    private const int BlurLimit = 4;

    /// <summary>The window of a carrier is sampled every canvas unit over ±<see cref="GridHalf"/> units in its own frame.</summary>
    private const int GridHalf = 6;

    /// <summary>Window side in samples.</summary>
    private const int GridSide = 2 * GridHalf + 1;

    /// <summary>Samples of a window.</summary>
    private const int GridPoints = GridSide * GridSide;

    /// <summary>σ in canvas units of the Gaussian that weights the window.</summary>
    private const double WindowSigma = 3.6;

    /// <summary>σ in canvas units of the Gaussian that follows a carrier's centroid.</summary>
    private const double CentroidSigma = 2.6;

    /// <summary>Lit petals within this distance of a carrier are modelled as its neighbours.</summary>
    private const double NeighbourDistance = 19.0;

    /// <summary>
    /// A neighbour's template is subtracted within this distance of its centre. Radially adjacent
    /// cell centres are 11 units apart and the window grid is integral, so a whole number here would
    /// put grid points exactly on the boundary, where the last bit of a platform's <c>sin</c> and
    /// <c>cos</c> decides.
    /// </summary>
    private const double NeighbourReach = 9.5;

    /// <summary>The warp and the blur are fitted on every this-many-th carrier.</summary>
    private const int SampleStep = 17;

    /// <summary>Carriers of four sectors at most.</summary>
    private const int MaxCarriers = PetalLayout.Sectors * TurnCarriers;

    /// <summary>Carriers the warp and the blur are fitted on, at most.</summary>
    private const int MaxSampled = (MaxCarriers + SampleStep - 1) / SampleStep;

    private static readonly double[] BlurValues = [1.0, 1.4, 1.9, 2.5, 3.2, 4.0];
    private static readonly PetalReedSolomon TurnCode = new(TurnParityLength);
    private static readonly int[] CarrierOrders = BuildCarrierOrders();
    private static readonly IReadOnlyList<int>[] CarrierOrderViews = BuildCarrierOrderViews();
    private static readonly byte[][] Whitenings = BuildWhitenings();
    private static readonly double[] TurnAngles = [TurnAngle(0), TurnAngle(1), TurnAngle(2), TurnAngle(3)];

    /// <summary>Gaussian blurs (σ in canvas units) the turn reader fits, sharpest first.</summary>
    public static IReadOnlyList<double> TurnBlurs { get; } = Array.AsReadOnly(BlurValues);

    /// <summary>
    /// The angle (radians, clockwise on screen) by which a petal at turn position
    /// <paramref name="position"/> is turned from its outward direction: <c>π · position / 4</c>.
    /// </summary>
    /// <param name="position">Turn position, 0 to 3.</param>
    /// <returns>The turn angle.</returns>
    public static double TurnAngle(byte position) => Math.PI * position / TurnPositions;

    /// <summary>
    /// The two bits a turn position shows: its Gray code <c>position ^ (position &gt;&gt; 1)</c>.
    /// Neighbouring positions (135° and 0° neighbour too, a petal turned by 180° differing only by
    /// its notch) differ in one bit.
    /// </summary>
    /// <param name="position">Turn position, 0 to 3.</param>
    /// <returns>The label.</returns>
    public static byte TurnLabel(byte position) => (byte)(position ^ (position >> 1));

    /// <summary>The turn position that shows the two bits <paramref name="label"/> (the inverse of <see cref="TurnLabel"/>).</summary>
    /// <param name="label">Two bits, 0 to 3.</param>
    /// <returns>The turn position.</returns>
    public static byte TurnPosition(byte label) => (byte)(label ^ (label >> 1));

    /// <summary>
    /// The order in which the lit petals of sector <paramref name="sector"/> become carriers: its
    /// codeword cells (the first <c>8 ×</c> <see cref="PetalLanes.FieldWordLength"/> of
    /// <see cref="PetalLayout.SectorCellOrder"/>) shuffled by Fisher–Yates with a
    /// <see cref="PetalXorshift32"/> seeded <c>0x4F524430 + sector</c> ("ORD0" … "ORD3"): for
    /// <c>i</c> from 839 down to 1, position <c>i</c> swaps with position
    /// <c>NextUInt32() mod (i + 1)</c>.
    /// </summary>
    /// <param name="sector">Sector, 0 to 3.</param>
    /// <returns>The 840 cells.</returns>
    public static IReadOnlyList<int> CarrierOrder(int sector)
    {
        CheckSector(sector);
        return CarrierOrderViews[sector];
    }

    /// <summary>
    /// The carriers of sector <paramref name="sector"/> under the lit map <paramref name="petals"/>
    /// (one flag per field cell): the first <see cref="TurnCarriers"/> lit cells in
    /// <see cref="CarrierOrder"/>.
    /// </summary>
    /// <param name="petals">Lit state of every field cell.</param>
    /// <param name="sector">Sector, 0 to 3.</param>
    /// <returns>The carrier cells, or <see langword="null"/> when fewer are lit.</returns>
    /// <exception cref="ArgumentException"><paramref name="petals"/> holds fewer than 3376 flags.</exception>
    public static int[]? Carriers(ReadOnlySpan<bool> petals, int sector)
    {
        CheckSector(sector);
        if (petals.Length < PetalLayout.FieldCells)
            throw new ArgumentException("The lit map holds one flag per field cell.", nameof(petals));
        var chosen = new int[TurnCarriers];
        return TryCarriers(petals, sector, chosen) ? chosen : null;
    }

    /// <summary>
    /// Fountain id of the first of the <see cref="TurnAtoms"/> atoms the turn lane of
    /// <paramref name="sector"/> carries in <paramref name="frame"/>:
    /// <c>2³¹ + 16 · frame + 4 · sector</c>.
    /// </summary>
    /// <param name="frame">Frame counter.</param>
    /// <param name="sector">Sector, 0 to 3.</param>
    /// <returns>The id.</returns>
    public static uint TurnFirstId(ushort frame, int sector)
    {
        CheckSector(sector);
        return TurnIdBase + (uint)frame * (PetalLayout.Sectors * TurnAtoms) + (uint)sector * TurnAtoms;
    }

    /// <summary>
    /// The check of a turn lane: CRC-32C over the stream tag, the frame (big-endian), the sector and
    /// the atoms, so a turn lane read under the wrong field lane, frame or stream is refused.
    /// </summary>
    /// <param name="header">The header of the field lane the turn lane rides on.</param>
    /// <param name="sector">Sector, 0 to 3.</param>
    /// <param name="atoms">The atoms.</param>
    /// <returns>The check.</returns>
    public static uint TurnCheck(PetalLaneHeader header, int sector, ReadOnlySpan<byte> atoms)
    {
        CheckSector(sector);
        var length = 4 + atoms.Length;
        Span<byte> bytes = length <= 256 ? stackalloc byte[length] : new byte[length];
        bytes[0] = header.Tag;
        BinaryPrimitives.WriteUInt16BigEndian(bytes[1..], header.Frame);
        bytes[3] = (byte)sector;
        atoms.CopyTo(bytes[4..]);
        return PetalCrc32C.Compute(bytes);
    }

    /// <summary>The data of a turn lane: the atoms followed by their <see cref="TurnCheck"/> (big-endian).</summary>
    /// <param name="header">The header of the field lane the turn lane rides on.</param>
    /// <param name="sector">Sector, 0 to 3.</param>
    /// <param name="atoms"><see cref="TurnAtoms"/> atoms, 64 bytes.</param>
    /// <returns><see cref="TurnDataLength"/> bytes.</returns>
    /// <exception cref="ArgumentException"><paramref name="atoms"/> is not 64 bytes.</exception>
    public static byte[] TurnData(PetalLaneHeader header, int sector, ReadOnlySpan<byte> atoms)
    {
        if (atoms.Length != AtomBytes)
            throw new ArgumentException("A turn lane carries four 16-byte atoms.", nameof(atoms));
        var data = new byte[TurnDataLength];
        atoms.CopyTo(data);
        BinaryPrimitives.WriteUInt32BigEndian(data.AsSpan(AtomBytes), TurnCheck(header, sector, atoms));
        return data;
    }

    /// <summary>The atoms of turn lane data whose check matches <paramref name="header"/> and <paramref name="sector"/>.</summary>
    /// <param name="header">The header of the field lane the turn lane was read on.</param>
    /// <param name="sector">Sector of that field lane.</param>
    /// <param name="data">Decoded turn lane data.</param>
    /// <returns>
    /// The atoms, or <see langword="null"/> for a wrong length, a sector out of range or a check
    /// that does not match.
    /// </returns>
    public static PetalAtomPacket? TurnPacket(PetalLaneHeader header, int sector, ReadOnlySpan<byte> data)
    {
        if (!Checks(header, sector, data))
            return null;
        var atoms = new byte[TurnAtoms][];
        for (var i = 0; i < atoms.Length; i++)
            atoms[i] = data.Slice(i * PetalLanes.AtomLength, PetalLanes.AtomLength).ToArray();
        return PetalAtomPacket.Owned(header, TurnFirstId(header.Frame, sector), atoms);
    }

    /// <summary>
    /// The whitening sequence of the turn lane of <paramref name="sector"/>: one byte per codeword
    /// byte from a <see cref="PetalXorshift32"/> seeded <c>0x54524E30 + sector</c> ("TRN0" … "TRN3").
    /// </summary>
    /// <param name="sector">Sector, 0 to 3.</param>
    /// <returns>A fresh copy of the 90-byte sequence.</returns>
    public static byte[] TurnWhitening(int sector) => (byte[])WhiteningOf(sector).Clone();

    /// <summary>Encodes turn lane data into the transmitted (whitened) codeword.</summary>
    /// <param name="sector">Sector, 0 to 3.</param>
    /// <param name="data">Exactly <see cref="TurnDataLength"/> bytes.</param>
    /// <returns>The transmitted 90-byte codeword.</returns>
    /// <exception cref="ArgumentException">The data is not 68 bytes.</exception>
    public static byte[] EncodeTurnLane(int sector, ReadOnlySpan<byte> data)
    {
        var whitening = WhiteningOf(sector);
        if (data.Length != TurnDataLength)
            throw new ArgumentException("Turn lane data length mismatch.", nameof(data));
        var word = new byte[TurnWordLength];
        data.CopyTo(word);
        TurnCode.EncodeParity(data, word.AsSpan(data.Length));
        for (var i = 0; i < word.Length; i++)
            word[i] ^= whitening[i];
        return word;
    }

    /// <summary>
    /// Decodes a transmitted turn codeword, also reporting how many byte positions the Reed–Solomon
    /// decoder rewrote. The check is not verified here (see <see cref="TurnPacket"/>).
    /// </summary>
    /// <param name="sector">Sector, 0 to 3.</param>
    /// <param name="transmitted">The received, still whitened codeword.</param>
    /// <param name="erasures">Byte positions the caller distrusts.</param>
    /// <param name="data">The turn lane data on success.</param>
    /// <param name="corrected">The errata count on success (0 for a clean word).</param>
    /// <returns>
    /// <see langword="false"/> when the word has the wrong length, the erasure list is invalid, or
    /// the word is uncorrectable.
    /// </returns>
    public static bool TryDecodeTurnLane(
        int sector,
        ReadOnlySpan<byte> transmitted,
        ReadOnlySpan<int> erasures,
        [NotNullWhen(true)] out byte[]? data,
        out int corrected)
    {
        var whitening = WhiteningOf(sector);
        data = null;
        corrected = 0;
        if (transmitted.Length != TurnWordLength)
            return false;
        Span<byte> word = stackalloc byte[TurnWordLength];
        for (var i = 0; i < word.Length; i++)
            word[i] = (byte)(transmitted[i] ^ whitening[i]);
        if (!TurnCode.TryDecode(word, erasures, out corrected))
            return false;
        data = word[..TurnDataLength].ToArray();
        return true;
    }

    /// <summary>The turn position carrier <paramref name="index"/> of a transmitted turn codeword shows.</summary>
    /// <param name="word">The transmitted codeword.</param>
    /// <param name="index">Carrier index, 0 to 359.</param>
    /// <returns>The turn position.</returns>
    public static byte CarrierPosition(ReadOnlySpan<byte> word, int index) =>
        TurnPosition((byte)((word[index / 4] >> (6 - 2 * (index % 4))) & 3));

    /// <summary>
    /// The turn reader's blurred petal template for blur <paramref name="blur"/> (an index into
    /// <see cref="TurnBlurs"/>) at a point of the unturned petal's own frame: what conformance tests
    /// compare an SDK's tables with.
    /// </summary>
    /// <param name="blur">Index into <see cref="TurnBlurs"/>.</param>
    /// <param name="along">Along the petal's axis, canvas units.</param>
    /// <param name="across">Across it, canvas units.</param>
    /// <returns>The template value (0 outside ±9 units).</returns>
    public static double TurnTemplate(int blur, double along, double across)
    {
        if ((uint)blur >= (uint)BlurValues.Length)
            throw new ArgumentOutOfRangeException(nameof(blur));
        return At(ReaderTables.Templates.AsSpan(blur * TemplateCells, TemplateCells), along, across);
    }

    /// <summary>
    /// Reads the turn lanes of the sectors <paramref name="frame"/> decoded from
    /// <paramref name="image"/>, except those in <paramref name="skip"/>.
    /// </summary>
    /// <remarks>
    /// The carriers of every decoded sector are decided (so that each carrier's neighbours are
    /// modelled at their own turns), but only the lanes of the sectors not skipped are decoded, and
    /// none when the image is too blurred to show turns. A lane passes when its Reed–Solomon
    /// codeword decodes, with 0, 2, 5, 7 or 11 of its least confident bytes erased (a byte's
    /// confidence is the smallest lead of its four carriers' turns), into data whose
    /// <see cref="TurnCheck"/> matches its sector's field lane header. Never throws on image
    /// content.
    /// </remarks>
    /// <param name="image">The luma plane <paramref name="frame"/> was decoded from.</param>
    /// <param name="frame">The decoded frame.</param>
    /// <param name="skip">Sectors not to read (four flags, sector 0 first); empty reads every sector.</param>
    /// <returns>The turn lane data (atoms and check) per sector, sector 0 first.</returns>
    /// <exception cref="ArgumentException"><paramref name="skip"/> is neither empty nor four flags long.</exception>
    public static PetalLaneResult?[] ReadTurns(PetalLuma image, PetalDecodedFrame frame, ReadOnlySpan<bool> skip = default)
    {
        ArgumentNullException.ThrowIfNull(image);
        ArgumentNullException.ThrowIfNull(frame);
        if (!skip.IsEmpty && skip.Length != PetalLayout.Sectors)
            throw new ArgumentException("Skip flags name the four sectors.", nameof(skip));
        Span<bool> need = stackalloc bool[PetalLayout.Sectors];
        for (var sector = 0; sector < need.Length; sector++)
            need[sector] = skip.IsEmpty || !skip[sector];
        var results = new PetalLaneResult?[PetalLayout.Sectors];
        var reader = Reader.Lease();
        try
        {
            if (reader.Decide(image, frame, need, limit: true) is null)
                return results;
            Span<byte> word = stackalloc byte[TurnWordLength];
            Span<double> confidence = stackalloc double[TurnWordLength];
            for (var sector = 0; sector < PetalLayout.Sectors; sector++)
            {
                var start = reader.RangeStarts[sector];
                if (!need[sector] || start < 0)
                    continue;
                word.Clear();
                confidence.Fill(double.MaxValue);
                for (var j = 0; j < TurnCarriers; j++)
                {
                    word[j / 4] |= (byte)(TurnLabel(reader.Positions[start + j]) << (6 - 2 * (j % 4)));
                    confidence[j / 4] = PetalMath.Min(confidence[j / 4], reader.Leads[start + j]);
                }

                results[sector] = DecodeTurnWord(sector, reader.Headers[sector], word, confidence);
            }
        }
        finally
        {
            Reader.Return(reader);
        }

        return results;
    }

    /// <summary>
    /// What <see cref="ReadTurns"/> decides for every carrier of the sectors <paramref name="frame"/>
    /// decoded, before any Reed–Solomon correction and whatever the blur.
    /// </summary>
    /// <param name="image">The luma plane <paramref name="frame"/> was decoded from.</param>
    /// <param name="frame">The decoded frame.</param>
    /// <returns>The decisions, or <see langword="null"/> when no decoded sector has carriers.</returns>
    public static PetalObservedTurns? ObservedTurns(PetalLuma image, PetalDecodedFrame frame)
    {
        ArgumentNullException.ThrowIfNull(image);
        ArgumentNullException.ThrowIfNull(frame);
        Span<bool> need = [true, true, true, true];
        var reader = Reader.Lease();
        try
        {
            if (reader.Decide(image, frame, need, limit: false) is not { } fitted)
                return null;
            var carriers = new PetalObservedCarrier[reader.Count];
            for (var index = 0; index < carriers.Length; index++)
                carriers[index] = new PetalObservedCarrier(reader.Cells[index], reader.Positions[index], reader.Leads[index]);
            return new PetalObservedTurns(BlurValues[fitted], carriers);
        }
        finally
        {
            Reader.Return(reader);
        }
    }

    /// <summary>The carrier order of every sector: sector <c>s</c> occupies <c>[s·840, (s+1)·840)</c>.</summary>
    internal static ReadOnlySpan<int> CarrierOrderTable(int sector) =>
        CarrierOrders.AsSpan(sector * PetalLanes.FieldBits, PetalLanes.FieldBits);

    /// <summary>
    /// Writes the carriers of <paramref name="sector"/> under <paramref name="petals"/> to
    /// <paramref name="chosen"/> (at least <see cref="TurnCarriers"/> long); <see langword="false"/>
    /// when fewer petals are lit.
    /// </summary>
    internal static bool TryCarriers(ReadOnlySpan<bool> petals, int sector, Span<int> chosen)
    {
        var count = 0;
        foreach (var cell in CarrierOrderTable(sector))
        {
            if (!petals[cell])
                continue;
            chosen[count++] = cell;
            if (count == TurnCarriers)
                return true;
        }

        return false;
    }

    /// <summary>Whether <paramref name="data"/> is turn lane data whose check matches <paramref name="header"/> and <paramref name="sector"/>.</summary>
    private static bool Checks(PetalLaneHeader header, int sector, ReadOnlySpan<byte> data) =>
        data.Length == TurnDataLength
        && (uint)sector < PetalLayout.Sectors
        && BinaryPrimitives.ReadUInt32BigEndian(data[AtomBytes..]) == TurnCheck(header, sector, data[..AtomBytes]);

    /// <summary>
    /// Tries Reed–Solomon with 0, 2, 5, 7 and 11 of the least confident bytes erased (ties keep byte
    /// order, a stable sort like Rust's <c>sort_by</c>) and accepts the first result whose check
    /// matches.
    /// </summary>
    private static PetalLaneResult? DecodeTurnWord(int sector, PetalLaneHeader header, ReadOnlySpan<byte> word, ReadOnlySpan<double> confidence)
    {
        const int nsym = TurnParityLength;
        Span<int> order = stackalloc int[TurnWordLength];
        for (var i = 0; i < order.Length; i++)
            order[i] = i;
        PetalMath.StableSortByKey(order, confidence);
        Span<int> schedule = [0, nsym / 8, nsym / 4, nsym / 3, nsym / 2];
        Span<byte> trial = stackalloc byte[TurnWordLength];
        var previous = -1;
        foreach (var erasures in schedule)
        {
            // Rust `dedup` drops consecutive repeats.
            if (erasures == previous)
                continue;
            previous = erasures;
            var positions = order[..erasures];
            word.CopyTo(trial);
            foreach (var position in positions)
                trial[position] = 0;
            if (TryDecodeTurnLane(sector, trial, positions, out var data, out var corrected) && Checks(header, sector, data))
                return new PetalLaneResult(data, corrected, positions.Length);
        }

        return null;
    }

    private static void CheckSector(int sector)
    {
        if ((uint)sector >= PetalLayout.Sectors)
            throw new ArgumentOutOfRangeException(nameof(sector));
    }

    private static byte[] WhiteningOf(int sector)
    {
        CheckSector(sector);
        return Whitenings[sector];
    }

    private static int[] BuildCarrierOrders()
    {
        var orders = new int[PetalLayout.Sectors * PetalLanes.FieldBits];
        for (var sector = 0; sector < PetalLayout.Sectors; sector++)
        {
            var order = orders.AsSpan(sector * PetalLanes.FieldBits, PetalLanes.FieldBits);
            PetalLayout.SectorOrderTable.Slice(sector * PetalLayout.SectorCells, PetalLanes.FieldBits).CopyTo(order);
            var rng = new PetalXorshift32(0x4F52_4430u + (uint)sector); // "ORD0" … "ORD3"
            for (var i = order.Length - 1; i >= 1; i--)
            {
                var j = (int)(rng.NextUInt32() % (uint)(i + 1));
                (order[i], order[j]) = (order[j], order[i]);
            }
        }

        return orders;
    }

    private static IReadOnlyList<int>[] BuildCarrierOrderViews()
    {
        var views = new IReadOnlyList<int>[PetalLayout.Sectors];
        for (var sector = 0; sector < views.Length; sector++)
            views[sector] = Array.AsReadOnly(CarrierOrderTable(sector).ToArray());
        return views;
    }

    private static byte[][] BuildWhitenings()
    {
        var whitenings = new byte[PetalLayout.Sectors][];
        for (var sector = 0; sector < whitenings.Length; sector++)
        {
            var rng = new PetalXorshift32(0x5452_4E30u + (uint)sector); // "TRN0" … "TRN3"
            var sequence = new byte[TurnWordLength];
            for (var i = 0; i < sequence.Length; i++)
                sequence[i] = rng.NextByte();
            whitenings[sector] = sequence;
        }

        return whitenings;
    }

    /// <summary>A template at a point of the petal's own frame (bilinear; zero outside the table).</summary>
    private static double At(ReadOnlySpan<double> table, double along, double across)
    {
        var x = (along + TemplateHalf) / TemplateStep;
        var y = (across + TemplateHalf) / TemplateStep;
        const double last = TemplateSide - 1;
        if (!(x >= 0.0 && y >= 0.0 && x < last && y < last))
            return 0.0;
        var (i, j) = ((int)x, (int)y);
        var (fx, fy) = (x - i, y - j);
        var row = j * TemplateSide + i;
        var top = table[row] * (1.0 - fx) + table[row + 1] * fx;
        var bottom = table[row + TemplateSide] * (1.0 - fx) + table[row + TemplateSide + 1] * fx;
        return top * (1.0 - fy) + bottom * fy;
    }

    /// <summary>
    /// Samples the image on the window grid of the petal at <paramref name="cell"/>, centred
    /// <c>(shiftX, shiftY)</c> canvas units off the cell centre, in the petal's outward frame (grid
    /// rows across, columns along). Within a window the pose is taken as affine: with <c>o</c> the
    /// image point of the centre and <c>u</c>, <c>v</c> the image steps of one unit along and
    /// across, grid point <c>(a, b)</c> is sampled at <c>(o + b·v) + a·u</c>.
    /// </summary>
    private static void SampleWindow(PetalLuma image, in PetalPose pose, int cell, double shiftX, double shiftY, Span<double> output)
    {
        var x = PetalLayout.CellXTable[cell] + shiftX;
        var y = PetalLayout.CellYTable[cell] + shiftY;
        var (sin, cos) = (PetalLayout.CellSineTable[cell], PetalLayout.CellCosineTable[cell]);
        pose.Apply(x, y, out var ox, out var oy);
        pose.Apply(x + cos, y + sin, out var alongX, out var alongY);
        pose.Apply(x - sin, y + cos, out var acrossX, out var acrossY);
        var (ux, uy) = (alongX - ox, alongY - oy);
        var (vx, vy) = (acrossX - ox, acrossY - oy);
        var index = 0;
        for (var b = -GridHalf; b <= GridHalf; b++)
        {
            var (rowX, rowY) = (ox + b * vx, oy + b * vy);
            for (var a = -GridHalf; a <= GridHalf; a++)
                output[index++] = image.Sample(rowX + a * ux, rowY + a * uy);
        }
    }

    /// <summary>
    /// Rescales a window so that its 10th percentile (of 169 samples, the 17th smallest) becomes 0
    /// and its 98th (the 166th smallest) 1 (a span below one grey level counts as one), clamped to
    /// −0.2 … 1.2.
    /// </summary>
    /// <remarks>
    /// The two order statistics are those of IEEE total order (Rust <c>f64::total_cmp</c>), as a
    /// selection or a sort finds them; two short sorted lists collect the 17 smallest and the 4
    /// largest samples.
    /// </remarks>
    private static void Normalise(Span<double> window)
    {
        const int lowCount = GridPoints / 10 + 1;
        const int highCount = GridPoints - GridPoints * 98 / 100;
        Span<long> low = stackalloc long[lowCount];
        Span<long> high = stackalloc long[highCount];
        var (lows, highs) = (0, 0);
        foreach (var value in window)
        {
            var key = TotalKey(value);
            if (lows < lowCount || key < low[lowCount - 1])
            {
                var slot = lows < lowCount ? lows++ : lowCount - 1;
                while (slot > 0 && low[slot - 1] > key)
                {
                    low[slot] = low[slot - 1];
                    slot--;
                }

                low[slot] = key;
            }

            if (highs < highCount)
            {
                var slot = highs++;
                while (slot > 0 && high[slot - 1] > key)
                {
                    high[slot] = high[slot - 1];
                    slot--;
                }

                high[slot] = key;
            }
            else if (key > high[0])
            {
                var slot = 0;
                while (slot + 1 < highCount && high[slot + 1] < key)
                {
                    high[slot] = high[slot + 1];
                    slot++;
                }

                high[slot] = key;
            }
        }

        var bottom = TotalValue(low[lowCount - 1]);
        var top = TotalValue(high[0]);
        var span = PetalMath.Max(top - bottom, 1.0);
        for (var i = 0; i < window.Length; i++)
            window[i] = Math.Clamp((window[i] - bottom) / span, -0.2, 1.2);
    }

    /// <summary>The key of <paramref name="value"/> under IEEE total order, as a signed integer.</summary>
    private static long TotalKey(double value)
    {
        var bits = BitConverter.DoubleToInt64Bits(value);
        return bits ^ (long)((ulong)(bits >> 63) >> 1);
    }

    /// <summary>The value of a <see cref="TotalKey"/> (the mapping is its own inverse).</summary>
    private static double TotalValue(long key) => BitConverter.Int64BitsToDouble(key ^ (long)((ulong)(key >> 63) >> 1));

    /// <summary>
    /// The centroid (along, across) of the light of a normalised window: every sample weighs its
    /// level (clamped to 0 … 1) times a Gaussian of σ <see cref="CentroidSigma"/> that starts at the
    /// window centre and follows the centroid three times; the Gaussian is its along factor times
    /// its across factor.
    /// </summary>
    private static (double Along, double Across) Centroid(ReadOnlySpan<double> window)
    {
        var (ea, eb) = (0.0, 0.0);
        Span<double> along = stackalloc double[GridSide];
        Span<double> across = stackalloc double[GridSide];
        const double k = 2.0 * CentroidSigma * CentroidSigma;
        for (var round = 0; round < 3; round++)
        {
            for (var i = 0; i < GridSide; i++)
            {
                double t = i - GridHalf;
                along[i] = Math.Exp(-(t - ea) * (t - ea) / k);
                across[i] = Math.Exp(-(t - eb) * (t - eb) / k);
            }

            var (sum, sa, sb) = (0.0, 0.0, 0.0);
            for (var j = 0; j < GridSide; j++)
            {
                double b = j - GridHalf;
                var row = window.Slice(j * GridSide, GridSide);
                for (var i = 0; i < GridSide; i++)
                {
                    double a = i - GridHalf;
                    var w = Math.Clamp(row[i], 0.0, 1.0) * along[i] * across[j];
                    sum += w;
                    sa += w * a;
                    sb += w * b;
                }
            }

            if (sum <= 0.0)
                break;
            ea = sa / sum;
            eb = sb / sum;
        }

        return (ea, eb);
    }

    /// <summary>The ten cubic monomials of a canvas point, centred on the canvas and scaled by 384.</summary>
    private static void Monomials(double x, double y, Span<double> f)
    {
        (x, y) = ((x - 512.0) / 384.0, (y - 512.0) / 384.0);
        f[0] = 1.0;
        f[1] = x;
        f[2] = y;
        f[3] = x * x;
        f[4] = x * y;
        f[5] = y * y;
        f[6] = x * x * x;
        f[7] = x * x * y;
        f[8] = x * y * y;
        f[9] = y * y * y;
    }

    /// <summary>
    /// The least-squares cubic through <paramref name="values"/> at the points
    /// <c>(xs, ys)</c>: normal equations (summed point by point) with 10⁻⁶ added to the diagonal,
    /// solved by Gauss–Jordan elimination with partial pivoting (a pivot below 10⁻¹² skips its
    /// column).
    /// </summary>
    private static void FitCubic(ReadOnlySpan<double> xs, ReadOnlySpan<double> ys, ReadOnlySpan<double> values, Span<double> coefficients)
    {
        const int width = 11;
        Span<double> a = stackalloc double[10 * width];
        Span<double> f = stackalloc double[10];
        Span<double> pivotRow = stackalloc double[width];
        a.Clear();
        for (var point = 0; point < values.Length; point++)
        {
            Monomials(xs[point], ys[point], f);
            var value = values[point];
            for (var r = 0; r < 10; r++)
            {
                for (var c = 0; c < 10; c++)
                    a[r * width + c] += f[r] * f[c];
                a[r * width + 10] += f[r] * value;
            }
        }

        for (var r = 0; r < 10; r++)
            a[r * width + r] += 1e-6;
        for (var col = 0; col < 10; col++)
        {
            var pivot = col;
            for (var r = col + 1; r < 10; r++)
            {
                if (Math.Abs(a[r * width + col]) > Math.Abs(a[pivot * width + col]))
                    pivot = r;
            }

            if (pivot != col)
            {
                for (var c = 0; c < width; c++)
                    (a[col * width + c], a[pivot * width + c]) = (a[pivot * width + c], a[col * width + c]);
            }

            var p = a[col * width + col];
            if (Math.Abs(p) < 1e-12)
                continue;
            for (var c = col; c < width; c++)
                a[col * width + c] /= p;
            a.Slice(col * width, width).CopyTo(pivotRow);
            for (var r = 0; r < 10; r++)
            {
                if (r == col)
                    continue;
                var factor = a[r * width + col];
                for (var c = col; c < width; c++)
                    a[r * width + c] -= factor * pivotRow[c];
            }
        }

        for (var r = 0; r < 10; r++)
            coefficients[r] = a[r * width + 10];
    }

    /// <summary>The cubic with <paramref name="coefficients"/> at a canvas point (summed from −0 like Rust's <c>Iterator::sum</c>).</summary>
    private static double EvalCubic(ReadOnlySpan<double> coefficients, double x, double y)
    {
        Span<double> f = stackalloc double[10];
        Monomials(x, y, f);
        var sum = PetalMath.SumIdentity;
        for (var i = 0; i < 10; i++)
            sum += f[i] * coefficients[i];
        return sum;
    }

    /// <summary>
    /// Adds <paramref name="sign"/> × the template of the neighbour at <c>(along, across)</c> in the
    /// window's frame, whose outward direction is <paramref name="angle"/> from the window's and which
    /// is turned by <paramref name="turn"/>, to a window, at the grid points within
    /// <see cref="NeighbourReach"/> of the neighbour's centre.
    /// </summary>
    private static void AddNeighbour(Span<double> window, ReadOnlySpan<double> template, double along, double across, double angle, double turn, double sign)
    {
        var axis = angle + turn;
        var (sin, cos) = (Math.Sin(axis), Math.Cos(axis));
        const double reach2 = NeighbourReach * NeighbourReach;
        // only the grid rows and columns within reach of the neighbour's centre
        var a0 = (int)PetalMath.Max(Math.Ceiling(along - NeighbourReach), -GridHalf);
        var a1 = (int)PetalMath.Min(Math.Floor(along + NeighbourReach), GridHalf);
        var b0 = (int)PetalMath.Max(Math.Ceiling(across - NeighbourReach), -GridHalf);
        var b1 = (int)PetalMath.Min(Math.Floor(across + NeighbourReach), GridHalf);
        for (var b = b0; b <= b1; b++)
        {
            var ub = b - across;
            var row = (b + GridHalf) * GridSide;
            for (var a = a0; a <= a1; a++)
            {
                var ua = a - along;
                if (ua * ua + ub * ub > reach2)
                    continue;
                window[row + a + GridHalf] += sign * At(template, ua * cos + ub * sin, -ua * sin + ub * cos);
            }
        }
    }

    /// <summary>
    /// The best turn position for a window and how far its score leads the runner-up's (ties: the
    /// lower position). A score is the sum of window × kernel in grid order, from −0 like Rust's
    /// <c>Iterator::sum</c>; the four are summed in one pass, each in its own accumulator.
    /// </summary>
    internal static (byte Position, double Lead) Decide(ReadOnlySpan<double> window, ReadOnlySpan<double> kernels)
    {
        var k0 = kernels[..GridPoints];
        var k1 = kernels.Slice(GridPoints, GridPoints);
        var k2 = kernels.Slice(2 * GridPoints, GridPoints);
        var k3 = kernels.Slice(3 * GridPoints, GridPoints);
        window = window[..GridPoints];
        double s0 = PetalMath.SumIdentity, s1 = PetalMath.SumIdentity, s2 = PetalMath.SumIdentity, s3 = PetalMath.SumIdentity;
        for (var g = 0; g < window.Length; g++)
        {
            var x = window[g];
            s0 += x * k0[g];
            s1 += x * k1[g];
            s2 += x * k2[g];
            s3 += x * k3[g];
        }

        var (best, bestScore) = (0, s0);
        if (s1 > bestScore)
            (best, bestScore) = (1, s1);
        if (s2 > bestScore)
            (best, bestScore) = (2, s2);
        if (s3 > bestScore)
            (best, bestScore) = (3, s3);
        // the runner-up: Rust `fold(f64::NEG_INFINITY, f64::max)` over the other positions in order
        var runnerUp = double.NegativeInfinity;
        if (best != 0)
            runnerUp = PetalMath.Max(runnerUp, s0);
        if (best != 1)
            runnerUp = PetalMath.Max(runnerUp, s1);
        if (best != 2)
            runnerUp = PetalMath.Max(runnerUp, s2);
        if (best != 3)
            runnerUp = PetalMath.Max(runnerUp, s3);
        return ((byte)best, bestScore - runnerUp);
    }

    /// <summary>
    /// The turn reader's tables, built once on first use (the type initializer runs exactly once,
    /// thread-safely): the window grid; per blur the template, the template turned to every turn
    /// position on the grid and the kernel (the same, weighted); and the neighbours of every field
    /// cell. About 0.9 MB.
    /// </summary>
    internal static class ReaderTables
    {
        /// <summary>The window Gaussian at every grid point (rows across, columns along).</summary>
        internal static readonly double[] GridWeights = new double[GridPoints];

        /// <summary>The template of every blur, <see cref="TemplateCells"/> values each, row <c>j</c> (across) by column <c>i</c> (along).</summary>
        internal static readonly double[] Templates = new double[BlurValues.Length * TemplateCells];

        /// <summary>Per blur and turn position, the template turned to that position on the grid, at <c>(blur·4 + position)·169 + point</c>.</summary>
        internal static readonly double[] Turned = new double[BlurValues.Length * TurnPositions * GridPoints];

        /// <summary>Per blur and turn position, the kernel: <see cref="Turned"/> weighted by <see cref="GridWeights"/>.</summary>
        internal static readonly double[] Kernels = new double[BlurValues.Length * TurnPositions * GridPoints];

        /// <summary>
        /// The cells within <see cref="NeighbourDistance"/> of every field cell (itself excluded), in
        /// index order: entries <c>[NearStarts[c], NearStarts[c + 1])</c>, with their centres in
        /// cell <c>c</c>'s outward frame and the angle of their outward direction from cell
        /// <c>c</c>'s.
        /// </summary>
        internal static readonly int[] NearStarts = new int[PetalLayout.FieldCells + 1];

        internal static readonly ushort[] NearCells;

        internal static readonly double[] NearAlong;

        internal static readonly double[] NearAcross;

        internal static readonly double[] NearAngles;

        static ReaderTables()
        {
            Span<double> gridA = stackalloc double[GridPoints];
            Span<double> gridB = stackalloc double[GridPoints];
            var point = 0;
            for (var b = -GridHalf; b <= GridHalf; b++)
            {
                for (var a = -GridHalf; a <= GridHalf; a++)
                {
                    gridA[point] = a;
                    gridB[point] = b;
                    GridWeights[point] = Math.Exp(-((double)a * a + (double)b * b) / (2.0 * WindowSigma * WindowSigma));
                    point++;
                }
            }

            for (var blur = 0; blur < BlurValues.Length; blur++)
            {
                var template = Templates.AsSpan(blur * TemplateCells, TemplateCells);
                BuildTemplate(BlurValues[blur], template);
                for (var position = 0; position < TurnPositions; position++)
                {
                    var angle = TurnAngle((byte)position);
                    var (sin, cos) = (Math.Sin(angle), Math.Cos(angle));
                    var at = (blur * TurnPositions + position) * GridPoints;
                    for (var g = 0; g < GridPoints; g++)
                    {
                        var (a, b) = (gridA[g], gridB[g]);
                        Turned[at + g] = At(template, a * cos + b * sin, -a * sin + b * cos);
                        Kernels[at + g] = GridWeights[g] * Turned[at + g];
                    }
                }
            }

            // bands are 11 units apart, so only the two bands on either side can hold neighbours
            var xs = PetalLayout.CellXTable;
            var ys = PetalLayout.CellYTable;
            var angles = PetalLayout.CellAngleTable;
            var bands = PetalLayout.CellBandTable;
            var offsets = PetalLayout.BandOffsetTable;
            var cells = new List<ushort>(PetalLayout.FieldCells * 8);
            var along = new List<double>(PetalLayout.FieldCells * 8);
            var across = new List<double>(PetalLayout.FieldCells * 8);
            var turns = new List<double>(PetalLayout.FieldCells * 8);
            for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
            {
                NearStarts[cell] = cells.Count;
                var (x, y) = (xs[cell], ys[cell]);
                var (sin, cos) = (PetalLayout.CellSineTable[cell], PetalLayout.CellCosineTable[cell]);
                var band = bands[cell];
                var first = offsets[Math.Max(band - 2, 0)];
                var last = offsets[Math.Min(band + 3, PetalLayout.BandCount)];
                for (var other = first; other < last; other++)
                {
                    if (other == cell)
                        continue;
                    var (dx, dy) = (xs[other] - x, ys[other] - y);
                    if (dx * dx + dy * dy > NeighbourDistance * NeighbourDistance)
                        continue;
                    cells.Add((ushort)other);
                    along.Add(dx * cos + dy * sin);
                    across.Add(-dx * sin + dy * cos);
                    turns.Add(angles[other] - angles[cell]);
                }
            }

            NearStarts[PetalLayout.FieldCells] = cells.Count;
            NearCells = cells.ToArray();
            NearAlong = along.ToArray();
            NearAcross = across.ToArray();
            NearAngles = turns.ToArray();
        }

        /// <summary>
        /// The coverage of every table cell by the unturned petal (4 × 4 samples per cell), blurred
        /// by a Gaussian of <paramref name="sigma"/> canvas units truncated at <c>3σ</c> (rows, then
        /// columns; taps outside the table count as zero, and the kernel is normalised by its full
        /// sum).
        /// </summary>
        private static void BuildTemplate(double sigma, Span<double> table)
        {
            const int side = TemplateSide;
            var cover = new double[TemplateCells];
            for (var j = 0; j < side; j++)
            {
                for (var i = 0; i < side; i++)
                {
                    var along = -TemplateHalf + i * TemplateStep;
                    var across = -TemplateHalf + j * TemplateStep;
                    var hits = 0;
                    for (var sj = 0; sj < 4; sj++)
                    {
                        for (var si = 0; si < 4; si++)
                        {
                            var da = (si - 1.5) * TemplateStep / 4.0;
                            var dc = (sj - 1.5) * TemplateStep / 4.0;
                            if (PetalLayout.PetalLit(along + da, across + dc, 0.0))
                                hits++;
                        }
                    }

                    cover[j * side + i] = hits / 16.0;
                }
            }

            var radius = (int)Math.Ceiling(3.0 * sigma / TemplateStep);
            var kernel = new double[2 * radius + 1];
            var norm = PetalMath.SumIdentity;
            for (var t = 0; t < kernel.Length; t++)
            {
                var step = (t - radius) * TemplateStep;
                kernel[t] = Math.Exp(-(step * step) / (2.0 * sigma * sigma));
            }

            foreach (var weight in kernel)
                norm += weight;
            var rows = new double[TemplateCells];
            Blur(cover, rows, kernel, radius, norm, alongRows: true);
            Blur(rows, table, kernel, radius, norm, alongRows: false);
        }

        private static void Blur(ReadOnlySpan<double> source, Span<double> output, ReadOnlySpan<double> kernel, int radius, double norm, bool alongRows)
        {
            const int side = TemplateSide;
            for (var j = 0; j < side; j++)
            {
                for (var i = 0; i < side; i++)
                {
                    var sum = 0.0;
                    for (var t = 0; t < kernel.Length; t++)
                    {
                        var k = t - radius;
                        var (ii, jj) = alongRows ? (i + k, j) : (i, j + k);
                        if (ii >= 0 && jj >= 0 && ii < side && jj < side)
                            sum += kernel[t] * source[jj * side + ii];
                    }

                    output[j * side + i] = sum / norm;
                }
            }
        }
    }

    /// <summary>
    /// The working memory of the reader, reused across captures so that a read allocates next to
    /// nothing: the windows of up to 1,440 carriers (about 2 MB) and the per-cell maps. One idle
    /// reader is kept; a reader is used by one thread at a time.
    /// </summary>
    private sealed class Reader
    {
        private static Reader? idle;

        /// <summary>Whether each field cell is lit, in the sectors that decoded.</summary>
        private readonly bool[] lit = new bool[PetalLayout.FieldCells];

        /// <summary>The carrier index of every field cell, −1 for cells that carry nothing.</summary>
        private readonly int[] carrierOf = new int[PetalLayout.FieldCells];

        /// <summary>The normalised window of every carrier, <see cref="GridPoints"/> values each.</summary>
        private readonly double[] windows = new double[MaxCarriers * GridPoints];

        /// <summary>Per carrier, its best turn position under the blur being fitted (−1 until decided).</summary>
        private readonly sbyte[] turnOf = new sbyte[MaxCarriers];

        private readonly double[] scratch = new double[GridPoints];
        private readonly double[] model = new double[GridPoints];
        private readonly double[] sampleX = new double[MaxSampled];
        private readonly double[] sampleY = new double[MaxSampled];
        private readonly double[] offsetX = new double[MaxSampled];
        private readonly double[] offsetY = new double[MaxSampled];

        /// <summary>The header of every decoded sector's field lane.</summary>
        public PetalLaneHeader[] Headers { get; } = new PetalLaneHeader[PetalLayout.Sectors];

        /// <summary>Where each sector's carriers start in <see cref="Cells"/>, −1 for a sector without carriers.</summary>
        public int[] RangeStarts { get; } = new int[PetalLayout.Sectors];

        /// <summary>The cell of every carrier, sector by sector in carrier order.</summary>
        public int[] Cells { get; } = new int[MaxCarriers];

        /// <summary>The turn position decided for every carrier.</summary>
        public byte[] Positions { get; } = new byte[MaxCarriers];

        /// <summary>How far each decided position's score led the runner-up's.</summary>
        public double[] Leads { get; } = new double[MaxCarriers];

        /// <summary>Carriers of every decoded sector.</summary>
        public int Count { get; private set; }

        public static Reader Lease() => Interlocked.Exchange(ref idle, null) ?? new Reader();

        public static void Return(Reader reader) => Volatile.Write(ref idle, reader);

        /// <summary>
        /// Decides the turn of every carrier of the sectors <paramref name="frame"/> decoded, when a
        /// sector that <paramref name="need"/> names has carriers; with <paramref name="limit"/>, not
        /// when the fitted blur reaches <see cref="BlurLimit"/>. Returns the fitted blur.
        /// </summary>
        /// <remarks>
        /// <list type="number">
        /// <item>The lit petals of the decoded sectors come from their field lanes, and the carriers of each from <see cref="Carriers"/>.</item>
        /// <item>The pose's smooth error: for every carrier in steps of <see cref="SampleStep"/>, the window is sampled at its cell centre and normalised, and its centroid turned into a canvas offset; a cubic fitted to the offsets over the canvas moves every carrier's window, which is then sampled and normalised.</item>
        /// <item>The blur: for every carrier in steps of <see cref="SampleStep"/>, the model of the petal at its best turn plus its lit neighbours (within <see cref="NeighbourDistance"/>) at theirs (each decided on its own window, unturned when it carries nothing) is correlated with the window, weighted by the window Gaussian; the blur with the largest sum fits best.</item>
        /// <item>The turns: with the next wider blur's kernels, every carrier takes the best turn of its own window, then, carrier by carrier in order, the best turn of its window with its lit neighbours' templates at their latest turns subtracted.</item>
        /// </list>
        /// </remarks>
        public int? Decide(PetalLuma image, PetalDecodedFrame frame, ReadOnlySpan<bool> need, bool limit)
        {
            // the lit petals of every sector that decoded
            Array.Clear(lit);
            Span<bool> decoded = stackalloc bool[PetalLayout.Sectors];
            for (var sector = 0; sector < PetalLayout.Sectors; sector++)
            {
                // a frame's field lanes are always 83 bytes; anything else names no lit petals
                if (frame.Fields[sector] is not { } field || field.Data.Length != PetalLanes.FieldDataLength)
                    continue;
                var data = field.Data;
                Headers[sector] = new PetalLaneHeader(data[0], (ushort)((data[1] << 8) | data[2]));
                decoded[sector] = true;
                var word = PetalLanes.EncodeLane(PetalLanes.Field(sector), data);
                var bits = PetalLayout.SectorOrderTable.Slice(sector * PetalLayout.SectorCells, PetalLanes.FieldBits);
                for (var bit = 0; bit < bits.Length; bit++)
                    lit[bits[bit]] = ((word[bit / 8] >> (7 - bit % 8)) & 1) == 1;
            }

            // the carriers of every decoded sector, sector by sector in carrier order
            var count = 0;
            var needed = false;
            for (var sector = 0; sector < PetalLayout.Sectors; sector++)
            {
                RangeStarts[sector] = -1;
                if (!decoded[sector] || !TryCarriers(lit, sector, Cells.AsSpan(count, TurnCarriers)))
                    continue;
                RangeStarts[sector] = count;
                count += TurnCarriers;
                needed |= need[sector];
            }

            Count = count;
            if (!needed)
                return null;
            Array.Fill(carrierOf, -1);
            for (var index = 0; index < count; index++)
                carrierOf[Cells[index]] = index;
            Sample(image, frame.Pose);
            var fitted = FitBlur();
            if (limit && fitted >= BlurLimit)
                return null;
            DecideTurns(Math.Min(fitted + 1, BlurValues.Length - 1));
            return fitted;
        }

        private Span<double> Window(int index) => windows.AsSpan(index * GridPoints, GridPoints);

        /// <summary>Samples, warps and normalises the windows of every carrier.</summary>
        private void Sample(PetalLuma image, in PetalPose pose)
        {
            // the pose's smooth error, from the centroids of a sample of the carriers
            var sampled = 0;
            for (var index = 0; index < Count; index += SampleStep)
            {
                var cell = Cells[index];
                SampleWindow(image, pose, cell, 0.0, 0.0, scratch);
                Normalise(scratch);
                var (a, b) = Centroid(scratch);
                var (sin, cos) = (PetalLayout.CellSineTable[cell], PetalLayout.CellCosineTable[cell]);
                sampleX[sampled] = PetalLayout.CellXTable[cell];
                sampleY[sampled] = PetalLayout.CellYTable[cell];
                offsetX[sampled] = a * cos - b * sin;
                offsetY[sampled] = a * sin + b * cos;
                sampled++;
            }

            Span<double> fitX = stackalloc double[10];
            Span<double> fitY = stackalloc double[10];
            FitCubic(sampleX.AsSpan(0, sampled), sampleY.AsSpan(0, sampled), offsetX.AsSpan(0, sampled), fitX);
            FitCubic(sampleX.AsSpan(0, sampled), sampleY.AsSpan(0, sampled), offsetY.AsSpan(0, sampled), fitY);
            for (var index = 0; index < Count; index++)
            {
                var cell = Cells[index];
                var (x, y) = (PetalLayout.CellXTable[cell], PetalLayout.CellYTable[cell]);
                var window = Window(index);
                SampleWindow(image, pose, cell, EvalCubic(fitX, x, y), EvalCubic(fitY, x, y), window);
                Normalise(window);
            }
        }

        /// <summary>
        /// The blur under which a sample of the carriers, each at its best turn with its lit
        /// neighbours at theirs, explains the windows best (ties keep the sharper).
        /// </summary>
        private int FitBlur()
        {
            var fitted = 0;
            var bestScore = double.NegativeInfinity;
            var weights = ReaderTables.GridWeights;
            for (var blur = 0; blur < BlurValues.Length; blur++)
            {
                var template = ReaderTables.Templates.AsSpan(blur * TemplateCells, TemplateCells);
                var kernels = ReaderTables.Kernels.AsSpan(blur * TurnPositions * GridPoints, TurnPositions * GridPoints);
                turnOf.AsSpan(0, Count).Fill(-1);
                var score = 0.0;
                for (var index = 0; index < Count; index += SampleStep)
                {
                    var own = TurnOf(index, kernels);
                    ReaderTables.Turned.AsSpan((blur * TurnPositions + own) * GridPoints, GridPoints).CopyTo(model);
                    var cell = Cells[index];
                    for (var near = ReaderTables.NearStarts[cell]; near < ReaderTables.NearStarts[cell + 1]; near++)
                    {
                        var other = ReaderTables.NearCells[near];
                        if (!lit[other])
                            continue;
                        var carrier = carrierOf[other];
                        var turn = carrier < 0 ? 0.0 : TurnAngles[TurnOf(carrier, kernels)];
                        AddNeighbour(model, template, ReaderTables.NearAlong[near], ReaderTables.NearAcross[near], ReaderTables.NearAngles[near], turn, 1.0);
                    }

                    var window = Window(index);
                    var (cross, mm, xx) = (0.0, 0.0, 0.0);
                    for (var g = 0; g < GridPoints; g++)
                    {
                        var (x, m, w) = (window[g], model[g], weights[g]);
                        cross += w * x * m;
                        mm += w * m * m;
                        xx += w * x * x;
                    }

                    score += cross / PetalMath.Max(Math.Sqrt(mm * xx), 1e-9);
                }

                if (score > bestScore)
                {
                    bestScore = score;
                    fitted = blur;
                }
            }

            return fitted;
        }

        /// <summary>The best turn position of carrier <paramref name="carrier"/>'s own window under <paramref name="kernels"/>, decided once per blur.</summary>
        private int TurnOf(int carrier, ReadOnlySpan<double> kernels)
        {
            if (turnOf[carrier] < 0)
                turnOf[carrier] = (sbyte)PetalTurns.Decide(Window(carrier), kernels).Position;
            return turnOf[carrier];
        }

        /// <summary>
        /// Every carrier takes the best turn of its own window under blur <paramref name="blur"/>;
        /// then, carrier by carrier in order, the best turn of its window minus its lit neighbours at
        /// their latest turns.
        /// </summary>
        private void DecideTurns(int blur)
        {
            var template = ReaderTables.Templates.AsSpan(blur * TemplateCells, TemplateCells);
            var kernels = ReaderTables.Kernels.AsSpan(blur * TurnPositions * GridPoints, TurnPositions * GridPoints);
            for (var index = 0; index < Count; index++)
                (Positions[index], Leads[index]) = PetalTurns.Decide(Window(index), kernels);
            for (var index = 0; index < Count; index++)
            {
                Window(index).CopyTo(scratch);
                var cell = Cells[index];
                for (var near = ReaderTables.NearStarts[cell]; near < ReaderTables.NearStarts[cell + 1]; near++)
                {
                    var other = ReaderTables.NearCells[near];
                    if (!lit[other])
                        continue;
                    var carrier = carrierOf[other];
                    var turn = carrier < 0 ? 0.0 : TurnAngles[Positions[carrier]];
                    AddNeighbour(scratch, template, ReaderTables.NearAlong[near], ReaderTables.NearAcross[near], ReaderTables.NearAngles[near], turn, -1.0);
                }

                (Positions[index], Leads[index]) = PetalTurns.Decide(scratch, kernels);
            }
        }
    }
}
