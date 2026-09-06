import assert from "node:assert/strict";
import test from "node:test";

import {
  createNativeRuntime,
  resolveNativeRuntimeBinding,
  resolveOptionalNativeRuntimeBinding,
} from "../src/nativeRuntime.js";

test("injected native runtimes snapshot dependencies and are immutable", () => {
  const first = function () { return this.state; };
  const injected = { operation: first, state: "first" };
  const runtime = createNativeRuntime(injected);

  injected.operation = () => "mutated";
  injected.state = "mutated";
  injected.addedLater = () => "late";

  const required = resolveNativeRuntimeBinding(runtime);
  assert.equal(Object.isFrozen(runtime), true);
  assert.equal(Object.isFrozen(required), true);
  assert.equal(Object.getPrototypeOf(required), null);
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

test("injected native runtimes reject mutable data exports", () => {
  assert.throws(
    () => createNativeRuntime({ state: { value: "mutable" } }),
    /data exports must be primitive values/,
  );
});

test("optional native runtimes hide only a genuinely missing binding", () => {
  const missing = Object.assign(new Error("missing"), {
    code: "ERR_IROHA_NATIVE_BINDING",
    nativeStatus: "missing_file",
  });
  assert.equal(
    resolveOptionalNativeRuntimeBinding(createNativeRuntime(), () => {
      throw missing;
    }),
    null,
  );

  for (const nativeStatus of [
    "hash_mismatch",
    "manifest_error",
    "source_provenance_error",
    "load_error",
  ]) {
    const integrityFailure = Object.assign(new Error(nativeStatus), {
      code: "ERR_IROHA_NATIVE_BINDING",
      nativeStatus,
    });
    assert.throws(
      () => resolveOptionalNativeRuntimeBinding(
        createNativeRuntime(),
        () => { throw integrityFailure; },
      ),
      (error) => error === integrityFailure,
    );
  }
});

test("native runtimes cache a missing loader outcome", () => {
  const missing = Object.assign(new Error("missing"), {
    code: "ERR_IROHA_NATIVE_BINDING",
    nativeStatus: "missing_file",
  });
  const runtime = createNativeRuntime();
  let loads = 0;
  const load = () => {
    loads += 1;
    throw missing;
  };

  assert.equal(resolveOptionalNativeRuntimeBinding(runtime, load), null);
  assert.equal(resolveOptionalNativeRuntimeBinding(runtime, load), null);
  assert.equal(loads, 1);
});

test("production native runtimes snapshot and cache the verified loader result", () => {
  let loads = 0;
  const binding = {
    state: "verified",
    operation() { return this.state; },
  };
  const runtime = createNativeRuntime();
  const load = () => {
    loads += 1;
    return binding;
  };

  const first = resolveOptionalNativeRuntimeBinding(runtime, load);
  binding.state = "mutated";
  const second = resolveOptionalNativeRuntimeBinding(runtime, load);
  const isolated = resolveOptionalNativeRuntimeBinding(runtime, () => ({
    state: "isolated",
    operation() { return this.state; },
  }));

  assert.equal(loads, 1);
  assert.equal(first, second);
  assert.equal(first.operation(), "verified");
  assert.notEqual(isolated, first);
  assert.equal(isolated.operation(), "isolated");
});
