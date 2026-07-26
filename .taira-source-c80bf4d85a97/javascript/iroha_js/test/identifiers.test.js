"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import { looksLikeIban, normalizeIban } from "../src/identifiers.js";
import { ValidationError, ValidationErrorCode } from "../src/validationError.js";

const VALID_IBAN = "GB82WEST12345698765432";

test("normalizeIban enforces country-specific lengths and check digits", () => {
  assert.equal(normalizeIban(" gb82 west12345698765432 "), VALID_IBAN);

  let lengthError;
  assert.throws(() => {
    try {
      normalizeIban("GB82WEST1234569876543");
    } catch (error) {
      lengthError = error;
      throw error;
    }
  });
  assert.ok(lengthError instanceof ValidationError);
  assert.equal(lengthError.code, ValidationErrorCode.INVALID_IBAN);
  assert.equal(lengthError.path, "iban");
  assert.match(lengthError.message, /22 characters long for country GB/);

  let countryError;
  assert.throws(() => {
    try {
      normalizeIban("ZZ82WEST12345698765432");
    } catch (error) {
      countryError = error;
      throw error;
    }
  });
  assert.ok(countryError instanceof ValidationError);
  assert.equal(countryError.code, ValidationErrorCode.INVALID_IBAN);
  assert.equal(countryError.path, "iban");
  assert.match(countryError.message, /supported IBAN country code/);

  let checkDigits;
  assert.throws(() => {
    try {
      normalizeIban("GBB2WEST12345698765432", "alias");
    } catch (error) {
      checkDigits = error;
      throw error;
    }
  });
  assert.ok(checkDigits instanceof ValidationError);
  assert.equal(checkDigits.code, ValidationErrorCode.INVALID_IBAN);
  assert.equal(checkDigits.path, "alias");
  assert.match(checkDigits.message, /numeric check digits/);
});

test("normalizeIban reports typed errors for non-string input", () => {
  let errorInstance;
  assert.throws(() => {
    try {
      normalizeIban(123, "alias");
    } catch (error) {
      errorInstance = error;
      throw error;
    }
  });
  assert.ok(errorInstance instanceof ValidationError);
  assert.equal(errorInstance.code, ValidationErrorCode.INVALID_STRING);
  assert.equal(errorInstance.path, "alias");
});

test("looksLikeIban rejects obvious non-IBAN literals", () => {
  assert.ok(looksLikeIban(VALID_IBAN));
  assert.ok(!looksLikeIban("not-an-iban"));
  assert.ok(!looksLikeIban("GB82WEST!2345698765432"));
  assert.ok(!looksLikeIban("GB82WEST123456987654321234567890123"));
});
