import assert from "node:assert/strict";
import test from "node:test";

import {
  ToriiClient,
  canonicalRequestSignatureMessage,
  generateKeyPair,
  verifyEd25519,
} from "../src/index.js";
import { AccountAddress } from "../src/address.js";

test("ToriiClient attaches canonical signing headers for app endpoints", async () => {
  const captured = [];
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async (url, init) => {
      captured.push({ url, init });
      return new Response(JSON.stringify({ items: [], total: 0 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 9) });
  const accountId = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey,
  }).toI105();

  await client.listAccountAssets(accountId, {
    canonicalAuth: { accountId, privateKey },
    limit: 1,
  });

  assert.equal(captured.length, 1);
  const { url, init } = captured[0];
  assert.equal(init.headers["X-Iroha-Account"], accountId);
  const signatureB64 = init.headers["X-Iroha-Signature"];
  assert.ok(typeof signatureB64 === "string" && signatureB64.length > 0);
  const timestampMs = Number(init.headers["X-Iroha-Timestamp-Ms"]);
  const nonce = init.headers["X-Iroha-Nonce"];
  assert.ok(Number.isFinite(timestampMs));
  assert.ok(typeof nonce === "string" && nonce.length > 0);

  const parsed = new URL(url);
  const message = canonicalRequestSignatureMessage({
    method: init.method,
    path: parsed.pathname,
    query: parsed.search ? parsed.search.slice(1) : "",
    body: "",
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(signatureB64, "base64");
  assert.ok(verifyEd25519(message, signature, publicKey));
});

test("ToriiClient canonical auth accepts byte-array private keys", async () => {
  const captured = [];
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async (url, init) => {
      captured.push({ url, init });
      return new Response(JSON.stringify({ items: [], total: 0 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 3) });
  const accountId = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey,
  }).toI105();

  await client.listAccountAssets(accountId, {
    canonicalAuth: { accountId, privateKey: Array.from(privateKey) },
    limit: 1,
  });

  assert.equal(captured.length, 1);
  const { url, init } = captured[0];
  const parsed = new URL(url);
  const timestampMs = Number(init.headers["X-Iroha-Timestamp-Ms"]);
  const nonce = init.headers["X-Iroha-Nonce"];
  assert.ok(Number.isFinite(timestampMs));
  assert.ok(typeof nonce === "string" && nonce.length > 0);
  const message = canonicalRequestSignatureMessage({
    method: init.method,
    path: parsed.pathname,
    query: parsed.search ? parsed.search.slice(1) : "",
    body: "",
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(init.headers["X-Iroha-Signature"], "base64");
  assert.ok(verifyEd25519(message, signature, publicKey));
});

test("ToriiClient canonical auth rejects non-byte private key arrays", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () =>
      new Response(JSON.stringify({ items: [], total: 0 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
  });
  const { publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 7) });
  const accountId = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey,
  }).toI105();

  await assert.rejects(
    () =>
      client.listAccountAssets(accountId, {
        canonicalAuth: { accountId, privateKey: [256] },
        limit: 1,
      }),
    (error) => error?.name === "ValidationError" && /privateKey\[0\]/i.test(error.message),
  );
});
