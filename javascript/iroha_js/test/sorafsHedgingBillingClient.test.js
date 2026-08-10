import assert from "node:assert/strict";
import crypto from "node:crypto";
import { test } from "node:test";

import {
  canonicalRequestSignatureMessage,
  NetworkId,
  noritoEncodeSorafsBillingAcknowledgementProofV1,
  SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1,
  validateNoritoFrame,
  verifyEd25519,
} from "../src/index.js";
import { LocalSigningContext, ToriiClient } from "../src/toriiClient.js";

const BASE_URL = "https://torii.example";
const CHECKPOINT = "11".repeat(32);
const STATEMENT_ID = "22".repeat(32);
const AFTER_STATEMENT_ID = "33".repeat(32);
const AFTER_PROJECTION = "44".repeat(32);
const PRIVATE_KEY = Buffer.alloc(32, 29);
const NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const LOCAL_SIGNING_CONTEXT = new LocalSigningContext(NETWORK_ID);
const PRIVATE_KEY_OBJECT = crypto.createPrivateKey({
  key: Buffer.concat([
    Buffer.from("302e020100300506032b657004220420", "hex"),
    PRIVATE_KEY,
  ]),
  format: "der",
  type: "pkcs8",
});
const PUBLIC_KEY = Buffer.from(
  crypto
    .createPublicKey(PRIVATE_KEY_OBJECT)
    .export({ format: "der", type: "spki" }),
).subarray(-32);
const CANONICAL_AUTH = Object.freeze({
  accountId: "billing-reader@sora",
  privateKey: PRIVATE_KEY,
});

function byteResponse(
  bytes,
  {
    contentType = "application/json",
    contentEncoding,
    contentLength = bytes.byteLength,
    chunks = [bytes],
  } = {},
) {
  const headers = {
    "content-type": contentType,
    "content-length": String(contentLength),
  };
  if (contentEncoding !== undefined) {
    headers["content-encoding"] = contentEncoding;
  }
  return {
    status: 200,
    statusText: "OK",
    body: new ReadableStream({
      start(controller) {
        for (const chunk of chunks) controller.enqueue(chunk);
        controller.close();
      },
    }),
    headers: {
      get(name) {
        const normalized = String(name).toLowerCase();
        return headers[normalized] ?? null;
      },
    },
  };
}

function jsonResponse(payload, options) {
  return byteResponse(
    new TextEncoder().encode(JSON.stringify(payload)),
    options,
  );
}

function assertCanonicalSignature(
  call,
  expectedUrl,
  { method = "GET", body = Buffer.alloc(0) } = {},
) {
  assert.equal(call.url, expectedUrl);
  assert.equal(call.init.method, method);
  assert.equal(call.init.redirect, "error");
  assert.equal(call.init.headers["Accept-Encoding"], "identity");
  const url = new URL(call.url);
  const message = canonicalRequestSignatureMessage({
    networkId: NETWORK_ID,
    method,
    path: url.pathname,
    query: url.search.slice(1),
    body,
    timestampMs: Number(call.init.headers["X-Iroha-Timestamp-Ms"]),
    nonce: call.init.headers["X-Iroha-Nonce"],
  });
  assert.equal(
    verifyEd25519(
      message,
      Buffer.from(call.init.headers["X-Iroha-Signature"], "base64"),
      PUBLIC_KEY,
    ),
    true,
  );
}

