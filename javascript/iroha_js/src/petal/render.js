// Reference software renderer and the platform-neutral draw list.
//
// The picture is black, with four sakura-blossom finders in the canvas
// corners and a `天`-shaped field of 256 tiles inside three dotted rings. A
// light tile is a pale rounded square with a near-black katakana; a dark tile
// is empty except for its sakura-pink katakana. Vector backends draw the
// `petalDrawList` primitives with their own 2D APIs; geometry and polarity
// match the software renderer.

import { GLYPH_COUNT, GLYPH_GRID, STROKES, STROKE_WIDTH, inked } from "./glyphs.js";
import { PetalFrameCells } from "./lanes.js";
import {
  CANVAS,
  CENTER,
  DOT_RADIUS,
  FINDER_CENTERS,
  FINDER_CORE,
  FINDER_NOTCH_RADIUS,
  FINDER_OUTER,
  FINDER_PETALS,
  FINDER_PETAL_DISTANCE,
  FINDER_PETAL_RADIUS,
  GLYPH_BOX,
  PETAL_COS,
  PETAL_SIN,
  RING_COUNT,
  RING_RADII,
  RING_SLOTS,
  TILE_CENTER_X,
  TILE_CENTER_Y,
  TILE_CORNER_RADIUS,
  TILE_COUNT,
  TILE_GRID,
  TILE_LOOKUP,
  TILE_ORIGIN,
  TILE_PITCH,
  TILE_SIZE,
  TOTAL_SLOTS,
  finderLit,
  ringOffset,
} from "./layout.js";
import { deepFreeze } from "./support.js";

/** Colours of the picture as `[r, g, b]` triples. */
export const DEFAULT_PALETTE = deepFreeze({
  /** Background. */
  background: [0, 0, 0],
  /** Light tile fill and finders. */
  light: [250, 235, 244],
  /** Sakura pink of dots and of glyphs on dark tiles. */
  pink: [245, 175, 208],
  /** Glyph colour on a light tile. */
  ink: [20, 4, 14],
});

function readColor(value, name) {
  if (!Array.isArray(value) || value.length !== 3 || value.some((c) => !Number.isInteger(c) || c < 0 || c > 255)) {
    throw new TypeError(`palette.${name} must be an [r, g, b] byte triple`);
  }
  return Object.freeze(value.slice());
}

/** Completes a partial palette with the defaults. */
export function resolvePalette(palette) {
  if (palette === undefined || palette === null) {
    return DEFAULT_PALETTE;
  }
  if (typeof palette !== "object") {
    throw new TypeError("palette must be an object");
  }
  return Object.freeze({
    background: readColor(palette.background ?? DEFAULT_PALETTE.background, "background"),
    light: readColor(palette.light ?? DEFAULT_PALETTE.light, "light"),
    pink: readColor(palette.pink ?? DEFAULT_PALETTE.pink, "pink"),
    ink: readColor(palette.ink ?? DEFAULT_PALETTE.ink, "ink"),
  });
}

function requireCells(cells) {
  if (!(cells instanceof PetalFrameCells)) {
    throw new TypeError("cells must be a PetalFrameCells");
  }
  return cells;
}

const BITMAP_N = 128;
let glyphBitmaps = null;

/** 128x128 ink bitmaps of every glyph, built on first use. */
function bitmaps() {
  if (glyphBitmaps === null) {
    const all = new Uint8Array(GLYPH_COUNT * BITMAP_N * BITMAP_N);
    for (let glyph = 0; glyph < GLYPH_COUNT; glyph += 1) {
      const base = glyph * BITMAP_N * BITMAP_N;
      for (let v = 0; v < BITMAP_N; v += 1) {
        for (let u = 0; u < BITMAP_N; u += 1) {
          const x = ((u + 0.5) / BITMAP_N) * GLYPH_GRID;
          const y = ((v + 0.5) / BITMAP_N) * GLYPH_GRID;
          all[base + v * BITMAP_N + u] = inked(glyph, x, y) ? 1 : 0;
        }
      }
    }
    glyphBitmaps = all;
  }
  return glyphBitmaps;
}

/**
 * Inside test for a rounded square of half-side `half` and corner radius
 * `radius`, relative to its centre.
 */
function inRoundedSquare(dx, dy, half, radius) {
  const ax = Math.abs(dx);
  const ay = Math.abs(dy);
  if (ax > half || ay > half) {
    return false;
  }
  const cx = ax - (half - radius);
  const cy = ay - (half - radius);
  return cx <= 0 || cy <= 0 || cx * cx + cy * cy <= radius * radius;
}

const BACKGROUND = 0;
const LIGHT = 1;
const PINK = 2;
const INK = 3;
const TAU = 2 * Math.PI;

