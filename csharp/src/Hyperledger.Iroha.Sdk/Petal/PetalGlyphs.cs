namespace Hyperledger.Iroha.Petal;

/// <summary>
/// The sixteen katakana of lane <c>K</c>.
/// </summary>
/// <remarks>
/// A tile's glyph is a four-bit symbol. The alphabet is the subset of the Iroha
/// ordering whose members stay most distinguishable after camera blur:
/// イ ロ ハ ニ ヘ ト ワ カ レ ム ノ ケ ア ヒ ス ン (symbol 0 is イ, symbol 15 is ン).
/// Glyphs are stroke polylines on a 32×32 grid (<c>y</c> grows downward) drawn
/// with round caps and joins. The strokes are the rendering definition; the
/// 8×8 coverage <see cref="Template(int)"/> tables are the derived, byte-exact
/// matching tables every decoder embeds, so classification never depends on a
/// rasteriser.
/// </remarks>
public static class PetalGlyphs
{
    /// <summary>Number of glyphs (symbols) in the alphabet.</summary>
    public const int GlyphCount = 16;

    /// <summary>Side of the glyph design grid.</summary>
    public const double GlyphGrid = 32.0;

    /// <summary>Stroke width on the design grid.</summary>
    public const double StrokeWidth = 6.5;

    /// <summary>Side of the matching template grid.</summary>
    public const int TemplateSize = 8;

    /// <summary>The sixteen characters in symbol order.</summary>
    public const string Characters = "イロハニヘトワカレムノケアヒスン";

    private static readonly PetalPoint[][][] StrokeTable =
    [
        // イ
        [Line(22, 4, 8, 18), Line(19, 10, 19, 29)],
        // ロ
        [Line(6, 6, 26, 6, 26, 26, 6, 26, 6, 6)],
        // ハ
        [Line(14, 6, 6, 27), Line(18, 10, 27, 27)],
        // ニ
        [Line(8, 10, 24, 10), Line(4, 24, 28, 24)],
        // ヘ
        [Line(3, 19, 12, 10, 29, 24)],
        // ト
        [Line(11, 3, 11, 29), Line(11, 13, 26, 21)],
        // ワ
        [Line(7, 21, 7, 8, 25, 8, 23, 19, 10, 29)],
        // カ
        [Line(14, 3, 14, 16, 8, 28), Line(4, 12, 25, 12, 25, 23, 21, 28)],
        // レ
        [Line(9, 4, 9, 27, 27, 8)],
        // ム
        [Line(17, 4, 7, 23, 27, 24), Line(21, 16, 26, 22)],
        // ノ
        [Line(22, 4, 17, 16, 9, 28)],
        // ケ
        [Line(13, 3, 6, 13), Line(10, 12, 27, 12), Line(19, 12, 16, 22, 9, 29)],
        // ア
        [Line(5, 10, 26, 10, 25, 18, 19, 24), Line(15, 10, 15, 21, 8, 29)],
        // ヒ
        [Line(10, 5, 10, 26, 26, 26), Line(10, 14, 25, 14)],
        // ス
        [Line(6, 5, 24, 5, 17, 15, 6, 27), Line(13, 15, 28, 28)],
        // ン
        [Line(6, 7, 12, 13), Line(6, 25, 14, 27, 27, 9)],
    ];

