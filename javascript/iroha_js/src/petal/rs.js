// Reed-Solomon over GF(2^8) with errors-and-erasures decoding.
//
// The field uses the primitive polynomial x^8 + x^4 + x^3 + x^2 + 1 (0x11D,
// the same field as QR Code) with alpha = 2. A codeword is `data || parity`,
// systematic, and the generator is g(x) = (x - a^0)(x - a^1)...(x - a^(nsym-1)),
// so the first consecutive root is a^0. The first byte of a codeword is the
// highest-degree coefficient.
//
// Every Petal lane is one codeword (n <= 255). Low-confidence cells are passed
// to the decoder as erasures, which cost one parity symbol each instead of two.

import { PetalError, toBytes } from "./support.js";

/** Primitive polynomial of the field. */
const PRIMITIVE = 0x11d;

const GF_EXP = new Uint8Array(512);
const GF_LOG = new Uint8Array(256);
(() => {
  let x = 1;
  for (let index = 0; index < 255; index += 1) {
    GF_EXP[index] = x;
    GF_LOG[x] = index;
    x <<= 1;
    if ((x & 0x100) !== 0) {
      x ^= PRIMITIVE;
    }
  }
  for (let index = 255; index < 512; index += 1) {
    GF_EXP[index] = GF_EXP[index - 255];
  }
})();

/** Multiplies two field elements. */
export function gfMul(a, b) {
  return a === 0 || b === 0 ? 0 : GF_EXP[GF_LOG[a] + GF_LOG[b]];
}

/** Divides `a` by a non-zero `b`. */
function gfDiv(a, b) {
  return a === 0 ? 0 : GF_EXP[GF_LOG[a] + 255 - GF_LOG[b]];
}

/** Returns alpha^exponent for a non-negative integer exponent. */
export function gfExp(exponent) {
  return GF_EXP[exponent % 255];
}

/** Returns the multiplicative inverse of a non-zero element. */
function gfInv(a) {
  return GF_EXP[255 - GF_LOG[a]];
}

/** Multiplies two polynomials stored lowest-degree first. */
function polyMul(a, b) {
  const out = new Uint8Array(a.length + b.length - 1);
  for (let i = 0; i < a.length; i += 1) {
    const x = a[i];
    if (x === 0) continue;
    for (let j = 0; j < b.length; j += 1) {
      out[i + j] ^= gfMul(x, b[j]);
    }
  }
  return out;
}

/** Evaluates a lowest-degree-first polynomial at `x` (Horner). */
function polyEval(poly, x) {
  let acc = 0;
  for (let index = poly.length - 1; index >= 0; index -= 1) {
    acc = gfMul(acc, x) ^ poly[index];
  }
  return acc;
}

/** Highest-degree-first monic generator with `nsym` consecutive roots. */
function makeGenerator(nsym) {
  let generator = Uint8Array.of(1);
  for (let i = 0; i < nsym; i += 1) {
    const root = gfExp(i);
    const next = new Uint8Array(generator.length + 1);
    for (let k = 0; k < generator.length; k += 1) {
      next[k] ^= generator[k];
      next[k + 1] ^= gfMul(generator[k], root);
    }
    generator = next;
  }
  return generator;
}

function syndromes(nsym, word) {
  const out = new Uint8Array(nsym);
  for (let j = 0; j < nsym; j += 1) {
    const root = gfExp(j);
    let acc = 0;
    for (let index = 0; index < word.length; index += 1) {
      acc = gfMul(acc, root) ^ word[index];
    }
    out[j] = acc;
  }
  return out;
}

function allZero(bytes) {
  for (let index = 0; index < bytes.length; index += 1) {
    if (bytes[index] !== 0) return false;
  }
  return true;
}

/** Berlekamp-Massey over GF(256); returns the lowest-degree-first locator. */
function berlekampMassey(syndromeList) {
  const n = syndromeList.length;
  let c = new Uint8Array(n + 1);
  let b = new Uint8Array(n + 1);
  c[0] = 1;
  b[0] = 1;
  let l = 0;
  let m = 1;
  let previousDiscrepancy = 1;
  for (let i = 0; i < n; i += 1) {
    let d = syndromeList[i];
    for (let j = 1; j <= l; j += 1) {
      d ^= gfMul(c[j], syndromeList[i - j]);
    }
    if (d === 0) {
      m += 1;
      continue;
    }
    const scale = gfDiv(d, previousDiscrepancy);
    const span = n + 1 - m;
    if (2 * l <= i) {
      const snapshot = c.slice();
      for (let j = 0; j < span; j += 1) {
        c[j + m] ^= gfMul(scale, b[j]);
      }
      l = i + 1 - l;
      b = snapshot;
      previousDiscrepancy = d;
      m = 1;
    } else {
      for (let j = 0; j < span; j += 1) {
        c[j + m] ^= gfMul(scale, b[j]);
      }
      m += 1;
    }
  }
  return c.slice(0, l + 1);
}

/** Status of {@link rsCorrect}: malformed arguments. */
export const RS_INVALID_SHAPE = -1;
/** Status of {@link rsCorrect}: more errata than the code can correct. */
export const RS_UNCORRECTABLE = -2;

/**
 * Corrects `word` in place; returns the number of corrected positions or a
 * negative status. The corrected word is re-checked against zero syndromes,
 * so a success always yields a valid codeword.
 */
