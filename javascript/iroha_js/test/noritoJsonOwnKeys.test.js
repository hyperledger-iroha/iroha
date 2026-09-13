import test from "node:test";
import assert from "node:assert/strict";
import { _createNoritoInstructionApi } from "../src/norito.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";

// Inspect the JSON handed to the Rust owner. These tests do not implement or
// claim a Norito wire codec; native/Wasm parity is qualified separately.
function ownerInput(instruction) {
  let input;
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction(json) { input = JSON.parse(json); return Uint8Array.of(0); },
  }));
  api.noritoEncodeInstruction(instruction, 753);
  return input;
}

for (const mode of ["object", "JSON string"]) {
  test(`Rust receives nested own prototype-named keys from ${mode}`, () => {
    const instruction = JSON.parse('{"Custom":{"payload":{"z":1,"__proto__":{"injected":true},"constructor":{"prototype":{"__proto__":"data"}},"nested":[{"__proto__":null,"hasOwnProperty":"own","toString":"own"}]}}}');
    const original = JSON.stringify(instruction);
    const actual = ownerInput(mode === "object" ? instruction : original);
    assert.deepEqual(actual, instruction);
    assert.ok(Object.hasOwn(actual.Custom.payload, "__proto__"));
    assert.equal(actual.Custom.payload.injected, undefined);
    assert.equal(Object.prototype.injected, undefined);
    assert.equal(JSON.stringify(instruction), original);
  });
}

for (const value of [null, false, 0, "data", [], { own: true }]) {
  test(`Rust receives an own __proto__ value ${JSON.stringify(value)}`, () => {
    const payload = Object.fromEntries([["__proto__", value], ["ordinary", 7]]);
    assert.deepEqual(ownerInput({ Custom: { payload } }).Custom.payload, payload);
  });
}

test("own JSON data bypasses inherited setters without invoking them", () => {
  const name = "bpngOwnKeyRegressionSentinel";
  assert.equal(Object.getOwnPropertyDescriptor(Object.prototype, name), undefined);
  let setterCalls = 0;
  Object.defineProperty(Object.prototype, name, {
    configurable: true,
    set() { setterCalls += 1; throw new Error("inherited setter invoked"); },
  });
  try {
    const payload = JSON.parse('{"bpngOwnKeyRegressionSentinel":{"ordinary":1}}');
    assert.deepEqual(ownerInput({ Custom: { payload } }).Custom.payload, payload);
    assert.equal(setterCalls, 0);
  } finally { delete Object.prototype[name]; }
});

test("inherited and non-enumerable properties are not sent to Rust", () => {
  const payload = Object.create({ inherited: { omit: true }, constructor: "inherited" });
  Object.defineProperty(payload, "own", { value: { keep: true }, enumerable: true });
  Object.defineProperty(payload, "hidden", { value: "omit", enumerable: false });
  assert.deepEqual(ownerInput({ Custom: { payload } }).Custom.payload, { own: { keep: true } });
});

test("a prototype-named key cannot become an inherited instruction variant", () => {
  const instruction = '{"__proto__":{"Custom":{"payload":{"forged":true}}}}';
  const api = _createNoritoInstructionApi(createNativeRuntime({
    noritoEncodeInstruction(json) {
      const value = JSON.parse(json);
      assert.ok(Object.hasOwn(value, "__proto__"));
      assert.equal(value.Custom, undefined);
      throw new Error("Rust owner rejects unknown instruction variant");
    },
  }));
  assert.throws(() => api.noritoEncodeInstruction(instruction, 753), /Rust owner rejects/);
});

test("Map and null-prototype objects preserve own prototype-named keys", () => {
  const expected = JSON.parse('{"constructor":"own","__proto__":{"keep":true},"ordinary":1}');
  for (const payload of [new Map(Object.entries(expected)), Object.assign(Object.create(null), expected)]) {
    assert.deepEqual(ownerInput({ Custom: { payload } }).Custom.payload, expected);
  }
});

test("JSON clone fallback preserves own prototype-named keys", () => {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, "structuredClone");
  Object.defineProperty(globalThis, "structuredClone", { configurable: true, writable: true, value: undefined });
  try {
    const payload = JSON.parse('{"__proto__":{"keep":true},"constructor":"own"}');
    assert.deepEqual(ownerInput({ Custom: { payload } }).Custom.payload, payload);
  } finally {
    if (descriptor) Object.defineProperty(globalThis, "structuredClone", descriptor);
    else delete globalThis.structuredClone;
  }
});

for (const kind of ["ExecuteTrigger", "Register.Domain"]) {
  test(`${kind} retains own prototype-named JSON fields at the owner boundary`, () => {
    const value = JSON.parse('{"__proto__":{"keep":true},"constructor":"own","ordinary":1}');
    const instruction = kind === "ExecuteTrigger"
      ? { ExecuteTrigger: { trigger: "request", args: value } }
      : { Register: { Domain: { id: "wonderland.sora", logo: null, metadata: { entry: value } } } };
    assert.deepEqual(ownerInput(instruction), instruction);
  });
}
