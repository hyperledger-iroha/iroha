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

test("norito encode accepts base64 payloads in JS mode", async () => {
  await withDisabledNative(async () => {
    const mod = await import("../src/norito.js?js-fallback-base64");
    const payload = Buffer.from(JSON.stringify(REGISTER_DOMAIN), "utf8");
    const base64 = payload.toString("base64");
    const encoded = mod.noritoEncodeInstruction(base64);
    assert.ok(Buffer.isBuffer(encoded));
    assert.equal(encoded.toString("base64"), base64);
  });
});

test("norito encode rejects invalid base64 payloads in JS mode", async () => {
  await withDisabledNative(async () => {
    const mod = await import("../src/norito.js?js-fallback-bad-base64");
    assert.throws(() => mod.noritoEncodeInstruction("AAAA===="));
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
