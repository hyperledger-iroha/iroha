import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

import * as sdk from "../src/index.js";
import * as distSdk from "../dist/index.js";
import { ToriiClient } from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import { blake2b256 } from "../src/blake2b.js";
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
import { computeHashLiteralCrc } from "../src/hashLiteralCrc.js";

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

function testIrohaHash(bytes) {
  const digest = blake2b256(bytes);
  digest[digest.length - 1] |= 1;
  return digest;
}

function testHashLiteral(bytes) {
  const body = Buffer.from(bytes).toString("hex").toUpperCase();
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}

function appliedTopUpStatus() {
  const operationId = Uint8Array.from({ length: 32 }, () => 0x11);
  const anchorDigest = Uint8Array.from({ length: 32 }, () => 0x37);
  // Independent vectors for the Rust Hash::new leaf/node/post-state formulas.
  const rightLeaf = Buffer.from(
    "15464a83b3b00ac58769c03c31db71e68728a4a68cbf93913554c5f6571192f3",
    "hex",
  );
  const topUpRoot = Buffer.from(
    "e7f5692eba6838b2af3a7bcff9193f6f412b58d8f9257ee7125238032e2785ef",
    "hex",
  );
  const ordinaryWritesRoot = Buffer.from(
    "bb589bfbd50c9bf8e3e52bfbd6a33a9ad3d410d0049cb7a5e904d1b51cbf1215",
    "hex",
  );
  const postStateRoot = Buffer.from(
    "995688156171042abd0ab32d9075ca59caa874253636c9134641e0fedcda7f27",
    "hex",
  );
  const networkId = testHashLiteral(testIrohaHash(new TextEncoder().encode("network")));
  return {
    state: "applied",
    value: {
      operation_id: OPERATION_ID,
      result: {
        kind: "top_up",
        result: {
          transaction_hash: TRANSACTION_HASH,
          finalized_block_height: 42,
          anchor: {
            version: 4,
            network_id: networkId,
            topup_operation_id: [...operationId],
            anchor_digest: [...anchorDigest],
            finalized_height: 42,
            finalized_tx_hash: Array.from({ length: 32 }, () => 0x23),
            artifact_binding: { version: 4 },
          },
          finality_proof: {
            version: 1,
            anchor: {
              topup_operation_id: [...operationId],
              anchor_digest: [...anchorDigest],
            },
            commit_qc: {
              height_context: { height: 42, network_id: networkId },
              certificate: {
                execution_commitment: {
                  post_state_root: testHashLiteral(postStateRoot),
                  ordinary_writes_root: testHashLiteral(ordinaryWritesRoot),
                  topup_anchor_root: testHashLiteral(topUpRoot),
                  topup_anchor_count: 2,
                },
              },
            },
            anchor_path: {
              leaf_index: 0,
              leaf_count: 2,
              siblings: [[...rightLeaf]],
            },
          },
        },
      },
    },
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
  const status = await client.getKagemushaOperationStatus(redeem);

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

test("ToriiClient never redispatches an ambiguous Kagemusha POST", async () => {
  for (const Client of [sdk.ToriiClient, distSdk.ToriiClient]) {
    let dispatches = 0;
    const ambiguous = Object.assign(new Error("socket closed after dispatch"), {
      code: "ECONNRESET",
    });
    const client = new Client("https://torii.example", {
      fetchImpl: async () => {
        dispatches += 1;
        throw ambiguous;
      },
      maxRetries: 3,
      retryMethods: ["POST"],
    });

    await assert.rejects(
      () => client.submitKagemushaTopUpV4(requestV4()),
      (error) => error === ambiguous,
    );
    assert.equal(dispatches, 1);
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
  const status = await client.getKagemushaOperationStatus(reference);

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

test("Kagemusha readiness responses are exact JSON bounded to 4 KiB", async () => {
  const duplicateCapability =
    '{"cash_handoff_capability":"cash_handoff_v1",' +
    '"required_bridge_abi_version":23,"max_hops":8,"ready":true,"ready":true}';
  const oversizedCapability = JSON.stringify({
    ...universalCapability(),
    padding: "x".repeat(4 * 1024),
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
      /4096-byte response (?:limit|size bound)/u,
    );
  }
});

test("Kagemusha accepted-operation references are bounded to 4 KiB", async () => {
  const oversizedReference = JSON.stringify(operationReference("top_up"));
  const responseHeaders = {
    "content-length": "4097",
    location: `/v1/offline/operations/${OPERATION_ID}`,
    "retry-after": "1",
  };
  for (const createClient of [
    () => new ToriiClient("https://torii.example", {
      fetchImpl: async () => rawJsonResponse(oversizedReference, {
        status: 202,
        headers: responseHeaders,
      }),
      maxRetries: 0,
    }),
    () => new ToriiBrowserClient("https://torii.example", {
      fetchImpl: async () => rawJsonResponse(oversizedReference, {
        status: 202,
        headers: responseHeaders,
      }),
    }),
  ]) {
    await assert.rejects(
      () => createClient().submitKagemushaTopUpV4(requestV4()),
      /exceeds (?:its |the )4096-byte response limit/u,
    );
  }
});

test("Kagemusha operation status accepts valid JSON above 256 KiB", async () => {
  const detail = "x".repeat(300 * 1024);
  const body = JSON.stringify({
    state: "rejected",
    value: {
      operation_id: OPERATION_ID,
      kind: { kind: "redeem", value: null },
      transaction_hash: TRANSACTION_HASH,
      error: {
        code: "offline_operation_rejected",
        message: "rejected",
        details: { detail },
      },
    },
  });
  assert.ok(Buffer.byteLength(body) > 256 * 1024);
  assert.ok(Buffer.byteLength(body) < 16 * 1024 * 1024);

  for (const createClient of [
    () => new ToriiClient("https://torii.example", {
      fetchImpl: async () => rawJsonResponse(body),
      maxRetries: 0,
    }),
    () => new ToriiBrowserClient("https://torii.example", {
      fetchImpl: async () => rawJsonResponse(body),
    }),
  ]) {
    const status = await createClient().getKagemushaOperationStatus(
      operationReference("redeem"),
    );
    assert.equal(status.state, "rejected");
    assert.equal(status.value.error.details.detail.length, detail.length);
  }
});

test("Kagemusha operation status responses are bounded to 16 MiB", async () => {
  for (const createClient of [
    () => new ToriiClient("https://torii.example", {
      fetchImpl: async () => rawJsonResponse("{}", {
        headers: { "content-length": String(16 * 1024 * 1024 + 1) },
      }),
      maxRetries: 0,
    }),
    () => new ToriiBrowserClient("https://torii.example", {
      fetchImpl: async () => rawJsonResponse("{}", {
        headers: { "content-length": String(16 * 1024 * 1024 + 1) },
      }),
    }),
  ]) {
    await assert.rejects(
      () => createClient().getKagemushaOperationStatus(operationReference("redeem")),
      /exceeds (?:its |the )16777216-byte response limit/u,
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

test("operation reference normalization reads accessor-backed tags exactly once", () => {
  for (const normalize of [
    normalizeKagemushaOperationReference,
    distSdk.normalizeKagemushaOperationReference,
  ]) {
    const reference = operationReference("top_up");
    let reads = 0;
    Object.defineProperty(reference.kind, "kind", {
      configurable: true,
      enumerable: true,
      get() {
        reads += 1;
        return reads === 1 ? "top_up" : "redeem";
      },
    });
    const normalized = normalize(reference);
    assert.equal(reads, 1);
    assert.equal(normalized.kind.kind, "top_up");
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
    }, operationReference("top_up")),
    /submitted_at_ms must be a positive safe unsigned integer/u,
  );
});

test("pending operation status preserves identity while allowing an exact retry attempt", () => {
  const pending = {
    state: "pending",
    value: {
      operation_id: OPERATION_ID,
      kind: { kind: "top_up", value: null },
      transaction_hash: TRANSACTION_HASH,
      submitted_at_ms: 1234,
    },
  };
  for (const normalize of [
    sdk.normalizeKagemushaOperationStatus,
    distSdk.normalizeKagemushaOperationStatus,
  ]) {
    assert.equal(
      normalize(
        pending,
        operationReference("top_up"),
      ).state,
      "pending",
    );
    const advanced = normalize(
      {
        ...pending,
        value: {
          ...pending.value,
          transaction_hash: "25".repeat(32),
          submitted_at_ms: 1235,
        },
      },
      operationReference("top_up"),
    );
    assert.equal(advanced.value.transaction_hash, "25".repeat(32));
    assert.equal(advanced.value.submitted_at_ms, 1235);
    for (const value of [
      { ...pending.value, operation_id: "21".repeat(32) },
      { ...pending.value, kind: { kind: "redeem", value: null } },
      { ...pending.value, submitted_at_ms: 1235 },
    ]) {
      assert.throws(
        () => normalize(
          { ...pending, value },
          operationReference("top_up"),
        ),
        /does not match the accepted operation reference/u,
      );
    }
  }
});

test("operation status keeps kind immutable while allowing a newer carrier hash", () => {
  const appliedRedeem = {
    state: "applied",
    value: {
      operation_id: OPERATION_ID,
      result: {
        kind: "redeem",
        result: {
          transaction_hash: TRANSACTION_HASH,
          finalized_block_height: 42,
        },
      },
    },
  };
  const rejected = {
    state: "rejected",
    value: {
      operation_id: OPERATION_ID,
      kind: { kind: "redeem", value: null },
      transaction_hash: TRANSACTION_HASH,
      error: { code: "offline_operation_rejected", message: "rejected" },
    },
  };
  for (const normalize of [
    sdk.normalizeKagemushaOperationStatus,
    distSdk.normalizeKagemushaOperationStatus,
  ]) {
    assert.throws(
      () => normalize(
        appliedRedeem,
        operationReference("top_up"),
      ),
      /does not match the accepted operation reference/u,
    );
    const appliedWinner = normalize(
      {
        ...appliedRedeem,
        value: {
          ...appliedRedeem.value,
          result: {
            ...appliedRedeem.value.result,
            result: {
              ...appliedRedeem.value.result.result,
              transaction_hash: "25".repeat(32),
            },
          },
        },
      },
      operationReference("redeem"),
    );
    assert.equal(
      appliedWinner.value.result.result.transaction_hash,
      "25".repeat(32),
    );
    assert.throws(
      () => normalize(
        rejected,
        operationReference("top_up"),
      ),
      /does not match the accepted operation reference/u,
    );
    const retriedRejection = normalize(
      {
        ...rejected,
        value: { ...rejected.value, transaction_hash: "25".repeat(32) },
      },
      operationReference("redeem"),
    );
    assert.equal(retriedRejection.value.transaction_hash, "25".repeat(32));
  }
});

test("operation status normalization snapshots its outer discriminant once", () => {
  const status = {
    state: "applied",
    value: {
      operation_id: OPERATION_ID,
      result: {
        kind: "redeem",
        result: {
          transaction_hash: TRANSACTION_HASH,
          finalized_block_height: 42,
        },
      },
    },
  };
  let reads = 0;
  Object.defineProperty(status, "state", {
    configurable: true,
    enumerable: true,
    get() {
      reads += 1;
      return reads === 1 ? "ambiguous" : "applied";
    },
  });
  assert.throws(
    () => normalizeKagemushaOperationStatus(
      status,
      operationReference("redeem"),
    ),
    /state must be pending, applied, or rejected/u,
  );
  assert.equal(reads, 1);
});

test("Kagemusha clients reject an invalid accepted reference before fetch", async () => {
  let nodeFetches = 0;
  const nodeClient = new ToriiClient("https://torii.example", {
    fetchImpl: async () => {
      nodeFetches += 1;
      return jsonResponse({});
    },
    maxRetries: 0,
  });
  await assert.rejects(
    () => nodeClient.getKagemushaOperationStatus({
      ...operationReference("top_up"),
      status_uri: "/v1/offline/operations/wrong",
    }),
    /status_uri does not match its operation_id/u,
  );
  assert.equal(nodeFetches, 0);

  let browserFetches = 0;
  const browserClient = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => {
      browserFetches += 1;
      return jsonResponse({});
    },
  });
  assert.throws(
    () => browserClient.getKagemushaOperationStatus(OPERATION_ID),
    /must be an object/u,
  );
  assert.equal(browserFetches, 0);
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
            anchor: { version: 3, artifact_binding: { version: 4 } },
            finality_proof: {},
          },
        },
      },
    }),
  });

  await assert.rejects(
    () => client.getKagemushaOperationStatus(operationReference("top_up")),
    /anchor and artifact binding must use V4/u,
  );
});

