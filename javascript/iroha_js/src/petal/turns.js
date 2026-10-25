// The turn layer: data in the way the lit petals point.
//
// A field lane tells the decoder exactly which petals of its sector are lit.
// The first `TURN_CARRIERS` lit petals of the sector, in the fixed
// pseudo-random `carrierOrder`, carry two more bits each: the petal is turned
// about its own centre by 0, 45, 90 or 135 degrees from its outward direction
// (`turnAngle`). A turned petal stays inside its own cell, so it moves no light
// toward its neighbours and the field lanes read as before; a camera that
// resolves the petals' shapes also reads the sector's turn lane.
//
// The turn lane of a sector is one Reed-Solomon codeword, RS(90, 68),
// XOR-whitened: four fountain atoms from the repair range (ids from
// `TURN_ID_BASE`, see `turnFirstId`) and a CRC-32C check that binds them to the
// stream tag, frame and sector of the field lane they ride on (`turnCheck`).
// Carrier `j` shows bits `2j` and `2j + 1` of the transmitted codeword (most
// significant bit first) as a Gray-coded position (`turnPosition`). A sector
// with fewer than `TURN_CARRIERS` lit petals turns none (about one sector in
// 60,000).
//
// `readTurns` reads the turn lanes of a decoded capture: every carrier's
// neighbourhood is sampled in the petal's own outward frame, following a smooth
// correction of the pose fitted to the carriers' centroids; the blur is the one
// under which the carriers and their lit neighbours explain the image best; and
// each carrier takes the turn whose (slightly wider) template matches best once
// its lit neighbours, at their own decided turns, are subtracted.
//
// Every floating-point expression follows the reference operation by operation
// (sums over a sequence start at -0 like Rust's `Iterator::sum`). The tables of
// the reader are built on first use and its buffers are reused, so a read
// allocates almost nothing.

import { crc32c } from "./crc.js";
import { PetalPose } from "./geometry.js";
import { sampleLuma } from "./image.js";
import { ATOM_LEN, FIELD_BITS, decodeLaneCountedStatus, encodeLane, fieldLane } from "./lanes.js";
import {
  BAND_COUNT,
  BAND_OFFSETS,
  CELL_ANGLE,
  CELL_BAND,
  CELL_CENTER_X,
  CELL_CENTER_Y,
  CELL_COS,
  CELL_SIN,
  FIELD_CELLS,
  SECTORS,
  SECTOR_CELL_LIST,
  petalLitAlong,
} from "./layout.js";
import { PetalXorshift32 } from "./prng.js";
import { PetalReedSolomon, RS_INVALID_SHAPE, RS_UNCORRECTABLE } from "./rs.js";
import { PetalError, clamp, fmax, fmin, requireIndex, toBytes, totalCmp } from "./support.js";

// Nothing at the top level of this module reads a binding of `lanes.js`, which imports this module
// in turn: the two can be loaded in either order.

/** Turn positions a carrier petal can take. */
export const TURN_POSITIONS = 4;
/** Bits one carrier shows. */
export const TURN_BITS = 2;
/** Carrier petals of a sector. */
export const TURN_CARRIERS = 360;
/** Codeword length of a turn lane in bytes. */
export const TURN_WORD = (TURN_CARRIERS * TURN_BITS) / 8;
/** Parity bytes of a turn lane. */
export const TURN_PARITY = 22;
/** Data bytes of a turn lane: the atoms, then their check. */
export const TURN_DATA = TURN_WORD - TURN_PARITY;
/** Atoms carried by each turn lane. */
export const TURN_ATOMS = 4;
/** Bytes of the check that closes a turn lane's data. */
export const TURN_CHECK_LEN = 4;
/**
 * Fountain id of the first turn atom of frame 0. The field lanes and lane D number their atoms
 * from 0 and never come near it.
 */
export const TURN_ID_BASE = 2 ** 31;
/** Gaussian blurs (sigma in canvas units) the turn reader fits. */
export const TURN_BLURS = Object.freeze([1.0, 1.4, 1.9, 2.5, 3.2, 4.0]);

/** The angle of every turn position: `pi * position / 4`. */
const TURN_ANGLE = Float64Array.from(
  { length: TURN_POSITIONS },
  (_, position) => (Math.PI * position) / TURN_POSITIONS,
);
const TURN_SIN = TURN_ANGLE.map((angle) => Math.sin(angle));
const TURN_COS = TURN_ANGLE.map((angle) => Math.cos(angle));
/** The label (the two bits shown, see {@link turnLabel}) of every turn position. */
export const TURN_LABELS = Object.freeze([0, 1, 2, 3].map((position) => position ^ (position >> 1)));

function requireSector(sector) {
  requireIndex(sector, "sector");
  if (sector >= SECTORS) {
    throw new RangeError("sector out of range");
  }
  return sector;
}

function requirePosition(position, name) {
  if (!Number.isInteger(position) || position < 0 || position >= TURN_POSITIONS) {
    throw new RangeError(`${name} must be an integer in 0..=3`);
  }
  return position;
}

function isByte(value) {
  return Number.isInteger(value) && value >= 0 && value <= 255;
}

/** Validates a lane header `{tag, frame}`. */
function requireHeader(header) {
  if (
    header === null ||
    typeof header !== "object" ||
    !isByte(header.tag) ||
    !Number.isInteger(header.frame) ||
    header.frame < 0 ||
    header.frame > 0xffff
  ) {
    throw new TypeError("lane header must be {tag: 0..=255, frame: 0..=65535}");
  }
  return header;
}

function frameNumber(frame) {
  if (!Number.isInteger(frame)) {
    throw new TypeError("frame must be an integer");
  }
  return ((frame % 65536) + 65536) % 65536;
}

/**
 * The angle (radians, clockwise on screen) by which a petal at turn position `position` is
 * turned from its outward direction: `pi * position / 4`.
 */
export function turnAngle(position) {
  return TURN_ANGLE[requirePosition(position, "turn position")];
}

/**
 * The two bits a turn position shows: its Gray code `position ^ (position >> 1)`. Neighbouring
 * positions (135 and 0 degrees neighbour too, a petal turned by 180 degrees differing only by its
 * notch) differ in one bit.
 */
export function turnLabel(position) {
  requirePosition(position, "turn position");
  return position ^ (position >> 1);
}

/** The turn position that shows the two bits `label` (the inverse of {@link turnLabel}). */
export function turnPosition(label) {
  requirePosition(label, "turn label");
  return label ^ (label >> 1);
}

let carrierOrders = null;

