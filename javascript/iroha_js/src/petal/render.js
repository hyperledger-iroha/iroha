// Reference software renderer and the platform-neutral draw list.
//
// The picture is black, with four sakura-blossom finders in the canvas
// corners, three dotted rings, and inside them a field of petals on concentric
// bands, each lit petal turned about its centre by its turn. A lit petal whose
// centre lies inside the `天` is drawn in the light colour, the others in
// sakura pink, over a faint backdrop of the `天` itself, so the character
// shows through the storm of petals. Vector backends draw the `petalDrawList`
// primitives with their own 2D APIs; geometry, turns and polarity match the
// software renderer.

import { PetalFrameCells } from "./lanes.js";
import {
  BACKDROP_RADIUS,
  BAND_COUNT,
  BAND_INNER,
  BAND_OFFSETS,
  BAND_SLOTS,
  BAND_SPACING,
  CANVAS,
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
  TOTAL_SLOTS,
  bandPhase,
  bandRadius,
  finderLit,
  inMask,
  petalLitAlong,
  ringOffset,
} from "./layout.js";
import { deepFreeze } from "./support.js";
import { TURN_POSITIONS, turnAngle } from "./turns.js";

/** Colours of the picture as `[r, g, b]` triples. */
export const DEFAULT_PALETTE = deepFreeze({
  /** Background. */
  background: [0, 0, 0],
  /** Finders and the petals inside the `天`. */
  light: [250, 235, 244],
  /** Sakura pink of the ring dots and of the petals outside the `天`. */
  pink: [245, 175, 208],
  /** The faint `天` drawn under the petals (a tenth of the light colour). */
  backdrop: [25, 24, 24],
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
  });
}

function requireCells(cells) {
  if (!(cells instanceof PetalFrameCells)) {
    throw new TypeError("cells must be a PetalFrameCells");
  }
  return cells;
}

const BACKGROUND = 0;
const LIGHT = 1;
const PINK = 2;
const BACKDROP = 3;
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

/**
 * Renders one frame in software, exactly like the reference renderer.
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
  const colors = [palette.background, palette.light, palette.pink, palette.backdrop];
  const axis = axes();
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
          const color = colors[shade(cells, x, y, axis)];
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
 * The frame as vector primitives in canvas design units (0..1024), for
 * backends that draw with their own 2D APIs (Canvas 2D, SVG, Skia...).
 *
 * Paint the background, then the `backdrop` cells (squares filled with
 * `palette.backdrop`, clipped to the backdrop disc), then the lit `petals`
 * (each an ellipse of `petal.halfLength` along `angle`, the outward direction
 * plus the petal's turn, and `petal.halfWidth` across it, centred on the petal
 * and filled with its `fill`, minus the notch of `petal.notchRadius` centred
 * `petal.notchDistance` along `angle`, which must be drawn: it tells a petal
 * from one turned by half a turn), then the lit dots in `palette.pink`, then
 * each finder (`core` and `petals` filled with `palette.light`, then `notches`
 * filled with `palette.background`). Petals are small (12 x 7.5 units): draw
 * them antialiased, at 768 pixels or more.
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
  return Object.freeze({
    canvas: CANVAS,
    palette,
    finders: FINDER_SHAPES,
    backdrop: BACKDROP_SHAPE,
    petal: PETAL_SHAPE,
    petals: Object.freeze(petals),
    dots: Object.freeze(dots),
  });
}
