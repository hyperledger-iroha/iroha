import assert from "node:assert/strict";
import { test } from "node:test";

import { blake2s } from "../src/blake2s.js";

test("blake2s preserves numeric byte arrays", () => {
  assert.deepEqual(blake2s([0, 1, 255]), blake2s(Uint8Array.of(0, 1, 255)));
});

test("blake2s rejects coercible non-byte array entries", () => {
  for (const entry of ["1", true, null]) {
    assert.throws(
      () => blake2s([entry]),
      (error) =>
        error instanceof TypeError && /blake2s input\[0\] must be a byte/.test(error.message),
    );
    assert.throws(
      () => blake2s([0], { key: [entry] }),
      (error) =>
        error instanceof TypeError && /blake2s key\[0\] must be a byte/.test(error.message),
    );
  }
});
