// The one pseudo-random generator shared by whitening and the fountain code.

/**
 * Marsaglia xorshift32 with shifts 13, 17, 5.
 *
 * The generator is part of the wire format: whitening sequences and fountain
 * masks are derived from it, so every implementation must match bit for bit.
 * A zero seed is replaced by `0xDEADBEEF` because xorshift cannot leave the
 * all-zero state.
 */
export class PetalXorshift32 {
  /** @param {number} seed unsigned 32-bit seed */
  constructor(seed) {
    if (!Number.isInteger(seed) || seed < 0 || seed > 0xffffffff) {
      throw new TypeError("xorshift32 seed must be an unsigned 32-bit integer");
    }
    this._state = seed === 0 ? 0xdeadbeef : seed;
  }

  /** Advances the generator and returns the next unsigned 32-bit word. */
  nextU32() {
    let x = this._state;
    x ^= x << 13;
    x ^= x >>> 17;
    x ^= x << 5;
    this._state = x >>> 0;
    return this._state;
  }

  /** Returns the top byte of the next word. */
  nextByte() {
    return this.nextU32() >>> 24;
  }
}
