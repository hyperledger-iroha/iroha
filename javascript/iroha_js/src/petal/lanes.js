// Lane codecs: bytes <-> transmitted cell states.
//
// A frame carries five lanes. Each lane is exactly one Reed-Solomon codeword,
// XOR-whitened with a fixed pseudo-random sequence so the picture is
// statistically balanced whatever the payload is:
//
// | lane      | cells                                 | codeword | data | parity |
// |-----------|---------------------------------------|----------|------|--------|
// | D         | 240 ring slots x 1 bit                | 30 B     | 19 B | 11 B   |
// | F0 ... F3 | 840 field cells of one sector x 1 bit | 105 B    | 83 B | 22 B   |
//
// Bit order is most-significant-bit first.

import {
  D_BITS,
  DATA_SLOT,
  FIELD_CELLS,
  ROLE_GATE,
  SECTORS,
  SECTOR_CELLS,
  SECTOR_CELL_LIST,
  SLOT_ROLE,
  TOTAL_SLOTS,
} from "./layout.js";
import { PetalXorshift32 } from "./prng.js";
import { PetalReedSolomon, RS_INVALID_SHAPE, RS_UNCORRECTABLE, rsCorrect } from "./rs.js";
import { PetalError, toBytes } from "./support.js";

/** Length of an atom in bytes. */
export const ATOM_LEN = 16;
/** Bytes of the per-lane header (`tag`, `frame` high, `frame` low). */
export const LANE_HEADER_LEN = 3;
/** Atoms carried by lane D on frames that do not carry a beacon. */
export const D_ATOMS = 1;
/** Atoms carried by each field lane. */
export const FIELD_ATOMS = 5;
/** Most atoms one frame can carry (the four field lanes and lane D). */
export const ATOMS_PER_FRAME = SECTORS * FIELD_ATOMS + D_ATOMS;

/** Codeword length of lane D in bytes. */
export const D_WORD = D_BITS / 8;
/** Parity bytes of lane D. */
export const D_PARITY = 11;
/** Data bytes of lane D. */
export const D_DATA = D_WORD - D_PARITY;
/** Codeword length of a field lane in bytes. */
export const FIELD_WORD = 105;
/** Parity bytes of a field lane. */
export const FIELD_PARITY = 22;
/** Data bytes of a field lane. */
export const FIELD_DATA = FIELD_WORD - FIELD_PARITY;
/** Field cells of a sector that carry no bit (they stay dark). */
export const SPARE_SECTOR_CELLS = SECTOR_CELLS - FIELD_WORD * 8;
/** Field cells of a sector that carry the bits of its codeword. */
export const FIELD_BITS = FIELD_WORD * 8;

/** The four field lanes, sector 0 first. */
export const FIELD_LANES = Object.freeze(["F0", "F1", "F2", "F3"]);
/** All lanes in decode order: lane D, then the field lanes. */
export const LANE_ORDER = Object.freeze(["D", ...FIELD_LANES]);

function makeSpec(name, sector, wordLen, parityLen, atoms, whiteningSeed) {
  const rs = new PetalReedSolomon(parityLen);
  const rng = new PetalXorshift32(whiteningSeed);
  const whitening = new Uint8Array(wordLen);
  for (let index = 0; index < wordLen; index += 1) {
    whitening[index] = rng.nextByte();
  }
  // the letter that names the lane in reports: `D`, or the sector digit
  const letter = sector === null ? "D" : String(sector);
  return { name, letter, sector, wordLen, parityLen, dataLen: wordLen - parityLen, atoms, whiteningSeed, rs, whitening };
}

const D_SPEC = makeSpec("D", null, D_WORD, D_PARITY, D_ATOMS, 0x444f5453); // "DOTS"
// "PET0" ... "PET3"
const FIELD_SPECS = FIELD_LANES.map((name, sector) =>
  makeSpec(name, sector, FIELD_WORD, FIELD_PARITY, FIELD_ATOMS, 0x50455430 + sector),
);

if (
  D_SPEC.dataLen !== LANE_HEADER_LEN + D_ATOMS * ATOM_LEN ||
  FIELD_DATA !== LANE_HEADER_LEN + FIELD_ATOMS * ATOM_LEN ||
  FIELD_BITS > SECTOR_CELLS
) {
  throw new Error("petal lane sizes are inconsistent");
}

