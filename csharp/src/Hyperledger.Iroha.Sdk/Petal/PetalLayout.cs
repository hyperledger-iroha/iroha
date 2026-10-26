namespace Hyperledger.Iroha.Petal;

/// <summary>A point in canvas design units or pixel-edge image coordinates.</summary>
/// <param name="X">Horizontal coordinate (grows to the right).</param>
/// <param name="Y">Vertical coordinate (grows downward).</param>
public readonly record struct PetalPoint(double X, double Y);

/// <summary>What a ring slot is used for.</summary>
public enum PetalSlotKind
{
    /// <summary>A gate dot, always lit.</summary>
    Gate,

    /// <summary>A slot next to a gate, always dark.</summary>
    Guard,

    /// <summary>Carries one data bit of lane <c>D</c>.</summary>
    Data,

    /// <summary>Unused data slot, always dark.</summary>
    Spare,
}

/// <summary>The role of one ring slot.</summary>
/// <param name="Kind">Slot kind.</param>
/// <param name="DataBit">Lane <c>D</c> bit index for <see cref="PetalSlotKind.Data"/>; otherwise <c>-1</c>.</param>
public readonly record struct PetalSlotRole(PetalSlotKind Kind, int DataBit);

/// <summary>One katakana tile of the card's bands (see <see cref="PetalLayout.KanaTiles"/>).</summary>
/// <param name="Band">Band: 0 above the square, 1 below it.</param>
/// <param name="Row">Row within its band, top first.</param>
/// <param name="Column">Column, left first.</param>
/// <param name="X">Left edge of the tile square in square coordinates.</param>
/// <param name="Y">Top edge of the tile square in square coordinates (negative above the square).</param>
/// <param name="Pilot">Index among the pilots of its band, or <see langword="null"/> for a data tile.</param>
public readonly record struct PetalKanaTile(int Band, int Row, int Column, double X, double Y, int? Pilot)
{
    /// <summary>Centre of the tile in square coordinates.</summary>
    public PetalPoint Center => new(X + PetalLayout.KanaTile / 2.0, Y + PetalLayout.KanaTile / 2.0);

    /// <summary>Whether the tile is a pilot with a fixed symbol.</summary>
    public bool IsPilot => Pilot is not null;
}

/// <summary>
/// Normative Petal frame geometry (format 5).
/// </summary>
/// <remarks>
/// <para>
/// All coordinates are design units in the square's coordinates: the square code spans
/// <see cref="Canvas"/> units on both axes with the origin at its top-left and <c>y</c> growing
/// downward. A frame is a <em>card</em> of <see cref="Canvas"/> × <see cref="CardHeight"/> units:
/// the square with a band of katakana tiles above it (<c>y</c> from −322 to 0) and below it
/// (<c>y</c> from 1024 to 1346), so card coordinates are square coordinates plus
/// <see cref="SquareTop"/> in <c>y</c>. A renderer scales the card to any pixel width; a decoder maps
/// camera pixels back to design units.
/// </para>
/// <para>
/// Nothing depends on floating-point rounding for bit placement: cell and tile order are defined by
/// integer indices only, and the slot count of every band of the petal field is a fixed table.
/// </para>
/// </remarks>
public static class PetalLayout
{
    /// <summary>Canvas side length in design units.</summary>
    public const double Canvas = 1024.0;

    /// <summary>Canvas centre coordinate on both axes.</summary>
    public const double Center = 512.0;

    /// <summary>Number of petal bands (concentric rings of field cells).</summary>
    public const int BandCount = 34;

    /// <summary>Radius of band 0.</summary>
    public const double BandInner = 16.0;

    /// <summary>Radial distance between consecutive bands.</summary>
    public const double BandSpacing = 11.0;

    /// <summary>Number of field cells.</summary>
    public const int FieldCells = 3376;

    /// <summary>Angular sectors of the field; each is one lane codeword.</summary>
    public const int Sectors = 4;

    /// <summary>Cells in each sector.</summary>
    public const int SectorCells = FieldCells / Sectors;

    /// <summary>
    /// Radius of the disc a petal stands in for: decoders sample within it, and no other petal,
    /// however it is turned, reaches into it.
    /// </summary>
    /// <remarks>
    /// The petal dimensions are single precision, as in the reference; sampling and rendering
    /// widen them exactly to double (4.6 becomes 4.599999904632568).
    /// </remarks>
    public const float PetalRadius = 4.6f;

    /// <summary>
    /// Half the length of a drawn petal, along its axis: the outward radial direction, turned by
    /// the petal's turn (see <see cref="PetalTurns"/>; single precision).
    /// </summary>
    public const float PetalHalfLength = 6.0f;

    /// <summary>Half the width of a drawn petal (single precision).</summary>
    public const float PetalHalfWidth = 3.73f;

    /// <summary>
    /// Radius of the notch cut into each petal tip; it tells a petal from one turned by half a
    /// turn (single precision).
    /// </summary>
    public const float PetalNotchRadius = 1.12f;

    /// <summary>Distance from a petal centre to its notch centre, along its axis (single precision).</summary>
    public const float PetalNotchDistance = 6.24f;

    /// <summary>Number of lattice columns and rows of the <c>天</c> mask.</summary>
    public const int MaskGrid = 20;

    /// <summary>Canvas coordinate of the mask lattice's left and top edge.</summary>
    public const double MaskOrigin = 182.0;

    /// <summary>Mask lattice pitch in design units.</summary>
    public const double MaskPitch = 33.0;

    /// <summary>Radius beyond which the <c>天</c> backdrop is not drawn.</summary>
    public const double BackdropRadius = 390.0;

    /// <summary>Number of concentric dot rings.</summary>
    public const int RingCount = 3;

    /// <summary>Radius of a drawn ring dot.</summary>
    public const double DotRadius = 9.0;