test("SoraFS hedging and billing reads sign exact routes once and bound responses", async () => {
  const statementBytes = Uint8Array.of(0x4e, 0x52, 0x54, 0x31);
  const acknowledgementProof = {
    requestNonceHex: "91".repeat(32),
    authenticationProof: Buffer.alloc(64, 0xa5),
  };
  const acknowledgementBody =
    noritoEncodeSorafsBillingAcknowledgementProofV1(acknowledgementProof);
  const responses = [
    jsonResponse({ route: "status" }),
    jsonResponse({ route: "statements" }),
    byteResponse(statementBytes, { contentType: "application/x-norito" }),
    jsonResponse({ route: "acknowledgement" }),
    jsonResponse({ route: "reconciliation" }),
    jsonResponse({ route: "exposure" }),
    jsonResponse({ route: "intents" }),
  ];
  const calls = [];
  const client = new ToriiClient(BASE_URL, {
    localSigningContext: LOCAL_SIGNING_CONTEXT,
    maxRetries: 9,
    fetchImpl: async (url, init) => {
      calls.push({ url, init });
      const response = responses.shift();
      assert.ok(response, `unexpected request ${url}`);
      return response;
    },
  });

  assert.equal(
    (
      await client.getSorafsBillingStatus({ canonicalAuth: CANONICAL_AUTH })
    ).route,
    "status",
  );
  assert.equal(
    (
      await client.listSorafsBillingStatements({
        expectedCheckpointFingerprintHex: CHECKPOINT,
        afterStatementIdHex: AFTER_STATEMENT_ID,
        limit: 25,
        canonicalAuth: CANONICAL_AUTH,
      })
    ).route,
    "statements",
  );
  assert.deepEqual(
    await client.getSorafsBillingStatement(
      STATEMENT_ID,
      CHECKPOINT,
      { canonicalAuth: CANONICAL_AUTH },
    ),
    Buffer.from(statementBytes),
  );
  assert.equal(
    (
      await client.acknowledgeSorafsBillingStatement(
        STATEMENT_ID,
        CHECKPOINT,
        acknowledgementProof,
        { canonicalAuth: CANONICAL_AUTH },
      )
    ).route,
    "acknowledgement",
  );
  assert.equal(
    (
      await client.getSorafsBillingReconciliation({
        canonicalAuth: CANONICAL_AUTH,
      })
    ).route,
    "reconciliation",
  );
  assert.equal(
    (
      await client.getSorafsHedgingExposure({
        expectedCheckpointFingerprintHex: CHECKPOINT,
        afterHex: AFTER_PROJECTION,
        limit: 50,
        canonicalAuth: CANONICAL_AUTH,
      })
    ).route,
    "exposure",
  );
  assert.equal(
    (
      await client.getSorafsHedgingIntents({
        expectedCheckpointFingerprintHex: CHECKPOINT,
        limit: 100,
        canonicalAuth: CANONICAL_AUTH,
      })
    ).route,
    "intents",
  );

  const expectedUrls = [
    `${BASE_URL}/v1/sorafs/billing/status`,
    `${BASE_URL}/v1/sorafs/billing/statements?expected_checkpoint_fingerprint=${CHECKPOINT}&after_statement_id=${AFTER_STATEMENT_ID}&limit=25`,
    `${BASE_URL}/v1/sorafs/billing/statements/${STATEMENT_ID}?expected_checkpoint_fingerprint=${CHECKPOINT}`,
    `${BASE_URL}/v1/sorafs/billing/statements/${STATEMENT_ID}/acknowledgements?expected_checkpoint_fingerprint=${CHECKPOINT}`,
    `${BASE_URL}/v1/sorafs/billing/reconciliation`,
    `${BASE_URL}/v1/sorafs/hedging/exposure?expected_checkpoint_fingerprint=${CHECKPOINT}&after=${AFTER_PROJECTION}&limit=50`,
    `${BASE_URL}/v1/sorafs/hedging/intents?expected_checkpoint_fingerprint=${CHECKPOINT}&limit=100`,
  ];
  assert.equal(calls.length, expectedUrls.length);
  calls.forEach((call, index) => {
    const isAcknowledgement = index === 3;
    assertCanonicalSignature(call, expectedUrls[index], {
      method: isAcknowledgement ? "POST" : "GET",
      body: isAcknowledgement ? acknowledgementBody : Buffer.alloc(0),
    });
  });
  calls.forEach((call, index) => {
    assert.equal(
      call.init.headers.Accept,
      index === 2 ? "application/x-norito" : "application/json",
    );
  });
  assert.equal(
    calls[3].init.headers["Content-Type"],
    "application/x-norito",
  );
  assert.deepEqual(Buffer.from(calls[3].init.body), acknowledgementBody);
});

test("SoraFS billing acknowledgement encoder matches the shared Rust schema and bytes", () => {
  const encoded = noritoEncodeSorafsBillingAcknowledgementProofV1({
    requestNonceHex: "91".repeat(32),
    authenticationProof: Buffer.alloc(64, 0xa5),
  });
  assert.equal(
    encoded.toString("hex"),
    "4e5254300000fe75acabe03d788012f2e7c556319997006a0000000000000080460fddbba276090220" +
      "91".repeat(32) +
      "484000000000000000" +
      "a5".repeat(64),
  );
  const frame = validateNoritoFrame(encoded, {
    expectedTypeName:
      SORAFS_BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1,
    expectedPaddingLength: 0,
  });
  assert.equal(frame.schemaHash.toString("hex"), "fe75acabe03d788012f2e7c556319997");
  assert.equal(frame.flags, 0x02);

  for (const requestNonceHex of [
    "0".repeat(64),
    "AA".repeat(32),
    `0x${"91".repeat(32)}`,
    "91".repeat(31),
  ]) {
    assert.throws(() =>
      noritoEncodeSorafsBillingAcknowledgementProofV1({
        requestNonceHex,
        authenticationProof: Buffer.of(1),
      }),
    );
  }
  for (const authenticationProof of [
    Buffer.alloc(0),
    Buffer.alloc(64 * 1024 + 1),
    "a5",
    [0xa5],
  ]) {
    assert.throws(() =>
      noritoEncodeSorafsBillingAcknowledgementProofV1({
        requestNonceHex: "91".repeat(32),
        authenticationProof,
      }),
    );
  }
  assert.throws(() =>
    noritoEncodeSorafsBillingAcknowledgementProofV1({
      request_nonce_hex: "91".repeat(32),
      authentication_proof: Buffer.of(1),
    }),
  );
});

