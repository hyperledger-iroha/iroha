import { test } from "node:test";
import assert from "node:assert/strict";
import { Buffer } from "node:buffer";
import {
  createConnectSessionPreview,
  generateConnectSid,
} from "../src/connectSession.js";
import { NetworkId } from "../src/networkId.js";

function testNetworkId(fill = 0x41) {
  const bytes = new Uint8Array(32).fill(fill);
  bytes[31] |= 1;
  return NetworkId.fromBytes(bytes);
}

test("generateConnectSid derives deterministic sid", () => {
  const networkId = testNetworkId();
  const appPublicKey = Buffer.alloc(32, 0x01);
  const nonce = Buffer.alloc(16, 0x02);

  const result = generateConnectSid({ networkId, appPublicKey, nonce });

  assert.equal(
    result.sidBase64Url,
    "NYWJG9y5e88ugmF2QZQP7dTCwL6UbSG2A8YNvPpX9LI",
  );
  assert.equal(result.sidBytes.length, 32);
  assert.equal(result.nonce.toString("hex"), nonce.toString("hex"));
});

test("createConnectSessionPreview builds URIs and reuses supplied keypair", () => {
  const networkId = testNetworkId(0x43);
  const node = "torii.devnet.example";
  const nonce = Buffer.from("aabbccddeeff00998877665544332211", "hex");
  const appKeyPair = {
    publicKey: Buffer.from("d8".repeat(32), "hex"),
    privateKey: Buffer.from("44".repeat(32), "hex"),
  };

  const preview = createConnectSessionPreview({ networkId, node, nonce, appKeyPair });

  assert.equal(preview.networkId, networkId);
  assert.equal(preview.node, node);
  assert.equal(preview.sidBytes.length, 32);
  assert.match(preview.sidBase64Url, /^[A-Za-z0-9_-]+$/);
  assert.equal(preview.sidBase64Url.includes("="), false);
  assert.equal(preview.appKeyPair.publicKey.equals(Buffer.from(appKeyPair.publicKey)), true);
  assert.equal(preview.appKeyPair.privateKey.equals(Buffer.from(appKeyPair.privateKey)), true);
  assert.match(preview.walletUri, /^iroha:\/\/connect\?/);
  assert.match(preview.appUri, /^iroha:\/\/connect\?/);
  assert(preview.walletUri.includes(`network_id=${encodeURIComponent(networkId.toString())}`));
  assert(preview.walletUri.includes(`app_pk=${encodeURIComponent(appKeyPair.publicKey.toString("base64url"))}`));
  assert(preview.walletUri.includes(`node=${encodeURIComponent(node)}`));
  assert(preview.appUri.includes("role=app"));
});

test("createConnectSessionPreview generates keypair when omitted", () => {
  const preview = createConnectSessionPreview({ networkId: testNetworkId() });

  assert.equal(preview.appKeyPair.publicKey.length, 32);
  assert.equal(preview.appKeyPair.privateKey.length, 32);
  assert.equal(preview.nonce.length, 16);
  assert.equal(preview.sidBytes.length, 32);
});

test("generateConnectSid accepts base64url inputs", () => {
  const networkId = testNetworkId();
  const toBase64Url = (buffer) =>
    buffer
      .toString("base64")
      .replace(/\+/g, "-")
      .replace(/\//g, "_")
      .replace(/=+$/g, "");
  const appPublicKey = Buffer.alloc(32, 0x01);
  const nonce = Buffer.alloc(16, 0x02);

  const result = generateConnectSid({
    networkId,
    appPublicKey: toBase64Url(appPublicKey),
    nonce: toBase64Url(nonce),
  });

  assert.equal(result.sidBytes.length, 32);
  assert.equal(result.nonce.length, 16);
});

test("generateConnectSid rejects invalid base64 inputs", () => {
  const networkId = testNetworkId();
  const nonce = Buffer.alloc(16, 0x02);

  assert.throws(
    () => generateConnectSid({ networkId, appPublicKey: "not*base64", nonce }),
    (error) =>
      error instanceof TypeError && /hex or base64/.test(error.message),
  );
});

test("generateConnectSid rejects invalid byte arrays", () => {
  const networkId = testNetworkId();
  const appPublicKey = new Array(32).fill(0);
  appPublicKey[0] = 256;
  const nonce = new Array(16).fill(1);

  assert.throws(
    () => generateConnectSid({ networkId, appPublicKey, nonce }),
    (error) =>
      error instanceof TypeError && /appPublicKey\[0\] must be a byte/.test(error.message),
  );
});

test("generateConnectSid rejects coercible non-byte array entries", () => {
  const networkId = testNetworkId();
  const nonce = new Array(16).fill(1);

  for (const entry of ["1", true, null]) {
    const appPublicKey = new Array(32).fill(0);
    appPublicKey[0] = entry;
    assert.throws(
      () => generateConnectSid({ networkId, appPublicKey, nonce }),
      (error) =>
        error instanceof TypeError && /appPublicKey\[0\] must be a byte/.test(error.message),
    );
  }
});

test("generateConnectSid separates same-label deployments by exact genesis", () => {
  const appPublicKey = Buffer.alloc(32, 0x01);
  const nonce = Buffer.alloc(16, 0x02);
  const left = generateConnectSid({
    networkId: testNetworkId(0x45),
    appPublicKey,
    nonce,
  });
  const right = generateConnectSid({
    networkId: testNetworkId(0x47),
    appPublicKey,
    nonce,
  });
  assert.notEqual(left.sidBase64Url, right.sidBase64Url);
  assert.throws(
    () => generateConnectSid({ chainId: "same-label", appPublicKey, nonce }),
    /NetworkId/,
  );
});
