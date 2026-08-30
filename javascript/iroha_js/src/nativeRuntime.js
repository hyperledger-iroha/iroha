"use strict";

import { getNativeBinding } from "./native.js";

const RUNTIME_STATE = new WeakMap();

function snapshotInjectedBinding(binding) {
  if (binding === null || typeof binding !== "object") {
    throw new TypeError("injected native binding must be an object");
  }
  const snapshot = Object.create(null);
  for (const key of Reflect.ownKeys(binding)) {
    const descriptor = Object.getOwnPropertyDescriptor(binding, key);
    if (!descriptor || !("value" in descriptor)) {
      throw new TypeError("injected native binding must not expose accessors");
    }
    Object.defineProperty(snapshot, key, {
      value:
        typeof descriptor.value === "function"
          ? descriptor.value.bind(binding)
          : descriptor.value,
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
 * Production contexts resolve only through the verified native loader. The
 * optional binding exists for source-level tests and is snapshotted so later
 * mutation cannot change a client's dependencies.
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
          : snapshotInjectedBinding(injectedBinding),
    }),
  );
  return runtime;
}

function requireRuntimeState(runtime) {
  const state = RUNTIME_STATE.get(runtime);
  if (!state) {
    throw new TypeError("native runtime must be created by createNativeRuntime");
  }
  return state;
}

/** @param {Readonly<object>} runtime */
export function resolveNativeRuntimeBinding(runtime) {
  const { binding } = requireRuntimeState(runtime);
  return binding ?? getNativeBinding();
}

/** @param {Readonly<object>} runtime */
export function resolveOptionalNativeRuntimeBinding(runtime) {
  const { binding } = requireRuntimeState(runtime);
  if (binding !== undefined) {
    return binding;
  }
  try {
    return getNativeBinding();
  } catch {
    return null;
  }
}