/** Colour index of canvas point `(x, y)`. */
function shade(cells, glyphs, x, y) {
  // finders
  for (let f = 0; f < 4; f += 1) {
    const dx = x - FINDER_CENTERS[f][0];
    const dy = y - FINDER_CENTERS[f][1];
    if (Math.sqrt(dx * dx + dy * dy) <= FINDER_OUTER) {
      return finderLit(dx, dy) ? LIGHT : BACKGROUND;
    }
  }
  // tiles
  if (x >= TILE_ORIGIN && y >= TILE_ORIGIN) {
    const col = Math.floor((x - TILE_ORIGIN) / TILE_PITCH);
    const row = Math.floor((y - TILE_ORIGIN) / TILE_PITCH);
    if (col < TILE_GRID && row < TILE_GRID) {
      const tile = TILE_LOOKUP[row * TILE_GRID + col];
      if (tile >= 0) {
        const cx = TILE_ORIGIN + TILE_PITCH * (col + 0.5);
        const cy = TILE_ORIGIN + TILE_PITCH * (row + 0.5);
        const dx = x - cx;
        const dy = y - cy;
        if (!inRoundedSquare(dx, dy, TILE_SIZE / 2, TILE_CORNER_RADIUS)) {
          return BACKGROUND;
        }
        const boxHalf = GLYPH_BOX / 2;
        let isInk = false;
        if (Math.abs(dx) < boxHalf && Math.abs(dy) < boxHalf) {
          const u = Math.floor(((dx + boxHalf) / (2 * boxHalf)) * BITMAP_N);
          const v = Math.floor(((dy + boxHalf) / (2 * boxHalf)) * BITMAP_N);
          const base = cells.glyph[tile] * BITMAP_N * BITMAP_N;
          isInk = glyphs[base + Math.min(v, BITMAP_N - 1) * BITMAP_N + Math.min(u, BITMAP_N - 1)] === 1;
        }
        if (cells.light[tile] === 1) {
          return isInk ? INK : LIGHT;
        }
        return isInk ? PINK : BACKGROUND;
      }
    }
  }
  // ring dots
  const dx = x - CENTER;
  const dy = y - CENTER;
  const radius = Math.sqrt(dx * dx + dy * dy);
  for (let ring = 0; ring < RING_COUNT; ring += 1) {
    const ringRadius = RING_RADII[ring];
    if (Math.abs(radius - ringRadius) > DOT_RADIUS) {
      continue;
    }
    const slots = RING_SLOTS[ring];
    let theta = Math.atan2(dy, dx);
    if (theta < 0) {
      theta += TAU;
    }
    const slot = Math.round((theta / TAU) * slots) % slots;
    if (cells.dots[ringOffset(ring) + slot] !== 1) {
      continue;
    }
    const angle = (TAU * slot) / slots;
    const px = ringRadius * Math.cos(angle);
    const py = ringRadius * Math.sin(angle);
    if (Math.sqrt((dx - px) * (dx - px) + (dy - py) * (dy - py)) <= DOT_RADIUS) {
      return PINK;
    }
  }
  return BACKGROUND;
}

/**
 * Renders one frame in software.
 *
 * @param {PetalFrameCells} cells
 * @param {{size?: number, supersample?: number, palette?: object}} [options]
 *   output side in pixels (default 1024), samples per pixel side for
 *   anti-aliasing (1-4, default 3) and colours
 * @returns {{width: number, height: number, data: Uint8ClampedArray}} RGBA pixels, ready for `ImageData`
 */
export function renderPetalFrame(cells, options = {}) {
  requireCells(cells);
  const size = options.size ?? 1024;
  const supersample = options.supersample ?? 3;
  if (!Number.isInteger(size) || size <= 0) {
    throw new RangeError("render size must be a positive integer");
  }
  if (!Number.isInteger(supersample) || supersample < 1 || supersample > 4) {
    throw new RangeError("supersample must be an integer in 1..=4");
  }
  const palette = resolvePalette(options.palette);
  const colors = [palette.background, palette.light, palette.pink, palette.ink];
  const glyphs = bitmaps();
  const unit = CANVAS / size;
  const data = new Uint8ClampedArray(size * size * 4);
  const n = supersample * supersample;
  for (let py = 0; py < size; py += 1) {
    for (let px = 0; px < size; px += 1) {
      let r = 0;
      let g = 0;
      let b = 0;
      for (let sy = 0; sy < supersample; sy += 1) {
        for (let sx = 0; sx < supersample; sx += 1) {
          const x = (px + (sx + 0.5) / supersample) * unit;
          const y = (py + (sy + 0.5) / supersample) * unit;
          const color = colors[shade(cells, glyphs, x, y)];
          r += color[0];
          g += color[1];
          b += color[2];
        }
      }
      const at = (py * size + px) * 4;
      data[at] = Math.floor((r + (n >> 1)) / n);
      data[at + 1] = Math.floor((g + (n >> 1)) / n);
      data[at + 2] = Math.floor((b + (n >> 1)) / n);
      data[at + 3] = 255;
    }
  }
  return { width: size, height: size, data };
}

