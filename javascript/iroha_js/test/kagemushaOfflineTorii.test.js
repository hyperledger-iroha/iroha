import { test } from "node:test";
import assert from "node:assert/strict";

import * as sdk from "../src/index.js";
import * as distSdk from "../dist/index.js";
import { ToriiClient } from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import {
  KAGEMUSHA_CASH_HANDOFF_CAPABILITY,
  KAGEMUSHA_MANIFEST_VERSION,
  KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES,
  KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION,
  KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES,
  normalizeOfflineStatus,
  normalizeKagemushaOperationStatus,
  normalizeKagemushaTopUpRequestV4,
} from "../src/kagemushaOffline.js";

const OPERATION_ID = "11".repeat(32);
const TRANSACTION_HASH = "22".repeat(32);

function jsonResponse(payload, { status = 200, headers = {} } = {}) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json", ...headers },
  });
}

function universalCapability(overrides = {}) {
  return {
    cash_handoff_capability: "cash_handoff_v1",
    required_bridge_abi_version: 23,
    max_hops: 8,
    ready: true,
    ...overrides,
  };
}

function noritoArchive() {
  const archive = new Uint8Array(40);
  archive.set([0x4e, 0x52, 0x54, 0x30]);
  return archive;
}

function requestV4() {
  return { version: 4, operationId: OPERATION_ID, norito: noritoArchive() };
}

function operationReference(kind) {
  return {
    operation_id: OPERATION_ID,
    kind: { kind, value: null },
    state: { state: "pending", value: null },
    transaction_hash: TRANSACTION_HASH,
    status_uri: `/v1/offline/operations/${OPERATION_ID}`,
    submitted_at_ms: 1234,
  };
}

test("Kagemusha JavaScript surface is transport-only ABI-23/V4", () => {
  assert.equal(KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION, 23);
  assert.equal(KAGEMUSHA_CASH_HANDOFF_CAPABILITY, "cash_handoff_v1");
  assert.equal(KAGEMUSHA_MANIFEST_VERSION, 4);
  assert.equal(KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES, 512 * 1024);
  assert.equal(KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES, 48 * 1024 * 1024);
  assert.equal(distSdk.KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION, 23);
  assert.equal(typeof sdk.ToriiClient.prototype.getOfflineCapability, "function");
  assert.equal(typeof distSdk.ToriiClient.prototype.getOfflineCapability, "function");
  for (const Client of [ToriiClient, ToriiBrowserClient]) {
    assert.equal(Client.prototype.getKagemushaReadinessV4, undefined);
  }
  for (const publicSurface of [sdk, distSdk]) {
    assert.equal(publicSurface.normalizeKagemushaAssetSelector, undefined);
    assert.equal(publicSurface.normalizeKagemushaReadinessV4, undefined);
  }
  assert.equal(
    Object.keys(sdk).some((name) => /kagemusha.*prover/iu.test(name)),
    false,
  );
  assert.equal(
    Object.keys(distSdk).some((name) => /kagemusha.*prover/iu.test(name)),
    false,
  );

  assert.throws(
    () => normalizeOfflineStatus(universalCapability({ required_bridge_abi_version: 19 })),
    /required_bridge_abi_version must be 23/u,
  );
  assert.throws(
    () => normalizeOfflineStatus(universalCapability({ mandatory: false })),
    /missing or unknown fields/u,
  );
  assert.throws(
    () => normalizeOfflineStatus(universalCapability({ assets: [] })),
    /missing or unknown fields/u,
  );
  assert.throws(
    () => normalizeOfflineStatus(universalCapability({ blockers: [] })),
    /missing or unknown fields/u,
  );
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4({ ...requestV4(), version: 3 }),
    /version must be 4; V3 archives are not upgraded/u,
  );
});