/**
 * The carrier orders of the four sectors (built on first use): each sector's codeword cells (the
 * first 840 of its bit order) shuffled by Fisher-Yates with an xorshift32 seeded
 * `0x4F524430 + sector` ("ORD0" ... "ORD3"): for `i` from 839 down to 1, position `i` swaps with
 * position `nextU32() mod (i + 1)`.
 */
export function carrierOrderTable() {
  if (carrierOrders === null) {
    carrierOrders = Object.freeze(
      SECTOR_CELL_LIST.map((cells, sector) => {
        const order = cells.slice(0, FIELD_BITS);
        const rng = new PetalXorshift32(0x4f524430 + sector);
        for (let i = order.length - 1; i >= 1; i -= 1) {
          const j = rng.nextU32() % (i + 1);
          const swap = order[i];
          order[i] = order[j];
          order[j] = swap;
        }
        return order;
      }),
    );
  }
  return carrierOrders;
}

/**
 * The order in which the lit petals of sector `sector` become carriers: its codeword cells
 * shuffled by Fisher-Yates with an xorshift32 seeded `0x4F524430 + sector` (a copy).
 */
export function carrierOrder(sector) {
  return Array.from(carrierOrderTable()[requireSector(sector)]);
}

/**
 * Writes the carriers of `sector` under the lit map `petals` (one flag per field cell) to
 * `out` from `offset`; returns whether the sector has all `TURN_CARRIERS` of them.
 */
export function carriersInto(petals, sector, out, offset) {
  const order = carrierOrderTable()[sector];
  let count = 0;
  for (let index = 0; index < order.length && count < TURN_CARRIERS; index += 1) {
    const cell = order[index];
    if (petals[cell]) {
      out[offset + count] = cell;
      count += 1;
    }
  }
  return count === TURN_CARRIERS;
}

/**
 * The carriers of sector `sector` under the lit map `petals` (one flag per field cell): the first
 * `TURN_CARRIERS` lit cells in {@link carrierOrder}, or `null` when fewer are lit.
 *
 * @param {ArrayLike<number | boolean>} petals
 * @returns {number[] | null}
 */
export function carriers(petals, sector) {
  requireSector(sector);
  if (petals === null || typeof petals !== "object" || typeof petals.length !== "number" || petals.length < FIELD_CELLS) {
    throw new TypeError(`petals must hold one flag per field cell (${FIELD_CELLS})`);
  }
  const chosen = new Array(TURN_CARRIERS);
  return carriersInto(petals, sector, chosen, 0) ? chosen : null;
}

/**
 * Fountain id of the first of the `TURN_ATOMS` atoms the turn lane of `sector` carries in `frame`
 * (a counter reduced modulo 65536): `2^31 + 16 * frame + 4 * sector`.
 */
export function turnFirstId(frame, sector) {
  return TURN_ID_BASE + frameNumber(frame) * (SECTORS * TURN_ATOMS) + requireSector(sector) * TURN_ATOMS;
}

/** The check of the atoms of a turn lane (arguments already validated). */
function checkOf(tag, frame, sector, atoms) {
  const bytes = new Uint8Array(4 + atoms.length);
  bytes[0] = tag;
  bytes[1] = frame >> 8;
  bytes[2] = frame & 0xff;
  bytes[3] = sector;
  bytes.set(atoms, 4);
  return crc32c(bytes);
}

function readU32(bytes, offset) {
  return ((bytes[offset] << 24) | (bytes[offset + 1] << 16) | (bytes[offset + 2] << 8) | bytes[offset + 3]) >>> 0;
}

/**
 * The check of a turn lane: CRC-32C over the stream tag, the frame (big-endian), the sector and
 * the atoms, so a turn lane read under the wrong field lane, frame or stream is refused.
 *
 * @param {{tag: number, frame: number}} header the header of the field lane the turn lane rides on
 * @returns {number} the check as an unsigned 32-bit integer
 */
export function turnCheck(header, sector, atoms) {
  const { tag, frame } = requireHeader(header);
  requireSector(sector);
  return checkOf(tag, frame, sector, toBytes(atoms, "turn atoms"));
}

/**
 * The data of a turn lane: the `TURN_ATOMS` atoms followed by their {@link turnCheck}
 * (big-endian).
 *
 * @returns {Uint8Array} `TURN_DATA` bytes
 * @throws {RangeError} when `atoms` is not `TURN_ATOMS` atoms long
 */
export function turnData(header, sector, atoms) {
  const { tag, frame } = requireHeader(header);
  requireSector(sector);
  const bytes = toBytes(atoms, "turn atoms");
  if (bytes.length !== TURN_ATOMS * ATOM_LEN) {
    throw new RangeError("turn atoms length mismatch");
  }
  const data = new Uint8Array(TURN_DATA);
  data.set(bytes);
  const check = checkOf(tag, frame, sector, bytes);
  data[bytes.length] = check >>> 24;
  data[bytes.length + 1] = (check >>> 16) & 0xff;
  data[bytes.length + 2] = (check >>> 8) & 0xff;
  data[bytes.length + 3] = check & 0xff;
  return data;
}

/** Whether turn lane data checks out against `header` and `sector` (both already valid). */
function checks(header, sector, data) {
  const atoms = data.subarray(0, TURN_ATOMS * ATOM_LEN);
  return checkOf(header.tag, header.frame, sector, atoms) === readU32(data, TURN_ATOMS * ATOM_LEN);
}

/**
 * The atoms of turn lane data whose check matches `header` and `sector`; `null` for data of the
 * wrong length, a sector out of range or a check that does not match.
 *
 * @returns {{header: {tag: number, frame: number}, firstId: number, atoms: Uint8Array[]} | null}
 */
export function turnPacket(header, sector, data) {
  const { tag, frame } = requireHeader(header);
  const bytes = toBytes(data, "turn lane data");
  if (bytes.length !== TURN_DATA || !Number.isInteger(sector) || sector < 0 || sector >= SECTORS) {
    return null;
  }
  const lane = Object.freeze({ tag, frame });
  if (!checks(lane, sector, bytes)) {
    return null;
  }
  const atoms = [];
  for (let index = 0; index < TURN_ATOMS; index += 1) {
    atoms.push(bytes.slice(index * ATOM_LEN, (index + 1) * ATOM_LEN));
  }
  return Object.freeze({ header: lane, firstId: turnFirstId(frame, sector), atoms: Object.freeze(atoms) });
}

const TURN_RS = new PetalReedSolomon(TURN_PARITY);
// "TRN0" ... "TRN3"
const TURN_SPECS = Object.freeze(
  Array.from({ length: SECTORS }, (_, sector) => {
    const whiteningSeed = 0x54524e30 + sector;
    const rng = new PetalXorshift32(whiteningSeed);
    const whitening = new Uint8Array(TURN_WORD);
    for (let index = 0; index < TURN_WORD; index += 1) {
      whitening[index] = rng.nextByte();
    }
    return Object.freeze({
      name: `T${sector}`,
      sector,
      wordLen: TURN_WORD,
      parityLen: TURN_PARITY,
      dataLen: TURN_DATA,
      atoms: TURN_ATOMS,
      whiteningSeed,
      rs: TURN_RS,
      whitening,
    });
  }),
);

