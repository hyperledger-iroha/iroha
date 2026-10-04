//! Signed collection queries: optional canonical account signatures over the exact one-shot target.
import assert from "node:assert/strict";
import test from "node:test";

import { ed25519 } from "@noble/curves/ed25519";

import {
  NetworkId,
  canonicalRequestSignatureMessage,
  field,
  verifyEd25519,
} from "../src/index.js";
import {
  LocalSigningContext,
  ToriiClient,
  ToriiHttpError,
} from "../src/toriiClient.js";

const NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const FOREIGN_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa7));
const PRIVATE_KEY = Buffer.alloc(32, 0x5a);
const PUBLIC_KEY = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
const ACCOUNT_ID = "alice@wonderland";
const AUTH = Object.freeze({ accountId: ACCOUNT_ID, privateKey: PRIVATE_KEY });

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function header(headers, name) {
  if (typeof headers?.get === "function") return headers.get(name);
  const entry = Object.entries(headers ?? {}).find(
    ([key]) => key.toLowerCase() === name.toLowerCase(),
  );
  return entry?.[1] ?? null;
}

function client(fetchImpl, options = {}) {
  return new ToriiClient("https://torii.example", {
    fetchImpl,
    localSigningContext: new LocalSigningContext(NETWORK_ID, 753),
    maxRetries: 4,
    ...options,
  });
}

test("every collection query signs the exact one-shot target when credentials are given", async () => {
  const requests = [];
  const torii = client(async (url, init) => {
    requests.push({ url: new URL(url), init });
    return jsonResponse({ items: [], next_cursor: null });
  });
  const query = { filter: field("owned_by").eq(ACCOUNT_ID), limit: 1 };
  const options = { canonicalAuth: AUTH };

  await torii.accountTransactions(ACCOUNT_ID).list(query, options);
  await torii.accountAssets(ACCOUNT_ID).list(query, options);
  await torii.domains.list(query, options);
  await torii.accounts.list(query, options);
  await torii.repoAgreements.list(query, options);
  await torii.assetHolders("rose#wonderland").list(query, options);
  await torii.assetDefinitions.list(query, options);
  await torii.nfts.list(query, options);
  await torii.rwas.list(query, options);
  await torii.transactions.list(query, options);

  assert.deepEqual(requests.map(({ url }) => url.pathname), [
    `/v1/accounts/${encodeURIComponent(ACCOUNT_ID)}/transactions/query`,
    `/v1/accounts/${encodeURIComponent(ACCOUNT_ID)}/assets/query`,
    "/v1/domains/query",
    "/v1/accounts/query",
    "/v1/repo/agreements/query",
    "/v1/assets/rose%23wonderland/holders/query",
    "/v1/assets/definitions/query",
    "/v1/nfts/query",
    "/v1/rwas/query",
    "/v1/transactions/query",
  ]);
  for (const { url, init } of requests) {
    assert.equal(init.method, "POST");
    assert.equal(init.redirect, "error");
    assert.equal(header(init.headers, "X-Iroha-Account"), ACCOUNT_ID);
    const message = canonicalRequestSignatureMessage({
      networkId: NETWORK_ID,
      method: init.method,
      path: url.pathname,
      query: url.search.slice(1),
      body: Buffer.from(init.body),
      timestampMs: Number(header(init.headers, "X-Iroha-Timestamp-Ms")),
      nonce: header(init.headers, "X-Iroha-Nonce"),
    });
    const signature = Buffer.from(header(init.headers, "X-Iroha-Signature"), "base64");
    assert.equal(verifyEd25519(message, signature, PUBLIC_KEY), true);
  }
});

test("collection query signatures reject foreign genesis, path, and body substitution", async () => {
  let captured;
  const torii = client(async (url, init) => {
    captured = { url: new URL(url), init };
    return jsonResponse({ items: [], next_cursor: null });
  });
  await torii.accountTransactions(ACCOUNT_ID).list({ limit: 2 }, { canonicalAuth: AUTH });

  const signatureInput = {
    method: captured.init.method,
    path: captured.url.pathname,
    query: captured.url.search.slice(1),
    body: Buffer.from(captured.init.body),
    timestampMs: Number(header(captured.init.headers, "X-Iroha-Timestamp-Ms")),
    nonce: header(captured.init.headers, "X-Iroha-Nonce"),
  };
  const signature = Buffer.from(header(captured.init.headers, "X-Iroha-Signature"), "base64");
  const verify = (networkId, overrides = {}) => verifyEd25519(
    canonicalRequestSignatureMessage({ networkId, ...signatureInput, ...overrides }),
    signature,
    PUBLIC_KEY,
  );

  assert.equal(verify(NETWORK_ID), true);
  assert.equal(verify(FOREIGN_NETWORK_ID), false);
  assert.equal(verify(NETWORK_ID, { path: "/v1/accounts/query" }), false);
  assert.equal(verify(NETWORK_ID, { body: Buffer.from('{"limit":3}') }), false);
});

test("signed collection queries are one-shot and reject precomputed or inline credentials", async () => {
  let calls = 0;
  const torii = client(async () => {
    calls += 1;
    return jsonResponse({ code: "unavailable", message: "try later" }, 503);
  });
  await assert.rejects(
    torii.accounts.list({ limit: 1 }, { canonicalAuth: AUTH }),
    (error) => error instanceof ToriiHttpError && error.status === 503 && error.code === "unavailable",
  );
  assert.equal(calls, 1, "signed requests are never retried");

  const noFetch = client(async () => {
    throw new Error("invalid authentication must fail before fetch");
  });
  await assert.rejects(
    noFetch.accounts.list({ limit: 1 }, { canonicalAuth: { ...AUTH, accountId: " alice@wonderland" } }),
    /exact canonical I105 account or ASCII account alias/u,
  );
  await assert.rejects(
    noFetch.accounts.list({ limit: 1 }, { canonicalAuth: AUTH, privateKey: "inline-secret" }),
    /unsupported fields: privateKey/u,
  );
  const precomputed = client(async () => {
    throw new Error("precomputed headers must fail before fetch");
  }, { defaultHeaders: { "X-Iroha-Signature": "precomputed" } });
  await assert.rejects(
    precomputed.accounts.list({}, { canonicalAuth: AUTH }),
    /cannot be precomputed/u,
  );
  const unsigned = client(async (_url, init) => {
    assert.equal(header(init.headers, "X-Iroha-Signature"), null);
    return jsonResponse({ items: [], next_cursor: null });
  });
  await unsigned.accounts.list({ limit: 1 });
});
