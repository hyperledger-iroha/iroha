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

test("numeric I105 sentinel boundary belongs to the canonical owner", async () => {
  const { _parseCanonicalAccountAddress } = await import("../src/address.js");
  for (const literal of ["n422payload", "n423payload"]) {
    const bytes = Uint8Array.of(2, 0, 1, 32);
    const runtime = createNativeRuntime({
      accountAddressParseEncoded(input, expected) {
        assert.equal(input, literal);
        assert.equal(expected, 42);
        return { canonicalBytes: bytes, networkPrefix: 42 };
      },
      accountAddressRender(input, prefix) {
        assert.deepEqual(input, bytes);
        assert.equal(prefix, 42);
        return { canonicalHex: "0x02000120", i105: literal };
      },
    });
    const [prefix, result] = _parseCanonicalAccountAddress(literal, 42, runtime);
    assert.equal(prefix, 42);
    assert.deepEqual(result, bytes);
    result.fill(0);
    assert.deepEqual(bytes, Uint8Array.of(2, 0, 1, 32));
  }
});

test("canonical parse rejects inconsistent owner results and noncanonical rerenders", async () => {
  const { _parseCanonicalAccountAddress } = await import("../src/address.js");
  for (const result of [
    { canonicalBytes: [], networkPrefix: 42 },
    { canonicalBytes: new Uint8Array(), networkPrefix: 42 },
    { canonicalBytes: Uint8Array.of(2), networkPrefix: 422 },
    { canonicalBytes: Uint8Array.of(2), networkPrefix: 42.5 },
    { canonicalBytes: Uint8Array.of(2), networkPrefix: 65536 },
    { canonicalBytes: Uint8Array.of(2), networkPrefix: 42 },
  ]) {
    assert.throws(() => _parseCanonicalAccountAddress("n422payload", 42, createNativeRuntime({
      accountAddressParseEncoded() { return result; },
      accountAddressRender() { return { canonicalHex: "0x02", i105: "changed" }; },
    })), { code: "ERR_UNSUPPORTED_ADDRESS_FORMAT" });
  }
});

test("canonical owner diagnostics preserve prefix and character details", async () => {
  const { _parseCanonicalAccountAddress } = await import("../src/address.js");
  for (const [message, details] of [
    ["ERR_UNEXPECTED_NETWORK_PREFIX: unexpected i105 chain discriminant: expected 42, found 369", { expected: 42, found: 369 }],
    ["ERR_INVALID_I105_CHAR: invalid character `!` in i105 address", { char: "!" }],
  ]) {
    const cause = new Error(message);
    assert.throws(() => _parseCanonicalAccountAddress("input", 42, createNativeRuntime({
      accountAddressParseEncoded() { throw cause; },
    })), (error) => {
      assert.equal(error.cause, cause);
      assert.deepEqual(error.details, details);
      return true;
    });
  }
});
