// Shared curve-admission rejection controls.
export const BAD_KEYS = [
  ["ed25519 identity", 1, `01${"00".repeat(31)}`],
  ["ed25519 noncanonical y", 1, "ff".repeat(32)],
  ["secp256k1 x out of field", 4, `02${"ff".repeat(32)}`],
  ["bls normal invalid encoding", 3, "ff".repeat(48)],
  ["bls small invalid encoding", 5, "ff".repeat(96)],
  ["ml dsa all zero", 2, "00".repeat(1952)],
  ["gost 256 a out of field", 10, "ff".repeat(64)],
  ["gost 256 b out of field", 11, "ff".repeat(64)],
  ["gost 256 c out of field", 12, "ff".repeat(64)],
  ["gost 512 a out of field", 13, "ff".repeat(128)],
  ["gost 512 b out of field", 14, "ff".repeat(128)],
  ["sm2 coordinates out of field", 15, `000004${"ff".repeat(64)}`],
];
