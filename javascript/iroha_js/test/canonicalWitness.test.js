"use strict";

import assert from "node:assert/strict";
import test from "node:test";

import { CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1 } from "../src/canonicalRequest.js";
import { normalizeCanonicalWitnessHeader } from "../src/canonicalWitness.js";

test("canonical witness headers enforce exact padded base64 and the V1 byte cap", () => {
  const exact = Buffer.alloc(CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1, 1).toString("base64");
  assert.equal(normalizeCanonicalWitnessHeader(exact, "witness"), exact);

  for (const invalid of [
    Buffer.alloc(CANONICAL_REQUEST_MAX_WITNESS_BYTES_V1 + 1, 1).toString("base64"),
    "AQ",
    " AQ==",
    "AA=A",
  ]) {
    assert.throws(
      () => normalizeCanonicalWitnessHeader(invalid, "witness"),
      /exact standard-base64 with padding/u,
    );
  }
});
