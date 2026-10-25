namespace Hyperledger.Iroha.Petal;

/// <summary>An 8-bit sRGB colour.</summary>
/// <param name="R">Red.</param>
/// <param name="G">Green.</param>
/// <param name="B">Blue.</param>
public readonly record struct PetalColor(byte R, byte G, byte B);

/// <summary>Colours of a Petal picture.</summary>
/// <param name="Background">Background (black by default).</param>
/// <param name="Light">Finders and the petals inside the <c>天</c>.</param>
/// <param name="Pink">Sakura pink of the ring dots and of the petals outside the <c>天</c>.</param>
/// <param name="Backdrop">The faint <c>天</c> drawn under the petals (a tenth of the light colour).</param>
public sealed record PetalPalette(PetalColor Background, PetalColor Light, PetalColor Pink, PetalColor Backdrop)
{
    /// <summary>
    /// The reference palette: background <c>(0, 0, 0)</c>, light
    /// <c>(250, 235, 244)</c>, pink <c>(245, 175, 208)</c>, backdrop <c>(25, 24, 24)</c>.
    /// </summary>
    public static PetalPalette Default { get; } = new(
        new PetalColor(0, 0, 0),
        new PetalColor(250, 235, 244),
        new PetalColor(245, 175, 208),
        new PetalColor(25, 24, 24));
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
/// The picture is black, with four sakura-blossom finders in the canvas corners,
/// three dotted rings, and inside them a field of petals on concentric bands. A
/// lit petal whose centre lies inside the <c>天</c> is drawn in the light colour,
/// the others in sakura pink, over a faint backdrop of the <c>天</c> itself, so the
/// character shows through the storm of petals. Each sample is shaded in this
/// order: finders, ring dots, petals, the backdrop, background. The output matches
/// the Rust reference renderer pixel for pixel. Platform code may instead draw a
/// <see cref="PetalDrawList"/> with native 2D APIs, as long as geometry and
/// polarity match.
/// </remarks>
public static class PetalRenderer
{
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
        var shader = new Shader(cells, options.Palette);
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
        return new Shader(cells, palette ?? PetalPalette.Default).Shade(x, y);
    }

    private readonly struct Shader(PetalFrameCells cells, PetalPalette palette)
    {
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
                    if (PetalLayout.PetalLit(x - xs[cell], y - ys[cell], PetalLayout.CellSineTable[cell], PetalLayout.CellCosineTable[cell]))
                        return PetalLayout.CellLightTable[cell] ? palette.Light : palette.Pink;
                }
            }

            return null;
        }
    }
}
