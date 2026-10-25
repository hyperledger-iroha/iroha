// Normative frame geometry.
//
// All coordinates are design units on a square canvas of 1024 units with the
// origin at the top-left and `y` growing downward. A renderer scales the
// canvas to any pixel size; a decoder maps camera pixels back to design units.
// Cell order is defined by integer indices only, and the slot count of every
// band is a fixed table.

import { deepFreeze, requireIndex } from "./support.js";

/** Canvas side length in design units. */
export const CANVAS = 1024;
/** Canvas centre coordinate on both axes. */
export const CENTER = 512;

/** Number of petal bands (concentric rings of field cells). */
export const BAND_COUNT = 34;
/** Radius of band 0. */
export const BAND_INNER = 16;
/** Radial distance between consecutive bands. */
export const BAND_SPACING = 11;
/**
 * Cells on each band, inner band first: `4 * round(2 pi r / 50)` for the band
 * radius `r`, about one cell every 12.5 units. Every count is a multiple of
 * four, so the four sectors split each band evenly and the field maps onto
 * itself under quarter turns. The table, not the formula, is normative.
 */
export const BAND_SLOTS = Object.freeze([
  8, 12, 20, 24, 32, 36, 40, 48, 52, 56, 64, 68, 76, 80, 84, 92, 96, 104, 108, 112, 120, 124, 128,
  136, 140, 148, 152, 156, 164, 168, 172, 180, 184, 192,
]);
/** Number of field cells. */
export const FIELD_CELLS = 3376;
/** Angular sectors of the field; each is one lane codeword. */
export const SECTORS = 4;
/** Cells in each sector. */
export const SECTOR_CELLS = FIELD_CELLS / SECTORS;
/**
 * Radius of the disc a petal stands in for: decoders sample within it, and no other petal,
 * however it is turned, reaches into it.
 */
export const PETAL_RADIUS = 4.6;
/**
 * Half the length of a drawn petal, along its axis (the outward radial direction, turned by the
 * petal's turn, see `turns.js`).
 */
export const PETAL_HALF_LENGTH = 6.0;
/** Half the width of a drawn petal. */
export const PETAL_HALF_WIDTH = 3.73;
/** Radius of the notch cut into each petal tip; it tells a petal from one turned by half a turn. */
export const PETAL_NOTCH_RADIUS = 1.12;
/** Distance from a petal centre to its notch centre, along its axis. */
export const PETAL_NOTCH_DISTANCE = 6.24;

// The reference keeps the petal constants in single precision and widens them
// where it computes; these are the widened values its arithmetic sees.
/** `PETAL_RADIUS` as the reference computes with it (its `f32` value). */
export const PETAL_RADIUS_F32 = Math.fround(PETAL_RADIUS);
const HALF_LENGTH_F32 = Math.fround(PETAL_HALF_LENGTH);
const HALF_WIDTH_F32 = Math.fround(PETAL_HALF_WIDTH);
const NOTCH_RADIUS_F32 = Math.fround(PETAL_NOTCH_RADIUS);
const NOTCH_DISTANCE_F32 = Math.fround(PETAL_NOTCH_DISTANCE);
/** `PETAL_HALF_LENGTH` as the reference renderer computes with it (its `f32` value). */
export const PETAL_HALF_LENGTH_F32 = HALF_LENGTH_F32;

const TAU = 2 * Math.PI;

/** Index of the first cell of every band (and the total at the end). */
export const BAND_OFFSETS = (() => {
  const offsets = new Uint16Array(BAND_COUNT + 1);
  for (let band = 0; band < BAND_COUNT; band += 1) {
    if (BAND_SLOTS[band] % 4 !== 0) {
      throw new Error("band slot counts are multiples of four");
    }
    offsets[band + 1] = offsets[band] + BAND_SLOTS[band];
  }
  if (offsets[BAND_COUNT] !== FIELD_CELLS) {
    throw new Error("the band table must hold every cell");
  }
  return offsets;
})();

/** Radius of band `band`. */
export function bandRadius(band) {
  return BAND_INNER + BAND_SPACING * band;
}

/**
 * Fraction of a slot by which band `band` is turned: odd bands sit half a slot
 * over, so neighbouring bands interleave.
 */
export function bandPhase(band) {
  return band % 2 === 1 ? 0.5 : 0;
}

