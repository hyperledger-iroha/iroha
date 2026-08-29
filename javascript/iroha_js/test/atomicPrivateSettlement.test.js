import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

import {
  AtomicPrivateSettlementAuthV1,
  AtomicPrivateSettlementIdentifierV1,
  AtomicPrivateSettlementOperationV1,
  AtomicPrivateSettlementPreparedRequestV1,
  AtomicPrivateSettlementToriiClientV1,
  AtomicPrivateSettlementToriiErrorV1,
} from "../src/atomicPrivateSettlement.js";

const fixture = JSON.parse(
  readFileSync(
    new URL(
      "../../../fixtures/norito_rpc/atomic_private_settlement_sdk_v1.json",
      import.meta.url,
    ),
    "utf8",
  ),
);

const textEncoder = new TextEncoder();

function jsonBytes(value) {
  return textEncoder.encode(JSON.stringify(value));
}

function response(bodyValue, target, overrides = {}) {
  const retained = bodyValue instanceof Uint8Array ? bodyValue.slice() : jsonBytes(bodyValue);
  const headers = new Headers({
    "content-length": String(retained.byteLength),
    "content-type": "application/json",
    ...(overrides.headers ?? {}),
  });
  let delivered = false;
  let cancelled = false;
  return {
    body: {
      async cancel() {
        cancelled = true;
      },
      getReader() {
        return {
          async cancel() {
            cancelled = true;
          },
          async read() {
            if (delivered) return { done: true, value: undefined };
            delivered = true;
            return { done: false, value: retained.slice() };
          },
        };
      },
    },
    headers,
    ok: overrides.ok ?? true,
    redirected: overrides.redirected ?? false,
    status: overrides.status ?? 200,
    url: overrides.url ?? target,
    get cancelled() {
      return cancelled;
    },
  };
}

function sponsorHeaders() {
  return {
    "x-iroha-account": "sponsor@network",
    "x-iroha-signature": "signature",
    "x-iroha-timestamp-ms": "1234",
    "x-iroha-nonce": "nonce",
  };
}

function roleHeaders() {
  return {
    "x-iroha-operator-public-key": "public-key",
    "x-iroha-operator-timestamp-ms": "1234",
    "x-iroha-operator-nonce": "nonce",
    "x-iroha-operator-signature": "signature",
  };
}

test("shared fixture pins the complete JavaScript operation catalog", () => {
  assert.equal(fixture.fixture_kind, "norito_json_transport_contract_v1");
  assert.equal(fixture.version, 1);
  for (const expected of fixture.request_routes) {
    const actual = AtomicPrivateSettlementOperationV1[expected.operation];
    assert.ok(actual, `missing operation ${expected.operation}`);
    assert.equal(actual.path, expected.path);
    assert.equal(actual.auth, AtomicPrivateSettlementAuthV1[expected.auth]);
    assert.deepEqual(actual.topLevelFields, expected.top_level_fields);
  }
  assert.equal(
    Object.keys(AtomicPrivateSettlementOperationV1).length,
    fixture.request_routes.length,
  );
});

test("settlement identifiers enforce marker, checksum, and canonical literals", () => {
  const fromHex = new AtomicPrivateSettlementIdentifierV1(fixture.identifiers.bundle_hex);
  const fromLiteral = new AtomicPrivateSettlementIdentifierV1(fixture.identifiers.bundle_json);
  assert.equal(fromHex.pathComponent, fixture.identifiers.bundle_hex);
  assert.equal(fromHex.jsonLiteral, fixture.identifiers.bundle_json);
  assert.equal(fromLiteral.pathComponent, fromHex.pathComponent);
  assert.throws(
    () => new AtomicPrivateSettlementIdentifierV1("22".repeat(32)),
    /marker bit/u,
  );
  assert.throws(
    () => new AtomicPrivateSettlementIdentifierV1(`${fixture.identifiers.bundle_json.slice(0, -1)}0`),
    /checksum/u,
  );
  assert.throws(
    () => new AtomicPrivateSettlementIdentifierV1(fixture.identifiers.payload_json.toLowerCase()),
    /canonical Norito hash literal/u,
  );
});

test("prepared requests are operation-bound, strict, redacted, and erasable", () => {
  const prepared = new AtomicPrivateSettlementPreparedRequestV1(
    AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
    '{"approval":{"signature":"canary-secret"}}',
  );
  assert.match(prepared.toString(), /body=\[REDACTED\]/u);
  assert.doesNotMatch(prepared.toString(), /canary-secret/u);
  assert.notEqual(prepared.bytes(), prepared.bytes());
  prepared.close();
  assert.throws(() => prepared.bytes(), /closed/u);

  assert.throws(
    () => new AtomicPrivateSettlementPreparedRequestV1(
      AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
      '{"approval":{},"approval":{}}',
    ),
    /duplicate/u,
  );
  assert.throws(
    () => new AtomicPrivateSettlementPreparedRequestV1(
      AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
      '{"transaction":{}}',
    ),
    /unexpected public fields/u,
  );
});

