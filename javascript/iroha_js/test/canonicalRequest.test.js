"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import {
  canonicalQueryString,
  canonicalRequestMessage,
  canonicalRequestSignatureMessage,
  buildCanonicalRequestHeaders,
  buildCanonicalJsonRequest,
  generateKeyPair,
  signEd25519,
  verifyEd25519,
} from "../src/index.js";
import { AccountAddress } from "../src/address.js";

test("canonical request signing: canonical query sorts pairs", () => {
  const rendered = canonicalQueryString("b=2&a=3&b=1&space=a+b");
  assert.equal(rendered, "a=3&b=1&b=2&space=a+b");
});

test("canonical request signing: canonical query uses form encoding", () => {
  const rendered = canonicalQueryString("b=!*()~'&a=1");
  assert.equal(rendered, "a=1&b=%21*%28%29%7E%27");
});

test("canonical request signing: headers include a verifiable signature", () => {
  const { privateKey, publicKey } = generateKeyPair({
    seed: Buffer.alloc(32, 7),
  });
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105();
  const body = Buffer.from('{"foo":1}');
  const path = `/v1/accounts/${accountId}/assets`;
  const timestampMs = 1_717_171_717_000;
  const nonce = "deterministic-nonce";
  const message = canonicalRequestSignatureMessage({
    method: "get",
    path,
    query: "limit=10",
    body,
    timestampMs,
    nonce,
  });
  const headers = buildCanonicalRequestHeaders({
    accountId,
    method: "get",
    path,
    query: "limit=10",
    body,
    privateKey,
    timestampMs,
    nonce,
  });
  const signature = Buffer.from(headers["X-Iroha-Signature"], "base64");
  assert.equal(headers["X-Iroha-Timestamp-Ms"], String(timestampMs));
  assert.equal(headers["X-Iroha-Nonce"], nonce);
  assert.equal(verifyEd25519(message, signature, publicKey), true);
});

test("canonical request signing: JSON helper signs the exact request body with callback signers", async () => {
  const { privateKey, publicKey } = generateKeyPair({
    seed: Buffer.alloc(32, 8),
  });
  const accountId = AccountAddress.fromAccount({ publicKey }).toI105();
  const path = "/v1/aliases/resolve?b=2&a=1";
  const timestampMs = 1_717_171_717_001;
  const nonce = "browser-keystore-nonce";
  let signerInput = null;

  const request = await buildCanonicalJsonRequest({
    accountId,
    method: "post",
    path,
    body: { alias: "tidal-river-4160@mibank.bpng" },
    headers: { "X-Request-Id": "req_1" },
    timestampMs,
    nonce,
    sign: async (input) => {
      signerInput = input;
      return signEd25519(input.message, privateKey).toString("base64");
    },
  });

  assert.equal(request.method, "POST");
  assert.equal(request.body, '{"alias":"tidal-river-4160@mibank.bpng"}');
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
