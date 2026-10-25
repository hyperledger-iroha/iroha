namespace Hyperledger.Iroha.Petal;

/// <summary>
/// A vector 2D surface that can replay a <see cref="PetalDrawList"/>: SkiaSharp,
/// .NET MAUI <c>ICanvas</c>, WPF/Avalonia <c>DrawingContext</c>, Win2D, Core
/// Graphics or an Android <c>Canvas</c> adapter implements these five calls.
/// </summary>
/// <remarks>
/// Coordinates arrive already scaled to the caller's target size. Shapes are
/// filled without stroke. Anti-aliasing is up to the backend.
/// </remarks>
public interface IPetalCanvas
{
    /// <summary>Fills an axis-aligned rectangle.</summary>
    /// <param name="x">Left edge.</param>
    /// <param name="y">Top edge.</param>
    /// <param name="width">Width.</param>
    /// <param name="height">Height.</param>
    /// <param name="color">Fill colour.</param>
    void FillRectangle(double x, double y, double width, double height, PetalColor color);

    /// <summary>Fills a circle.</summary>
    /// <param name="centerX">Centre x.</param>
    /// <param name="centerY">Centre y.</param>
    /// <param name="radius">Radius.</param>
    /// <param name="color">Fill colour.</param>
    void FillCircle(double centerX, double centerY, double radius, PetalColor color);

    /// <summary>Fills an ellipse turned by <paramref name="angle"/>.</summary>
    /// <param name="centerX">Centre x.</param>
    /// <param name="centerY">Centre y.</param>
    /// <param name="radiusAlong">Half-length along the direction <paramref name="angle"/>.</param>
    /// <param name="radiusAcross">Half-width across that direction.</param>
    /// <param name="angle">Direction of the long axis: radians, clockwise on the screen from 3 o'clock.</param>
    /// <param name="color">Fill colour.</param>
    void FillEllipse(double centerX, double centerY, double radiusAlong, double radiusAcross, double angle, PetalColor color);

    /// <summary>Intersects the clip region with a circle until the matching <see cref="PopClip"/>.</summary>
    /// <param name="centerX">Centre x.</param>
    /// <param name="centerY">Centre y.</param>
    /// <param name="radius">Radius.</param>
    void PushCircleClip(double centerX, double centerY, double radius);

    /// <summary>Restores the clip region saved by the last <see cref="PushCircleClip"/>.</summary>
    void PopClip();
}

/// <summary>One corner finder: a solid five-petal sakura blossom.</summary>
/// <remarks>
/// Draw the core disc and the five petal circles in the light colour, then the
/// five notch circles in the background colour.
/// </remarks>
public sealed class PetalFinderShape
{
    internal PetalFinderShape(PetalPoint center, PetalPoint[] petals, PetalPoint[] notches)
    {
        Center = center;
        PetalCenters = Array.AsReadOnly(petals);
        NotchCenters = Array.AsReadOnly(notches);
    }

    /// <summary>Blossom centre in canvas units.</summary>
    public PetalPoint Center { get; }

    /// <summary>Radius of the solid centre disc.</summary>
    public double CoreRadius => PetalLayout.FinderCore;

    /// <summary>Centres of the five petal circles, the first pointing straight up.</summary>
    public IReadOnlyList<PetalPoint> PetalCenters { get; }

    /// <summary>Radius of each petal circle.</summary>
    public double PetalRadius => PetalLayout.FinderPetalRadius;

    /// <summary>Centres of the five notch circles cut into the petal tips.</summary>
    public IReadOnlyList<PetalPoint> NotchCenters { get; }

    /// <summary>Radius of each notch circle.</summary>
    public double NotchRadius => PetalLayout.FinderNotchRadius;
}

/// <summary>One <c>#</c> cell of the <c>天</c> lattice, drawn in the backdrop colour inside <see cref="PetalLayout.BackdropRadius"/>.</summary>
/// <param name="Column">Lattice column.</param>
/// <param name="Row">Lattice row.</param>
/// <param name="X">Left edge in canvas units.</param>
/// <param name="Y">Top edge in canvas units.</param>
/// <param name="Size">Side in canvas units (the 33-unit lattice pitch).</param>
public readonly record struct PetalBackdropCell(int Column, int Row, double X, double Y, double Size);

/// <summary>A lit petal of the field.</summary>
/// <param name="Cell">Field cell index, 0 to 3375.</param>
/// <param name="Center">Petal centre in canvas units.</param>
/// <param name="Angle">Outward direction of the petal: radians, clockwise on the screen from 3 o'clock.</param>
/// <param name="Light">Whether the petal lies inside the <c>天</c> and is drawn in the light colour (pink otherwise).</param>
public readonly record struct PetalCellShape(int Cell, PetalPoint Center, double Angle, bool Light);

/// <summary>A lit ring dot.</summary>
/// <param name="Center">Dot centre in canvas units.</param>
/// <param name="Radius">Dot radius (9 units).</param>
/// <param name="Ring">Ring index.</param>
/// <param name="Slot">Slot within the ring.</param>
public readonly record struct PetalDotShape(PetalPoint Center, double Radius, int Ring, int Slot);

