import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  ToriiClient,
  ValidationErrorCode,
  canonicalRequestSignatureMessage,
  generateKeyPair,
  verifyEd25519,
} from "../src/index.js";
import { AccountAddress } from "../src/address.js";

const AUTH_ALIAS = "operator-1@hbl.sbp";

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
  assert.deepEqual(Object.keys(init).sort(), ["body", "headers", "method", "signal"]);

  const parsed = new URL(url);
  const timestampMs = Number(init.headers["X-Iroha-Timestamp-Ms"]);
  const nonce = init.headers["X-Iroha-Nonce"];
  const message = canonicalRequestSignatureMessage({
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
    targetAccountId,
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
        /exact canonical ASCII account alias/u.test(error.message),
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
