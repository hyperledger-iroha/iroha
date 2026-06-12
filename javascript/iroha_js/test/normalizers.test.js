"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import {
  assetReferencesMatch,
  canonicalizeMultihashHex,
  composeAssetHoldingId,
  extractAssetDefinitionId,
  normalizeAccountAliasFqn,
  normalizeAssetAliasFqn,
  normalizeAssetDefinitionId,
  normalizeAssetHoldingId,
  normalizeIdentifierInput,
  normalizeToriiAccountReference,
  tryNormalizeAccountAliasFqn,
  tryExtractAssetDefinitionId,
  tryNormalizeAssetDefinitionId,
} from "../src/normalizers.js";
import { AccountAddress } from "../src/address.js";
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

test("normalizes reusable account and asset aliases", () => {
  assert.equal(normalizeAccountAliasFqn("CBDC@POB.CBSI"), "cbdc@pob.cbsi");
  assert.equal(normalizeAssetAliasFqn("SBD#POB.CBSI"), "sbd#pob.cbsi");
  assert.equal(tryNormalizeAccountAliasFqn("bad alias@pob.cbsi"), null);
});

test("rejects malformed account and asset aliases adversarially", () => {
  for (const value of [
    "banking@@cbsi",
    "banking@pob.cbsi.extra",
    " banking @pob.cbsi",
    "-banking@cbsi",
    "banking@pob..cbsi",
  ]) {
    assert.throws(
      () => normalizeAccountAliasFqn(value),
      (error) => {
        assert(error instanceof ValidationError);
        assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
        return true;
      },
      value,
    );
  }
  for (const value of [
    "sbd##cbsi",
    "sbd#pob.cbsi.extra",
    "sbd#pob..cbsi",
    "sbd #cbsi",
    "sbd#-cbsi",
  ]) {
    assert.throws(
      () => normalizeAssetAliasFqn(value),
      (error) => {
        assert(error instanceof ValidationError);
        assert.equal(error.code, ValidationErrorCode.INVALID_STRING);
        return true;
      },
      value,
    );
  }
});

test("validates canonical asset definition ids and asset holdings", () => {
  const assetId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa";
  const accountId = AccountAddress.fromAccount({ publicKey: Buffer.alloc(32, 1) }).toI105();
  assert.equal(normalizeAssetDefinitionId(assetId), assetId);
  assert.equal(
    composeAssetHoldingId(assetId, accountId, "42"),
    `${assetId}#${accountId}#dataspace:42`,
  );
  assert.equal(assetReferencesMatch(assetId, `${assetId}#${accountId}`), true);
  assert.equal(tryNormalizeAssetDefinitionId("66owaQmAQMuHxPzxUN3bqZ6FJfDb"), null);
});

test("rejects malformed asset definitions and holdings instead of matching by shape", () => {
  const assetId = "66owaQmAQMuHxPzxUN3bqZ6FJfDa";
  const badChecksumAssetId = "66owaQmAQMuHxPzxUN3bqZ6FJfDb";
  const accountId = AccountAddress.fromAccount({ publicKey: Buffer.alloc(32, 2) }).toI105();

  for (const value of [
    "",
    "0".repeat(28),
    `${assetId}:metadata`,
    `${assetId}#${accountId}`,
    "sbd#pob.cbsi",
    badChecksumAssetId,
  ]) {
    assert.equal(tryNormalizeAssetDefinitionId(value), null, value);
  }
  assert.throws(
    () => normalizeAssetHoldingId(`${badChecksumAssetId}#${accountId}`),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_ASSET_DEFINITION_ID);
      return true;
    },
  );
  assert.throws(
    () => normalizeAssetHoldingId(`${assetId}#${accountId}#dataspace:not-a-number`),
    (error) => {
      assert(error instanceof ValidationError);
      assert.equal(error.code, ValidationErrorCode.INVALID_ASSET_ID);
      return true;
    },
  );
  assert.equal(tryExtractAssetDefinitionId(`${badChecksumAssetId}#${accountId}`), null);
  assert.equal(assetReferencesMatch(`${badChecksumAssetId}#${accountId}`, `${badChecksumAssetId}#${accountId}`), false);
  assert.equal(extractAssetDefinitionId(`${assetId}#${accountId}`), assetId);
});

test("normalizes Torii account references without accepting aliases", () => {
  const accountId = AccountAddress.fromAccount({ publicKey: Buffer.alloc(32, 1) }).toI105();
  assert.equal(normalizeToriiAccountReference(accountId), accountId);
  assert.equal(normalizeToriiAccountReference("cbdc@pob.cbsi"), "");
});