    /// <summary>
    /// Area-coverage templates (<c>0..=255</c>) of the alphabet, row-major 8×8
    /// per glyph, in symbol order. Generated from the strokes by
    /// <see cref="GenerateTemplates"/> and pinned by the shared fixtures.
    /// </summary>
    private static ReadOnlySpan<byte> TemplateData =>
    [
        // イ
          0,   0,   0,   0,  55, 195,  37,   0,
          0,   0,   0,  55, 240, 240,  37,   0,
          0,   0,  55, 240, 255, 142,   0,   0,
          0,  55, 240, 245, 255, 143,   0,   0,
          0, 195, 240,  71, 255, 143,   0,   0,
          0,  37,  37,  16, 255, 143,   0,   0,
          0,   0,   0,  16, 255, 143,   0,   0,
          0,   0,   0,   8, 238, 119,   0,   0,
        // ロ
          3,  74,  80,  80,  80,  80,  74,   3,
         74, 255, 255, 255, 255, 255, 255,  74,
         80, 255, 134,  80,  80, 134, 255,  80,
         80, 255,  80,   0,   0,  80, 255,  80,
         80, 255,  80,   0,   0,  80, 255,  80,
         80, 255, 134,  80,  80, 134, 255,  80,
         74, 255, 255, 255, 255, 255, 255,  74,
          3,  74,  80,  80,  80,  80,  74,   3,
        // ハ
          0,   0,   3,  68,   3,   0,   0,   0,
          0,   0,  94, 255, 123,   3,   0,   0,
          0,   0, 191, 255, 255, 108,   0,   0,
          0,  35, 254, 161, 221, 230,  12,   0,
          0, 130, 255,  58,  93, 255, 123,   0,
          2, 225, 215,   0,   2, 210, 239,  18,
         63, 255, 119,   0,   0,  78, 255, 124,
         17, 131,  17,   0,   0,   0, 119,  47,
        // ニ
          0,   0,   0,   0,   0,   0,   0,   0,
          0,  37,  80,  80,  80,  80,  37,   0,
          0, 195, 255, 255, 255, 255, 195,   0,
          0,  37,  80,  80,  80,  80,  37,   0,
          0,   0,   0,   0,   0,   0,   0,   0,
        134, 207, 207, 207, 207, 207, 207, 134,
        134, 207, 207, 207, 207, 207, 207, 134,
          0,   0,   0,   0,   0,   0,   0,   0,
        // ヘ
          0,   0,   0,   0,   0,   0,   0,   0,
          0,   0,  37,  37,   0,   0,   0,   0,
          0,  55, 240, 244,  82,   0,   0,   0,
         55, 240, 240, 225, 254, 126,   1,   0,
        239, 240,  55,  22, 194, 255, 169,  10,
        119,  51,   0,   0,   6, 155, 255, 204,
          0,   0,   0,   0,   0,   0, 110, 182,
          0,   0,   0,   0,   0,   0,   0,   0,
        // ト
          0,   8, 238, 119,   0,   0,   0,   0,
          0,  16, 255, 143,   0,   0,   0,   0,
          0,  16, 255, 156,   0,   0,   0,   0,
          0,  16, 255, 255, 188,  53,   0,   0,
          0,  16, 255, 224, 249, 254, 171,  17,
          0,  16, 255, 143,  33, 162, 251,  57,
          0,  16, 255, 143,   0,   0,   8,   0,
          0,   8, 238, 119,   0,   0,   0,   0,
        // ワ
          0,   0,   0,   0,   0,   0,   0,   0,
          4, 182, 207, 207, 207, 207, 182,   4,
         16, 255, 234, 207, 207, 243, 247,   4,
         16, 255, 143,   0,   0, 216, 204,   0,
         16, 255, 143,   0,  81, 252, 158,   0,
          8, 238, 123, 138, 254, 226,  48,   0,
          0,  25, 193, 255, 185,  20,   0,   0,
          0,  57, 251, 128,   3,   0,   0,   0,
        // カ
          0,   0,  57, 251,  57,   0,   0,   0,
          0,   0,  80, 255,  80,   0,   0,   0,
        134, 207, 222, 255, 222, 207, 182,   4,
        134, 207, 224, 255, 222, 234, 255,  16,
          0,   0, 167, 253,  40, 143, 255,  16,
          0,  42, 253, 167,   0, 172, 255,  16,
          0, 165, 253,  42,  90, 255, 174,   0,
          0, 134, 136,   0,  83, 182,  13,   0,
        // レ
          0,  83, 182,   4,   0,   0,   0,   0,
          0, 143, 255,  16,   0,  19, 183,  83,
          0, 143, 255,  16,  15, 200, 254,  87,
          0, 143, 255,  26, 190, 255, 115,   0,
          0, 143, 255, 193, 255, 128,   0,   0,
          0, 143, 255, 255, 140,   0,   0,   0,
          0, 143, 255, 152,   1,   0,   0,   0,
          0,  47, 120,   3,   0,   0,   0,   0,
        // ム
          0,   0,   0,  85, 182,   4,   0,   0,
          0,   0,   9, 229, 225,   4,   0,   0,
          0,   0, 117, 255,  96,   0,   0,   0,
          0,  16, 235, 213,  87, 183,  15,   0,
          0, 129, 255,  83,  90, 255, 182,   3,
          8, 243, 255, 249, 237, 252, 255, 101,
          0, 119, 153, 165, 177, 191, 205,  83,
          0,   0,   0,   0,   0,   0,   0,   0,
        // ノ
          0,   0,   0,   0,  38, 195,  37,   0,
          0,   0,   0,   0, 150, 255,  43,   0,
          0,   0,   0,  14, 242, 192,   0,   0,
          0,   0,   0, 113, 255,  87,   0,   0,
          0,   0,  30, 241, 218,   5,   0,   0,
          0,   1, 185, 253,  61,   0,   0,   0,
          0,  95, 255, 143,   0,   0,   0,   0,
          0,  83, 182,  10,   0,   0,   0,   0,
        // ケ
          0,   0, 142, 238,   8,   0,   0,   0,
          0,  70, 254, 182,   0,   0,   0,   0,
         18, 228, 255, 222, 207, 207, 207,  83,
         57, 251, 206, 225, 255, 223, 207,  83,
          0,   8,   0, 140, 255,  38,   0,   0,
          0,   0,  55, 240, 216,   0,   0,   0,
          0,  51, 240, 240,  55,   0,   0,   0,
          0, 119, 239,  55,   0,   0,   0,   0,
        // ア
          0,   0,   0,   0,   0,   0,   0,   0,
         17,  80,  80,  80,  80,  80,  74,   3,
        131, 255, 255, 255, 255, 255, 255,  71,
         17,  80,  91, 255, 178, 161, 255,  50,
          0,   0,  16, 255, 164, 211, 253,  17,
          0,   0, 139, 255, 255, 254, 105,   0,
          0, 106, 255, 183, 182, 101,   0,   0,
          0, 182, 205,  13,   0,   0,   0,   0,
        // ヒ
          0,  17, 131,  17,   0,   0,   0,   0,
          0,  80, 255,  80,   0,   0,   0,   0,
          0,  80, 255, 134,  80,  80,  57,   0,
          0,  80, 255, 255, 255, 255, 251,   8,
          0,  80, 255, 134,  80,  80,  57,   0,
          0,  80, 255, 134,  80,  80,  74,   3,
          0,  74, 255, 255, 255, 255, 255,  68,
          0,   3,  74,  80,  80,  80,  74,   3,
        // ス
         17, 137, 143, 143, 143, 143,  83,   0,
         57, 253, 255, 255, 255, 255, 185,   0,
          0,  12,  16,  32, 220, 245,  40,   0,
          0,   0, 119, 253, 255, 107,   0,   0,
          0,   0, 156, 255, 254,  98,   0,   0,
          0, 116, 255, 189, 215, 255, 130,   1,
         57, 254, 200,  11,  17, 192, 255, 145,
         17, 131,  21,   0,   0,   6, 152, 134,
        // ン
          0,   8,   0,   0,   0,   0,   0,   0,
         57, 251, 105,   0,   0,   1, 119,  47,
         17, 210, 254, 101,   0, 111, 255, 121,
          0,  21, 209, 182,  47, 248, 209,   8,
          0,   0,   4,  14, 214, 246,  42,   0,
         17, 133,  88, 158, 255, 104,   0,   0,
         57, 253, 255, 255, 174,   0,   0,   0,
          0,  24,  88, 133,  18,   0,   0,   0,
    ];

