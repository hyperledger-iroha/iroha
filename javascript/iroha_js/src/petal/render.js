// Reference software renderer and the platform-neutral draw list.
//
// The picture is black, with four sakura-blossom finders in the canvas
// corners, three dotted rings, and inside them a field of petals on concentric
// bands, each lit petal turned about its centre by its turn. A lit petal whose
// centre lies inside the `天` is drawn in the light colour, the others in
// sakura pink, over a faint backdrop of the `天` itself, so the character
// shows through the storm of petals. Above and below that square, the card's
// two bands show rows of katakana tiles (see `kana.js`): a light tile filled
// light with its glyph in ink, a dark tile black with its glyph in pink. Vector
// backends draw the `petalDrawList` primitives with their own 2D APIs;
// geometry, turns, glyphs and polarity match the software renderer.

import { GLYPH_COUNT, GLYPH_GRID, GLYPH_STROKES, STROKE_WIDTH } from "./glyphs.js";
import { PetalFrameCells } from "./lanes.js";
import {
  BACKDROP_RADIUS,
  BAND_COUNT,
  BAND_INNER,
  BAND_OFFSETS,
  BAND_SLOTS,
  BAND_SPACING,
  CANVAS,
  CARD_HEIGHT,
  CELL_ANGLE,
  CELL_CENTER_X,
  CELL_CENTER_Y,
  CELL_LIGHT,
  CENTER,
  DOT_RADIUS,
  FIELD_CELLS,
  FINDER_CENTERS,
  FINDER_CORE,
  FINDER_NOTCH_RADIUS,
  FINDER_OUTER,
  FINDER_PETALS,
  FINDER_PETAL_DISTANCE,
  FINDER_PETAL_RADIUS,
  KANA_CORNER,
  KANA_GLYPH_BOX,
  KANA_TILE,
  KANA_TILE_COUNT,
  MASK,
  MASK_ORIGIN,
  MASK_PITCH,
  PETAL_COS,
  PETAL_HALF_LENGTH,
  PETAL_HALF_LENGTH_F32,
  PETAL_HALF_WIDTH,
  PETAL_NOTCH_DISTANCE,
  PETAL_NOTCH_RADIUS,
  PETAL_SIN,
  RING_COUNT,
  RING_RADII,
  RING_SLOTS,
  SQUARE_TOP,
  TILE_X,
  TILE_Y,
  TOTAL_SLOTS,
  bandPhase,
  bandRadius,
  finderLit,
  inMask,
  kanaInkedAt,
  kanaInside,
  kanaTileAtInto,
  petalLitAlong,
  ringOffset,
} from "./layout.js";
import { deepFreeze } from "./support.js";
import { TURN_POSITIONS, turnAngle } from "./turns.js";

/** Colours of the picture as `[r, g, b]` triples. */
export const DEFAULT_PALETTE = deepFreeze({
  /** Background. */
  background: [0, 0, 0],
  /** Finders, the petals inside the `天` and the light katakana tiles. */
  light: [250, 235, 244],
  /** Sakura pink of the ring dots, of the petals outside the `天` and of the glyphs on dark tiles. */
  pink: [245, 175, 208],
  /** The faint `天` drawn under the petals (a tenth of the light colour). */
  backdrop: [25, 24, 24],
  /** Glyphs on light katakana tiles. */
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
    backdrop: readColor(palette.backdrop ?? DEFAULT_PALETTE.backdrop, "backdrop"),
    ink: readColor(palette.ink ?? DEFAULT_PALETTE.ink, "ink"),
  });
}

function requireCells(cells) {
  if (!(cells instanceof PetalFrameCells)) {
    throw new TypeError("cells must be a PetalFrameCells");
  }
  const { kanaGlyphs, kanaLight } = cells;
  if (
    !(kanaGlyphs instanceof Uint8Array) ||
    !(kanaLight instanceof Uint8Array) ||
    kanaGlyphs.length !== KANA_TILE_COUNT ||
    kanaLight.length !== KANA_TILE_COUNT ||
    kanaGlyphs.some((glyph) => glyph >= GLYPH_COUNT)
  ) {
    throw new RangeError("cells must show a glyph of 0..=15 on each of the 612 katakana tiles");
  }
  return cells;
}

const BACKGROUND = 0;
const LIGHT = 1;
const PINK = 2;
const BACKDROP = 3;
const INK = 4;
const TAU = 2 * Math.PI;

/** Rust `f64::round`: halves round away from zero. */
function roundHalfAway(value) {
  return value < 0 ? -Math.round(-value) : Math.round(value);
}