    /// <summary>Total dot slots over the three rings.</summary>
    public const int TotalSlots = 88 + 100 + 108;

    /// <summary>Number of lane <c>D</c> bits carried by the rings.</summary>
    public const int DBits = 240;

    /// <summary>Radius of the finder's solid centre disc.</summary>
    public const double FinderCore = 12.0;

    /// <summary>Number of petals of a finder blossom.</summary>
    public const int FinderPetals = 5;

    /// <summary>Distance from the finder centre to each petal centre.</summary>
    public const double FinderPetalDistance = 34.0;

    /// <summary>Radius of each petal.</summary>
    public const double FinderPetalRadius = 26.0;

    /// <summary>Radius of the notch cut into each petal tip.</summary>
    public const double FinderNotchRadius = 6.0;

    /// <summary>Outer radius of a finder blossom (tip of a petal).</summary>
    public const double FinderOuter = 60.0;

    /// <summary>Side of a katakana tile (see <see cref="PetalKana"/>).</summary>
    public const double KanaTile = 25.0;

    /// <summary>Pitch of the katakana tile lattice.</summary>
    public const double KanaPitch = 29.0;

    /// <summary>Side of a tile's glyph box, centred in the tile.</summary>
    public const double KanaGlyphBox = 23.0;

    /// <summary>Corner radius of a tile.</summary>
    public const double KanaCorner = 3.0;

    /// <summary>Tile rows of each band.</summary>
    public const int KanaRows = 10;

    /// <summary>Tile columns of each band.</summary>
    public const int KanaColumns = 35;

    /// <summary>Gap between a band's lattice and the square.</summary>
    public const double KanaGap = 24.0;

    /// <summary>Margin outside a band's lattice.</summary>
    public const double KanaMargin = 8.0;

    /// <summary>Height of a band with its gap and margin (322 units).</summary>
    public const double KanaBandHeight = KanaMargin + KanaRows * KanaPitch + KanaGap;

    /// <summary>Height of the card: the square with a band above and below it (1668 units).</summary>
    public const double CardHeight = Canvas + 2.0 * KanaBandHeight;

    /// <summary>Card <c>y</c> of the square's top edge: card coordinates are square coordinates plus this in <c>y</c>.</summary>
    public const double SquareTop = KanaBandHeight;

    /// <summary>
    /// Tile centres keep at least this distance from every finder centre: the blossom's radius, a
    /// blossom diameter of clearance, and a tile's half-diagonal, so no tile looks like part of a
    /// finder.
    /// </summary>
    public const double KanaFinderClearance = 198.0;

    /// <summary>Bands: above the square and below it.</summary>
    public const int KanaBands = 2;

    /// <summary>Pilot tiles of each band.</summary>
    public const int KanaPilots = 8;

    /// <summary>Tiles of each band: the lattice cells clear of the finders.</summary>
    public const int KanaBandTiles = 306;

    /// <summary>Tiles of both bands.</summary>
    public const int KanaTileCount = KanaBands * KanaBandTiles;

    /// <summary>Data tiles of each band (its tiles that are not pilots).</summary>
    internal const int KanaBandDataTiles = KanaBandTiles - KanaPilots;

    /// <summary>
    /// The pilot tiles of each band as <c>row, column</c> pairs in pilot order (band 1 mirrors band
    /// 0's rows, so the pilots sit at the same distances from the square).
    /// </summary>
    private static readonly int[][] KanaPilotCellValues =
    [
        [0, 0, 0, 17, 0, 34, 4, 0, 4, 34, 9, 8, 9, 17, 9, 26],
        [9, 0, 9, 17, 9, 34, 5, 0, 5, 34, 0, 8, 0, 17, 0, 26],
    ];

    /// <summary>What every pilot shows, in pilot order (ロ ケ ヒ ム ス ワ ア カ).</summary>
    private static readonly PetalKanaSymbol[] KanaPilotSymbolValues =
    [
        new(1, true),
        new(11, false),
        new(13, true),
        new(9, false),
        new(14, false),
        new(6, true),
        new(12, false),
        new(7, true),
    ];

    /// <summary>Every tile of both bands, band 0 first, each in reading order.</summary>
    private static readonly PetalKanaTile[] KanaTileValues = new PetalKanaTile[KanaTileCount];

    /// <summary>Left edge, top edge and pilot index (−1 for a data tile) of every tile (renderer and reader hot paths).</summary>
    private static readonly double[] TileXs = new double[KanaTileCount];

    private static readonly double[] TileYs = new double[KanaTileCount];

    private static readonly int[] TilePilots = new int[KanaTileCount];

    /// <summary>The data tiles of band <c>b</c> from <c>b · 298</c>, as tile indices.</summary>
    private static readonly int[] KanaDataTileValues = new int[KanaBands * KanaBandDataTiles];

    /// <summary>The pilot tiles of band <c>b</c> from <c>b · 8</c>, in pilot order, as tile indices.</summary>
    private static readonly int[] KanaPilotTileValues = new int[KanaBands * KanaPilots];

    private static readonly IReadOnlyList<int>[] KanaDataTileViews = new IReadOnlyList<int>[KanaBands];

    private static readonly IReadOnlyList<int>[] KanaPilotTileViews = new IReadOnlyList<int>[KanaBands];

    /// <summary>The tile of every lattice cell, <c>(band · 10 + row) · 35 + column</c>, or −1.</summary>
    private static readonly int[] KanaLattice = new int[KanaBands * KanaRows * KanaColumns];

