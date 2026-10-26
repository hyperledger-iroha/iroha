namespace Hyperledger.Iroha.Petal;

/// <summary>An 8-bit sRGB colour.</summary>
/// <param name="R">Red.</param>
/// <param name="G">Green.</param>
/// <param name="B">Blue.</param>
public readonly record struct PetalColor(byte R, byte G, byte B);

/// <summary>Colours of a Petal picture.</summary>
/// <param name="Background">Background (black by default), also the gutters between katakana tiles and the fill of dark tiles.</param>
/// <param name="Light">Finders, the petals inside the <c>天</c> and the fill of light katakana tiles.</param>
/// <param name="Pink">Sakura pink of the ring dots, of the petals outside the <c>天</c> and of the glyphs on dark tiles.</param>
/// <param name="Backdrop">The faint <c>天</c> drawn under the petals (a tenth of the light colour).</param>
/// <param name="Ink">The glyphs on light katakana tiles.</param>
public sealed record PetalPalette(PetalColor Background, PetalColor Light, PetalColor Pink, PetalColor Backdrop, PetalColor Ink)
{
    /// <summary>
    /// The reference palette: background <c>(0, 0, 0)</c>, light
    /// <c>(250, 235, 244)</c>, pink <c>(245, 175, 208)</c>, backdrop <c>(25, 24, 24)</c>,
    /// ink <c>(20, 4, 14)</c>.
    /// </summary>
    public static PetalPalette Default { get; } = new(
        new PetalColor(0, 0, 0),
        new PetalColor(250, 235, 244),
        new PetalColor(245, 175, 208),
        new PetalColor(25, 24, 24),
        new PetalColor(20, 4, 14));
}

/// <summary>Software rendering options.</summary>
public sealed record PetalRenderOptions
{
    private readonly int size = 1024;
    private readonly int supersample = 3;
    private readonly PetalPalette palette = PetalPalette.Default;

    /// <summary>The reference defaults: 1024 px, 3×3 supersampling, default palette.</summary>
    public static PetalRenderOptions Default { get; } = new();

    /// <summary>Output width in pixels; the card is <see cref="PetalRenderer.CardHeightPx"/> pixels tall.</summary>
    /// <exception cref="ArgumentOutOfRangeException">The size is not positive.</exception>
    public int Size
    {
        get => size;
        init
        {
            ArgumentOutOfRangeException.ThrowIfNegativeOrZero(value);
            size = value;
        }
    }

    /// <summary>Samples per pixel side for anti-aliasing (1–4).</summary>
    /// <exception cref="ArgumentOutOfRangeException">The value is outside 1–4.</exception>
    public int Supersample
    {
        get => supersample;
        init
        {
            if (value is < 1 or > 4)
                throw new ArgumentOutOfRangeException(nameof(value), "Supersampling must be within 1..=4.");
            supersample = value;
        }
    }

    /// <summary>Colours.</summary>
    public PetalPalette Palette
    {
        get => palette;
        init
        {
            ArgumentNullException.ThrowIfNull(value);
            palette = value;
        }
    }
}

