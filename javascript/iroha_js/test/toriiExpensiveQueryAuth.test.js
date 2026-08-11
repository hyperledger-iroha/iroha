import assert from "node:assert/strict";
import test from "node:test";

import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import {
  NetworkId,
  canonicalRequestSignatureMessage,
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
const ACCOUNT_ID = AccountAddress.fromAccount({ publicKey: PUBLIC_KEY }).toI105();
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
    localSigningContext: new LocalSigningContext(NETWORK_ID),
    maxRetries: 4,
    ...options,
  });
}

test("all expensive application query callers sign the exact one-shot target", async () => {
  const requests = [];
  const torii = client(async (url, init) => {
    requests.push({ url: new URL(url), init });
    return jsonResponse({ items: [], total: 0 });
  });
  const options = { canonicalAuth: AUTH, limit: 1 };

  await torii.queryAccountTransactions(ACCOUNT_ID, options);
  await torii.queryAccountAssets(ACCOUNT_ID, options);
  await torii.queryDomains(options);
  await torii.queryAccounts(options);
  await torii.queryTransactions(options);
  await torii.queryVisibleTransactions(options);
  await torii.queryRepoAgreements(options);
  await torii.queryAssetHolders("rose#wonderland", options);
  await torii.queryAssetDefinitions(options);
  await torii.queryNfts(options);
  await torii.queryRwas(options);

  assert.deepEqual(requests.map(({ url }) => url.pathname), [
    `/v1/accounts/${encodeURIComponent(ACCOUNT_ID)}/transactions/query`,
    `/v1/accounts/${encodeURIComponent(ACCOUNT_ID)}/assets/query`,
    "/v1/domains/query",
    "/v1/accounts/query",
    "/v1/transactions/query",
    "/v1/transactions/visible/query",
    "/v1/repo/agreements/query",
    "/v1/assets/rose%23wonderland/holders/query",
    "/v1/assets/definitions/query",
    "/v1/nfts/query",
    "/v1/rwas/query",
  ]);
  for (const { url, init } of requests) {
    assert.equal(init.method, "POST");
    assert.equal(init.redirect, "error");
    assert.equal(
      Buffer.from(header(init.headers, "X-Iroha-Account"), "latin1").toString("utf8"),
      ACCOUNT_ID,
    );
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

test("query signatures reject foreign genesis, path, and body substitution", async () => {
  let captured;
  const torii = client(async (url, init) => {
    captured = { url: new URL(url), init };
    return jsonResponse({ items: [], total: 0 });
  });
  await torii.queryAccountTransactions(ACCOUNT_ID, {
    canonicalAuth: AUTH,
    limit: 2,
  });

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
  assert.equal(verify(NETWORK_ID, { path: "/v1/transactions/query" }), false);
  assert.equal(verify(NETWORK_ID, { body: Buffer.from('{"pagination":{"limit":3},"sort":[]}') }), false);
});

test("expensive query authentication rejects legacy shapes before one-shot dispatch", async () => {
  let calls = 0;
  const torii = client(async () => {
    calls += 1;
    return jsonResponse({ error: "unavailable" }, 503);
  });
  await assert.rejects(
    torii.queryAccounts({ canonicalAuth: AUTH, limit: 1 }),
    (error) => error instanceof ToriiHttpError && error.status === 503,
  );
  assert.equal(calls, 1);

  const noFetch = client(async () => {
    throw new Error("invalid authentication must fail before fetch");
  });
  await assert.rejects(noFetch.queryAccounts({ limit: 1 }), /canonicalAuth is required/);
  await assert.rejects(
    noFetch.queryAccounts({ canonicalAuth: { ...AUTH, accountId: "alice@wonderland" } }),
    /canonical I105/,
  );
  await assert.rejects(
    noFetch.queryAccounts({ canonicalAuth: AUTH, privateKey: "inline-secret" }),
    /unsupported fields: privateKey/,
  );
  await assert.rejects(
    noFetch.queryAccounts({
      canonicalAuth: AUTH,
      headers: { "X-Iroha-Signature": "precomputed" },
    }),
    /unsupported fields: headers|cannot be precomputed/,
  );
  const precomputed = client(async () => {
    throw new Error("precomputed headers must fail before fetch");
  }, { defaultHeaders: { "X-Iroha-Signature": "precomputed" } });
  await assert.rejects(
    precomputed.queryAccounts({ canonicalAuth: AUTH }),
    /cannot be precomputed/,
  );
});
