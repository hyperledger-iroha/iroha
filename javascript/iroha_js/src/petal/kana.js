// The kana layer: data in the katakana tiles of the bands above and below the square.
//
// The card (`CARD_HEIGHT`) is the square code with a band of katakana tiles
// above and below it, in the part of an upright phone's camera frame that the
// square leaves unused. Each band holds 306 tiles (`kanaTiles`); eight are
// pilots with fixed symbols (`KANA_PILOT_SYMBOLS`) and the other 298 carry two
// lanes:
//
// | lane              | cells                                    | codeword | data  | parity |
// |-------------------|------------------------------------------|----------|-------|--------|
// | P0, P1 polarity   | 296 data tiles x 1 bit (light or dark)   | 37 B     | 20 B  | 17 B   |
// | K0, K1 glyph      | 298 data tiles x 4 bits (one of sixteen) | 149 B    | 116 B | 33 B   |
//
// A light tile is filled light with its glyph in ink; a dark tile is black with
// its glyph in pink. Data tile `i` of a band (its tiles that are not pilots, in
// reading order) shows bit `i` of the band's polarity codeword (most
// significant bit first; the last two data tiles, beyond the codeword, are
// light) and nibble `i` of its glyph codeword (the high nibble of byte `i / 2`
// for even `i`).
//
// Every kana lane is one Reed-Solomon codeword, XOR-whitened: fountain atoms
// from their own repair range (`KANA_ID_BASE`, see `kanaFirstId`) and a CRC-32C
// check that binds them to the stream tag, the frame and the lane
// (`kanaCheck`). The lanes have no header: a reader takes the frame from the
// square's lanes and also tries the neighbouring frames, because a rolling
// shutter or a stale frame can show a band from the frame before or after the
// square.
//
// `readKana` reads the bands of a decoded capture. The square's pose, extended
// beyond the square, places every tile; each band's pilots are found by a small
// search around their places, and an affine correction fitted to their offsets
// moves every tile of the band. The pilots also pick the blur of the glyph
// templates and the levels that tell light tiles from dark ones. Each data tile
// then takes the polarity its mean level shows and the glyph whose template
// correlates best with its samples.
//
// Every floating-point expression follows the reference operation by operation
// (sums over a sequence start at -0 like Rust's `Iterator::sum`; the only `exp`
// is in the template blur weights). The templates are built once, on first use,
// and the reader's buffers are reused, so a read allocates almost nothing.
//
// This module does not import `lanes.js` (which imports it for the tile
// symbols), so the two load in either order.

import { crc32c } from "./crc.js";
import { PetalPose } from "./geometry.js";
import { GLYPH_COUNT } from "./glyphs.js";
import { sampleLuma } from "./image.js";
import {
  CANVAS,
  KANA_BANDS,
  KANA_BAND_DATA_TILES,
  KANA_DATA_TILE_LIST,
  KANA_PILOTS,
  KANA_PILOT_SYMBOLS,
  KANA_PILOT_TILE_LIST,
  KANA_TILE,
  KANA_TILE_COUNT,
  TILE_PILOT,
  TILE_X,
  TILE_Y,
  kanaInkedAt,
  kanaInside,
} from "./layout.js";
import { PetalXorshift32 } from "./prng.js";
import { PetalReedSolomon, RS_INVALID_SHAPE, RS_UNCORRECTABLE, rsCorrect } from "./rs.js";
import { PetalError, fmax, fmin, requireIndex, toBytes, totalCmp } from "./support.js";

/** Bytes of a fountain atom (`ATOM_LEN` of `lanes.js`). */
const ATOM_LEN = 16;

/** Data tiles of each band. */
export const KANA_DATA_TILES = KANA_BAND_DATA_TILES;
/** Codeword length of a glyph lane in bytes (two data tiles per byte). */
export const KANA_GLYPH_WORD = KANA_DATA_TILES / 2;
/** Parity bytes of a glyph lane. */
export const KANA_GLYPH_PARITY = 33;
/** Atoms carried by each glyph lane. */
export const KANA_GLYPH_ATOMS = 7;
/** Codeword length of a polarity lane in bytes (eight data tiles per byte). */
export const KANA_POLARITY_WORD = Math.floor(KANA_DATA_TILES / 8);
/** Parity bytes of a polarity lane. */
export const KANA_POLARITY_PARITY = 17;
/** Atoms carried by each polarity lane. */
export const KANA_POLARITY_ATOMS = 1;
/** Bytes of the check that closes a kana lane's data. */
export const KANA_CHECK_LEN = 4;
/**
 * Fountain id of the first kana atom of frame 0: `2^31 + 2^20`, just above the turn lanes' range
 * (`TURN_ID_BASE + 16 * 65536`).
 */
export const KANA_ID_BASE = 2 ** 31 + 2 ** 20;
/** Kana atom ids reserved per frame (eight per band). */
const KANA_IDS_PER_FRAME = 16;
/** The four kana lanes: the polarity and glyph lanes of the band above the square, then below it. */
export const KANA_LANES = Object.freeze(["P0", "K0", "P1", "K1"]);

if (
  KANA_GLYPH_WORD - KANA_GLYPH_PARITY !== KANA_GLYPH_ATOMS * ATOM_LEN + KANA_CHECK_LEN ||
  KANA_POLARITY_WORD - KANA_POLARITY_PARITY !== KANA_POLARITY_ATOMS * ATOM_LEN + KANA_CHECK_LEN ||
  KANA_GLYPH_ATOMS + KANA_POLARITY_ATOMS > KANA_IDS_PER_FRAME / KANA_BANDS ||
  KANA_TILE_COUNT !== KANA_BANDS * (KANA_DATA_TILES + KANA_PILOTS)
) {
  throw new Error("petal kana lane sizes are inconsistent");
}

function makeSpec(index) {
  const band = index >> 1;
  const glyph = (index & 1) === 1;
  const wordLen = glyph ? KANA_GLYPH_WORD : KANA_POLARITY_WORD;
  const parityLen = glyph ? KANA_GLYPH_PARITY : KANA_POLARITY_PARITY;
  // "POL0", "POL1" for polarity, "KAN0", "KAN1" for glyphs
  const whiteningSeed = (glyph ? 0x4b414e30 : 0x504f4c30) + band;
  const rng = new PetalXorshift32(whiteningSeed);
  const whitening = new Uint8Array(wordLen);
  for (let byte = 0; byte < wordLen; byte += 1) {
    whitening[byte] = rng.nextByte();
  }
  return Object.freeze({
    name: KANA_LANES[index],
    index,
    band,
    glyph,
    wordLen,
    parityLen,
    dataLen: wordLen - parityLen,
    atoms: glyph ? KANA_GLYPH_ATOMS : KANA_POLARITY_ATOMS,
    // the byte that names the lane in its check: `0x10 | band << 1 | kind` (polarity 0, glyph 1)
    code: 0x10 | (band << 1) | (glyph ? 1 : 0),
    whiteningSeed,
    rs: new PetalReedSolomon(parityLen),
    whitening,
  });
}

