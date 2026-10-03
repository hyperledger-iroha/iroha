// The sixteen katakana of lane `K`.
//
// A tile's glyph is a four-bit symbol. The alphabet is the subset of the Iroha
// ordering whose members stay most distinguishable after camera blur:
// イ ロ ハ ニ ヘ ト ワ カ レ ム ノ ケ ア ヒ ス ン. Symbol 0 is イ and symbol 15 is ン.
//
// Glyphs are defined as stroke polylines on a 32x32 grid (`y` grows downward)
// drawn with round caps and joins. The strokes are the rendering definition;
// `TEMPLATES` is the derived, byte-exact matching table that every decoder
// embeds, so classification never depends on a rasteriser.

import { deepFreeze, requireIndex } from "./support.js";

/** Number of glyphs (symbols) in the alphabet. */
export const GLYPH_COUNT = 16;
/** Side of the glyph design grid. */
export const GLYPH_GRID = 32;
/** Stroke width on the design grid. */
export const STROKE_WIDTH = 6.5;
/** Side of the matching template grid. */
export const TEMPLATE_N = 8;

/** The sixteen characters in symbol order. */
export const GLYPH_CHARS = Object.freeze([
  "イ", "ロ", "ハ", "ニ", "ヘ", "ト", "ワ", "カ", "レ", "ム", "ノ", "ケ", "ア", "ヒ", "ス", "ン",
]);

/**
 * Stroke definitions for every glyph, in symbol order. Each stroke is a flat
 * polyline `[x0, y0, x1, y1, ...]` on the 32-unit design grid.
 */
export const STROKES = deepFreeze([
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
]);

// Segments of every glyph as flat `[ax, ay, bx, by, ...]` lists.
const SEGMENTS = STROKES.map((strokes) => {
  const segments = [];
  for (const stroke of strokes) {
    for (let index = 0; index + 3 < stroke.length; index += 2) {
      segments.push(stroke[index], stroke[index + 1], stroke[index + 2], stroke[index + 3]);
    }
  }
  return Float64Array.from(segments);
});

/** Distance from point `(px, py)` to the segment `a`-`b`. */
function segmentDistance(px, py, ax, ay, bx, by) {
  const dx = bx - ax;
  const dy = by - ay;
  const lengthSq = dx * dx + dy * dy;
  let t = 0;
  if (lengthSq !== 0) {
    t = ((px - ax) * dx + (py - ay) * dy) / lengthSq;
    t = t < 0 ? 0 : t > 1 ? 1 : t;
  }
  const ex = px - (ax + t * dx);
  const ey = py - (ay + t * dy);
  return Math.sqrt(ex * ex + ey * ey);
}

/** Returns whether design-grid point `(x, y)` is inked in `glyph`. */
export function isInked(glyph, x, y) {
  requireIndex(glyph, "glyph");
  if (glyph >= GLYPH_COUNT) {
    throw new RangeError("glyph out of range");
  }
  return inked(glyph, x, y);
}

/** Unchecked {@link isInked} for the renderer and the template generator. */
export function inked(glyph, x, y) {
  const radius = STROKE_WIDTH / 2;
  const segments = SEGMENTS[glyph];
  for (let index = 0; index < segments.length; index += 4) {
    if (
      segmentDistance(x, y, segments[index], segments[index + 1], segments[index + 2], segments[index + 3]) <=
      radius
    ) {
      return true;
    }
  }
  return false;
}

/**
 * Derives the matching templates from the stroke definitions.
 *
 * Each of the 8x8 cells holds the inked fraction of its 4x4 design cells,
 * sampled on a 16x16 grid and scaled to `0..=255`.
 *
 * @returns {Uint8Array[]} sixteen row-major 64-byte templates
 */
export function generateTemplates() {
  const supersample = 16;
  const cell = GLYPH_GRID / TEMPLATE_N;
  const out = [];
  for (let glyph = 0; glyph < GLYPH_COUNT; glyph += 1) {
    const table = new Uint8Array(TEMPLATE_N * TEMPLATE_N);
    for (let v = 0; v < TEMPLATE_N; v += 1) {
      for (let u = 0; u < TEMPLATE_N; u += 1) {
        let count = 0;
        for (let sy = 0; sy < supersample; sy += 1) {
          for (let sx = 0; sx < supersample; sx += 1) {
            const x = (u + (sx + 0.5) / supersample) * cell;
            const y = (v + (sy + 0.5) / supersample) * cell;
            if (inked(glyph, x, y)) {
              count += 1;
            }
          }
        }
        const coverage = count / (supersample * supersample);
        table[v * TEMPLATE_N + u] = Math.floor(coverage * 255 + 0.5);
      }
    }
    out.push(table);
  }
  return out;
}

