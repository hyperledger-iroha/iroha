import assert from "node:assert/strict";
import test from "node:test";

import {
  hasNoritoBinding,
  hasSm2Binding,
  makeNativeTest,
  nativeSkipMessage,
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

test("makeNativeTest skips when required methods are missing", () => {
  const calls = [];
  const baseTest = (name, options, fn) => {
    calls.push({ name, options, fn });
  };
  const wrapper = makeNativeTest(baseTest, {
    require: noritoRequiredMethods,
    binding: { noritoEncodeInstruction() {} },
  });
  wrapper("native gating", () => {});
  assert.equal(calls.length, 1);
  assert.equal(calls[0].options.skip, nativeSkipMessage);
});

test("makeNativeTest returns base test when requirements are satisfied", () => {
  const baseTest = () => {};
  const wrapper = makeNativeTest(baseTest, {
    require: noritoRequiredMethods,
    binding: buildBinding(noritoRequiredMethods),
  });
  assert.equal(wrapper, baseTest);
});