    /// <summary>
    /// Cells on each band, inner band first: <c>4 · round(2π r / 50)</c> for the band radius
    /// <c>r</c>, about one cell every 12.5 units. Every count is a multiple of four, so the four
    /// sectors split each band evenly. The table, not the formula, is normative.
    /// </summary>
    private static readonly int[] BandSlotCounts =
    [
        8, 12, 20, 24, 32, 36, 40, 48, 52, 56, 64, 68, 76, 80, 84, 92, 96, 104, 108, 112, 120, 124,
        128, 136, 140, 148, 152, 156, 164, 168, 172, 180, 184, 192,
    ];

    private static readonly string[] MaskRows =
    [
        "....############....",
        "...##############...",
        "..################..",
        ".##################.",
        "###..............###",
        "###..............###",
        "#########..#########",
        "#########..#########",
        "###..............###",
        "###..............###",
        "########....########",
        "########....########",
        "#######......#######",
        "#######..##..#######",
        "######..####..######",
        "#####...####...#####",
        ".###...######...###.",
        "..###.########.###..",
        "......########......",
        ".....##########.....",
    ];

    private static readonly int[] RingSlotCounts = [88, 100, 108];
    private static readonly float[] RingRadiiSingle = [405.0f, 440.0f, 475.0f];
    private static readonly double[] RingRadiiValues = [405.0, 440.0, 475.0];

    /// <summary>Dots per gate (right, bottom, left) and ring; there is deliberately no top gate.</summary>
    private static readonly int[,] GateDotCounts = { { 1, 1, 3 }, { 2, 2, 2 }, { 1, 2, 2 } };

    private static readonly PetalPoint[] FinderCenterValues =
    [
        new(72.0, 72.0),
        new(952.0, 72.0),
        new(952.0, 952.0),
        new(72.0, 952.0),
    ];

    private static readonly int[] BandOffsetValues = new int[BandCount + 1];
    private static readonly int[] CellBands = new int[FieldCells];
    private static readonly int[] CellSlots = new int[FieldCells];
    private static readonly double[] CellAngles = new double[FieldCells];
    private static readonly double[] CellCosines = new double[FieldCells];
    private static readonly double[] CellSines = new double[FieldCells];
    private static readonly double[] CellXs = new double[FieldCells];
    private static readonly double[] CellYs = new double[FieldCells];
    private static readonly bool[] CellLights = new bool[FieldCells];
    private static readonly int[] SectorOrderValues = new int[FieldCells];
    private static readonly IReadOnlyList<int>[] SectorOrderViews = new IReadOnlyList<int>[Sectors];
    private static readonly PetalSlotRole[] Roles;
    private static readonly int[] DataSlotIndices;
    private static readonly int[] GateSlotIndices;
    private static readonly int[] GuardSlotIndices;
    private static readonly PetalPoint[] SlotCenters = new PetalPoint[TotalSlots];

    static PetalLayout()
    {
        for (var band = 0; band < BandCount; band++)
        {
            if (BandSlotCounts[band] % Sectors != 0)
                throw new InvalidOperationException("Band slot counts are multiples of four.");
            BandOffsetValues[band + 1] = BandOffsetValues[band] + BandSlotCounts[band];
        }

        if (BandOffsetValues[BandCount] != FieldCells)
            throw new InvalidOperationException("The band table must hold every field cell.");
        for (var band = 0; band < BandCount; band++)
        {
            double radius = BandRadius(band);
            for (var slot = 0; slot < BandSlotCounts[band]; slot++)
            {
                var cell = BandOffsetValues[band] + slot;
                CellBands[cell] = band;
                CellSlots[cell] = slot;
                // Exactly the reference `cell_angle` and `cell_center`, in double precision.
                var angle = Math.Tau * (slot + BandPhase(band)) / BandSlotCounts[band];
                CellAngles[cell] = angle;
                CellCosines[cell] = Math.Cos(angle);
                CellSines[cell] = Math.Sin(angle);
                CellXs[cell] = Center + radius * CellCosines[cell];
                CellYs[cell] = Center + radius * CellSines[cell];
                CellLights[cell] = InMask(CellXs[cell], CellYs[cell]);
            }
        }

        var at = 0;
        for (var sector = 0; sector < Sectors; sector++)
        {
            for (var band = 0; band < BandCount; band++)
            {
                var quarter = BandSlotCounts[band] / Sectors;
                for (var slot = sector * quarter; slot < (sector + 1) * quarter; slot++)
                    SectorOrderValues[at++] = BandOffsetValues[band] + slot;
            }

            SectorOrderViews[sector] = Array.AsReadOnly(SectorOrderValues[(sector * SectorCells)..((sector + 1) * SectorCells)]);
        }

        if (at != FieldCells)
            throw new InvalidOperationException("Every field cell belongs to exactly one sector.");
        Roles = BuildSlotRoles();
        DataSlotIndices = new int[DBits];
        var gates = new List<int>();
        var guards = new List<int>();
        for (var slot = 0; slot < Roles.Length; slot++)
        {
            switch (Roles[slot].Kind)
            {
                case PetalSlotKind.Data:
                    DataSlotIndices[Roles[slot].DataBit] = slot;
                    break;
                case PetalSlotKind.Gate:
                    gates.Add(slot);
                    break;
                case PetalSlotKind.Guard:
                    guards.Add(slot);
                    break;
            }
        }

        GateSlotIndices = gates.ToArray();
        GuardSlotIndices = guards.ToArray();
        for (var flat = 0; flat < TotalSlots; flat++)
        {
            var (ring, slot) = SplitSlot(flat);
            SlotCenters[flat] = ComputeSlotCenter(ring, slot);
        }

        BuildKanaTiles();
    }