test("applied top-up parsing authenticates the balanced-Merkle execution commitment", () => {
  for (const normalize of [
    normalizeKagemushaOperationStatus,
    distSdk.normalizeKagemushaOperationStatus,
  ]) {
    const normalized = normalize(
      appliedTopUpStatus(),
      operationReference("top_up"),
    );
    assert.equal(normalized.state, "applied");
    assert.equal(normalized.value.result.kind, "top_up");
    assert.equal(normalized.value.result.result.finalized_block_height, 42);
  }
});

test("applied top-up parsing rejects a forged Merkle sibling and root", () => {
  for (const normalize of [
    normalizeKagemushaOperationStatus,
    distSdk.normalizeKagemushaOperationStatus,
  ]) {
    const forgedSibling = appliedTopUpStatus();
    forgedSibling.value.result.result.finality_proof.anchor_path.siblings[0][0] ^= 1;
    assert.throws(
      () => normalize(
        forgedSibling,
        operationReference("top_up"),
      ),
      /anchor path does not match the committed root/u,
    );

    const forgedRoot = appliedTopUpStatus();
    forgedRoot.value.result.result.finality_proof.commit_qc.certificate
      .execution_commitment.topup_anchor_root = testHashLiteral(
        testIrohaHash(new TextEncoder().encode("forged top-up root")),
      );
    assert.throws(
      () => normalize(
        forgedRoot,
        operationReference("top_up"),
      ),
      /anchor path does not match the committed root/u,
    );
  }
});