const OUTER_DOT_EDGE = RING_RADII[RING_COUNT - 1] + DOT_RADIUS;
const INNER_DOT_EDGE = RING_RADII[0] - DOT_RADIUS;

// No point further than this from a petal's centre lies inside it, however it is turned (the
// half-length is the larger half-axis; the margin keeps the shortcut clear of rounding at the tips).
const PETAL_REACH2 = (PETAL_HALF_LENGTH_F32 + 0.01) * (PETAL_HALF_LENGTH_F32 + 0.01);

let petalAxes = null;

/**
 * The sine and cosine of the axis of every field cell's petal at every turn position (the outward
 * direction plus the turn), `[cell * 4 + turn]`; built on first use.
 */
function axes() {
  if (petalAxes === null) {
    const sin = new Float64Array(FIELD_CELLS * TURN_POSITIONS);
    const cos = new Float64Array(FIELD_CELLS * TURN_POSITIONS);
    for (let cell = 0; cell < FIELD_CELLS; cell += 1) {
      for (let turn = 0; turn < TURN_POSITIONS; turn += 1) {
        const angle = CELL_ANGLE[cell] + turnAngle(turn);
        sin[cell * TURN_POSITIONS + turn] = Math.sin(angle);
        cos[cell * TURN_POSITIONS + turn] = Math.cos(angle);
      }
    }
    petalAxes = { sin, cos };
  }
  return petalAxes;
}

/** Colour index of the lit petal covering canvas point `(x, y)`, or -1. */
function petalAt(cells, x, y, radius, theta, axis) {
  const nearest = roundHalfAway((radius - BAND_INNER) / BAND_SPACING);
  for (let band = nearest - 1; band <= nearest + 1; band += 1) {
    if (band < 0 || band >= BAND_COUNT) {
      continue;
    }
    if (Math.abs(radius - bandRadius(band)) > PETAL_HALF_LENGTH_F32) {
      continue;
    }
    const slots = BAND_SLOTS[band];
    const around = roundHalfAway((theta / TAU) * slots - bandPhase(band));
    for (let slot = around - 1; slot <= around + 1; slot += 1) {
      const cell = BAND_OFFSETS[band] + (((slot % slots) + slots) % slots);
      if (cells.petals[cell] !== 1) {
        continue;
      }
      const dx = x - CELL_CENTER_X[cell];
      const dy = y - CELL_CENTER_Y[cell];
      const at = cell * TURN_POSITIONS + cells.turns[cell];
      if (dx * dx + dy * dy <= PETAL_REACH2 && petalLitAlong(dx, dy, axis.sin[at], axis.cos[at])) {
        return CELL_LIGHT[cell] === 1 ? LIGHT : PINK;
      }
    }
  }
  return -1;
}