    /// <summary>
    /// The <c>天</c> silhouette, one string per lattice row from top to bottom; <c>#</c> marks a
    /// lattice cell inside the character.
    /// </summary>
    /// <remarks>
    /// Petals whose centre lies in a <c>#</c> cell are drawn in the light colour, the others in
    /// pink, and the <c>#</c> cells inside <see cref="BackdropRadius"/> are drawn as a faint
    /// backdrop, so the character shows through the petals. The mask is mirror-symmetric left
    /// to right and not top to bottom.
    /// </remarks>
    public static IReadOnlyList<string> Mask { get; } = Array.AsReadOnly(MaskRows);

    /// <summary>Cells on each band, inner band first (the normative table).</summary>
    public static IReadOnlyList<int> BandSlots { get; } = Array.AsReadOnly(BandSlotCounts);

    /// <summary>Index of the first cell of every band, followed by <see cref="FieldCells"/>.</summary>
    public static IReadOnlyList<int> BandOffsets { get; } = Array.AsReadOnly(BandOffsetValues);

    /// <summary>Ring radii in design units, inner to outer.</summary>
    public static IReadOnlyList<double> RingRadii { get; } = Array.AsReadOnly(RingRadiiValues);

    /// <summary>Dot slots on each ring, all multiples of four so the cardinal gates sit exactly on a slot.</summary>
    public static IReadOnlyList<int> RingSlots { get; } = Array.AsReadOnly(RingSlotCounts);

    /// <summary>Canvas centres of the four corner finders, clockwise from the top-left.</summary>
    public static IReadOnlyList<PetalPoint> FinderCenters { get; } = Array.AsReadOnly(FinderCenterValues);

    /// <summary>Flat slot indices of the always-lit gate dots, ascending.</summary>
    public static IReadOnlyList<int> GateSlots => Array.AsReadOnly(GateSlotIndices);

    /// <summary>Flat slot indices of the always-dark guard slots, ascending.</summary>
    public static IReadOnlyList<int> GuardSlots => Array.AsReadOnly(GuardSlotIndices);

    /// <summary>Radius of band <paramref name="band"/>: <c>16 + 11·band</c>.</summary>
    /// <param name="band">Band index, 0 to 33.</param>
    /// <returns>The band radius in design units.</returns>
    public static double BandRadius(int band)
    {
        CheckBand(band);
        return BandInner + BandSpacing * band;
    }

    /// <summary>
    /// Fraction of a slot by which band <paramref name="band"/> is turned: odd bands sit half a
    /// slot over, so neighbouring bands interleave.
    /// </summary>
    /// <param name="band">Band index.</param>
    /// <returns><c>0.5</c> on odd bands, otherwise <c>0</c>.</returns>
    public static double BandPhase(int band) => band % 2 == 1 ? 0.5 : 0.0;

    /// <summary>Band and slot of field cell <paramref name="cell"/>.</summary>
    /// <param name="cell">Field cell index, 0 to 3375.</param>
    /// <returns>The band and the slot within it.</returns>
    public static (int Band, int Slot) CellBandSlot(int cell)
    {
        CheckCell(cell);
        return (CellBands[cell], CellSlots[cell]);
    }

    /// <summary>
    /// Angle (radians, clockwise on screen from 3 o'clock) of the centre of field cell
    /// <paramref name="cell"/>: <c>2π (slot + phase) / n</c>.
    /// </summary>
    /// <param name="cell">Field cell index.</param>
    /// <returns>The angle; also the outward direction of the cell's petal.</returns>
    public static double CellAngle(int cell)
    {
        CheckCell(cell);
        return CellAngles[cell];
    }

    /// <summary>Canvas coordinates of the centre of field cell <paramref name="cell"/>.</summary>
    /// <param name="cell">Field cell index.</param>
    /// <returns>The cell centre (double precision, as the reference computes it).</returns>
    public static PetalPoint CellCenter(int cell)
    {
        CheckCell(cell);
        return new PetalPoint(CellXs[cell], CellYs[cell]);
    }

    /// <summary>Whether field cell <paramref name="cell"/> lies inside the <c>天</c> (drawn in the light colour).</summary>
    /// <param name="cell">Field cell index.</param>
    /// <returns><see langword="true"/> for a light cell, <see langword="false"/> for a pink one.</returns>
    public static bool CellLight(int cell)
    {
        CheckCell(cell);
        return CellLights[cell];
    }

    /// <summary>
    /// The cells of sector <paramref name="sector"/> in bit order: band by band from the centre
    /// out, and within a band in ascending slot order.
    /// </summary>
    /// <remarks>
    /// Sector <c>s</c> holds slots <c>s·n/4 … (s+1)·n/4 − 1</c> of every band of <c>n</c> slots.
    /// The first <c>8 ×</c> <see cref="PetalLanes.FieldWordLength"/> cells carry the sector's
    /// codeword; the rest are spare and stay dark.
    /// </remarks>
    /// <param name="sector">Sector index, 0 to 3.</param>
    /// <returns>The <see cref="SectorCells"/> cell indices.</returns>
    public static IReadOnlyList<int> SectorCellOrder(int sector)
    {
        if ((uint)sector >= Sectors)
            throw new ArgumentOutOfRangeException(nameof(sector));
        return SectorOrderViews[sector];
    }

    /// <summary>Whether the canvas point <c>(x, y)</c> lies inside the <c>天</c> mask.</summary>
    /// <param name="x">Canvas x.</param>
    /// <param name="y">Canvas y.</param>
    /// <returns><see langword="true"/> when the point lies in a <c>#</c> lattice cell.</returns>
    public static bool InMask(double x, double y)
    {
        if (x < MaskOrigin || y < MaskOrigin)
            return false;
        var column = LatticeIndex((x - MaskOrigin) / MaskPitch);
        var row = LatticeIndex((y - MaskOrigin) / MaskPitch);
        return column < MaskGrid && row < MaskGrid && MaskRows[row][column] == '#';
    }