    private static readonly IReadOnlyList<IReadOnlyList<PetalPoint>>[] StrokeViews = StrokeTable
        .Select(static strokes => (IReadOnlyList<IReadOnlyList<PetalPoint>>)Array.AsReadOnly(
            strokes.Select(static stroke => (IReadOnlyList<PetalPoint>)Array.AsReadOnly(stroke)).ToArray()))
        .ToArray();

    /// <summary>Stroke polylines of <paramref name="glyph"/> on the 32×32 design grid.</summary>
    /// <param name="glyph">Symbol, 0 to 15.</param>
    /// <returns>One read-only point list per stroke.</returns>
    public static IReadOnlyList<IReadOnlyList<PetalPoint>> Strokes(int glyph)
    {
        CheckGlyph(glyph);
        return StrokeViews[glyph];
    }

    /// <summary>The 8×8 coverage template of <paramref name="glyph"/>, row-major.</summary>
    /// <param name="glyph">Symbol, 0 to 15.</param>
    /// <returns>64 coverage values in <c>0..=255</c>.</returns>
    public static ReadOnlySpan<byte> Template(int glyph)
    {
        CheckGlyph(glyph);
        return TemplateData.Slice(glyph * TemplateSize * TemplateSize, TemplateSize * TemplateSize);
    }

