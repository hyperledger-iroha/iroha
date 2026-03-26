"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
  inspectAccountId,
} from "../src/address.js";

const SAMPLE_KEY = Buffer.from(
  "B935AAF1F4E44B3DB79E5E5A9BA4569E6F3E2310C219F3DDD56D3277828D5480",
  "hex",
);
const DEFAULT_DOMAIN_CANONICAL_I105 =
  "sorauロ1NラhBUd2BツヲトiヤニツヌKSテaリメモQラrメoリナnウリbQウQJニLJ5HSE";

function buildAccount() {
  const address = AccountAddress.fromAccount({
    publicKey: SAMPLE_KEY,
  });
  return {
    address,
    i105: address.toI105(),
    canonicalHex: address.canonicalHex(),
  };
}

test("inspectAccountId reports canonical i105 details", () => {
  const { i105, canonicalHex } = buildAccount();
  const summary = inspectAccountId(i105);

  assert.deepEqual(summary.warnings, []);
  assert.equal(summary.i105.value, i105);
  assert.equal(summary.i105.chainDiscriminant, 753);
  assert.equal(summary.i105Warning.length > 0, true);
  assert.equal(summary.canonicalHex, canonicalHex);
  assert.equal(summary.detectedFormat.kind, "i105");

  assert.throws(
    () => inspectAccountId(canonicalHex),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
  );
});

test("inspectAccountId handles canonical i105 addresses without warnings", () => {
  const { i105 } = buildAccount();
  const summary = inspectAccountId(i105, { chainDiscriminant: 7 });
  assert.deepEqual(summary.warnings, []);
  assert.equal(summary.i105.chainDiscriminant, 7);
});

test("inspectAccountId accepts canonical i105 literals", () => {
  const { i105 } = buildAccount();
  const summary = inspectAccountId(i105);
  assert.equal(summary.detectedFormat.kind, "i105");
});

test("inspectAccountId rejects noncanonical fullwidth-sentinel literals", () => {
  const noncanonical = DEFAULT_DOMAIN_CANONICAL_I105.replace(/^sora/, "ｓｏｒａ");
  assert.throws(
    () => inspectAccountId(noncanonical),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
  );
});

test("inspectAccountId rejects malformed literals", () => {
  assert.throws(
    () => inspectAccountId("@invalid-domain"),
    (error) => error instanceof TypeError && error.message.includes("must not include '@domain'"),
  );
  assert.throws(
    () => inspectAccountId("i105@"),
    (error) => error instanceof TypeError && error.message.includes("must not include '@domain'"),
  );
});

test("inspectAccountId rejects encoded literals with domain suffix", () => {
  const { i105 } = buildAccount();
  const mismatched = `${i105}@dataspace`;
  assert.throws(
    () => inspectAccountId(mismatched),
    (error) => error instanceof TypeError && error.message.includes("must not include '@domain'"),
  );
});

test("inspectAccountId normalizes prefix options and enforces validation", () => {
  const { address, i105 } = buildAccount();
  const summary = inspectAccountId(i105, { expectDiscriminant: "753", chainDiscriminant: "7" });
  assert.equal(summary.i105.chainDiscriminant, 7);
  assert.equal(summary.i105.value, address.toI105(7));

  const literal = address.toI105();
  assert.throws(
    () => inspectAccountId(literal, { expectDiscriminant: "7" }),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
  );
  assert.throws(
    () => inspectAccountId(literal, { chainDiscriminant: "padded" }),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_I105_DISCRIMINANT,
  );
});
