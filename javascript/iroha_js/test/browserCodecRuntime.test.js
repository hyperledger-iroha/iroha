import assert from "node:assert/strict";
import test from "node:test";
import {
  createBrowserCodecRuntime,
  fetchBrowserCodecBytes,
} from "../src/browserCodecRuntime.js";
import { createNativeRuntime, resolveOptionalNativeRuntimeBinding } from "../src/nativeRuntime.js";

function moduleFixture() {
  return {
    accountAddressParseEncoded: () => JSON.stringify({ canonicalBytes: [2, 0, 1], networkPrefix: 369 }),
    accountAddressRender: () => JSON.stringify({ canonicalHex: "0x020001", i105: "owner-rendered" }),
    noritoEncodeInstruction: () => Uint8Array.of(1, 2),
    noritoDecodeInstruction: () => '{"Log":{"msg":"fixture"}}',
    noritoEncodeInstructionBoxArchive: () => Uint8Array.of(3, 4),
    noritoDecodeInstructionBoxArchive: () => '{"Log":{"msg":"fixture"}}',
  };
}

test("browser initialization is single-flight and publishes only a complete immutable binding", async () => {
  let finish;
  let calls = 0;
  const module = moduleFixture();
  const runtime = createBrowserCodecRuntime(() => {
    calls += 1;
    return new Promise((resolve) => { finish = resolve; });
  });
  const first = runtime.initialize();
  assert.equal(first, runtime.initialize());
  assert.throws(() => runtime.binding(), { code: "ERR_IROHA_CODEC_NOT_READY" });
  await Promise.resolve();
  assert.equal(calls, 1);
  finish(module);
  await first;
  const binding = runtime.binding();
  assert.ok(Object.isFrozen(binding));
  assert.equal(Object.keys(binding).length, 6);
  module.noritoEncodeInstruction = () => Uint8Array.of(99);
  assert.deepEqual(binding.noritoEncodeInstruction("{}"), Uint8Array.of(1, 2));
  await runtime.initialize();
  assert.equal(calls, 1);
});

test("failed initialization and early account access do not poison an immutable native context", async () => {
  let calls = 0;
  const browser = createBrowserCodecRuntime(() => {
    if (++calls === 1) throw new Error("temporary fetch failure");
    return moduleFixture();
  });
  const native = createNativeRuntime();
  const load = () => browser.binding();
  const resolve = () => resolveOptionalNativeRuntimeBinding(native, load);
  assert.throws(resolve, { code: "ERR_IROHA_CODEC_NOT_READY" });
  await assert.rejects(browser.initialize(), { code: "ERR_IROHA_CODEC_INITIALIZATION" });
  assert.throws(resolve, { code: "ERR_IROHA_CODEC_NOT_READY" });
  await browser.initialize();
  assert.deepEqual(resolve().accountAddressParseEncoded("fixture"), {
    canonicalBytes: Uint8Array.of(2, 0, 1), networkPrefix: 369,
  });
  assert.equal(resolve(), resolve());
});

test("an incomplete module cannot become ready and a later complete load can recover", async () => {
  const module = moduleFixture();
  delete module.noritoDecodeInstructionBoxArchive;
  const runtime = createBrowserCodecRuntime(() => module);
  await assert.rejects(runtime.initialize(), (error) => error.cause.code === "ERR_IROHA_CODEC_ABI");
  assert.throws(() => runtime.binding(), { code: "ERR_IROHA_CODEC_NOT_READY" });
  Object.assign(module, moduleFixture());
  await runtime.initialize();
  assert.ok(runtime.binding());
});

test("Wasm byte results are independent of reused linear memory", async () => {
  const memory = Uint8Array.of(4, 5, 6);
  const runtime = createBrowserCodecRuntime(() => ({
    ...moduleFixture(), noritoEncodeInstruction: () => memory.subarray(1),
  }));
  await runtime.initialize();
  const result = runtime.binding().noritoEncodeInstruction("{}");
  memory.fill(0);
  assert.deepEqual(result, Uint8Array.of(5, 6));
});