test("applied top-up parsing rejects noncanonical siblings and commitment projections", () => {
  const unmarkedSibling = appliedTopUpStatus();
  unmarkedSibling.value.result.result.finality_proof.anchor_path.siblings[0][31] &= 0xfe;
  assert.throws(
    () => normalizeKagemushaOperationStatus(
      unmarkedSibling,
      operationReference("top_up"),
    ),
    /invalid Iroha hash marker bit/u,
  );

  const mismatchedCount = appliedTopUpStatus();
  mismatchedCount.value.result.result.finality_proof.commit_qc.certificate
    .execution_commitment.topup_anchor_count = 3;
  assert.throws(
    () => normalizeKagemushaOperationStatus(
      mismatchedCount,
      operationReference("top_up"),
    ),
    /anchor count does not match its path/u,
  );

  for (const field of ["ordinary_writes_root", "post_state_root"]) {
    const forgedPostState = appliedTopUpStatus();
    forgedPostState.value.result.result.finality_proof.commit_qc.certificate
      .execution_commitment[field] = testHashLiteral(
        testIrohaHash(new TextEncoder().encode(`forged ${field}`)),
      );
    assert.throws(
      () => normalizeKagemushaOperationStatus(
        forgedPostState,
        operationReference("top_up"),
      ),
      /execution post-state root is invalid/u,
    );
  }
});

