import assert from "node:assert/strict";
import test from "node:test";
import { _renderCanonicalAccountAddress, AccountAddressError } from "../src/address.js";
import { createNativeRuntime } from "../src/nativeRuntime.js";

test("account rendering returns the Rust owner result with exact bytes and prefix", () => {
  const canonical = Uint8Array.of(2, 0, 1, 32);
  const runtime = createNativeRuntime({
    accountAddressRender(bytes, prefix) {
      assert.deepEqual(bytes, canonical);
      assert.equal(prefix, 369);
      return { canonicalHex: "0x02000120", i105: "rust-owner-result" };
    },
  });
  assert.equal(_renderCanonicalAccountAddress(canonical, 369, runtime), "rust-owner-result");
});

test("digit-containing native I105 diagnostics preserve typed account errors", () => {
  for (const code of ["ERR_INVALID_I105_CHAR", "ERR_INVALID_I105_BASE", "ERR_INVALID_I105_DIGIT"]) {
    const cause = new Error(`${code}: malformed address fixture`);
    assert.throws(() => _renderCanonicalAccountAddress(Uint8Array.of(2), 369, createNativeRuntime({
      accountAddressRender() { throw cause; },
    })), (error) => error instanceof AccountAddressError && error.code === code && error.cause === cause);
  }
});

test("account rendering propagates native rejection and rejects changed identity", () => {
  const rejection = new Error("Rust key admission rejected");
  assert.throws(() => _renderCanonicalAccountAddress(Uint8Array.of(2), 369, createNativeRuntime({
    accountAddressRender() { throw rejection; },
  })), (error) => error === rejection);
  assert.throws(() => _renderCanonicalAccountAddress(Uint8Array.of(2), 369, createNativeRuntime({
    accountAddressRender() { return { canonicalHex: "0x03", i105: "different-account" }; },
  })), /exact canonical identity/);
});
