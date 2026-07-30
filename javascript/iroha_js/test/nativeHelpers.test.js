import assert from "node:assert/strict";
import test from "node:test";

import {
  hasNoritoBinding,
  hasNativeBinding,
  hasSm2Binding,
  makeNativeTest,
  nativeBinding,
  nativeBindingError,
  nativeUnavailableMessage,
  noritoRequiredMethods,
  sm2RequiredMethods,
} from "./helpers/native.js";

function buildBinding(methods) {
  const binding = {};
  for (const method of methods) {
    binding[method] = () => {};
  }
  return binding;
}

test("hasNoritoBinding checks for required native methods", () => {
  assert.equal(hasNoritoBinding(null), false);
  assert.equal(hasNoritoBinding({ noritoEncodeInstruction() {} }), false);
  assert.equal(hasNoritoBinding(buildBinding(noritoRequiredMethods)), true);
});

test("hasSm2Binding checks for required native methods", () => {
  assert.equal(hasSm2Binding(null), false);
  assert.equal(hasSm2Binding({ sm2Keypair() {} }), false);
  assert.equal(hasSm2Binding(buildBinding(sm2RequiredMethods)), true);
});

test("native helper records binding load failures without aborting import", () => {
  assert.equal(hasNativeBinding, nativeBinding !== null);
  if (nativeBindingError !== null) {
    assert.equal(hasNativeBinding, false);
    assert.match(nativeBindingError.message, /Native binding required/);
  }
});

test("makeNativeTest hard-fails when the native binding is unavailable", () => {
  const calls = [];
  const baseTest = (name, optionsOrFn, maybeFn) => {
    calls.push({
      name,
      options:
        typeof optionsOrFn === "function" ? undefined : optionsOrFn,
      fn: typeof optionsOrFn === "function" ? optionsOrFn : maybeFn,
    });
  };
  const wrapper = makeNativeTest(baseTest, {
    require: noritoRequiredMethods,
    binding: null,
  });
  wrapper("native unavailable", () => {});
  assert.equal(calls.length, 1);
  assert.equal(calls[0].options, undefined);
  assert.throws(
    calls[0].fn,
    (error) => {
      assert.equal(error.code, "ERR_IROHA_NATIVE_TEST_REQUIREMENT");
      assert.equal(error.message, nativeUnavailableMessage);
      assert.equal(error.cause, undefined);
      return true;
    },
  );
});

test("makeNativeTest names every missing required native method", () => {
  const calls = [];
  const baseTest = (name, optionsOrFn, maybeFn) => {
    calls.push({
      name,
      options:
        typeof optionsOrFn === "function" ? undefined : optionsOrFn,
      fn: typeof optionsOrFn === "function" ? optionsOrFn : maybeFn,
    });
  };
  const wrapper = makeNativeTest(baseTest, {
    require: noritoRequiredMethods,
    binding: { noritoEncodeInstruction() {} },
  });
  wrapper("native method gate", { timeout: 1_000 }, () => {});
  assert.equal(calls.length, 1);
  assert.deepEqual(calls[0].options, { timeout: 1_000 });
  assert.throws(
    calls[0].fn,
    (error) => {
      assert.equal(error.code, "ERR_IROHA_NATIVE_TEST_REQUIREMENT");
      assert.equal(
        error.message,
        "native iroha_js_host binding is missing required method(s): noritoDecodeInstruction",
      );
      return true;
    },
  );
});

test("makeNativeTest distinguishes failed capability predicates", () => {
  const calls = [];
  const baseTest = (name, fn) => {
    calls.push({ name, fn });
  };
  const wrapper = makeNativeTest(baseTest, {
    require: () => false,
    binding: {},
  });
  wrapper("native predicate gate", () => {});
  assert.equal(calls.length, 1);
  assert.throws(
    calls[0].fn,
    /does not satisfy the required capability predicate/u,
  );
});

test("makeNativeTest returns base test when requirements are satisfied", () => {
  const baseTest = () => {};
  const wrapper = makeNativeTest(baseTest, {
    require: noritoRequiredMethods,
    binding: buildBinding(noritoRequiredMethods),
  });
  assert.equal(wrapper, baseTest);
});
