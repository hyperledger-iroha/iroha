import { test } from "node:test";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import {
  createConnectSessionPreview,
  generateConnectSid,
} from "../src/connectSession.js";

test("generateConnectSid derives deterministic sid", () => {
  const chainId = "test-chain";
  const appPublicKey = Buffer.alloc(32, 0x01);
  const nonce = Buffer.alloc(16, 0x02);

  const result = generateConnectSid({ chainId, appPublicKey, nonce });

  assert.equal(
    result.sidBase64Url,
    "F9outoYkA--2Yd_l7DDGBFilF9ZFJLw6Jb4OVzwpZCk",
  );
  assert.equal(result.sidBytes.length, 32);
  assert.equal(result.nonce.toString("hex"), nonce.toString("hex"));
});

test("createConnectSessionPreview builds URIs and reuses supplied keypair", () => {
  const chainId = "nexus-devnet";
  const node = "torii.devnet.example";
  const nonce = Buffer.from("aabbccddeeff00998877665544332211", "hex");
  const appKeyPair = {
    publicKey: Buffer.from("d8".repeat(32), "hex"),
    privateKey: Buffer.from("44".repeat(32), "hex"),
  };

  const preview = createConnectSessionPreview({ chainId, node, nonce, appKeyPair });

  assert.equal(preview.chainId, chainId);
  assert.equal(preview.node, node);
  assert.equal(preview.sidBytes.length, 32);
  assert.match(preview.sidBase64Url, /^[A-Za-z0-9_-]+$/);
  assert.equal(preview.sidBase64Url.includes("="), false);
  assert.equal(preview.appKeyPair.publicKey.equals(Buffer.from(appKeyPair.publicKey)), true);
  assert.equal(preview.appKeyPair.privateKey.equals(Buffer.from(appKeyPair.privateKey)), true);
  assert.match(preview.walletUri, /^iroha:\/\/connect\?/);
  assert.match(preview.appUri, /^iroha:\/\/connect\/app\?/);
  assert(preview.walletUri.includes(`chain_id=${encodeURIComponent(chainId)}`));
  assert(preview.walletUri.includes(`node=${encodeURIComponent(node)}`));
});

test("createConnectSessionPreview generates keypair when omitted", () => {
  const preview = createConnectSessionPreview({ chainId: "alpha-net" });

  assert.equal(preview.appKeyPair.publicKey.length, 32);
  assert.equal(preview.appKeyPair.privateKey.length, 32);
  assert.equal(preview.nonce.length, 16);
  assert.equal(preview.sidBytes.length, 32);
});

test("generateConnectSid accepts base64url inputs", () => {
  const chainId = "test-chain";
  const toBase64Url = (buffer) =>
    buffer
      .toString("base64")
      .replace(/\+/g, "-")
      .replace(/\//g, "_")
      .replace(/=+$/g, "");
  const appPublicKey = Buffer.alloc(32, 0x01);
  const nonce = Buffer.alloc(16, 0x02);

  const result = generateConnectSid({
    chainId,
    appPublicKey: toBase64Url(appPublicKey),
    nonce: toBase64Url(nonce),
  });

  assert.equal(result.sidBytes.length, 32);
  assert.equal(result.nonce.length, 16);
});

test("generateConnectSid rejects invalid base64 inputs", () => {
  const chainId = "test-chain";
  const nonce = Buffer.alloc(16, 0x02);

  assert.throws(
    () => generateConnectSid({ chainId, appPublicKey: "not*base64", nonce }),
    (error) =>
      error instanceof TypeError && /hex or base64/.test(error.message),
  );
});
