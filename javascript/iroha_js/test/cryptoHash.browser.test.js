// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import {
  createHash as createNodeHash,
  randomBytes as nodeRandomBytes,
} from "node:crypto";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  createHash as createBrowserHash,
  randomBytes as browserRandomBytes,
} from "../src/cryptoHash.browser.js";
import {
  createHash as createAdapterHash,
  randomBytes as adapterRandomBytes,
} from "../src/cryptoHash.js";

function withGlobalCrypto(value, action) {
  const descriptor = Object.getOwnPropertyDescriptor(globalThis, "crypto");
  Object.defineProperty(globalThis, "crypto", {
    configurable: true,
    enumerable: descriptor?.enumerable ?? true,
    value,
    writable: true,
  });
  try {
    return action();
  } finally {
    if (descriptor) {
      Object.defineProperty(globalThis, "crypto", descriptor);
    } else {
      delete globalThis.crypto;
    }
  }
}

test("package browser field maps only the local crypto adapter", () => {
  const packageJson = JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url), "utf8"),
  );
  assert.equal(
    packageJson.browser["./dist/cryptoHash.js"],
    "./dist/cryptoHash.browser.js",
  );
  assert.equal(packageJson.browser["node:crypto"], undefined);
  assert.equal(
    packageJson.exports["./canonical-request"].browser,
    "./dist/canonicalRequest.js",
  );
  assert.equal(packageJson.dependencies.buffer, "^6.0.3");
  assert.equal(packageJson.dependencies["@noble/hashes"], "^1.5.0");
  const instructionBuilders = readFileSync(
    new URL("../src/instructionBuilders.js", import.meta.url),
    "utf8",
  );
  assert.doesNotMatch(
    instructionBuilders,
    /from "\.\/cryptoHash\.js"/u,
    "instruction builders must not retain the removed hashing adapter import",
  );
  const canonicalRequest = readFileSync(
    new URL("../src/canonicalRequest.js", import.meta.url),
    "utf8",
  );
  assert.match(
    canonicalRequest,
    /import \{ randomBytes \} from "\.\/cryptoHash\.js";/u,
  );
  const nativeAdapter = readFileSync(
    new URL("../src/cryptoHash.js", import.meta.url),
    "utf8",
  );
  assert.match(
    nativeAdapter,
    /export \{ createHash, randomBytes \} from "node:crypto";/u,
    "Node resolution must retain native crypto semantics",
  );
  for (const file of [
    "canonicalRequest.js",
    "instructionBuilders.js",
    "normalizers.js",
    "norito.js",
  ]) {
    const source = readFileSync(new URL(`../src/${file}`, import.meta.url), "utf8");
    assert.match(source, /import \{ Buffer \} from "buffer";/u);
    assert.doesNotMatch(source, /from "node:buffer"/u);
  }
});

test("local crypto adapter preserves native Node exports", () => {
  assert.equal(createAdapterHash, createNodeHash);
  assert.equal(adapterRandomBytes, nodeRandomBytes);
});

function nodeDigest(encoding) {
  const hash = createNodeHash("sha256");
  hash.update("browser-stream", "utf8");
  hash.update(Uint8Array.of(0, 1, 2, 255));
  hash.update(new DataView(Uint8Array.of(3, 4, 5).buffer));
  return hash.digest(encoding);
}

function browserDigest(algorithm, encoding) {
  const hash = createBrowserHash(algorithm);
  assert.equal(hash.update("browser-stream", "utf8"), hash);
  hash.update(Uint8Array.of(0, 1, 2, 255));
  hash.update(new DataView(Uint8Array.of(3, 4, 5).buffer));
  return hash.digest(encoding);
}

test("browser SHA-256 shim matches native streaming byte and hex digests", () => {
  for (const algorithm of ["sha256", "SHA256", "sha-256"]) {
    const bytes = browserDigest(algorithm);
    assert.equal(Buffer.isBuffer(bytes), true);
    assert.deepEqual(bytes, nodeDigest());
    assert.equal(browserDigest(algorithm, "hex"), nodeDigest("hex"));
  }
});

test("browser SHA-256 shim rejects unsupported algorithms and inputs", () => {
  assert.throws(() => createBrowserHash("md5"), /Digest method not supported/u);
  assert.throws(
    () => createBrowserHash(42),
    (error) => error instanceof TypeError && error.code === "ERR_INVALID_ARG_TYPE",
  );
  assert.throws(
    () => createBrowserHash("sha256").update(new ArrayBuffer(1)),
    (error) => error instanceof TypeError && error.code === "ERR_INVALID_ARG_TYPE",
  );
  assert.throws(
    () => createBrowserHash("sha256").update(null),
    (error) => error instanceof TypeError && error.code === "ERR_INVALID_ARG_TYPE",
  );
  assert.throws(
    () => createBrowserHash("sha256").update("x").digest("base64"),
    /only byte and hexadecimal output/u,
  );
});

test("browser SHA-256 shim preserves finalized-state failures", () => {
  const hash = createBrowserHash("sha256").update("payload");
  assert.equal(hash.digest("hex"), createNodeHash("sha256").update("payload").digest("hex"));
  for (const action of [() => hash.update("again"), () => hash.digest()]) {
    assert.throws(
      action,
      (error) => error instanceof Error && error.code === "ERR_CRYPTO_HASH_FINALIZED",
    );
  }
});

test("browser randomBytes uses secure chunked Web Crypto entropy", () => {
  const calls = [];
  const output = withGlobalCrypto(
    {
      getRandomValues(view) {
        calls.push(view.byteLength);
        view.fill(calls.length);
        return view;
      },
    },
    () => browserRandomBytes(65_537),
  );
  assert.equal(Buffer.isBuffer(output), true);
  assert.equal(output.length, 65_537);
  assert.deepEqual(calls, [65_536, 1]);
  assert.equal(output[0], 1);
  assert.equal(output[65_535], 1);
  assert.equal(output[65_536], 2);
});

test("browser randomBytes rejects invalid sizes before entropy access", () => {
  let entropyCalls = 0;
  withGlobalCrypto(
    { getRandomValues() { entropyCalls += 1; } },
    () => {
      for (const size of [-1, 1.5, Number.NaN, Number.POSITIVE_INFINITY, 0x8000_0000]) {
        assert.throws(
          () => browserRandomBytes(size),
          (error) => error instanceof RangeError && error.code === "ERR_OUT_OF_RANGE",
        );
      }
      for (const size of ["16", null, undefined, 16n]) {
        assert.throws(
          () => browserRandomBytes(size),
          (error) => error instanceof TypeError && error.code === "ERR_INVALID_ARG_TYPE",
        );
      }
    },
  );
  assert.equal(entropyCalls, 0);
});

test("browser randomBytes fails closed without Web Crypto", () => {
  for (const cryptoValue of [undefined, null, {}, { getRandomValues: null }]) {
    withGlobalCrypto(cryptoValue, () => {
      assert.throws(
        () => browserRandomBytes(16),
        /globalThis\.crypto\.getRandomValues/u,
      );
    });
  }
});
