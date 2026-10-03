// Rateless fountain code over GF(2).
//
// A payload is cut into `k` source atoms of 16 bytes (the last one
// zero-padded). Encoded atom `id` is source atom `id` for `id < k`
// (systematic), and otherwise the XOR of a pseudo-random half of the source
// atoms chosen by `maskWords`. A receiver that holds any `k + 2` or so
// independent atoms, in any order, recovers the payload by Gaussian
// elimination; lost frames cost nothing but time.

import { ATOM_LEN } from "./lanes.js";
import { requireIndex, toBytes } from "./support.js";

/** Splits a payload into zero-padded 16-byte source atoms. */
export function splitPayload(payload) {
  const bytes = toBytes(payload, "payload");
  const atoms = [];
  for (let start = 0; start < bytes.length; start += ATOM_LEN) {
    const atom = new Uint8Array(ATOM_LEN);
    atom.set(bytes.subarray(start, Math.min(start + ATOM_LEN, bytes.length)));
    atoms.push(atom);
  }
  return atoms;
}

/** Number of 32-bit words needed for a mask over `k` source atoms. */
export function maskLen(k) {
  return Math.ceil(k / 32);
}

/**
 * The 32-bit finalizer of MurmurHash3 (`fmix32`).
 *
 * Masks must not come from a GF(2)-linear generator such as xorshift: every
 * mask would then lie in a subspace of dimension at most 32. The
 * multiplications make this mixer nonlinear over GF(2).
 */
export function mix32(value) {
  let x = value >>> 0;
  x ^= x >>> 16;
  x = Math.imul(x, 0x85ebca6b);
  x ^= x >>> 13;
  x = Math.imul(x, 0xc2b2ae35);
  x ^= x >>> 16;
  return x >>> 0;
}

function requireU32(value, name) {
  if (!Number.isInteger(value) || value < 0 || value > 0xffffffff) {
    throw new TypeError(`${name} must be an unsigned 32-bit integer`);
  }
  return value;
}

function countTrailingZeros(word) {
  return 31 - Math.clz32(word & -word);
}

/**
 * The combination mask of encoded atom `id`, as little-endian bit words.
 *
 * `crc` is the payload CRC-32C and only diversifies masks between streams;
 * `k` must be at least one. Atoms with `id < k` are systematic (a unit
 * vector); every other atom combines a pseudo-random half of the sources:
 *
 *     seed    = mix32((id * 0x9E3779B1) ^ crc ^ 0xA5A5A5A5)
 *     word[w] = mix32(seed + (w + 1) * 0x9E3779B9)      (all arithmetic mod 2^32)
 *
 * Bits at or above `k` are cleared, and an all-zero mask is replaced by the
 * single bit `id mod k`.
 *
 * @returns {Uint32Array}
 */
export function maskWords(k, crc, id) {
  requireIndex(k, "source atom count");
  if (k === 0) {
    throw new RangeError("a stream has at least one source atom");
  }
  return maskWordsUnchecked(k, requireU32(crc, "crc"), requireU32(id, "atom id"));
}

function maskWordsUnchecked(k, crc, id) {
  const mask = new Uint32Array(maskLen(k));
  if (id < k) {
    mask[Math.floor(id / 32)] = 2 ** (id % 32);
    return mask;
  }
  const seed = mix32(Math.imul(id, 0x9e3779b1) ^ crc ^ 0xa5a5a5a5);
  for (let w = 0; w < mask.length; w += 1) {
    mask[w] = mix32(seed + Math.imul(w + 1, 0x9e3779b9));
  }
  const tail = k % 32;
  if (tail !== 0) {
    mask[mask.length - 1] &= 2 ** tail - 1;
  }
  let zero = true;
  for (let w = 0; w < mask.length; w += 1) {
    if (mask[w] !== 0) {
      zero = false;
      break;
    }
  }
  if (zero) {
    const bit = id % k;
    mask[Math.floor(bit / 32)] |= 2 ** (bit % 32);
  }
  return mask;
}

