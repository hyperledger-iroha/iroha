import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  LocalSigningContext,
  NetworkId,
  ToriiClient as BaseToriiClient,
  ValidationErrorCode,
  canonicalRequestSignatureMessage,
  generateKeyPair,
  verifyEd25519,
} from "../src/index.js";
import { AccountAddress } from "../src/address.js";

const AUTH_ALIAS = "operator-1@hbl.sbp";
const NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const LOCAL_SIGNING_CONTEXT = new LocalSigningContext(NETWORK_ID);

class ToriiClient extends BaseToriiClient {
  constructor(baseUrl, options = {}) {
    super(baseUrl, { localSigningContext: LOCAL_SIGNING_CONTEXT, ...options });
  }
}

test("ToriiClient emits an exact ASCII alias credential and a verifiable signature", async () => {
  const captured = [];
  const fetchImpl = async (url, init) => {
    captured.push({ url, init });
    return new Response(JSON.stringify({ items: [], total: 0 }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient("https://localhost:8080", { fetchImpl });
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 9) });
  const targetAccountId = AccountAddress.fromAccount({ publicKey }).toI105(369);

  await client.listAccountAssets(targetAccountId, {
    canonicalAuth: { accountId: AUTH_ALIAS, privateKey },
    limit: 1,
  });

  assert.equal(captured.length, 1);
  const { url, init } = captured[0];
  assert.equal(init.headers["X-Iroha-Account"], AUTH_ALIAS);
  const retiredRawHeaderInitKey = ["__iroha", "RawUtf8Headers"].join("");
  assert.equal(Object.hasOwn(init, retiredRawHeaderInitKey), false);
  assert.deepEqual(Object.keys(init).sort(), [
    "body",
    "headers",
    "method",
    "redirect",
    "signal",
  ]);
  assert.equal(init.redirect, "error");

  const parsed = new URL(url);
  const timestampMs = Number(init.headers["X-Iroha-Timestamp-Ms"]);
  const nonce = init.headers["X-Iroha-Nonce"];
  const message = canonicalRequestSignatureMessage({
    networkId: NETWORK_ID,
    method: init.method,
    path: parsed.pathname,
    query: parsed.search ? parsed.search.slice(1) : "",
    body: "",
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(init.headers["X-Iroha-Signature"], "base64");
  assert.equal(verifyEd25519(message, signature, publicKey), true);
});

test("ToriiClient transports an exact canonical I105 credential as canonical hex", async () => {
  const captured = [];
  const fetchImpl = async (url, init) => {
    captured.push({ url, init });
    return new Response(JSON.stringify({ items: [], total: 0 }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient("https://localhost:8080", { fetchImpl });
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 10) });
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105(369);

  await client.listAccountAssets(accountId, {
    canonicalAuth: { accountId, privateKey },
    limit: 1,
  });

  assert.equal(captured.length, 1);
  assert.equal(
    captured[0].init.headers["X-Iroha-Account"],
    AccountAddress.parseEncoded(accountId).address.canonicalHex(),
  );
});

test("ToriiClient rejects every noncanonical canonical-auth credential before fetch", async () => {
  let fetchCalls = 0;
  const fetchImpl = async () => {
    fetchCalls += 1;
    throw new Error("fetch must not run for invalid canonical auth");
  };
  const client = new ToriiClient("https://localhost:8080", { fetchImpl });
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 11) });
  const targetAccountId = AccountAddress.fromAccount({ publicKey }).toI105();
  const invalidCredentials = [
    ` ${AUTH_ALIAS}`,
    `${AUTH_ALIAS} `,
    "Operator-1@hbl.sbp",
    "operator-1@HBL.SBP",
    "opérator-1@hbl.sbp",
    "operator-1%40hbl.sbp",
    Buffer.from(AUTH_ALIAS, "utf8").toString("base64"),
  ];

  for (const accountId of invalidCredentials) {
    await assert.rejects(
      () =>
        client.listAccountAssets(targetAccountId, {
          canonicalAuth: { accountId, privateKey },
          limit: 1,
        }),
      (error) =>
        error?.name === "ValidationError" &&
        error?.code === ValidationErrorCode.INVALID_OBJECT &&
        error?.path === "canonicalAuth.accountId" &&
        /exact canonical I105 account or ASCII account alias/u.test(error.message),
      accountId,
    );
  }
  assert.equal(fetchCalls, 0);
});