/** Band of every field cell. */
export const CELL_BAND = new Uint8Array(FIELD_CELLS);
/** Slot (within its band) of every field cell. */
export const CELL_SLOT = new Uint8Array(FIELD_CELLS);
/** Angle (radians, clockwise on screen from 3 o'clock) of every field cell centre. */
export const CELL_ANGLE = new Float64Array(FIELD_CELLS);
/** Canvas `x` of every field cell centre. */
export const CELL_CENTER_X = new Float64Array(FIELD_CELLS);
/** Canvas `y` of every field cell centre. */
export const CELL_CENTER_Y = new Float64Array(FIELD_CELLS);
/** Cosine of every field cell's angle (the outward direction of its petal). */
export const CELL_COS = new Float64Array(FIELD_CELLS);
/** Sine of every field cell's angle. */
export const CELL_SIN = new Float64Array(FIELD_CELLS);
(() => {
  for (let band = 0; band < BAND_COUNT; band += 1) {
    const slots = BAND_SLOTS[band];
    const radius = bandRadius(band);
    for (let slot = 0; slot < slots; slot += 1) {
      const cell = BAND_OFFSETS[band] + slot;
      const angle = (TAU * (slot + bandPhase(band))) / slots;
      CELL_BAND[cell] = band;
      CELL_SLOT[cell] = slot;
      CELL_ANGLE[cell] = angle;
      CELL_COS[cell] = Math.cos(angle);
      CELL_SIN[cell] = Math.sin(angle);
      CELL_CENTER_X[cell] = CENTER + radius * CELL_COS[cell];
      CELL_CENTER_Y[cell] = CENTER + radius * CELL_SIN[cell];
    }
  }
})();

function requireCell(cell) {
  requireIndex(cell, "field cell");
  if (cell >= FIELD_CELLS) {
    throw new RangeError("field cell out of range");
  }
  return cell;
}

/**
 * Band and slot of field cell `cell`.
 *
 * @returns {[number, number]}
 */
export function cellBandSlot(cell) {
  requireCell(cell);
  return [CELL_BAND[cell], CELL_SLOT[cell]];
}

/** Angle (radians, clockwise on screen from 3 o'clock) of the centre of field cell `cell`. */
export function cellAngle(cell) {
  return CELL_ANGLE[requireCell(cell)];
}

/**
 * Canvas coordinates of the centre of field cell `cell` (double precision).
 *
 * @returns {[number, number]}
 */
export function cellCenter(cell) {
  requireCell(cell);
  return [CELL_CENTER_X[cell], CELL_CENTER_Y[cell]];
}

/**
 * The cells of every sector in bit order: band by band from the centre out,
 * and within a band in ascending slot order. Sector `s` holds slots
 * `s*n/4 ... (s+1)*n/4 - 1` of every band of `n` slots. The first 840 cells
 * carry the sector's codeword; the last four are spare and stay dark.
 */
export const SECTOR_CELL_LIST = Object.freeze(
  Array.from({ length: SECTORS }, (_, sector) => {
    const cells = new Uint16Array(SECTOR_CELLS);
    let count = 0;
    for (let band = 0; band < BAND_COUNT; band += 1) {
      const quarter = BAND_SLOTS[band] / SECTORS;
      for (let slot = sector * quarter; slot < (sector + 1) * quarter; slot += 1) {
        cells[count] = BAND_OFFSETS[band] + slot;
        count += 1;
      }
    }
    return cells;
  }),
);

/** The cells of sector `sector` in bit order (a copy). */
export function sectorCells(sector) {
  requireIndex(sector, "sector");
  if (sector >= SECTORS) {
    throw new RangeError("sector out of range");
  }
  return Array.from(SECTOR_CELL_LIST[sector]);
}

/** Number of lattice columns and rows of the `天` mask. */
export const MASK_GRID = 20;
/** Canvas coordinate of the mask lattice's left and top edge. */
export const MASK_ORIGIN = 182;
/** Mask lattice pitch in design units. */
export const MASK_PITCH = 33;
/** Radius beyond which the `天` backdrop is not drawn. */
export const BACKDROP_RADIUS = 390;

/**
 * The `天` silhouette, one string per lattice row from top to bottom. `#`
 * marks a lattice cell inside the character. Petals whose centre lies in a `#`
 * cell are drawn in the light colour, the others in pink, and the `#` cells
 * inside `BACKDROP_RADIUS` are drawn as a faint backdrop, so the character
 * shows through the petals. The mask is mirror-symmetric left to right and not
 * top to bottom.
 */