/**
 * The whitening sequence of the turn lane of `sector`: one byte per codeword byte from an
 * xorshift32 seeded `0x54524E30 + sector` ("TRN0" ... "TRN3"); a copy.
 */
export function turnWhitening(sector) {
  return TURN_SPECS[requireSector(sector)].whitening.slice();
}

/**
 * Encodes turn lane data into the transmitted (whitened) codeword.
 *
 * @param {Uint8Array | ArrayBuffer | ArrayBufferView | number[]} data exactly `TURN_DATA` bytes
 * @returns {Uint8Array} `TURN_WORD` bytes
 */
export function encodeTurnLane(sector, data) {
  const spec = TURN_SPECS[requireSector(sector)];
  const bytes = toBytes(data, "turn lane data");
  if (bytes.length !== TURN_DATA) {
    throw new RangeError("turn data length mismatch");
  }
  const word = spec.rs.encode(bytes);
  for (let index = 0; index < word.length; index += 1) {
    word[index] ^= spec.whitening[index];
  }
  return word;
}

/**
 * Decodes a transmitted turn codeword, returning its data and how many byte positions the
 * Reed-Solomon decoder rewrote. The check is not verified here (see {@link turnPacket}).
 *
 * @param {readonly number[]} [erasures] byte positions the caller distrusts
 * @returns {{data: Uint8Array, corrected: number}}
 * @throws {PetalError} `rs_invalid_shape` or `rs_uncorrectable`
 */
export function decodeTurnLaneCounted(sector, transmitted, erasures = []) {
  const spec = TURN_SPECS[requireSector(sector)];
  const bytes = toBytes(transmitted, "transmitted turn lane word");
  if (!Array.isArray(erasures)) {
    throw new TypeError("erasures must be an array of byte positions");
  }
  const result = decodeLaneCountedStatus(spec, bytes, erasures);
  if (result === RS_INVALID_SHAPE) throw new PetalError("rs_invalid_shape");
  if (result === RS_UNCORRECTABLE) throw new PetalError("rs_uncorrectable");
  return Object.freeze({ data: result.data, corrected: result.corrected });
}

/** The turn position carrier `index` of a transmitted turn codeword shows (arguments valid). */
export function carrierPositionOf(word, index) {
  const label = (word[index >> 2] >> (6 - 2 * (index & 3))) & 3;
  return label ^ (label >> 1);
}

/** The turn position carrier `index` of a transmitted turn codeword shows. */
export function carrierPosition(word, index) {
  const bytes = toBytes(word, "turn lane word");
  requireIndex(index, "carrier index");
  if (index >= bytes.length * 4) {
    throw new RangeError("carrier index out of range");
  }
  return carrierPositionOf(bytes, index);
}

// ---- reading ----------------------------------------------------------------------------------

/** Half the side, in canvas units, of the square a petal template is tabulated over. */
const TEMPLATE_HALF = 9.0;
/** Template table step in canvas units. */
const TEMPLATE_STEP = 0.25;
/** Template table side in steps. */
const TEMPLATE_SIDE = 73;
/** The last table index, as the bound of the bilinear interpolation. */
const TEMPLATE_LAST = TEMPLATE_SIDE - 1;
/** From this fitted blur on (3.2 units), the camera cannot resolve the turns and the reader stops. */
const BLUR_LIMIT = 4;
/** The window of a carrier is sampled every canvas unit over +-`GRID_HALF` units in its own frame. */
const GRID_HALF = 6;
/** Window side in samples. */
const GRID_SIDE = 13;
/** Samples of a window. */
const GRID_POINTS = GRID_SIDE * GRID_SIDE;
/** Sigma in canvas units of the Gaussian that weights the window. */
const WINDOW_SIGMA = 3.6;
/** Sigma in canvas units of the Gaussian that follows a carrier's centroid. */
const CENTROID_SIGMA = 2.6;
/** Lit petals within this distance of a carrier are modelled as its neighbours. */
const NEIGHBOUR_DISTANCE = 19.0;
/**
 * A neighbour's template is subtracted within this distance of its centre. Radially adjacent cell
 * centres are 11 units apart and the window grid is integral, so a whole number here would put grid
 * points exactly on the boundary, where the last bit of a platform's `sin` and `cos` decides.
 */
const NEIGHBOUR_REACH = 9.5;
/** The warp and the blur are fitted on every this-many-th carrier. */
const SAMPLE_STEP = 17;
/** The 10th percentile of a window's samples (the 17th smallest) becomes 0 ... */
const WINDOW_LOW = Math.floor(GRID_POINTS / 10);
/** ... and the 98th (the 166th smallest) becomes 1. */
const WINDOW_HIGH = Math.floor((GRID_POINTS * 98) / 100);
/** Carriers of a capture whose four sectors decoded. */
const MAX_CARRIERS = SECTORS * TURN_CARRIERS;
/** The smallest samples of a window kept to find its `WINDOW_LOW`-th smallest ... */
const LOW_KEEP = WINDOW_LOW + 1;
/** ... and the largest kept to find its `WINDOW_HIGH`-th smallest. */
const HIGH_KEEP = GRID_POINTS - WINDOW_HIGH;
const LOWS = new Float64Array(LOW_KEEP);
const HIGHS = new Float64Array(HIGH_KEEP);

/**
 * A petal template at a point of its own frame (`along` its axis, `across` it): the bilinear
 * interpolation of `table`, zero outside it.
 */
function templateAt(table, along, across) {
  const x = (along + TEMPLATE_HALF) / TEMPLATE_STEP;
  const y = (across + TEMPLATE_HALF) / TEMPLATE_STEP;
  if (!(x >= 0 && y >= 0 && x < TEMPLATE_LAST && y < TEMPLATE_LAST)) {
    return 0;
  }
  const i = x | 0;
  const j = y | 0;
  const fx = x - i;
  const fy = y - j;
  const row = j * TEMPLATE_SIDE + i;
  const top = table[row] * (1 - fx) + table[row + 1] * fx;
  const bottom = table[row + TEMPLATE_SIDE] * (1 - fx) + table[row + TEMPLATE_SIDE + 1] * fx;
  return top * (1 - fy) + bottom * fy;
}