/** Encodes atom `id` from the 16-byte source atoms. */
export function encodeAtom(source, crc, id) {
  if (!Array.isArray(source) || source.length === 0) {
    throw new TypeError("source must be a non-empty array of 16-byte atoms");
  }
  const mask = maskWordsUnchecked(source.length, requireU32(crc, "crc"), requireU32(id, "atom id"));
  const out = new Uint8Array(ATOM_LEN);
  for (let index = 0; index < source.length; index += 1) {
    if (((mask[index >>> 5] >>> (index & 31)) & 1) === 1) {
      const atom = source[index];
      for (let byte = 0; byte < ATOM_LEN; byte += 1) {
        out[byte] ^= atom[byte];
      }
    }
  }
  return out;
}

/** Incremental Gaussian-elimination decoder of the fountain code. */
export class PetalFountainDecoder {
  /** @param {number} k number of source atoms, at least one */
  constructor(k) {
    requireIndex(k, "source atom count");
    if (k === 0) {
      throw new RangeError("a stream has at least one source atom");
    }
    this._k = k;
    this._pivot = new Int32Array(k).fill(-1);
    this._rows = [];
  }

  /** Number of source atoms. */
  get sourceAtoms() {
    return this._k;
  }

  /** Number of linearly independent atoms received so far. */
  get rank() {
    return this._rows.length;
  }

  /** Whether enough independent atoms arrived to recover the payload. */
  isComplete() {
    return this._rows.length === this._k;
  }

  /** Adds encoded atom `id`; returns whether it increased the rank. */
  addEncoded(crc, id, atom) {
    const data = toBytes(atom, "atom");
    if (data.length !== ATOM_LEN) {
      throw new RangeError("an atom is exactly 16 bytes");
    }
    const mask = maskWordsUnchecked(this._k, requireU32(crc, "crc"), requireU32(id, "atom id"));
    return this._add(mask, Uint8Array.from(data));
  }

  /**
   * Adds a received combination (`mask` as little-endian 32-bit words over
   * the source atoms); returns whether it increased the rank.
   */
  add(mask, atom) {
    const data = toBytes(atom, "atom");
    if (data.length !== ATOM_LEN) {
      throw new RangeError("an atom is exactly 16 bytes");
    }
    if (mask === null || typeof mask !== "object" || typeof mask.length !== "number") {
      throw new TypeError("mask must be an array of 32-bit words");
    }
    const words = new Uint32Array(mask.length);
    for (let index = 0; index < mask.length; index += 1) {
      words[index] = requireU32(mask[index], "mask word");
    }
    return this._add(words, Uint8Array.from(data));
  }

  _add(mask, data) {
    if (mask.length !== maskLen(this._k)) {
      return false;
    }
    let word = 0;
    for (;;) {
      while (word < mask.length && mask[word] === 0) {
        word += 1;
      }
      if (word === mask.length) {
        return false;
      }
      const column = word * 32 + countTrailingZeros(mask[word]);
      if (column >= this._k) {
        return false;
      }
      const row = this._pivot[column];
      if (row >= 0) {
        const pivot = this._rows[row];
        for (let w = word; w < mask.length; w += 1) {
          mask[w] ^= pivot.mask[w];
        }
        for (let byte = 0; byte < ATOM_LEN; byte += 1) {
          data[byte] ^= pivot.data[byte];
        }
      } else {
        this._pivot[column] = this._rows.length;
        this._rows.push({ mask, data });
        return true;
      }
    }
  }

  /**
   * Returns the source atoms once the decoder is complete, otherwise `null`.
   *
   * @returns {Uint8Array[] | null}
   */
  solve() {
    if (!this.isComplete()) {
      return null;
    }
    const k = this._k;
    const solution = new Array(k);
    for (let column = k - 1; column >= 0; column -= 1) {
      const row = this._rows[this._pivot[column]];
      const value = Uint8Array.from(row.data);
      const firstWord = column >>> 5;
      for (let word = firstWord; word < row.mask.length; word += 1) {
        let bits = row.mask[word];
        if (word === firstWord) {
          // keep only columns strictly above the pivot
          const shift = (column % 32) + 1;
          bits = shift >= 32 ? 0 : ((bits >>> shift) << shift) >>> 0;
        }
        while (bits !== 0) {
          const bit = countTrailingZeros(bits);
          bits = (bits & (bits - 1)) >>> 0;
          const other = solution[word * 32 + bit];
          for (let byte = 0; byte < ATOM_LEN; byte += 1) {
            value[byte] ^= other[byte];
          }
        }
      }
      solution[column] = value;
    }
    return solution;
  }
}
