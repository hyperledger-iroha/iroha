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

/// <summary>
/// Normative Petal frame geometry (format 3).
/// </summary>
/// <remarks>
/// All coordinates are design units on a square canvas of <see cref="Canvas"/>
/// units with the origin at the top-left and <c>y</c> growing downward. A
/// renderer scales the canvas to any pixel size; a decoder maps camera pixels
/// back to design units. Nothing depends on floating-point rounding for bit
/// placement: cell order is defined by integer indices only, and the slot count
/// of every band is a fixed table.
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
