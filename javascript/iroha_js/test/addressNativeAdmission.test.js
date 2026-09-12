"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
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
    let forgedReceiverReads = 0;
    const forgedReceiver = new Proxy(Object.create(Address.prototype), {
      get() { forgedReceiverReads += 1; throw new Error("receiver state must remain private"); },
    });
    for (const receiver of [undefined, null, {}, forgedReceiver]) {
      assert.throws(() => Address.prototype.controllerInfo.call(receiver), TypeError);
    }
    assert.equal(forgedReceiverReads, 0);
    const publicKey = Buffer.from(VALID_KEY_HEX, "hex");
    const header = { ...HEADER };
    const controller = { tag: 0, curve: 1, publicKey };
    const address = new Address(header, controller);
    const original = address.canonicalHex();
    const snapshot = address.controllerInfo();
    assert.deepEqual(snapshot, {
      tag: 0, curve: 1, publicKey: Uint8Array.from(publicKey),
    });
    assert.ok(Object.isFrozen(snapshot));
    assert.throws(() => { snapshot.curve = 99; }, TypeError);
    assert.throws(() => { snapshot.publicKey = new Uint8Array(32); }, TypeError);
    snapshot.publicKey.fill(0);
    publicKey.fill(0);
    header.classId = 1;
    controller.curve = 99;
    address._header = header;
    address._controller = controller;
    assert.equal(address.canonicalHex(), original);
    const freshSnapshot = address.controllerInfo();
    assert.notStrictEqual(freshSnapshot, snapshot);
    assert.notStrictEqual(freshSnapshot.publicKey, snapshot.publicKey);
    assert.equal(freshSnapshot.tag, 0);
    assert.equal(freshSnapshot.curve, 1);
    assert.equal(Buffer.from(freshSnapshot.publicKey).toString("hex"), VALID_KEY_HEX);
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

nativeTest("controller snapshots preserve normalized multisig policy and nested key isolation", () => {
  const otherKeyHex = "7ea0e3bd52e207c9d3b0eba65c0704e66fca2d8e165a175218b174fc4160e413";
  for (const Address of [AccountAddress, DistAccountAddress, PublicAccountAddress]) {
    const members = [
      { curve: 1, weight: 2, publicKey: Buffer.from(otherKeyHex, "hex") },
      { curve: 1, weight: 1, publicKey: Buffer.from(VALID_KEY_HEX, "hex") },
    ];
    const policy = { tag: 1, version: 1, threshold: 2, members };
    const address = new Address({ ...HEADER, classId: 1 }, policy);
    const canonical = address.canonicalHex();
    const literal = address.toI105();
    const snapshot = address.controllerInfo();
    assert.equal(snapshot.tag, 1);
    assert.equal(snapshot.version, 1);
    assert.equal(snapshot.threshold, 2);
    assert.deepEqual(snapshot.members.map(({ curve, weight, publicKey }) => ({
      curve, weight, publicKey: Buffer.from(publicKey).toString("hex"),
    })), [
      { curve: 1, weight: 1, publicKey: VALID_KEY_HEX },
      { curve: 1, weight: 2, publicKey: otherKeyHex },
    ]);
    assert.ok(Object.isFrozen(snapshot));
    assert.ok(Object.isFrozen(snapshot.members));
    for (const member of snapshot.members) {
      assert.ok(Object.isFrozen(member));
      assert.deepEqual(Object.keys(member).sort(), ["curve", "publicKey", "weight"]);
      assert.throws(() => { member.weight = 99; }, TypeError);
      assert.throws(() => { member.publicKey = new Uint8Array(32); }, TypeError);
      member.publicKey.fill(0);
    }
    assert.throws(() => { snapshot.threshold = 99; }, TypeError);
    assert.throws(() => snapshot.members.push({}), TypeError);
    for (const member of members) member.publicKey.fill(0);
    members.length = 0;
    policy.threshold = 99;
    address._controller = { tag: 0, curve: 99, publicKey: new Uint8Array(32) };
    assert.equal(address.canonicalHex(), canonical);
    assert.equal(address.toI105(), literal);
    const fresh = address.controllerInfo();
    assert.notStrictEqual(fresh, snapshot);
    assert.notStrictEqual(fresh.members, snapshot.members);
    assert.equal(fresh.threshold, 2);
    for (let index = 0; index < fresh.members.length; index += 1) {
      assert.notStrictEqual(fresh.members[index], snapshot.members[index]);
      assert.notStrictEqual(fresh.members[index].publicKey, snapshot.members[index].publicKey);
    }
    assert.deepEqual(fresh.members.map(({ publicKey }) => Buffer.from(publicKey).toString("hex")),
      [VALID_KEY_HEX, otherKeyHex]);
    assert.deepEqual(Address.fromI105(literal).controllerInfo(), fresh);
  }
});

nativeTest("controller snapshots normalize extended single-key wire controllers", () => {
  const key = Buffer.from(readFileSync(
    new URL("../../../fixtures/account/ml_dsa_public_key.hex", import.meta.url), "utf8",
  ).trim(), "hex");
  for (const Address of [AccountAddress, DistAccountAddress, PublicAccountAddress]) {
    const address = Address.fromAccount({ algorithm: "ml-dsa", publicKey: key });
    const canonical = address.canonicalHex();
    assert.equal(address.canonicalBytes()[1], 2);
    const snapshot = address.controllerInfo();
    assert.equal(snapshot.tag, 0);
    assert.equal(getCurveEntryById(snapshot.curve).algorithm, "ml-dsa");
    assert.deepEqual(Buffer.from(snapshot.publicKey), key);
    snapshot.publicKey.fill(0);
    assert.deepEqual(Buffer.from(address.controllerInfo().publicKey), key);
    assert.equal(address.canonicalHex(), canonical);
  }
});

test("account controller snapshot declarations narrow tags and protect policy metadata", () => {
  const result = spawnSync(process.execPath, [
    fileURLToPath(new URL("../node_modules/typescript/bin/tsc", import.meta.url)),
    "--noEmit", "--strict", "--exactOptionalPropertyTypes", "--noUncheckedIndexedAccess",
    "--module", "NodeNext",
    "--moduleResolution", "NodeNext", "--target", "ES2022", "--types", "node",
    fileURLToPath(new URL("./fixtures/typescript/accountControllerInfo.types.ts", import.meta.url)),
  ], { cwd: fileURLToPath(new URL("..", import.meta.url)), encoding: "utf8" });
  assert.equal(result.status, 0, `tsc failed:\n${result.stdout}\n${result.stderr}`);
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
