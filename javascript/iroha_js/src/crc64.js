const CRC64_POLY = 0x42F0E1EBA9EA3693n;
const CRC64_MASK = (1n << 64n) - 1n;

function buildLookupTable() {
  const table = new Array(256);
  for (let byte = 0; byte < 256; byte += 1) {
    let crc = BigInt(byte) << 56n;
    for (let bit = 0; bit < 8; bit += 1) {
      if ((crc & (1n << 63n)) !== 0n) {
        crc = ((crc << 1n) ^ CRC64_POLY) & CRC64_MASK;
      } else {
        crc = (crc << 1n) & CRC64_MASK;
      }
    }
    table[byte] = crc;
  }
  return table;
}

const CRC64_TABLE = buildLookupTable();

function asUint8Array(value) {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  throw new TypeError("crc64 input must be an ArrayBuffer or Uint8Array");
}

export function crc64(data, initial = 0n) {
  const input = asUint8Array(data);
  let crc = BigInt(initial) & CRC64_MASK;
  for (let index = 0; index < input.length; index += 1) {
    const byte = BigInt(input[index]);
    const tableIndex = Number(((crc >> 56n) ^ byte) & 0xffn);
    crc = (CRC64_TABLE[tableIndex] ^ ((crc << 8n) & CRC64_MASK)) & CRC64_MASK;
  }
  return crc;
}

export function crc64Concat(parts) {
  let checksum = 0n;
  for (const part of parts) {
    checksum = crc64(part, checksum);
  }
  return checksum;
}

export const CRC64 = Object.freeze({
  POLY: CRC64_POLY,
  MASK: CRC64_MASK,
});
