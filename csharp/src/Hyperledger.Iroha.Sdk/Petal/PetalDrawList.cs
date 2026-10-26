namespace Hyperledger.Iroha.Petal;

/// <summary>
/// A vector 2D surface that can replay a <see cref="PetalDrawList"/>: SkiaSharp,
/// .NET MAUI <c>ICanvas</c>, WPF/Avalonia <c>DrawingContext</c>, Win2D, Core
/// Graphics or an Android <c>Canvas</c> adapter implements these seven calls.
/// </summary>
/// <remarks>
/// Coordinates arrive already scaled to the caller's target size. Shapes are
/// filled without stroke and should be antialiased; petals and glyph strokes must
/// be (the turn reader reads the petals' shapes and the band reader the glyphs').
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

    /// <summary>Fills an axis-aligned rectangle whose corners are rounded to <paramref name="radius"/> (a light katakana tile), antialiased.</summary>
    /// <param name="x">Left edge.</param>
    /// <param name="y">Top edge.</param>
    /// <param name="width">Width.</param>
    /// <param name="height">Height.</param>
    /// <param name="radius">Corner radius.</param>
    /// <param name="color">Fill colour.</param>
    void FillRoundedRectangle(double x, double y, double width, double height, double radius, PetalColor color);

    /// <summary>
    /// Strokes an open polyline (one stroke of a katakana glyph), antialiased, with round caps and
    /// round joins: every point within <paramref name="width"/> / 2 of one of its segments is painted.
    /// </summary>
    /// <remarks>
    /// For example SkiaSharp <c>SKPaint</c> with <c>SKPaintStyle.Stroke</c>,
    /// <c>SKStrokeCap.Round</c> and <c>SKStrokeJoin.Round</c>, or a WPF <c>Pen</c> with
    /// <c>PenLineCap.Round</c> and <c>PenLineJoin.Round</c>. A glyph's strokes stay inside their
    /// tile, so no clip is needed.
    /// </remarks>
    /// <param name="points">The polyline's points, at least two.</param>
    /// <param name="width">Stroke width.</param>
    /// <param name="color">Stroke colour.</param>
    void StrokePolyline(ReadOnlySpan<PetalPoint> points, double width, PetalColor color);

    /// <summary>
    /// Fills a petal, antialiased: the ellipse of half-length <paramref name="radiusAlong"/> along
    /// the direction <paramref name="angle"/> and half-width <paramref name="radiusAcross"/>, minus
    /// its notch, the disc of radius <paramref name="notchRadius"/> centred
    /// <paramref name="notchDistance"/> from the centre along <paramref name="angle"/>.
    /// </summary>
    /// <remarks>
    /// The notch must be cut out, leaving whatever was drawn under it visible: it tells a petal from
    /// one turned by half a turn, and the turn reader's templates include it. Fill the ellipse path
    /// minus the notch circle (for example SkiaSharp <c>SKPath.Op</c> with
    /// <c>SKPathOp.Difference</c>, WPF <c>CombinedGeometry</c> with
    /// <c>GeometryCombineMode.Exclude</c>, Win2D <c>CanvasGeometry.CombineWith</c> with
    /// <c>CanvasGeometryCombine.Exclude</c>), or fill the ellipse under a clip that leaves the
    /// notch out (Android <c>Canvas.clipOutPath</c>).
    /// </remarks>
    /// <param name="centerX">Centre x.</param>
    /// <param name="centerY">Centre y.</param>
    /// <param name="radiusAlong">Half-length along the direction <paramref name="angle"/>.</param>
    /// <param name="radiusAcross">Half-width across that direction.</param>
    /// <param name="angle">The petal's axis: radians, clockwise on the screen from 3 o'clock.</param>
    /// <param name="notchDistance">Distance from the centre to the notch centre, along the axis.</param>
    /// <param name="notchRadius">Radius of the notch.</param>
    /// <param name="color">Fill colour.</param>
    void FillPetal(
        double centerX,
        double centerY,
        double radiusAlong,
        double radiusAcross,
        double angle,
        double notchDistance,
        double notchRadius,
        PetalColor color);

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
/// <param name="Angle">
/// The petal's axis: its outward direction plus its turn (<see cref="PetalTurns.TurnAngle"/> of
/// <see cref="PetalFrameCells.Turns"/>), radians, clockwise on the screen from 3 o'clock.
/// </param>
/// <param name="Light">Whether the petal lies inside the <c>天</c> and is drawn in the light colour (pink otherwise).</param>
public readonly record struct PetalCellShape(int Cell, PetalPoint Center, double Angle, bool Light);

