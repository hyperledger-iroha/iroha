// CRC-32C (Castagnoli) used to bind a reassembled payload to its beacon.

import { toBytes } from "./support.js";

/** Reflected CRC-32C polynomial. */
const CRC32C_POLY = 0x82f63b78;

const CRC32C_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let index = 0; index < 256; index += 1) {
    let crc = index;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1) === 1 ? (crc >>> 1) ^ CRC32C_POLY : crc >>> 1;
    }
    table[index] = crc >>> 0;
  }
  return table;
})();

/**
 * Computes CRC-32C (init `0xFFFFFFFF`, reflected, final xor `0xFFFFFFFF`).
 *
 * @param {Uint8Array | ArrayBuffer | ArrayBufferView | number[]} bytes
 * @returns {number} the checksum as an unsigned 32-bit integer
 */
export function crc32c(bytes) {
  const input = toBytes(bytes, "crc32c input");
  let crc = 0xffffffff;
  for (let index = 0; index < input.length; index += 1) {
    crc = CRC32C_TABLE[(crc ^ input[index]) & 0xff] ^ (crc >>> 8);
  }
  return ~crc >>> 0;
}