export const MASK = Object.freeze([
  "....############....",
  "...##############...",
  "..################..",
  ".##################.",
  "###..............###",
  "###..............###",
  "#########..#########",
  "#########..#########",
  "###..............###",
  "###..............###",
  "########....########",
  "########....########",
  "#######......#######",
  "#######..##..#######",
  "######..####..######",
  "#####...####...#####",
  ".###...######...###.",
  "..###.########.###..",
  "......########......",
  ".....##########.....",
]);

/** `MASK_LIT[row * 20 + col]` is 1 for a `#` lattice cell. */
const MASK_LIT = new Uint8Array(MASK_GRID * MASK_GRID);
MASK.forEach((line, row) => {
  for (let col = 0; col < MASK_GRID; col += 1) {
    MASK_LIT[row * MASK_GRID + col] = line[col] === "#" ? 1 : 0;
  }
});

/** Whether the canvas point `(x, y)` lies inside the `天` mask. */
export function inMask(x, y) {
  if (x < MASK_ORIGIN || y < MASK_ORIGIN) {
    return false;
  }
  const col = Math.floor((x - MASK_ORIGIN) / MASK_PITCH);
  const row = Math.floor((y - MASK_ORIGIN) / MASK_PITCH);
  return col < MASK_GRID && row < MASK_GRID && MASK_LIT[row * MASK_GRID + col] === 1;
}

/** `1` for every field cell inside the `天` (drawn in the light colour). */
export const CELL_LIGHT = Uint8Array.from({ length: FIELD_CELLS }, (_, cell) =>
  inMask(CELL_CENTER_X[cell], CELL_CENTER_Y[cell]) ? 1 : 0,
);

/** Whether field cell `cell` lies inside the `天` (drawn in the light colour). */
export function cellLight(cell) {
  return CELL_LIGHT[requireCell(cell)] === 1;
}

/** Number of concentric dot rings. */
export const RING_COUNT = 3;
/** Ring radii in design units. */
export const RING_RADII = Object.freeze([405, 440, 475]);
/**
 * Dot slots on each ring (all multiples of four so the three cardinal gates
 * sit exactly on a slot).
 */
export const RING_SLOTS = Object.freeze([88, 100, 108]);
/** Radius of a drawn ring dot. */
export const DOT_RADIUS = 9;
/** Total dot slots over the three rings. */
export const TOTAL_SLOTS = RING_SLOTS[0] + RING_SLOTS[1] + RING_SLOTS[2];
/**
 * Dots in each cardinal gate, per ring, for the right, bottom and left gates.
 * There is deliberately no gate at the top.
 */
export const GATE_DOTS = deepFreeze([
  [1, 1, 3],
  [2, 2, 2],
  [1, 2, 2],
]);
/** Number of lane `D` bits carried by the rings. */
export const D_BITS = 240;

/** Offset of ring `ring` inside the flat slot index space. */
export function ringOffset(ring) {
  if (ring === 0) return 0;
  if (ring === 1) return RING_SLOTS[0];
  if (ring === 2) return RING_SLOTS[0] + RING_SLOTS[1];
  throw new RangeError("ring index out of range");
}

function gateSlots(ring) {
  const n = RING_SLOTS[ring];
  const bases = [0, n / 4, n / 2]; // right, bottom, left
  const gates = [];
  const guards = [];
  for (let gate = 0; gate < 3; gate += 1) {
    const base = bases[gate];
    const count = GATE_DOTS[gate][ring];
    let first;
    let last;
    if (count === 1) {
      first = base;
      last = base;
    } else if (count === 2) {
      first = base;
      last = base + 1;
    } else {
      first = base + n - 1;
      last = base + 1;
    }
    const span = ((last + n - first) % n) + 1;
    for (let step = 0; step < span; step += 1) {
      gates.push((first + step) % n);
    }
    guards.push((first + n - 1) % n);
    guards.push((last + 1) % n);
  }
  return { gates, guards };
}

/** Slot role code of a gate dot (always lit). */
export const ROLE_GATE = -1;
/** Slot role code of a guard slot next to a gate (always dark). */
export const ROLE_GUARD = -2;
/** Slot role code of an unused data slot (always dark). */
export const ROLE_SPARE = -3;