    /// <summary>
    /// Returns whether the point <c>(dx, dy)</c>, relative to the centre of a petal that points
    /// along <paramref name="angle"/>, is lit.
    /// </summary>
    /// <remarks>
    /// A petal is an ellipse of half-length <see cref="PetalHalfLength"/> along
    /// <paramref name="angle"/> and half-width <see cref="PetalHalfWidth"/>, minus the notch at its
    /// tip (a disc of radius <see cref="PetalNotchRadius"/> centred <see cref="PetalNotchDistance"/>
    /// along <paramref name="angle"/>).
    /// </remarks>
    /// <param name="dx">Horizontal offset from the petal centre.</param>
    /// <param name="dy">Vertical offset from the petal centre.</param>
    /// <param name="angle">
    /// The petal's axis (radians, clockwise from 3 o'clock): its outward direction plus its turn
    /// (<see cref="PetalTurns.TurnAngle"/>).
    /// </param>
    /// <returns><see langword="true"/> when the point is inside the petal.</returns>
    public static bool PetalLit(double dx, double dy, double angle) => PetalLit(dx, dy, Math.Sin(angle), Math.Cos(angle));

    /// <summary>Dots in gate <paramref name="gate"/> (0 right, 1 bottom, 2 left) on ring <paramref name="ring"/>.</summary>
    /// <param name="gate">Gate index.</param>
    /// <param name="ring">Ring index.</param>
    /// <returns>The number of lit gate dots.</returns>
    public static int GateDots(int gate, int ring)
    {
        if ((uint)gate >= 3)
            throw new ArgumentOutOfRangeException(nameof(gate));
        CheckRing(ring);
        return GateDotCounts[gate, ring];
    }

    /// <summary>Offset of ring <paramref name="ring"/> inside the flat slot index space.</summary>
    /// <param name="ring">Ring index.</param>
    /// <returns>The first flat index of the ring.</returns>
    public static int RingOffset(int ring) => ring switch
    {
        0 => 0,
        1 => RingSlotCounts[0],
        _ => RingSlotCounts[0] + RingSlotCounts[1],
    };

    /// <summary>The role of every ring slot in flat index order.</summary>
    /// <returns>A fresh array of <see cref="TotalSlots"/> roles.</returns>
    public static PetalSlotRole[] SlotRoles() => (PetalSlotRole[])Roles.Clone();

    /// <summary>Flat slot index of every lane <c>D</c> bit, in bit order.</summary>
    /// <returns>A fresh array of <see cref="DBits"/> slot indices.</returns>
    public static int[] DataSlots() => (int[])DataSlotIndices.Clone();

    /// <summary>Splits a flat slot index into <c>(Ring, Slot)</c>.</summary>
    /// <param name="flat">Flat slot index.</param>
    /// <returns>The ring and the slot within it.</returns>
    public static (int Ring, int Slot) SplitSlot(int flat)
    {
        if (flat < RingSlotCounts[0])
            return (0, flat);
        if (flat < RingSlotCounts[0] + RingSlotCounts[1])
            return (1, flat - RingSlotCounts[0]);
        return (2, flat - RingSlotCounts[0] - RingSlotCounts[1]);
    }

    /// <summary>Canvas coordinates of the centre of slot <paramref name="slot"/> on ring <paramref name="ring"/>.</summary>
    /// <remarks>
    /// Slot 0 is at 3 o'clock and slots advance clockwise on the screen. The
    /// reference computes this point in single precision; the returned doubles
    /// are those single-precision values widened exactly.
    /// </remarks>
    /// <param name="ring">Ring index.</param>
    /// <param name="slot">Slot within the ring.</param>
    /// <returns>The slot centre.</returns>
    public static PetalPoint SlotCenter(int ring, int slot)
    {
        CheckRing(ring);
        if ((uint)slot >= (uint)RingSlotCounts[ring])
            throw new ArgumentOutOfRangeException(nameof(slot));
        return SlotCenters[RingOffset(ring) + slot];
    }

    /// <summary>
    /// Returns whether the point <c>(dx, dy)</c>, relative to a finder centre, is lit.
    /// </summary>
    /// <remarks>
    /// A finder is a solid five-petal sakura blossom whose first petal points
    /// straight up. The petal notches are cosmetic; decoders only rely on the
    /// blossom being one large, isolated, roughly round blob.
    /// </remarks>
    /// <param name="dx">Horizontal offset from the finder centre.</param>
    /// <param name="dy">Vertical offset from the finder centre.</param>
    /// <returns><see langword="true"/> when the point is inside the blossom.</returns>
    public static bool FinderLit(double dx, double dy)
    {
        if (Math.Sqrt(dx * dx + dy * dy) <= FinderCore)
            return true;
        for (var petal = 0; petal < FinderPetals; petal++)
        {
            var angle = -(Math.PI / 2.0) + Math.Tau * petal / FinderPetals;
            var (cx, cy) = (FinderPetalDistance * Math.Cos(angle), FinderPetalDistance * Math.Sin(angle));
            if (Math.Sqrt((dx - cx) * (dx - cx) + (dy - cy) * (dy - cy)) <= FinderPetalRadius)
            {
                var (nx, ny) = (FinderOuter * Math.Cos(angle), FinderOuter * Math.Sin(angle));
                return Math.Sqrt((dx - nx) * (dx - nx) + (dy - ny) * (dy - ny)) > FinderNotchRadius;
            }
        }

        return false;
    }

