"use strict";

// This symbol is intentionally absent from the package export map. It keeps
// dependency replacement available to source-level tests without making
// executable hooks part of the public SDK constructor contract.
export const TORII_TEST_HOOKS = Symbol("iroha.js.torii.testHooks");
export const TORII_TEST_NATIVE_BINDING = Symbol(
  "iroha.js.torii.testNativeBinding",
);