/** The coverage of every table cell by the unturned petal (4 x 4 samples per cell). */
function petalCoverage() {
  const side = TEMPLATE_SIDE;
  const cover = new Float64Array(side * side);
  for (let j = 0; j < side; j += 1) {
    for (let i = 0; i < side; i += 1) {
      const along = -TEMPLATE_HALF + i * TEMPLATE_STEP;
      const across = -TEMPLATE_HALF + j * TEMPLATE_STEP;
      let hits = 0;
      for (let sj = 0; sj < 4; sj += 1) {
        for (let si = 0; si < 4; si += 1) {
          const da = ((si - 1.5) * TEMPLATE_STEP) / 4;
          const dc = ((sj - 1.5) * TEMPLATE_STEP) / 4;
          // an unturned petal: sin 0 = 0, cos 0 = 1
          if (petalLitAlong(along + da, across + dc, 0, 1)) {
            hits += 1;
          }
        }
      }
      cover[j * side + i] = hits / 16;
    }
  }
  return cover;
}

/**
 * The petal coverage blurred by a Gaussian of `sigma` canvas units truncated at `3 sigma` (rows,
 * then columns; taps outside the table count as zero, and the kernel is normalised by its full
 * sum).
 */
function blurredTemplate(cover, sigma) {
  const side = TEMPLATE_SIDE;
  const radius = Math.ceil((3 * sigma) / TEMPLATE_STEP);
  const taps = new Float64Array(2 * radius + 1);
  let norm = -0;
  for (let k = -radius; k <= radius; k += 1) {
    const t = k * TEMPLATE_STEP;
    taps[k + radius] = Math.exp(-(t * t) / (2 * sigma * sigma));
    norm += taps[k + radius];
  }
  const blur = (source, rows) => {
    const out = new Float64Array(side * side);
    const step = rows ? 1 : side;
    for (let j = 0; j < side; j += 1) {
      for (let i = 0; i < side; i += 1) {
        // the taps that land inside the table, in tap order
        const at = rows ? i : j;
        const first = Math.max(radius - at, 0);
        const last = Math.min(radius + side - 1 - at, taps.length - 1);
        const origin = j * side + i - radius * step;
        let sum = 0;
        for (let t = first; t <= last; t += 1) {
          sum += taps[t] * source[origin + t * step];
        }
        out[j * side + i] = sum / norm;
      }
    }
    return out;
  };
  return blur(blur(cover, true), false);
}

let readerTables = null;

/**
 * The window grid (along the outward direction fastest) with its Gaussian weights, and per blur
 * the template and the weighted kernel of every turn position on the grid; built on first use.
 */
function tables() {
  if (readerTables === null) {
    const gridA = new Float64Array(GRID_POINTS);
    const gridB = new Float64Array(GRID_POINTS);
    const gridW = new Float64Array(GRID_POINTS);
    let g = 0;
    for (let b = -GRID_HALF; b <= GRID_HALF; b += 1) {
      for (let a = -GRID_HALF; a <= GRID_HALF; a += 1) {
        gridA[g] = a;
        gridB[g] = b;
        gridW[g] = Math.exp(-(a * a + b * b) / (2 * WINDOW_SIGMA * WINDOW_SIGMA));
        g += 1;
      }
    }
    const cover = petalCoverage();
    const templates = TURN_BLURS.map((sigma) => blurredTemplate(cover, sigma));
    const kernels = templates.map((table) =>
      Array.from({ length: TURN_POSITIONS }, (_, k) => {
        const sin = TURN_SIN[k];
        const cos = TURN_COS[k];
        const kernel = new Float64Array(GRID_POINTS);
        for (let index = 0; index < GRID_POINTS; index += 1) {
          const a = gridA[index];
          const b = gridB[index];
          kernel[index] = gridW[index] * templateAt(table, a * cos + b * sin, -a * sin + b * cos);
        }
        return kernel;
      }),
    );
    readerTables = { gridA, gridB, gridW, templates, kernels };
  }
  return readerTables;
}

/**
 * The turn reader's blurred petal template for blur `blur` (an index into `TURN_BLURS`) at a point
 * of the unturned petal's own frame (`along` its axis, `across` it): what conformance tests compare
 * an SDK's tables with.
 */
export function turnTemplate(blur, along, across) {
  requireIndex(blur, "blur index");
  if (blur >= TURN_BLURS.length) {
    throw new RangeError("blur index out of range");
  }
  if (typeof along !== "number" || typeof across !== "number") {
    throw new TypeError("template coordinates must be numbers");
  }
  return templateAt(tables().templates[blur], along, across);
}

let neighbourTable = null;

/**
 * The cells within `NEIGHBOUR_DISTANCE` of every field cell (itself excluded), in index order, as
 * `cells[start[c] .. start[c + 1]]`, with the other centre (`along`, `across`) and outward
 * direction (`angle`) in the cell's own outward frame. Bands are 11 units apart, so only the two
 * bands on either side can hold them. Built on first use. Exported for tests only.
 */
export function neighbours() {
  if (neighbourTable === null) {
    const start = new Uint32Array(FIELD_CELLS + 1);
    const cells = [];
    const along = [];
    const across = [];
    const angles = [];
    const limit = NEIGHBOUR_DISTANCE * NEIGHBOUR_DISTANCE;
    for (let cell = 0; cell < FIELD_CELLS; cell += 1) {
      start[cell] = cells.length;
      const x = CELL_CENTER_X[cell];
      const y = CELL_CENTER_Y[cell];
      const angle = CELL_ANGLE[cell];
      const sin = CELL_SIN[cell];
      const cos = CELL_COS[cell];
      const band = CELL_BAND[cell];
      const first = BAND_OFFSETS[Math.max(band - 2, 0)];
      const last = BAND_OFFSETS[Math.min(band + 3, BAND_COUNT)];
      for (let other = first; other < last; other += 1) {
        if (other === cell) continue;
        const dx = CELL_CENTER_X[other] - x;
        const dy = CELL_CENTER_Y[other] - y;
        if (dx * dx + dy * dy <= limit) {
          cells.push(other);
          along.push(dx * cos + dy * sin);
          across.push(-dx * sin + dy * cos);
          angles.push(CELL_ANGLE[other] - angle);
        }
      }
    }
    start[FIELD_CELLS] = cells.length;
    neighbourTable = Object.freeze({
      start,
      cells: Uint16Array.from(cells),
      along: Float64Array.from(along),
      across: Float64Array.from(across),
      angle: Float64Array.from(angles),
    });
  }
  return neighbourTable;
}

// Buffers of the reader, allocated on first use and reused by every read.
let buffers = null;