/** Area-coverage templates (`0..=255`) of the glyph alphabet, row-major 8x8. */
export const TEMPLATES = deepFreeze([
  // イ
  [
    0, 0, 0, 0, 55, 195, 37, 0,
    0, 0, 0, 55, 240, 240, 37, 0,
    0, 0, 55, 240, 255, 142, 0, 0,
    0, 55, 240, 245, 255, 143, 0, 0,
    0, 195, 240, 71, 255, 143, 0, 0,
    0, 37, 37, 16, 255, 143, 0, 0,
    0, 0, 0, 16, 255, 143, 0, 0,
    0, 0, 0, 8, 238, 119, 0, 0,
  ],
  // ロ
  [
    3, 74, 80, 80, 80, 80, 74, 3,
    74, 255, 255, 255, 255, 255, 255, 74,
    80, 255, 134, 80, 80, 134, 255, 80,
    80, 255, 80, 0, 0, 80, 255, 80,
    80, 255, 80, 0, 0, 80, 255, 80,
    80, 255, 134, 80, 80, 134, 255, 80,
    74, 255, 255, 255, 255, 255, 255, 74,
    3, 74, 80, 80, 80, 80, 74, 3,
  ],
  // ハ
  [
    0, 0, 3, 68, 3, 0, 0, 0,
    0, 0, 94, 255, 123, 3, 0, 0,
    0, 0, 191, 255, 255, 108, 0, 0,
    0, 35, 254, 161, 221, 230, 12, 0,
    0, 130, 255, 58, 93, 255, 123, 0,
    2, 225, 215, 0, 2, 210, 239, 18,
    63, 255, 119, 0, 0, 78, 255, 124,
    17, 131, 17, 0, 0, 0, 119, 47,
  ],
  // ニ
  [
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 37, 80, 80, 80, 80, 37, 0,
    0, 195, 255, 255, 255, 255, 195, 0,
    0, 37, 80, 80, 80, 80, 37, 0,
    0, 0, 0, 0, 0, 0, 0, 0,
    134, 207, 207, 207, 207, 207, 207, 134,
    134, 207, 207, 207, 207, 207, 207, 134,
    0, 0, 0, 0, 0, 0, 0, 0,
  ],
  // ヘ
  [
    0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 37, 37, 0, 0, 0, 0,
    0, 55, 240, 244, 82, 0, 0, 0,
    55, 240, 240, 225, 254, 126, 1, 0,
    239, 240, 55, 22, 194, 255, 169, 10,
    119, 51, 0, 0, 6, 155, 255, 204,
    0, 0, 0, 0, 0, 0, 110, 182,
    0, 0, 0, 0, 0, 0, 0, 0,
  ],
  // ト
  [
    0, 8, 238, 119, 0, 0, 0, 0,
    0, 16, 255, 143, 0, 0, 0, 0,
    0, 16, 255, 156, 0, 0, 0, 0,
    0, 16, 255, 255, 188, 53, 0, 0,
    0, 16, 255, 224, 249, 254, 171, 17,
    0, 16, 255, 143, 33, 162, 251, 57,
    0, 16, 255, 143, 0, 0, 8, 0,
    0, 8, 238, 119, 0, 0, 0, 0,
  ],
  // ワ
  [
    0, 0, 0, 0, 0, 0, 0, 0,
    4, 182, 207, 207, 207, 207, 182, 4,
    16, 255, 234, 207, 207, 243, 247, 4,
    16, 255, 143, 0, 0, 216, 204, 0,
    16, 255, 143, 0, 81, 252, 158, 0,
    8, 238, 123, 138, 254, 226, 48, 0,
    0, 25, 193, 255, 185, 20, 0, 0,
    0, 57, 251, 128, 3, 0, 0, 0,
  ],
  // カ
  [
    0, 0, 57, 251, 57, 0, 0, 0,
    0, 0, 80, 255, 80, 0, 0, 0,
    134, 207, 222, 255, 222, 207, 182, 4,
    134, 207, 224, 255, 222, 234, 255, 16,
    0, 0, 167, 253, 40, 143, 255, 16,
    0, 42, 253, 167, 0, 172, 255, 16,
    0, 165, 253, 42, 90, 255, 174, 0,
    0, 134, 136, 0, 83, 182, 13, 0,
  ],
  // レ
  [
    0, 83, 182, 4, 0, 0, 0, 0,
    0, 143, 255, 16, 0, 19, 183, 83,
    0, 143, 255, 16, 15, 200, 254, 87,
    0, 143, 255, 26, 190, 255, 115, 0,
    0, 143, 255, 193, 255, 128, 0, 0,
    0, 143, 255, 255, 140, 0, 0, 0,
    0, 143, 255, 152, 1, 0, 0, 0,
    0, 47, 120, 3, 0, 0, 0, 0,
  ],
  // ム
  [
    0, 0, 0, 85, 182, 4, 0, 0,
    0, 0, 9, 229, 225, 4, 0, 0,
    0, 0, 117, 255, 96, 0, 0, 0,
    0, 16, 235, 213, 87, 183, 15, 0,
    0, 129, 255, 83, 90, 255, 182, 3,
    8, 243, 255, 249, 237, 252, 255, 101,
    0, 119, 153, 165, 177, 191, 205, 83,
    0, 0, 0, 0, 0, 0, 0, 0,
  ],
  // ノ
  [
    0, 0, 0, 0, 38, 195, 37, 0,
    0, 0, 0, 0, 150, 255, 43, 0,
    0, 0, 0, 14, 242, 192, 0, 0,
    0, 0, 0, 113, 255, 87, 0, 0,
    0, 0, 30, 241, 218, 5, 0, 0,
    0, 1, 185, 253, 61, 0, 0, 0,
    0, 95, 255, 143, 0, 0, 0, 0,
    0, 83, 182, 10, 0, 0, 0, 0,
  ],
  // ケ
  [
    0, 0, 142, 238, 8, 0, 0, 0,
    0, 70, 254, 182, 0, 0, 0, 0,
    18, 228, 255, 222, 207, 207, 207, 83,
    57, 251, 206, 225, 255, 223, 207, 83,
    0, 8, 0, 140, 255, 38, 0, 0,
    0, 0, 55, 240, 216, 0, 0, 0,
    0, 51, 240, 240, 55, 0, 0, 0,
    0, 119, 239, 55, 0, 0, 0, 0,
  ],
  // ア
  [
    0, 0, 0, 0, 0, 0, 0, 0,
    17, 80, 80, 80, 80, 80, 74, 3,
    131, 255, 255, 255, 255, 255, 255, 71,
    17, 80, 91, 255, 178, 161, 255, 50,
    0, 0, 16, 255, 164, 211, 253, 17,
    0, 0, 139, 255, 255, 254, 105, 0,
    0, 106, 255, 183, 182, 101, 0, 0,
    0, 182, 205, 13, 0, 0, 0, 0,
  ],
  // ヒ
  [
    0, 17, 131, 17, 0, 0, 0, 0,
    0, 80, 255, 80, 0, 0, 0, 0,
    0, 80, 255, 134, 80, 80, 57, 0,
    0, 80, 255, 255, 255, 255, 251, 8,
    0, 80, 255, 134, 80, 80, 57, 0,
    0, 80, 255, 134, 80, 80, 74, 3,
    0, 74, 255, 255, 255, 255, 255, 68,
    0, 3, 74, 80, 80, 80, 74, 3,
  ],
  // ス
  [
    17, 137, 143, 143, 143, 143, 83, 0,
    57, 253, 255, 255, 255, 255, 185, 0,
    0, 12, 16, 32, 220, 245, 40, 0,
    0, 0, 119, 253, 255, 107, 0, 0,
    0, 0, 156, 255, 254, 98, 0, 0,
    0, 116, 255, 189, 215, 255, 130, 1,
    57, 254, 200, 11, 17, 192, 255, 145,
    17, 131, 21, 0, 0, 6, 152, 134,
  ],
  // ン
  [
    0, 8, 0, 0, 0, 0, 0, 0,
    57, 251, 105, 0, 0, 1, 119, 47,
    17, 210, 254, 101, 0, 111, 255, 121,
    0, 21, 209, 182, 47, 248, 209, 8,
    0, 0, 4, 14, 214, 246, 42, 0,
    17, 133, 88, 158, 255, 104, 0, 0,
    57, 253, 255, 255, 174, 0, 0, 0,
    0, 24, 88, 133, 18, 0, 0, 0,
  ],
]);