    /// <summary>Returns whether design-grid point <c>(x, y)</c> is inked in <paramref name="glyph"/>.</summary>
    /// <param name="glyph">Symbol, 0 to 15.</param>
    /// <param name="x">Design-grid x.</param>
    /// <param name="y">Design-grid y.</param>
    /// <returns><see langword="true"/> within half a stroke width of any stroke segment.</returns>
    public static bool IsInked(int glyph, double x, double y)
    {
        CheckGlyph(glyph);
        var radius = StrokeWidth / 2.0;
        foreach (var stroke in StrokeTable[glyph])
        {
            for (var i = 0; i + 1 < stroke.Length; i++)
            {
                if (SegmentDistance(x, y, stroke[i], stroke[i + 1]) <= radius)
                    return true;
            }
        }

        return false;
    }

    /// <summary>
    /// Derives the matching templates from the stroke definitions.
    /// </summary>
    /// <remarks>
    /// Each of the 8×8 cells holds the inked fraction of its 4×4 design cells,
    /// sampled on a 16×16 grid and scaled to <c>0..=255</c>.
    /// </remarks>
    /// <returns>Sixteen 64-byte templates in symbol order.</returns>
    public static byte[][] GenerateTemplates()
    {
        const int super = 16;
        var cell = GlyphGrid / TemplateSize;
        var output = new byte[GlyphCount][];
        for (var glyph = 0; glyph < GlyphCount; glyph++)
        {
            var table = new byte[TemplateSize * TemplateSize];
            for (var v = 0; v < TemplateSize; v++)
            {
                for (var u = 0; u < TemplateSize; u++)
                {
                    var inked = 0u;
                    for (var sy = 0; sy < super; sy++)
                    {
                        for (var sx = 0; sx < super; sx++)
                        {
                            var x = (u + (sx + 0.5) / super) * cell;
                            var y = (v + (sy + 0.5) / super) * cell;
                            if (IsInked(glyph, x, y))
                                inked++;
                        }
                    }

                    var coverage = (double)inked / (super * super);
                    table[v * TemplateSize + u] = (byte)Math.Floor(coverage * 255.0 + 0.5);
                }
            }

            output[glyph] = table;
        }

        return output;
    }

    /// <summary>Distance from point <c>(px, py)</c> to the segment <paramref name="a"/>–<paramref name="b"/>.</summary>
    private static double SegmentDistance(double px, double py, PetalPoint a, PetalPoint b)
    {
        var (ax, ay, bx, by) = (a.X, a.Y, b.X, b.Y);
        var (dx, dy) = (bx - ax, by - ay);
        var lengthSquared = dx * dx + dy * dy;
        var t = lengthSquared == 0.0
            ? 0.0
            : Math.Clamp(((px - ax) * dx + (py - ay) * dy) / lengthSquared, 0.0, 1.0);
        var ex = px - (ax + t * dx);
        var ey = py - (ay + t * dy);
        return Math.Sqrt(ex * ex + ey * ey);
    }

    private static PetalPoint[] Line(params double[] coordinates)
    {
        var points = new PetalPoint[coordinates.Length / 2];
        for (var i = 0; i < points.Length; i++)
            points[i] = new PetalPoint(coordinates[2 * i], coordinates[2 * i + 1]);
        return points;
    }

    private static void CheckGlyph(int glyph)
    {
        if ((uint)glyph >= GlyphCount)
            throw new ArgumentOutOfRangeException(nameof(glyph));
    }
}