/** Role of every flat slot: a lane `D` bit index (>= 0) or a negative role code. */
export const SLOT_ROLE = new Int16Array(TOTAL_SLOTS).fill(ROLE_SPARE);
/** Flat slot index of every lane `D` bit, in bit order. */
export const DATA_SLOT = new Uint16Array(D_BITS);
/** Flat indices of the gate slots, ascending. */
export const GATE_SLOT_LIST = [];
/** Flat indices of the guard slots, ascending. */
export const GUARD_SLOT_LIST = [];
/** Ring of every flat slot. */
export const SLOT_RING = new Uint8Array(TOTAL_SLOTS);

(() => {
  for (let ring = 0; ring < RING_COUNT; ring += 1) {
    const { gates, guards } = gateSlots(ring);
    const offset = ringOffset(ring);
    for (const slot of guards) SLOT_ROLE[offset + slot] = ROLE_GUARD;
    for (const slot of gates) SLOT_ROLE[offset + slot] = ROLE_GATE;
    for (let slot = 0; slot < RING_SLOTS[ring]; slot += 1) SLOT_RING[offset + slot] = ring;
  }
  let next = 0;
  for (let flat = 0; flat < TOTAL_SLOTS; flat += 1) {
    if (SLOT_ROLE[flat] === ROLE_SPARE && next < D_BITS) {
      SLOT_ROLE[flat] = next;
      DATA_SLOT[next] = flat;
      next += 1;
    }
    if (SLOT_ROLE[flat] === ROLE_GATE) GATE_SLOT_LIST.push(flat);
    if (SLOT_ROLE[flat] === ROLE_GUARD) GUARD_SLOT_LIST.push(flat);
  }
  Object.freeze(GATE_SLOT_LIST);
  Object.freeze(GUARD_SLOT_LIST);
})();

const GATE_ROLE = Object.freeze({ kind: "gate" });
const GUARD_ROLE = Object.freeze({ kind: "guard" });
const SPARE_ROLE = Object.freeze({ kind: "spare" });

/**
 * Computes the role of every ring slot in flat index order: `gate` (always
 * lit), `guard` (always dark), `data` (carries lane `D` bit `bit`) or
 * `spare` (unused, always dark).
 *
 * @returns {Array<{kind: "gate" | "guard" | "spare"} | {kind: "data", bit: number}>}
 */
export function slotRoles() {
  const roles = new Array(TOTAL_SLOTS);
  for (let flat = 0; flat < TOTAL_SLOTS; flat += 1) {
    const role = SLOT_ROLE[flat];
    if (role >= 0) roles[flat] = Object.freeze({ kind: "data", bit: role });
    else if (role === ROLE_GATE) roles[flat] = GATE_ROLE;
    else if (role === ROLE_GUARD) roles[flat] = GUARD_ROLE;
    else roles[flat] = SPARE_ROLE;
  }
  return roles;
}

/** Flat slot index of every lane `D` bit, in bit order. */
export function dataSlots() {
  return Array.from(DATA_SLOT);
}

/**
 * Splits a flat slot index into `[ring, slot]`.
 *
 * @returns {[number, number]}
 */
export function splitSlot(flat) {
  requireIndex(flat, "flat slot index");
  if (flat < RING_SLOTS[0]) return [0, flat];
  if (flat < RING_SLOTS[0] + RING_SLOTS[1]) return [1, flat - RING_SLOTS[0]];
  return [2, flat - RING_SLOTS[0] - RING_SLOTS[1]];
}

const fround = Math.fround;
const TAU_F32 = fround(TAU);

// The reference computes slot centres in single precision; every step is
// rounded to f32 here so the decoder samples exactly the same positions.
function slotCenterF32(ring, slot) {
  const theta = fround(fround(TAU_F32 * fround(slot)) / fround(RING_SLOTS[ring]));
  const radius = fround(RING_RADII[ring]);
  return [
    fround(CENTER + fround(radius * fround(Math.cos(theta)))),
    fround(CENTER + fround(radius * fround(Math.sin(theta)))),
  ];
}

