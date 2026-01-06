"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import { canonicalizeMultihashHex } from "../src/normalizers.js";
import { ValidationError, ValidationErrorCode } from "../src/validationError.js";

test("canonicalizeMultihashHex rejects non-hex characters", () => {
  assert.throws(
    () => canonicalizeMultihashHex("1202zz", "value"),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_HEX);
      return true;
    },
  );
});

test("canonicalizeMultihashHex rejects mismatched length varints", () => {
  assert.throws(
    () => canonicalizeMultihashHex("1202aa", "value"),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_MULTIHASH);
      return true;
    },
  );
});
