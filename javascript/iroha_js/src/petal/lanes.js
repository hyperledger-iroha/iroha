// Lane codecs: bytes <-> transmitted cell states.
//
// A frame carries three lanes. Each lane is exactly one Reed-Solomon codeword,
// XOR-whitened with a fixed pseudo-random sequence so the picture is
// statistically balanced whatever the payload is:
//
// | lane | cells                  | codeword | data | parity |
// |------|------------------------|----------|------|--------|
// | P    | 256 tiles x 1 bit      | 32 B     | 19 B | 13 B   |
// | K    | 256 tiles x 4 bits     | 128 B    | 83 B | 45 B   |
// | D    | 240 ring slots x 1 bit | 30 B     | 19 B | 11 B   |
//
// Bit order is most-significant-bit first; lane K packs the first tile of a
// pair into the high nibble.

import { D_BITS, DATA_SLOT, ROLE_GATE, SLOT_ROLE, TILE_COUNT, TOTAL_SLOTS } from "./layout.js";
import { PetalXorshift32 } from "./prng.js";
import { PetalReedSolomon, RS_INVALID_SHAPE, RS_UNCORRECTABLE, rsCorrect } from "./rs.js";
import { PetalError, toBytes } from "./support.js";

/** Length of an atom in bytes. */
export const ATOM_LEN = 16;
/** Bytes of the per-lane header (`tag`, `frame` high, `frame` low). */
export const LANE_HEADER_LEN = 3;
/** Atoms carried by lane P. */
export const P_ATOMS = 1;
/** Atoms carried by lane D on frames that do not carry a beacon. */
export const D_ATOMS = 1;
/** Atoms carried by lane K. */
export const K_ATOMS = 5;
/** Most atoms one frame can carry (lanes P, D and K). */
export const ATOMS_PER_FRAME = P_ATOMS + D_ATOMS + K_ATOMS;

/** Codeword length of lane P in bytes. */
export const P_WORD = TILE_COUNT / 8;
/** Codeword length of lane K in bytes. */
export const K_WORD = TILE_COUNT / 2;
/** Codeword length of lane D in bytes. */
export const D_WORD = D_BITS / 8;
/** Parity bytes of lane P. */
export const P_PARITY = 13;
/** Parity bytes of lane K. */
export const K_PARITY = 45;
/** Parity bytes of lane D. */
export const D_PARITY = 11;
/** Data bytes of lane P. */
export const P_DATA = P_WORD - P_PARITY;
/** Data bytes of lane K. */
export const K_DATA = K_WORD - K_PARITY;
/** Data bytes of lane D. */
export const D_DATA = D_WORD - D_PARITY;

/** All lanes in decode order. */
export const LANE_ORDER = Object.freeze(["P", "D", "K"]);

function makeSpec(name, wordLen, parityLen, atoms, whiteningSeed) {
  const rs = new PetalReedSolomon(parityLen);
  const rng = new PetalXorshift32(whiteningSeed);
  const whitening = new Uint8Array(wordLen);
  for (let index = 0; index < wordLen; index += 1) {
    whitening[index] = rng.nextByte();
  }
  return { name, wordLen, parityLen, dataLen: wordLen - parityLen, atoms, whiteningSeed, rs, whitening };
}

const P_SPEC = makeSpec("P", P_WORD, P_PARITY, P_ATOMS, 0x50455441); // "PETA"
const K_SPEC = makeSpec("K", K_WORD, K_PARITY, K_ATOMS, 0x4b414e41); // "KANA"
const D_SPEC = makeSpec("D", D_WORD, D_PARITY, D_ATOMS, 0x444f5453); // "DOTS"

if (
  P_SPEC.dataLen !== LANE_HEADER_LEN + P_ATOMS * ATOM_LEN ||
  K_SPEC.dataLen !== LANE_HEADER_LEN + K_ATOMS * ATOM_LEN ||
  D_SPEC.dataLen !== LANE_HEADER_LEN + D_ATOMS * ATOM_LEN
) {
  throw new Error("petal lane sizes are inconsistent");
}

