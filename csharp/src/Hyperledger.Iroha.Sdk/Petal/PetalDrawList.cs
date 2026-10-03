namespace Hyperledger.Iroha.Petal;

/// <summary>
/// A vector 2D surface that can replay a <see cref="PetalDrawList"/>: SkiaSharp,
/// .NET MAUI <c>ICanvas</c>, WPF/Avalonia <c>DrawingContext</c>, Win2D, Core
/// Graphics or an Android <c>Canvas</c> adapter implements these six calls.
/// </summary>
/// <remarks>
/// Coordinates arrive already scaled to the caller's target size. Shapes are
/// filled without stroke unless stated; polylines use round caps and round
/// joins. Anti-aliasing is up to the backend.
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

    /// <summary>Fills an axis-aligned rectangle with circular corners.</summary>
    /// <param name="x">Left edge.</param>
    /// <param name="y">Top edge.</param>
    /// <param name="width">Width.</param>
    /// <param name="height">Height.</param>
    /// <param name="cornerRadius">Corner radius.</param>
    /// <param name="color">Fill colour.</param>
    void FillRoundedRectangle(double x, double y, double width, double height, double cornerRadius, PetalColor color);

    /// <summary>Fills a circle.</summary>
    /// <param name="centerX">Centre x.</param>
    /// <param name="centerY">Centre y.</param>
    /// <param name="radius">Radius.</param>
    /// <param name="color">Fill colour.</param>
    void FillCircle(double centerX, double centerY, double radius, PetalColor color);

    /// <summary>Strokes an open polyline with round caps and round joins.</summary>
    /// <param name="points">Polyline vertices (at least two).</param>
    /// <param name="strokeWidth">Full stroke width.</param>
    /// <param name="color">Stroke colour.</param>
    void StrokePolyline(ReadOnlySpan<PetalPoint> points, double strokeWidth, PetalColor color);

    /// <summary>Intersects the clip region with a rectangle until the matching <see cref="PopClip"/>.</summary>
    /// <param name="x">Left edge.</param>
    /// <param name="y">Top edge.</param>
    /// <param name="width">Width.</param>
    /// <param name="height">Height.</param>
    void PushClip(double x, double y, double width, double height);

    /// <summary>Restores the clip region saved by the last <see cref="PushClip"/>.</summary>
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
        PetalCenters = petals;
        NotchCenters = notches;
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

/// <summary>One data tile: a rounded square (light tiles only) and its katakana.</summary>
public sealed class PetalTileShape
{
    internal PetalTileShape(int index, PetalPoint center, bool light, int glyph, PetalPoint[][] strokes)
    {
        Index = index;
        Center = center;
        Light = light;
        Glyph = glyph;
        GlyphStrokes = strokes;
    }

    /// <summary>Tile index (row-major order of the <c>天</c> mask).</summary>
    public int Index { get; }

    /// <summary>Tile centre in canvas units.</summary>
    public PetalPoint Center { get; }

    /// <summary>Left edge of the drawn tile.</summary>
    public double X => Center.X - PetalLayout.TileSize / 2.0;

    /// <summary>Top edge of the drawn tile.</summary>
    public double Y => Center.Y - PetalLayout.TileSize / 2.0;

    /// <summary>Side of the drawn tile (25 units).</summary>
    public double Size => PetalLayout.TileSize;

    /// <summary>Corner radius of the drawn tile (3 units).</summary>
    public double CornerRadius => PetalLayout.TileCornerRadius;

    /// <summary>Whether the tile is light (filled) or dark (background only).</summary>
    public bool Light { get; }

    /// <summary>Katakana symbol, 0 to 15 (see <see cref="PetalGlyphs.Characters"/>).</summary>
    public int Glyph { get; }

    /// <summary>Left edge of the 23-unit glyph box; the glyph is clipped to this box.</summary>
    public double GlyphBoxX => Center.X - PetalLayout.GlyphBox / 2.0;

    /// <summary>Top edge of the 23-unit glyph box.</summary>
    public double GlyphBoxY => Center.Y - PetalLayout.GlyphBox / 2.0;

    /// <summary>Side of the glyph box (23 units).</summary>
    public double GlyphBoxSize => PetalLayout.GlyphBox;

    /// <summary>Glyph stroke width in canvas units (6.5/32 of the glyph box).</summary>
    public double StrokeWidth => PetalGlyphs.StrokeWidth / PetalGlyphs.GlyphGrid * PetalLayout.GlyphBox;

    /// <summary>Glyph stroke polylines in canvas units (round caps and joins).</summary>
    public IReadOnlyList<IReadOnlyList<PetalPoint>> GlyphStrokes { get; }
}

/// <summary>A lit ring dot.</summary>
/// <param name="Center">Dot centre in canvas units.</param>
/// <param name="Radius">Dot radius (11 units).</param>
/// <param name="Ring">Ring index.</param>
/// <param name="Slot">Slot within the ring.</param>
public readonly record struct PetalDotShape(PetalPoint Center, double Radius, int Ring, int Slot);

/// <summary>
/// A platform-neutral description of one frame for vector backends, in canvas
/// design units (a 1024 × 1024 square).
/// </summary>
/// <remarks>
/// Painting order: background; finders (core and petals in the light colour,
/// then notches in the background colour); light tiles as rounded squares
/// (dark tiles are background); each glyph clipped to its glyph box, in the
/// ink colour on light tiles and pink on dark tiles; lit dots in pink. The
/// result matches <see cref="PetalRenderer"/> up to anti-aliasing, which
/// decoders tolerate.
/// </remarks>
public sealed class PetalDrawList
{
    private readonly PetalFinderShape[] finders;
    private readonly PetalTileShape[] tiles;
    private readonly PetalDotShape[] dots;