test("applied top-up parsing binds the anchor, proof, height, network, and transaction", () => {
  const mismatchedProofAnchor = appliedTopUpStatus();
  mismatchedProofAnchor.value.result.result.finality_proof.anchor.anchor_digest[0] ^= 1;
  assert.throws(
    () => normalizeKagemushaOperationStatus(
      mismatchedProofAnchor,
      operationReference("top_up"),
    ),
    /finality_proof\.anchor does not match the V4 top-up anchor/u,
  );

  for (const mutate of [
    (status) => { status.value.result.result.anchor.finalized_height = 43; },
    (status) => { status.value.result.result.anchor.finalized_tx_hash[0] ^= 1; },
    (status) => {
      status.value.result.result.finality_proof.commit_qc.height_context.network_id =
        testHashLiteral(testIrohaHash(new TextEncoder().encode("other network")));
    },
    (status) => {
      const markedZero = new Uint8Array(32);
      markedZero[31] = 1;
      const markedZeroLiteral = testHashLiteral(markedZero);
      status.value.result.result.anchor.network_id = markedZeroLiteral;
      status.value.result.result.finality_proof.commit_qc.height_context.network_id =
        markedZeroLiteral;
    },
  ]) {
    const status = appliedTopUpStatus();
    mutate(status);
    assert.throws(
      () => normalizeKagemushaOperationStatus(status, operationReference("top_up")),
      /top-up anchor, proof, and terminal result do not match/u,
    );
  }
});