test("ToriiClient preserves all four Kagemusha routes and V4 request headers", async () => {
  const observed = [];
  const responses = [
    jsonResponse(universalCapability()),
    jsonResponse(operationReference("top_up"), {
      status: 202,
      headers: { location: `/v1/offline/operations/${OPERATION_ID}` },
    }),
    jsonResponse(operationReference("redeem"), {
      status: 202,
      headers: { location: `/v1/offline/operations/${OPERATION_ID}` },
    }),
    jsonResponse({
      state: "applied",
      value: {
        operation_id: OPERATION_ID,
        result: {
          kind: "redeem",
          result: {
            transaction_hash: TRANSACTION_HASH,
            finalized_block_height: 42,
            server_time_ms: 1234,
          },
        },
      },
    }),
  ];
  const client = new ToriiClient("https://torii.example", {
    fetchImpl: async (url, init) => {
      observed.push({ url: new URL(url), init });
      return responses.shift();
    },
    maxRetries: 0,
  });

  const capability = await client.getOfflineCapability();
  const topUp = await client.submitKagemushaTopUpV4(requestV4());
  const redeem = await client.submitKagemushaRedeemV4(requestV4());
  const status = await client.getKagemushaOperationStatus(OPERATION_ID);

  assert.deepEqual(capability, universalCapability());
  assert.equal(topUp.kind.kind, "top_up");
  assert.equal(redeem.kind.kind, "redeem");
  assert.equal(status.state, "applied");
  assert.equal(status.value.result.kind, "redeem");
  assert.deepEqual(
    observed.map(({ url }) => url.pathname),
    [
      "/v1/offline/readiness",
      "/v1/offline/top-up",
      "/v1/offline/redeem",
      `/v1/offline/operations/${OPERATION_ID}`,
    ],
  );
  assert.equal(observed[0].url.search, "");
  for (const { init } of observed.slice(1, 3)) {
    const headers = new Headers(init.headers);
    assert.equal(headers.get("content-type"), "application/x-norito");
    assert.equal(headers.get("idempotency-key"), OPERATION_ID);
    assert.deepEqual([...new Uint8Array(init.body)], [...noritoArchive()]);
  }
});

test("ToriiBrowserClient exposes the same transport-only Kagemusha contract", async () => {
  const observed = [];
  const responses = [
    jsonResponse(universalCapability()),
    jsonResponse(operationReference("top_up"), {
      status: 202,
      headers: { location: `/v1/offline/operations/${OPERATION_ID}` },
    }),
    jsonResponse({
      state: "pending",
      value: {
        operation_id: OPERATION_ID,
        kind: { kind: "top_up", value: null },
        transaction_hash: TRANSACTION_HASH,
        submitted_at_ms: 1234,
      },
    }),
  ];
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async (url, init) => {
      observed.push({ url: new URL(url), init });
      return responses.shift();
    },
  });

  const capability = await client.getOfflineCapability();
  const reference = await client.submitKagemushaTopUpV4(requestV4());
  const status = await client.getKagemushaOperationStatus(OPERATION_ID);

  assert.equal(capability.ready, true);
  assert.equal(observed[0].url.search, "");
  assert.equal(reference.state.state, "pending");
  assert.equal(status.state, "pending");
  assert.deepEqual(
    observed.map(({ url }) => url.pathname),
    [
      "/v1/offline/readiness",
      "/v1/offline/top-up",
      `/v1/offline/operations/${OPERATION_ID}`,
    ],
  );
});

test("operation parsing rejects a V3 top-up anchor instead of upgrading it", async () => {
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => jsonResponse({
      state: "applied",
      value: {
        operation_id: OPERATION_ID,
        result: {
          kind: "top_up",
          result: {
            transaction_hash: TRANSACTION_HASH,
            finalized_block_height: 42,
            server_time_ms: 1234,
            anchor: { version: 3, artifact_binding: { version: 4 } },
            finality_proof: {},
          },
        },
      },
    }),
  });

  await assert.rejects(
    () => client.getKagemushaOperationStatus(OPERATION_ID),
    /anchor and artifact binding must use V4/u,
  );
});

test("rejected operation parsing requires the exact error envelope", () => {
  const rejected = {
    state: "rejected",
    value: {
      operation_id: OPERATION_ID,
      kind: { kind: "redeem", value: null },
      transaction_hash: TRANSACTION_HASH,
      error: {
        code: "offline_operation_rejected",
        message: "rejected",
      },
    },
  };
  for (const normalize of [
    normalizeKagemushaOperationStatus,
    distSdk.normalizeKagemushaOperationStatus,
  ]) {
    assert.equal(
      normalize(rejected, OPERATION_ID).value.error.code,
      "offline_operation_rejected",
    );
    assert.throws(
      () => normalize({
        ...rejected,
        value: {
          ...rejected.value,
          error: { ...rejected.value.error, retryable: true },
        },
      }, OPERATION_ID),
      /error contains missing or unknown fields/u,
    );
    assert.throws(
      () => normalize({
        ...rejected,
        value: {
          ...rejected.value,
          error: { ...rejected.value.error, details: null },
        },
      }, OPERATION_ID),
      /error\.details must be an object/u,
    );
  }
});
