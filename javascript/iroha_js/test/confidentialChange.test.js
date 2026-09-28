import assert from "node:assert/strict";
import test from "node:test";
import { createConfidentialChangeHelpers } from "../src/confidentialProofBuilders.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";
import * as publicApi from "../src/index.js";

const rhoHex = "a1".repeat(32);
const maximum = (1n << 128n) - 1n;

test("change conversion preserves exact amounts and retained opening with the native default", () => {
  const nativeValue = Buffer.alloc(32, 3);
  const binding = { defaultConfidentialDiversifier: () => nativeValue };
  const helpers = createConfidentialChangeHelpers(createNativeRuntime(binding));
  binding.defaultConfidentialDiversifier = () => { throw new Error("mutated binding"); };
  for (const [amount, leafIndex] of [[1, 0], [maximum, 65535], [maximum.toString(), 1]]) {
    const change = Object.freeze({ amount, rhoHex });
    const input = helpers.confidentialChangeToInput(change, leafIndex);
    assert.deepEqual(input, { amount: amount.toString(), rhoHex, diversifierHex: "03".repeat(32), leafIndex });
    input.rhoHex = "00".repeat(32);
    assert.equal(change.rhoHex, rhoHex);
    assert.equal(change.amount, amount);
  }
  const result = helpers.defaultConfidentialDiversifier();
  nativeValue.fill(0);
  assert.deepEqual(result, Buffer.alloc(32, 3));
});

test("change conversion validates every private field and authenticated-index shape before native work", () => {
  let calls = 0;
  const helpers = createConfidentialChangeHelpers(createNativeRuntime({ defaultConfidentialDiversifier() { calls += 1; return Buffer.alloc(32); } }));
  const change = { amount: 2n, rhoHex };
  for (const amount of [undefined, null, 0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1, maximum + 1n, " 2", "2.0"]) {
    assert.throws(() => helpers.confidentialChangeToInput({ ...change, amount }, 0));
  }
  for (const invalidRho of [undefined, "AA".repeat(32), "00", Buffer.alloc(32)]) {
    assert.throws(() => helpers.confidentialChangeToInput({ ...change, rhoHex: invalidRho }, 0));
  }
  for (const index of [undefined, -1, 65536, 0.5, "1", 1n, NaN, Infinity]) {
    assert.throws(() => helpers.confidentialChangeToInput(change, index));
  }
  for (const invalid of [null, [], {}, { ...change, diversifierHex: rhoHex }, { ...change, ownerTagHex: rhoHex }]) {
    assert.throws(() => helpers.confidentialChangeToInput(invalid, 0));
  }
  assert.equal(calls, 0);
});

test("default helper rejects missing or malformed native results and public exports are available", () => {
  const absent = createConfidentialChangeHelpers(createNativeRuntime({}));
  assert.throws(() => absent.defaultConfidentialDiversifier(), error => error.code === "NATIVE_UNAVAILABLE");
  const malformed = createConfidentialChangeHelpers(createNativeRuntime({ defaultConfidentialDiversifier: () => Buffer.alloc(31) }));
  assert.throws(() => malformed.confidentialChangeToInput({ amount: 1, rhoHex }, 0), /must be 32 bytes/u);
  assert.equal(typeof publicApi.defaultConfidentialDiversifier, "function");
  assert.equal(typeof publicApi.confidentialChangeToInput, "function");
});
