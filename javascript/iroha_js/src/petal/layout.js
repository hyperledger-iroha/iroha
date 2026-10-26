// Normative frame geometry.
//
// All coordinates are design units on a square canvas of 1024 units with the
// origin at the top-left and `y` growing downward. A renderer scales the
// canvas to any pixel size; a decoder maps camera pixels back to design units.
// Cell order is defined by integer indices only, and the slot count of every
// band is a fixed table.
//
// A frame is a card of 1024 x 1668 units: the square code with a band of
// katakana tiles above and below it (see `kana.js`). Square coordinates are used
// throughout; the card's `y` is the square's plus `SQUARE_TOP`, so band 0 lies at
// negative `y` and band 1 below the square.

import { GLYPH_GRID, glyphInkedAt, requireGlyph } from "./glyphs.js";
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

// ---- the card: the square and a band of katakana tiles above and below it ---------------------

/** Side of a katakana tile (see `kana.js`). */
export const KANA_TILE = 25;
/** Pitch of the katakana tile lattice. */
export const KANA_PITCH = 29;
/** Side of a tile's glyph box, centred in the tile. */
export const KANA_GLYPH_BOX = 23;
/** Corner radius of a tile. */
export const KANA_CORNER = 3;
/** Tile rows of each band. */
export const KANA_ROWS = 10;
/** Tile columns of each band. */
export const KANA_COLUMNS = 35;
/** Gap between a band's lattice and the square. */
export const KANA_GAP = 24;
/** Margin outside a band's lattice. */
export const KANA_MARGIN = 8;
/** Height of a band with its gap and margin (322 units). */
export const KANA_BAND_HEIGHT = KANA_MARGIN + KANA_ROWS * KANA_PITCH + KANA_GAP;
/** Height of the card: the square with a band above and below it (1668 units). */
export const CARD_HEIGHT = CANVAS + 2 * KANA_BAND_HEIGHT;
/** Card `y` of the square's top edge: card coordinates are square coordinates plus this in `y`. */
export const SQUARE_TOP = KANA_BAND_HEIGHT;
/**
 * Tile centres keep at least this distance from every finder centre: the blossom's radius, a
 * blossom diameter of clearance, and a tile's half-diagonal, so no tile looks like part of a
 * finder.
 */
export const KANA_FINDER_CLEARANCE = 198;
/** Bands: above the square and below it. */
export const KANA_BANDS = 2;
/** Pilot tiles of each band. */
export const KANA_PILOTS = 8;
/** Tiles of each band: the lattice cells clear of the finders. */
export const KANA_BAND_TILES = 306;
/** Tiles of both bands. */
export const KANA_TILE_COUNT = KANA_BANDS * KANA_BAND_TILES;
/** Data tiles of each band (its tiles that are not pilots). */
export const KANA_BAND_DATA_TILES = KANA_BAND_TILES - KANA_PILOTS;

/**
 * The pilot tiles of each band as `[row, column]` (band 1 mirrors band 0's rows, so the pilots
 * sit at the same distances from the square).
 */
export const KANA_PILOT_CELLS = deepFreeze([
  [
    [0, 0],
    [0, 17],
    [0, 34],
    [4, 0],
    [4, 34],
    [9, 8],
    [9, 17],
    [9, 26],
  ],
  [
    [9, 0],
    [9, 17],
    [9, 34],
    [5, 0],
    [5, 34],
    [0, 8],
    [0, 17],
    [0, 26],
  ],
]);
/** What every pilot shows: `[glyph, light]` in pilot order (ロ ケ ヒ ム ス ワ ア カ). */
export const KANA_PILOT_SYMBOLS = deepFreeze([
  [1, true],
  [11, false],
  [13, true],
  [9, false],
  [14, false],
  [6, true],
  [12, false],
  [7, true],
]);

/** Left edge of the lattice (the columns are centred on the card). */
const KANA_LATTICE_LEFT = (CANVAS - KANA_COLUMNS * KANA_PITCH) / 2;
/** The inset of a tile square in its lattice cell. */
const KANA_INSET = (KANA_PITCH - KANA_TILE) / 2;

/** Top of band `band`'s lattice in square coordinates. */
function kanaLatticeTop(band) {
  return band === 0 ? -KANA_BAND_HEIGHT + KANA_MARGIN : CANVAS + KANA_GAP;
}