test("applied top-up parsing returns the exact proof snapshot it validated", () => {
  const status = appliedTopUpStatus();
  const siblings = status.value.result.result.finality_proof.anchor_path.siblings;
  const validSibling = [...siblings[0]];
  const forgedSibling = [...validSibling];
  forgedSibling[0] ^= 1;
  let reads = 0;
  Object.defineProperty(siblings, 0, {
    configurable: true,
    enumerable: true,
    get() {
      reads += 1;
      return reads === 1 ? validSibling : forgedSibling;
    },
  });

  const normalized = normalizeKagemushaOperationStatus(
    status,
    operationReference("top_up"),
  );
  assert.equal(reads, 1);
  assert.deepEqual(
    normalized.value.result.result.finality_proof.anchor_path.siblings[0],
    validSibling,
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
      normalize(rejected, operationReference("redeem")).value.error.code,
      "offline_operation_rejected",
    );
    assert.equal(
      normalize({
        ...rejected,
        value: {
          ...rejected.value,
          error: { ...rejected.value.error, message: "😀".repeat(1024) },
        },
      }, operationReference("redeem")).value.error.message,
      "😀".repeat(1024),
    );
    assert.throws(
      () => normalize({
        ...rejected,
        value: {
          ...rejected.value,
          error: { ...rejected.value.error, retryable: true },
        },
      }, operationReference("redeem")),
      /error contains missing or unknown fields/u,
    );
    assert.throws(
      () => normalize({
        ...rejected,
        value: {
          ...rejected.value,
          error: { ...rejected.value.error, details: null },
        },
      }, operationReference("redeem")),
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
      }, operationReference("redeem")),
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
        }, operationReference("redeem")),
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
      }, operationReference("redeem")),
      /exact non-empty text/u,
    );
    assert.throws(
      () => normalize({
        ...rejected,
        value: {
          ...rejected.value,
          error: { ...rejected.value.error, message: "x".repeat(1025) },
        },
      }, operationReference("redeem")),
      /exact non-empty text/u,
    );
  }
});

test("applied operation parsing rejects a zero finalized height", () => {
  const applied = {
    state: "applied",
    value: {
      operation_id: OPERATION_ID,
      result: {
        kind: "redeem",
        result: {
          transaction_hash: TRANSACTION_HASH,
          finalized_block_height: 1,
        },
      },
    },
  };
  for (const field of ["finalized_block_height"]) {
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
      }, operationReference("redeem")),
      new RegExp(`${field} must be a positive safe unsigned integer`, "u"),
    );
  }
});
