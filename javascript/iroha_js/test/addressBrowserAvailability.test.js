import assert from "node:assert/strict";
import test from "node:test";
import { build } from "esbuild";
import * as sourceAddress from "../src/address.js";

import { BAD_KEYS } from "./fixtures/malformedAccountKeys.js";
import { getCurveEntryById } from "../src/curveRegistry.js";

const unavailableMessage = "Native binding required; iroha_js_host is unavailable in browser builds.";

for (const entryPoint of [
  "@iroha/iroha-js/address", "@iroha/iroha-js/browser", "./dist/address.js",
]) {
  test(`${entryPoint} actual browser bundle rejects account admission before inspecting inputs`, async () => {
    const result = await build({
      stdin: { contents: `export * from ${JSON.stringify(entryPoint)};`, resolveDir: process.cwd() },
      bundle: true, platform: "browser", format: "esm", target: "es2020",
      write: false, minify: true, metafile: true,
    });
    const inputs = Object.keys(result.metafile.inputs);
    assert.ok(inputs.some((name) => name.endsWith("/address.browser.js")));
    assert.equal(inputs.some((name) => /(?:^|\/)address\.js$/u.test(name)
      && !name.endsWith("/public/address.js")), false);
    assert.equal(inputs.some((name) => name.endsWith("/native.js")), false);
    const sdk = await import(`data:text/javascript;base64,${Buffer.from(result.outputFiles[0].text).toString("base64")}`);
    assert.deepEqual(Object.keys(sdk.AccountAddressErrorCode), Object.keys(sourceAddress.AccountAddressErrorCode));
    assert.deepEqual(Object.getOwnPropertyNames(sdk.AccountAddress).sort(),
      Object.getOwnPropertyNames(sourceAddress.AccountAddress).sort());
    assert.deepEqual(Object.getOwnPropertyNames(sdk.AccountAddress.prototype).sort(),
      Object.getOwnPropertyNames(sourceAddress.AccountAddress.prototype).sort());
    let inspected = 0;
    const hostile = new Proxy({}, { get() { inspected += 1; throw new Error("input inspected"); },
      ownKeys() { inspected += 1; throw new Error("input inspected"); } });
    const invalid = [undefined, null, "", "not-an-account", [], new Uint8Array(0), hostile];
    for (const value of invalid) {
      const calls = [
        () => new sdk.AccountAddress(value, hostile),
        ...["fromAccount", "fromCanonicalBytes", "fromI105", "fromAccountId", "parseEncoded"]
          .map((name) => () => sdk.AccountAddress[name](value, hostile)),
        ...["encodeI105AccountAddress", "decodeI105AccountAddress", "inspectAccountId"]
          .map((name) => () => sdk[name](value, hostile)),
      ];
      for (const call of calls) assert.throws(call, { name: "Error", message: unavailableMessage });
    }
    for (const [, curve, publicKeyHex] of BAD_KEYS) {
      assert.throws(() => sdk.AccountAddress.fromAccount({ algorithm: getCurveEntryById(curve).algorithm,
        publicKey: Buffer.from(publicKeyHex, "hex") }), { message: unavailableMessage });
    }
    for (const name of Object.getOwnPropertyNames(sdk.AccountAddress.prototype)) {
      if (name === "constructor") continue;
      assert.throws(() => sdk.AccountAddress.prototype[name].call(hostile, hostile),
        { message: unavailableMessage });
    }
    if (sdk.validatePublicKeyForCurve) {
      assert.throws(() => sdk.validatePublicKeyForCurve(hostile, hostile, hostile),
        { message: unavailableMessage });
      assert.throws(() => sdk.parseCanonicalI105AccountLiteral(hostile),
        { message: unavailableMessage });
    }
    assert.equal(inspected, 0);
  });
}
