// Raw canonical Pasta Fp values shared by Kaigi's instruction and wire codecs.
const MODULUS_LE = Uint8Array.from([
  1, 0, 0, 0, 237, 48, 45, 153, 27, 249, 76, 9, 252, 152, 70, 34,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64,
]);

/**
 * Pre-action roster root of a call with no participants, as lowercase hex.
 * A private `CreateKaigi` proof must bind exactly this value.
 */
export const KAIGI_EMPTY_ROSTER_ROOT_HEX_V1 =
  "bdd344046b24a25a5ec75a0c58712f09180b15b4c2c9ee51f348bd3e984dc3d7";

/**
 * Sample a uniform nonzero Pasta Fp blinding by rejection.
 *
 * `fill` must overwrite its 32-byte argument from a secure random source. The
 * modulus lies just above 2^254, so masking the top bit makes most draws
 * canonical while rejection keeps the result uniform over the field.
 *
 * @param {(bytes: Uint8Array) => void} fill
 * @returns {Uint8Array}
 */
export function sampleKaigiBlindingV1(fill) {
  const candidate = new Uint8Array(32);
  for (let attempt = 0; attempt < 128; attempt += 1) {
    fill(candidate);
    candidate[31] &= 0x7f;
    if (isNonzeroCanonicalScalar(candidate)) {
      return candidate;
    }
  }
  candidate.fill(0);
  throw new Error("the random source never produced a canonical nonzero Kaigi scalar");
}

function isNonzeroCanonicalScalar(bytes) {
  if (bytes.every((byte) => byte === 0)) {
    return false;
  }
  for (let index = 31; index >= 0; index -= 1) {
    if (bytes[index] < MODULUS_LE[index]) {
      return true;
    }
    if (bytes[index] > MODULUS_LE[index]) {
      return false;
    }
  }
  return false;
}

export function kaigiScalarBytesV1(value, context) {
  if (!(value instanceof Uint8Array) && !Array.isArray(value)) {
    throw new TypeError(`${context} must contain 32 raw canonical Pasta Fp bytes`);
  }
  if (value.length !== 32) {
    throw new RangeError(`${context} must contain exactly 32 bytes`);
  }
  const bytes = new Uint8Array(32);
  for (let index = 0; index < 32; index += 1) {
    const byte = value[index];
    if (!Number.isInteger(byte) || byte < 0 || byte > 255) {
      throw new TypeError(`${context} contains an invalid byte`);
    }
    bytes[index] = byte;
  }
  for (let index = 31; index >= 0; index -= 1) {
    if (bytes[index] < MODULUS_LE[index]) {
      return bytes;
    }
    if (bytes[index] > MODULUS_LE[index]) {
      break;
    }
  }
  throw new RangeError(`${context} must be strictly below the Pasta Fp modulus`);
}
