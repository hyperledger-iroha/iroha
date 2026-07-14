import { test } from "node:test";
import assert from "node:assert/strict";

import * as sdk from "../src/index.js";
import * as distSdk from "../dist/index.js";
import { ToriiClient } from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import {
  KAGEMUSHA_MANIFEST_VERSION,
  KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES,
  KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION,
  KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES,
  normalizeKagemushaReadinessV4,
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

function unavailableReadiness(abiVersion = 20) {
  return {
    required_bridge_abi_version: abiVersion,
    max_hops: 8,
    asset_definition_id: "coin#wonderland",
    asset_scale: null,
    evaluated_block_height: 7,
    evaluated_block_hash: "aa".repeat(32),
    active_transfer_verifier: null,
    active_topup_shield_verifier: null,
    active_unshield_verifier: null,
    active_recursive_step_eq_verifier: null,
    active_recursive_step_ep_verifier: null,
    artifact_set: null,
    proof_backend_available: false,
    recursive_lineage_supported: false,
    ready: false,
    blockers: [
      { code: "recursive_v4_registry_unavailable", message: "not provisioned" },
    ],
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

test("Kagemusha JavaScript surface is transport-only ABI-20/V4", () => {
  assert.equal(KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION, 20);
  assert.equal(KAGEMUSHA_MANIFEST_VERSION, 4);
  assert.equal(KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES, 512 * 1024);
  assert.equal(KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES, 48 * 1024 * 1024);
  assert.equal(distSdk.KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION, 20);
  assert.equal(typeof distSdk.ToriiClient.prototype.getKagemushaReadinessV4, "function");
  assert.equal(
    Object.keys(sdk).some((name) => /kagemusha.*prover/iu.test(name)),
    false,
  );
  assert.equal(
    Object.keys(distSdk).some((name) => /kagemusha.*prover/iu.test(name)),
    false,
  );

  assert.throws(
    () => normalizeKagemushaReadinessV4(unavailableReadiness(19), "coin#wonderland"),
    /required_bridge_abi_version must be 20/u,
  );
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4({ ...requestV4(), version: 3 }),
    /version must be 4; V3 archives are not upgraded/u,
  );
});

test("ToriiClient preserves all four Kagemusha routes and V4 request headers", async () => {
  const observed = [];
  const responses = [
    jsonResponse(unavailableReadiness()),
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

  const readiness = await client.getKagemushaReadinessV4("coin#wonderland");
  const topUp = await client.submitKagemushaTopUpV4(requestV4());
  const redeem = await client.submitKagemushaRedeemV4(requestV4());
  const status = await client.getKagemushaOperationStatus(OPERATION_ID);

  assert.equal(readiness.required_bridge_abi_version, 20);
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
  assert.equal(observed[0].url.searchParams.get("asset_definition_id"), "coin#wonderland");
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
    jsonResponse(unavailableReadiness()),
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

  const readiness = await client.getKagemushaReadinessV4("coin#wonderland");
  const reference = await client.submitKagemushaTopUpV4(requestV4());
  const status = await client.getKagemushaOperationStatus(OPERATION_ID);

  assert.equal(readiness.ready, false);
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
