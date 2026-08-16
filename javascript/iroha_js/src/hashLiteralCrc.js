/** Return the uppercase CRC-16/CCITT-FALSE checksum for a tagged literal body. */
export function computeHashLiteralCrc(tag, body) {
  let crc = 0xffff;
  for (const byte of new TextEncoder().encode(`${tag}:${body}`)) {
    crc ^= (byte & 0xff) << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        (crc & 0x8000) !== 0
          ? ((crc << 1) ^ 0x1021) & 0xffff
          : (crc << 1) & 0xffff;
    }
  }
  return crc.toString(16).toUpperCase().padStart(4, "0");
}