export function rsCorrect(nsym, word, erasures) {
  const n = word.length;
  if (n <= nsym || n > 255 || erasures.length > nsym) {
    return RS_INVALID_SHAPE;
  }
  const seen = new Uint8Array(255);
  for (const position of erasures) {
    if (!Number.isInteger(position) || position < 0 || position >= n || seen[position] === 1) {
      return RS_INVALID_SHAPE;
    }
    seen[position] = 1;
  }
  const syndromeList = syndromes(nsym, word);
  if (allZero(syndromeList)) {
    return 0;
  }
  const f = erasures.length;
  // Erasure locator gamma(x) = prod (1 + X_e x), lowest degree first.
  let gamma = Uint8Array.of(1);
  for (const position of erasures) {
    gamma = polyMul(gamma, Uint8Array.of(1, gfExp(n - 1 - position)));
  }
  // Forney syndromes: the coefficients of S(x)gamma(x) from index f upward
  // are the syndromes of the error-only word.
  const forney = polyMul(syndromeList, gamma).subarray(0, nsym);
  const lambda = berlekampMassey(forney.subarray(f));
  const errorCount = lambda.length - 1;
  if (2 * errorCount + f > nsym) {
    return RS_UNCORRECTABLE;
  }
  const psi = polyMul(lambda, gamma);
  const degree = psi.length - 1;
  // Chien search over all positions.
  const positions = [];
  for (let i = 0; i < n; i += 1) {
    const xInverse = gfExp(255 - ((n - 1 - i) % 255));
    if (polyEval(psi, xInverse) === 0) {
      positions.push(i);
    }
  }
  if (positions.length !== degree) {
    return RS_UNCORRECTABLE;
  }
  // omega(x) = S(x)psi(x) mod x^nsym.
  const omega = polyMul(syndromeList, psi).subarray(0, nsym);
  // Formal derivative of psi in characteristic 2 keeps odd-degree terms.
  const derivative = new Uint8Array(psi.length - 1);
  for (let k = 1; k < psi.length; k += 1) {
    derivative[k - 1] = k % 2 === 1 ? psi[k] : 0;
  }
  const corrected = Uint8Array.from(word);
  for (const i of positions) {
    const x = gfExp(n - 1 - i);
    const xInverse = gfInv(x);
    const numerator = polyEval(omega, xInverse);
    const denominator = polyEval(derivative, xInverse);
    if (denominator === 0) {
      return RS_UNCORRECTABLE;
    }
    corrected[i] ^= gfMul(x, gfDiv(numerator, denominator));
  }
  if (!allZero(syndromes(nsym, corrected))) {
    return RS_UNCORRECTABLE;
  }
  word.set(corrected);
  return positions.length;
}

/**
 * A Reed-Solomon code over GF(256) with a fixed number of parity bytes.
 *
 * Codewords are systematic `data || parity`; decoding corrects errors and
 * erasures while `2 * errors + erasures <= parityLen`.
 */
export class PetalReedSolomon {
  /** @param {number} nsym parity byte count, 1..=254 */
  constructor(nsym) {
    if (!Number.isInteger(nsym) || nsym < 1 || nsym > 254) {
      throw new RangeError("parity byte count out of range");
    }
    this._nsym = nsym;
    this._generator = makeGenerator(nsym);
  }

  /** Number of parity bytes. */
  get parityLen() {
    return this._nsym;
  }

  /**
   * Encodes `data`, returning `data || parity`.
   *
   * @throws {RangeError} when the codeword would exceed 255 bytes
   */
  encode(data) {
    const input = toBytes(data, "Reed-Solomon data");
    const nsym = this._nsym;
    if (input.length + nsym > 255) {
      throw new RangeError("Reed-Solomon codeword longer than 255 bytes");
    }
    const generator = this._generator;
    const remainder = new Uint8Array(nsym);
    for (let index = 0; index < input.length; index += 1) {
      const feedback = input[index] ^ remainder[0];
      for (let j = 0; j < nsym; j += 1) {
        const next = j + 1 < nsym ? remainder[j + 1] : 0;
        remainder[j] = next ^ gfMul(feedback, generator[j + 1]);
      }
    }
    const word = new Uint8Array(input.length + nsym);
    word.set(input);
    word.set(remainder, input.length);
    return word;
  }

  /**
   * Corrects `word` in place, treating `erasures` as known-bad positions.
   *
   * Succeeds when `2 * errors + erasures <= parityLen` and returns the number
   * of corrected positions.
   *
   * @param {Uint8Array} word codeword, corrected in place
   * @param {readonly number[]} [erasures] distrusted byte positions
   * @throws {PetalError} `rs_invalid_shape` or `rs_uncorrectable`
   */
  decode(word, erasures = []) {
    if (!(word instanceof Uint8Array)) {
      throw new TypeError("Reed-Solomon word must be a Uint8Array");
    }
    if (!Array.isArray(erasures)) {
      throw new TypeError("Reed-Solomon erasures must be an array of positions");
    }
    const status = rsCorrect(this._nsym, word, erasures);
    if (status === RS_INVALID_SHAPE) {
      throw new PetalError("rs_invalid_shape");
    }
    if (status === RS_UNCORRECTABLE) {
      throw new PetalError("rs_uncorrectable");
    }
    return status;
  }
}