    /// <summary>
    /// Every katakana tile of both bands: band 0 first, each in reading order (rows top first,
    /// columns left first).
    /// </summary>
    /// <remarks>
    /// Each band is a lattice of <see cref="KanaRows"/> × <see cref="KanaColumns"/> cells of pitch
    /// <see cref="KanaPitch"/>, centred on the card; a cell's tile is a <see cref="KanaTile"/>-unit
    /// square inset 2 units into the cell, at <c>(6.5 + 29c, −312 + 29r)</c> in band 0 and
    /// <c>(6.5 + 29c, 1050 + 29r)</c> in band 1. A cell whose tile centre lies within
    /// <see cref="KanaFinderClearance"/> of a finder centre has no tile, which leaves
    /// <see cref="KanaBandTiles"/> tiles in each band.
    /// </remarks>
    public static IReadOnlyList<PetalKanaTile> KanaTiles { get; } = Array.AsReadOnly(KanaTileValues);

    /// <summary>What every pilot shows, in pilot order (ロ ケ ヒ ム ス ワ ア カ): its glyph and polarity.</summary>
    public static IReadOnlyList<PetalKanaSymbol> KanaPilotSymbols { get; } = Array.AsReadOnly(KanaPilotSymbolValues);

    /// <summary>The lattice cell of pilot <paramref name="pilot"/> of band <paramref name="band"/>.</summary>
    /// <remarks>Band 1's pilots mirror band 0's rows, so the pilots sit at the same distances from the square.</remarks>
    /// <param name="band">Band, 0 or 1.</param>
    /// <param name="pilot">Pilot, 0 to 7.</param>
    /// <returns>The pilot's row and column.</returns>
    public static (int Row, int Column) KanaPilotCell(int band, int pilot)
    {
        CheckKanaBand(band);
        if ((uint)pilot >= KanaPilots)
            throw new ArgumentOutOfRangeException(nameof(pilot));
        return (KanaPilotCellValues[band][2 * pilot], KanaPilotCellValues[band][2 * pilot + 1]);
    }

    /// <summary>
    /// The data tiles of <paramref name="band"/> (its tiles that are not pilots) in reading order, as
    /// indices into <see cref="KanaTiles"/>: data tile <c>i</c> shows bit <c>i</c> of the band's
    /// polarity lane and nibble <c>i</c> of its glyph lane.
    /// </summary>
    /// <param name="band">Band, 0 or 1.</param>
    /// <returns>The 298 tile indices.</returns>
    public static IReadOnlyList<int> KanaDataTiles(int band)
    {
        CheckKanaBand(band);
        return KanaDataTileViews[band];
    }

    /// <summary>The pilot tiles of <paramref name="band"/> in pilot order, as indices into <see cref="KanaTiles"/>.</summary>
    /// <param name="band">Band, 0 or 1.</param>
    /// <returns>The eight tile indices.</returns>
    public static IReadOnlyList<int> KanaPilotTiles(int band)
    {
        CheckKanaBand(band);
        return KanaPilotTileViews[band];
    }

    /// <summary>
    /// The tile under the square-coordinate point <c>(x, y)</c> and the point's position inside the
    /// tile's square (0 to <see cref="KanaTile"/> on both axes from its top-left corner).
    /// </summary>
    /// <param name="x">Square x.</param>
    /// <param name="y">Square y (negative above the square, 1024 or more below it).</param>
    /// <returns>The tile index and the tile-local point, or <see langword="null"/> outside every tile square.</returns>
    public static (int Tile, double U, double V)? KanaTileAt(double x, double y) =>
        TryKanaTileAt(x, y, out var tile, out var u, out var v) ? (tile, u, v) : null;

    /// <summary>Whether the tile-local point <c>(u, v)</c> lies inside the rounded tile square (corner radius <see cref="KanaCorner"/>).</summary>
    /// <param name="u">Tile-local x, from the tile's left edge.</param>
    /// <param name="v">Tile-local y, from the tile's top edge.</param>
    /// <returns><see langword="true"/> inside the tile.</returns>
    public static bool KanaInside(double u, double v)
    {
        const double side = KanaTile;
        const double corner = KanaCorner;
        if (!(u >= 0.0 && u <= side) || !(v >= 0.0 && v <= side))
            return false;
        var cx = Math.Clamp(u, corner, side - corner);
        var cy = Math.Clamp(v, corner, side - corner);
        return (u - cx) * (u - cx) + (v - cy) * (v - cy) <= corner * corner;
    }

    /// <summary>
    /// Whether the tile-local point <c>(u, v)</c> lies on <paramref name="glyph"/>'s strokes: inside
    /// the tile square and inked in the glyph box (inset by half the difference of tile and box), so
    /// the design point <c>((u − 1)·32/23, (v − 1)·32/23)</c> is inked (<see cref="PetalGlyphs.IsInked"/>).
    /// </summary>
    /// <param name="glyph">Glyph, 0 to 15.</param>
    /// <param name="u">Tile-local x.</param>
    /// <param name="v">Tile-local y.</param>
    /// <returns><see langword="true"/> on the glyph's ink.</returns>
    public static bool KanaInked(int glyph, double u, double v)
    {
        if ((uint)glyph >= PetalGlyphs.GlyphCount)
            throw new ArgumentOutOfRangeException(nameof(glyph), "A glyph is 0 to 15.");
        return KanaInkedUnchecked(glyph, u, v);
    }

    /// <summary><see cref="KanaInked"/> without the range check.</summary>
    internal static bool KanaInkedUnchecked(int glyph, double u, double v)
    {
        const double inset = (KanaTile - KanaGlyphBox) / 2.0;
        const double scale = PetalGlyphs.GlyphGrid / KanaGlyphBox;
        return KanaInside(u, v) && PetalGlyphs.Inked(glyph, (u - inset) * scale, (v - inset) * scale);
    }

