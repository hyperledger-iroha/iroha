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

function buildAccountForDomain(domain) {
  const address = AccountAddress.fromAccount({
    domain,
    publicKey: SAMPLE_KEY,
  });
  return {
    address,
    ih58: address.toIH58(),
    canonicalHex: address.canonicalHex(),
    compressed: address.toCompressedSora(),
  };
}

test("inspectAccountId reports selector-free domain summary", () => {
  const { ih58, canonicalHex, compressed } = buildAccountForDomain("wonderland");
  const summary = inspectAccountId(ih58);

  assert.equal(summary.domain.kind, "default");
  assert.equal(summary.domain.warning, null);
  assert.deepEqual(summary.warnings, []);
  assert.equal(summary.ih58.value, ih58);
  assert.equal(summary.ih58.networkPrefix, 753);
  assert.equal(summary.compressed, compressed);
  assert.equal(summary.canonicalHex, canonicalHex);
  assert.equal(summary.detectedFormat.kind, "ih58");
  assert.equal(summary.inputDomain, null);

  const canonicalSummary = inspectAccountId(canonicalHex);
  assert.equal(canonicalSummary.detectedFormat.kind, "canonical-hex");
});

test("inspectAccountId handles default-domain addresses without warnings", () => {
  const { ih58 } = buildAccountForDomain(DEFAULT_DOMAIN_NAME);
  const summary = inspectAccountId(ih58, { networkPrefix: 7 });
  assert.equal(summary.domain.kind, "default");
  assert.equal(summary.domain.warning, null);
  assert.deepEqual(summary.warnings, []);
  assert.equal(summary.ih58.networkPrefix, 7);
});

test("inspectAccountId accepts literals without domain suffix", () => {
  const { ih58 } = buildAccountForDomain("wonderland");
  const summary = inspectAccountId(ih58);
  assert.equal(summary.inputDomain, null);
  assert.equal(summary.detectedFormat.kind, "ih58");
});

test("inspectAccountId warns when a compressed literal is provided", () => {
  const { address } = buildAccountForDomain(DEFAULT_DOMAIN_NAME);
  const compressed = address.toCompressedSora();

  const summary = inspectAccountId(compressed);
  assert.equal(summary.detectedFormat.kind, "compressed");
  assert.equal(summary.compressedWarning.includes("Compressed Sora"), true);
  assert.deepEqual(summary.warnings, [summary.compressedWarning]);
});

test("inspectAccountId rejects malformed literals", () => {
  assert.throws(
    () => inspectAccountId("@legacy-domain"),
    (error) => error instanceof TypeError && error.message.includes("must not include '@domain'"),
  );
  assert.throws(
    () => inspectAccountId("ih58@"),
    (error) => error instanceof TypeError && error.message.includes("must not include '@domain'"),
  );
});

test("inspectAccountId rejects encoded literals with domain suffix", () => {
  const { ih58 } = buildAccountForDomain("wonderland");
  const mismatched = `${ih58}@${DEFAULT_DOMAIN_NAME}`;
  assert.throws(
    () => inspectAccountId(mismatched),
    (error) => error instanceof TypeError && error.message.includes("must not include '@domain'"),
  );
});

test("inspectAccountId normalizes prefix options and enforces validation", () => {
  const { address, ih58 } = buildAccountForDomain("wonderland");
  const summary = inspectAccountId(ih58, { expectPrefix: "753", networkPrefix: "7" });
  assert.equal(summary.ih58.networkPrefix, 7);
  assert.equal(summary.ih58.value, address.toIH58(7));

  const literal = address.toIH58();
  assert.throws(
    () => inspectAccountId(literal, { expectPrefix: "7" }),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.UNEXPECTED_NETWORK_PREFIX,
  );
  assert.throws(
    () => inspectAccountId(literal, { networkPrefix: "padded" }),
    (error) =>
      error instanceof AccountAddressError &&
      error.code === AccountAddressErrorCode.INVALID_IH58_PREFIX,
  );
});