/// <summary>
/// Reference software renderer.
/// </summary>
/// <remarks>
/// <para>
/// The picture is a portrait card (<see cref="PetalLayout.CardHeight"/>): a black square with four
/// sakura-blossom finders in its corners, three dotted rings, and inside them a field of petals on
/// concentric bands, each lit petal turned about its centre by its turn
/// (<see cref="PetalFrameCells.Turns"/>). A lit petal whose centre lies inside the <c>天</c> is
/// drawn in the light colour, the others in sakura pink, over a faint backdrop of the <c>天</c>
/// itself, so the character shows through the storm of petals. Above and below that square, the
/// card's two bands show rows of katakana tiles (see <see cref="PetalKana"/>): a light tile filled
/// light with its glyph in ink, a dark tile black with its glyph in pink, black gutters between them.
/// </para>
/// <para>
/// In the square each sample is shaded in this order: finders, ring dots, petals, the backdrop,
/// background; in the bands it takes its tile's colour or black. The output matches the Rust
/// reference renderer pixel for pixel. Platform code may instead draw a <see cref="PetalDrawList"/>
/// with native 2D APIs, as long as geometry, turns, glyphs and polarity match.
/// </para>
/// </remarks>
public static class PetalRenderer
{
    /// <summary>Height in pixels of a card rendered <paramref name="size"/> pixels wide: <c>size · 1668 / 1024</c>, rounded.</summary>
    /// <param name="size">Card width in pixels.</param>
    /// <returns>The card height in pixels.</returns>
    public static int CardHeightPx(int size)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(size);
        var height = size * PetalLayout.CardHeight / PetalLayout.Canvas;
        return (int)Math.Floor(height + 0.5);
    }

    /// <summary>
    /// Renders one frame: the card, <see cref="PetalRenderOptions.Size"/> pixels wide and
    /// <see cref="CardHeightPx"/> tall, whose square starts <see cref="PetalLayout.SquareTop"/> units
    /// below the top edge.
    /// </summary>
    /// <param name="cells">The frame cells.</param>
    /// <param name="options">Options; <see cref="PetalRenderOptions.Default"/> when omitted.</param>
    /// <returns>An RGB image of the card.</returns>
    /// <exception cref="ArgumentException">A katakana tile's glyph is not 0 to 15.</exception>
    public static PetalRgbImage Render(PetalFrameCells cells, PetalRenderOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(cells);
        CheckGlyphs(cells);
        options ??= PetalRenderOptions.Default;
        return RenderRows(cells, options, CardHeightPx(options.Size), PetalLayout.SquareTop);
    }

    /// <summary>
    /// The colour of square-coordinate point <c>(x, y)</c> of the card without anti-aliasing: in the
    /// square (<c>0 ≤ y &lt; 1024</c>) its petals, dots and finders, outside it the bands' tiles.
    /// </summary>
    /// <param name="cells">The frame cells.</param>
    /// <param name="x">Square x.</param>
    /// <param name="y">Square y (negative above the square).</param>
    /// <param name="palette">Colours; <see cref="PetalPalette.Default"/> when omitted.</param>
    /// <returns>The shaded colour.</returns>
    /// <exception cref="ArgumentException">A katakana tile's glyph is not 0 to 15.</exception>
    public static PetalColor Shade(PetalFrameCells cells, double x, double y, PetalPalette? palette = null)
    {
        ArgumentNullException.ThrowIfNull(cells);
        CheckGlyphs(cells);
        var shader = new Shader(cells, palette ?? PetalPalette.Default);
        return y >= 0.0 && y < PetalLayout.Canvas ? shader.Shade(x, y) : shader.Kana(x, y);
    }

    /// <summary>
    /// Renders only the square of the card, <see cref="PetalRenderOptions.Size"/> pixels on a side
    /// (the reference's test-only <c>render_square</c>): tests of the decoder that work on bare
    /// squares use it.
    /// </summary>
    internal static PetalRgbImage RenderSquare(PetalFrameCells cells, PetalRenderOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(cells);
        options ??= PetalRenderOptions.Default;
        return RenderRows(cells, options, options.Size, 0.0);
    }

    /// <summary>Renders <paramref name="height"/> pixel rows of the card, the first <paramref name="top"/> canvas units above the square's top edge.</summary>
    private static PetalRgbImage RenderRows(PetalFrameCells cells, PetalRenderOptions options, int height, double top)
    {
        var size = options.Size;
        var s = options.Supersample;
        var shader = new Shader(cells, options.Palette);
        var unit = PetalLayout.Canvas / size;
        var data = new byte[checked(size * height * 3)];
        var n = (uint)(s * s);
        for (var py = 0; py < height; py++)
        {
            for (var px = 0; px < size; px++)
            {
                uint r = 0;
                uint g = 0;
                uint b = 0;
                for (var sy = 0; sy < s; sy++)
                {
                    for (var sx = 0; sx < s; sx++)
                    {
                        var x = (px + (sx + 0.5) / s) * unit;
                        var y = (py + (sy + 0.5) / s) * unit - top;
                        var c = y >= 0.0 && y < PetalLayout.Canvas ? shader.Shade(x, y) : shader.Kana(x, y);
                        r += c.R;
                        g += c.G;
                        b += c.B;
                    }
                }

                var at = (py * size + px) * 3;
                data[at] = (byte)((r + n / 2) / n);
                data[at + 1] = (byte)((g + n / 2) / n);
                data[at + 2] = (byte)((b + n / 2) / n);
            }
        }

        return new PetalRgbImage(size, height, data);
    }

    /// <summary>Refuses tiles whose glyph lies outside the alphabet before any of them is drawn.</summary>
    internal static void CheckGlyphs(PetalFrameCells cells)
    {
        foreach (var symbol in cells.Kana)
        {
            if (symbol.Glyph >= PetalGlyphs.GlyphCount)
                throw new ArgumentException("A katakana tile's glyph is 0 to 15.", nameof(cells));
        }
    }

    private readonly struct Shader(PetalFrameCells cells, PetalPalette palette)
    {
        /// <summary>
        /// The colour of a katakana tile at the square-coordinate point <c>(x, y)</c> outside the
        /// square, or the background between tiles.
        /// </summary>
        public PetalColor Kana(double x, double y)
        {
            if (!PetalLayout.TryKanaTileAt(x, y, out var tile, out var u, out var v) || !PetalLayout.KanaInside(u, v))
                return palette.Background;
            var symbol = cells.Kana[tile];
            var inked = PetalLayout.KanaInkedUnchecked(symbol.Glyph, u, v);
            return (inked, symbol.Light) switch
            {
                (true, true) => palette.Ink,
                (false, true) => palette.Light,
                (true, false) => palette.Pink,
                _ => palette.Background,
            };
        }

        public PetalColor Shade(double x, double y)
        {
            // finders
            foreach (var finder in PetalLayout.FinderCenterTable)
            {
                var (fx, fy) = (x - finder.X, y - finder.Y);
                if (Math.Sqrt(fx * fx + fy * fy) <= PetalLayout.FinderOuter)
                    return PetalLayout.FinderLit(fx, fy) ? palette.Light : palette.Background;
            }

            var (dx, dy) = (x - PetalLayout.Center, y - PetalLayout.Center);
            var radius = Math.Sqrt(dx * dx + dy * dy);
            var theta = Math.Atan2(dy, dx);
            if (theta < 0.0)
                theta += Math.Tau;
            // ring dots
            for (var ring = 0; ring < PetalLayout.RingCount; ring++)
            {
                var ringRadius = PetalLayout.RingRadiusTable[ring];
                if (Math.Abs(radius - ringRadius) > PetalLayout.DotRadius)
                    continue;
                var slots = PetalLayout.RingSlotTable[ring];
                var slot = PetalMath.ToIndex(PetalMath.RoundHalfAwayFromZero(theta / Math.Tau * slots)) % slots;
                if (!cells.Dots[PetalLayout.RingOffset(ring) + slot])
                    continue;
                var angle = Math.Tau * slot / slots;
                var (px, py) = (ringRadius * Math.Cos(angle), ringRadius * Math.Sin(angle));
                if (Math.Sqrt((dx - px) * (dx - px) + (dy - py) * (dy - py)) <= PetalLayout.DotRadius)
                    return palette.Pink;
            }

            // petals, then the faint 天 under them
            if (radius <= PetalLayout.BackdropRadius)
            {
                if (Petal(x, y, radius, theta) is { } colour)
                    return colour;
                if (PetalLayout.InMask(x, y))
                    return palette.Backdrop;
            }

            return palette.Background;
        }

        /// <summary>The colour of the lit petal that covers <c>(x, y)</c>, if any.</summary>
        private PetalColor? Petal(double x, double y, double radius, double theta)
        {
            const double reach = PetalLayout.PetalHalfLength;
            var nearest = PetalMath.RoundHalfAwayFromZero((radius - PetalLayout.BandInner) / PetalLayout.BandSpacing);
            var slotCounts = PetalLayout.BandSlotTable;
            var offsets = PetalLayout.BandOffsetTable;
            var xs = PetalLayout.CellXTable;
            var ys = PetalLayout.CellYTable;
            for (var step = -1; step <= 1; step++)
            {
                var candidate = nearest + step;
                if (candidate < 0.0 || candidate >= PetalLayout.BandCount)
                    continue;
                var band = (int)candidate;
                if (Math.Abs(radius - PetalLayout.BandRadius(band)) > reach)
                    continue;
                var count = slotCounts[band];
                double slots = count;
                var around = (long)PetalMath.RoundHalfAwayFromZero(theta / Math.Tau * slots - PetalLayout.BandPhase(band));
                for (var slot = around - 1; slot <= around + 1; slot++)
                {
                    var cell = offsets[band] + (int)(((slot % count) + count) % count);
                    if (!cells.Petals[cell])
                        continue;
                    var (sin, cos) = Axis(cell, cells.Turns[cell]);
                    if (PetalLayout.PetalLit(x - xs[cell], y - ys[cell], sin, cos))
                        return PetalLayout.CellLightTable[cell] ? palette.Light : palette.Pink;
                }
            }

            return null;
        }

        /// <summary>
        /// The sine and cosine of the axis of the petal at <paramref name="cell"/>: its outward
        /// direction plus the turn of <paramref name="turn"/> (as the reference computes it).
        /// </summary>
        private static (double Sin, double Cos) Axis(int cell, byte turn)
        {
            if (turn < PetalTurns.TurnPositions)
            {
                var at = turn * PetalLayout.FieldCells + cell;
                return (Axes.Sines[at], Axes.Cosines[at]);
            }

            var angle = PetalLayout.CellAngleTable[cell] + PetalTurns.TurnAngle(turn);
            return (Math.Sin(angle), Math.Cos(angle));
        }
    }

    /// <summary>
    /// The sine and cosine of every petal's axis, its outward direction plus turn position
    /// <c>p</c>, at <c>p · 3376 + cell</c>; built once, on first use.
    /// </summary>
    private static class Axes
    {
        internal static readonly double[] Sines = new double[PetalTurns.TurnPositions * PetalLayout.FieldCells];

        internal static readonly double[] Cosines = new double[PetalTurns.TurnPositions * PetalLayout.FieldCells];

        static Axes()
        {
            for (var position = 0; position < PetalTurns.TurnPositions; position++)
            {
                for (var cell = 0; cell < PetalLayout.FieldCells; cell++)
                {
                    var angle = PetalLayout.CellAngleTable[cell] + PetalTurns.TurnAngle((byte)position);
                    Sines[position * PetalLayout.FieldCells + cell] = Math.Sin(angle);
                    Cosines[position * PetalLayout.FieldCells + cell] = Math.Cos(angle);
                }
            }
        }
    }
}
