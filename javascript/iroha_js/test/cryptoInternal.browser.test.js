import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { _createCryptoApi } from "../src/cryptoInternal.browser.js";
import * as publicBrowserCrypto from "../src/public/crypto.browser.js";

test("internal browser crypto context keeps the public browser surface closed", () => {
  const context = _createCryptoApi({});
  const seed = Buffer.alloc(32, 0x11);
  assert.deepEqual(Object.keys(context), ["publicKeyFromPrivate"]);
  assert.deepEqual(
    context.publicKeyFromPrivate(seed),
    publicBrowserCrypto.publicKeyFromPrivate(seed),
  );
  assert.equal("_createCryptoApi" in publicBrowserCrypto, false);

  const packageJson = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  assert.equal(
    packageJson.browser["./dist/crypto.js"],
    "./dist/cryptoInternal.browser.js",
  );
});