test("SoraFS hedging and billing reads reject aliases before requesting", async () => {
  let requests = 0;
  const client = new ToriiClient(BASE_URL, {
    localSigningContext: LOCAL_SIGNING_CONTEXT,
    fetchImpl: async () => {
      requests += 1;
      return jsonResponse({});
    },
  });
  const list = (overrides = {}) =>
    client.listSorafsBillingStatements({
      expectedCheckpointFingerprintHex: CHECKPOINT,
      limit: 1,
      canonicalAuth: CANONICAL_AUTH,
      ...overrides,
    });

  for (const invalid of [
    "AA".repeat(32),
    `0x${CHECKPOINT}`,
    ` ${CHECKPOINT}`,
    "0".repeat(64),
    "11".repeat(31),
    Buffer.alloc(32, 1),
  ]) {
    await assert.rejects(
      list({ expectedCheckpointFingerprintHex: invalid }),
    );
  }
  for (const invalid of [0, 101, "1", 1n, true]) {
    await assert.rejects(list({ limit: invalid }));
  }
  await assert.rejects(list({ afterStatementIdHex: "0".repeat(64) }));
  await assert.rejects(
    client.getSorafsHedgingExposure({
      expectedCheckpointFingerprintHex: CHECKPOINT,
      afterHex: "AA".repeat(32),
      limit: 1,
      canonicalAuth: CANONICAL_AUTH,
    }),
  );
  await assert.rejects(
    client.getSorafsBillingStatement(STATEMENT_ID, CHECKPOINT, {}),
  );
  await assert.rejects(
    client.acknowledgeSorafsBillingStatement(
      STATEMENT_ID,
      CHECKPOINT,
      {
        requestNonceHex: "0".repeat(64),
        authenticationProof: Buffer.of(1),
      },
      { canonicalAuth: CANONICAL_AUTH },
    ),
  );
  await assert.rejects(
    client.getSorafsBillingStatus({
      canonicalAuth: CANONICAL_AUTH,
      headers: {},
    }),
    /unsupported fields/u,
  );
  assert.equal(requests, 0);
});

test("SoraFS hedging and billing reads reject transformed, oversized, and ambiguous bodies", async () => {
  const oversized = new ToriiClient(BASE_URL, {
    localSigningContext: LOCAL_SIGNING_CONTEXT,
    fetchImpl: async () =>
      jsonResponse({}, { contentLength: 1024 * 1024 + 1 }),
  });
  await assert.rejects(
    oversized.getSorafsBillingStatus({ canonicalAuth: CANONICAL_AUTH }),
    /1048576-byte response limit/u,
  );

  const transformed = new ToriiClient(BASE_URL, {
    localSigningContext: LOCAL_SIGNING_CONTEXT,
    fetchImpl: async () =>
      jsonResponse({}, { contentEncoding: "gzip" }),
  });
  await assert.rejects(
    transformed.getSorafsBillingStatus({ canonicalAuth: CANONICAL_AUTH }),
    /Content-Encoding must be identity/u,
  );

  const ambiguousNorito = new ToriiClient(BASE_URL, {
    localSigningContext: LOCAL_SIGNING_CONTEXT,
    fetchImpl: async () =>
      byteResponse(Uint8Array.of(1), {
        contentType: "application/x-norito; version=1",
      }),
  });
  await assert.rejects(
    ambiguousNorito.getSorafsBillingStatement(
      STATEMENT_ID,
      CHECKPOINT,
      { canonicalAuth: CANONICAL_AUTH },
    ),
    /application\/x-norito media type/u,
  );
});

test("SoraFS hedging and billing canonical requests are never retried", async () => {
  let requests = 0;
  const client = new ToriiClient(BASE_URL, {
    localSigningContext: LOCAL_SIGNING_CONTEXT,
    maxRetries: 9,
    fetchImpl: async () => {
      requests += 1;
      throw new TypeError("network failed");
    },
  });
  await assert.rejects(
    client.getSorafsBillingStatus({ canonicalAuth: CANONICAL_AUTH }),
    /network failed/u,
  );
  assert.equal(requests, 1);
});
