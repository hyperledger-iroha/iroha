import assert from "node:assert/strict";
import test from "node:test";

import { crc64Xz } from "../src/crc64Xz.js";

test("crc64Xz matches the canonical CRC-64/XZ check value", () => {
  assert.equal(crc64Xz(new Uint8Array()), 0n);
  assert.equal(
    crc64Xz(new TextEncoder().encode("123456789")),
    0x995d_c9bb_df19_39fan,
  );
});