    /// <summary>
    /// <see cref="KanaTileAt"/> without allocating: <see langword="false"/> outside every tile square.
    /// A NaN coordinate is outside (the reference reaches the same verdict through a NaN offset).
    /// </summary>
    internal static bool TryKanaTileAt(double x, double y, out int tile, out double u, out double v)
    {
        (tile, u, v) = (-1, 0.0, 0.0);
        int band;
        if (y < 0.0)
            band = 0;
        else if (y >= Canvas)
            band = 1;
        else
            return false;
        var gx = (x - KanaLatticeLeft) / KanaPitch;
        var gy = (y - KanaLatticeTop(band)) / KanaPitch;
        if (!(gx >= 0.0) || !(gy >= 0.0))
            return false;
        var column = Math.Floor(gx);
        var row = Math.Floor(gy);
        if (column >= KanaColumns || row >= KanaRows)
            return false;
        var index = KanaLattice[(band * KanaRows + (int)row) * KanaColumns + (int)column];
        if (index < 0)
            return false;
        const double inset = (KanaPitch - KanaTile) / 2.0;
        var tu = (gx - column) * KanaPitch - inset;
        var tv = (gy - row) * KanaPitch - inset;
        if (!(tu >= 0.0 && tu <= KanaTile && tv >= 0.0 && tv <= KanaTile))
            return false;
        (tile, u, v) = (index, tu, tv);
        return true;
    }

    /// <summary>Left edge of every tile (square coordinates).</summary>
    internal static ReadOnlySpan<double> TileXTable => TileXs;

    /// <summary>Top edge of every tile (square coordinates).</summary>
    internal static ReadOnlySpan<double> TileYTable => TileYs;

    /// <summary>Pilot index of every tile, −1 for a data tile.</summary>
    internal static ReadOnlySpan<int> TilePilotTable => TilePilots;

    /// <summary>The data tiles of band <c>b</c> at <c>[b · 298, (b + 1) · 298)</c>.</summary>
    internal static ReadOnlySpan<int> KanaDataTileTable => KanaDataTileValues;

    /// <summary>The pilot tiles of band <c>b</c> at <c>[b · 8, (b + 1) · 8)</c>, in pilot order.</summary>
    internal static ReadOnlySpan<int> KanaPilotTileTable => KanaPilotTileValues;

    /// <summary>What every pilot shows (no copy).</summary>
    internal static ReadOnlySpan<PetalKanaSymbol> KanaPilotSymbolTable => KanaPilotSymbolValues;

    /// <summary>Left edge of the tile lattice: the columns are centred on the card (4.5 units).</summary>
    private const double KanaLatticeLeft = (Canvas - KanaColumns * KanaPitch) / 2.0;

    /// <summary>Top of band <paramref name="band"/>'s lattice in square coordinates.</summary>
    private static double KanaLatticeTop(int band) => band == 0 ? -KanaBandHeight + KanaMargin : Canvas + KanaGap;

    /// <summary>Lays out the tiles of both bands, exactly as the reference's <c>kana_tiles</c>.</summary>
    private static void BuildKanaTiles()
    {
        const double inset = (KanaPitch - KanaTile) / 2.0;
        const double half = KanaTile / 2.0;
        const double clearance = KanaFinderClearance;
        Array.Fill(KanaLattice, -1);
        var count = 0;
        for (var band = 0; band < KanaBands; band++)
        {
            var (data, pilots) = (0, 0);
            for (var row = 0; row < KanaRows; row++)
            {
                for (var column = 0; column < KanaColumns; column++)
                {
                    var x = KanaLatticeLeft + KanaPitch * column + inset;
                    var y = KanaLatticeTop(band) + KanaPitch * row + inset;
                    var (cx, cy) = (x + half, y + half);
                    var clear = true;
                    foreach (var finder in FinderCenterValues)
                    {
                        var (dx, dy) = (cx - finder.X, cy - finder.Y);
                        clear &= dx * dx + dy * dy >= clearance * clearance;
                    }

                    if (!clear)
                        continue;
                    int? pilot = null;
                    var cells = KanaPilotCellValues[band];
                    for (var p = 0; p < KanaPilots && pilot is null; p++)
                    {
                        if (cells[2 * p] == row && cells[2 * p + 1] == column)
                            pilot = p;
                    }

                    KanaTileValues[count] = new PetalKanaTile(band, row, column, x, y, pilot);
                    TileXs[count] = x;
                    TileYs[count] = y;
                    TilePilots[count] = pilot ?? -1;
                    KanaLattice[(band * KanaRows + row) * KanaColumns + column] = count;
                    if (pilot is { } index)
                    {
                        KanaPilotTileValues[band * KanaPilots + index] = count;
                        pilots++;
                    }
                    else
                    {
                        KanaDataTileValues[band * KanaBandDataTiles + data] = count;
                        data++;
                    }

                    count++;
                }
            }

            if (count != (band + 1) * KanaBandTiles || data != KanaBandDataTiles || pilots != KanaPilots)
                throw new InvalidOperationException("A band holds 306 tiles, eight of them pilots.");
            KanaDataTileViews[band] = Array.AsReadOnly(KanaDataTileValues[(band * KanaBandDataTiles)..((band + 1) * KanaBandDataTiles)]);
            KanaPilotTileViews[band] = Array.AsReadOnly(KanaPilotTileValues[(band * KanaPilots)..((band + 1) * KanaPilots)]);
        }
    }

    private static void CheckKanaBand(int band)
    {
        if ((uint)band >= KanaBands)
            throw new ArgumentOutOfRangeException(nameof(band), "A band is 0 (above the square) or 1 (below it).");
    }

    /// <summary>Shared finder centre table (no copy, no enumerator allocation).</summary>
    internal static ReadOnlySpan<PetalPoint> FinderCenterTable => FinderCenterValues;

    /// <summary>Shared ring radius table.</summary>
    internal static ReadOnlySpan<double> RingRadiusTable => RingRadiiValues;

    /// <summary>Shared ring slot count table.</summary>
    internal static ReadOnlySpan<int> RingSlotTable => RingSlotCounts;

