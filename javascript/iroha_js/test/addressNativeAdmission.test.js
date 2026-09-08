"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";
import { AccountAddress, AccountAddressErrorCode } from "../src/address.js";
import { AccountAddress as DistAccountAddress } from "../dist/address.js";
import { AccountAddress as PublicAccountAddress } from "@iroha/iroha-js/address";
import { getCurveEntryById } from "../src/curveRegistry.js";
import { makeNativeTest, nativeBinding } from "./helpers/native.js";

const VALID_KEY_HEX = "68f4b6017d0f876a55c80a82b8388a54aad264d367269e2de8be079c935b5f96";
const HEADER = { version: 0, classId: 0, normVersion: 1, extFlag: false };
import { BAD_KEYS } from "./fixtures/malformedAccountKeys.js";
const nativeTest = makeNativeTest(test, {
  require: ["accountAddressParseEncoded", "accountAddressRender"],
});

function canonicalKey(curve, key) {
  const length = key.length;
  return Buffer.concat([
    Buffer.from(length > 255 ? [2, 2, curve, length >> 8, length & 255] : [2, 0, curve, length]),
    key,
  ]);
}

nativeTest("every account constructor and decoder rejects all twelve malformed key vectors", () => {
  for (const [name, curve, hex] of BAD_KEYS) {
    const publicKey = Buffer.from(hex, "hex");
    const canonical = canonicalKey(curve, publicKey);
    assert.throws(() => nativeBinding.accountAddressRender(canonical, 753), undefined, name);
    for (const Address of [AccountAddress, DistAccountAddress, PublicAccountAddress]) {
      for (const call of [
        () => Address.fromAccount({ publicKey, algorithm: getCurveEntryById(curve).algorithm }),
        () => Address.fromCanonicalBytes(canonical),
        () => new Address(HEADER, { tag: 0, curve, publicKey }),
        () => new Address({ ...HEADER, classId: 1 }, {
          tag: 1, version: 1, threshold: 1, members: [{ curve, weight: 1, publicKey }],
        }),
      ]) {
        assert.throws(call, { code: AccountAddressErrorCode.INVALID_PUBLIC_KEY }, name);
      }
    }
  }
});

nativeTest("admitted account controller is isolated from caller and property mutation", () => {
  for (const Address of [AccountAddress, DistAccountAddress, PublicAccountAddress]) {
    const publicKey = Buffer.from(VALID_KEY_HEX, "hex");
    const header = { ...HEADER };
    const controller = { tag: 0, curve: 1, publicKey };
    const address = new Address(header, controller);
    const original = address.canonicalHex();
    publicKey.fill(0);
    header.classId = 1;
    controller.curve = 99;
    address._header = header;
    address._controller = controller;
    assert.equal(address.canonicalHex(), original);
    const bytes = address.canonicalBytes();
    bytes.fill(0);
    assert.equal(address.canonicalHex(), original);
    const text = address.toI105();
    assert.equal(Address.parseEncoded(text).address.canonicalHex(), original);
    for (const padded of [` ${text}`, `${text} `, `\t${text}\n`]) {
      assert.throws(() => Address.parseEncoded(padded));
      assert.throws(() => Address.fromI105(padded));
    }
  }
});

test("account admission fails closed without a verified native binding", () => {
  const script = `
    import assert from 'node:assert/strict';
    for (const path of ['./src/address.js', './dist/address.js', '@iroha/iroha-js/address']) {
      const { AccountAddress } = await import(path);
      const publicKey = Buffer.from('${VALID_KEY_HEX}', 'hex');
      const canonical = Buffer.concat([Buffer.from([2,0,1,32]),publicKey]);
      for (const call of [
        () => AccountAddress.fromAccount({publicKey}),
        () => AccountAddress.fromCanonicalBytes(canonical),
        () => new AccountAddress(${JSON.stringify(HEADER)}, {tag:0,curve:1,publicKey}),
      ]) assert.throws(call, {code:'ERR_IROHA_NATIVE_BINDING',nativeStatus:'missing_file'});
    }
  `;
  const child = spawnSync(process.execPath, ["--input-type=module", "--eval", script], {
    cwd: fileURLToPath(new URL("..", import.meta.url)),
    env: { ...process.env, IROHA_JS_NATIVE_DIR: fileURLToPath(new URL("./missing-address-native", import.meta.url)) },
    encoding: "utf8",
  });
  assert.equal(child.status, 0, child.stderr || child.stdout);
});

test("browser account admission has no structural validation fallback", async () => {
  const bundled = await build({
    stdin: {
      contents: 'export { AccountAddress } from "@iroha/iroha-js/address";',
      resolveDir: fileURLToPath(new URL("..", import.meta.url)),
    },
    platform: "browser",
    format: "esm",
    bundle: true,
    write: false,
  });
  const { AccountAddress: BrowserAddress } = await import(
    `data:text/javascript;base64,${Buffer.from(bundled.outputFiles[0].contents).toString("base64")}`
  );
  const publicKey = Buffer.from(VALID_KEY_HEX, "hex");
  for (const call of [
    () => BrowserAddress.fromAccount({ publicKey }),
    () => BrowserAddress.fromCanonicalBytes(canonicalKey(1, publicKey)),
    () => new BrowserAddress(HEADER, { tag: 0, curve: 1, publicKey }),
  ]) assert.throws(call, /Native binding required;.*unavailable in browser/u);
});
