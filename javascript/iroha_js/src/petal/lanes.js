// Lane codecs: bytes <-> transmitted cell states.
//
// A frame carries five lanes, and four turn lanes on top of the field lanes
// (see `turns.js`). Each lane is exactly one Reed-Solomon codeword,
// XOR-whitened with a fixed pseudo-random sequence so the picture is
// statistically balanced whatever the payload is:
//
// | lane      | cells                                  | codeword | data | parity |
// |-----------|----------------------------------------|----------|------|--------|
// | D         | 240 ring slots x 1 bit                 | 30 B     | 19 B | 11 B   |
// | F0 ... F3 | 840 field cells of one sector x 1 bit  | 105 B    | 83 B | 22 B   |
// | T0 ... T3 | 360 lit petals of one sector x 2 bits  | 90 B     | 68 B | 22 B   |
//
// The katakana tiles of the bands above and below the square carry four more
// lanes (`P0`, `K0`, `P1`, `K1`, see `kana.js`). Bit order is
// most-significant-bit first.

import { GLYPH_COUNT } from "./glyphs.js";
import {
  KANA_GLYPH_WORD,
  KANA_LANES,
  KANA_POLARITY_WORD,
  kanaGlyphWordOf,
  kanaPilotSymbolsInto,
  kanaPolarityWordOf,
  setKanaSymbolsInto,
} from "./kana.js";
import {
  D_BITS,
  DATA_SLOT,
  FIELD_CELLS,
  KANA_BANDS,
  KANA_TILE_COUNT,
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
import {
  TURN_CARRIERS,
  TURN_LABELS,
  TURN_POSITIONS,
  TURN_WORD,
  carrierOrderTable,
  carrierPositionOf,
  carriersInto,
} from "./turns.js";

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

function turnArray(value) {
  const out = new Uint8Array(FIELD_CELLS);
  if (value === undefined) {
    return out;
  }
  if (value === null || typeof value !== "object" || typeof value.length !== "number") {
    throw new TypeError(`turns must be an array of ${FIELD_CELLS} turn positions`);
  }
  if (value.length !== FIELD_CELLS) {
    throw new RangeError(`turns must hold exactly ${FIELD_CELLS} turn positions`);
  }
  for (let index = 0; index < FIELD_CELLS; index += 1) {
    const turn = value[index];
    if (!Number.isInteger(turn) || turn < 0 || turn >= TURN_POSITIONS) {
      throw new RangeError("a turn position is an integer in 0..=3");
    }
    out[index] = turn;
  }
  return out;
}

function glyphArray(value) {
  const out = new Uint8Array(KANA_TILE_COUNT);
  if (value === null || typeof value !== "object" || typeof value.length !== "number") {
    throw new TypeError(`kanaGlyphs must be an array of ${KANA_TILE_COUNT} glyphs`);
  }
  if (value.length !== KANA_TILE_COUNT) {
    throw new RangeError(`kanaGlyphs must hold exactly ${KANA_TILE_COUNT} glyphs`);
  }
  for (let index = 0; index < KANA_TILE_COUNT; index += 1) {
    const glyph = value[index];
    if (!Number.isInteger(glyph) || glyph < 0 || glyph >= GLYPH_COUNT) {
      throw new RangeError("a glyph is an integer in 0..=15");
    }
    out[index] = glyph;
  }
  return out;
}

function requireBand(band) {
  if (!Number.isInteger(band) || band < 0 || band >= KANA_BANDS) {
    throw new RangeError("band out of range");
  }
  return band;
}

/**
 * Every cell of one frame: what a renderer draws and a decoder samples: which ring slots and
 * petals are lit, how every petal is turned (see `turns.js`), and what every katakana tile of the
 * card's bands shows (see `kana.js`).
 */
export class PetalFrameCells {
  /**
   * @param {ArrayLike<number | boolean>} [dots] lit state of each of the 296 ring slots, gate dots included
   * @param {ArrayLike<number | boolean>} [petals] lit state of each of the 3,376 field cells
   * @param {ArrayLike<number>} [turns] turn position (0-3) of each of the 3,376 field cells
   * @param {ArrayLike<number>} [kanaGlyphs] glyph (0-15) of each of the 612 katakana tiles
   * @param {ArrayLike<number | boolean>} [kanaLight] polarity (light or dark) of each of the 612
   *   katakana tiles; omitted symbols are the pilots' fixed ones and light tiles of glyph 0
   */
  constructor(dots, petals, turns, kanaGlyphs, kanaLight) {
    /** Lit state (`0`/`1`) of every ring slot, gate dots included. */
    this.dots = cellArray(dots, TOTAL_SLOTS, "dots");
    /** Lit state (`0`/`1`) of every field cell (spare cells are dark). */
    this.petals = cellArray(petals, FIELD_CELLS, "petals");
    /**
     * Turn position (0-3, see `turnAngle`) of every field cell: zero for dark petals and for lit
     * petals that carry no turn bits.
     */
    this.turns = turnArray(turns);
    const glyphs = new Uint8Array(KANA_TILE_COUNT);
    const light = new Uint8Array(KANA_TILE_COUNT);
    kanaPilotSymbolsInto(glyphs, light);
    /** The glyph (0-15, see `glyphs.js`) every katakana tile shows, in `kanaTiles` order. */
    this.kanaGlyphs = kanaGlyphs === undefined ? glyphs : glyphArray(kanaGlyphs);
    /** Whether every katakana tile is light (`1`: filled, glyph in ink) or dark (`0`: glyph in pink). */
    this.kanaLight = kanaLight === undefined ? light : cellArray(kanaLight, KANA_TILE_COUNT, "kanaLight");
  }

  /**
   * Builds the cells from the transmitted codewords of lane D and the four
   * field lanes (sector 0 first), with every petal unturned (see
   * {@link PetalFrameCells#setTurns}) and only the pilots set among the
   * katakana tiles (see {@link PetalFrameCells#setKana}).
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

  /**
   * Turns the carriers of every sector to show its transmitted turn codeword
   * (see `turns.js`; sector 0 first); a sector with too few lit petals stays
   * unturned.
   */
  setTurns(words) {
    if (!Array.isArray(words) || words.length !== SECTORS) {
      throw new TypeError("words must hold the four turn lane words");
    }
    const turnWords = words.map((word, sector) => toBytes(word, `turn lane T${sector} word`));
    if (turnWords.some((word) => word.length !== TURN_WORD)) {
      throw new RangeError("turn lane word length mismatch");
    }
    const chosen = new Uint16Array(TURN_CARRIERS);
    for (let sector = 0; sector < SECTORS; sector += 1) {
      for (const cell of carrierOrderTable()[sector]) {
        this.turns[cell] = 0;
      }
      if (!carriersInto(this.petals, sector, chosen, 0)) {
        continue;
      }
      const word = turnWords[sector];
      for (let index = 0; index < chosen.length; index += 1) {
        this.turns[chosen[index]] = carrierPositionOf(word, index);
      }
    }
  }

  /**
   * Packs the turns of the carriers of sector `sector` into its turn codeword,
   * or `null` when the sector has too few lit petals to carry one.
   *
   * @returns {Uint8Array | null}
   */
  turnWord(sector) {
    if (SECTOR_CELL_LIST[sector] === undefined) {
      throw new RangeError("sector out of range");
    }
    const chosen = new Uint16Array(TURN_CARRIERS);
    if (!carriersInto(this.petals, sector, chosen, 0)) {
      return null;
    }
    const word = new Uint8Array(TURN_WORD);
    for (let index = 0; index < chosen.length; index += 1) {
      word[index >> 2] |= TURN_LABELS[this.turns[chosen[index]]] << (6 - 2 * (index & 3));
    }
    return word;
  }

  /**
   * Shows the transmitted kana codewords `words` (`P0`, `K0`, `P1`, `K1`: 37, 149, 37 and 149
   * bytes) on the data tiles of both bands: data tile `i` of a band shows bit `i` of its polarity
   * codeword (the last two data tiles, beyond it, are light) and nibble `i` of its glyph codeword.
   * The pilots keep their fixed symbols.
   */
  setKana(words) {
    if (!Array.isArray(words) || words.length !== KANA_LANES.length) {
      throw new TypeError("words must hold the four kana lane words (P0, K0, P1, K1)");
    }
    const kanaWords = words.map((word, index) => toBytes(word, `kana lane ${KANA_LANES[index]} word`));
    if (kanaWords.some((word, index) => word.length !== ((index & 1) === 1 ? KANA_GLYPH_WORD : KANA_POLARITY_WORD))) {
      throw new RangeError("kana lane word length mismatch");
    }
    for (let band = 0; band < KANA_BANDS; band += 1) {
      setKanaSymbolsInto(this.kanaGlyphs, this.kanaLight, band, kanaWords[2 * band], kanaWords[2 * band + 1]);
    }
  }

  /** Packs the polarity codeword the data tiles of band `band` (0 above the square, 1 below it) show. */
  kanaPolarityWord(band) {
    return kanaPolarityWordOf(this.kanaLight, requireBand(band));
  }

  /** Packs the glyph codeword the data tiles of band `band` (0 above the square, 1 below it) show. */
  kanaGlyphWord(band) {
    return kanaGlyphWordOf(this.kanaGlyphs, requireBand(band));
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

  /** Whether `other` holds exactly the same cells, turns and katakana tiles included. */
  equals(other) {
    if (!(other instanceof PetalFrameCells)) return false;
    for (let index = 0; index < TOTAL_SLOTS; index += 1) {
      if (this.dots[index] !== other.dots[index]) return false;
    }
    for (let index = 0; index < FIELD_CELLS; index += 1) {
      if (this.petals[index] !== other.petals[index] || this.turns[index] !== other.turns[index]) return false;
    }
    for (let index = 0; index < KANA_TILE_COUNT; index += 1) {
      if (this.kanaGlyphs[index] !== other.kanaGlyphs[index] || this.kanaLight[index] !== other.kanaLight[index]) {
        return false;
      }
    }
    return true;
  }
}