function readerBuffers() {
  if (buffers === null) {
    const samples = Math.ceil(MAX_CARRIERS / SAMPLE_STEP);
    buffers = {
      lit: new Uint8Array(FIELD_CELLS),
      carrierOf: new Int32Array(FIELD_CELLS),
      cells: new Uint16Array(MAX_CARRIERS),
      windows: new Float64Array(MAX_CARRIERS * GRID_POINTS),
      positions: new Uint8Array(MAX_CARRIERS),
      leads: new Float64Array(MAX_CARRIERS),
      cache: new Int8Array(MAX_CARRIERS),
      scratch: new Float64Array(GRID_POINTS),
      model: new Float64Array(GRID_POINTS),
      sorted: new Float64Array(GRID_POINTS),
      pointX: new Float64Array(samples),
      pointY: new Float64Array(samples),
      offsetX: new Float64Array(samples),
      offsetY: new Float64Array(samples),
      fitX: new Float64Array(10),
      fitY: new Float64Array(10),
    };
  }
  return buffers;
}

// Small scratch values of the reader's helpers.
const POINT = new Float64Array(2);
const CENTROID = new Float64Array(2);
const ALONG = new Float64Array(GRID_SIDE);
const ACROSS = new Float64Array(GRID_SIDE);
const MONOMIALS = new Float64Array(10);
const NORMAL = new Float64Array(10 * 11);
const SCORES = new Float64Array(TURN_POSITIONS);

/** Maps canvas `(x, y)` through `pose` (exactly as `PetalPose.apply`) into `POINT`. */
function applyPose(pose, x, y) {
  const m = pose._m;
  const w = m[6] * x + m[7] * y + m[8];
  let px = (m[0] * x + m[1] * y + m[2]) / w;
  let py = (m[3] * x + m[4] * y + m[5]) / w;
  const k = pose.distortion;
  if (k !== 0) {
    const dx = px - pose._cx;
    const dy = py - pose._cy;
    const factor = 1 + (k * (dx * dx + dy * dy)) / pose.scale2;
    px = pose._cx + dx * factor;
    py = pose._cy + dy * factor;
  }
  POINT[0] = px;
  POINT[1] = py;
}

/**
 * Samples the image on the window grid of the petal at `cell`, centred `(shiftX, shiftY)` canvas
 * units off the cell centre, in the petal's outward frame (grid rows across, columns along), into
 * `out` from `offset`. Within a window the pose is taken as affine: with `o` the image point of the
 * centre and `u`, `v` the image steps of one unit along and across, grid point `(a, b)` is sampled
 * at `(o + b v) + a u`.
 */
function sampleWindow(image, pose, cell, shiftX, shiftY, out, offset) {
  const x = CELL_CENTER_X[cell] + shiftX;
  const y = CELL_CENTER_Y[cell] + shiftY;
  const sin = CELL_SIN[cell];
  const cos = CELL_COS[cell];
  applyPose(pose, x, y);
  const ox = POINT[0];
  const oy = POINT[1];
  applyPose(pose, x + cos, y + sin);
  const ux = POINT[0] - ox;
  const uy = POINT[1] - oy;
  applyPose(pose, x - sin, y + cos);
  const vx = POINT[0] - ox;
  const vy = POINT[1] - oy;
  const { data, width, height } = image;
  let index = offset;
  for (let b = -GRID_HALF; b <= GRID_HALF; b += 1) {
    const rowX = ox + b * vx;
    const rowY = oy + b * vy;
    for (let a = -GRID_HALF; a <= GRID_HALF; a += 1) {
      out[index] = sampleLuma(data, width, height, rowX + a * ux, rowY + a * uy);
      index += 1;
    }
  }
}

/**
 * Rescales the window at `offset` so that its 10th percentile (of 169 samples, the 17th smallest)
 * becomes 0 and its 98th (the 166th smallest) 1 (a span below one grey level counts as one),
 * clamped to -0.2 ... 1.2.
 *
 * The two order statistics come from one pass that keeps the 17 smallest and the 4 largest
 * samples in order; a window holding NaN (only a broken pose samples one) is sorted instead, in the
 * reference's `total_cmp` order.
 */
function normalise(window, offset, sorted) {
  let lows = 0;
  let highs = 0;
  let nan = false;
  for (let index = offset; index < offset + GRID_POINTS; index += 1) {
    const value = window[index];
    nan ||= value !== value;
    if (lows < LOW_KEEP || value < LOWS[LOW_KEEP - 1]) {
      let at = lows < LOW_KEEP ? lows : LOW_KEEP - 1;
      while (at > 0 && LOWS[at - 1] > value) {
        LOWS[at] = LOWS[at - 1];
        at -= 1;
      }
      LOWS[at] = value;
      if (lows < LOW_KEEP) lows += 1;
    }
    if (highs < HIGH_KEEP || value > HIGHS[HIGH_KEEP - 1]) {
      let at = highs < HIGH_KEEP ? highs : HIGH_KEEP - 1;
      while (at > 0 && HIGHS[at - 1] < value) {
        HIGHS[at] = HIGHS[at - 1];
        at -= 1;
      }
      HIGHS[at] = value;
      if (highs < HIGH_KEEP) highs += 1;
    }
  }
  let bottom = LOWS[LOW_KEEP - 1];
  let top = HIGHS[HIGH_KEEP - 1];
  if (nan) {
    for (let index = 0; index < GRID_POINTS; index += 1) sorted[index] = window[offset + index];
    // ascending, -0 before +0 and NaN last: the reference's `total_cmp` order
    sorted.sort();
    bottom = sorted[WINDOW_LOW];
    top = sorted[WINDOW_HIGH];
  }
  const span = fmax(top - bottom, 1);
  for (let index = offset; index < offset + GRID_POINTS; index += 1) {
    window[index] = clamp((window[index] - bottom) / span, -0.2, 1.2);
  }
}

/**
 * The centroid (along, across) of the light of a normalised window, into `CENTROID`: every sample
 * weighs its level (clamped to 0 ... 1) times a Gaussian of sigma `CENTROID_SIGMA` that starts at
 * the window centre and follows the centroid three times; the Gaussian is its along factor times
 * its across factor.
 */
function centroid(window, offset) {
  let ea = 0;
  let eb = 0;
  const k = 2 * CENTROID_SIGMA * CENTROID_SIGMA;
  for (let round = 0; round < 3; round += 1) {
    for (let i = 0; i < GRID_SIDE; i += 1) {
      const t = i - GRID_HALF;
      ALONG[i] = Math.exp((-(t - ea) * (t - ea)) / k);
      ACROSS[i] = Math.exp((-(t - eb) * (t - eb)) / k);
    }
    let sum = 0;
    let sa = 0;
    let sb = 0;
    for (let j = 0; j < GRID_SIDE; j += 1) {
      const b = j - GRID_HALF;
      const row = offset + j * GRID_SIDE;
      for (let i = 0; i < GRID_SIDE; i += 1) {
        const a = i - GRID_HALF;
        const w = clamp(window[row + i], 0, 1) * ALONG[i] * ACROSS[j];
        sum += w;
        sa += w * a;
        sb += w * b;
      }
    }
    if (sum <= 0) {
      break;
    }
    ea = sa / sum;
    eb = sb / sum;
  }
  CENTROID[0] = ea;
  CENTROID[1] = eb;
}

