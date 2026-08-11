"use strict";

import test from "node:test";
import assert from "node:assert/strict";
import { ed25519 } from "@noble/curves/ed25519";

import {
  canonicalQueryString,
  canonicalRequestMessage,
  canonicalRequestSignatureMessage,
  buildCanonicalRequestHeaders,
  buildCanonicalJsonRequest,
  signEd25519,
  verifyEd25519,
  NetworkId,
} from "../src/index.js";
import { AccountAddress } from "../src/address.js";

const TEST_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));

function deterministicKeyPair(seedByte) {
  const privateKey = Buffer.alloc(32, seedByte);
  return {
    privateKey,
    publicKey: Buffer.from(ed25519.getPublicKey(privateKey)),
  };
}

test("canonical request signing: canonical query sorts pairs", () => {
  const rendered = canonicalQueryString("b=2&a=3&b=1&space=a+b");
  assert.equal(rendered, "a=3&b=1&b=2&space=a+b");
});

test("canonical request signing: canonical query uses form encoding", () => {
  const rendered = canonicalQueryString("b=!*()~'&a=1");
  assert.equal(rendered, "a=1&b=%21*%28%29%7E%27");
});

test("canonical request signing: headers include a verifiable signature", () => {
  const { privateKey, publicKey } = deterministicKeyPair(7);
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105();
  const accountAlias = "alice-1@wonderland";
  const body = Buffer.from('{"foo":1}');
  const path = `/v1/accounts/${accountId}/assets`;
  const timestampMs = 1_717_171_717_000;
  const nonce = "deterministic-nonce";
  const message = canonicalRequestSignatureMessage({
    networkId: TEST_NETWORK_ID,
    method: "get",
    path,
    query: "limit=10",
    body,
    timestampMs,
    nonce,
  });
  const headers = buildCanonicalRequestHeaders({
    accountId: accountAlias,
    networkId: TEST_NETWORK_ID,
    method: "get",
    path,
    query: "limit=10",
    body,
    privateKey,
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(headers["X-Iroha-Signature"], "base64");
  assert.equal(headers["X-Iroha-Account"], accountAlias);
  assert.equal(headers["X-Iroha-Timestamp-Ms"], String(timestampMs));
  assert.equal(headers["X-Iroha-Nonce"], nonce);
  assert.equal(verifyEd25519(message, signature, publicKey), true);

  const i105Headers = buildCanonicalRequestHeaders({
    accountId,
    networkId: TEST_NETWORK_ID,
    method: "get",
    path,
    query: "limit=10",
    body,
    privateKey,
    timestampMs,
    nonce: `${nonce}-i105`,
  });
  assert.notEqual(i105Headers["X-Iroha-Account"], accountId);
  assert.equal(
    Buffer.from(i105Headers["X-Iroha-Account"], "latin1").toString("utf8"),
    accountId,
  );
  assert.ok(
    Array.from(i105Headers["X-Iroha-Account"]).every(
      (character) => character.codePointAt(0) <= 0xff,
    ),
  );
});

test("canonical request signing: exact NetworkId separates same-label deployments", () => {
  const networkId = NetworkId.fromBytes(Uint8Array.from({ length: 32 }, (_value, index) =>
    index === 31 ? 1 : 0,
  ));
  const foreignNetworkId = NetworkId.fromBytes(
    Uint8Array.from({ length: 32 }, (_value, index) => (index === 31 ? 3 : 0)),
  );
  const input = {
    method: "POST",
    path: "/v1/gov/ballots/plain",
    body: Buffer.from('{"network_id":"fixture"}'),
    timestampMs: 1_717_171_717_004,
    nonce: "exact-network-nonce",
  };
  const message = canonicalRequestSignatureMessage({ networkId, ...input });
  const foreignMessage = canonicalRequestSignatureMessage({
    networkId: foreignNetworkId,
    ...input,
  });
  assert.notDeepEqual(message, foreignMessage);
  assert.deepEqual(
    message.subarray(0, "iroha.app.request.network.v1\0".length),
    Buffer.from("iroha.app.request.network.v1\0", "utf8"),
  );
});

test("canonical request signing: rejects padded auth fields", async () => {
  const { privateKey, publicKey } = deterministicKeyPair(11);
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105();
  const accountAlias = "alice-1@wonderland";
  const timestampMs = 1_717_171_717_003;

  assert.throws(
    () =>
      canonicalRequestSignatureMessage({
        networkId: TEST_NETWORK_ID,
        method: "get",
        path: "/v1/accounts",
        timestampMs,
        nonce: " nonce",
      }),
    /surrounding whitespace/,
  );
  assert.throws(
    () =>
      buildCanonicalRequestHeaders({
        accountId: ` ${accountAlias}`,
        networkId: TEST_NETWORK_ID,
        method: "get",
        path: "/v1/accounts",
        privateKey,
        timestampMs,
        nonce: "nonce",
      }),
    /exact canonical I105 account or ASCII account alias/,
  );
  assert.throws(
    () =>
      buildCanonicalRequestHeaders({
        accountId: accountAlias,
        networkId: TEST_NETWORK_ID,
        method: "get",
        path: "/v1/accounts",
        privateKey,
        timestampMs,
        nonce: "nonce\n",
      }),
    /surrounding whitespace/,
  );
  await assert.rejects(
    () =>
      buildCanonicalJsonRequest({
        accountId: `${accountAlias} `,
        networkId: TEST_NETWORK_ID,
        path: "/v1/accounts",
        body: {},
        privateKey,
        timestampMs,
        nonce: "nonce",
      }),
    /exact canonical I105 account or ASCII account alias/,
  );
  await assert.rejects(
    () =>
      buildCanonicalJsonRequest({
        accountId: accountAlias,
        networkId: TEST_NETWORK_ID,
        path: "/v1/accounts",
        body: {},
        privateKey,
        timestampMs,
        nonce: "\tnonce",
      }),
    /surrounding whitespace/,
  );

  for (const invalidAccountId of [
    "Alice-1@wonderland",
    "alice-1@Wonderland",
    "alíce-1@wonderland",
    "alice-1%40wonderland",
    Buffer.from(accountAlias, "utf8").toString("base64"),
  ]) {
    assert.throws(
      () =>
        buildCanonicalRequestHeaders({
          accountId: invalidAccountId,
          networkId: TEST_NETWORK_ID,
          method: "get",
          path: "/v1/accounts",
          privateKey,
          timestampMs,
          nonce: "nonce",
        }),
      /exact canonical I105 account or ASCII account alias/,
      invalidAccountId,
    );
    await assert.rejects(
      () =>
        buildCanonicalJsonRequest({
          accountId: invalidAccountId,
          networkId: TEST_NETWORK_ID,
          path: "/v1/accounts",
          body: {},
          privateKey,
          timestampMs,
          nonce: "nonce",
        }),
      /exact canonical I105 account or ASCII account alias/,
      invalidAccountId,
    );
  }
});

test("canonical request signing: JSON helper signs the exact request body with callback signers", async () => {
  const { privateKey, publicKey } = deterministicKeyPair(8);
  const accountId = "operator-1@mibank.paynet";
  const path = "/v1/aliases/resolve?b=2&a=1";
  const timestampMs = 1_717_171_717_001;
  const nonce = "browser-keystore-nonce";
  let signerInput = null;

  const request = await buildCanonicalJsonRequest({
    accountId,
    networkId: TEST_NETWORK_ID,
    method: "post",
    path,
    body: { alias: "tidal-river-4160@mibank.paynet" },
    headers: { "X-Request-Id": "req_1" },
    timestampMs,
    nonce,
    sign: async (input) => {
      signerInput = input;
      return signEd25519(input.message, privateKey).toString("base64");
    },
  });

  assert.equal(request.method, "POST");
  assert.equal(request.body, '{"alias":"tidal-river-4160@mibank.paynet"}');
  assert.equal(request.headers["Content-Type"], "application/json");
  assert.equal(request.headers.Accept, "application/json");
  assert.equal(request.headers["X-Request-Id"], "req_1");
  assert.equal(request.headers["X-Iroha-Account"], accountId);
  assert.equal(request.headers["X-Iroha-Timestamp-Ms"], String(timestampMs));
  assert.equal(request.headers["X-Iroha-Nonce"], nonce);
  assert.ok(signerInput);
  assert.equal(signerInput.messageBase64, signerInput.message.toString("base64"));
  assert.equal(signerInput.path, "/v1/aliases/resolve");
  assert.equal(signerInput.query, "b=2&a=1");
  assert.equal(signerInput.body, request.body);

  const message = canonicalRequestSignatureMessage({
    networkId: TEST_NETWORK_ID,
    method: request.method,
    path: "/v1/aliases/resolve",
    query: "b=2&a=1",
    body: request.body,
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(request.headers["X-Iroha-Signature"], "base64");
  assert.equal(verifyEd25519(message, signature, publicKey), true);
});

test("canonical request signing: JSON helper includes reverse-proxy base paths", async () => {
  const { privateKey, publicKey } = deterministicKeyPair(9);
  const accountId = "operator-1@mibank.paynet";
  const timestampMs = 1_717_171_717_002;
  const nonce = "torii-prefix-nonce";
  let signerInput = null;

  const request = await buildCanonicalJsonRequest({
    accountId,
    networkId: TEST_NETWORK_ID,
    method: "post",
    baseUrl: "https://explorer.example/torii/",
    path: "/v1/aliases/resolve?alias_scope=paynet",
    body: { alias: "tidal-river-4160@mibank.paynet" },
    timestampMs,
    nonce,
    sign: async (input) => {
      signerInput = input;
      return signEd25519(input.message, privateKey);
    },
  });

  assert.ok(signerInput);
  assert.equal(signerInput.path, "/torii/v1/aliases/resolve");
  assert.equal(signerInput.query, "alias_scope=paynet");

  const message = canonicalRequestSignatureMessage({
    networkId: TEST_NETWORK_ID,
    method: request.method,
    path: "/torii/v1/aliases/resolve",
    query: "alias_scope=paynet",
    body: request.body,
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(request.headers["X-Iroha-Signature"], "base64");
  assert.equal(verifyEd25519(message, signature, publicKey), true);
});

test("canonical request signing: explicit query overrides query strings in paths", async () => {
  const { privateKey } = deterministicKeyPair(10);
  let signerInput = null;

  await buildCanonicalJsonRequest({
    accountId: "operator@paynet",
    networkId: TEST_NETWORK_ID,
    baseUrl: "https://explorer.example/torii",
    path: "/v1/aliases/resolve?ignored=1",
    query: "alias_scope=paynet",
    body: { alias: "banking@paynet" },
    sign: (input) => {
      signerInput = input;
      return signEd25519(input.message, privateKey);
    },
  });

  assert.ok(signerInput);
  assert.equal(signerInput.path, "/torii/v1/aliases/resolve");
  assert.equal(signerInput.query, "alias_scope=paynet");
});
