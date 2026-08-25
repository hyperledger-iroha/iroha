import assert from "node:assert/strict";
import { test } from "node:test";

import { NetworkId, ToriiClient } from "../src/index.js";
import { makeTestOperatorSigningContext } from "./toriiClientTestHelpers.js";


const NETWORK_ID = NetworkId.parse(
  "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149",
);

test("Connect aggregate status requires operator context before dispatch", async () => {
  let dispatched = false;
  const client = new ToriiClient("https://torii.example", {
    fetchImpl: async () => {
      dispatched = true;
      throw new Error("unexpected dispatch");
    },
  });

  await assert.rejects(
    () => client.getConnectStatus(),
    /requires an immutable OperatorSigningContext/,
  );
  assert.equal(dispatched, false);
});

test("Connect aggregate status signs the exact one-shot target", async () => {
  const calls = [];
  const client = new ToriiClient("https://torii.example", {
    operatorSigningContext: makeTestOperatorSigningContext(NETWORK_ID),
    retry: { maxRetries: 9, retryOnMethods: ["GET"], retryOnStatus: [503] },
    fetchImpl: async (url, init) => {
      calls.push({ url: String(url), init });
      return new Response(JSON.stringify({ enabled: true }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });

  assert.deepEqual(await client.getConnectStatus(), {
    enabled: true,
    sessionsTotal: 0,
    sessionsActive: 0,
    perIpSessions: [],
    bufferedSessions: 0,
    totalBufferBytes: 0,
    dedupeSize: 0,
    framesInTotal: 0,
    framesOutTotal: 0,
    ciphertextTotal: 0,
    dedupeDropsTotal: 0,
    bufferDropsTotal: 0,
    plaintextControlDropsTotal: 0,
    monotonicDropsTotal: 0,
    sequenceViolationClosesTotal: 0,
    roleDirectionMismatchTotal: 0,
    pingMissTotal: 0,
    p2pRebroadcastsTotal: 0,
    p2pRebroadcastSkippedTotal: 0,
    p2pAuthFailuresTotal: 0,
    p2pTtlDropsTotal: 0,
    p2pUnknownSessionDropsTotal: 0,
    p2pSessionClaimsInTotal: 0,
    p2pSessionClaimsInstalledTotal: 0,
    p2pSessionClaimConflictsTotal: 0,
    p2pRoleConsumedTotal: 0,
    p2pSessionTerminatedTotal: 0,
    policy: null,
  });
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, "https://torii.example/v1/connect/status/aggregate");
  assert.equal(calls[0].init.method, "GET");
  assert.equal(calls[0].init.redirect, "error");
  const headers = new Headers(calls[0].init.headers);
  assert.equal(headers.has("x-iroha-operator-public-key"), true);
  assert.equal(headers.has("x-iroha-operator-timestamp-ms"), true);
  assert.equal(headers.has("x-iroha-operator-nonce"), true);
  assert.equal(headers.has("x-iroha-operator-signature"), true);
});