/** Resolves a lane name (`"P"`, `"K"` or `"D"`) to its internal description. */
export function laneSpec(lane) {
  switch (lane) {
    case "P":
      return P_SPEC;
    case "K":
      return K_SPEC;
    case "D":
      return D_SPEC;
    default:
      throw new TypeError('petal lane must be "P", "K" or "D"');
  }
}

/** The fixed whitening sequence of a lane. */
export function laneWhitening(lane) {
  return laneSpec(lane).whitening.slice();
}

/**
 * Encodes lane data into the transmitted (whitened) codeword.
 *
 * @param {"P" | "K" | "D"} lane
 * @param {Uint8Array | ArrayBuffer | ArrayBufferView | number[]} data exactly the lane's data length
 * @returns {Uint8Array}
 */
export function encodeLane(lane, data) {
  const spec = laneSpec(lane);
  const bytes = toBytes(data, "lane data");
  if (bytes.length !== spec.dataLen) {
    throw new RangeError("lane data length mismatch");
  }
  const word = spec.rs.encode(bytes);
  for (let index = 0; index < word.length; index += 1) {
    word[index] ^= spec.whitening[index];
  }
  return word;
}

/**
 * Decodes a transmitted codeword. Returns `{data, corrected}` (`corrected`
 * counts the byte positions the Reed-Solomon decoder rewrote: the erased bytes
 * plus unflagged errors) or a negative status.
 */