    private PetalDrawList(PetalPalette palette, PetalFinderShape[] finders, PetalTileShape[] tiles, PetalDotShape[] dots)
    {
        Palette = palette;
        this.finders = finders;
        this.tiles = tiles;
        this.dots = dots;
    }

    /// <summary>Canvas side in design units.</summary>
    public static double CanvasSize => PetalLayout.Canvas;

    /// <summary>Colours of the frame.</summary>
    public PetalPalette Palette { get; }

    /// <summary>The four finders, clockwise from the top-left.</summary>
    public IReadOnlyList<PetalFinderShape> Finders => finders;

    /// <summary>All 256 tiles in index order.</summary>
    public IReadOnlyList<PetalTileShape> Tiles => tiles;

    /// <summary>Every lit ring dot (gates and set data bits).</summary>
    public IReadOnlyList<PetalDotShape> Dots => dots;

    /// <summary>Builds the draw list of a frame.</summary>
    /// <param name="cells">The frame cells.</param>
    /// <param name="palette">Colours; <see cref="PetalPalette.Default"/> when omitted.</param>
    /// <returns>The draw list.</returns>
    public static PetalDrawList Create(PetalFrameCells cells, PetalPalette? palette = null)
    {
        ArgumentNullException.ThrowIfNull(cells);
        var finderShapes = new PetalFinderShape[4];
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

            finderShapes[i] = new PetalFinderShape(center, petals, notches);
        }

        var scale = PetalLayout.GlyphBox / PetalGlyphs.GlyphGrid;
        var tileShapes = new PetalTileShape[PetalLayout.TileCount];
        for (var tile = 0; tile < PetalLayout.TileCount; tile++)
        {
            var center = PetalLayout.TileCenterUnchecked(tile);
            var glyph = cells.Glyph[tile] & 0x0F;
            var boxX = center.X - PetalLayout.GlyphBox / 2.0;
            var boxY = center.Y - PetalLayout.GlyphBox / 2.0;
            var source = PetalGlyphs.Strokes(glyph);
            var strokes = new PetalPoint[source.Count][];
            for (var s = 0; s < source.Count; s++)
            {
                var points = new PetalPoint[source[s].Count];
                for (var p = 0; p < points.Length; p++)
                    points[p] = new PetalPoint(boxX + source[s][p].X * scale, boxY + source[s][p].Y * scale);
                strokes[s] = points;
            }

            tileShapes[tile] = new PetalTileShape(tile, center, cells.Light[tile], glyph, strokes);
        }

        var dotShapes = new List<PetalDotShape>();
        for (var ring = 0; ring < PetalLayout.RingCount; ring++)
        {
            var slots = PetalLayout.RingSlotTable[ring];
            var radius = PetalLayout.RingRadiusTable[ring];
            for (var slot = 0; slot < slots; slot++)
            {
                if (!cells.Dots[PetalLayout.RingOffset(ring) + slot])
                    continue;
                // Double precision, as the reference renderer draws its dots.
                var angle = Math.Tau * slot / slots;
                var center = new PetalPoint(
                    PetalLayout.Center + radius * Math.Cos(angle),
                    PetalLayout.Center + radius * Math.Sin(angle));
                dotShapes.Add(new PetalDotShape(center, PetalLayout.DotRadius, ring, slot));
            }
        }

        return new PetalDrawList(palette ?? PetalPalette.Default, finderShapes, tileShapes, dotShapes.ToArray());
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
        foreach (var finder in finders)
        {
            canvas.FillCircle(X(finder.Center.X), Y(finder.Center.Y), finder.CoreRadius * scale, palette.Light);
            foreach (var petal in finder.PetalCenters)
                canvas.FillCircle(X(petal.X), Y(petal.Y), finder.PetalRadius * scale, palette.Light);
            foreach (var notch in finder.NotchCenters)
                canvas.FillCircle(X(notch.X), Y(notch.Y), finder.NotchRadius * scale, palette.Background);
        }

        Span<PetalPoint> buffer = stackalloc PetalPoint[16];
        foreach (var tile in tiles)
        {
            if (tile.Light)
                canvas.FillRoundedRectangle(X(tile.X), Y(tile.Y), tile.Size * scale, tile.Size * scale, tile.CornerRadius * scale, palette.Light);
            canvas.PushClip(X(tile.GlyphBoxX), Y(tile.GlyphBoxY), tile.GlyphBoxSize * scale, tile.GlyphBoxSize * scale);
            var color = tile.Light ? palette.Ink : palette.Pink;
            foreach (var stroke in tile.GlyphStrokes)
            {
                var points = stroke.Count <= buffer.Length ? buffer[..stroke.Count] : new PetalPoint[stroke.Count];
                for (var i = 0; i < stroke.Count; i++)
                    points[i] = new PetalPoint(X(stroke[i].X), Y(stroke[i].Y));
                canvas.StrokePolyline(points, tile.StrokeWidth * scale, color);
            }

            canvas.PopClip();
        }

        foreach (var dot in dots)
            canvas.FillCircle(X(dot.Center.X), Y(dot.Center.Y), dot.Radius * scale, palette.Pink);
    }
}