/** The four kana lanes' descriptions, in `KANA_LANES` order. */
const KANA_SPECS = Object.freeze([0, 1, 2, 3].map(makeSpec));

/** Resolves a kana lane name (`"P0"`, `"K0"`, `"P1"` or `"K1"`) to its internal description. */
export function kanaLaneSpec(lane) {
  const index = KANA_LANES.indexOf(lane);
  if (index < 0) {
    throw new TypeError('kana lane must be "P0", "K0", "P1" or "K1"');
  }
  return KANA_SPECS[index];
}

/** The kana lane of band `band` (0 above the square, 1 below it): its glyph lane when `glyph`, else its polarity lane. */
export function kanaLane(band, glyph) {
  requireIndex(band, "band");
  if (band >= KANA_BANDS) {
    throw new RangeError("band out of range");
  }
  return KANA_LANES[2 * band + (glyph ? 1 : 0)];
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

/** The whitening sequence of a kana lane: one byte per codeword byte from an xorshift32 seeded "POL0", "POL1", "KAN0" or "KAN1" (a copy). */
export function kanaWhitening(lane) {
  return kanaLaneSpec(lane).whitening.slice();
}

/** {@link kanaFirstId} with the arguments already resolved. */
function firstIdOf(frame, spec) {
  const slot = spec.glyph ? 0 : KANA_GLYPH_ATOMS;
  return KANA_ID_BASE + frame * KANA_IDS_PER_FRAME + spec.band * 8 + slot;
}

/**
 * Fountain id of the first atom `lane` carries in `frame` (a counter reduced modulo 65536):
 * `KANA_ID_BASE + 16 * frame + 8 * band`, plus 7 for the polarity lane (the glyph lane's seven
 * atoms come first).
 */
export function kanaFirstId(frame, lane) {
  return firstIdOf(frameNumber(frame), kanaLaneSpec(lane));
}

/** CRC-32C of `tag, frame:u16, code, atoms` (arguments already validated). */
function checkOf(tag, frame, spec, atoms) {
  const bytes = new Uint8Array(4 + atoms.length);
  bytes[0] = tag;
  bytes[1] = frame >> 8;
  bytes[2] = frame & 0xff;
  bytes[3] = spec.code;
  bytes.set(atoms, 4);
  return crc32c(bytes);
}

function readU32(bytes, offset) {
  return ((bytes[offset] << 24) | (bytes[offset + 1] << 16) | (bytes[offset + 2] << 8) | bytes[offset + 3]) >>> 0;
}

/**
 * The check of a kana lane: CRC-32C over the stream tag, the frame (big-endian), the lane's code
 * (`0x10 | band << 1 | kind`, polarity 0 and glyph 1) and the atoms.
 *
 * @param {{tag: number, frame: number}} header the stream tag and the frame
 * @returns {number} the check as an unsigned 32-bit integer
 */
export function kanaCheck(header, lane, atoms) {
  const { tag, frame } = requireHeader(header);
  return checkOf(tag, frame, kanaLaneSpec(lane), toBytes(atoms, "kana atoms"));
}

/**
 * The data of a kana lane: its atoms (one for a polarity lane, seven for a glyph lane) followed by
 * their {@link kanaCheck} (big-endian).
 *
 * @throws {RangeError} when `atoms` is not the lane's atom count long
 */
export function kanaData(header, lane, atoms) {
  const { tag, frame } = requireHeader(header);
  const spec = kanaLaneSpec(lane);
  const bytes = toBytes(atoms, "kana atoms");
  if (bytes.length !== spec.atoms * ATOM_LEN) {
    throw new RangeError("kana atoms length");
  }
  const data = new Uint8Array(spec.dataLen);
  data.set(bytes);
  const check = checkOf(tag, frame, spec, bytes);
  data[bytes.length] = check >>> 24;
  data[bytes.length + 1] = (check >>> 16) & 0xff;
  data[bytes.length + 2] = (check >>> 8) & 0xff;
  data[bytes.length + 3] = check & 0xff;
  return data;
}

/** Whether kana lane data (of the right length) checks out against `tag` and `frame`. */
function checks(tag, frame, spec, data) {
  const length = spec.atoms * ATOM_LEN;
  return checkOf(tag, frame, spec, data.subarray(0, length)) === readU32(data, length);
}

/**
 * The atoms of kana lane data whose check matches `header` and `lane`; `null` for data of the
 * wrong length or a check that does not match.
 *
 * @returns {{header: {tag: number, frame: number}, firstId: number, atoms: Uint8Array[]} | null}
 */
export function kanaPacket(header, lane, data) {
  const { tag, frame } = requireHeader(header);
  const spec = kanaLaneSpec(lane);
  const bytes = toBytes(data, "kana lane data");
  if (bytes.length !== spec.dataLen || !checks(tag, frame, spec, bytes)) {
    return null;
  }
  const atoms = [];
  for (let index = 0; index < spec.atoms; index += 1) {
    atoms.push(bytes.slice(index * ATOM_LEN, (index + 1) * ATOM_LEN));
  }
  return Object.freeze({
    header: Object.freeze({ tag, frame }),
    firstId: firstIdOf(frame, spec),
    atoms: Object.freeze(atoms),
  });
}

/**
 * Encodes kana lane data into the transmitted (whitened) codeword.
 *
 * @param {"P0" | "K0" | "P1" | "K1"} lane
 * @param {Uint8Array | ArrayBuffer | ArrayBufferView | number[]} data exactly the lane's data length
 * @returns {Uint8Array} the lane's codeword (37 or 149 bytes)
 */
export function encodeKanaLane(lane, data) {
  const spec = kanaLaneSpec(lane);
  const bytes = toBytes(data, "kana lane data");
  if (bytes.length !== spec.dataLen) {
    throw new RangeError("kana data length mismatch");
  }
  const word = spec.rs.encode(bytes);
  for (let index = 0; index < word.length; index += 1) {
    word[index] ^= spec.whitening[index];
  }
  return word;
}

/** Decodes a transmitted kana codeword: `{data, corrected}` or a negative status. */
function decodeStatus(spec, transmitted, erasures) {
  if (transmitted.length !== spec.wordLen) {
    return RS_INVALID_SHAPE;
  }
  const word = new Uint8Array(spec.wordLen);
  for (let index = 0; index < word.length; index += 1) {
    word[index] = transmitted[index] ^ spec.whitening[index];
  }
  const corrected = rsCorrect(spec.parityLen, word, erasures);
  if (corrected < 0) {
    return corrected;
  }
  return { data: word.slice(0, spec.dataLen), corrected };
}

/**
 * Decodes a transmitted kana codeword, returning its data and how many byte positions the
 * Reed-Solomon decoder rewrote. The check is not verified here (see {@link kanaPacket}).
 *
 * @param {readonly number[]} [erasures] byte positions the caller distrusts
 * @returns {{data: Uint8Array, corrected: number}}
 * @throws {PetalError} `rs_invalid_shape` or `rs_uncorrectable`
 */
export function decodeKanaLaneCounted(lane, transmitted, erasures = []) {
  const spec = kanaLaneSpec(lane);
  const bytes = toBytes(transmitted, "transmitted kana lane word");
  if (!Array.isArray(erasures)) {
    throw new TypeError("erasures must be an array of byte positions");
  }
  const result = decodeStatus(spec, bytes, erasures);
  if (result === RS_INVALID_SHAPE) throw new PetalError("rs_invalid_shape");
  if (result === RS_UNCORRECTABLE) throw new PetalError("rs_uncorrectable");
  return Object.freeze({ data: result.data, corrected: result.corrected });
}

// ---- tile symbols -------------------------------------------------------------------------------

/** Every tile light with glyph 0, then the pilots' fixed symbols, into `glyphs` and `light`. */
export function kanaPilotSymbolsInto(glyphs, light) {
  glyphs.fill(0);
  light.fill(1);
  for (let tile = 0; tile < KANA_TILE_COUNT; tile += 1) {
    const pilot = TILE_PILOT[tile];
    if (pilot < 0) continue;
    glyphs[tile] = KANA_PILOT_SYMBOLS[pilot][0];
    light[tile] = KANA_PILOT_SYMBOLS[pilot][1] ? 1 : 0;
  }
}

/**
 * Shows the transmitted codewords of band `band` on its data tiles, in `glyphs` and `light` (one
 * entry per tile): `polarity` (37 bytes; the data tiles beyond it are light) and `glyphWord`
 * (149 bytes). Arguments are validated by the caller.
 */
export function setKanaSymbolsInto(glyphs, light, band, polarity, glyphWord) {
  const tiles = KANA_DATA_TILE_LIST[band];
  for (let i = 0; i < KANA_DATA_TILES; i += 1) {
    const tile = tiles[i];
    light[tile] = i < KANA_POLARITY_WORD * 8 ? (polarity[i >> 3] >> (7 - (i & 7))) & 1 : 1;
    const byte = glyphWord[i >> 1];
    glyphs[tile] = (i & 1) === 0 ? byte >> 4 : byte & 0x0f;
  }
}

/** Packs the polarity codeword band `band`'s data tiles show under `light` (one flag per tile). */
export function kanaPolarityWordOf(light, band) {
  const tiles = KANA_DATA_TILE_LIST[band];
  const word = new Uint8Array(KANA_POLARITY_WORD);
  for (let i = 0; i < KANA_POLARITY_WORD * 8; i += 1) {
    if (light[tiles[i]]) {
      word[i >> 3] |= 1 << (7 - (i & 7));
    }
  }
  return word;
}

/** Packs the glyph codeword band `band`'s data tiles show under `glyphs` (one symbol per tile). */
export function kanaGlyphWordOf(glyphs, band) {
  const tiles = KANA_DATA_TILE_LIST[band];
  const word = new Uint8Array(KANA_GLYPH_WORD);
  for (let i = 0; i < KANA_DATA_TILES; i += 1) {
    const nibble = glyphs[tiles[i]] & 0x0f;
    word[i >> 1] |= (i & 1) === 0 ? nibble << 4 : nibble;
  }
  return word;
}

// ---- reading ------------------------------------------------------------------------------------

/** Gaussian blurs (sigma in canvas units) of the glyph templates; the pilots pick one per band. */
export const KANA_BLURS = Object.freeze([1.0, 1.6, 2.4, 3.4, 4.8]);
/**
 * The blur of the template the pilots are first searched with (3.4 units): coarse enough to find a
 * pilot on a 2-unit grid under any blur.
 */
const COARSE_BLUR = 3;
/** Step of the coarse pilot search in canvas units. */
const COARSE_STEP = 2.0;
/** Samples per tile side: points `(i + 1/2) * 25 / 12` units from the tile's top-left corner. */
export const KANA_GRID = 12;
/** Samples per tile side in the coarse pilot search: points `(i + 1/2) * 25 / 6`. */
export const KANA_COARSE_GRID = 6;
/** Template raster step in canvas units. */
const RASTER_STEP = 0.5;
/** The raster starts this far outside the tile square, so blur reaches in from the gutter. */
const RASTER_MARGIN = 4.0;
/** Raster side in steps. */
const RASTER_SIDE = 67;
/** A pilot whose coarse correlation is below this is not followed further. */
const PILOT_COARSE_MIN = 0.35;
/** A pilot whose final correlation is below this is not used. */
const PILOT_MIN = 0.5;

const POINTS = KANA_GRID * KANA_GRID;
const COARSE_POINTS = KANA_COARSE_GRID * KANA_COARSE_GRID;
const HALF_TILE = KANA_TILE / 2;
const REFINE_COARSE = Object.freeze([1.0]);
const REFINE_FINE = Object.freeze([1.0, 0.5, 0.25]);

/** Tile-local coordinate of grid index `i` on either axis of a grid of `side` points. */
function gridPoint(i, side) {
  return ((i + 0.5) * KANA_TILE) / side;
}

/** The fine grid's offsets from the tile centre on either axis. */
const FINE_OFFSETS = Float64Array.from({ length: KANA_GRID }, (_, i) => gridPoint(i, KANA_GRID) - HALF_TILE);
/** The coarse grid's offsets from the tile centre on either axis. */
const COARSE_OFFSETS = Float64Array.from({ length: KANA_COARSE_GRID }, (_, i) => gridPoint(i, KANA_COARSE_GRID) - HALF_TILE);

/**
 * Coverage on the template raster: raster point `a` is at `-4 + a / 2` units; each value is the
 * share of its 4 x 4 sub-points `(m - 1.5) / 8` units around it that `covered` holds.
 */
function raster(covered) {
  const table = new Float64Array(RASTER_SIDE * RASTER_SIDE);
  for (let b = 0; b < RASTER_SIDE; b += 1) {
    for (let a = 0; a < RASTER_SIDE; a += 1) {
      const u0 = -RASTER_MARGIN + a * RASTER_STEP;
      const v0 = -RASTER_MARGIN + b * RASTER_STEP;
      let hits = 0;
      for (let n = 0; n < 4; n += 1) {
        for (let m = 0; m < 4; m += 1) {
          if (covered(u0 + (m - 1.5) / 8, v0 + (n - 1.5) / 8)) {
            hits += 1;
          }
        }
      }
      table[b * RASTER_SIDE + a] = hits / 16;
    }
  }
  return table;
}

/**
 * Separable Gaussian blur of a raster: taps every `RASTER_STEP` out to `ceil(3 sigma / step)`,
 * weights `exp(-d^2 / (2 sigma^2))` normalised by their sum, points outside counting as zero; rows
 * first.
 */
function blurRaster(table, sigma) {
  const reach = Math.ceil((3 * sigma) / RASTER_STEP);
  const weights = new Float64Array(2 * reach + 1);
  let sum = -0;
  for (let k = -reach; k <= reach; k += 1) {
    const d = k * RASTER_STEP;
    weights[k + reach] = Math.exp(-(d * d) / (2 * sigma * sigma));
    sum += weights[k + reach];
  }
  for (let index = 0; index < weights.length; index += 1) {
    weights[index] /= sum;
  }
  const pass = (source, alongRows) => {
    const out = new Float64Array(RASTER_SIDE * RASTER_SIDE);
    for (let b = 0; b < RASTER_SIDE; b += 1) {
      for (let a = 0; a < RASTER_SIDE; a += 1) {
        let acc = 0;
        for (let k = -reach; k <= reach; k += 1) {
          const aa = alongRows ? a + k : a;
          const bb = alongRows ? b : b + k;
          if (aa < 0 || bb < 0 || aa >= RASTER_SIDE || bb >= RASTER_SIDE) {
            continue;
          }
          acc += weights[k + reach] * source[bb * RASTER_SIDE + aa];
        }
        out[b * RASTER_SIDE + a] = acc;
      }
    }
    return out;
  };
  return pass(pass(table, true), false);
}

/** A blurred raster at the `side x side` grid points (bilinear), mean-centred. */
function sampleRaster(table, side) {
  const out = new Float64Array(side * side);
  for (let j = 0; j < side; j += 1) {
    for (let i = 0; i < side; i += 1) {
      const fa = (gridPoint(i, side) + RASTER_MARGIN) / RASTER_STEP;
      const fb = (gridPoint(j, side) + RASTER_MARGIN) / RASTER_STEP;
      const a0 = Math.floor(fa);
      const b0 = Math.floor(fb);
      const ta = fa - a0;
      const tb = fb - b0;
      const at = (a, b) => table[b * RASTER_SIDE + a];
      const top = at(a0, b0) * (1 - ta) + at(a0 + 1, b0) * ta;
      const bottom = at(a0, b0 + 1) * (1 - ta) + at(a0 + 1, b0 + 1) * ta;
      out[j * side + i] = top * (1 - tb) + bottom * tb;
    }
  }
  let sum = -0;
  for (let p = 0; p < out.length; p += 1) sum += out[p];
  const mean = sum / out.length;
  for (let p = 0; p < out.length; p += 1) out[p] -= mean;
  return out;
}

/** The length of a vector, at least `1e-12`. */
function norm(values) {
  let sum = -0;
  for (let p = 0; p < values.length; p += 1) sum += values[p] * values[p];
  return fmax(Math.sqrt(sum), 1e-12);
}

/** A vector scaled by `1 / length`. */
function scaled(values, length) {
  return values.map((value) => value / length);
}

/** The light-tile template of a glyph: the inside less the ink, point by point. */
function less(inside, ink) {
  return inside.map((value, p) => value - ink[p]);
}

let kanaTables = null;

/**
 * The templates of every blur at the `KANA_GRID` points, each mean-centred: the tile's inside,
 * every glyph's ink (a dark tile's template), and every glyph's light-tile template (inside less
 * ink), with the lengths of the dark and light templates and both as unit vectors; and the coarse
 * templates of every glyph under `COARSE_BLUR` at the `KANA_COARSE_GRID` points, unit length.
 * Built on first use (it takes a moment on a phone core).
 */
function tables() {
  if (kanaTables === null) {
    const inside = raster(kanaInside);
    const inks = Array.from({ length: GLYPH_COUNT }, (_, glyph) => raster((u, v) => kanaInkedAt(glyph, u, v)));
    const coarse = { light: [], dark: [] };
    const blurs = KANA_BLURS.map((sigma, index) => {
      const insideBlurred = blurRaster(inside, sigma);
      const insidePoints = sampleRaster(insideBlurred, KANA_GRID);
      const t = {
        inside: insidePoints,
        ink: [],
        lightNorm: new Float64Array(GLYPH_COUNT),
        darkNorm: new Float64Array(GLYPH_COUNT),
        light: [],
        dark: [],
      };
      for (let glyph = 0; glyph < GLYPH_COUNT; glyph += 1) {
        const inkBlurred = blurRaster(inks[glyph], sigma);
        const inkPoints = sampleRaster(inkBlurred, KANA_GRID);
        const light = less(insidePoints, inkPoints);
        t.lightNorm[glyph] = norm(light);
        t.darkNorm[glyph] = norm(inkPoints);
        t.light.push(scaled(light, t.lightNorm[glyph]));
        t.dark.push(scaled(inkPoints, t.darkNorm[glyph]));
        t.ink.push(inkPoints);
        if (index === COARSE_BLUR) {
          const coarseInside = sampleRaster(insideBlurred, KANA_COARSE_GRID);
          const coarseInk = sampleRaster(inkBlurred, KANA_COARSE_GRID);
          const coarseLight = less(coarseInside, coarseInk);
          coarse.light.push(scaled(coarseLight, norm(coarseLight)));
          coarse.dark.push(scaled(coarseInk, norm(coarseInk)));
        }
      }
      return t;
    });
    kanaTables = { blurs, coarse };
  }
  return kanaTables;
}

function requireGlyphIndex(glyph) {
  requireIndex(glyph, "glyph");
  if (glyph >= GLYPH_COUNT) {
    throw new RangeError("glyph out of range");
  }
  return glyph;
}

function requireGridPoint(i, side) {
  requireIndex(i, "grid index");
  if (i >= side) {
    throw new RangeError("grid point out of range");
  }
  return i;
}

/**
 * The template value of `glyph` (a light or dark tile) under blur `blur` (an index into
 * `KANA_BLURS`) at grid point `(i, j)` of the 12 x 12 grid, mean-centred and scaled to unit length:
 * what conformance tests compare an SDK's tables with.
 */
export function kanaTemplateProbe(blur, glyph, light, i, j) {
  requireIndex(blur, "blur index");
  if (blur >= KANA_BLURS.length) {
    throw new RangeError("blur index out of range");
  }
  requireGlyphIndex(glyph);
  requireGridPoint(i, KANA_GRID);
  requireGridPoint(j, KANA_GRID);
  const t = tables().blurs[blur];
  return (light ? t.light : t.dark)[glyph][j * KANA_GRID + i];
}

/**
 * The coarse template value of `glyph` (a light or dark tile) at grid point `(i, j)` of the 6 x 6
 * coarse grid: what conformance tests compare an SDK's tables with.
 */
export function kanaCoarseTemplateProbe(glyph, light, i, j) {
  requireGlyphIndex(glyph);
  requireGridPoint(i, KANA_COARSE_GRID);
  requireGridPoint(j, KANA_COARSE_GRID);
  const coarse = tables().coarse;
  return (light ? coarse.light : coarse.dark)[glyph][j * KANA_COARSE_GRID + i];
}

// Scratch of the reader, reused by every read.
const POINT = new Float64Array(2);
const FINE_SAMPLES = new Float64Array(POINTS);
const COARSE_SAMPLES = new Float64Array(COARSE_POINTS);
/** The mean and energy of the last `sampleTile`. */
let tileMean = 0;
let tileEnergy = 0;
/** The best shift of the last pilot search and its correlation. */
let bestX = 0;
let bestY = 0;
let bestScore = 0;
// pilots kept after the coarse search, then found
const COARSE_TILE = new Uint16Array(KANA_PILOTS);
const COARSE_X = new Float64Array(KANA_PILOTS);
const COARSE_Y = new Float64Array(KANA_PILOTS);
const FOUND_CENTRE_X = new Float64Array(KANA_PILOTS);
const FOUND_CENTRE_Y = new Float64Array(KANA_PILOTS);
const FOUND_SHIFT_X = new Float64Array(KANA_PILOTS);
const FOUND_SHIFT_Y = new Float64Array(KANA_PILOTS);
const FOUND_LEVEL = new Float64Array(KANA_PILOTS);
const FOUND_LIGHT = new Uint8Array(KANA_PILOTS);
// the band's shift field and level lines
const FIELD_X = new Float64Array(3);
const FIELD_Y = new Float64Array(3);
let fieldCentre = 0;
const SHIFT = new Float64Array(2);
const LINE_X = new Float64Array(KANA_PILOTS);
const LINE_Y = new Float64Array(KANA_PILOTS);
const LINE = new Float64Array(2);
// the band's readings, in data order
const TILE_READ = new Uint8Array(KANA_DATA_TILES);
const TILE_LIGHT = new Uint8Array(KANA_DATA_TILES);
const TILE_MARGIN = new Float64Array(KANA_DATA_TILES);
const TILE_GLYPH = new Uint8Array(KANA_DATA_TILES);
const TILE_LEAD = new Float64Array(KANA_DATA_TILES);
const SCORES = new Float64Array(GLYPH_COUNT);
let bandBlur = 0;
let bandPilots = 0;

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
 * Samples the tile whose top-left corner is `(x, y)` (square coordinates), moved by
 * `(shiftX, shiftY)`, at the grid points `offsets` into `out`: the pose maps the tile's centre `o`
 * and the points one unit to its right and below it (`o + u`, `o + v`), and grid point `(a, b)` from
 * the centre is sampled at `(o + b v) + a u`. The samples are left minus their mean; that mean goes
 * to `tileMean` and their energy (the sum of their squares) to `tileEnergy`.
 */
function sampleTile(image, pose, x, y, shiftX, shiftY, offsets, out) {
  const cx = x + HALF_TILE + shiftX;
  const cy = y + HALF_TILE + shiftY;
  applyPose(pose, cx, cy);
  const ox = POINT[0];
  const oy = POINT[1];
  applyPose(pose, cx + 1, cy);
  const ux = POINT[0] - ox;
  const uy = POINT[1] - oy;
  applyPose(pose, cx, cy + 1);
  const vx = POINT[0] - ox;
  const vy = POINT[1] - oy;
  const { data, width, height } = image;
  const side = offsets.length;
  let sum = 0;
  let index = 0;
  for (let j = 0; j < side; j += 1) {
    const b = offsets[j];
    const rowX = ox + b * vx;
    const rowY = oy + b * vy;
    for (let i = 0; i < side; i += 1) {
      const a = offsets[i];
      const value = sampleLuma(data, width, height, rowX + a * ux, rowY + a * uy);
      out[index] = value;
      sum += value;
      index += 1;
    }
  }
  const mean = sum / index;
  let energy = 0;
  for (let k = 0; k < index; k += 1) {
    const value = out[k] - mean;
    out[k] = value;
    energy += value * value;
  }
  tileMean = mean;
  tileEnergy = energy;
}

/** The dot product of `samples` with `template` (both `template.length` long). */
function dot(samples, template) {
  let sum = -0;
  for (let k = 0; k < template.length; k += 1) {
    sum += samples[k] * template[k];
  }
  return sum;
}

/** Correlation of mean-centred samples of energy `energy` with a unit, mean-centred template. */
function correlate(samples, energy, template) {
  if (energy <= 1e-9) {
    return 0;
  }
  return dot(samples, template) / Math.sqrt(energy);
}

/** Whether every corner of tile `tile` maps inside the image. */
function tileInImage(image, pose, tile) {
  const x = TILE_X[tile];
  const y = TILE_Y[tile];
  for (let corner = 0; corner < 4; corner += 1) {
    // (0, 0), (25, 0), (0, 25), (25, 25)
    applyPose(pose, x + ((corner & 1) === 1 ? KANA_TILE : 0), y + (corner >= 2 ? KANA_TILE : 0));
    const px = POINT[0];
    const py = POINT[1];
    if (!(px >= 0 && py >= 0 && px < image.width && py < image.height)) {
      return false;
    }
  }
  return true;
}

/**
 * How far (canvas units) the pose can miss a pilot centred `y` units down the square (negative
 * above it): 4 units at the square's edge, growing by 10 over a band's 290-unit height, because a
 * lens's distortion grows away from the square the pose was fitted to.
 */
function pilotReach(y) {
  const beyond = y < 0 ? -y : y - CANVAS;
  return 4 + 10 * fmin(fmax(beyond, 0) / 290, 1.2);
}

/** The correlation of pilot tile `tile`'s samples, moved by `(shiftX, shiftY)`, with its template under blur `blur`. */
function pilotScore(image, pose, tile, blur, shiftX, shiftY) {
  const pilot = TILE_PILOT[tile];
  if (pilot < 0) {
    return -Number.MAX_VALUE;
  }
  const symbol = KANA_PILOT_SYMBOLS[pilot];
  sampleTile(image, pose, TILE_X[tile], TILE_Y[tile], shiftX, shiftY, FINE_OFFSETS, FINE_SAMPLES);
  const t = tables().blurs[blur];
  return correlate(FINE_SAMPLES, tileEnergy, symbol[1] ? t.light[symbol[0]] : t.dark[symbol[0]]);
}

/**
 * Coarse search of pilot tile `tile`: shifts on a `COARSE_STEP` grid within its `pilotReach`
 * (`dy` outer, `dx` inner, both ascending), sampled on the coarse grid under the coarse template;
 * leaves the best shift in `bestX`, `bestY` and its correlation in `bestScore`.
 */
function coarsePilot(image, pose, tile) {
  const symbol = KANA_PILOT_SYMBOLS[TILE_PILOT[tile]];
  const coarse = tables().coarse;
  const template = symbol[1] ? coarse.light[symbol[0]] : coarse.dark[symbol[0]];
  const steps = Math.floor(pilotReach(TILE_Y[tile] + HALF_TILE) / COARSE_STEP);
  let bx = 0;
  let by = 0;
  let best = -Number.MAX_VALUE;
  for (let dy = -steps; dy <= steps; dy += 1) {
    for (let dx = -steps; dx <= steps; dx += 1) {
      const sx = COARSE_STEP * dx;
      const sy = COARSE_STEP * dy;
      sampleTile(image, pose, TILE_X[tile], TILE_Y[tile], sx, sy, COARSE_OFFSETS, COARSE_SAMPLES);
      const score = correlate(COARSE_SAMPLES, tileEnergy, template);
      if (score > best) {
        bx = sx;
        by = sy;
        best = score;
      }
    }
  }
  bestX = bx;
  bestY = by;
  bestScore = best;
}

/**
 * Refines a pilot's shift under blur `blur`: the best of the shifts one step around it, for steps of
 * `steps` in turn (ties keep the earlier); leaves the result in `bestX`, `bestY` and `bestScore`.
 */
function refinePilot(image, pose, tile, blur, startX, startY, steps) {
  let bx = startX;
  let by = startY;
  let best = pilotScore(image, pose, tile, blur, startX, startY);
  for (const step of steps) {
    const centreX = bx;
    const centreY = by;
    for (let dy = -1; dy <= 1; dy += 1) {
      for (let dx = -1; dx <= 1; dx += 1) {
        if (dx === 0 && dy === 0) {
          continue;
        }
        const sx = centreX + step * dx;
        const sy = centreY + step * dy;
        const score = pilotScore(image, pose, tile, blur, sx, sy);
        if (score > best) {
          bx = sx;
          by = sy;
          best = score;
        }
      }
    }
  }
  bestX = bx;
  bestY = by;
  bestScore = best;
}

/**
 * Solves the 3 x 3 system `m x = r` by Gauss-Jordan elimination with partial pivoting (of equal
 * pivots the last wins, as Rust's `max_by` picks); `null` when a pivot is below `1e-9`.
 */
function solve3(normal, right) {
  const m = normal.map((row) => row.slice());
  const r = right.slice();
  for (let col = 0; col < 3; col += 1) {
    let pivot = col;
    for (let row = col + 1; row < 3; row += 1) {
      if (totalCmp(Math.abs(m[row][col]), Math.abs(m[pivot][col])) >= 0) {
        pivot = row;
      }
    }
    if (Math.abs(m[pivot][col]) < 1e-9) {
      return null;
    }
    const swapRow = m[col];
    m[col] = m[pivot];
    m[pivot] = swapRow;
    const swapValue = r[col];
    r[col] = r[pivot];
    r[pivot] = swapValue;
    for (let row = 0; row < 3; row += 1) {
      if (row === col) {
        continue;
      }
      const factor = m[row][col] / m[col][col];
      const pivotRow = m[col];
      for (let k = col; k < 3; k += 1) {
        m[row][k] -= factor * pivotRow[k];
      }
      r[row] -= factor * r[col];
    }
  }
  return [r[0] / m[0][0], r[1] / m[1][1], r[2] / m[2][2]];
}

/**
 * Fits the band's shift field to the `count` found pilots: per axis an affine function of the tile
 * centre (`x` scaled by 512 around the square's middle, `y` by 145 around the pilots' mean) by least
 * squares, or their mean shift when fewer than three pilots fit one.
 */
function fitShiftField(count) {
  let sumY = -0;
  for (let p = 0; p < count; p += 1) sumY += FOUND_CENTRE_Y[p];
  fieldCentre = sumY / count;
  let meanX = -0;
  for (let p = 0; p < count; p += 1) meanX += FOUND_SHIFT_X[p];
  let meanY = -0;
  for (let p = 0; p < count; p += 1) meanY += FOUND_SHIFT_Y[p];
  FIELD_X[0] = meanX / count;
  FIELD_X[1] = 0;
  FIELD_X[2] = 0;
  FIELD_Y[0] = meanY / count;
  FIELD_Y[1] = 0;
  FIELD_Y[2] = 0;
  if (count < 3) {
    return;
  }
  const normal = [
    [0, 0, 0],
    [0, 0, 0],
    [0, 0, 0],
  ];
  const rightX = [0, 0, 0];
  const rightY = [0, 0, 0];
  const basis = [1, 0, 0];
  for (let p = 0; p < count; p += 1) {
    basis[1] = (FOUND_CENTRE_X[p] - CANVAS / 2) / 512;
    basis[2] = (FOUND_CENTRE_Y[p] - fieldCentre) / 145;
    for (let i = 0; i < 3; i += 1) {
      for (let j = 0; j < 3; j += 1) {
        normal[i][j] += basis[i] * basis[j];
      }
      rightX[i] += basis[i] * FOUND_SHIFT_X[p];
      rightY[i] += basis[i] * FOUND_SHIFT_Y[p];
    }
  }
  const x = solve3(normal, rightX);
  const y = solve3(normal, rightY);
  if (x === null || y === null) {
    return;
  }
  FIELD_X.set(x);
  FIELD_Y.set(y);
}

/** The shift of the band's field at the tile centre `(x, y)`, into `SHIFT`. */
function shiftAt(x, y) {
  const b1 = (x - CANVAS / 2) / 512;
  const b2 = (y - fieldCentre) / 145;
  SHIFT[0] = FIELD_X[0] * 1 + FIELD_X[1] * b1 + FIELD_X[2] * b2;
  SHIFT[1] = FIELD_Y[0] * 1 + FIELD_Y[1] * b1 + FIELD_Y[2] * b2;
}

/**
 * A level that varies linearly across the band, fitted by least squares to the levels of the
 * light (`light`) or dark found pilots against `(x - 512) / 512`; a constant when the pilots do not
 * spread. Leaves `[intercept, slope]` in `LINE`; `false` without such pilots.
 */
function levelLine(count, light) {
  let n = 0;
  for (let p = 0; p < count; p += 1) {
    if ((FOUND_LIGHT[p] === 1) !== light) continue;
    LINE_X[n] = (FOUND_CENTRE_X[p] - CANVAS / 2) / 512;
    LINE_Y[n] = FOUND_LEVEL[p];
    n += 1;
  }
  if (n === 0) {
    return false;
  }
  let sx = -0;
  for (let p = 0; p < n; p += 1) sx += LINE_X[p];
  const mx = sx / n;
  let sy = -0;
  for (let p = 0; p < n; p += 1) sy += LINE_Y[p];
  const my = sy / n;
  let sxx = -0;
  for (let p = 0; p < n; p += 1) sxx += (LINE_X[p] - mx) * (LINE_X[p] - mx);
  if (sxx < 1e-6) {
    LINE[0] = my;
    LINE[1] = 0;
    return true;
  }
  let sxy = -0;
  for (let p = 0; p < n; p += 1) sxy += (LINE_X[p] - mx) * (LINE_Y[p] - my);
  const slope = sxy / sxx;
  LINE[0] = my - slope * mx;
  LINE[1] = slope;
  return true;
}

/** Rust `f64::midpoint`: `(a + b) / 2` without overflow. */
function midpoint(a, b) {
  const low = 2 * 2.2250738585072014e-308;
  const high = Number.MAX_VALUE / 2;
  const absA = Math.abs(a);
  const absB = Math.abs(b);
  if (absA <= high && absB <= high) {
    return (a + b) / 2;
  }
  if (absA < low) {
    return a + b / 2;
  }
  if (absB < low) {
    return a / 2 + b;
  }
  return a / 2 + b / 2;
}

/**
 * Reads band `band`'s data tiles into the reader's tile buffers (`TILE_READ` ... `TILE_LEAD`, the
 * band's blur index in `bandBlur` and its pilots in `bandPilots`); `false` when fewer than two
 * pilots are found or the pilots show no contrast between light and dark tiles.
 */
function observeInto(image, pose, band) {
  // coarse search of every pilot in view
  const pilotTiles = KANA_PILOT_TILE_LIST[band];
  let coarseCount = 0;
  for (let p = 0; p < KANA_PILOTS; p += 1) {
    const tile = pilotTiles[p];
    if (!tileInImage(image, pose, tile)) continue;
    coarsePilot(image, pose, tile);
    if (bestScore >= PILOT_COARSE_MIN) {
      COARSE_TILE[coarseCount] = tile;
      COARSE_X[coarseCount] = bestX;
      COARSE_Y[coarseCount] = bestY;
      coarseCount += 1;
    }
  }
  if (coarseCount < 2) {
    return false;
  }
  // the blur under which the pilots, each at its best shift within a unit, correlate best
  let blur = 0;
  let best = -Number.MAX_VALUE;
  for (let index = 0; index < KANA_BLURS.length; index += 1) {
    let sum = -0;
    for (let c = 0; c < coarseCount; c += 1) {
      refinePilot(image, pose, COARSE_TILE[c], index, COARSE_X[c], COARSE_Y[c], REFINE_COARSE);
      sum += bestScore;
    }
    if (sum > best) {
      best = sum;
      blur = index;
    }
  }
  let found = 0;
  for (let c = 0; c < coarseCount; c += 1) {
    const tile = COARSE_TILE[c];
    refinePilot(image, pose, tile, blur, COARSE_X[c], COARSE_Y[c], REFINE_FINE);
    if (bestScore < PILOT_MIN) continue;
    const shiftX = bestX;
    const shiftY = bestY;
    sampleTile(image, pose, TILE_X[tile], TILE_Y[tile], shiftX, shiftY, FINE_OFFSETS, FINE_SAMPLES);
    FOUND_CENTRE_X[found] = TILE_X[tile] + HALF_TILE;
    FOUND_CENTRE_Y[found] = TILE_Y[tile] + HALF_TILE;
    FOUND_SHIFT_X[found] = shiftX;
    FOUND_SHIFT_Y[found] = shiftY;
    FOUND_LEVEL[found] = tileMean;
    FOUND_LIGHT[found] = KANA_PILOT_SYMBOLS[TILE_PILOT[tile]][1] ? 1 : 0;
    found += 1;
  }
  if (found < 2) {
    return false;
  }
  fitShiftField(found);
  if (!levelLine(found, true)) {
    return false;
  }
  const lightIntercept = LINE[0];
  const lightSlope = LINE[1];
  if (!levelLine(found, false)) {
    return false;
  }
  const darkIntercept = LINE[0];
  const darkSlope = LINE[1];
  const t = tables().blurs[blur];
  const tiles = KANA_DATA_TILE_LIST[band];
  for (let i = 0; i < KANA_DATA_TILES; i += 1) {
    const tile = tiles[i];
    TILE_READ[i] = 0;
    if (!tileInImage(image, pose, tile)) continue;
    const centreX = TILE_X[tile] + HALF_TILE;
    const centreY = TILE_Y[tile] + HALF_TILE;
    shiftAt(centreX, centreY);
    sampleTile(image, pose, TILE_X[tile], TILE_Y[tile], SHIFT[0], SHIFT[1], FINE_OFFSETS, FINE_SAMPLES);
    const mean = tileMean;
    const energy = tileEnergy;
    const x = (centreX - CANVAS / 2) / 512;
    const high = lightIntercept + lightSlope * x;
    const low = darkIntercept + darkSlope * x;
    const half = (high - low) / 2;
    if (half <= 1) continue;
    const threshold = midpoint(high, low);
    // a light template is the inside less the ink, so one dot product with the inside and one
    // with each glyph's ink give both polarities' correlations
    SCORES.fill(0);
    if (energy > 1e-9) {
      const root = Math.sqrt(energy);
      const inside = dot(FINE_SAMPLES, t.inside);
      for (let glyph = 0; glyph < GLYPH_COUNT; glyph += 1) {
        const ink = dot(FINE_SAMPLES, t.ink[glyph]);
        const light = (inside - ink) / t.lightNorm[glyph] / root;
        const dark = ink / t.darkNorm[glyph] / root;
        SCORES[glyph] = fmax(light, dark);
      }
    }
    let first = 0;
    for (let glyph = 1; glyph < GLYPH_COUNT; glyph += 1) {
      if (SCORES[glyph] > SCORES[first]) first = glyph;
    }
    let second = -Number.MAX_VALUE;
    for (let glyph = 0; glyph < GLYPH_COUNT; glyph += 1) {
      if (glyph !== first) second = fmax(second, SCORES[glyph]);
    }
    TILE_READ[i] = 1;
    TILE_LIGHT[i] = mean > threshold ? 1 : 0;
    TILE_MARGIN[i] = Math.abs(mean - threshold) / half;
    TILE_GLYPH[i] = first;
    TILE_LEAD[i] = SCORES[first] - second;
  }
  bandBlur = blur;
  bandPilots = found;
  return true;
}

/** The erasure schedule of a kana lane: 0, 1/8, 1/4, 1/3 and 1/2 of its parity (consecutive repeats dropped). */
function scheduleOf(nsym) {
  return [0, nsym >> 3, nsym >> 2, Math.floor(nsym / 3), nsym >> 1].filter(
    (erasures, index, steps) => index === 0 || steps[index - 1] !== erasures,
  );
}

/**
 * Tries Reed-Solomon with 0, n/8, n/4, n/3 and n/2 of the least confident bytes erased (n the
 * parity bytes; ties keep byte order) and accepts the first result whose check matches the square's
 * header, the frame after it or the frame before it.
 */
function decodeKanaWord(spec, header, word, confidence) {
  const order = [];
  for (let index = 0; index < word.length; index += 1) order.push(index);
  // a stable sort, like the reference's
  order.sort((a, b) => totalCmp(confidence[a], confidence[b]));
  const frames = [header.frame, (header.frame + 1) & 0xffff, (header.frame - 1) & 0xffff];
  for (const erasures of scheduleOf(spec.parityLen)) {
    const positions = order.slice(0, erasures);
    const trial = Uint8Array.from(word);
    for (const position of positions) trial[position] = 0;
    const result = decodeStatus(spec, trial, positions);
    if (typeof result === "number") {
      continue;
    }
    for (const frame of frames) {
      if (checks(header.tag, frame, spec, result.data)) {
        return Object.freeze({
          data: result.data,
          corrected: result.corrected,
          erasures: positions.length,
          header: Object.freeze({ tag: header.tag, frame }),
        });
      }
    }
  }
  return null;
}

function isByteArray(value) {
  return value instanceof Uint8Array || value instanceof Uint8ClampedArray;
}

/**
 * Validates a luma plane and a frame with a pose; returns whether the plane can be sampled (a plane
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
  if (frame === null || typeof frame !== "object" || !(frame.pose instanceof PetalPose)) {
    throw new TypeError("frame must be a decoded frame ({pose: PetalPose})");
  }
  return image.width > 0 && image.height > 0 && image.data.length === image.width * image.height;
}

const NO_SKIP = Object.freeze([false, false]);
const POLARITY = new Uint8Array(KANA_POLARITY_WORD);
const POLARITY_CONFIDENCE = new Float64Array(KANA_POLARITY_WORD);
const GLYPHS = new Uint8Array(KANA_GLYPH_WORD);
const GLYPH_CONFIDENCE = new Float64Array(KANA_GLYPH_WORD);

/**
 * Reads the kana lanes of the bands not flagged in `skip` (one flag per band, band 0 first) of a
 * capture whose square `frame` decoded under `header` (the stream tag and frame its lanes name).
 *
 * Every pilot in view is found by a coarse search around where the square's pose puts it, the
 * pilots pick the band's blur, an affine shift field fitted to their offsets moves every tile and
 * their levels set the threshold between light and dark tiles; each data tile then takes the
 * polarity its level shows and the glyph whose template correlates best. A lane passes when its
 * Reed-Solomon codeword decodes, with 0, 1/8, 1/4, 1/3 or 1/2 of its parity bytes' worth of its
 * least confident bytes erased (a byte's confidence is the least of its tiles'), into data whose
 * {@link kanaCheck} matches `header`'s frame, the frame after it or the frame before it.
 *
 * @param {{width: number, height: number, data: Uint8Array | Uint8ClampedArray}} image the luma
 *   plane `frame` was decoded from
 * @param {{pose: PetalPose}} frame usually a `PetalDecodedFrame`
 * @param {{tag: number, frame: number}} header the stream tag and the frame the square's lanes name
 * @param {readonly boolean[]} [skip] one flag per band
 * @returns {Array<{data: Uint8Array, corrected: number, erasures: number,
 *   header: {tag: number, frame: number}} | null>} `P0`, `K0`, `P1`, `K1`; `header` is the one the
 *   lane's check matched
 */
export function readKana(image, frame, header, skip = NO_SKIP) {
  const usable = readable(image, frame);
  requireHeader(header);
  if (!Array.isArray(skip) || skip.length !== KANA_BANDS) {
    throw new TypeError("skip must hold one flag per band");
  }
  const results = [null, null, null, null];
  if (!usable) {
    return results;
  }
  const tag = header.tag;
  const shown = Object.freeze({ tag, frame: header.frame });
  for (let band = 0; band < KANA_BANDS; band += 1) {
    if (skip[band]) continue;
    if (!observeInto(image, frame.pose, band)) continue;
    POLARITY.fill(0);
    POLARITY_CONFIDENCE.fill(Number.MAX_VALUE);
    GLYPHS.fill(0);
    GLYPH_CONFIDENCE.fill(Number.MAX_VALUE);
    for (let i = 0; i < KANA_DATA_TILES; i += 1) {
      const read = TILE_READ[i] === 1;
      const light = read && TILE_LIGHT[i] === 1;
      const margin = read ? TILE_MARGIN[i] : 0;
      const glyph = read ? TILE_GLYPH[i] : 0;
      const lead = read ? TILE_LEAD[i] : 0;
      if (i < KANA_POLARITY_WORD * 8) {
        if (light) POLARITY[i >> 3] |= 1 << (7 - (i & 7));
        POLARITY_CONFIDENCE[i >> 3] = fmin(POLARITY_CONFIDENCE[i >> 3], margin);
      }
      GLYPHS[i >> 1] |= (i & 1) === 0 ? glyph << 4 : glyph;
      GLYPH_CONFIDENCE[i >> 1] = fmin(GLYPH_CONFIDENCE[i >> 1], lead);
    }
    results[2 * band] = decodeKanaWord(KANA_SPECS[2 * band], shown, POLARITY, POLARITY_CONFIDENCE);
    results[2 * band + 1] = decodeKanaWord(KANA_SPECS[2 * band + 1], shown, GLYPHS, GLYPH_CONFIDENCE);
  }
  return results;
}

/**
 * What {@link readKana} sees of band `band` (0 above the square, 1 below it) before any
 * Reed-Solomon correction, for diagnostics: the blur (sigma in canvas units) the pilots chose, the
 * pilots found, and every data tile of the band in data order, `null` for a tile outside the image
 * or where the pilots show no contrast, otherwise its tile index, decided polarity and glyph, how
 * far its level lay from the threshold in half-contrasts (`lightMargin`) and how far the glyph's
 * correlation led the runner-up's (`glyphLead`). `null` when fewer than two pilots are found or
 * the pilots show no contrast between light and dark tiles.
 *
 * @returns {{blur: number, pilots: number, tiles: Array<{tile: number, light: boolean,
 *   lightMargin: number, glyph: number, glyphLead: number} | null>} | null}
 */
export function observeBand(image, frame, band) {
  const usable = readable(image, frame);
  requireIndex(band, "band");
  if (band >= KANA_BANDS) {
    throw new RangeError("band out of range");
  }
  if (!usable || !observeInto(image, frame.pose, band)) {
    return null;
  }
  const dataTiles = KANA_DATA_TILE_LIST[band];
  const tiles = [];
  for (let i = 0; i < KANA_DATA_TILES; i += 1) {
    tiles.push(
      TILE_READ[i] === 1
        ? Object.freeze({
            tile: dataTiles[i],
            light: TILE_LIGHT[i] === 1,
            lightMargin: TILE_MARGIN[i],
            glyph: TILE_GLYPH[i],
            glyphLead: TILE_LEAD[i],
          })
        : null,
    );
  }
  return Object.freeze({ blur: KANA_BLURS[bandBlur], pilots: bandPilots, tiles: Object.freeze(tiles) });
}