/** Resolves a lane name (`"D"` or `"F0"` ... `"F3"`) to its internal description. */
export function laneSpec(lane) {
  switch (lane) {
    case "D":
      return D_SPEC;
    case "F0":
      return FIELD_SPECS[0];
    case "F1":
      return FIELD_SPECS[1];
    case "F2":
      return FIELD_SPECS[2];
    case "F3":
      return FIELD_SPECS[3];
    default:
      throw new TypeError('petal lane must be "D", "F0", "F1", "F2" or "F3"');
  }
}

/** The name of the field lane of sector `sector` (`"F0"` ... `"F3"`). */
export function fieldLane(sector) {
  const lane = FIELD_LANES[sector];
  if (lane === undefined) {
    throw new RangeError("sector out of range");
  }
  return lane;
}

/** The letter that names `lane` in reports: `D`, or the sector digit `0`-`3`. */
export function laneLetter(lane) {
  return laneSpec(lane).letter;
}

/** The fixed whitening sequence of a lane. */
export function laneWhitening(lane) {
  return laneSpec(lane).whitening.slice();
}

/**
 * Encodes lane data into the transmitted (whitened) codeword.
 *
 * @param {"D" | "F0" | "F1" | "F2" | "F3"} lane
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
 * @param {"D" | "F0" | "F1" | "F2" | "F3"} lane
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

function cellArray(value, length, name) {
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
    out[index] = value[index] ? 1 : 0;
  }
  return out;
}

/** Every cell of one frame: what a renderer draws and a decoder samples. */
export class PetalFrameCells {
  /**
   * @param {ArrayLike<number | boolean>} [dots] lit state of each of the 296 ring slots, gate dots included
   * @param {ArrayLike<number | boolean>} [petals] lit state of each of the 3,376 field cells
   */
  constructor(dots, petals) {
    /** Lit state (`0`/`1`) of every ring slot, gate dots included. */
    this.dots = cellArray(dots, TOTAL_SLOTS, "dots");
    /** Lit state (`0`/`1`) of every field cell (spare cells are dark). */
    this.petals = cellArray(petals, FIELD_CELLS, "petals");
  }

  /**
   * Builds the cells from the transmitted codewords of lane D and the four
   * field lanes (sector 0 first).
   */
  static fromWords(d, fields) {
    const dWord = toBytes(d, "lane D word");
    if (!Array.isArray(fields) || fields.length !== SECTORS) {
      throw new TypeError("fields must hold the four field lane words");
    }
    const fieldWords = fields.map((word, sector) => toBytes(word, `lane F${sector} word`));
    if (dWord.length !== D_WORD || fieldWords.some((word) => word.length !== FIELD_WORD)) {
      throw new RangeError("lane word length mismatch");
    }
    const cells = new PetalFrameCells();
    for (let slot = 0; slot < TOTAL_SLOTS; slot += 1) {
      const role = SLOT_ROLE[slot];
      if (role === ROLE_GATE) {
        cells.dots[slot] = 1;
      } else if (role >= 0) {
        cells.dots[slot] = (dWord[role >> 3] >> (7 - (role & 7))) & 1;
      }
    }
    for (let sector = 0; sector < SECTORS; sector += 1) {
      const word = fieldWords[sector];
      const order = SECTOR_CELL_LIST[sector];
      for (let bit = 0; bit < FIELD_BITS; bit += 1) {
        cells.petals[order[bit]] = (word[bit >> 3] >> (7 - (bit & 7))) & 1;
      }
    }
    return cells;
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

  /** Packs the petals of sector `sector` into its field codeword. */
  fieldWord(sector) {
    const order = SECTOR_CELL_LIST[sector];
    if (order === undefined) {
      throw new RangeError("sector out of range");
    }
    const word = new Uint8Array(FIELD_WORD);
    for (let bit = 0; bit < FIELD_BITS; bit += 1) {
      if (this.petals[order[bit]]) {
        word[bit >> 3] |= 1 << (7 - (bit & 7));
      }
    }
    return word;
  }

  /** Whether `other` holds exactly the same cells. */
  equals(other) {
    if (!(other instanceof PetalFrameCells)) return false;
    for (let index = 0; index < TOTAL_SLOTS; index += 1) {
      if (this.dots[index] !== other.dots[index]) return false;
    }
    for (let index = 0; index < FIELD_CELLS; index += 1) {
      if (this.petals[index] !== other.petals[index]) return false;
    }
    return true;
  }
}
