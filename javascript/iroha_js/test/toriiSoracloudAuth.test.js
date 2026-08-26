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
const PUBLIC_KEY =
  "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245";
const HASH =
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0";
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
const MUTATION_RESPONSE = Object.freeze({
  ok: true,
  authority: ACCOUNT_ID,
  signed_by: PUBLIC_KEY,
  tx_instructions: Object.freeze([
    Object.freeze({
wire_id: "iroha.instruction.v1::soracloud::DeploySoracloudAppInfra",
      payload_hex: "00",
    }),
  ]),
});
const STATUS_RESPONSE = Object.freeze({
  schema_version: 1,
  app_count: 1,
  audit_event_count: 0,
  apps: Object.freeze([
    Object.freeze({
      schema_version: 1,
      app_name: "hayahi",
      current_app_version: "1.0.0",
      current_manifest_hash: HASH,
      revision_count: 1,
      deployed_sequence: 1,
      updated_sequence: 1,
      manifest: Object.freeze({
        schema_version: 1,
        app_name: "hayahi",
        app_version: "1.0.0",
        public_url: "https://hayahi.example",
        static_site: null,
        services: Object.freeze([
          Object.freeze({
            schema_version: 1,
            service_name: "api",
            service_version: "1.0.0",
            service_manifest_hash: HASH,
            container_manifest_hash: HASH,
            execution_plane: Object.freeze({
              execution_plane: "HttpService",
              value: null,
            }),
            runtime: Object.freeze({ runtime: "Inrou", value: null }),
            routes: Object.freeze([]),
            lease_volumes: Object.freeze([]),
            shard: null,
          }),
        ]),
      }),
    }),
  ]),
  recent_audit_events: Object.freeze([]),
});

test("Soracloud app infra mutations require one-shot canonical auth", async () => {
  const calls = [];
  const client = new ToriiClient("https://torii.example", {
    fetchImpl: async (url, init) => {
      calls.push({ url: new URL(url), init });
      return new Response(JSON.stringify(MUTATION_RESPONSE), {
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

  assert.deepEqual(
    await client.deploySoracloudAppInfra(REQUEST, OPTIONS),
    MUTATION_RESPONSE,
  );
  assert.deepEqual(
    await client.upgradeSoracloudAppInfra(REQUEST, OPTIONS),
    MUTATION_RESPONSE,
  );
  assert.deepEqual(calls.map(({ url }) => url.pathname), [
    "/v1/soracloud/apps/deploy",
    "/v1/soracloud/apps/upgrade",
  ]);
  for (const { init } of calls) {
    assert.equal(init.redirect, "error");
    assert.deepEqual(JSON.parse(init.body), REQUEST);
    assert.equal(
      init.headers["X-Iroha-Account"],
      AccountAddress.parseEncoded(ACCOUNT_ID).address.canonicalHex(),
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

test("Soracloud mutation success requires an exact nonempty V1 draft", async () => {
  const invalidResponses = [
    null,
    { ...MUTATION_RESPONSE, legacy: true },
    { ...MUTATION_RESPONSE, ok: false },
    { ...MUTATION_RESPONSE, tx_instructions: [] },
    {
      ...MUTATION_RESPONSE,
      tx_instructions: [{ ...MUTATION_RESPONSE.tx_instructions[0], payload_hex: "0A" }],
    },
  ];
  for (const [index, payload] of invalidResponses.entries()) {
    const client = new ToriiClient("https://torii.example", {
      fetchImpl: async () =>
        new Response(payload === null ? null : JSON.stringify(payload), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      localSigningContext: SIGNING_CONTEXT,
    });
    await assert.rejects(
      () => client.deploySoracloudAppInfra(REQUEST, OPTIONS),
      /response/u,
      `invalid response ${index} must fail closed`,
    );
  }
});

test("Soracloud mutation success rejects non-JSON and malformed bodies", async () => {
  const responses = [
    () =>
      new Response("not-json", {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    () =>
      new Response(JSON.stringify(MUTATION_RESPONSE), {
        status: 200,
        headers: { "content-type": "text/plain" },
      }),
    () =>
      new Response("", {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    () =>
      new Response(JSON.stringify(MUTATION_RESPONSE), {
        status: 200,
        headers: { "content-type": "application/json; charset=utf-8" },
      }),
    () => {
      const headers = new Headers();
      headers.append("content-type", "application/json");
      headers.append("content-type", "application/json");
      return new Response(JSON.stringify(MUTATION_RESPONSE), {
        status: 200,
        headers,
      });
    },
  ];
  for (const [index, response] of responses.entries()) {
    const client = new ToriiClient("https://torii.example", {
      fetchImpl: async () => response(),
      localSigningContext: SIGNING_CONTEXT,
    });
    await assert.rejects(
      () => client.deploySoracloudAppInfra(REQUEST, OPTIONS),
      /JSON|response/u,
      `invalid HTTP body ${index} must fail closed`,
    );
  }
});

test("Soracloud status success requires exact JSON transport bytes", async () => {
  const responses = [
    () =>
      new Response("", {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    () =>
      new Response("{", {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    () =>
      new Response(JSON.stringify(STATUS_RESPONSE), {
        status: 200,
        headers: { "content-type": "Application/JSON" },
      }),
  ];
  for (const [index, response] of responses.entries()) {
    const client = new ToriiClient("https://torii.example", {
      fetchImpl: async () => response(),
      localSigningContext: SIGNING_CONTEXT,
    });
    await assert.rejects(
      () => client.getSoracloudNamedAppInfraStatus("hayahi", OPTIONS),
      /JSON|response|Content-Type/u,
      `invalid status HTTP body ${index} must fail closed`,
    );
  }
});

test("Soracloud status reads require exact path/query account signatures", async () => {
  const calls = [];
  const client = new ToriiClient("https://torii.example", {
    fetchImpl: async (url, init) => {
      calls.push({ url: new URL(url), init });
      return new Response(JSON.stringify(STATUS_RESPONSE), {
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

  assert.deepEqual(await client.getSoracloudAppInfraStatus({
    ...OPTIONS,
    appName: "hayahi",
    auditLimit: 3,
  }), STATUS_RESPONSE);
  assert.deepEqual(await client.getSoracloudNamedAppInfraStatus("hayahi", {
    ...OPTIONS,
    auditLimit: 2,
  }), STATUS_RESPONSE);
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

test("Soracloud status success requires an exact identity-bound V1 snapshot", async () => {
  const invalidResponses = [
    null,
    { ...STATUS_RESPONSE, legacy: true },
    { ...STATUS_RESPONSE, app_count: 0 },
    { ...STATUS_RESPONSE, apps: [] },
    {
      ...STATUS_RESPONSE,
      apps: [{ ...STATUS_RESPONSE.apps[0], app_name: "foreign" }],
    },
    {
      ...STATUS_RESPONSE,
      apps: [{
        ...STATUS_RESPONSE.apps[0],
        manifest: { ...STATUS_RESPONSE.apps[0].manifest, legacy: true },
      }],
    },
  ];
  for (const [index, payload] of invalidResponses.entries()) {
    const client = new ToriiClient("https://torii.example", {
      fetchImpl: async () =>
        new Response(payload === null ? null : JSON.stringify(payload), {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      localSigningContext: SIGNING_CONTEXT,
    });
    await assert.rejects(
      () => client.getSoracloudNamedAppInfraStatus("hayahi", OPTIONS),
      /response/u,
      `invalid response ${index} must fail closed`,
    );
  }
});