test("account conversion rejects wrong Wasm result fields and out-of-range bytes", async () => {
  for (const result of [
    { canonicalBytes: [256], networkPrefix: 369 },
    { canonicalBytes: [1], networkPrefix: 65536 },
    { canonicalBytes: [1], networkPrefix: 369, ignored: true },
  ]) {
    const runtime = createBrowserCodecRuntime(() => ({
      ...moduleFixture(), accountAddressParseEncoded: () => JSON.stringify(result),
    }));
    await runtime.initialize();
    assert.throws(() => runtime.binding().accountAddressParseEncoded("fixture"), { code: "ERR_IROHA_CODEC_ABI" });
  }
});

test("Wasm prefix arguments cannot truncate or wrap and invalid values never enter Rust", async () => {
  let calls = 0;
  const module = moduleFixture();
  const parse = module.accountAddressParseEncoded;
  const render = module.accountAddressRender;
  module.accountAddressParseEncoded = (...args) => { calls += 1; return parse(...args); };
  module.accountAddressRender = (...args) => { calls += 1; return render(...args); };
  const runtime = createBrowserCodecRuntime(() => module);
  await runtime.initialize();
  const binding = runtime.binding();
  for (const prefix of [-1, 0.5, 65536, NaN, Infinity, 369n, "369", true]) {
    assert.throws(() => binding.accountAddressParseEncoded("fixture", prefix), { code: "InvalidArg" });
    assert.throws(() => binding.accountAddressRender(Uint8Array.of(2), prefix), { code: "InvalidArg" });
  }
  assert.throws(() => binding.accountAddressRender(Uint8Array.of(2), null), { code: "InvalidArg" });
  assert.equal(calls, 0);
  for (const prefix of [undefined, null, 0, 369, 65535]) binding.accountAddressParseEncoded("fixture", prefix);
  assert.equal(calls, 5);
});

test("Wasm string and byte entrypoints reject coercion before invoking generated glue", async () => {
  const runtime = createBrowserCodecRuntime(() => moduleFixture());
  await runtime.initialize();
  const binding = runtime.binding();
  for (const input of [null, 1, {}, new String("fixture")]) {
    assert.throws(() => binding.accountAddressParseEncoded(input), { code: "InvalidArg" });
    assert.throws(() => binding.noritoEncodeInstruction(input), { code: "InvalidArg" });
    assert.throws(() => binding.noritoEncodeInstructionBoxArchive(input), { code: "InvalidArg" });
  }
  for (const input of [null, "bytes", [], new ArrayBuffer(2), new Uint16Array(2)]) {
    assert.throws(() => binding.accountAddressRender(input, 369), { code: "InvalidArg" });
    assert.throws(() => binding.noritoDecodeInstruction(input), { code: "InvalidArg" });
    assert.throws(() => binding.noritoDecodeInstructionBoxArchive(input), { code: "InvalidArg" });
  }
});

test("codec transport omits credentials, rejects redirects and collects bounded streams", async () => {
  let request;
  const bytes = await fetchBrowserCodecBytes(new URL("https://example.invalid/codec.wasm"), async (url, options) => {
    request = { url, options };
    return new Response(Uint8Array.of(0, 97, 115, 109));
  });
  assert.deepEqual(bytes, Uint8Array.of(0, 97, 115, 109));
  assert.equal(request.options.credentials, "omit");
  assert.equal(request.options.redirect, "error");
  assert.ok(request.options.signal instanceof AbortSignal);
});

test("codec transport refuses HTTP errors and oversized declared artifacts before consumption", async () => {
  for (const response of [
    new Response("missing", { status: 404 }),
    new Response(null, { headers: { "content-length": String(64 * 1024 * 1024 + 1) } }),
  ]) {
    await assert.rejects(fetchBrowserCodecBytes(new URL("https://example.invalid/codec.wasm"), async () => response));
  }
});

test("codec transport enforces its bound even when content length lies", async () => {
  let cancelled = false;
  const response = {
    ok: true, headers: new Headers({ "content-length": "1" }),
    body: { getReader: () => ({
      read: async () => ({ done: false, value: { byteLength: 64 * 1024 * 1024 + 1 } }),
      cancel: async () => { cancelled = true; }, releaseLock() {},
    }) },
  };
  await assert.rejects(
    fetchBrowserCodecBytes(new URL("https://example.invalid/codec.wasm"), async () => response),
    /transport bound/,
  );
  assert.equal(cancelled, true);
});