/** The ten cubic monomials of a canvas point, centred on the canvas and scaled by 384, into `out`. */
function monomials(x, y, out) {
  const u = (x - 512) / 384;
  const v = (y - 512) / 384;
  out[0] = 1;
  out[1] = u;
  out[2] = v;
  out[3] = u * u;
  out[4] = u * v;
  out[5] = v * v;
  out[6] = u * u * u;
  out[7] = u * u * v;
  out[8] = u * v * v;
  out[9] = v * v * v;
}

/**
 * The least-squares cubic through `values` at the first `n` points `(xs, ys)`, into `out`: normal
 * equations (summed point by point) with 1e-6 added to the diagonal, solved by Gauss-Jordan
 * elimination with partial pivoting (a pivot below 1e-12 skips its column).
 */
function fitCubic(xs, ys, values, n, out) {
  const a = NORMAL.fill(0);
  for (let point = 0; point < n; point += 1) {
    monomials(xs[point], ys[point], MONOMIALS);
    const value = values[point];
    for (let r = 0; r < 10; r += 1) {
      const f = MONOMIALS[r];
      for (let c = 0; c < 10; c += 1) {
        a[r * 11 + c] += f * MONOMIALS[c];
      }
      a[r * 11 + 10] += f * value;
    }
  }
  for (let r = 0; r < 10; r += 1) {
    a[r * 11 + r] += 1e-6;
  }
  for (let col = 0; col < 10; col += 1) {
    let pivot = col;
    for (let r = col + 1; r < 10; r += 1) {
      if (Math.abs(a[r * 11 + col]) > Math.abs(a[pivot * 11 + col])) {
        pivot = r;
      }
    }
    if (pivot !== col) {
      for (let c = 0; c < 11; c += 1) {
        const swap = a[col * 11 + c];
        a[col * 11 + c] = a[pivot * 11 + c];
        a[pivot * 11 + c] = swap;
      }
    }
    const p = a[col * 11 + col];
    if (Math.abs(p) < 1e-12) {
      continue;
    }
    for (let c = col; c < 11; c += 1) {
      a[col * 11 + c] /= p;
    }
    for (let r = 0; r < 10; r += 1) {
      if (r === col) continue;
      const f = a[r * 11 + col];
      for (let c = col; c < 11; c += 1) {
        a[r * 11 + c] -= f * a[col * 11 + c];
      }
    }
  }
  for (let r = 0; r < 10; r += 1) {
    out[r] = a[r * 11 + 10];
  }
}

function evalCubic(coefficients, x, y) {
  monomials(x, y, MONOMIALS);
  let sum = -0;
  for (let k = 0; k < 10; k += 1) {
    sum += MONOMIALS[k] * coefficients[k];
  }
  return sum;
}

/**
 * Adds `sign x` the template of neighbour entry `entry` (see `neighbours`), turned by `turn`, to the
 * window at `offset`, at the grid points within `NEIGHBOUR_REACH` of the neighbour's centre.
 */
function addNeighbour(window, offset, table, near, entry, turn, sign) {
  const angle = near.angle[entry] + turn;
  const sin = Math.sin(angle);
  const cos = Math.cos(angle);
  const along = near.along[entry];
  const across = near.across[entry];
  const reach2 = NEIGHBOUR_REACH * NEIGHBOUR_REACH;
  // only the grid rows and columns within reach of the neighbour's centre
  const a0 = fmax(Math.ceil(along - NEIGHBOUR_REACH), -GRID_HALF);
  const a1 = fmin(Math.floor(along + NEIGHBOUR_REACH), GRID_HALF);
  const b0 = fmax(Math.ceil(across - NEIGHBOUR_REACH), -GRID_HALF);
  const b1 = fmin(Math.floor(across + NEIGHBOUR_REACH), GRID_HALF);
  for (let b = b0; b <= b1; b += 1) {
    const ub = b - across;
    const row = offset + (b + GRID_HALF) * GRID_SIDE + GRID_HALF;
    for (let a = a0; a <= a1; a += 1) {
      const ua = a - along;
      if (ua * ua + ub * ub > reach2) {
        continue;
      }
      window[row + a] += sign * templateAt(table, ua * cos + ub * sin, -ua * sin + ub * cos);
    }
  }
}

/**
 * The best turn position for the window at `offset` (ties: the lower position), written to
 * `positions[index]`, and how far its score leads the runner-up's, written to `leads[index]`.
 */
function decide(window, offset, kernels, positions, leads, index) {
  const k0 = kernels[0];
  const k1 = kernels[1];
  const k2 = kernels[2];
  const k3 = kernels[3];
  let s0 = -0;
  let s1 = -0;
  let s2 = -0;
  let s3 = -0;
  for (let g = 0; g < GRID_POINTS; g += 1) {
    const x = window[offset + g];
    s0 += x * k0[g];
    s1 += x * k1[g];
    s2 += x * k2[g];
    s3 += x * k3[g];
  }
  SCORES[0] = s0;
  SCORES[1] = s1;
  SCORES[2] = s2;
  SCORES[3] = s3;
  let best = 0;
  for (let k = 1; k < TURN_POSITIONS; k += 1) {
    if (SCORES[k] > SCORES[best]) best = k;
  }
  let runnerUp = -Infinity;
  for (let k = 0; k < TURN_POSITIONS; k += 1) {
    if (k !== best) runnerUp = fmax(runnerUp, SCORES[k]);
  }
  positions[index] = best;
  leads[index] = SCORES[best] - runnerUp;
}

/** The four weighted kernels of blur `blur` (an index into `TURN_BLURS`). Exported for tests only. */
export function turnKernels(blur) {
  return tables().kernels[blur];
}

/**
 * The best turn position of a 169-sample `window` under `kernels` and how far it leads the
 * runner-up, as `[position, lead]`. Exported for tests only.
 */
export function decideWindow(window, kernels) {
  const positions = new Uint8Array(1);
  const leads = new Float64Array(1);
  decide(window, 0, kernels, positions, leads, 0);
  return [positions[0], leads[0]];
}

