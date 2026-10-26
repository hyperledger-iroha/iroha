import Foundation

/// The sixteen katakana of the band tiles (port of
/// `crates/iroha_petal/src/glyphs.rs`, see ``PetalKana``).
///
/// A band tile's glyph is a four-bit symbol. The alphabet is the subset of the
/// Iroha ordering `イロハニホヘト…ス` whose members stay most distinguishable
/// after camera blur: イ ロ ハ ニ ヘ ト ワ カ レ ム ノ ケ ア ヒ ス ン. Symbol `0`
/// is `イ` and symbol `15` is `ン`.
///
/// Glyphs are defined as stroke polylines on a 32 × 32 grid (`y` grows
/// downward) drawn with round caps and joins of width ``strokeWidth``. A point
/// is inked when it lies within half a stroke width of a stroke segment
/// (``isInked(_:x:y:)``); renderers and the band reader's templates both
/// follow that definition, so classification never depends on a rasteriser.
public enum PetalGlyphs {
    /// Number of glyphs (symbols) in the alphabet.
    public static let glyphCount = 16
    /// Side of the glyph design grid.
    public static let glyphGrid: Double = 32
    /// Stroke width on the design grid.
    public static let strokeWidth: Double = 6.5
    /// The sixteen characters in symbol order.
    public static let characters: [Character] = Array("イロハニヘトワカレムノケアヒスン")

    /// Stroke definitions for every glyph, in symbol order: each glyph is a
    /// list of strokes and each stroke a polyline on the design grid.
    static let strokeTable: [[[PetalPoint]]] = [
        // イ
        [[p(22, 4), p(8, 18)], [p(19, 10), p(19, 29)]],
        // ロ
        [[p(6, 6), p(26, 6), p(26, 26), p(6, 26), p(6, 6)]],
        // ハ
        [[p(14, 6), p(6, 27)], [p(18, 10), p(27, 27)]],
        // ニ
        [[p(8, 10), p(24, 10)], [p(4, 24), p(28, 24)]],
        // ヘ
        [[p(3, 19), p(12, 10), p(29, 24)]],
        // ト
        [[p(11, 3), p(11, 29)], [p(11, 13), p(26, 21)]],
        // ワ
        [[p(7, 21), p(7, 8), p(25, 8), p(23, 19), p(10, 29)]],
        // カ
        [[p(14, 3), p(14, 16), p(8, 28)], [p(4, 12), p(25, 12), p(25, 23), p(21, 28)]],
        // レ
        [[p(9, 4), p(9, 27), p(27, 8)]],
        // ム
        [[p(17, 4), p(7, 23), p(27, 24)], [p(21, 16), p(26, 22)]],
        // ノ
        [[p(22, 4), p(17, 16), p(9, 28)]],
        // ケ
        [[p(13, 3), p(6, 13)], [p(10, 12), p(27, 12)], [p(19, 12), p(16, 22), p(9, 29)]],
        // ア
        [[p(5, 10), p(26, 10), p(25, 18), p(19, 24)], [p(15, 10), p(15, 21), p(8, 29)]],
        // ヒ
        [[p(10, 5), p(10, 26), p(26, 26)], [p(10, 14), p(25, 14)]],
        // ス
        [[p(6, 5), p(24, 5), p(17, 15), p(6, 27)], [p(13, 15), p(28, 28)]],
        // ン
        [[p(6, 7), p(12, 13)], [p(6, 25), p(14, 27), p(27, 9)]],
    ]

    private static func p(_ x: Double, _ y: Double) -> PetalPoint {
        PetalPoint(x: x, y: y)
    }

    /// The strokes of `glyph` (`0..<16`), each a polyline on the design grid;
    /// empty for a glyph out of range.
    public static func strokes(_ glyph: Int) -> [[PetalPoint]] {
        (0..<glyphCount).contains(glyph) ? strokeTable[glyph] : []
    }

    /// Whether the design-grid point `(x, y)` is inked in `glyph` (`0..<16`);
    /// `false` for a glyph out of range.
    public static func isInked(_ glyph: Int, x: Double, y: Double) -> Bool {
        (0..<glyphCount).contains(glyph) && inked(glyph, x, y)
    }

    /// ``isInked(_:x:y:)`` without the range check.
    static func inked(_ glyph: Int, _ x: Double, _ y: Double) -> Bool {
        let radius = strokeWidth / 2.0
        for stroke in strokeTable[glyph] {
            for index in 1..<stroke.count
            where segmentDistance(x, y, stroke[index - 1], stroke[index]) <= radius {
                return true
            }
        }
        return false
    }

    /// Distance from the point `(px, py)` to the segment `a`–`b`.
    private static func segmentDistance(_ px: Double, _ py: Double, _ a: PetalPoint, _ b: PetalPoint) -> Double {
        let dx = b.x - a.x
        let dy = b.y - a.y
        let lengthSq = dx * dx + dy * dy
        let t = lengthSq == 0.0 ? 0.0 : PetalNumeric.clamp(((px - a.x) * dx + (py - a.y) * dy) / lengthSq, 0.0, 1.0)
        let ex = px - (a.x + t * dx)
        let ey = py - (a.y + t * dy)
        return (ex * ex + ey * ey).squareRoot()
    }
}
