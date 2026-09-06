"use strict";

import { getNativeBinding } from "./native.js";

const RUNTIME_STATE = new WeakMap();
const RESOLVED_BINDINGS = new WeakMap();
const TEST_RESOLVED_BINDINGS = new WeakMap();

function snapshotNativeBinding(binding) {
  if (binding === null || typeof binding !== "object") {
    throw new TypeError("native binding must be an object");
  }
  const snapshot = Object.create(null);
  const functions = [];
  for (const key of Reflect.ownKeys(binding)) {
    const descriptor = Object.getOwnPropertyDescriptor(binding, key);
    if (!descriptor || !("value" in descriptor)) {
      throw new TypeError("native binding must not expose accessors");
    }
    if (typeof descriptor.value === "function") {
      functions.push([key, descriptor]);
      continue;
    }
    if (descriptor.value !== null && typeof descriptor.value === "object") {
      throw new TypeError(
        "native binding data exports must be primitive values",
      );
    }
    Object.defineProperty(snapshot, key, {
      value: descriptor.value,
      enumerable: descriptor.enumerable,
      writable: false,
      configurable: false,
    });
  }
  for (const [key, descriptor] of functions) {
    Object.defineProperty(snapshot, key, {
      value: descriptor.value.bind(snapshot),
      enumerable: descriptor.enumerable,
      writable: false,
      configurable: false,
    });
  }
  return Object.freeze(snapshot);
}

/**
 * Create an immutable native dependency context.
 *
 * Production contexts resolve only through the verified native loader. Both
 * verified and source-test bindings are snapshotted so later mutation cannot
 * change a client's dependencies.
 *
 * @param {object} [injectedBinding]
 * @returns {Readonly<object>}
 */
export function createNativeRuntime(injectedBinding) {
  const runtime = Object.freeze(Object.create(null));
  RUNTIME_STATE.set(
    runtime,
    Object.freeze({
      binding:
        injectedBinding === undefined
          ? undefined
          : snapshotNativeBinding(injectedBinding),
    }),
  );
  return runtime;
}

/** @internal Shared immutable runtime for production SDK entrypoints. */
export const defaultNativeRuntime = /* @__PURE__ */ createNativeRuntime();

function requireRuntimeState(runtime) {
  const state = RUNTIME_STATE.get(runtime);
  if (!state) {
    throw new TypeError("native runtime must be created by createNativeRuntime");
  }
  return state;
}

function resolveLoadedBinding(runtime, loadBinding) {
  if (typeof loadBinding !== "function") {
    throw new TypeError("native binding loader must be a function");
  }
  let cache = RESOLVED_BINDINGS;
  let cacheKey = runtime;
  if (loadBinding !== getNativeBinding) {
    cache = TEST_RESOLVED_BINDINGS.get(runtime);
    if (cache === undefined) {
      cache = new WeakMap();
      TEST_RESOLVED_BINDINGS.set(runtime, cache);
    }
    cacheKey = loadBinding;
  }
  const cached = cache.get(cacheKey);
  if (cached !== undefined) {
    if (!cached.ok) throw cached.error;
    return cached.binding;
  }
  try {
    const binding = snapshotNativeBinding(loadBinding());
    cache.set(cacheKey, Object.freeze({ ok: true, binding }));
    return binding;
  } catch (error) {
    cache.set(cacheKey, Object.freeze({ ok: false, error }));
    throw error;
  }
}

function isMissingNativeBindingError(error) {
  if (error === null || typeof error !== "object") return false;
  const code = Object.getOwnPropertyDescriptor(error, "code");
  const nativeStatus = Object.getOwnPropertyDescriptor(error, "nativeStatus");
  return (
    code?.value === "ERR_IROHA_NATIVE_BINDING" &&
    nativeStatus?.value === "missing_file"
  );
}

/** @param {Readonly<object>} runtime */
export function resolveNativeRuntimeBinding(runtime) {
  const { binding } = requireRuntimeState(runtime);
  return binding ?? resolveLoadedBinding(runtime, getNativeBinding);
}

/**
 * @param {Readonly<object>} runtime
 * @param {() => object} [loadBinding]
 */
export function resolveOptionalNativeRuntimeBinding(
  runtime,
  loadBinding = getNativeBinding,
) {
  const { binding } = requireRuntimeState(runtime);
  if (binding !== undefined) {
    return binding;
  }
  try {
    return resolveLoadedBinding(runtime, loadBinding);
  } catch (error) {
    if (isMissingNativeBindingError(error)) return null;
    throw error;
  }
}
