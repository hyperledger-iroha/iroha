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

const NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xc7));
const FOREIGN_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xd9));
const PRIVATE_KEY = Buffer.alloc(32, 0x3d);
const PUBLIC_KEY = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
const ACCOUNT_ID = AccountAddress.fromAccount({ publicKey: PUBLIC_KEY }).toI105();
const AUTH = Object.freeze({ accountId: ACCOUNT_ID, privateKey: PRIVATE_KEY });
const REQUEST = Object.freeze({
  vkRef: Object.freeze({ backend: "halo2/ipa", name: "ivm-exec-v1" }),
  authority: ACCOUNT_ID,
  metadata: Object.freeze({}),
  bytecode: "Y29kZQ==",
});
const PROVED = Object.freeze({
  bytecode: "Y29kZQ==",
  overlay: Object.freeze([]),
  events_commitment: "01".repeat(32),
  gas_policy_commitment: "02".repeat(32),
});

function client(fetchImpl, options = {}) {
  return new ToriiClient("https://torii.example", {
    fetchImpl,
    localSigningContext: new LocalSigningContext(NETWORK_ID),
    maxRetries: 8,
    ...options,
  });
}

test("IVM derive is authority-bound, exact-network signed, and one-shot", async () => {
  const calls = [];
  const torii = client(async (url, init) => {
    calls.push({ url, init });
    return new Response(JSON.stringify({ proved: PROVED }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  });

  assert.deepEqual(await torii.deriveIvmProved(REQUEST, { canonicalAuth: AUTH }), {
    proved: PROVED,
  });
  assert.equal(calls.length, 1);
  const [{ url, init }] = calls;
  assert.equal(url, "https://torii.example/v1/zk/ivm/derive");
  assert.equal(init.method, "POST");
  assert.equal(init.redirect, "error");
  assert.equal(
    init.headers["X-Iroha-Account"],
    AccountAddress.parseEncoded(ACCOUNT_ID).address.canonicalHex(),
  );
  const timestampMs = Number(init.headers["X-Iroha-Timestamp-Ms"]);
  const nonce = init.headers["X-Iroha-Nonce"];
  const body = Buffer.from(init.body);
  const canonical = canonicalRequestSignatureMessage({
    networkId: NETWORK_ID,
    method: "POST",
    path: "/v1/zk/ivm/derive",
    query: "",
    body,
    timestampMs,
    nonce,
  });
  const foreign = canonicalRequestSignatureMessage({
    networkId: FOREIGN_NETWORK_ID,
    method: "POST",
    path: "/v1/zk/ivm/derive",
    query: "",
    body,
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(init.headers["X-Iroha-Signature"], "base64");
  assert.equal(ed25519.verify(signature, canonical, PUBLIC_KEY), true);
  assert.equal(ed25519.verify(signature, foreign, PUBLIC_KEY), false);
});

test("IVM derive fails before dispatch without exact owner authentication", async () => {
  let calls = 0;
  const torii = client(async () => {
    calls += 1;
    throw new Error("must not dispatch");
  });
  await assert.rejects(() => torii.deriveIvmProved(REQUEST), /canonicalAuth is required/u);
  await assert.rejects(
    () => torii.deriveIvmProved(REQUEST, {
      canonicalAuth: { accountId: "foreign-owner@wonderland", privateKey: PRIVATE_KEY },
    }),
    /must equal the exact payload authority/u,
  );
  assert.equal(calls, 0);

  const contextless = new ToriiClient("https://torii.example", {
    fetchImpl: async () => { throw new Error("must not dispatch"); },
  });
  await assert.rejects(
    () => contextless.deriveIvmProved(REQUEST, { canonicalAuth: AUTH }),
    /immutable LocalSigningContext/u,
  );
});

test("IVM derive does not retry an authenticated compute request", async () => {
  let calls = 0;
  const torii = client(async (_url, init) => {
    calls += 1;
    assert.equal(init.redirect, "error");
    return new Response("unavailable", { status: 503 });
  });
  await assert.rejects(
    () => torii.deriveIvmProved(REQUEST, { canonicalAuth: AUTH }),
    /503/u,
  );
  assert.equal(calls, 1);
});
