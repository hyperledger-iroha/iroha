import assert from "node:assert/strict";
import test from "node:test";

import { AccountAddress } from "../src/address.js";
import { signEd25519 } from "../src/crypto.js";
import { NetworkId } from "../src/networkId.js";
import { LocalSigningContext, ToriiClient } from "../src/toriiClient.js";
import { canonicalRequestSignatureMessage } from "../src/canonicalRequest.js";

const ACCOUNT_ID = AccountAddress.fromAccount({
  publicKey: Buffer.from(
    "EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245",
    "hex",
  ),
}).toI105();
const SIGNING_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const SIGNING_CONTEXT = new LocalSigningContext(SIGNING_NETWORK_ID);
const OPTIONS = Object.freeze({
  canonicalAuth: {
    accountId: ACCOUNT_ID,
    privateKey: Buffer.alloc(32, 0x31),
  },
});
const REQUEST = Object.freeze({
  manifest: { app_name: "hayahi" },
  provenance: { signer: "signer", signature: "ABCD" },
});

test("Soracloud app infra mutations require one-shot canonical auth", async () => {
  const calls = [];
  const client = new ToriiClient("https://torii.example", {
    fetchImpl: async (url, init) => {
      calls.push({ url: new URL(url), init });
      return new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
    localSigningContext: SIGNING_CONTEXT,
    maxRetries: 8,
  });

  await assert.rejects(
    () => client.deploySoracloudAppInfra(REQUEST),
    /canonicalAuth is required/u,
  );
  assert.equal(calls.length, 0);

  assert.deepEqual(await client.deploySoracloudAppInfra(REQUEST, OPTIONS), { ok: true });
  assert.deepEqual(await client.upgradeSoracloudAppInfra(REQUEST, OPTIONS), { ok: true });
  assert.deepEqual(calls.map(({ url }) => url.pathname), [
    "/v1/soracloud/apps/deploy",
    "/v1/soracloud/apps/upgrade",
  ]);
  for (const { init } of calls) {
    assert.equal(init.redirect, "error");
    assert.deepEqual(JSON.parse(init.body), REQUEST);
    assert.equal(
      Buffer.from(init.headers["X-Iroha-Account"], "latin1").toString("utf8"),
      ACCOUNT_ID,
    );
  }
});

test("Soracloud canonical mutation transport failures are not replayed", async () => {
  let calls = 0;
  const client = new ToriiClient("https://torii.example", {
    fetchImpl: async () => {
      calls += 1;
      throw new Error("ambiguous Soracloud transport failure");
    },
    localSigningContext: SIGNING_CONTEXT,
    maxRetries: 8,
  });

  await assert.rejects(
    () => client.deploySoracloudAppInfra(REQUEST, OPTIONS),
    /ambiguous Soracloud transport failure/u,
  );
  assert.equal(calls, 1);
});

test("Soracloud status reads require exact path/query account signatures", async () => {
  const calls = [];
  const client = new ToriiClient("https://torii.example", {
    fetchImpl: async (url, init) => {
      calls.push({ url: new URL(url), init });
      return new Response(JSON.stringify({ app_count: 1 }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
    localSigningContext: SIGNING_CONTEXT,
    maxRetries: 8,
  });

  await assert.rejects(
    () => client.getSoracloudAppInfraStatus(),
    /canonicalAuth is required/u,
  );
  assert.equal(calls.length, 0);

  await client.getSoracloudAppInfraStatus({
    ...OPTIONS,
    appName: "hayahi",
    auditLimit: 3,
  });
  await client.getSoracloudNamedAppInfraStatus("hayahi", {
    ...OPTIONS,
    auditLimit: 2,
  });
  assert.deepEqual(calls.map(({ url }) => `${url.pathname}${url.search}`), [
    "/v1/soracloud/apps/status?app_name=hayahi&audit_limit=3",
    "/v1/soracloud/apps/hayahi/status?audit_limit=2",
  ]);

  for (const { url, init } of calls) {
    assert.equal(init.method, "GET");
    assert.equal(init.redirect, "error");
    const signature = Buffer.from(init.headers["X-Iroha-Signature"], "base64");
    const exactMessage = canonicalRequestSignatureMessage({
      networkId: SIGNING_NETWORK_ID,
      method: "GET",
      path: url.pathname,
      query: url.searchParams,
      body: "",
      timestampMs: Number(init.headers["X-Iroha-Timestamp-Ms"]),
      nonce: init.headers["X-Iroha-Nonce"],
    });
    assert.deepEqual(signature, signEd25519(exactMessage, OPTIONS.canonicalAuth.privateKey));

    const foreignMessage = canonicalRequestSignatureMessage({
      networkId: NetworkId.fromBytes(Buffer.alloc(32, 0xa7)),
      method: "GET",
      path: `${url.pathname}/wrong`,
      query: url.searchParams,
      body: "wrong",
      timestampMs: Number(init.headers["X-Iroha-Timestamp-Ms"]),
      nonce: init.headers["X-Iroha-Nonce"],
    });
    assert.notDeepEqual(signature, signEd25519(foreignMessage, OPTIONS.canonicalAuth.privateKey));
  }
});

test("Soracloud canonical status transport failures are not replayed", async () => {
  let calls = 0;
  const client = new ToriiClient("https://torii.example", {
    fetchImpl: async () => {
      calls += 1;
      throw new Error("ambiguous Soracloud status failure");
    },
    localSigningContext: SIGNING_CONTEXT,
    maxRetries: 8,
  });

  await assert.rejects(
    () => client.getSoracloudAppInfraStatus(OPTIONS),
    /ambiguous Soracloud status failure/u,
  );
  assert.equal(calls, 1);
});
