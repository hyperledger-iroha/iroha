"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import {
  AccountAddress,
  AccountAddressError,
  AccountAddressErrorCode,
  DEFAULT_DOMAIN_NAME,
  inspectAccountId,
} from "../src/address.js";

const SAMPLE_KEY = Buffer.from(
  "B935AAF1F4E44B3DB79E5E5A9BA4569E6F3E2310C219F3DDD56D3277828D5480",
  "hex",
);
const DEFAULT_DOMAIN_SORA_I105 =
  "sorauﾛ1P5ﾁXEｴﾕGjgﾕﾚﾎﾕｸﾁEtﾀ3ﾂｺ2gALｺﾒefﾍ8DLgｾoCVGUYHS5";
const DEFAULT_DOMAIN_CANONICAL_I105 =
  "6cmzPVPX6eXMQPXrQzgef9LubBFmrK8yVoJ51F9DSpWfztubMTChZA6";

function buildAccountForDomain(domain) {
  const address = AccountAddress.fromAccount({
    domain,
    publicKey: SAMPLE_KEY,
  });
  return {
    address,
    i105: address.toI105(),
    canonicalHex: address.canonicalHex(),
  };
}

test("inspectAccountId reports selector-free domain summary", () => {
  const { i105, canonicalHex } = buildAccountForDomain("wonderland");
  const summary = inspectAccountId(i105);

  assert.equal(summary.domain.kind, "default");
  assert.equal(summary.domain.warning, null);
  assert.deepEqual(summary.warnings, []);
  assert.equal(summary.i105.value, i105);
  assert.equal(summary.i105.chainDiscriminant, 753);
  assert.equal(summary.i105Warning.length > 0, true);
  assert.equal(summary.canonicalHex, canonicalHex);
  assert.equal(summary.detectedFormat.kind, "i105");
  assert.equal(summary.inputDomain, null);

  assert.throws(
    () => inspectAccountId(canonicalHex),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.UNSUPPORTED_ADDRESS_FORMAT,
  );
});

test("inspectAccountId handles default-domain addresses without warnings", () => {
  const { i105 } = buildAccountForDomain(DEFAULT_DOMAIN_NAME);
  const summary = inspectAccountId(i105, { chainDiscriminant: 7 });
  assert.equal(summary.domain.kind, "default");
  assert.equal(summary.domain.warning, null);
  assert.deepEqual(summary.warnings, []);
  assert.equal(summary.i105.chainDiscriminant, 7);
});

test("inspectAccountId accepts literals without domain suffix", () => {
  const { i105 } = buildAccountForDomain("wonderland");
  const summary = inspectAccountId(i105);
  assert.equal(summary.inputDomain, null);
  assert.equal(summary.detectedFormat.kind, "i105");
});

test("inspectAccountId treats `sora` sentinel literals as canonical i105", () => {
  const summary = inspectAccountId(DEFAULT_DOMAIN_SORA_I105);
  assert.equal(summary.detectedFormat.kind, "i105");
  assert.equal(summary.detectedFormat.chainDiscriminant, 753);
  assert.equal(summary.i105.value, DEFAULT_DOMAIN_CANONICAL_I105);
  assert.deepEqual(summary.warnings, []);
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
  const { i105 } = buildAccountForDomain("wonderland");
  const mismatched = `${i105}@${DEFAULT_DOMAIN_NAME}`;
  assert.throws(
    () => inspectAccountId(mismatched),
    (error) => error instanceof TypeError && error.message.includes("must not include '@domain'"),
  );
});

test("inspectAccountId normalizes prefix options and enforces validation", () => {
  const { address, i105 } = buildAccountForDomain("wonderland");
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
