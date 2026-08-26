import { test } from "node:test";
import assert from "node:assert/strict";
import { sha256 } from "@noble/hashes/sha2";

import * as sdk from "../src/index.js";
import * as distSdk from "../dist/index.js";
import { LocalSigningContext, ToriiClient } from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import { NetworkId } from "../src/networkId.js";
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
import { computeHashLiteralCrc } from "../src/hashLiteralCrc.js";
import { crc64Xz } from "../src/crc64Xz.js";

const OPERATION_ID = "11".repeat(32);
const TRANSACTION_HASH = "23".repeat(32);
const UNMARKED_TRANSACTION_HASH = "22".repeat(32);
const NETWORK_ID_BYTES = "a1".repeat(32);
const FOREIGN_NETWORK_ID_BYTES = "b1".repeat(32);

function canonicalHashLiteral(bytes) {
  const body = bytes.toUpperCase();
  return `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
}

const NETWORK_ID = canonicalHashLiteral(NETWORK_ID_BYTES);
const FOREIGN_NETWORK_ID = canonicalHashLiteral(FOREIGN_NETWORK_ID_BYTES);
const EXACT_NETWORK_ID = NetworkId.parse(NETWORK_ID_BYTES);
const LOCAL_SIGNING_CONTEXT = new LocalSigningContext(EXACT_NETWORK_ID);
const ACTIVATION_BLOCKERS = [
  {
    code: "offline_cash_authenticated_release_unavailable",
    message: "No authenticated Offline Cash V1 release is selected by this asset-neutral response.",
  },
  {
    code: "offline_cash_eligible_asset_unavailable",
    message: "No eligible Offline Cash V1 asset is selected by this asset-neutral response.",
  },
  {
    code: "offline_cash_proof_backend_unavailable",
    message:
      "No reviewed production Offline Cash V1 proof and secure-device backend is authenticated by this response.",
  },
];

function jsonResponse(payload, { status = 200, headers = {} } = {}) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json", ...headers },
  });
}

function universalCapability(overrides = {}) {
  return {
    mandatory: false,
    cash_handoff_capability: "cash_handoff_v1",
    required_bridge_abi_version: 22,
    max_hops: 8,
    ready: false,
    assets: [],
    blockers: ACTIVATION_BLOCKERS.map((blocker) => ({ ...blocker })),
    ...overrides,
  };
}

function compactLength(value) {
  let remaining = BigInt(value);
  const encoded = [];
  do {
    let byte = Number(remaining & 0x7fn);
    remaining >>= 7n;
    if (remaining !== 0n) byte |= 0x80;
    encoded.push(byte);
  } while (remaining !== 0n);
  return Uint8Array.from(encoded);
}

function concatBytes(...values) {
  const output = new Uint8Array(values.reduce((sum, value) => sum + value.byteLength, 0));
  let offset = 0;
  for (const value of values) {
    output.set(value, offset);
    offset += value.byteLength;
  }
  return output;
}

function canonicalField(value) {
  return concatBytes(compactLength(value.byteLength), value);
}

function canonicalStruct(fields) {
  return concatBytes(...fields.map(canonicalField));
}

function littleEndianU16(value) {
  const bytes = new Uint8Array(2);
  new DataView(bytes.buffer).setUint16(0, value, true);
  return bytes;
}

function littleEndianU64(value) {
  const bytes = new Uint8Array(8);
  new DataView(bytes.buffer).setBigUint64(0, BigInt(value), true);
  return bytes;
}

function bytesFromHex(value) {
  return Uint8Array.from(value.match(/../gu), (pair) => Number.parseInt(pair, 16));
}

function noritoArchive(kind = "top_up", options = {}) {
  const schema = kind === "top_up"
    ? "iroha.torii.v1.offline.top_up.request"
    : "iroha.torii.v1.offline.redeem.request";
  const operationId = bytesFromHex(options.operationId ?? OPERATION_ID);
  const authorizationOperationId = bytesFromHex(
    options.authorizationOperationId ?? options.operationId ?? OPERATION_ID,
  );
  const networkId = bytesFromHex(options.networkId ?? NETWORK_ID_BYTES);
  const empty = new Uint8Array(0);
  const authorizationFields = Array.from({ length: 10 }, () => empty);
  authorizationFields[3] = authorizationOperationId;
  authorizationFields[4] = littleEndianU64(options.issuedAtMs ?? 1234n);
  const authorization = canonicalStruct(authorizationFields);
  let fields;
  if (kind === "top_up") {
    const currentNoteFields = Array.from({ length: 5 }, () => empty);
    currentNoteFields[0] = networkId;
    fields = Array.from({ length: 8 }, () => empty);
    fields[0] = littleEndianU16(4);
    fields[3] = canonicalStruct(currentNoteFields);
    fields[6] = operationId;
    fields[7] = authorization;
  } else {
    const statementFields = Array.from({ length: 13 }, () => empty);
    statementFields[0] = networkId;
    const bundleFields = [canonicalStruct(statementFields), empty, empty];
    fields = Array.from({ length: 10 }, () => empty);
    fields[0] = littleEndianU16(4);
    fields[1] = canonicalStruct(bundleFields);
    fields[8] = operationId;
    fields[9] = authorization;
  }
  const payload = canonicalStruct(fields);
  const archive = new Uint8Array(40 + 8 + payload.byteLength);
  const header = new DataView(archive.buffer);
  archive.set([0x4e, 0x52, 0x54, 0x30]);
  archive.set(
    sha256(new TextEncoder().encode(`norito:v1:type-name\0${schema}`)).subarray(0, 16),
    6,
  );
  header.setBigUint64(23, BigInt(payload.byteLength), true);
  header.setBigUint64(31, crc64Xz(payload), true);
  archive[39] = 0x02;
  archive.set(payload, 48);
  return archive;
}

function requestV4(kind = "top_up", options = {}) {
  return {
    version: 4,
    operationId: options.wrapperOperationId ?? options.operationId ?? OPERATION_ID,
    norito: noritoArchive(kind, options),
  };
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

function hexBytes(value) {
  return Array.from(value.match(/../gu), (pair) => Number.parseInt(pair, 16));
}

function appliedTopUpStatus(networkId = NETWORK_ID) {
  const operationId = hexBytes(OPERATION_ID);
  const transactionHash = hexBytes(TRANSACTION_HASH);
  const anchorDigest = Array(32).fill(0x41);
  return {
    state: "applied",
    value: {
      operation_id: OPERATION_ID,
      result: {
        kind: "top_up",
        result: {
          transaction_hash: TRANSACTION_HASH,
          finalized_block_height: 42,
          server_time_ms: 1234,
          anchor: {
            version: 4,
            network_id: networkId,
            current_note: { network_id: networkId },
            topup_operation_id: operationId,
            artifact_binding: { version: 4 },
            finalized_height: 42,
            finalized_tx_hash: transactionHash,
            anchor_digest: anchorDigest,
          },
          finality_proof: {
            version: 1,
            anchor: {
              topup_operation_id: operationId,
              anchor_digest: [...anchorDigest],
            },
            commit_qc: {
              height_context: { height: 42, network_id: networkId },
            },
          },
        },
      },
    },
  };
}

test("Kagemusha JavaScript surface is transport-only ABI-22/V4", () => {
  assert.equal(KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION, 22);
  assert.equal(KAGEMUSHA_CASH_HANDOFF_CAPABILITY, "cash_handoff_v1");
  assert.equal(KAGEMUSHA_MANIFEST_VERSION, 4);
  assert.equal(KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES, 512 * 1024);
  assert.equal(KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES, 48 * 1024 * 1024);
  assert.equal(distSdk.KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION, 22);
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
    /required_bridge_abi_version must be 22/u,
  );
  const capability = normalizeOfflineStatus(universalCapability());
  assert.equal(capability.ready, false);
  assert.deepEqual(capability.blockers, ACTIVATION_BLOCKERS);
  assert.throws(
    () => normalizeOfflineStatus(universalCapability({ ready: true })),
    /ready must be false/u,
  );
  assert.throws(
    () =>
      normalizeOfflineStatus(
        universalCapability({
          blockers: [
            ...ACTIVATION_BLOCKERS.slice(0, 2),
            { ...ACTIVATION_BLOCKERS[2], code: "proof_backend_unavailable" },
          ],
        }),
      ),
    /not the canonical activation blocker/u,
  );
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4({ ...requestV4(), version: 3 }),
    /version must be 4; V3 archives are not upgraded/u,
  );
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4(requestV4("redeem")),
    /canonical compact/u,
  );
  const corruptChecksum = noritoArchive();
  corruptChecksum[corruptChecksum.length - 1] ^= 1;
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4({ ...requestV4(), norito: corruptChecksum }),
    /canonical compact/u,
  );
  const noncanonicalFlags = noritoArchive();
  noncanonicalFlags[39] = 0;
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4({ ...requestV4(), norito: noncanonicalFlags }),
    /canonical compact/u,
  );
});

test("Kagemusha request normalization derives the signed canonical bindings", () => {
  for (const [kind, normalize] of [
    ["top_up", normalizeKagemushaTopUpRequestV4],
    ["redeem", sdk.normalizeKagemushaRedeemRequestV4],
  ]) {
    const request = requestV4(kind);
    const original = Uint8Array.from(request.norito);
    const normalized = normalize(request);
    assert.equal(normalized.operationId, OPERATION_ID);
    assert.equal(normalized.issuedAtMs, 1234);
    assert.equal(normalized.networkId, NETWORK_ID_BYTES);
    assert.notEqual(normalized.norito, request.norito);
    assert.deepEqual(normalized.norito, original);
    request.norito.fill(0);
    assert.deepEqual(normalized.norito, original);
  }

  assert.throws(
    () => normalizeKagemushaTopUpRequestV4(requestV4("top_up", {
      wrapperOperationId: "33".repeat(32),
    })),
    /operationId must match the signed Norito request body/u,
  );
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4(requestV4("top_up", {
      authorizationOperationId: "33".repeat(32),
    })),
    /request and authorization operation ids must match exactly/u,
  );
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4(requestV4("top_up", {
      operationId: "00".repeat(32),
      wrapperOperationId: OPERATION_ID,
    })),
    /request operation id must contain exactly 32 non-zero bytes/u,
  );
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4(requestV4("top_up", { issuedAtMs: 0n })),
    /issued_at_ms must be at least 1/u,
  );
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4(requestV4("top_up", {
      issuedAtMs: BigInt(Number.MAX_SAFE_INTEGER) + 1n,
    })),
    /issued_at_ms must fit in a safe unsigned integer/u,
  );
  assert.throws(
    () => normalizeKagemushaTopUpRequestV4(requestV4("top_up", {
      networkId: "a0".repeat(32),
    })),
    /NetworkId must contain exactly 32 marked bytes/u,
  );
});

test("Kagemusha command dispatch requires and binds the exact local NetworkId", async () => {
  let fetchCalls = 0;
  const fetchImpl = async () => {
    fetchCalls += 1;
    throw new Error("invalid Kagemusha request reached fetch");
  };
  const foreignNetworkId = NetworkId.parse(FOREIGN_NETWORK_ID_BYTES);
  const clients = [
    {
      client: new ToriiClient("https://torii.example", { fetchImpl, maxRetries: 0 }),
      message: /options\.localSigningContext/u,
    },
    {
      client: new ToriiClient("https://torii.example", {
        fetchImpl,
        maxRetries: 0,
        localSigningContext: new LocalSigningContext(foreignNetworkId),
      }),
      message: /signed request network does not match the local signing context/u,
    },
    {
      client: new ToriiBrowserClient("https://torii.example", { fetchImpl }),
      message: /options\.networkId/u,
    },
    {
      client: new ToriiBrowserClient("https://torii.example", {
        fetchImpl,
        networkId: foreignNetworkId,
      }),
      message: /signed request network does not match the configured NetworkId/u,
    },
  ];
  for (const { client, message } of clients) {
    // eslint-disable-next-line no-await-in-loop
    await assert.rejects(async () => client.submitKagemushaTopUpV4(requestV4()), message);
  }
  assert.equal(fetchCalls, 0);
});

test("Kagemusha accepted response time must equal the signed issued_at_ms", async () => {
  const client = new ToriiClient("https://torii.example", {
    localSigningContext: LOCAL_SIGNING_CONTEXT,
    maxRetries: 0,
    fetchImpl: async () => jsonResponse(
      { ...operationReference("top_up"), submitted_at_ms: 1235 },
      {
        status: 202,
        headers: {
          location: `/v1/offline/operations/${OPERATION_ID}`,
          "retry-after": "1",
        },
      },
    ),
  });
  await assert.rejects(
    client.submitKagemushaTopUpV4(requestV4()),
    /submitted_at_ms does not match the signed V4 command/u,
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
    localSigningContext: LOCAL_SIGNING_CONTEXT,
    fetchImpl: async (url, init) => {
      observed.push({ url: new URL(url), init });
      return responses.shift();
    },
    maxRetries: 0,
  });

  const capability = await client.getOfflineCapability();
  const topUp = await client.submitKagemushaTopUpV4(requestV4());
  const redeem = await client.submitKagemushaRedeemV4(requestV4("redeem"));
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
  for (const [index, { init }] of observed.slice(1, 3).entries()) {
    const headers = new Headers(init.headers);
    assert.equal(headers.get("content-type"), "application/x-norito");
    assert.equal(headers.get("idempotency-key"), OPERATION_ID);
    assert.deepEqual(
      [...new Uint8Array(init.body)],
      [...noritoArchive(index === 0 ? "top_up" : "redeem")],
    );
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
    networkId: EXACT_NETWORK_ID,
    fetchImpl: async (url, init) => {
      observed.push({ url: new URL(url), init });
      return responses.shift();
    },
  });

  const capability = await client.getOfflineCapability();
  const reference = await client.submitKagemushaTopUpV4(requestV4());
  const status = await client.getKagemushaOperationStatus(OPERATION_ID);

  assert.equal(capability.ready, false);
  assert.deepEqual(capability.blockers, ACTIVATION_BLOCKERS);
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

test("all Kagemusha transaction carriers require the Iroha hash marker", () => {
  const statusPayloads = [
    {
      state: "pending",
      value: {
        operation_id: OPERATION_ID,
        kind: { kind: "top_up", value: null },
        transaction_hash: UNMARKED_TRANSACTION_HASH,
        submitted_at_ms: 1234,
      },
    },
    {
      state: "applied",
      value: {
        operation_id: OPERATION_ID,
        result: {
          kind: "redeem",
          result: {
            transaction_hash: UNMARKED_TRANSACTION_HASH,
            finalized_block_height: 42,
            server_time_ms: 1234,
          },
        },
      },
    },
    {
      state: "rejected",
      value: {
        operation_id: OPERATION_ID,
        kind: { kind: "redeem", value: null },
        transaction_hash: UNMARKED_TRANSACTION_HASH,
        error: {
          code: "offline_operation_rejected",
          message: "rejected",
        },
      },
    },
  ];
  for (const publicSurface of [sdk, distSdk]) {
    assert.throws(
      () => publicSurface.normalizeKagemushaOperationReference(
        {
          ...operationReference("top_up"),
          transaction_hash: UNMARKED_TRANSACTION_HASH,
        },
        {
          expectedOperationId: OPERATION_ID,
          expectedKind: "top_up",
          expectedSubmittedAtMs: 1234,
          location: `/v1/offline/operations/${OPERATION_ID}`,
          retryAfter: "1",
        },
      ),
      /marker bit/u,
    );
    for (const payload of statusPayloads) {
      assert.throws(
        () => publicSurface.normalizeKagemushaOperationStatus(payload, OPERATION_ID),
        /marker bit/u,
      );
    }
  }
});

test("accepted and terminal Kagemusha responses fail closed on ambiguous finality", () => {
  const statusUri = `/v1/offline/operations/${OPERATION_ID}`;
  const pending = {
    state: "pending",
    value: {
      operation_id: OPERATION_ID,
      kind: { kind: "top_up", value: null },
      transaction_hash: TRANSACTION_HASH,
      submitted_at_ms: 1234,
    },
  };
  const redeem = {
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
  };
  for (const publicSurface of [sdk, distSdk]) {
    const referenceOptions = {
      expectedOperationId: OPERATION_ID,
      expectedKind: "top_up",
      expectedSubmittedAtMs: 1234,
      location: statusUri,
      retryAfter: "1",
    };
    for (const retryAfter of [
      null,
      "0",
      "01",
      "+1",
      "-1",
      " 1",
      "1 ",
      "1.0",
      "1\u0661",
      "1, 1",
      "18446744073709551616",
    ]) {
      assert.throws(
        () => publicSurface.normalizeKagemushaOperationReference(
          operationReference("top_up"),
          { ...referenceOptions, retryAfter },
        ),
        /Retry-After/u,
      );
    }
    assert.throws(
      () => publicSurface.normalizeKagemushaOperationReference(
        operationReference("top_up"),
        { ...referenceOptions, location: `${statusUri}, ${statusUri}` },
      ),
      /does not match/u,
    );
    assert.equal(
      publicSurface.normalizeKagemushaOperationReference(
        operationReference("top_up"),
        { ...referenceOptions, retryAfter: "18446744073709551615" },
      ).operation_id,
      OPERATION_ID,
    );
    assert.throws(
      () => publicSurface.normalizeKagemushaOperationReference(
        { ...operationReference("top_up"), submitted_at_ms: 0 },
        referenceOptions,
      ),
      /positive/u,
    );
    assert.throws(
      () => publicSurface.normalizeKagemushaOperationStatus(
        { ...pending, value: { ...pending.value, submitted_at_ms: 0 } },
        OPERATION_ID,
      ),
      /positive/u,
    );
    for (const field of ["finalized_block_height", "server_time_ms"]) {
      assert.throws(
        () => publicSurface.normalizeKagemushaOperationStatus({
          ...redeem,
          value: {
            ...redeem.value,
            result: {
              ...redeem.value.result,
              result: { ...redeem.value.result.result, [field]: 0 },
            },
          },
        }, OPERATION_ID),
        /positive/u,
      );
    }

    const validTopUp = appliedTopUpStatus();
    assert.equal(
      publicSurface.normalizeKagemushaOperationStatus(
        validTopUp,
        OPERATION_ID,
        { expectedNetworkId: NETWORK_ID_BYTES },
      ).state,
      "applied",
    );
    const mutations = [
      (payload) => { payload.value.result.result.anchor.finalized_tx_hash[0] ^= 1; },
      (payload) => { payload.value.result.result.anchor.finalized_height += 1; },
      (payload) => {
        payload.value.result.result.anchor.current_note.network_id = FOREIGN_NETWORK_ID;
      },
      (payload) => { payload.value.result.result.finality_proof.anchor.anchor_digest[0] ^= 1; },
      (payload) => {
        payload.value.result.result.finality_proof.commit_qc.height_context.height += 1;
      },
      (payload) => {
        payload.value.result.result.finality_proof.commit_qc.height_context.network_id =
          FOREIGN_NETWORK_ID;
      },
    ];
    for (const mutate of mutations) {
      const invalid = structuredClone(validTopUp);
      mutate(invalid);
      assert.throws(
        () => publicSurface.normalizeKagemushaOperationStatus(
          invalid,
          OPERATION_ID,
          { expectedNetworkId: NETWORK_ID_BYTES },
        ),
        /bind/u,
      );
    }
    assert.throws(
      () => publicSurface.normalizeKagemushaOperationStatus(
        appliedTopUpStatus(FOREIGN_NETWORK_ID),
        OPERATION_ID,
        { expectedNetworkId: NETWORK_ID_BYTES },
      ),
      /bind/u,
    );
  }
});

test("configured browser clients bind applied top-ups to their exact NetworkId", async () => {
  const responses = [
    jsonResponse(appliedTopUpStatus()),
    jsonResponse(appliedTopUpStatus(FOREIGN_NETWORK_ID)),
  ];
  const client = new ToriiBrowserClient("https://torii.example", {
    networkId: sdk.NetworkId.parse(NETWORK_ID_BYTES),
    fetchImpl: async () => responses.shift(),
  });

  assert.equal(
    (await client.getKagemushaOperationStatus(OPERATION_ID)).state,
    "applied",
  );
  await assert.rejects(
    client.getKagemushaOperationStatus(OPERATION_ID),
    /bind/u,
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
    for (const code of ["rejected", "future_offline_rejection", ""]) {
      assert.throws(
        () => normalize({
          ...rejected,
          value: {
            ...rejected.value,
            error: { ...rejected.value.error, code },
          },
        }, OPERATION_ID),
        /code must be offline_operation_rejected/u,
      );
    }
    for (const details of [null, {}, { layer: "torii" }]) {
      assert.throws(
        () => normalize({
          ...rejected,
          value: {
            ...rejected.value,
            error: { ...rejected.value.error, details },
          },
        }, OPERATION_ID),
        /error contains missing or unknown fields/u,
      );
    }

    const maximumAstralMessage = "\u{1f600}".repeat(1024);
    assert.equal(
      normalize({
        ...rejected,
        value: {
          ...rejected.value,
          error: { ...rejected.value.error, message: maximumAstralMessage },
        },
      }, OPERATION_ID).value.error.message,
      maximumAstralMessage,
    );
    for (const message of [
      "",
      " leading",
      "trailing ",
      "line\nbreak",
      "control\u0085",
      "\ud800",
      "\ud800x",
      "\udc00",
      "\u{1f600}".repeat(1025),
    ]) {
      assert.throws(
        () => normalize({
          ...rejected,
          value: {
            ...rejected.value,
            error: { ...rejected.value.error, message },
          },
        }, OPERATION_ID),
        /error\.message/u,
      );
    }
  }
});
