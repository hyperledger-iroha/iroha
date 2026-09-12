import assert from "node:assert/strict";
import test from "node:test";
import { build } from "esbuild";
import * as sourceAddress from "../src/address.js";

// This actual package-graph test requires build:wasm and build:dist first.
// Pre-initialization checks do not substitute for real Wasm/browser conformance.
for (const entryPoint of [
  "@iroha/iroha-js/address", "@iroha/iroha-js/browser", "./dist/address.js",
]) {
  test(`${entryPoint} browser graph uses the shared account facade and explicit codec lifecycle`, async () => {
    const result = await build({
      stdin: { contents: `export * from ${JSON.stringify(entryPoint)};`, resolveDir: process.cwd() },
      bundle: true, platform: "browser", format: "esm", target: "es2020",
      write: false, minify: true, metafile: true,
    });
    const inputs = Object.keys(result.metafile.inputs);
    assert.ok(inputs.some((name) => /(?:^|\/)address\.js$/u.test(name)
      && !name.endsWith("/public/address.js")));
    assert.ok(inputs.some((name) => name.endsWith("/native.browser.js")));
    assert.ok(inputs.some((name) => name.endsWith("/browserCodec.js")));
    assert.equal(inputs.some((name) => name.endsWith("/address.browser.js")), false);
    assert.equal(inputs.some((name) => name.endsWith("/native.js")), false);
    const sdk = await import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString("base64")}`);
    assert.deepEqual(Object.keys(sdk.AccountAddressErrorCode), Object.keys(sourceAddress.AccountAddressErrorCode));
    assert.deepEqual(Object.getOwnPropertyNames(sdk.AccountAddress).sort(),
      Object.getOwnPropertyNames(sourceAddress.AccountAddress).sort());
    assert.deepEqual(Object.getOwnPropertyNames(sdk.AccountAddress.prototype).sort(),
      Object.getOwnPropertyNames(sourceAddress.AccountAddress.prototype).sort());
    const publicKey = Uint8Array.from(Buffer.from(
      "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a", "hex"));
    assert.throws(() => sdk.AccountAddress.fromAccount({ publicKey }),
      { code: "ERR_IROHA_CODEC_NOT_READY" });
  });
}