export function decodeLaneCountedStatus(spec, transmitted, erasures) {
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

function countedOrThrow(lane, transmitted, erasures) {
  const spec = laneSpec(lane);
  const bytes = toBytes(transmitted, "transmitted lane word");
  if (!Array.isArray(erasures)) {
    throw new TypeError("erasures must be an array of byte positions");
  }
  const result = decodeLaneCountedStatus(spec, bytes, erasures);
  if (result === RS_INVALID_SHAPE) throw new PetalError("rs_invalid_shape");
  if (result === RS_UNCORRECTABLE) throw new PetalError("rs_uncorrectable");
  return result;
}

/**
 * Decodes a transmitted codeword, returning the lane data bytes.
 *
 * @param {"P" | "K" | "D"} lane
 * @param {Uint8Array | ArrayBuffer | ArrayBufferView | number[]} transmitted
 * @param {readonly number[]} [erasures] byte positions the caller distrusts
 * @throws {PetalError} `rs_invalid_shape` or `rs_uncorrectable`
 */
export function decodeLane(lane, transmitted, erasures = []) {
  return countedOrThrow(lane, transmitted, erasures).data;
}

/**
 * Like {@link decodeLane}, also returning how many byte positions the
 * Reed-Solomon decoder rewrote (the erased bytes plus unflagged errors; `0`
 * for a word that was already valid).
 *
 * @returns {{data: Uint8Array, corrected: number}}
 * @throws {PetalError} `rs_invalid_shape` or `rs_uncorrectable`
 */
export function decodeLaneCounted(lane, transmitted, erasures = []) {
  const { data, corrected } = countedOrThrow(lane, transmitted, erasures);
  return Object.freeze({ data, corrected });
}

function cellArray(value, length, name, maximum) {
  const out = new Uint8Array(length);
  if (value === undefined) {
    return out;
  }
  if (value === null || typeof value !== "object" || typeof value.length !== "number") {
    throw new TypeError(`${name} must be an array of ${length} cells`);
  }
  if (value.length !== length) {
    throw new RangeError(`${name} must hold exactly ${length} cells`);
  }
  for (let index = 0; index < length; index += 1) {
    const cell = value[index];
    if (maximum === 1) {
      out[index] = cell ? 1 : 0;
    } else {
      if (!Number.isInteger(cell) || cell < 0 || cell > maximum) {
        throw new RangeError(`${name} cells must be integers in 0..=${maximum}`);
      }
      out[index] = cell;
    }
  }
  return out;
}

/** Every cell of one frame: what a renderer draws and a decoder samples. */
export class PetalFrameCells {
  /**
   * @param {ArrayLike<number | boolean>} [light] polarity of each of the 256 tiles; truthy is a light tile
   * @param {ArrayLike<number>} [glyph] glyph symbol (`0..16`) of each tile
   * @param {ArrayLike<number | boolean>} [dots] lit state of each of the 276 ring slots, gate dots included
   */
  constructor(light, glyph, dots) {
    /** Polarity of each tile; `1` is a light tile. */
    this.light = cellArray(light, TILE_COUNT, "light", 1);
    /** Glyph symbol (`0..16`) of each tile. */
    this.glyph = cellArray(glyph, TILE_COUNT, "glyph", 15);
    /** Lit state (`0`/`1`) of every ring slot, gate dots included. */
    this.dots = cellArray(dots, TOTAL_SLOTS, "dots", 1);
  }

  /** Builds the cells from the three transmitted codewords. */
  static fromWords(p, k, d) {
    const pWord = toBytes(p, "lane P word");
    const kWord = toBytes(k, "lane K word");
    const dWord = toBytes(d, "lane D word");
    if (pWord.length !== P_WORD || kWord.length !== K_WORD || dWord.length !== D_WORD) {
      throw new RangeError("lane word length mismatch");
    }
    const cells = new PetalFrameCells();
    for (let tile = 0; tile < TILE_COUNT; tile += 1) {
      cells.light[tile] = (pWord[tile >> 3] >> (7 - (tile & 7))) & 1;
      const byte = kWord[tile >> 1];
      cells.glyph[tile] = (tile & 1) === 0 ? byte >> 4 : byte & 0x0f;
    }
    for (let slot = 0; slot < TOTAL_SLOTS; slot += 1) {
      const role = SLOT_ROLE[slot];
      if (role === ROLE_GATE) {
        cells.dots[slot] = 1;
      } else if (role >= 0) {
        cells.dots[slot] = (dWord[role >> 3] >> (7 - (role & 7))) & 1;
      }
    }
    return cells;
  }

  /** Packs the polarity cells into a lane P codeword. */
  pWord() {
    const word = new Uint8Array(P_WORD);
    for (let tile = 0; tile < TILE_COUNT; tile += 1) {
      if (this.light[tile]) {
        word[tile >> 3] |= 1 << (7 - (tile & 7));
      }
    }
    return word;
  }

  /** Packs the glyph cells into a lane K codeword. */
  kWord() {
    const word = new Uint8Array(K_WORD);
    for (let tile = 0; tile < TILE_COUNT; tile += 1) {
      const nibble = this.glyph[tile] & 0x0f;
      word[tile >> 1] |= (tile & 1) === 0 ? nibble << 4 : nibble;
    }
    return word;
  }

  /** Packs the data dots into a lane D codeword. */
  dWord() {
    const word = new Uint8Array(D_WORD);
    for (let bit = 0; bit < D_BITS; bit += 1) {
      if (this.dots[DATA_SLOT[bit]]) {
        word[bit >> 3] |= 1 << (7 - (bit & 7));
      }
    }
    return word;
  }

  /** Whether `other` holds exactly the same cells. */
  equals(other) {
    if (!(other instanceof PetalFrameCells)) return false;
    for (let index = 0; index < TILE_COUNT; index += 1) {
      if (this.light[index] !== other.light[index] || this.glyph[index] !== other.glyph[index]) {
        return false;
      }
    }
    for (let index = 0; index < TOTAL_SLOTS; index += 1) {
      if (this.dots[index] !== other.dots[index]) return false;
    }
    return true;
  }
}
