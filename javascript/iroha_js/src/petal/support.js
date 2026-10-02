// Shared helpers of the Petal Stream port: the error type, byte-input
// normalisation and the IEEE-754 helpers that reproduce Rust `f64` semantics
// (`total_cmp`, `min`, `max`, `clamp`) exactly.

/** Messages of every {@link PetalError} code, identical to the Rust reference. */
const MESSAGES = Object.freeze({
  rs_invalid_shape: "invalid Reed-Solomon codeword shape",
  rs_uncorrectable: "Reed-Solomon word is uncorrectable",
  empty_payload: "petal stream payload is empty",
  payload_too_large: "petal stream payload exceeds the 24-bit length field",
  unsupported_image: "petal image has an unsupported size",
  no_finders: "petal finders not found",
  no_orientation: "no petal orientation produced a readable lane",
});

/**
 * Error raised by the Petal Stream codec.
 *
 * `code` is one of `rs_invalid_shape`, `rs_uncorrectable`, `empty_payload`,
 * `payload_too_large`, `unsupported_image`, `no_finders` or `no_orientation`.
 */
export class PetalError extends Error {
  constructor(code) {
    super(MESSAGES[code] ?? code);
    this.name = "PetalError";
    this.code = code;
  }
}

/** Returns a byte view of `value` without copying typed-array inputs. */
export function toBytes(value, name) {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  if (Array.isArray(value)) {
    const out = new Uint8Array(value.length);
    for (let index = 0; index < value.length; index += 1) {
      const byte = value[index];
      if (!Number.isInteger(byte) || byte < 0 || byte > 255) {
        throw new TypeError(`${name} must contain only bytes (0..=255)`);
      }
      out[index] = byte;
    }
    return out;
  }
  throw new TypeError(`${name} must be a Uint8Array, an ArrayBuffer (view) or a byte array`);
}

/** Validates a non-negative integer argument. */
export function requireIndex(value, name) {
  if (!Number.isInteger(value) || value < 0) {
    throw new TypeError(`${name} must be a non-negative integer`);
  }
  return value;
}

/**
 * Rust `f64::total_cmp` for the values the decoder produces: `-0` sorts
 * before `+0` and NaN sorts after every number (positive NaN).
 */
export function totalCmp(a, b) {
  if (a < b) return -1;
  if (a > b) return 1;
  if (a === b) {
    if (a !== 0) return 0;
    const negativeA = 1 / a < 0;
    const negativeB = 1 / b < 0;
    if (negativeA === negativeB) return 0;
    return negativeA ? -1 : 1;
  }
  const nanA = a !== a;
  const nanB = b !== b;
  if (nanA && nanB) return 0;
  return nanA ? 1 : -1;
}

/** Rust `f64::min`: a NaN operand yields the other operand. */
export function fmin(a, b) {
  return a < b || b !== b ? a : b;
}

/** Rust `f64::max`: a NaN operand yields the other operand. */
export function fmax(a, b) {
  return a > b || b !== b ? a : b;
}

/** Rust `f64::clamp` (NaN stays NaN). */
export function clamp(value, low, high) {
  if (value < low) return low;
  if (value > high) return high;
  return value;
}

/** Freezes a nested array of plain values. */
export function deepFreeze(value) {
  if (Array.isArray(value)) {
    for (const item of value) deepFreeze(item);
    return Object.freeze(value);
  }
  if (value !== null && typeof value === "object") {
    for (const item of Object.values(value)) deepFreeze(item);
    return Object.freeze(value);
  }
  return value;
}