/// <summary>
/// A platform-neutral description of one frame for vector backends, in canvas
/// design units (a 1024 × 1024 square).
/// </summary>
/// <remarks>
/// <para>
/// Painting order: background; finders (core and petals in the light colour, then
/// notches in the background colour); the <see cref="BackdropCells"/> in the backdrop
/// colour, clipped to the disc of radius <see cref="PetalLayout.BackdropRadius"/> around
/// the canvas centre; every lit petal in its colour (light inside the <c>天</c>, pink
/// outside): an ellipse of half-length <see cref="PetalHalfLength"/> along its outward
/// angle and half-width <see cref="PetalHalfWidth"/>; lit dots in pink. A petal's
/// cosmetic notch (a disc of radius <see cref="PetalNotchRadius"/> centred
/// <see cref="PetalNotchDistance"/> outward from the petal centre, which the reference
/// renderer cuts out) may be omitted; <see cref="Draw"/> omits it.
/// </para>
/// <para>
/// The result matches <see cref="PetalRenderer"/> up to anti-aliasing and the notches,
/// which decoders tolerate. The finder and backdrop shapes are shared by every list.
/// </para>
/// </remarks>
public sealed class PetalDrawList
{
    private static readonly PetalFinderShape[] FinderShapes = BuildFinders();
    private static readonly PetalBackdropCell[] BackdropShapes = BuildBackdrop();
    private static readonly PetalCellShape[] CellShapes = BuildCells();
    private static readonly PetalDotShape[] SlotShapes = BuildDots();
    private static readonly IReadOnlyList<PetalFinderShape> FinderView = Array.AsReadOnly(FinderShapes);
    private static readonly IReadOnlyList<PetalBackdropCell> BackdropView = Array.AsReadOnly(BackdropShapes);

    private readonly PetalCellShape[] petals;
    private readonly PetalDotShape[] dots;

    private PetalDrawList(PetalPalette palette, PetalCellShape[] petals, PetalDotShape[] dots)
    {
        Palette = palette;
        this.petals = petals;
        this.dots = dots;
        Petals = Array.AsReadOnly(petals);
        Dots = Array.AsReadOnly(dots);
    }

    /// <summary>Canvas side in design units.</summary>
    public static double CanvasSize => PetalLayout.Canvas;

    /// <summary>Half the length of a petal along its outward direction (the reference's single-precision value, widened).</summary>
    public static double PetalHalfLength => PetalLayout.PetalHalfLength;

    /// <summary>Half the width of a petal.</summary>
    public static double PetalHalfWidth => PetalLayout.PetalHalfWidth;

    /// <summary>Radius of the cosmetic notch at a petal tip.</summary>
    public static double PetalNotchRadius => PetalLayout.PetalNotchRadius;

    /// <summary>Distance from a petal centre to its notch centre, outward.</summary>
    public static double PetalNotchDistance => PetalLayout.PetalNotchDistance;

    /// <summary>Colours of the frame.</summary>
    public PetalPalette Palette { get; }

    /// <summary>The four finders, clockwise from the top-left.</summary>
    public IReadOnlyList<PetalFinderShape> Finders => FinderView;

    /// <summary>The <c>#</c> cells of the <c>天</c> lattice, row-major; clip them to the backdrop disc.</summary>
    public IReadOnlyList<PetalBackdropCell> BackdropCells => BackdropView;

    /// <summary>Every lit petal, in field cell order.</summary>
    public IReadOnlyList<PetalCellShape> Petals { get; }

    /// <summary>Every lit ring dot (gates and set data bits), in flat slot order.</summary>
    public IReadOnlyList<PetalDotShape> Dots { get; }

    /// <summary>Builds the draw list of a frame.</summary>
    /// <param name="cells">The frame cells.</param>
    /// <param name="palette">Colours; <see cref="PetalPalette.Default"/> when omitted.</param>
    /// <returns>The draw list.</returns>
    public static PetalDrawList Create(PetalFrameCells cells, PetalPalette? palette = null)
    {
        ArgumentNullException.ThrowIfNull(cells);
        var lit = 0;
        foreach (var petal in cells.Petals)
            lit += petal ? 1 : 0;
        var petals = new PetalCellShape[lit];
        var at = 0;
        for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
        {
            if (cells.Petals[cell])
                petals[at++] = CellShapes[cell];
        }

        var dots = new List<PetalDotShape>(PetalLayout.TotalSlots);
        for (var flat = 0; flat < PetalLayout.TotalSlots; flat++)
        {
            if (cells.Dots[flat])
                dots.Add(SlotShapes[flat]);
        }

        return new PetalDrawList(palette ?? PetalPalette.Default, petals, dots.ToArray());
    }