/**
 * Decides the turn of every carrier of the sectors `frame` decoded, when a sector that `need`
 * names has carriers; with `limit`, not when the fitted blur reaches `BLUR_LIMIT`. The decisions
 * are left in the reader's buffers (valid until the next read).
 *
 * 1. The lit petals of the decoded sectors come from their field lanes, and the carriers of each
 *    from `carriers`.
 * 2. The pose's smooth error: for every carrier in steps of `SAMPLE_STEP`, the window is sampled
 *    at its cell centre and normalised, and its centroid turned into a canvas offset; a cubic
 *    fitted to the offsets over the canvas moves every carrier's window, which is then sampled
 *    and normalised.
 * 3. The blur: for every carrier in steps of `SAMPLE_STEP`, the model of the petal at its best
 *    turn plus its lit neighbours (within `NEIGHBOUR_DISTANCE`) at theirs (each decided on its own
 *    window, unturned when it carries nothing) is correlated with the window, weighted by the
 *    window Gaussian; the blur with the largest sum fits best.
 * 4. The turns: with the next wider blur's kernels, every carrier takes the best turn of its own
 *    window, then, carrier by carrier in order, the best turn of its window with its lit
 *    neighbours' templates at their latest turns subtracted.
 *
 * @returns {{headers: Array<{tag: number, frame: number} | null>,
 *   ranges: Array<number | null>, count: number, fitted: number} | null}
 *   `ranges[s]` is the index of sector `s`'s first carrier (its carriers follow in order)
 */
function decideTurns(image, frame, need, limit) {
  const work = readerBuffers();
  const { lit, carrierOf, cells, windows, positions, leads, scratch, model, sorted } = work;
  // the lit petals of every sector that decoded
  lit.fill(0);
  const headers = [null, null, null, null];
  for (let sector = 0; sector < SECTORS; sector += 1) {
    const field = frame.fields[sector];
    if (field === null || field === undefined) continue;
    const data = toBytes(field.data, "field lane data");
    if (data.length < 3) continue;
    headers[sector] = Object.freeze({ tag: data[0], frame: (data[1] << 8) | data[2] });
    const word = encodeLane(fieldLane(sector), data);
    const order = SECTOR_CELL_LIST[sector];
    for (let bit = 0; bit < FIELD_BITS; bit += 1) {
      lit[order[bit]] = (word[bit >> 3] >> (7 - (bit & 7))) & 1;
    }
  }
  // the carriers of every decoded sector, sector by sector in carrier order
  const ranges = [null, null, null, null];
  let count = 0;
  let needed = false;
  for (let sector = 0; sector < SECTORS; sector += 1) {
    if (headers[sector] === null) continue;
    if (carriersInto(lit, sector, cells, count)) {
      ranges[sector] = count;
      count += TURN_CARRIERS;
      needed ||= need[sector];
    }
  }
  if (!needed) {
    return null;
  }
  const { gridA, gridB, gridW, templates, kernels } = tables();
  const near = neighbours();
  const pose = frame.pose;
  // the pose's smooth error, from the centroids of a sample of the carriers
  const { pointX, pointY, offsetX, offsetY, fitX, fitY } = work;
  let samples = 0;
  for (let index = 0; index < count; index += SAMPLE_STEP) {
    const cell = cells[index];
    sampleWindow(image, pose, cell, 0, 0, scratch, 0);
    normalise(scratch, 0, sorted);
    centroid(scratch, 0);
    const a = CENTROID[0];
    const b = CENTROID[1];
    const sin = CELL_SIN[cell];
    const cos = CELL_COS[cell];
    pointX[samples] = CELL_CENTER_X[cell];
    pointY[samples] = CELL_CENTER_Y[cell];
    offsetX[samples] = a * cos - b * sin;
    offsetY[samples] = a * sin + b * cos;
    samples += 1;
  }
  fitCubic(pointX, pointY, offsetX, samples, fitX);
  fitCubic(pointX, pointY, offsetY, samples, fitY);
  for (let index = 0; index < count; index += 1) {
    const cell = cells[index];
    const x = CELL_CENTER_X[cell];
    const y = CELL_CENTER_Y[cell];
    sampleWindow(image, pose, cell, evalCubic(fitX, x, y), evalCubic(fitY, x, y), windows, index * GRID_POINTS);
    normalise(windows, index * GRID_POINTS, sorted);
  }
  carrierOf.fill(-1);
  for (let index = 0; index < count; index += 1) {
    carrierOf[cells[index]] = index;
  }
  // the blur under which a sample of the carriers, each at its best turn with its lit neighbours
  // at theirs, explains the windows best
  const cache = work.cache;
  const turnOf = (carrier, blurKernels) => {
    if (cache[carrier] < 0) {
      decide(windows, carrier * GRID_POINTS, blurKernels, positions, leads, carrier);
      cache[carrier] = positions[carrier];
    }
    return cache[carrier];
  };
  let fitted = 0;
  let bestScore = -Infinity;
  for (let candidate = 0; candidate < TURN_BLURS.length; candidate += 1) {
    const table = templates[candidate];
    const blurKernels = kernels[candidate];
    cache.fill(-1, 0, count);
    let score = 0;
    for (let i = 0; i < count; i += SAMPLE_STEP) {
      const position = turnOf(i, blurKernels);
      const sin = TURN_SIN[position];
      const cos = TURN_COS[position];
      for (let g = 0; g < GRID_POINTS; g += 1) {
        const a = gridA[g];
        const b = gridB[g];
        model[g] = templateAt(table, a * cos + b * sin, -a * sin + b * cos);
      }
      const cell = cells[i];
      for (let entry = near.start[cell]; entry < near.start[cell + 1]; entry += 1) {
        const other = near.cells[entry];
        if (lit[other] === 0) continue;
        const j = carrierOf[other];
        const turn = j < 0 ? 0 : TURN_ANGLE[turnOf(j, blurKernels)];
        addNeighbour(model, 0, table, near, entry, turn, 1);
      }
      let cross = 0;
      let mm = 0;
      let xx = 0;
      const offset = i * GRID_POINTS;
      for (let g = 0; g < GRID_POINTS; g += 1) {
        const x = windows[offset + g];
        const m = model[g];
        const w = gridW[g];
        cross += w * x * m;
        mm += w * m * m;
        xx += w * x * x;
      }
      score += cross / fmax(Math.sqrt(mm * xx), 1e-9);
    }
    if (score > bestScore) {
      bestScore = score;
      fitted = candidate;
    }
  }
  if (limit && fitted >= BLUR_LIMIT) {
    return null;
  }
  // the turns, decided with the next wider blur
  const blur = Math.min(fitted + 1, TURN_BLURS.length - 1);
  const table = templates[blur];
  const blurKernels = kernels[blur];
  for (let index = 0; index < count; index += 1) {
    decide(windows, index * GRID_POINTS, blurKernels, positions, leads, index);
  }
  for (let index = 0; index < count; index += 1) {
    const offset = index * GRID_POINTS;
    for (let g = 0; g < GRID_POINTS; g += 1) scratch[g] = windows[offset + g];
    const cell = cells[index];
    for (let entry = near.start[cell]; entry < near.start[cell + 1]; entry += 1) {
      const other = near.cells[entry];
      if (lit[other] === 0) continue;
      const j = carrierOf[other];
      const turn = j < 0 ? 0 : TURN_ANGLE[positions[j]];
      addNeighbour(scratch, 0, table, near, entry, turn, -1);
    }
    decide(scratch, 0, blurKernels, positions, leads, index);
  }
  return { headers, ranges, count, fitted };
}

