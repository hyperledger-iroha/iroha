import assert from "node:assert/strict";
import { generateKeyPairSync } from "node:crypto";
import test from "node:test";
import { ed25519 } from "@noble/curves/ed25519";

import {
  AccountAddress,
  LocalSigningContext,
  NetworkId,
  ToriiClient,
  canonicalRequestSignatureMessage,
  verifyEd25519,
} from "../src/index.js";

function accountId() {
  const { publicKey } = generateKeyPairSync("ed25519");
  const der = publicKey.export({ format: "der", type: "spki" });
  return AccountAddress.fromAccount({ publicKey: der.subarray(der.length - 32) }).toI105();
}

const PRIVATE_KEY = Buffer.alloc(32, 0x5a);
const PUBLIC_KEY = Buffer.from(ed25519.getPublicKey(PRIVATE_KEY));
const AUTH_ACCOUNT = AccountAddress.fromAccount({ publicKey: PUBLIC_KEY }).toI105();
const FOREIGN_ACCOUNT = accountId();
const AUTH = Object.freeze({
  accountId: AUTH_ACCOUNT,
  privateKey: PRIVATE_KEY,
});

function client(networkByte, fetchImpl, defaultHeaders) {
  return new ToriiClient("https://torii.example", {
    fetchImpl,
    defaultHeaders,
    localSigningContext: new LocalSigningContext(
      NetworkId.fromBytes(Buffer.alloc(32, networkByte)),
    ),
  });
}

test("application POST signatures separate same-label foreign genesis contexts", async () => {
  const requests = [];
  const fetchImpl = async (url, init) => {
    requests.push({ url: new URL(url), init });
    assert.equal(init.redirect, "error");
    return new Response(null, { status: 404 });
  };

  await client(0xa5, fetchImpl).executeRamLfeProgram("lookup", {
    encryptedInput: "ABCD",
    canonicalAuth: AUTH,
  });
  await client(0xa7, fetchImpl).executeRamLfeProgram("lookup", {
    encryptedInput: "ABCD",
    canonicalAuth: AUTH,
  });

  assert.equal(requests.length, 2);
  const networks = [NetworkId.fromBytes(Buffer.alloc(32, 0xa5)), NetworkId.fromBytes(Buffer.alloc(32, 0xa7))];
  for (let index = 0; index < requests.length; index += 1) {
    const { url, init } = requests[index];
    const message = (networkId) => canonicalRequestSignatureMessage({
      networkId,
      method: init.method,
      path: url.pathname,
      query: url.search.slice(1),
      body: init.body,
      timestampMs: Number(init.headers["X-Iroha-Timestamp-Ms"]),
      nonce: init.headers["X-Iroha-Nonce"],
    });
    const signature = Buffer.from(init.headers["X-Iroha-Signature"], "base64");
    assert.equal(verifyEd25519(message(networks[index]), signature, PUBLIC_KEY), true);
    assert.equal(verifyEd25519(message(networks[1 - index]), signature, PUBLIC_KEY), false);
  }
});

test("application POST failures are dispatched once without retry", async () => {
  let calls = 0;
  const torii = client(0xa5, async () => {
    calls += 1;
    return new Response("unavailable", { status: 503 });
  });

  await assert.rejects(() => torii.verifyRamLfeReceipt({
    receipt: {},
    canonicalAuth: AUTH,
  }));
  assert.equal(calls, 1);
});

test("claim path substitution and precomputed canonical headers fail before dispatch", async () => {
  let calls = 0;
  const fetchImpl = async () => {
    calls += 1;
    return new Response(null, { status: 500 });
  };
  const torii = client(0xa5, fetchImpl);
  await assert.rejects(
    () => torii.issueIdentifierClaimReceipt(FOREIGN_ACCOUNT, { canonicalAuth: AUTH }),
    /must equal the exact canonical I105 authority/u,
  );
  const injected = client(0xa5, fetchImpl, { "X-Iroha-Signature": "precomputed" });
  await assert.rejects(
    () => injected.executeRamLfeProgram("lookup", {
      encryptedInput: "ABCD",
      canonicalAuth: AUTH,
    }),
    /cannot be precomputed/u,
  );
  assert.equal(calls, 0);
});
