import assert from "node:assert/strict";
import test from "node:test";

const REGISTER_DOMAIN = {
  Register: {
    Domain: {
      id: "wonderland",
      logo: null,
      metadata: {
        key: "value",
      },
    },
  },
};

function withDisabledNative(fn) {
  const previous = process.env.IROHA_JS_DISABLE_NATIVE;
  process.env.IROHA_JS_DISABLE_NATIVE = "1";
  return Promise.resolve()
    .then(() => fn())
    .finally(() => {
      if (previous === undefined) {
        delete process.env.IROHA_JS_DISABLE_NATIVE;
      } else {
        process.env.IROHA_JS_DISABLE_NATIVE = previous;
      }
    });
}

test("norito encode/decode falls back to JSON when native is disabled", async () => {
  await withDisabledNative(async () => {
    // Use a cache-busting suffix so we do not reuse an earlier native-loaded module instance.
    const mod = await import("../src/norito.js?js-fallback");
    const encoded = mod.noritoEncodeInstruction(REGISTER_DOMAIN);
    assert.ok(Buffer.isBuffer(encoded));
    const decoded = mod.noritoDecodeInstruction(encoded);
    assert.deepEqual(decoded, REGISTER_DOMAIN);
  });
});

test("norito decode surfaces hint when bytes are not JSON in JS mode", async () => {
  await withDisabledNative(async () => {
    const mod = await import("../src/norito.js?js-fallback-error");
    assert.throws(
      () => mod.noritoDecodeInstruction(Buffer.from([0xde, 0xad])),
      /run `npm run build:native`/,
    );
  });
});