/** Stroke width of a glyph in canvas units (6.5/32 of the glyph box). */
export const GLYPH_STROKE_WIDTH = (STROKE_WIDTH * GLYPH_BOX) / GLYPH_GRID;

function circle(x, y, r) {
  return Object.freeze({ x, y, r });
}

const FINDER_SHAPES = Object.freeze(
  FINDER_CENTERS.map(([fx, fy]) => {
    const petals = [];
    const notches = [];
    for (let petal = 0; petal < FINDER_PETALS; petal += 1) {
      const cos = PETAL_COS[petal];
      const sin = PETAL_SIN[petal];
      petals.push(circle(fx + FINDER_PETAL_DISTANCE * cos, fy + FINDER_PETAL_DISTANCE * sin, FINDER_PETAL_RADIUS));
      notches.push(circle(fx + FINDER_OUTER * cos, fy + FINDER_OUTER * sin, FINDER_NOTCH_RADIUS));
    }
    return Object.freeze({
      x: fx,
      y: fy,
      core: circle(fx, fy, FINDER_CORE),
      petals: Object.freeze(petals),
      notches: Object.freeze(notches),
    });
  }),
);

// Glyph polylines scaled into the glyph box, relative to the box origin.
const SCALED_STROKES = STROKES.map((strokes) =>
  strokes.map((stroke) => stroke.map((value) => (value * GLYPH_BOX) / GLYPH_GRID)),
);

const DOT_CENTERS = (() => {
  const xs = new Float64Array(TOTAL_SLOTS);
  const ys = new Float64Array(TOTAL_SLOTS);
  for (let ring = 0; ring < RING_COUNT; ring += 1) {
    const slots = RING_SLOTS[ring];
    for (let slot = 0; slot < slots; slot += 1) {
      const angle = (TAU * slot) / slots;
      xs[ringOffset(ring) + slot] = CENTER + RING_RADII[ring] * Math.cos(angle);
      ys[ringOffset(ring) + slot] = CENTER + RING_RADII[ring] * Math.sin(angle);
    }
  }
  return { xs, ys };
})();

/**
 * The frame as vector primitives in canvas design units (0..1024), for
 * backends that draw with their own 2D APIs (Canvas 2D, SVG, Skia...).
 *
 * Paint the background, then each finder (`core` and `petals` filled with
 * `palette.light`, then `notches` filled with `palette.background`), then the
 * light tiles (rounded squares filled with `palette.light`), then each tile's
 * glyph strokes in `tile.ink` (round caps and joins, width `strokeWidth`,
 * clipped to `tile.glyphBox`), then the lit dots in `palette.pink`.
 *
 * @param {PetalFrameCells} cells
 * @param {{palette?: object}} [options]
 */
export function petalDrawList(cells, options = {}) {
  requireCells(cells);
  const palette = resolvePalette(options.palette);
  const tiles = [];
  for (let tile = 0; tile < TILE_COUNT; tile += 1) {
    const cx = TILE_CENTER_X[tile];
    const cy = TILE_CENTER_Y[tile];
    const light = cells.light[tile] === 1;
    const glyph = cells.glyph[tile];
    const boxX = cx - GLYPH_BOX / 2;
    const boxY = cy - GLYPH_BOX / 2;
    tiles.push(
      Object.freeze({
        index: tile,
        x: cx - TILE_SIZE / 2,
        y: cy - TILE_SIZE / 2,
        size: TILE_SIZE,
        radius: TILE_CORNER_RADIUS,
        light,
        glyph,
        fill: light ? palette.light : null,
        ink: light ? palette.ink : palette.pink,
        glyphBox: Object.freeze({ x: boxX, y: boxY, size: GLYPH_BOX }),
        strokes: Object.freeze(
          SCALED_STROKES[glyph].map((stroke) =>
            Object.freeze(stroke.map((value, index) => value + (index % 2 === 0 ? boxX : boxY))),
          ),
        ),
      }),
    );
  }
  const dots = [];
  for (let slot = 0; slot < TOTAL_SLOTS; slot += 1) {
    if (cells.dots[slot] === 1) {
      dots.push(circle(DOT_CENTERS.xs[slot], DOT_CENTERS.ys[slot], DOT_RADIUS));
    }
  }
  return Object.freeze({
    canvas: CANVAS,
    palette,
    strokeWidth: GLYPH_STROKE_WIDTH,
    finders: FINDER_SHAPES,
    tiles: Object.freeze(tiles),
    dots: Object.freeze(dots),
  });
}