/** Band of every tile. */
export const TILE_BAND = new Uint8Array(KANA_TILE_COUNT);
/** Row (within its band) of every tile. */
export const TILE_ROW = new Uint8Array(KANA_TILE_COUNT);
/** Column of every tile. */
export const TILE_COLUMN = new Uint8Array(KANA_TILE_COUNT);
/** Left edge of every tile square in square coordinates. */
export const TILE_X = new Float64Array(KANA_TILE_COUNT);
/** Top edge of every tile square in square coordinates. */
export const TILE_Y = new Float64Array(KANA_TILE_COUNT);
/** Index among the pilots of its band of every tile, or `-1` for a data tile. */
export const TILE_PILOT = new Int8Array(KANA_TILE_COUNT);
/** The data tiles of band `b` (`KANA_DATA_TILE_LIST[b]`, in reading order) as tile indices. */
export const KANA_DATA_TILE_LIST = Object.freeze([
  new Uint16Array(KANA_BAND_DATA_TILES),
  new Uint16Array(KANA_BAND_DATA_TILES),
]);
/** The pilot tiles of band `b` (`KANA_PILOT_TILE_LIST[b]`, in pilot order) as tile indices. */
export const KANA_PILOT_TILE_LIST = Object.freeze([new Uint16Array(KANA_PILOTS), new Uint16Array(KANA_PILOTS)]);
/** The tile of every lattice cell (`(band * 10 + row) * 35 + column`), or `-1`. */
const KANA_LATTICE = new Int16Array(KANA_BANDS * KANA_ROWS * KANA_COLUMNS).fill(-1);

(() => {
  const half = KANA_TILE / 2;
  const clearance = KANA_FINDER_CLEARANCE;
  let count = 0;
  for (let band = 0; band < KANA_BANDS; band += 1) {
    let data = 0;
    for (let row = 0; row < KANA_ROWS; row += 1) {
      for (let column = 0; column < KANA_COLUMNS; column += 1) {
        const x = KANA_LATTICE_LEFT + KANA_PITCH * column + KANA_INSET;
        const y = kanaLatticeTop(band) + KANA_PITCH * row + KANA_INSET;
        const cx = x + half;
        const cy = y + half;
        const clear = FINDER_CENTERS.every(([fx, fy]) => {
          const dx = cx - fx;
          const dy = cy - fy;
          return dx * dx + dy * dy >= clearance * clearance;
        });
        if (!clear) {
          continue;
        }
        const pilot = KANA_PILOT_CELLS[band].findIndex(([r, c]) => r === row && c === column);
        TILE_BAND[count] = band;
        TILE_ROW[count] = row;
        TILE_COLUMN[count] = column;
        TILE_X[count] = x;
        TILE_Y[count] = y;
        TILE_PILOT[count] = pilot;
        KANA_LATTICE[(band * KANA_ROWS + row) * KANA_COLUMNS + column] = count;
        if (pilot >= 0) {
          KANA_PILOT_TILE_LIST[band][pilot] = count;
        } else {
          KANA_DATA_TILE_LIST[band][data] = count;
          data += 1;
        }
        count += 1;
      }
    }
    if (count !== (band + 1) * KANA_BAND_TILES || data !== KANA_BAND_DATA_TILES) {
      throw new Error(`a band holds ${KANA_BAND_TILES} tiles, ${KANA_PILOTS} of them pilots`);
    }
  }
})();

const KANA_TILE_OBJECTS = Object.freeze(
  Array.from({ length: KANA_TILE_COUNT }, (_, tile) =>
    Object.freeze({
      band: TILE_BAND[tile],
      row: TILE_ROW[tile],
      column: TILE_COLUMN[tile],
      x: TILE_X[tile],
      y: TILE_Y[tile],
      pilot: TILE_PILOT[tile] < 0 ? null : TILE_PILOT[tile],
    }),
  ),
);

/**
 * Every tile of both bands: band 0 first, each in reading order (rows top first, columns left
 * first), as `{band, row, column, x, y, pilot}` with the tile square's top-left corner `(x, y)` in
 * square coordinates and `pilot` the index among its band's pilots (`null` for a data tile). A
 * lattice cell whose tile centre lies within `KANA_FINDER_CLEARANCE` of a finder centre has no
 * tile (306 tiles per band remain).
 */
