// Raw canonical Pasta Fp values shared by Kaigi's instruction and wire codecs.
const MODULUS_LE = Uint8Array.from([
  1, 0, 0, 0, 237, 48, 45, 153, 27, 249, 76, 9, 252, 152, 70, 34,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 64,
]);

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