    /// <summary>
    /// Replays the frame onto <paramref name="canvas"/>, mapping canvas point
    /// <c>(x, y)</c> to <c>(offsetX + x * scale, offsetY + y * scale)</c>.
    /// </summary>
    /// <param name="canvas">The vector backend.</param>
    /// <param name="scale">Target pixels per design unit (side / 1024).</param>
    /// <param name="offsetX">Target x of the canvas origin.</param>
    /// <param name="offsetY">Target y of the canvas origin.</param>
    public void Draw(IPetalCanvas canvas, double scale = 1.0, double offsetX = 0.0, double offsetY = 0.0)
    {
        ArgumentNullException.ThrowIfNull(canvas);
        if (!double.IsFinite(scale) || scale <= 0.0)
            throw new ArgumentOutOfRangeException(nameof(scale));
        var palette = Palette;
        double X(double x) => offsetX + x * scale;
        double Y(double y) => offsetY + y * scale;
        canvas.FillRectangle(X(0.0), Y(0.0), PetalLayout.Canvas * scale, PetalLayout.Canvas * scale, palette.Background);
        foreach (var finder in FinderShapes)
        {
            canvas.FillCircle(X(finder.Center.X), Y(finder.Center.Y), finder.CoreRadius * scale, palette.Light);
            foreach (var petal in finder.PetalCenters)
                canvas.FillCircle(X(petal.X), Y(petal.Y), finder.PetalRadius * scale, palette.Light);
            foreach (var notch in finder.NotchCenters)
                canvas.FillCircle(X(notch.X), Y(notch.Y), finder.NotchRadius * scale, palette.Background);
        }

        canvas.PushCircleClip(X(PetalLayout.Center), Y(PetalLayout.Center), PetalLayout.BackdropRadius * scale);
        foreach (var cell in BackdropShapes)
            canvas.FillRectangle(X(cell.X), Y(cell.Y), cell.Size * scale, cell.Size * scale, palette.Backdrop);
        canvas.PopClip();
        foreach (var petal in petals)
        {
            canvas.FillEllipse(
                X(petal.Center.X),
                Y(petal.Center.Y),
                PetalHalfLength * scale,
                PetalHalfWidth * scale,
                petal.Angle,
                petal.Light ? palette.Light : palette.Pink);
        }

        foreach (var dot in dots)
            canvas.FillCircle(X(dot.Center.X), Y(dot.Center.Y), dot.Radius * scale, palette.Pink);
    }

    private static PetalFinderShape[] BuildFinders()
    {
        var shapes = new PetalFinderShape[4];
        for (var i = 0; i < 4; i++)
        {
            var center = PetalLayout.FinderCenterTable[i];
            var petals = new PetalPoint[PetalLayout.FinderPetals];
            var notches = new PetalPoint[PetalLayout.FinderPetals];
            for (var petal = 0; petal < PetalLayout.FinderPetals; petal++)
            {
                var angle = -(Math.PI / 2.0) + Math.Tau * petal / PetalLayout.FinderPetals;
                var (cos, sin) = (Math.Cos(angle), Math.Sin(angle));
                petals[petal] = new PetalPoint(
                    center.X + PetalLayout.FinderPetalDistance * cos,
                    center.Y + PetalLayout.FinderPetalDistance * sin);
                notches[petal] = new PetalPoint(
                    center.X + PetalLayout.FinderOuter * cos,
                    center.Y + PetalLayout.FinderOuter * sin);
            }

            shapes[i] = new PetalFinderShape(center, petals, notches);
        }

        return shapes;
    }

    private static PetalBackdropCell[] BuildBackdrop()
    {
        var cells = new List<PetalBackdropCell>();
        for (var row = 0; row < PetalLayout.MaskGrid; row++)
        {
            for (var column = 0; column < PetalLayout.MaskGrid; column++)
            {
                if (PetalLayout.Mask[row][column] != '#')
                    continue;
                cells.Add(new PetalBackdropCell(
                    column,
                    row,
                    PetalLayout.MaskOrigin + PetalLayout.MaskPitch * column,
                    PetalLayout.MaskOrigin + PetalLayout.MaskPitch * row,
                    PetalLayout.MaskPitch));
            }
        }

        return cells.ToArray();
    }

    private static PetalCellShape[] BuildCells()
    {
        var shapes = new PetalCellShape[PetalLayout.FieldCells];
        for (var cell = 0; cell < shapes.Length; cell++)
        {
            shapes[cell] = new PetalCellShape(
                cell,
                PetalLayout.CellCenter(cell),
                PetalLayout.CellAngle(cell),
                PetalLayout.CellLight(cell));
        }

        return shapes;
    }

    private static PetalDotShape[] BuildDots()
    {
        var shapes = new PetalDotShape[PetalLayout.TotalSlots];
        for (var flat = 0; flat < shapes.Length; flat++)
        {
            var (ring, slot) = PetalLayout.SplitSlot(flat);
            var slots = PetalLayout.RingSlotTable[ring];
            var radius = PetalLayout.RingRadiusTable[ring];
            // Double precision, as the reference renderer draws its dots.
            var angle = Math.Tau * slot / slots;
            var center = new PetalPoint(
                PetalLayout.Center + radius * Math.Cos(angle),
                PetalLayout.Center + radius * Math.Sin(angle));
            shapes[flat] = new PetalDotShape(center, PetalLayout.DotRadius, ring, slot);
        }

        return shapes;
    }
}
