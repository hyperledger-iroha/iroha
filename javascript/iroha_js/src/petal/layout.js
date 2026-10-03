// Normative frame geometry.
//
// All coordinates are design units on a square canvas of 1024 units with the
// origin at the top-left and `y` growing downward. A renderer scales the
// canvas to any pixel size; a decoder maps camera pixels back to design units.
// Cell order is defined by integer indices only.

import { deepFreeze, requireIndex } from "./support.js";

/** Canvas side length in design units. */
export const CANVAS = 1024;
/** Canvas centre coordinate on both axes. */
export const CENTER = 512;
/** Number of tile lattice columns and rows. */
export const TILE_GRID = 20;
/** Canvas coordinate of the lattice's left and top edge. */
export const TILE_ORIGIN = 222;
/** Lattice pitch in design units. */
export const TILE_PITCH = 29;
/** Side of a drawn tile (pitch minus a 4-unit gutter). */
export const TILE_SIZE = 25;
/** Corner radius of a drawn tile. */
export const TILE_CORNER_RADIUS = 3;
/** Side of the square glyph box inside a tile. */
export const GLYPH_BOX = 23;
/** Number of data tiles in the `天` mask. */
export const TILE_COUNT = 256;

/**
 * The `天` silhouette, one string per lattice row from top to bottom. `#`
 * marks a data tile. The mask is mirror-symmetric left to right and not
 * symmetric top to bottom, so it also tells a decoder which way is up.
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

/** Lattice column of every data tile in row-major order. */
export const TILE_COLS = new Uint8Array(TILE_COUNT);
/** Lattice row of every data tile in row-major order. */
export const TILE_ROWS = new Uint8Array(TILE_COUNT);
/** Canvas `x` of every tile centre. */
export const TILE_CENTER_X = new Float64Array(TILE_COUNT);
/** Canvas `y` of every tile centre. */
export const TILE_CENTER_Y = new Float64Array(TILE_COUNT);
/** `TILE_LOOKUP[row * 20 + col]` is the tile index or -1. */
export const TILE_LOOKUP = new Int16Array(TILE_GRID * TILE_GRID).fill(-1);

(() => {
  let count = 0;
  for (let row = 0; row < TILE_GRID; row += 1) {
    for (let col = 0; col < TILE_GRID; col += 1) {
      if (MASK[row][col] !== "#") continue;
      if (count >= TILE_COUNT) {
        throw new Error("mask must contain exactly 256 tiles");
      }
      TILE_COLS[count] = col;
      TILE_ROWS[count] = row;
      // Exact in f32 and f64 alike: 222 + 29 * (n + 0.5).
      TILE_CENTER_X[count] = TILE_ORIGIN + TILE_PITCH * (col + 0.5);
      TILE_CENTER_Y[count] = TILE_ORIGIN + TILE_PITCH * (row + 0.5);
      TILE_LOOKUP[row * TILE_GRID + col] = count;
      count += 1;
    }
  }
  if (count !== TILE_COUNT) {
    throw new Error("mask must contain exactly 256 tiles");
  }
})();

/**
 * Canvas coordinates of the centre of tile `index`.
 *
 * @returns {[number, number]}
 */
export function tileCenter(index) {
  requireIndex(index, "tile index");
  if (index >= TILE_COUNT) {
    throw new RangeError("tile index out of range");
  }
  return [TILE_CENTER_X[index], TILE_CENTER_Y[index]];
}

/** Number of concentric dot rings. */
export const RING_COUNT = 3;
/** Ring radii in design units. */
export const RING_RADII = Object.freeze([360, 410, 460]);
/**
 * Dot slots on each ring (all multiples of four so the three cardinal gates
 * sit exactly on a slot).
 */
export const RING_SLOTS = Object.freeze([80, 92, 104]);
/** Radius of a drawn ring dot. */
export const DOT_RADIUS = 11;
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
const TAU_F32 = fround(2 * Math.PI);

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
    const angle = -Math.PI / 2 + (2 * Math.PI * petal) / FINDER_PETALS;
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
