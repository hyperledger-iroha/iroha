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
/// Normative Petal frame geometry.
/// </summary>
/// <remarks>
/// All coordinates are design units on a square canvas of <see cref="Canvas"/>
/// units with the origin at the top-left and <c>y</c> growing downward. A
/// renderer scales the canvas to any pixel size; a decoder maps camera pixels
/// back to design units. Cell order is defined by integer indices only.
/// </remarks>
public static class PetalLayout
{
    /// <summary>Canvas side length in design units.</summary>
    public const double Canvas = 1024.0;

    /// <summary>Canvas centre coordinate on both axes.</summary>
    public const double Center = 512.0;

    /// <summary>Number of tile lattice columns and rows.</summary>
    public const int TileGrid = 20;

    /// <summary>Canvas coordinate of the lattice's left and top edge.</summary>
    public const double TileOrigin = 222.0;

    /// <summary>Lattice pitch in design units.</summary>
    public const double TilePitch = 29.0;

    /// <summary>Side of a drawn tile in design units (pitch minus a 4-unit gutter).</summary>
    public const double TileSize = 25.0;

    /// <summary>Corner radius of a drawn tile in design units.</summary>
    public const double TileCornerRadius = 3.0;

    /// <summary>Side of the square glyph box inside a tile.</summary>
    public const double GlyphBox = 23.0;

    /// <summary>Number of data tiles in the <c>天</c> mask.</summary>
    public const int TileCount = 256;

    /// <summary>Number of concentric dot rings.</summary>
    public const int RingCount = 3;

    /// <summary>Radius of a drawn ring dot.</summary>
    public const double DotRadius = 11.0;

    /// <summary>Total dot slots over the three rings.</summary>
    public const int TotalSlots = 80 + 92 + 104;

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

    private static readonly int[] RingSlotCounts = [80, 92, 104];
    private static readonly float[] RingRadiiSingle = [360.0f, 410.0f, 460.0f];
    private static readonly double[] RingRadiiValues = [360.0, 410.0, 460.0];

    /// <summary>Dots per gate (right, bottom, left) and ring; there is deliberately no top gate.</summary>
    private static readonly int[,] GateDotCounts = { { 1, 1, 3 }, { 2, 2, 2 }, { 1, 2, 2 } };

    private static readonly PetalPoint[] FinderCenterValues =
    [
        new(72.0, 72.0),
        new(952.0, 72.0),
        new(952.0, 952.0),
        new(72.0, 952.0),
    ];

    private static readonly byte[] TileColumns = new byte[TileCount];
    private static readonly byte[] TileRows = new byte[TileCount];
    private static readonly PetalPoint[] TileCenters = new PetalPoint[TileCount];
    private static readonly PetalSlotRole[] Roles;
    private static readonly int[] DataSlotIndices;
    private static readonly int[] GateSlotIndices;
    private static readonly int[] GuardSlotIndices;
    private static readonly PetalPoint[] SlotCenters = new PetalPoint[TotalSlots];

    static PetalLayout()
    {
        var count = 0;
        for (var row = 0; row < TileGrid; row++)
        {
            for (var column = 0; column < TileGrid; column++)
            {
                if (MaskRows[row][column] != '#')
                    continue;
                TileColumns[count] = (byte)column;
                TileRows[count] = (byte)row;
                count++;
            }
        }

        if (count != TileCount)
            throw new InvalidOperationException("The Petal mask must contain exactly 256 tiles.");
        for (var tile = 0; tile < TileCount; tile++)
        {
            // Exact in binary floating point: multiples of 0.5 below 1024.
            TileCenters[tile] = new PetalPoint(
                TileOrigin + TilePitch * (TileColumns[tile] + 0.5),
                TileOrigin + TilePitch * (TileRows[tile] + 0.5));
        }

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

    /// <summary>The <c>天</c> silhouette, one string per lattice row from top to bottom; <c>#</c> marks a data tile.</summary>
    /// <remarks>
    /// The mask is mirror-symmetric left to right and not symmetric top to
    /// bottom, so it also tells a decoder which way is up.
    /// </remarks>
    public static IReadOnlyList<string> Mask { get; } = Array.AsReadOnly(MaskRows);

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

    /// <summary>Lattice <c>(Column, Row)</c> of data tile <paramref name="index"/> (row-major order).</summary>
    /// <param name="index">Tile index, 0 to 255.</param>
    /// <returns>The lattice coordinates.</returns>
    public static (int Column, int Row) Tile(int index)
    {
        CheckTile(index);
        return (TileColumns[index], TileRows[index]);
    }

    /// <summary>Canvas coordinates of the centre of tile <paramref name="index"/>.</summary>
    /// <param name="index">Tile index, 0 to 255.</param>
    /// <returns>The tile centre.</returns>
    public static PetalPoint TileCenter(int index)
    {
        CheckTile(index);
        return TileCenters[index];
    }

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

    /// <summary>Shared gate slot table.</summary>
    internal static ReadOnlySpan<int> GateSlotTable => GateSlotIndices;

    /// <summary>Shared guard slot table.</summary>
    internal static ReadOnlySpan<int> GuardSlotTable => GuardSlotIndices;

    /// <summary>Shared slot role table (no copy).</summary>
    internal static ReadOnlySpan<PetalSlotRole> SlotRoleTable => Roles;

    /// <summary>Shared lane <c>D</c> bit-to-slot table (no copy).</summary>
    internal static ReadOnlySpan<int> DataSlotTable => DataSlotIndices;

    /// <summary>Precomputed slot centre by flat index (decoder hot path).</summary>
    internal static PetalPoint SlotCenterFlat(int flat) => SlotCenters[flat];

    /// <summary>Precomputed tile centre (decoder hot path).</summary>
    internal static PetalPoint TileCenterUnchecked(int index) => TileCenters[index];

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

    private static void CheckTile(int index)
    {
        if ((uint)index >= TileCount)
            throw new ArgumentOutOfRangeException(nameof(index));
    }

    private static void CheckRing(int ring)
    {
        if ((uint)ring >= RingCount)
            throw new ArgumentOutOfRangeException(nameof(ring));
    }
}
