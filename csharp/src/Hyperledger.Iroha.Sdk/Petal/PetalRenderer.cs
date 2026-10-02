namespace Hyperledger.Iroha.Petal;

/// <summary>An 8-bit sRGB colour.</summary>
/// <param name="R">Red.</param>
/// <param name="G">Green.</param>
/// <param name="B">Blue.</param>
public readonly record struct PetalColor(byte R, byte G, byte B);

/// <summary>Colours of a Petal picture.</summary>
/// <param name="Background">Background (black by default).</param>
/// <param name="Light">Light tile fill and finders.</param>
/// <param name="Pink">Sakura pink of dots and of glyphs on dark tiles.</param>
/// <param name="Ink">Glyph colour on a light tile.</param>
public sealed record PetalPalette(PetalColor Background, PetalColor Light, PetalColor Pink, PetalColor Ink)
{
    /// <summary>
    /// The reference palette: background <c>(0, 0, 0)</c>, light
    /// <c>(250, 235, 244)</c>, pink <c>(245, 175, 208)</c>, ink <c>(20, 4, 14)</c>.
    /// </summary>
    public static PetalPalette Default { get; } = new(
        new PetalColor(0, 0, 0),
        new PetalColor(250, 235, 244),
        new PetalColor(245, 175, 208),
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

    /// <summary>Output side in pixels.</summary>
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
/// The picture is black, with four sakura-blossom finders in the canvas
/// corners and a <c>天</c>-shaped field of 256 tiles inside three dotted rings.
/// A light tile is a pale rounded square with a near-black katakana; a dark
/// tile is empty except for its sakura-pink katakana. The output matches the
/// Rust reference renderer pixel for pixel. Platform code may instead draw a
/// <see cref="PetalDrawList"/> with native 2D APIs, as long as geometry and
/// polarity match.
/// </remarks>
public static class PetalRenderer
{
    private const int BitmapSize = 128;
    private static readonly Lazy<bool[][]> GlyphBitmaps = new(BuildGlyphBitmaps);
    private static readonly short[] TileLookup = BuildTileLookup();

    /// <summary>Renders one frame.</summary>
    /// <param name="cells">The frame cells.</param>
    /// <param name="options">Options; <see cref="PetalRenderOptions.Default"/> when omitted.</param>
    /// <returns>An RGB image of <see cref="PetalRenderOptions.Size"/> pixels square.</returns>
    public static PetalRgbImage Render(PetalFrameCells cells, PetalRenderOptions? options = null)
    {
        ArgumentNullException.ThrowIfNull(cells);
        options ??= PetalRenderOptions.Default;
        var size = options.Size;
        var s = options.Supersample;
        var shader = new Shader(cells, options.Palette, GlyphBitmaps.Value);
        var unit = PetalLayout.Canvas / size;
        var data = new byte[checked(size * size * 3)];
        var n = (uint)(s * s);
        for (var py = 0; py < size; py++)
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
                        var y = (py + (sy + 0.5) / s) * unit;
                        var c = shader.Shade(x, y);
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

        return new PetalRgbImage(size, size, data);
    }

    /// <summary>The colour of canvas point <c>(x, y)</c> without anti-aliasing.</summary>
    /// <param name="cells">The frame cells.</param>
    /// <param name="x">Canvas x.</param>
    /// <param name="y">Canvas y.</param>
    /// <param name="palette">Colours; <see cref="PetalPalette.Default"/> when omitted.</param>
    /// <returns>The shaded colour.</returns>
    public static PetalColor Shade(PetalFrameCells cells, double x, double y, PetalPalette? palette = null)
    {
        ArgumentNullException.ThrowIfNull(cells);
        return new Shader(cells, palette ?? PetalPalette.Default, GlyphBitmaps.Value).Shade(x, y);
    }

    /// <summary>Inside test for a rounded square of half-side <paramref name="half"/>, relative to its centre.</summary>
    internal static bool InRoundedSquare(double dx, double dy, double half, double radius)
    {
        var (ax, ay) = (Math.Abs(dx), Math.Abs(dy));
        if (ax > half || ay > half)
            return false;
        var (cx, cy) = (ax - (half - radius), ay - (half - radius));
        return cx <= 0.0 || cy <= 0.0 || cx * cx + cy * cy <= radius * radius;
    }

    private static bool[][] BuildGlyphBitmaps()
    {
        var bitmaps = new bool[PetalGlyphs.GlyphCount][];
        for (var glyph = 0; glyph < PetalGlyphs.GlyphCount; glyph++)
        {
            var bitmap = new bool[BitmapSize * BitmapSize];
            for (var v = 0; v < BitmapSize; v++)
            {
                for (var u = 0; u < BitmapSize; u++)
                {
                    var x = (u + 0.5) / BitmapSize * PetalGlyphs.GlyphGrid;
                    var y = (v + 0.5) / BitmapSize * PetalGlyphs.GlyphGrid;
                    bitmap[v * BitmapSize + u] = PetalGlyphs.IsInked(glyph, x, y);
                }
            }

            bitmaps[glyph] = bitmap;
        }

        return bitmaps;
    }

    private static short[] BuildTileLookup()
    {
        var table = new short[PetalLayout.TileGrid * PetalLayout.TileGrid];
        Array.Fill(table, (short)-1);
        for (var index = 0; index < PetalLayout.TileCount; index++)
        {
            var (column, row) = PetalLayout.Tile(index);
            table[row * PetalLayout.TileGrid + column] = (short)index;
        }

        return table;
    }

    private readonly struct Shader(PetalFrameCells cells, PetalPalette palette, bool[][] bitmaps)
    {
        public PetalColor Shade(double x, double y)
        {
            // finders
            foreach (var finder in PetalLayout.FinderCenterTable)
            {
                var (dx, dy) = (x - finder.X, y - finder.Y);
                if (Math.Sqrt(dx * dx + dy * dy) <= PetalLayout.FinderOuter)
                    return PetalLayout.FinderLit(dx, dy) ? palette.Light : palette.Background;
            }

            // tiles
            const double lattice = PetalLayout.TileOrigin;
            const double pitch = PetalLayout.TilePitch;
            if (x >= lattice && y >= lattice)
            {
                var column = PetalMath.ToIndex((x - lattice) / pitch);
                var row = PetalMath.ToIndex((y - lattice) / pitch);
                if (column < PetalLayout.TileGrid
                    && row < PetalLayout.TileGrid
                    && TileLookup[row * PetalLayout.TileGrid + column] is var tile and >= 0)
                {
                    var cx = lattice + pitch * (column + 0.5);
                    var cy = lattice + pitch * (row + 0.5);
                    var (dx, dy) = (x - cx, y - cy);
                    if (!InRoundedSquare(dx, dy, PetalLayout.TileSize / 2.0, PetalLayout.TileCornerRadius))
                        return palette.Background;
                    const double boxHalf = PetalLayout.GlyphBox / 2.0;
                    var inked = false;
                    if (Math.Abs(dx) < boxHalf && Math.Abs(dy) < boxHalf)
                    {
                        var u = PetalMath.ToIndex((dx + boxHalf) / (2.0 * boxHalf) * BitmapSize);
                        var v = PetalMath.ToIndex((dy + boxHalf) / (2.0 * boxHalf) * BitmapSize);
                        inked = bitmaps[cells.Glyph[tile] & 0x0F][
                            Math.Min(v, BitmapSize - 1) * BitmapSize + Math.Min(u, BitmapSize - 1)];
                    }

                    return (cells.Light[tile], inked) switch
                    {
                        (true, false) => palette.Light,
                        (true, true) => palette.Ink,
                        (false, true) => palette.Pink,
                        (false, false) => palette.Background,
                    };
                }
            }

            // ring dots
            var (rx, ry) = (x - PetalLayout.Center, y - PetalLayout.Center);
            var radius = Math.Sqrt(rx * rx + ry * ry);
            for (var ring = 0; ring < PetalLayout.RingCount; ring++)
            {
                var ringRadius = PetalLayout.RingRadiusTable[ring];
                if (Math.Abs(radius - ringRadius) > PetalLayout.DotRadius)
                    continue;
                var slots = PetalLayout.RingSlotTable[ring];
                var theta = Math.Atan2(ry, rx);
                if (theta < 0.0)
                    theta += Math.Tau;
                var slot = PetalMath.ToIndex(PetalMath.RoundHalfAwayFromZero(theta / Math.Tau * slots)) % slots;
                if (!cells.Dots[PetalLayout.RingOffset(ring) + slot])
                    continue;
                var angle = Math.Tau * slot / slots;
                var (px, py) = (ringRadius * Math.Cos(angle), ringRadius * Math.Sin(angle));
                if (Math.Sqrt((rx - px) * (rx - px) + (ry - py) * (ry - py)) <= PetalLayout.DotRadius)
                    return palette.Pink;
            }

            return palette.Background;
        }
    }
}
