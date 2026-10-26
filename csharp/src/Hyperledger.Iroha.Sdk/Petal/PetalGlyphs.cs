namespace Hyperledger.Iroha.Petal;

/// <summary>
/// The sixteen katakana of the band tiles (see <see cref="PetalKana"/>).
/// </summary>
/// <remarks>
/// <para>
/// A band tile's glyph is a four-bit symbol. The alphabet is the subset of the Iroha ordering
/// <c>イロハニホヘト…ス</c> whose members stay most distinguishable after camera blur:
/// イ ロ ハ ニ ヘ ト ワ カ レ ム ノ ケ ア ヒ ス ン. Symbol <c>0</c> is <c>イ</c> and symbol
/// <c>15</c> is <c>ン</c>.
/// </para>
/// <para>
/// Glyphs are defined as stroke polylines on a 32 × 32 grid (<c>y</c> grows downward) drawn with
/// round caps and joins of width <see cref="StrokeWidth"/>. A point is inked when it lies within
/// half a stroke width of a stroke segment (<see cref="IsInked"/>); renderers and the band reader's
/// templates both follow that definition, so classification never depends on a rasteriser.
/// </para>
/// </remarks>
public static class PetalGlyphs
{
    /// <summary>Number of glyphs (symbols) in the alphabet.</summary>
    public const int GlyphCount = 16;

    /// <summary>Side of the glyph design grid.</summary>
    public const double GlyphGrid = 32.0;

    /// <summary>Stroke width on the design grid.</summary>
    public const double StrokeWidth = 6.5;

    /// <summary>The sixteen characters in symbol order.</summary>
    public const string GlyphChars = "イロハニヘトワカレムノケアヒスン";

    /// <summary>
    /// Stroke definitions for every glyph, in symbol order: each glyph is a list of strokes and each
    /// stroke a polyline of <c>x0, y0, x1, y1, …</c> on the design grid (the reference's
    /// single-precision values, all whole numbers, so they widen exactly).
    /// </summary>
    private static readonly double[][][] StrokeTable =
    [
        // イ
        [[22, 4, 8, 18], [19, 10, 19, 29]],
        // ロ
        [[6, 6, 26, 6, 26, 26, 6, 26, 6, 6]],
        // ハ
        [[14, 6, 6, 27], [18, 10, 27, 27]],
        // ニ
        [[8, 10, 24, 10], [4, 24, 28, 24]],
        // ヘ
        [[3, 19, 12, 10, 29, 24]],
        // ト
        [[11, 3, 11, 29], [11, 13, 26, 21]],
        // ワ
        [[7, 21, 7, 8, 25, 8, 23, 19, 10, 29]],
        // カ
        [[14, 3, 14, 16, 8, 28], [4, 12, 25, 12, 25, 23, 21, 28]],
        // レ
        [[9, 4, 9, 27, 27, 8]],
        // ム
        [[17, 4, 7, 23, 27, 24], [21, 16, 26, 22]],
        // ノ
        [[22, 4, 17, 16, 9, 28]],
        // ケ
        [[13, 3, 6, 13], [10, 12, 27, 12], [19, 12, 16, 22, 9, 29]],
        // ア
        [[5, 10, 26, 10, 25, 18, 19, 24], [15, 10, 15, 21, 8, 29]],
        // ヒ
        [[10, 5, 10, 26, 26, 26], [10, 14, 25, 14]],
        // ス
        [[6, 5, 24, 5, 17, 15, 6, 27], [13, 15, 28, 28]],
        // ン
        [[6, 7, 12, 13], [6, 25, 14, 27, 27, 9]],
    ];

    private static readonly IReadOnlyList<IReadOnlyList<PetalPoint>>[] StrokeViews = BuildViews(0.0, 1.0);

    /// <summary>
    /// The strokes of <paramref name="glyph"/>: polylines of points on the 32 × 32 design grid, drawn
    /// with round caps and joins of width <see cref="StrokeWidth"/>.
    /// </summary>
    /// <param name="glyph">Glyph (symbol), 0 to 15.</param>
    /// <returns>The strokes, in the reference's order.</returns>
    public static IReadOnlyList<IReadOnlyList<PetalPoint>> Strokes(int glyph)
    {
        CheckGlyph(glyph);
        return StrokeViews[glyph];
    }

    /// <summary>
    /// Whether the design-grid point <c>(x, y)</c> is inked in <paramref name="glyph"/>: within half a
    /// stroke width (3.25) of one of its stroke segments.
    /// </summary>
    /// <param name="glyph">Glyph (symbol), 0 to 15.</param>
    /// <param name="x">Design-grid x.</param>
    /// <param name="y">Design-grid y.</param>
    /// <returns><see langword="true"/> when the point is inked.</returns>
    public static bool IsInked(int glyph, double x, double y)
    {
        CheckGlyph(glyph);
        return Inked(glyph, x, y);
    }

    /// <summary><see cref="IsInked"/> without the range check.</summary>
    internal static bool Inked(int glyph, double x, double y)
    {
        const double radius = StrokeWidth / 2.0;
        foreach (var stroke in StrokeTable[glyph])
        {
            for (var at = 0; at + 3 < stroke.Length; at += 2)
            {
                if (SegmentDistance(x, y, stroke[at], stroke[at + 1], stroke[at + 2], stroke[at + 3]) <= radius)
                    return true;
            }
        }

        return false;
    }

    /// <summary>
    /// The strokes of every glyph mapped by <c>offset + p · scale</c> on both axes (the draw list's
    /// tile-local strokes use the glyph box's inset and scale).
    /// </summary>
    internal static IReadOnlyList<IReadOnlyList<PetalPoint>>[] BuildViews(double offset, double scale)
    {
        var views = new IReadOnlyList<IReadOnlyList<PetalPoint>>[GlyphCount];
        for (var glyph = 0; glyph < GlyphCount; glyph++)
        {
            var strokes = StrokeTable[glyph];
            var lines = new IReadOnlyList<PetalPoint>[strokes.Length];
            for (var s = 0; s < strokes.Length; s++)
            {
                var points = new PetalPoint[strokes[s].Length / 2];
                for (var p = 0; p < points.Length; p++)
                    points[p] = new PetalPoint(offset + strokes[s][2 * p] * scale, offset + strokes[s][2 * p + 1] * scale);
                lines[s] = Array.AsReadOnly(points);
            }

            views[glyph] = Array.AsReadOnly(lines);
        }

        return views;
    }

    /// <summary>Distance from point <c>(px, py)</c> to the segment <c>(ax, ay)</c>–<c>(bx, by)</c>, as the reference computes it.</summary>
    private static double SegmentDistance(double px, double py, double ax, double ay, double bx, double by)
    {
        var (dx, dy) = (bx - ax, by - ay);
        var lengthSq = dx * dx + dy * dy;
        var t = lengthSq == 0.0 ? 0.0 : Math.Clamp(((px - ax) * dx + (py - ay) * dy) / lengthSq, 0.0, 1.0);
        var ex = px - (ax + t * dx);
        var ey = py - (ay + t * dy);
        return Math.Sqrt(ex * ex + ey * ey);
    }

    private static void CheckGlyph(int glyph)
    {
        if ((uint)glyph >= GlyphCount)
            throw new ArgumentOutOfRangeException(nameof(glyph), "A glyph is 0 to 15.");
    }
}
