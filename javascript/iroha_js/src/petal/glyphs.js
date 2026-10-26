// The sixteen katakana of the band tiles.
//
// A band tile's glyph is a four-bit symbol. The alphabet is the subset of the
// Iroha ordering `イロハニホヘト…ス` whose members stay most distinguishable
// after camera blur: イ ロ ハ ニ ヘ ト ワ カ レ ム ノ ケ ア ヒ ス ン. Symbol `0` is
// `イ` and symbol `15` is `ン`.
//
// Glyphs are defined as stroke polylines on a 32 x 32 grid (`y` grows
// downward) drawn with round caps and joins of width `STROKE_WIDTH`. A point is
// inked when it lies within half a stroke width of a stroke segment
// (`isGlyphInked`); renderers and the band reader's templates both follow that
// definition, so classification never depends on a rasteriser.

import { deepFreeze, requireIndex } from "./support.js";

/** Number of glyphs (symbols) in the alphabet. */
export const GLYPH_COUNT = 16;
/** Side of the glyph design grid. */
export const GLYPH_GRID = 32;
/** Stroke width on the design grid. */
export const STROKE_WIDTH = 6.5;
/** The sixteen characters in symbol order. */
export const GLYPH_CHARS = "イロハニヘトワカレムノケアヒスン";

/**
 * Stroke definitions for every glyph, in symbol order: each glyph is a list of strokes and each
 * stroke a polyline of `[x, y]` points on the design grid.
 */
export const GLYPH_STROKES = deepFreeze([
  // イ
  [
    [[22, 4], [8, 18]],
    [[19, 10], [19, 29]],
  ],
  // ロ
  [[[6, 6], [26, 6], [26, 26], [6, 26], [6, 6]]],
  // ハ
  [
    [[14, 6], [6, 27]],
    [[18, 10], [27, 27]],
  ],
  // ニ
  [
    [[8, 10], [24, 10]],
    [[4, 24], [28, 24]],
  ],
  // ヘ
  [[[3, 19], [12, 10], [29, 24]]],
  // ト
  [
    [[11, 3], [11, 29]],
    [[11, 13], [26, 21]],
  ],
  // ワ
  [[[7, 21], [7, 8], [25, 8], [23, 19], [10, 29]]],
  // カ
  [
    [[14, 3], [14, 16], [8, 28]],
    [[4, 12], [25, 12], [25, 23], [21, 28]],
  ],
  // レ
  [[[9, 4], [9, 27], [27, 8]]],
  // ム
  [
    [[17, 4], [7, 23], [27, 24]],
    [[21, 16], [26, 22]],
  ],
  // ノ
  [[[22, 4], [17, 16], [9, 28]]],
  // ケ
  [
    [[13, 3], [6, 13]],
    [[10, 12], [27, 12]],
    [[19, 12], [16, 22], [9, 29]],
  ],
  // ア
  [
    [[5, 10], [26, 10], [25, 18], [19, 24]],
    [[15, 10], [15, 21], [8, 29]],
  ],
  // ヒ
  [
    [[10, 5], [10, 26], [26, 26]],
    [[10, 14], [25, 14]],
  ],
  // ス
  [
    [[6, 5], [24, 5], [17, 15], [6, 27]],
    [[13, 15], [28, 28]],
  ],
  // ン
  [
    [[6, 7], [12, 13]],
    [[6, 25], [14, 27], [27, 9]],
  ],
]);

if (GLYPH_STROKES.length !== GLYPH_COUNT || Array.from(GLYPH_CHARS).length !== GLYPH_COUNT) {
  throw new Error("the glyph alphabet has sixteen glyphs");
}

/** Every glyph's segments as flat `ax, ay, bx, by` runs, `SEGMENTS[glyph]`. */
const SEGMENTS = GLYPH_STROKES.map((strokes) => {
  const flat = [];
  for (const stroke of strokes) {
    for (let index = 0; index + 1 < stroke.length; index += 1) {
      flat.push(stroke[index][0], stroke[index][1], stroke[index + 1][0], stroke[index + 1][1]);
    }
  }
  return Float64Array.from(flat);
});

/** Half a stroke width: a point this close to a segment is inked. */
const RADIUS = STROKE_WIDTH / 2;

/** Distance from point `(px, py)` to the segment `(ax, ay)`-`(bx, by)`, as the reference computes it. */
function segmentDistance(px, py, ax, ay, bx, by) {
  const dx = bx - ax;
  const dy = by - ay;
  const lengthSq = dx * dx + dy * dy;
  let t = 0;
  if (lengthSq !== 0) {
    t = ((px - ax) * dx + (py - ay) * dy) / lengthSq;
    // Rust `f64::clamp(0, 1)` (NaN stays NaN)
    if (t < 0) t = 0;
    else if (t > 1) t = 1;
  }
  const ex = px - (ax + t * dx);
  const ey = py - (ay + t * dy);
  return Math.sqrt(ex * ex + ey * ey);
}

/** {@link isGlyphInked} without the argument checks (`glyph` is `0..15`). */
export function glyphInkedAt(glyph, x, y) {
  const segments = SEGMENTS[glyph];
  for (let index = 0; index < segments.length; index += 4) {
    if (segmentDistance(x, y, segments[index], segments[index + 1], segments[index + 2], segments[index + 3]) <= RADIUS) {
      return true;
    }
  }
  return false;
}

/** Validates a glyph symbol (`0..15`). */
export function requireGlyph(glyph) {
  requireIndex(glyph, "glyph");
  if (glyph >= GLYPH_COUNT) {
    throw new RangeError("glyph out of range");
  }
  return glyph;
}

/** Whether the design-grid point `(x, y)` is inked in `glyph` (`0..15`). */
export function isGlyphInked(glyph, x, y) {
  requireGlyph(glyph);
  if (typeof x !== "number" || typeof y !== "number") {
    throw new TypeError("glyph coordinates must be numbers");
  }
  return glyphInkedAt(glyph, x, y);
}

/** The strokes of `glyph` (`0..15`): polylines of `[x, y]` points on the design grid (a copy). */
export function glyphStrokes(glyph) {
  return GLYPH_STROKES[requireGlyph(glyph)].map((stroke) => stroke.map((point) => [point[0], point[1]]));
}
