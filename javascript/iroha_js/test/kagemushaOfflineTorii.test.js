import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

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
  normalizeKagemushaOperationReference,
  normalizeKagemushaOperationStatus,
  normalizeKagemushaRedeemRequestV4,
  normalizeKagemushaTopUpRequestV4,
} from "../src/kagemushaOffline.js";
import { crc64Xz } from "../src/crc64Xz.js";

const OPERATION_ID = "11".repeat(32);
const TRANSACTION_HASH = "23".repeat(32);
const TOP_UP_SCHEMA_NAME = "iroha.torii.v1.offline.top_up.request";
const REDEEM_SCHEMA_NAME = "iroha.torii.v1.offline.redeem.request";

function jsonResponse(payload, { status = 200, headers = {} } = {}) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json", ...headers },
  });
}

function rawJsonResponse(payload, { status = 200, headers = {} } = {}) {
  return new Response(payload, {
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

function noritoArchive(schemaName = TOP_UP_SCHEMA_NAME) {
  const payload = Buffer.from([0x01]);
  const archive = Buffer.alloc(48 + payload.length);
  archive.write("NRT0", 0, "ascii");
  createHash("sha256")
    .update(Buffer.from("norito:v1:type-name\0", "utf8"))
    .update(Buffer.from(schemaName, "utf8"))
    .digest()
    .copy(archive, 6, 0, 16);
  archive.writeBigUInt64LE(BigInt(payload.length), 23);
  archive.writeBigUInt64LE(crc64Xz(payload), 31);
  archive[39] = 0x02;
  payload.copy(archive, 48);
  return archive;
}

function requestV4(schemaName = TOP_UP_SCHEMA_NAME) {
  return { version: 4, operationId: OPERATION_ID, norito: noritoArchive(schemaName) };
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

test("Kagemusha requests require an exact schema-bound Norito frame", () => {
  assert.equal(normalizeKagemushaTopUpRequestV4(requestV4()).norito.length, 49);
  assert.equal(
    normalizeKagemushaRedeemRequestV4(requestV4(REDEEM_SCHEMA_NAME)).norito.length,
    49,
  );

  const wrongSchema = requestV4();
  assert.throws(
    () => normalizeKagemushaRedeemRequestV4(wrongSchema),
    /schema hash did not match/u,
  );

  const badChecksum = requestV4();
  badChecksum.norito[48] ^= 0xff;
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4(badChecksum),
    /CRC64 mismatch/u,
  );

  const withoutAlignmentPadding = requestV4();
  withoutAlignmentPadding.norito = Buffer.concat([
    withoutAlignmentPadding.norito.subarray(0, 40),
    withoutAlignmentPadding.norito.subarray(48),
  ]);
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4(withoutAlignmentPadding),
    /exactly 8 bytes of header padding/u,
  );

  const alternateFlags = requestV4();
  alternateFlags.norito[39] = 0;
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4(alternateFlags),
    /canonical compact-length layout flags/u,
  );
});

test("ToriiClient preserves all four Kagemusha routes and V4 request headers", async () => {
  const observed = [];
  const responses = [
    jsonResponse(universalCapability()),
    jsonResponse(operationReference("top_up"), {
      status: 202,
      headers: {
        location: `/v1/offline/operations/${OPERATION_ID}`,
        "retry-after": "1",
      },
    }),
    jsonResponse(operationReference("redeem"), {
      status: 202,
      headers: {
        location: `/v1/offline/operations/${OPERATION_ID}`,
        "retry-after": "1",
      },
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
  const redeem = await client.submitKagemushaRedeemV4(requestV4(REDEEM_SCHEMA_NAME));
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
  assert.deepEqual(observed.map(({ init }) => init.redirect), [
    "error",
    "error",
    "error",
    "error",
  ]);
  const submittedArchives = [
    noritoArchive(TOP_UP_SCHEMA_NAME),
    noritoArchive(REDEEM_SCHEMA_NAME),
  ];
  for (const [{ init }, expectedArchive] of observed
    .slice(1, 3)
    .map((entry, index) => [entry, submittedArchives[index]])) {
    const headers = new Headers(init.headers);
    assert.equal(headers.get("content-type"), "application/x-norito");
    assert.equal(headers.get("idempotency-key"), OPERATION_ID);
    assert.equal(init.redirect, "error");
    assert.deepEqual([...new Uint8Array(init.body)], [...expectedArchive]);
  }
});

test("ToriiBrowserClient exposes the same transport-only Kagemusha contract", async () => {
  const observed = [];
  const responses = [
    jsonResponse(universalCapability()),
    jsonResponse(operationReference("top_up"), {
      status: 202,
      headers: {
        location: `/v1/offline/operations/${OPERATION_ID}`,
        "retry-after": "1",
      },
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
  assert.deepEqual(observed.map(({ init }) => init.redirect), [
    "error",
    "error",
    "error",
  ]);
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

test("Kagemusha clients reject duplicate and oversized JSON responses", async () => {
  const duplicateCapability =
    '{"cash_handoff_capability":"cash_handoff_v1",' +
    '"required_bridge_abi_version":23,"max_hops":8,"ready":true,"ready":true}';
  const oversizedCapability = JSON.stringify({
    ...universalCapability(),
    padding: "x".repeat(256 * 1024),
  });

  for (const createClient of [
    (response) => new ToriiClient("https://torii.example", {
      fetchImpl: async () => response,
      maxRetries: 0,
    }),
    (response) => new ToriiBrowserClient("https://torii.example", {
      fetchImpl: async () => response,
    }),
  ]) {
    await assert.rejects(
      () => createClient(rawJsonResponse(duplicateCapability)).getOfflineCapability(),
      /duplicate object key "ready"/u,
    );
    await assert.rejects(
      () => createClient(rawJsonResponse(oversizedCapability)).getOfflineCapability(),
      /262144-byte response (?:limit|size bound)/u,
    );
  }
});

test("operation references require Torii's positive Retry-After header", () => {
  for (const retryAfter of [
    null,
    "0",
    "soon",
    "18446744073709551616",
    "9".repeat(10_000),
  ]) {
    assert.throws(
      () => normalizeKagemushaOperationReference(operationReference("top_up"), {
        expectedOperationId: OPERATION_ID,
        expectedKind: "top_up",
        location: `/v1/offline/operations/${OPERATION_ID}`,
        retryAfter,
      }),
      /Retry-After must be a positive u64/u,
    );
  }
});

test("pending operation timestamps must be positive", () => {
  assert.throws(
    () => normalizeKagemushaOperationReference({
      ...operationReference("top_up"),
      submitted_at_ms: 0,
    }, {
      expectedOperationId: OPERATION_ID,
      expectedKind: "top_up",
      location: `/v1/offline/operations/${OPERATION_ID}`,
      retryAfter: "1",
    }),
    /submitted_at_ms must be a positive safe unsigned integer/u,
  );
  assert.throws(
    () => normalizeKagemushaOperationStatus({
      state: "pending",
      value: {
        operation_id: OPERATION_ID,
        kind: { kind: "top_up", value: null },
        transaction_hash: TRANSACTION_HASH,
        submitted_at_ms: 0,
      },
    }, OPERATION_ID),
    /submitted_at_ms must be a positive safe unsigned integer/u,
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
    assert.equal(
      normalize({
        ...rejected,
        value: {
          ...rejected.value,
          error: { ...rejected.value.error, message: "😀".repeat(1024) },
        },
      }, OPERATION_ID).value.error.message,
      "😀".repeat(1024),
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
    assert.throws(
      () => normalize({
        state: "pending",
        value: {
          operation_id: OPERATION_ID,
          kind: { kind: "redeem", value: null },
          transaction_hash: "22".repeat(32),
          submitted_at_ms: 1234,
        },
      }, OPERATION_ID),
      /canonical lowercase 32-byte Iroha hash/u,
    );
    for (const code of ["INVALID-CODE", "_private", `a${"b".repeat(64)}`]) {
      assert.throws(
        () => normalize({
          ...rejected,
          value: {
            ...rejected.value,
            error: { ...rejected.value.error, code },
          },
        }, OPERATION_ID),
        /(?:stable lowercase error code|exact non-empty text)/u,
      );
    }
    assert.throws(
      () => normalize({
        ...rejected,
        value: {
          ...rejected.value,
          error: { ...rejected.value.error, message: "control\u0085text" },
        },
      }, OPERATION_ID),
      /exact non-empty text/u,
    );
    assert.throws(
      () => normalize({
        ...rejected,
        value: {
          ...rejected.value,
          error: { ...rejected.value.error, message: "x".repeat(1025) },
        },
      }, OPERATION_ID),
      /exact non-empty text/u,
    );
  }
});

test("applied operation parsing rejects zero finality fields", () => {
  const applied = {
    state: "applied",
    value: {
      operation_id: OPERATION_ID,
      result: {
        kind: "redeem",
        result: {
          transaction_hash: TRANSACTION_HASH,
          finalized_block_height: 1,
          server_time_ms: 1,
        },
      },
    },
  };
  for (const field of ["finalized_block_height", "server_time_ms"]) {
    assert.throws(
      () => normalizeKagemushaOperationStatus({
        ...applied,
        value: {
          ...applied.value,
          result: {
            ...applied.value.result,
            result: { ...applied.value.result.result, [field]: 0 },
          },
        },
      }, OPERATION_ID),
      new RegExp(`${field} must be a positive safe unsigned integer`, "u"),
    );
  }
});
