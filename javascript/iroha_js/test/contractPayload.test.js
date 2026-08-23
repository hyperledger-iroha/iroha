import test from "node:test";
import assert from "node:assert/strict";

import {
  canonicalContractPayloadJson,
  contractPayloadDigestHex,
} from "../src/contractPayload.js";

test("contract payload digest matches Torii's absent-payload preimage", () => {
  const expected = "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262";
  assert.equal(canonicalContractPayloadJson(undefined), null);
  assert.equal(canonicalContractPayloadJson(null), null);
  assert.equal(contractPayloadDigestHex(undefined), expected);
  assert.equal(contractPayloadDigestHex(null), expected);
});

test("contract payload canonicalization matches the Torii Norito JSON vector", () => {
  const payload = {
    z: "line\ncontrol\u000b😀",
    b: [true, false, null],
    a: 1,
  };
  const canonical = '{"a":1,"b":[true,false,null],"z":"line\\ncontrol\\u000b😀"}';
  assert.equal(canonicalContractPayloadJson(payload), canonical);
  assert.equal(
    contractPayloadDigestHex(payload),
    "f10ae09b778159dda1747afacbf679edb4b61690abd3714dbb387c42c436a2df",
  );
});

test("contract payload keys use Rust UTF-8 byte ordering rather than JavaScript UTF-16 ordering", () => {
  const payload = { "𐀀": 2, "": 1 };
  const canonical = '{"":1,"𐀀":2}';
  assert.equal(canonicalContractPayloadJson(payload), canonical);
  assert.equal(
    contractPayloadDigestHex(payload),
    "420a7bf67b19b06cff5cee796118b273c37afe53dd11200d233929b4f918f827",
  );
});

test("contract payload canonicalization rejects values without exact browser-to-Norito parity", () => {
  for (const value of [1e-6, Number.MAX_SAFE_INTEGER + 1, -0, Number.NaN, Number.POSITIVE_INFINITY]) {
    assert.throws(() => canonicalContractPayloadJson({ value }), /safe integers/u);
  }
  assert.throws(() => canonicalContractPayloadJson({ value: undefined }), /unsupported undefined/u);
  assert.throws(() => canonicalContractPayloadJson({ value: "\ud800" }), /Unicode scalar/u);

  const sparse = [];
  sparse.length = 1;
  assert.throws(() => canonicalContractPayloadJson({ sparse }), /dense/u);

  const cyclic = {};
  cyclic.self = cyclic;
  assert.throws(() => canonicalContractPayloadJson(cyclic), /cycles/u);
});
