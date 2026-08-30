import assert from "node:assert/strict";
import test from "node:test";

import {
  createNativeRuntime,
  resolveNativeRuntimeBinding,
  resolveOptionalNativeRuntimeBinding,
} from "../src/nativeRuntime.js";

test("injected native runtimes snapshot dependencies and are immutable", () => {
  const first = () => "first";
  const injected = { operation: first };
  const runtime = createNativeRuntime(injected);

  injected.operation = () => "mutated";
  injected.addedLater = () => "late";

  const required = resolveNativeRuntimeBinding(runtime);
  assert.equal(Object.isFrozen(runtime), true);
  assert.equal(Object.isFrozen(required), true);
  assert.notEqual(required.operation, first);
  assert.equal(required.operation(), "first");
  assert.equal("addedLater" in required, false);
  assert.equal(resolveOptionalNativeRuntimeBinding(runtime), required);
});

test("injected native runtimes reject accessors", () => {
  const injected = {};
  Object.defineProperty(injected, "operation", {
    enumerable: true,
    get() {
      throw new Error("must not execute");
    },
  });

  assert.throws(
    () => createNativeRuntime(injected),
    /must not expose accessors/,
  );
});
