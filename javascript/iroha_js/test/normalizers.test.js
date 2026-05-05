"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import { canonicalizeMultihashHex, normalizeIdentifierInput } from "../src/normalizers.js";
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

test("normalizeIdentifierInput canonicalizes phone and account-number inputs", () => {
  assert.equal(
    normalizeIdentifierInput(" +1 (555) 123-4567 ", "phone_e164", "phone"),
    "+15551234567",
  );
  assert.equal(
    normalizeIdentifierInput(" ab-12 / cd ", "account_number", "accountNumber"),
    "AB12/CD",
  );
});

test("normalizeIdentifierInput rejects malformed emails", () => {
  assert.throws(
    () => normalizeIdentifierInput("broken-email", "email_address", "email"),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
      return true;
    },
  );
});