test("ToriiClient canonical auth accepts byte-array private keys", async () => {
  const captured = [];
  const fetchImpl = async (url, init) => {
    captured.push({ url, init });
    return new Response(JSON.stringify({ items: [], total: 0 }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient("https://localhost:8080", { fetchImpl });
  const { privateKey, publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 3) });
  const targetAccountId = AccountAddress.fromAccount({ publicKey }).toI105();

  await client.listAccountAssets(targetAccountId, {
    canonicalAuth: { accountId: AUTH_ALIAS, privateKey: Array.from(privateKey) },
    limit: 1,
  });

  assert.equal(captured.length, 1);
  assert.equal(captured[0].init.headers["X-Iroha-Account"], AUTH_ALIAS);
});

test("ToriiClient uses its configured signer for optional account reads", async () => {
  const captured = [];
  const fetchImpl = async (url, init) => {
    captured.push({ url: new URL(url), init });
    return new Response(JSON.stringify({ items: [], total: 0 }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  };
  const privateKey = Buffer.alloc(32, 12);
  const targetAccountId = "target-1@hbl.sbp";
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl,
    canonicalRequestAuth: { accountId: AUTH_ALIAS, privateKey },
  });

  await client.listAccountAssets(targetAccountId);
  await client.listAccountTransactions(targetAccountId);
  await client.listAccountPermissions(targetAccountId);

  assert.deepEqual(
    captured.map(({ url }) => url.pathname),
    [
      `/v1/accounts/${encodeURIComponent(targetAccountId)}/assets`,
      `/v1/accounts/${encodeURIComponent(targetAccountId)}/transactions`,
      `/v1/accounts/${encodeURIComponent(targetAccountId)}/permissions`,
    ],
  );
  for (const { init } of captured) {
    assert.equal(init.headers["X-Iroha-Account"], AUTH_ALIAS);
    assert.ok(init.headers["X-Iroha-Signature"]);
    assert.ok(init.headers["X-Iroha-Timestamp-Ms"]);
    assert.ok(init.headers["X-Iroha-Nonce"]);
    assert.equal(init.redirect, "error");
  }
});

test("ToriiClient permits explicit anonymous optional account reads", async () => {
  const captured = [];
  const fetchImpl = async (url, init) => {
    captured.push({ url: new URL(url), init });
    return new Response(JSON.stringify({ items: [], total: 0 }), {
      status: 200,
      headers: { "content-type": "application/json" },
    });
  };
  const privateKey = Buffer.alloc(32, 13);
  const targetAccountId = "target-2@hbl.sbp";
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl,
    canonicalRequestAuth: { accountId: AUTH_ALIAS, privateKey },
  });

  await client.listAccountAssets(targetAccountId, { canonicalAuth: null });
  await client.listAccountTransactions(targetAccountId, { canonicalAuth: null });
  await client.listAccountPermissions(targetAccountId, { canonicalAuth: null });

  assert.equal(captured.length, 3);
  for (const { init } of captured) {
    assert.equal(init.headers["X-Iroha-Account"], undefined);
    assert.equal(init.headers["X-Iroha-Signature"], undefined);
    assert.equal(init.headers["X-Iroha-Timestamp-Ms"], undefined);
    assert.equal(init.headers["X-Iroha-Nonce"], undefined);
    assert.equal(init.redirect, undefined);
  }
});

test("ToriiClient canonical auth rejects non-byte private key arrays", async () => {
  const client = new ToriiClient("https://localhost:8080", {
    fetchImpl: async () => {
      throw new Error("fetch must not run");
    },
  });
  const { publicKey } = generateKeyPair({ seed: Buffer.alloc(32, 7) });
  const targetAccountId = AccountAddress.fromAccount({ publicKey }).toI105();

  await assert.rejects(
    () =>
      client.listAccountAssets(targetAccountId, {
        canonicalAuth: { accountId: AUTH_ALIAS, privateKey: [256] },
        limit: 1,
      }),
    (error) => error?.name === "ValidationError" && /privateKey\[0\]/u.test(error.message),
  );
});

test("ToriiClient canonical auth has no raw-header or socket transport escape hatch", () => {
  const source = readFileSync(new URL("../src/toriiClient.js", import.meta.url), "utf8");
  assert.equal(source.includes(["__iroha", "RawUtf8Headers"].join("")), false);
  assert.equal(source.includes(["__iroha", "SupportsRawUtf8Headers"].join("")), false);
  assert.doesNotMatch(source, /node:(?:net|tls)/u);
  assert.equal(source.includes(["performNode", "RawUtf8Request"].join("")), false);
});