/** Canvas `x` of every flat slot centre. */
export const SLOT_CENTER_X = new Float64Array(TOTAL_SLOTS);
/** Canvas `y` of every flat slot centre. */
export const SLOT_CENTER_Y = new Float64Array(TOTAL_SLOTS);
(() => {
  for (let ring = 0; ring < RING_COUNT; ring += 1) {
    for (let slot = 0; slot < RING_SLOTS[ring]; slot += 1) {
      const [x, y] = slotCenterF32(ring, slot);
      SLOT_CENTER_X[ringOffset(ring) + slot] = x;
      SLOT_CENTER_Y[ringOffset(ring) + slot] = y;
    }
  }
})();

/**
 * Canvas coordinates of the centre of slot `slot` on ring `ring`.
 *
 * Slot `0` is at 3 o'clock and slots advance clockwise on the screen. The
 * coordinates are single-precision values, as in the reference.
 *
 * @returns {[number, number]}
 */
export function slotCenter(ring, slot) {
  ringOffset(ring);
  requireIndex(slot, "slot index");
  return slotCenterF32(ring, slot);
}

/** Canvas coordinates of the four corner finders, clockwise from top-left. */
export const FINDER_CENTERS = deepFreeze([
  [72, 72],
  [952, 72],
  [952, 952],
  [72, 952],
]);
/** Radius of the finder's solid centre disc. */
export const FINDER_CORE = 12;
/** Number of petals of a finder blossom. */
export const FINDER_PETALS = 5;
/** Distance from the finder centre to each petal centre. */
export const FINDER_PETAL_DISTANCE = 34;
/** Radius of each petal. */
export const FINDER_PETAL_RADIUS = 26;
/** Radius of the notch cut into each petal tip. */
export const FINDER_NOTCH_RADIUS = 6;
/** Outer radius of a finder blossom (tip of a petal). */
export const FINDER_OUTER = 60;

/** Unit vectors of the petal directions; the first petal points straight up. */
export const PETAL_COS = new Float64Array(FINDER_PETALS);
/** See {@link PETAL_COS}. */
export const PETAL_SIN = new Float64Array(FINDER_PETALS);
(() => {
  for (let petal = 0; petal < FINDER_PETALS; petal += 1) {
    const angle = -Math.PI / 2 + (TAU * petal) / FINDER_PETALS;
    PETAL_COS[petal] = Math.cos(angle);
    PETAL_SIN[petal] = Math.sin(angle);
  }
})();

/**
 * Returns whether the point `(dx, dy)`, relative to a finder centre, is lit.
 *
 * A finder is a solid five-petal sakura blossom whose first petal points
 * straight up. The petal notches are cosmetic; decoders only rely on the
 * blossom being one large, isolated, roughly round blob.
 */
export function finderLit(dx, dy) {
  if (Math.sqrt(dx * dx + dy * dy) <= FINDER_CORE) {
    return true;
  }
  for (let petal = 0; petal < FINDER_PETALS; petal += 1) {
    const cx = FINDER_PETAL_DISTANCE * PETAL_COS[petal];
    const cy = FINDER_PETAL_DISTANCE * PETAL_SIN[petal];
    if (Math.sqrt((dx - cx) * (dx - cx) + (dy - cy) * (dy - cy)) <= FINDER_PETAL_RADIUS) {
      const nx = FINDER_OUTER * PETAL_COS[petal];
      const ny = FINDER_OUTER * PETAL_SIN[petal];
      return Math.sqrt((dx - nx) * (dx - nx) + (dy - ny) * (dy - ny)) > FINDER_NOTCH_RADIUS;
    }
  }
  return false;
}

/**
 * Returns whether the point `(dx, dy)`, relative to the centre of a petal that
 * points along `angle`, is lit: an ellipse of half-length `PETAL_HALF_LENGTH`
 * along `angle` and half-width `PETAL_HALF_WIDTH`, minus the notch at its tip.
 */
export function petalLit(dx, dy, angle) {
  return petalLitAlong(dx, dy, Math.sin(angle), Math.cos(angle));
}

/** {@link petalLit} with the sine and cosine of the petal's angle given. */
export function petalLitAlong(dx, dy, sin, cos) {
  const along = dx * cos + dy * sin;
  const across = -dx * sin + dy * cos;
  const u = along / HALF_LENGTH_F32;
  const v = across / HALF_WIDTH_F32;
  if (u * u + v * v > 1) {
    return false;
  }
  const tip = along - NOTCH_DISTANCE_F32;
  return tip * tip + across * across > NOTCH_RADIUS_F32 * NOTCH_RADIUS_F32;
}