/** Colour index of canvas point `(x, y)`. */
function shade(cells, x, y, axis) {
  // finders
  for (let f = 0; f < 4; f += 1) {
    const dx = x - FINDER_CENTERS[f][0];
    const dy = y - FINDER_CENTERS[f][1];
    if (Math.sqrt(dx * dx + dy * dy) <= FINDER_OUTER) {
      return finderLit(dx, dy) ? LIGHT : BACKGROUND;
    }
  }
  const dx = x - CENTER;
  const dy = y - CENTER;
  const radius = Math.sqrt(dx * dx + dy * dy);
  // beyond the outer ring's dots, and between the backdrop and the inner ring's, nothing is drawn
  if (radius > OUTER_DOT_EDGE || (radius > BACKDROP_RADIUS && radius < INNER_DOT_EDGE)) {
    return BACKGROUND;
  }
  let theta = Math.atan2(dy, dx);
  if (theta < 0) {
    theta += TAU;
  }
  // ring dots
  for (let ring = 0; ring < RING_COUNT; ring += 1) {
    const ringRadius = RING_RADII[ring];
    if (Math.abs(radius - ringRadius) > DOT_RADIUS) {
      continue;
    }
    const slots = RING_SLOTS[ring];
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
  // petals, then the faint 天 under them
  if (radius <= BACKDROP_RADIUS) {
    const petal = petalAt(cells, x, y, radius, theta, axis);
    if (petal >= 0) {
      return petal;
    }
    if (inMask(x, y)) {
      return BACKDROP;
    }
  }
  return BACKGROUND;
}

const TILE_AT = new Float64Array(3);

/**
 * Colour index of the katakana tile at the square-coordinate point `(x, y)` outside the square, or
 * the background between tiles.
 */
function kanaShade(cells, x, y) {
  if (!kanaTileAtInto(x, y, TILE_AT)) {
    return BACKGROUND;
  }
  const tile = TILE_AT[0];
  const u = TILE_AT[1];
  const v = TILE_AT[2];
  if (!kanaInside(u, v)) {
    return BACKGROUND;
  }
  const inked = kanaInkedAt(cells.kanaGlyphs[tile], u, v);
  if (cells.kanaLight[tile] === 1) {
    return inked ? INK : LIGHT;
  }
  return inked ? PINK : BACKGROUND;
}

/** Height in pixels of a card rendered `size` pixels wide: `size * 1668 / 1024`, rounded. */
export function petalCardHeight(size) {
  if (!Number.isInteger(size) || size <= 0) {
    throw new RangeError("render size must be a positive integer");
  }
  return Math.floor((size * CARD_HEIGHT) / CANVAS + 0.5);
}

/**
 * Renders the whole card (`card`: `petalCardHeight(size)` rows, the first `SQUARE_TOP` canvas units
 * above the square's top edge) or only its square (`size` rows).
 */
function renderRows(cells, options, card) {
  requireCells(cells);
  const size = options.size ?? 1024;
  const supersample = options.supersample ?? 3;
  if (!Number.isInteger(size) || size <= 0) {
    throw new RangeError("render size must be a positive integer");
  }
  if (!Number.isInteger(supersample) || supersample < 1 || supersample > 4) {
    throw new RangeError("supersample must be an integer in 1..=4");
  }
  const height = card ? petalCardHeight(size) : size;
  const top = card ? SQUARE_TOP : 0;
  const palette = resolvePalette(options.palette);
  const colors = [palette.background, palette.light, palette.pink, palette.backdrop, palette.ink];
  const axis = axes();
  const unit = CANVAS / size;
  const data = new Uint8ClampedArray(size * height * 4);
  const n = supersample * supersample;
  for (let py = 0; py < height; py += 1) {
    for (let px = 0; px < size; px += 1) {
      let r = 0;
      let g = 0;
      let b = 0;
      for (let sy = 0; sy < supersample; sy += 1) {
        for (let sx = 0; sx < supersample; sx += 1) {
          const x = (px + (sx + 0.5) / supersample) * unit;
          const y = (py + (sy + 0.5) / supersample) * unit - top;
          const color = colors[y >= 0 && y < CANVAS ? shade(cells, x, y, axis) : kanaShade(cells, x, y)];
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
  return { width: size, height, data };
}

/**
 * Renders one frame in software, exactly like the reference renderer: the card, `size` pixels wide
 * and {@link petalCardHeight} pixels tall, whose square starts `SQUARE_TOP` (322) canvas units below
 * its top edge, with the band of katakana tiles above and below it.
 *
 * @param {PetalFrameCells} cells
 * @param {{size?: number, supersample?: number, palette?: object}} [options]
 *   output width in pixels (default 1024), samples per pixel side for
 *   anti-aliasing (1-4, default 3) and colours
 * @returns {{width: number, height: number, data: Uint8ClampedArray}} RGBA pixels, ready for `ImageData`
 */
export function renderPetalFrame(cells, options = {}) {
  return renderRows(cells, options, true);
}

/**
 * Renders only the square of the card, `size` pixels on a side (as {@link renderPetalFrame} draws
 * it). Not exported from the package: the decoder's own tests work on bare squares.
 */
export function renderPetalSquare(cells, options = {}) {
  return renderRows(cells, options, false);
}

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

/** The `#` cells of the mask lattice as squares, row-major; drawn clipped to the backdrop disc. */
const BACKDROP_SHAPE = Object.freeze({
  x: CENTER,
  y: CENTER,
  r: BACKDROP_RADIUS,
  cells: Object.freeze(
    MASK.flatMap((line, row) =>
      Array.from(line).flatMap((mark, col) =>
        mark === "#" ? [Object.freeze({ x: MASK_ORIGIN + MASK_PITCH * col, y: MASK_ORIGIN + MASK_PITCH * row, size: MASK_PITCH })] : [],
      ),
    ),
  ),
});

/** The tile every katakana tile is drawn as, in canvas units, with the strokes of every glyph. */
const TILE_SHAPE = (() => {
  const inset = (KANA_TILE - KANA_GLYPH_BOX) / 2;
  const scale = KANA_GLYPH_BOX / GLYPH_GRID;
  return deepFreeze({
    /** Side of the tile square. */
    side: KANA_TILE,
    /** Corner radius of the tile square. */
    corner: KANA_CORNER,
    /** Width of a glyph's strokes: the design grid's stroke width scaled into the glyph box. */
    strokeWidth: (STROKE_WIDTH * KANA_GLYPH_BOX) / GLYPH_GRID,
    /**
     * The strokes of every glyph in tile-local units (from the tile's top-left corner; the glyph
     * box is inset by 1 unit): polylines of `[u, v]` points, `glyphs[glyph][stroke][point]`.
     */
    glyphs: GLYPH_STROKES.map((strokes) =>
      strokes.map((stroke) => stroke.map(([x, y]) => [inset + x * scale, inset + y * scale])),
    ),
  });
})();

/** The petal outline every lit cell is drawn with, in canvas units. */
const PETAL_SHAPE = Object.freeze({
  /** Half the length along the petal's axis. */
  halfLength: PETAL_HALF_LENGTH,
  /** Half the width. */
  halfWidth: PETAL_HALF_WIDTH,
  /** Radius of the notch cut into the tip (always drawn: the turn reader's templates include it). */
  notchRadius: PETAL_NOTCH_RADIUS,
  /** Distance from the petal centre to the notch centre, along the axis. */
  notchDistance: PETAL_NOTCH_DISTANCE,
});

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
 * The frame as vector primitives in square design units (0..1024, `y` down), for backends that
 * draw with their own 2D APIs (Canvas 2D, SVG, Skia...). The card extends `squareTop` (322) units
 * above and below the square: a canvas of `canvas` x `cardHeight` (1024 x 1668) units translated
 * down by `squareTop` holds it all.
 *
 * Paint the card's background, then the `backdrop` cells (squares filled with
 * `palette.backdrop`, clipped to the backdrop disc), then the lit `petals`
 * (each an ellipse of `petal.halfLength` along `angle`, the outward direction
 * plus the petal's turn, and `petal.halfWidth` across it, centred on the petal
 * and filled with its `fill`, minus the notch of `petal.notchRadius` centred
 * `petal.notchDistance` along `angle`, which must be drawn: it tells a petal
 * from one turned by half a turn), then the lit dots in `palette.pink`, then
 * each finder (`core` and `petals` filled with `palette.light`, then `notches`
 * filled with `palette.background`), then the `kanaTiles`: a light tile is a
 * square of side `tile.side` at `(x, y)` with corners rounded to `tile.corner`,
 * filled with its `fill` (`palette.light`; a dark tile has no fill), and every
 * tile's glyph is stroked along `tile.glyphs[glyph]` (polylines in tile-local
 * units from the tile's top-left corner, round caps and joins, width
 * `tile.strokeWidth`) in its `ink` (`palette.ink` on a light tile,
 * `palette.pink` on a dark one); every stroke lies inside its rounded tile, so
 * no clip is needed. Petals are small (12 x 7.5 units) and tiles 25 units:
 * draw them antialiased, at 768 pixels or more.
 *
 * @param {PetalFrameCells} cells
 * @param {{palette?: object}} [options]
 */
export function petalDrawList(cells, options = {}) {
  requireCells(cells);
  const palette = resolvePalette(options.palette);
  const petals = [];
  for (let cell = 0; cell < FIELD_CELLS; cell += 1) {
    if (cells.petals[cell] !== 1) {
      continue;
    }
    const light = CELL_LIGHT[cell] === 1;
    petals.push(
      Object.freeze({
        index: cell,
        x: CELL_CENTER_X[cell],
        y: CELL_CENTER_Y[cell],
        angle: CELL_ANGLE[cell] + turnAngle(cells.turns[cell]),
        light,
        fill: light ? palette.light : palette.pink,
      }),
    );
  }
  const dots = [];
  for (let slot = 0; slot < TOTAL_SLOTS; slot += 1) {
    if (cells.dots[slot] === 1) {
      dots.push(circle(DOT_CENTERS.xs[slot], DOT_CENTERS.ys[slot], DOT_RADIUS));
    }
  }
  const kanaTiles = [];
  for (let tile = 0; tile < KANA_TILE_COUNT; tile += 1) {
    const light = cells.kanaLight[tile] === 1;
    kanaTiles.push(
      Object.freeze({
        index: tile,
        x: TILE_X[tile],
        y: TILE_Y[tile],
        glyph: cells.kanaGlyphs[tile],
        light,
        fill: light ? palette.light : null,
        ink: light ? palette.ink : palette.pink,
      }),
    );
  }
  return Object.freeze({
    canvas: CANVAS,
    cardHeight: CARD_HEIGHT,
    squareTop: SQUARE_TOP,
    palette,
    finders: FINDER_SHAPES,
    backdrop: BACKDROP_SHAPE,
    petal: PETAL_SHAPE,
    petals: Object.freeze(petals),
    dots: Object.freeze(dots),
    tile: TILE_SHAPE,
    kanaTiles: Object.freeze(kanaTiles),
  });
}