/// <summary>A lit ring dot.</summary>
/// <param name="Center">Dot centre in canvas units.</param>
/// <param name="Radius">Dot radius (9 units).</param>
/// <param name="Ring">Ring index.</param>
/// <param name="Slot">Slot within the ring.</param>
public readonly record struct PetalDotShape(PetalPoint Center, double Radius, int Ring, int Slot);

/// <summary>A katakana tile of the card's bands.</summary>
/// <param name="Tile">Tile index (<see cref="PetalLayout.KanaTiles"/> order).</param>
/// <param name="X">Left edge of the tile square in square coordinates.</param>
/// <param name="Y">Top edge of the tile square in square coordinates (negative above the square).</param>
/// <param name="Glyph">The glyph it shows, 0 to 15 (see <see cref="PetalGlyphs"/>).</param>
/// <param name="Light">Whether the tile is light (filled, glyph in ink) rather than dark (glyph in pink).</param>
public readonly record struct PetalTileShape(int Tile, double X, double Y, byte Glyph, bool Light);

/// <summary>
/// A platform-neutral description of one frame for vector backends: the portrait card, in square
/// design units (the square spans 0 to 1024 on both axes; the bands lie above and below it, so the
/// card spans <c>y</c> from −<see cref="SquareTop"/> to 1024 + <see cref="SquareTop"/>).
/// </summary>
/// <remarks>
/// <para>
/// Painting order: background over the whole card; finders (core and petals in the light colour,
/// then notches in the background colour); the <see cref="BackdropCells"/> in the backdrop colour,
/// clipped to the disc of radius <see cref="PetalLayout.BackdropRadius"/> around the canvas
/// centre; every lit petal in its colour (light inside the <c>天</c>, pink outside), antialiased:
/// an ellipse of half-length <see cref="PetalHalfLength"/> along its
/// <see cref="PetalCellShape.Angle"/> (its outward direction plus its turn) and half-width
/// <see cref="PetalHalfWidth"/>, minus its notch, a disc of radius <see cref="PetalNotchRadius"/>
/// centred <see cref="PetalNotchDistance"/> along that angle (<see cref="IPetalCanvas.FillPetal"/>);
/// lit dots in pink. The notch must be drawn: it tells a petal from one turned by half a turn, and
/// the turn reader's templates include it.
/// </para>
/// <para>
/// Then every one of the <see cref="Tiles"/>: a light tile is a square of side
/// <see cref="KanaTileSize"/> with corners rounded to <see cref="KanaCornerRadius"/>, filled with the
/// light colour (<see cref="IPetalCanvas.FillRoundedRectangle"/>); a dark tile is not filled. Its
/// glyph is stroked along <see cref="KanaGlyphStrokes"/> (polylines in tile-local units from the
/// tile's top-left corner, round caps and joins, width <see cref="KanaStrokeWidth"/>,
/// <see cref="IPetalCanvas.StrokePolyline"/>) in ink on a light tile and pink on a dark one
/// (<see cref="GlyphColor"/>). Every stroke lies inside its rounded tile, so no clip is needed.
/// </para>
/// <para>
/// The result matches <see cref="PetalRenderer"/> up to anti-aliasing. The finder and backdrop
/// shapes and the glyph strokes are shared by every list.
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

    /// <summary>The strokes of every glyph in tile-local units: the glyph box is inset by 1 unit and scaled by 23/32.</summary>
    private static readonly IReadOnlyList<IReadOnlyList<PetalPoint>>[] TileStrokes =
        PetalGlyphs.BuildViews((PetalLayout.KanaTile - PetalLayout.KanaGlyphBox) / 2.0, PetalLayout.KanaGlyphBox / PetalGlyphs.GlyphGrid);

    private readonly PetalCellShape[] petals;
    private readonly PetalDotShape[] dots;
    private readonly PetalTileShape[] tiles;

    private PetalDrawList(PetalPalette palette, PetalCellShape[] petals, PetalDotShape[] dots, PetalTileShape[] tiles)
    {
        Palette = palette;
        this.petals = petals;
        this.dots = dots;
        this.tiles = tiles;
        Petals = Array.AsReadOnly(petals);
        Dots = Array.AsReadOnly(dots);
        Tiles = Array.AsReadOnly(tiles);
    }

    /// <summary>Canvas side in design units: the width of the card and the side of its square.</summary>
    public static double CanvasSize => PetalLayout.Canvas;

    /// <summary>Height of the card in design units: the square with a band above and below it (1668).</summary>
    public static double CardHeight => PetalLayout.CardHeight;

    /// <summary>Distance from the card's top edge to the square's, in design units (322).</summary>
    public static double SquareTop => PetalLayout.SquareTop;

    /// <summary>Side of a katakana tile in design units.</summary>
    public static double KanaTileSize => PetalLayout.KanaTile;

    /// <summary>Corner radius of a katakana tile in design units.</summary>
    public static double KanaCornerRadius => PetalLayout.KanaCorner;

    /// <summary>Width of a glyph's strokes in design units: the design grid's stroke width scaled into the glyph box (<c>6.5 · 23 / 32</c>).</summary>
    public static double KanaStrokeWidth => PetalGlyphs.StrokeWidth * PetalLayout.KanaGlyphBox / PetalGlyphs.GlyphGrid;

    /// <summary>Half the length of a petal along its axis (the reference's single-precision value, widened).</summary>
    public static double PetalHalfLength => PetalLayout.PetalHalfLength;

    /// <summary>Half the width of a petal.</summary>
    public static double PetalHalfWidth => PetalLayout.PetalHalfWidth;

    /// <summary>Radius of the notch at a petal tip.</summary>
    public static double PetalNotchRadius => PetalLayout.PetalNotchRadius;

    /// <summary>Distance from a petal centre to its notch centre, along its axis.</summary>
    public static double PetalNotchDistance => PetalLayout.PetalNotchDistance;

    /// <summary>Colours of the frame.</summary>
    public PetalPalette Palette { get; }

    /// <summary>The four finders, clockwise from the top-left.</summary>
    public IReadOnlyList<PetalFinderShape> Finders => FinderView;

    /// <summary>The <c>#</c> cells of the <c>天</c> lattice, row-major; clip them to the backdrop disc.</summary>
    public IReadOnlyList<PetalBackdropCell> BackdropCells => BackdropView;

    /// <summary>Every lit petal, in field cell order, turned by its turn.</summary>
    public IReadOnlyList<PetalCellShape> Petals { get; }

    /// <summary>Every lit ring dot (gates and set data bits), in flat slot order.</summary>
    public IReadOnlyList<PetalDotShape> Dots { get; }

    /// <summary>Every katakana tile of both bands, in tile order, with the glyph and polarity it shows.</summary>
    public IReadOnlyList<PetalTileShape> Tiles { get; }

    /// <summary>
    /// The strokes of <paramref name="glyph"/> in tile-local design units, measured from a tile's
    /// top-left corner (the glyph box is inset by 1 unit): polylines drawn with round caps and joins
    /// of width <see cref="KanaStrokeWidth"/>.
    /// </summary>
    /// <param name="glyph">Glyph, 0 to 15.</param>
    /// <returns>The strokes.</returns>
    public static IReadOnlyList<IReadOnlyList<PetalPoint>> KanaGlyphStrokes(int glyph)
    {
        if ((uint)glyph >= PetalGlyphs.GlyphCount)
            throw new ArgumentOutOfRangeException(nameof(glyph), "A glyph is 0 to 15.");
        return TileStrokes[glyph];
    }

    /// <summary>The colour of <paramref name="tile"/>'s glyph: ink on a light tile, pink on a dark one.</summary>
    /// <param name="tile">A tile of this list.</param>
    /// <returns>The glyph colour.</returns>
    public PetalColor GlyphColor(PetalTileShape tile) => tile.Light ? Palette.Ink : Palette.Pink;

    /// <summary>Builds the draw list of a frame.</summary>
    /// <param name="cells">The frame cells.</param>
    /// <param name="palette">Colours; <see cref="PetalPalette.Default"/> when omitted.</param>
    /// <returns>The draw list.</returns>
    /// <exception cref="ArgumentException">A katakana tile's glyph is not 0 to 15.</exception>
    public static PetalDrawList Create(PetalFrameCells cells, PetalPalette? palette = null)
    {
        ArgumentNullException.ThrowIfNull(cells);
        PetalRenderer.CheckGlyphs(cells);
        var lit = 0;
        foreach (var petal in cells.Petals)
            lit += petal ? 1 : 0;
        var petals = new PetalCellShape[lit];
        var at = 0;
        for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
        {
            if (!cells.Petals[cell])
                continue;
            var shape = CellShapes[cell];
            petals[at++] = cells.Turns[cell] == 0
                ? shape
                : shape with { Angle = shape.Angle + PetalTurns.TurnAngle(cells.Turns[cell]) };
        }

        var dots = new List<PetalDotShape>(PetalLayout.TotalSlots);
        for (var flat = 0; flat < PetalLayout.TotalSlots; flat++)
        {
            if (cells.Dots[flat])
                dots.Add(SlotShapes[flat]);
        }

        var tiles = new PetalTileShape[PetalLayout.KanaTileCount];
        var tileX = PetalLayout.TileXTable;
        var tileY = PetalLayout.TileYTable;
        for (var tile = 0; tile < tiles.Length; tile++)
            tiles[tile] = new PetalTileShape(tile, tileX[tile], tileY[tile], cells.Kana[tile].Glyph, cells.Kana[tile].Light);
        return new PetalDrawList(palette ?? PetalPalette.Default, petals, dots.ToArray(), tiles);
    }

    /// <summary>
    /// Replays the card onto <paramref name="canvas"/>, mapping square point <c>(x, y)</c> to
    /// <c>(offsetX + x * scale, offsetY + (y + SquareTop) * scale)</c>: <c>(offsetX, offsetY)</c> is
    /// the card's top-left corner, and the card is <c>1024 · scale</c> wide and
    /// <c>1668 · scale</c> tall.
    /// </summary>
    /// <param name="canvas">The vector backend.</param>
    /// <param name="scale">Target pixels per design unit (card width / 1024).</param>
    /// <param name="offsetX">Target x of the card's left edge.</param>
    /// <param name="offsetY">Target y of the card's top edge.</param>
    public void Draw(IPetalCanvas canvas, double scale = 1.0, double offsetX = 0.0, double offsetY = 0.0)
    {
        ArgumentNullException.ThrowIfNull(canvas);
        if (!double.IsFinite(scale) || scale <= 0.0)
            throw new ArgumentOutOfRangeException(nameof(scale));
        var palette = Palette;
        double X(double x) => offsetX + x * scale;
        double Y(double y) => offsetY + (y + PetalLayout.SquareTop) * scale;
        canvas.FillRectangle(X(0.0), offsetY, PetalLayout.Canvas * scale, PetalLayout.CardHeight * scale, palette.Background);
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
            canvas.FillPetal(
                X(petal.Center.X),
                Y(petal.Center.Y),
                PetalHalfLength * scale,
                PetalHalfWidth * scale,
                petal.Angle,
                PetalNotchDistance * scale,
                PetalNotchRadius * scale,
                petal.Light ? palette.Light : palette.Pink);
        }

        foreach (var dot in dots)
            canvas.FillCircle(X(dot.Center.X), Y(dot.Center.Y), dot.Radius * scale, palette.Pink);
        Span<PetalPoint> points = stackalloc PetalPoint[8];
        foreach (var tile in tiles)
        {
            if (tile.Light)
            {
                canvas.FillRoundedRectangle(
                    X(tile.X),
                    Y(tile.Y),
                    PetalLayout.KanaTile * scale,
                    PetalLayout.KanaTile * scale,
                    PetalLayout.KanaCorner * scale,
                    palette.Light);
            }

            var colour = GlyphColor(tile);
            foreach (var stroke in TileStrokes[tile.Glyph])
            {
                for (var i = 0; i < stroke.Count; i++)
                    points[i] = new PetalPoint(X(tile.X + stroke[i].X), Y(tile.Y + stroke[i].Y));
                canvas.StrokePolyline(points[..stroke.Count], KanaStrokeWidth * scale, colour);
            }
        }
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