    /// <summary>Shared band slot count table.</summary>
    internal static ReadOnlySpan<int> BandSlotTable => BandSlotCounts;

    /// <summary>Shared band offset table (<see cref="BandCount"/> + 1 entries).</summary>
    internal static ReadOnlySpan<int> BandOffsetTable => BandOffsetValues;

    /// <summary>Shared gate slot table.</summary>
    internal static ReadOnlySpan<int> GateSlotTable => GateSlotIndices;

    /// <summary>Shared guard slot table.</summary>
    internal static ReadOnlySpan<int> GuardSlotTable => GuardSlotIndices;

    /// <summary>Shared slot role table (no copy).</summary>
    internal static ReadOnlySpan<PetalSlotRole> SlotRoleTable => Roles;

    /// <summary>Shared lane <c>D</c> bit-to-slot table (no copy).</summary>
    internal static ReadOnlySpan<int> DataSlotTable => DataSlotIndices;

    /// <summary>Precomputed cell centres, angles, their cosines and sines, and light flags (decoder and renderer hot paths).</summary>
    internal static ReadOnlySpan<double> CellXTable => CellXs;

    internal static ReadOnlySpan<double> CellYTable => CellYs;

    internal static ReadOnlySpan<double> CellAngleTable => CellAngles;

    internal static ReadOnlySpan<double> CellCosineTable => CellCosines;

    internal static ReadOnlySpan<double> CellSineTable => CellSines;

    internal static ReadOnlySpan<bool> CellLightTable => CellLights;

    internal static ReadOnlySpan<int> CellBandTable => CellBands;

    /// <summary>The cells of every sector in bit order: sector <c>s</c> occupies <c>[s·844, (s+1)·844)</c>.</summary>
    internal static ReadOnlySpan<int> SectorOrderTable => SectorOrderValues;

    /// <summary>Precomputed slot centre by flat index (decoder hot path).</summary>
    internal static PetalPoint SlotCenterFlat(int flat) => SlotCenters[flat];

    /// <summary><see cref="PetalLit(double, double, double)"/> with the sine and cosine of the angle already taken.</summary>
    internal static bool PetalLit(double dx, double dy, double sin, double cos)
    {
        // The reference widens its single-precision petal constants to double.
        const double halfLength = PetalHalfLength;
        const double halfWidth = PetalHalfWidth;
        const double notch = PetalNotchDistance;
        const double notchRadius = PetalNotchRadius;
        var along = dx * cos + dy * sin;
        var across = -dx * sin + dy * cos;
        var (u, v) = (along / halfLength, across / halfWidth);
        if (u * u + v * v > 1.0)
            return false;
        var tip = along - notch;
        return tip * tip + across * across > notchRadius * notchRadius;
    }

    /// <summary>Gate dots then guard slots of one ring, as the reference's private <c>gate_slots</c>.</summary>
    internal static (List<int> Gates, List<int> Guards) GateSlotsOfRing(int ring)
    {
        var n = RingSlotCounts[ring];
        int[] bases = [0, n / 4, n / 2]; // right, bottom, left
        var gates = new List<int>();
        var guards = new List<int>();
        for (var gate = 0; gate < 3; gate++)
        {
            var count = GateDotCounts[gate, ring];
            var start = bases[gate];
            var (first, last) = count switch
            {
                1 => (start, start),
                2 => (start, start + 1),
                _ => (start + n - 1, start + 1),
            };
            var span = (last + n - first) % n + 1;
            for (var step = 0; step < span; step++)
                gates.Add((first + step) % n);
            guards.Add((first + n - 1) % n);
            guards.Add((last + 1) % n);
        }

        return (gates, guards);
    }

    /// <summary>Rust <c>value as usize</c> for a lattice coordinate, saturated at <see cref="MaskGrid"/>.</summary>
    private static int LatticeIndex(double value)
    {
        if (double.IsNaN(value))
            return 0;
        return value < MaskGrid ? (int)value : MaskGrid;
    }

    private static PetalSlotRole[] BuildSlotRoles()
    {
        var roles = new PetalSlotRole[TotalSlots];
        Array.Fill(roles, new PetalSlotRole(PetalSlotKind.Spare, -1));
        for (var ring = 0; ring < RingCount; ring++)
        {
            var (gates, guards) = GateSlotsOfRing(ring);
            var offset = RingOffset(ring);
            foreach (var slot in guards)
                roles[offset + slot] = new PetalSlotRole(PetalSlotKind.Guard, -1);
            foreach (var slot in gates)
                roles[offset + slot] = new PetalSlotRole(PetalSlotKind.Gate, -1);
        }

        var next = 0;
        for (var i = 0; i < roles.Length; i++)
        {
            if (roles[i].Kind == PetalSlotKind.Spare && next < DBits)
            {
                roles[i] = new PetalSlotRole(PetalSlotKind.Data, next);
                next++;
            }
        }

        return roles;
    }

    private static PetalPoint ComputeSlotCenter(int ring, int slot)
    {
        // Single precision, exactly as the reference `slot_center`.
        var theta = MathF.Tau * slot / RingSlotCounts[ring];
        var x = 512.0f + RingRadiiSingle[ring] * MathF.Cos(theta);
        var y = 512.0f + RingRadiiSingle[ring] * MathF.Sin(theta);
        return new PetalPoint(x, y);
    }

    private static void CheckBand(int band)
    {
        if ((uint)band >= BandCount)
            throw new ArgumentOutOfRangeException(nameof(band));
    }

    private static void CheckCell(int cell)
    {
        if ((uint)cell >= FieldCells)
            throw new ArgumentOutOfRangeException(nameof(cell));
    }

    private static void CheckRing(int ring)
    {
        if ((uint)ring >= RingCount)
            throw new ArgumentOutOfRangeException(nameof(ring));
    }
}