export function kanaTiles() {
  return KANA_TILE_OBJECTS.slice();
}

function requireBand(band) {
  requireIndex(band, "band");
  if (band >= KANA_BANDS) {
    throw new RangeError("band out of range");
  }
  return band;
}

/** The data tiles of band `band` (its tiles that are not pilots) in reading order, as indices into {@link kanaTiles}. */
export function kanaDataTiles(band) {
  return Array.from(KANA_DATA_TILE_LIST[requireBand(band)]);
}

/** The pilot tiles of band `band` in pilot order, as indices into {@link kanaTiles}. */
export function kanaPilotTiles(band) {
  return Array.from(KANA_PILOT_TILE_LIST[requireBand(band)]);
}

/**
 * The tile under the square-coordinate point `(x, y)` into `out` as `[tile, u, v]`, with the
 * point's position inside the tile's square (`0..=25` on both axes); `false` outside every tile
 * square.
 */
export function kanaTileAtInto(x, y, out) {
  let band;
  if (y < 0) {
    band = 0;
  } else if (y >= CANVAS) {
    band = 1;
  } else {
    return false;
  }
  const gx = (x - KANA_LATTICE_LEFT) / KANA_PITCH;
  const gy = (y - kanaLatticeTop(band)) / KANA_PITCH;
  // NaN coordinates land on no tile (the reference finds no tile for them either)
  if (!(gx >= 0) || !(gy >= 0)) {
    return false;
  }
  const column = Math.floor(gx);
  const row = Math.floor(gy);
  if (column >= KANA_COLUMNS || row >= KANA_ROWS) {
    return false;
  }
  const tile = KANA_LATTICE[(band * KANA_ROWS + row) * KANA_COLUMNS + column];
  if (tile < 0) {
    return false;
  }
  const u = (gx - column) * KANA_PITCH - KANA_INSET;
  const v = (gy - row) * KANA_PITCH - KANA_INSET;
  if (!(u >= 0 && u <= KANA_TILE && v >= 0 && v <= KANA_TILE)) {
    return false;
  }
  out[0] = tile;
  out[1] = u;
  out[2] = v;
  return true;
}

/**
 * The tile under the square-coordinate point `(x, y)` and the point's position inside the tile's
 * square (`0..=25` on both axes), as `[tile, u, v]`; `null` outside every tile square.
 *
 * @returns {[number, number, number] | null}
 */
export function kanaTileAt(x, y) {
  if (typeof x !== "number" || typeof y !== "number") {
    throw new TypeError("tile coordinates must be numbers");
  }
  const out = [0, 0, 0];
  return kanaTileAtInto(x, y, out) ? out : null;
}

/** Whether the tile-local point `(u, v)` lies inside the rounded tile square. */
export function kanaInside(u, v) {
  const side = KANA_TILE;
  const corner = KANA_CORNER;
  if (!(u >= 0 && u <= side) || !(v >= 0 && v <= side)) {
    return false;
  }
  const cx = u < corner ? corner : u > side - corner ? side - corner : u;
  const cy = v < corner ? corner : v > side - corner ? side - corner : v;
  return (u - cx) * (u - cx) + (v - cy) * (v - cy) <= corner * corner;
}

/** The glyph box's inset in its tile. */
const GLYPH_INSET = (KANA_TILE - KANA_GLYPH_BOX) / 2;
/** Design-grid units per tile unit inside the glyph box. */
const GLYPH_SCALE = GLYPH_GRID / KANA_GLYPH_BOX;

/** {@link kanaInked} without the argument checks. */
export function kanaInkedAt(glyph, u, v) {
  return kanaInside(u, v) && glyphInkedAt(glyph, (u - GLYPH_INSET) * GLYPH_SCALE, (v - GLYPH_INSET) * GLYPH_SCALE);
}

/**
 * Whether the tile-local point `(u, v)` lies on `glyph`'s strokes: inside the tile square and
 * inked in the glyph box (inset by half the difference of tile and box).
 */
export function kanaInked(glyph, u, v) {
  requireGlyph(glyph);
  return kanaInkedAt(glyph, u, v);
}
