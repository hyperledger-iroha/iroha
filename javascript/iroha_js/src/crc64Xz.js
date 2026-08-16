const UINT64_MASK = 0xffff_ffff_ffff_ffffn;
const CRC64_XZ_REFLECTED_POLYNOMIAL = 0xc96c_5795_d787_0f42n;

const CRC64_XZ_TABLE = /* @__PURE__ */ (() => {
  const table = new Array(256);
  for (let index = 0; index < table.length; index += 1) {
    let crc = BigInt(index);
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 1n) === 0n
          ? crc >> 1n
          : (crc >> 1n) ^ CRC64_XZ_REFLECTED_POLYNOMIAL;
    }
    table[index] = crc;
  }
  return table;
})();

/** Compute the canonical reflected CRC-64/XZ checksum used by Norito frames. */
export function crc64Xz(bytes) {
  let crc = UINT64_MASK;
  for (const byte of bytes) {
    const index = Number((crc ^ BigInt(byte)) & 0xffn);
    crc = CRC64_XZ_TABLE[index] ^ (crc >> 8n);
  }
  return BigInt.asUintN(64, crc ^ UINT64_MASK);
}