const NO_SKIP = Object.freeze([false, false, false, false]);
const EVERY_SECTOR = Object.freeze([true, true, true, true]);

/** The erasure schedule of a turn lane: 0, 1/8, 1/4, 1/3 and 1/2 of its parity (0, 2, 5, 7, 11). */
const TURN_SCHEDULE = Object.freeze(
  [0, TURN_PARITY >> 3, TURN_PARITY >> 2, Math.floor(TURN_PARITY / 3), TURN_PARITY >> 1].filter(
    (erasures, index, steps) => index === 0 || steps[index - 1] !== erasures,
  ),
);

/**
 * Tries Reed-Solomon with 0, 2, 5, 7 and 11 of the least confident bytes erased (ties keep byte
 * order) and accepts the first result whose check matches.
 */
function decodeTurnWord(sector, header, word, confidence) {
  const order = [];
  for (let index = 0; index < word.length; index += 1) order.push(index);
  // a stable sort, like the reference's
  order.sort((a, b) => totalCmp(confidence[a], confidence[b]));
  for (const erasures of TURN_SCHEDULE) {
    const positions = order.slice(0, erasures);
    const trial = Uint8Array.from(word);
    for (const position of positions) trial[position] = 0;
    const result = decodeLaneCountedStatus(TURN_SPECS[sector], trial, positions);
    if (typeof result === "number") {
      continue;
    }
    if (checks(header, sector, result.data)) {
      return Object.freeze({ data: result.data, corrected: result.corrected, erasures: positions.length });
    }
  }
  return null;
}

function isByteArray(value) {
  return value instanceof Uint8Array || value instanceof Uint8ClampedArray;
}

/**
 * Validates a luma plane and a decoded frame; returns whether the plane can be sampled (a plane
 * whose buffer does not match its size is read as nothing).
 */
function readable(image, frame) {
  if (
    image === null ||
    typeof image !== "object" ||
    !Number.isInteger(image.width) ||
    !Number.isInteger(image.height) ||
    image.width < 0 ||
    image.height < 0 ||
    !isByteArray(image.data)
  ) {
    throw new TypeError("image must be a PetalLuma or {width, height, data: Uint8Array}");
  }
  if (
    frame === null ||
    typeof frame !== "object" ||
    !(frame.pose instanceof PetalPose) ||
    !Array.isArray(frame.fields) ||
    frame.fields.length !== SECTORS
  ) {
    throw new TypeError("frame must be a decoded frame ({pose: PetalPose, fields})");
  }
  return image.width > 0 && image.height > 0 && image.data.length === image.width * image.height;
}

/**
 * Reads the turn lanes of the sectors `frame` decoded from `image`, except those in `skip`.
 *
 * The carriers of every decoded sector are decided (so that each carrier's neighbours are
 * modelled at their own turns), but only the lanes of the sectors not in `skip` are decoded, and
 * none when the image is too blurred to show turns. A lane passes when its Reed-Solomon codeword
 * decodes, with 0, 2, 5, 7 or 11 of its least confident bytes erased (a byte's confidence is the
 * smallest lead of its four carriers' turns), into data whose {@link turnCheck} matches its
 * sector's field lane header. Returns the lane data (atoms and check) per sector.
 *
 * @param {{width: number, height: number, data: Uint8Array | Uint8ClampedArray}} image the luma
 *   plane `frame` was decoded from
 * @param {{pose: PetalPose, fields: Array<{data: Uint8Array} | null>}} frame usually a
 *   `PetalDecodedFrame`
 * @param {readonly boolean[]} [skip] one flag per sector
 * @returns {Array<{data: Uint8Array, corrected: number, erasures: number} | null>} sector 0 first
 */
export function readTurns(image, frame, skip = NO_SKIP) {
  const usable = readable(image, frame);
  if (!Array.isArray(skip) || skip.length !== SECTORS) {
    throw new TypeError("skip must hold one flag per sector");
  }
  const results = [null, null, null, null];
  if (!usable) {
    return results;
  }
  const skipped = skip.map(Boolean);
  const reading = decideTurns(
    image,
    frame,
    skipped.map((flag) => !flag),
    true,
  );
  if (reading === null) {
    return results;
  }
  const { positions, leads } = readerBuffers();
  for (let sector = 0; sector < SECTORS; sector += 1) {
    const first = reading.ranges[sector];
    const header = reading.headers[sector];
    if (first === null || header === null || skipped[sector]) {
      continue;
    }
    const word = new Uint8Array(TURN_WORD);
    const confidence = new Float64Array(TURN_WORD).fill(Number.MAX_VALUE);
    for (let j = 0; j < TURN_CARRIERS; j += 1) {
      word[j >> 2] |= TURN_LABELS[positions[first + j]] << (6 - 2 * (j & 3));
      confidence[j >> 2] = fmin(confidence[j >> 2], leads[first + j]);
    }
    results[sector] = decodeTurnWord(sector, header, word, confidence);
  }
  return results;
}

/**
 * What {@link readTurns} decides for every carrier of the sectors `frame` decoded, before any
 * Reed-Solomon correction and whatever the blur, for diagnostics: the blur (Gaussian sigma in
 * canvas units) that fits the carriers best, and every carrier's cell, the turn position decided
 * for it and how far that position's score led the runner-up's (`lead`). `null` when no decoded
 * sector has carriers or the image cannot be sampled.
 *
 * @returns {{blur: number, carriers: Array<{cell: number, position: number, lead: number}>} | null}
 */
export function observedTurns(image, frame) {
  if (!readable(image, frame)) {
    return null;
  }
  const reading = decideTurns(image, frame, EVERY_SECTOR, false);
  if (reading === null) {
    return null;
  }
  const { cells, positions, leads } = readerBuffers();
  const list = [];
  for (let index = 0; index < reading.count; index += 1) {
    list.push(Object.freeze({ cell: cells[index], position: positions[index], lead: leads[index] }));
  }
  return Object.freeze({ blur: TURN_BLURS[reading.fitted], carriers: Object.freeze(list) });
}