test("exact-route client binds sponsor requests and validates leg identity", async () => {
  const calls = [];
  const fetchImpl = async (target, options) => {
    calls.push({ target, options, body: options.body?.slice() });
    return response(fixture.responses.leg_status, target);
  };
  const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    fetchImpl,
    sponsorHeaderProvider(request) {
      assert.equal(request.method, "GET");
      assert.equal(request.body.byteLength, 0);
      return sponsorHeaders();
    },
  });
  const result = await client.getLegStatus(fixture.identifiers.payload_hex);
  assert.match(result.toString(), /body=\[REDACTED\]/u);
  assert.deepEqual(
    JSON.parse(new TextDecoder().decode(result.bytes())),
    fixture.responses.leg_status,
  );
  assert.equal(calls.length, 1);
  assert.equal(
    calls[0].target,
    `https://torii.example/v1/nexus/private-settlements/legs/${fixture.identifiers.payload_hex}/status`,
  );
  assert.equal(calls[0].options.redirect, "error");
  assert.equal(calls[0].options.credentials, "omit");
  assert.equal(calls[0].options.headers["Accept-Encoding"], "identity");
  assert.equal(calls[0].options.headers["x-iroha-signature"], "signature");
  assert.equal("Authorization" in calls[0].options.headers, false);
  result.close();
  assert.throws(() => result.bytes(), /closed/u);
});

test("native-prepared request bytes are isolated from signer mutation", async () => {
  const prepared = new AtomicPrivateSettlementPreparedRequestV1(
    AtomicPrivateSettlementOperationV1.LEG_UPLOAD,
    '{"manifest":{},"audit_policy":{},"committee_authority":{},"payload":{}}',
  );
  const original = prepared.bytes();
  let transported;
  const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    sponsorHeaderProvider(request) {
      request.body.fill(0);
      return sponsorHeaders();
    },
    async fetchImpl(target, options) {
      transported = options.body.slice();
      return response({
        bundle_id: fixture.identifiers.bundle_json,
        payload_digest: fixture.identifiers.payload_json,
        leg_ordinal: 0,
        disposition: { result: "stored", value: null },
        lifecycle: { status: "collecting", value: null },
      }, target);
    },
  });
  await client.uploadLeg(prepared);
  assert.deepEqual(transported, original);
  assert.deepEqual(prepared.bytes(), original);
});

test("role and public queries use disjoint authentication policies", async () => {
  const payloadPath = fixture.identifiers.payload_hex;
  const bundlePath = fixture.identifiers.bundle_hex;
  const seen = [];
  const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    async fetchImpl(target, options) {
      seen.push({ target, headers: options.headers });
      if (target.endsWith("/audit-approvals")) {
        return response(fixture.responses.audit_approval, target);
      }
      if (target.endsWith("/receipt")) {
        return response(fixture.responses.receipt_pending, target);
      }
      return response(fixture.responses.bundle_status_aborted, target);
    },
  });
  const approval = new AtomicPrivateSettlementPreparedRequestV1(
    AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
    '{"approval":{}}',
  );
  await client.submitAuditApproval(payloadPath, approval, {
    roleHeaderProvider: roleHeaders,
  });
  await client.getBundleStatus(bundlePath);
  await client.getBundleReceipt(bundlePath);
  assert.equal(seen[0].headers["x-iroha-operator-signature"], "signature");
  assert.equal("x-iroha-account" in seen[1].headers, false);
  assert.equal("x-iroha-operator-signature" in seen[1].headers, false);
  assert.equal("x-iroha-account" in seen[2].headers, false);
});

test("client rejects header collisions, redirects, substitutions, and extra fields", async () => {
  const prepared = new AtomicPrivateSettlementPreparedRequestV1(
    AtomicPrivateSettlementOperationV1.BUNDLE_SUBMIT,
    '{"transaction":{}}',
  );
  let fetchCalled = false;
  const colliding = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    sponsorHeaderProvider() {
      return {
        ...sponsorHeaders(),
        "X-Iroha-Account": "substituted",
      };
    },
    async fetchImpl() {
      fetchCalled = true;
      throw new Error("must not run");
    },
  });
  await assert.rejects(() => colliding.submitBundle(prepared), /duplicate header/u);
  assert.equal(fetchCalled, false);

  const redirected = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    async fetchImpl(target) {
      return response(fixture.responses.bundle_status_aborted, target, { redirected: true });
    },
  });
  await assert.rejects(
    () => redirected.getBundleStatus(fixture.identifiers.bundle_hex),
    AtomicPrivateSettlementToriiErrorV1,
  );

  const substituted = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    async fetchImpl(target) {
      return response({
        ...fixture.responses.receipt_pending,
        value: {
          ...fixture.responses.receipt_pending.value,
          bundle_id: fixture.identifiers.payload_json,
        },
      }, target);
    },
  });
  await assert.rejects(
    () => substituted.getBundleReceipt(fixture.identifiers.bundle_hex),
    /response is invalid/u,
  );

  const extraField = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    sponsorHeaderProvider: sponsorHeaders,
    async fetchImpl(target) {
      return response({ ...fixture.responses.leg_status, amount: "canary-secret" }, target);
    },
  });
  await assert.rejects(
    () => extraField.getLegStatus(fixture.identifiers.payload_hex),
    (error) => {
      assert.match(error.message, /response is invalid/u);
      assert.doesNotMatch(error.message, /canary-secret/u);
      return true;
    },
  );
});
