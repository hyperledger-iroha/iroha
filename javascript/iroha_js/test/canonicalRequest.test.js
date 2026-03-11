"use strict";

import test from "node:test";
import assert from "node:assert/strict";

import {
  canonicalQueryString,
  canonicalRequestMessage,
  buildCanonicalRequestHeaders,
  generateKeyPair,
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
  const accountId = AccountAddress.fromAccount({
    domain: "wonderland",
    publicKey,
  }).toI105();
  const body = Buffer.from('{"foo":1}');
  const path = `/v1/accounts/${accountId}/assets`;
  const message = canonicalRequestMessage({
    method: "get",
    path,
    query: "limit=10",
    body,
  });
  const headers = buildCanonicalRequestHeaders({
    accountId,
    method: "get",
    path,
    query: "limit=10",
    body,
    privateKey,
  });
  const signature = Buffer.from(headers["X-Iroha-Signature"], "base64");
  assert.equal(verifyEd25519(message, signature, publicKey), true);
});
