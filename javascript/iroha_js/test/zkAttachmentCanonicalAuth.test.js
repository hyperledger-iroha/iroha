import assert from "node:assert/strict";
import test from "node:test";
import { ed25519 } from "@noble/curves/ed25519";

import {
  LocalSigningContext,
  NetworkId,
  ToriiClient,
  canonicalRequestSignatureMessage,
} from "../src/index.js";
import { AccountAddress } from "../src/address.js";

const NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa7));
const FOREIGN_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xb9));
const PRIVATE_KEY = Buffer.alloc(32, 0x31);
const PUBLIC_KEY = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
const ACCOUNT_ID = AccountAddress.fromAccount({ publicKey: PUBLIC_KEY }).toI105();
const CANONICAL_AUTH = Object.freeze({ accountId: ACCOUNT_ID, privateKey: PRIVATE_KEY });

function client(fetchImpl, options = {}) {
  return new ToriiClient("https://torii.example", {
    fetchImpl,
    localSigningContext: new LocalSigningContext(NETWORK_ID),
    maxRetries: 8,
    ...options,
  });
}

function verifyExactNetworkSignature(call, expectedMethod, expectedPath, expectedBody) {
  const { init, url } = call;
  assert.equal(init.method, expectedMethod);
  assert.equal(new URL(url).pathname, expectedPath);
  assert.equal(init.redirect, "error");
  const timestampMs = Number(init.headers["X-Iroha-Timestamp-Ms"]);
  const nonce = init.headers["X-Iroha-Nonce"];
  const message = canonicalRequestSignatureMessage({
    networkId: NETWORK_ID,
    method: expectedMethod,
    path: expectedPath,
    query: "",
    body: expectedBody,
    timestampMs,
    nonce,
  });
  const foreign = canonicalRequestSignatureMessage({
    networkId: FOREIGN_NETWORK_ID,
    method: expectedMethod,
    path: expectedPath,
    query: "",
    body: expectedBody,
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(init.headers["X-Iroha-Signature"], "base64");
  assert.equal(
    init.headers["X-Iroha-Account"],
    AccountAddress.parseEncoded(ACCOUNT_ID).address.canonicalHex(),
  );
  assert.equal(ed25519.verify(signature, message, PUBLIC_KEY), true);
  assert.equal(ed25519.verify(signature, foreign, PUBLIC_KEY), false);
}

test("ZK attachment lifecycle is exact-network signed and one-shot", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url, init });
    if (init.method === "POST") {
      return new Response(JSON.stringify({
        id: "att/1", content_type: "text/plain", size: 7, created_ms: 1,
      }), { status: 201, headers: { "content-type": "application/json" } });
    }
    if (init.method === "GET" && new URL(url).pathname === "/v1/zk/attachments") {
      return new Response("[]", { status: 200, headers: { "content-type": "application/json" } });
    }
    if (init.method === "GET") {
      return new Response("payload", { status: 200, headers: { "content-type": "text/plain" } });
    }
    return new Response(null, { status: 204 });
  };
  const torii = client(fetchImpl);
  const body = Buffer.from("payload");

  await torii.uploadAttachment(body, { contentType: "text/plain", canonicalAuth: CANONICAL_AUTH });
  await torii.listAttachments({ canonicalAuth: CANONICAL_AUTH });
  await torii.getAttachment("att/1", { canonicalAuth: CANONICAL_AUTH });
  await torii.deleteAttachment("att/1", { canonicalAuth: CANONICAL_AUTH });

  assert.equal(calls.length, 4);
  verifyExactNetworkSignature(calls[0], "POST", "/v1/zk/attachments", body);
  verifyExactNetworkSignature(calls[1], "GET", "/v1/zk/attachments", Buffer.alloc(0));
  verifyExactNetworkSignature(calls[2], "GET", "/v1/zk/attachments/att%2F1", Buffer.alloc(0));
  verifyExactNetworkSignature(calls[3], "DELETE", "/v1/zk/attachments/att%2F1", Buffer.alloc(0));
});

test("ZK attachment APIs fail closed without canonical auth or signing context", async () => {
  let calls = 0;
  const torii = client(async () => {
    calls += 1;
    throw new Error("must not dispatch");
  });
  await assert.rejects(
    () => torii.uploadAttachment(Buffer.alloc(0), { contentType: "application/octet-stream" }),
    /canonicalAuth is required/u,
  );
  await assert.rejects(() => torii.listAttachments(), /canonicalAuth is required/u);
  await assert.rejects(() => torii.getAttachment("att-1"), /canonicalAuth is required/u);
  await assert.rejects(() => torii.deleteAttachment("att-1"), /canonicalAuth is required/u);
  assert.equal(calls, 0);

  const contextless = new ToriiClient("https://torii.example", {
    fetchImpl: async () => { throw new Error("must not dispatch"); },
  });
  await assert.rejects(
    () => contextless.listAttachments({ canonicalAuth: CANONICAL_AUTH }),
    /immutable LocalSigningContext/u,
  );
});

test("ZK attachment authentication disables status retries", async () => {
  let calls = 0;
  const torii = client(async (_url, init) => {
    calls += 1;
    assert.equal(init.redirect, "error");
    return new Response("unavailable", { status: 503 });
  });
  await assert.rejects(
    () => torii.listAttachments({ canonicalAuth: CANONICAL_AUTH }),
    /503/u,
  );
  assert.equal(calls, 1);
});
