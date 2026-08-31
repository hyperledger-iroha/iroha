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

function acceptingNativeVerifier(calls = undefined) {
  return {
    privateSettlementVerifyCommitteeProofResponseV1(...arguments_) {
      calls?.committee.push(arguments_);
    },
    privateSettlementVerifyAuditorCapsuleResponseV1(...arguments_) {
      calls?.capsule.push(arguments_);
    },
    privateSettlementVerifyAuditApprovalResponseV1(...arguments_) {
      calls?.approval.push(arguments_);
    },
  };
}

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

function attestationNetworkId() {
  return fixture.responses.auditor_capsule.responder_attestation.body.network_id;
}

function auditApprovalRequest(networkId = attestationNetworkId()) {
  return new AtomicPrivateSettlementPreparedRequestV1(
    AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
    JSON.stringify({
      approval: {
        body: {
          version: 1,
          network_id: networkId,
          bundle_id: fixture.identifiers.bundle_json,
          leg_ordinal: 0,
          dataspace_id: 7,
          auditor_id: "auditor-test",
          audit_policy_digest: fixture.identifiers.payload_json,
          audit_key_epoch: 1,
          proof_digest: fixture.identifiers.payload_json,
          capsule_digest: fixture.identifiers.payload_json,
          delta_digest: fixture.identifiers.payload_json,
          old_root: "11".repeat(32),
          new_root: "22".repeat(32),
          expiry_height: 200,
        },
        signature: "opaque-native-signature",
      },
    }),
  );
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

test("sponsor phase-certificate recovery is path-bound and strictly allowlisted", async () => {
  const calls = [];
  const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    sponsorHeaderProvider(request) {
      assert.equal(request.method, "GET");
      assert.equal(request.body.byteLength, 0);
      return sponsorHeaders();
    },
    async fetchImpl(target, options) {
      calls.push({ target, options });
      return response(fixture.responses.phase_certificates, target);
    },
  });

  const result = await client.getPhaseCertificates(fixture.identifiers.payload_hex);

  assert.deepEqual(
    JSON.parse(new TextDecoder().decode(result.bytes())),
    fixture.responses.phase_certificates,
  );
  assert.match(result.toString(), /body=\[REDACTED\]/u);
  assert.equal(
    calls[0].target,
    `https://torii.example/v1/nexus/private-settlements/legs/${fixture.identifiers.payload_hex}/phase-certificates`,
  );
  assert.equal(calls[0].options.headers["x-iroha-signature"], "signature");
  assert.equal("x-iroha-operator-signature" in calls[0].options.headers, false);

  const missing = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    sponsorHeaderProvider: sponsorHeaders,
    async fetchImpl(target) {
      const body = { ...fixture.responses.phase_certificates };
      delete body.commit_certificate;
      return response(body, target);
    },
  });
  await assert.rejects(
    () => missing.getPhaseCertificates(fixture.identifiers.payload_hex),
    /response is invalid/u,
  );

  const nonObject = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    sponsorHeaderProvider: sponsorHeaders,
    async fetchImpl(target) {
      return response({
        ...fixture.responses.phase_certificates,
        prepare_certificate: [],
      }, target);
    },
  });
  await assert.rejects(
    () => nonObject.getPhaseCertificates(fixture.identifiers.payload_hex),
    /response is invalid/u,
  );

  const leaked = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    sponsorHeaderProvider: sponsorHeaders,
    async fetchImpl(target) {
      return response({
        ...fixture.responses.phase_certificates,
        plaintext: "LEAK_CANARY",
      }, target);
    },
  });
  await assert.rejects(
    () => leaked.getPhaseCertificates(fixture.identifiers.payload_hex),
    (error) => {
      assert.match(error.message, /response is invalid/u);
      assert.doesNotMatch(error.message, /LEAK_CANARY/u);
      return true;
    },
  );
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

test("bundle admission response is nonterminal and exact", async () => {
  const request = new AtomicPrivateSettlementPreparedRequestV1(
    AtomicPrivateSettlementOperationV1.BUNDLE_SUBMIT,
    '{"transaction":{}}',
  );
  const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    sponsorHeaderProvider: sponsorHeaders,
    async fetchImpl(target) {
      return response(fixture.responses.bundle_submit, target, { status: 202 });
    },
  });
  const admitted = await client.submitBundle(request);
  assert.deepEqual(
    JSON.parse(new TextDecoder().decode(admitted.bytes())),
    fixture.responses.bundle_submit,
  );
  assert.equal("lifecycle" in fixture.responses.bundle_submit, false);
});

test("bundle admission rejects malformed identifiers, heights, and fields", async () => {
  const request = new AtomicPrivateSettlementPreparedRequestV1(
    AtomicPrivateSettlementOperationV1.BUNDLE_SUBMIT,
    '{"transaction":{}}',
  );
  const valid = fixture.responses.bundle_submit;
  const wrongStatusClient = new AtomicPrivateSettlementToriiClientV1(
    "https://torii.example",
    {
      sponsorHeaderProvider: sponsorHeaders,
      async fetchImpl(target) {
        return response(valid, target, { status: 200 });
      },
    },
  );
  await assert.rejects(
    () => wrongStatusClient.submitBundle(request),
    /response status is invalid/u,
  );

  const malformed = [
    { ...valid, bundle_id: fixture.identifiers.bundle_hex },
    { ...valid, bundle_id: 1 },
    { ...valid, carrier_id: `${fixture.identifiers.payload_json.slice(0, -1)}0` },
    { ...valid, carrier_id: null },
    { ...valid, accepted_at_height: true },
    { ...valid, accepted_at_height: -1 },
    { ...valid, accepted_at_height: String(valid.accepted_at_height) },
    { ...valid, accepted_at_height: 1.5 },
    { ...valid, lifecycle: { status: "finalized", value: null } },
    { bundle_id: valid.bundle_id, accepted_at_height: valid.accepted_at_height },
  ];
  for (const candidate of malformed) {
    const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
      sponsorHeaderProvider: sponsorHeaders,
      async fetchImpl(target) {
        return response(candidate, target, { status: 202 });
      },
    });
    await assert.rejects(
      () => client.submitBundle(request),
      /atomic private settlement response is invalid/u,
    );
  }

  const maxHeightResponse = jsonBytes({
    ...valid,
    accepted_at_height: 0,
  });
  const text = new TextDecoder().decode(maxHeightResponse)
    .replace('"accepted_at_height":0', '"accepted_at_height":18446744073709551615');
  const maxHeightClient = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    sponsorHeaderProvider: sponsorHeaders,
    async fetchImpl(target) {
      return response(new TextEncoder().encode(text), target, { status: 202 });
    },
  });
  await maxHeightClient.submitBundle(request);

  const overflow = text.replace("18446744073709551615", "18446744073709551616");
  const overflowClient = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    sponsorHeaderProvider: sponsorHeaders,
    async fetchImpl(target) {
      return response(new TextEncoder().encode(overflow), target, { status: 202 });
    },
  });
  await assert.rejects(
    () => overflowClient.submitBundle(request),
    /atomic private settlement response is invalid/u,
  );
});

test("role and public queries use disjoint authentication policies", async () => {
  const payloadPath = fixture.identifiers.payload_hex;
  const bundlePath = fixture.identifiers.bundle_hex;
  const seen = [];
  const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    networkId: attestationNetworkId(),
    nativeVerifier: acceptingNativeVerifier(),
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
  const approval = auditApprovalRequest();
  await client.submitAuditApproval(payloadPath, approval, {
    roleHeaderProvider: roleHeaders,
  });
  await client.getBundleStatus(bundlePath);
  await client.getBundleReceipt(bundlePath);
  assert.equal(seen[0].headers["x-iroha-operator-signature"], "signature");
  assert.equal("x-iroha-account" in seen[1].headers, false);
  assert.equal("x-iroha-operator-signature" in seen[1].headers, false);
  assert.equal("x-iroha-account" in seen[2].headers, false);

  const wrongStatusClient = new AtomicPrivateSettlementToriiClientV1(
    "https://torii.example",
    {
      async fetchImpl(target) {
        return response(fixture.responses.receipt_pending, target, { status: 201 });
      },
    },
  );
  await assert.rejects(
    () => wrongStatusClient.getBundleReceipt(bundlePath),
    /response status is invalid/u,
  );
});

test("auditor capsule requires one exact nonzero authoritative height", async () => {
  const valid = fixture.responses.auditor_capsule;
  assert.notEqual(attestationNetworkId(), fixture.identifiers.payload_json);
  const nativeCalls = { committee: [], capsule: [], approval: [] };
  const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    networkId: attestationNetworkId(),
    nativeVerifier: acceptingNativeVerifier(nativeCalls),
    async fetchImpl(target) {
      return response(valid, target);
    },
  });
  const received = await client.getAuditorCapsule(fixture.identifiers.payload_hex, {
    roleHeaderProvider: roleHeaders,
  });
  assert.deepEqual(
    JSON.parse(new TextDecoder().decode(received.bytes())),
    valid,
  );
  assert.equal(nativeCalls.capsule.length, 1);
  assert.deepEqual(
    nativeCalls.capsule[0].map((value) => (
      value instanceof Uint8Array ? Buffer.from(value).toString("hex") : value
    )),
    [
      Buffer.from(jsonBytes(valid)).toString("hex"),
      attestationNetworkId().slice(5, 69).toLowerCase(),
      fixture.identifiers.payload_hex,
      roleHeaders()["x-iroha-operator-public-key"],
    ],
  );

  const invalidHeights = [true, 0, -1, 1.5, "105", 18446744073709551616n];
  for (const authoritativeHeight of invalidHeights) {
    const invalid = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
      networkId: attestationNetworkId(),
      nativeVerifier: acceptingNativeVerifier(),
      async fetchImpl(target) {
        const body = { ...valid, authoritative_height: authoritativeHeight };
        if (typeof authoritativeHeight === "bigint") {
          const encoded = JSON.stringify(valid).replace(
            '"authoritative_height":105',
            `"authoritative_height":${authoritativeHeight}`,
          );
          return response(new TextEncoder().encode(encoded), target);
        }
        return response(body, target);
      },
    });
    await assert.rejects(
      () => invalid.getAuditorCapsule(fixture.identifiers.payload_hex, {
        roleHeaderProvider: roleHeaders,
      }),
      /atomic private settlement response is invalid/u,
    );
  }
});

test("auditor capsule attestation rejects substitutions and malformed scalar types", async () => {
  const valid = fixture.responses.auditor_capsule;
  const candidates = [];
  for (const [field, replacement] of [
    ["network_id", fixture.identifiers.payload_json],
    ["payload_digest", fixture.identifiers.bundle_json],
    ["view_digest", fixture.identifiers.payload_hex],
    ["authority_digest", fixture.identifiers.payload_hex],
    ["responder", ""],
    ["version", true],
    ["lifecycle_code", true],
  ]) {
    const candidate = cloneJson(valid);
    candidate.responder_attestation.body[field] = replacement;
    candidates.push(candidate);
  }

  const wrongSignature = cloneJson(valid);
  wrongSignature.responder_attestation.signature = "AQ==";
  candidates.push(wrongSignature);

  const booleanHeight = cloneJson(valid);
  booleanHeight.authoritative_height = 1;
  booleanHeight.responder_attestation.body.authoritative_height = true;
  candidates.push(booleanHeight);

  const manifestNetwork = cloneJson(valid);
  manifestNetwork.manifest.network_id = fixture.identifiers.payload_json;
  candidates.push(manifestNetwork);

  for (const candidate of candidates) {
    const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
      networkId: attestationNetworkId(),
      nativeVerifier: acceptingNativeVerifier(),
      async fetchImpl(target) {
        return response(candidate, target);
      },
    });
    await assert.rejects(
      () => client.getAuditorCapsule(fixture.identifiers.payload_hex, {
        roleHeaderProvider: roleHeaders,
      }),
      /atomic private settlement response is invalid/u,
    );
  }

  const wrongContext = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    networkId: fixture.identifiers.payload_json,
    nativeVerifier: acceptingNativeVerifier(),
    async fetchImpl(target) {
      return response(valid, target);
    },
  });
  await assert.rejects(
    () => wrongContext.getAuditorCapsule(fixture.identifiers.payload_hex, {
      roleHeaderProvider: roleHeaders,
    }),
    /response is invalid/u,
  );

  let fetched = false;
  const unconfigured = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    async fetchImpl() {
      fetched = true;
      throw new Error("must not fetch");
    },
  });
  await assert.rejects(
    () => unconfigured.getAuditorCapsule(fixture.identifiers.payload_hex, {
      roleHeaderProvider: roleHeaders,
    }),
    /configured settlement networkId/u,
  );
  assert.equal(fetched, false);
});

test("approval acknowledgement binds the prepared request and rejects substitutions", async () => {
  const valid = fixture.responses.audit_approval;
  const nativeCalls = { committee: [], capsule: [], approval: [] };
  const validClient = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    networkId: attestationNetworkId(),
    nativeVerifier: acceptingNativeVerifier(nativeCalls),
    async fetchImpl(target) {
      return response(valid, target);
    },
  });
  const request = auditApprovalRequest();
  const received = await validClient.submitAuditApproval(
    fixture.identifiers.payload_hex,
    request,
    { roleHeaderProvider: roleHeaders },
  );
  assert.deepEqual(JSON.parse(new TextDecoder().decode(received.bytes())), valid);
  assert.equal(nativeCalls.approval.length, 1);
  assert.deepEqual(
    nativeCalls.approval[0].map((value) => (
      value instanceof Uint8Array ? Buffer.from(value).toString("hex") : value
    )),
    [
      Buffer.from(jsonBytes(valid)).toString("hex"),
      Buffer.from(request.bytes()).toString("hex"),
      attestationNetworkId().slice(5, 69).toLowerCase(),
      fixture.identifiers.payload_hex,
      roleHeaders()["x-iroha-operator-public-key"],
    ],
  );

  const candidates = [];
  for (const [field, replacement] of [
    ["network_id", fixture.identifiers.payload_json],
    ["payload_digest", fixture.identifiers.bundle_json],
    ["approval_digest", fixture.identifiers.payload_hex],
    ["acknowledgement_digest", fixture.identifiers.payload_hex],
    ["authority_digest", fixture.identifiers.payload_hex],
    ["responder", ""],
    ["version", true],
    ["lifecycle_code", true],
  ]) {
    const candidate = cloneJson(valid);
    candidate.responder_attestation.body[field] = replacement;
    candidates.push(candidate);
  }

  const wrongSignature = cloneJson(valid);
  wrongSignature.responder_attestation.signature = "AQ==";
  candidates.push(wrongSignature);

  const booleanHeight = cloneJson(valid);
  booleanHeight.authoritative_height = 1;
  booleanHeight.responder_attestation.body.authoritative_height = true;
  candidates.push(booleanHeight);

  for (const [field, replacement] of [
    ["bundle_id", fixture.identifiers.payload_json],
    ["payload_digest", fixture.identifiers.bundle_json],
    ["leg_ordinal", true],
    ["leg_ordinal", 255],
    ["collected", true],
    ["required", true],
    ["newly_recorded", 1],
  ]) {
    const candidate = cloneJson(valid);
    candidate[field] = replacement;
    candidates.push(candidate);
  }

  const wrongDataspace = cloneJson(valid);
  wrongDataspace.committee_authority.route.dataspace_id = 8;
  candidates.push(wrongDataspace);

  const expired = cloneJson(valid);
  expired.authoritative_height = 201;
  expired.responder_attestation.body.authoritative_height = 201;
  candidates.push(expired);

  for (const candidate of candidates) {
    const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
      networkId: attestationNetworkId(),
      nativeVerifier: acceptingNativeVerifier(),
      async fetchImpl(target) {
        return response(candidate, target);
      },
    });
    await assert.rejects(
      () => client.submitAuditApproval(
        fixture.identifiers.payload_hex,
        auditApprovalRequest(),
        { roleHeaderProvider: roleHeaders },
      ),
      /atomic private settlement response is invalid/u,
    );
  }

  let fetched = false;
  const mismatchedRequest = new AtomicPrivateSettlementToriiClientV1(
    "https://torii.example",
    {
      networkId: attestationNetworkId(),
      async fetchImpl() {
        fetched = true;
        throw new Error("must not fetch");
      },
    },
  );
  await assert.rejects(
    () => mismatchedRequest.submitAuditApproval(
      fixture.identifiers.payload_hex,
      auditApprovalRequest(fixture.identifiers.payload_json),
      { roleHeaderProvider: roleHeaders },
    ),
    /differs from the configured networkId/u,
  );
  assert.equal(fetched, false);
});

test("committee proof is network-bound and restricted routes fail closed natively", async () => {
  const committeeProof = {
    manifest: {},
    audit_policy: {},
    committee_authority: {},
    statement: {},
    proof: "AQ==",
    delta: {},
    audit_approvals: [],
    audit_capsule_digest: fixture.identifiers.payload_json,
    availability: {},
    lifecycle: { status: "collecting", value: null },
  };
  const nativeCalls = { committee: [], capsule: [], approval: [] };
  const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    networkId: attestationNetworkId(),
    nativeVerifier: acceptingNativeVerifier(nativeCalls),
    async fetchImpl(target) {
      return response(committeeProof, target);
    },
  });

  const received = await client.getCommitteeProof(
    fixture.identifiers.payload_hex,
    { roleHeaderProvider: roleHeaders },
  );
  assert.deepEqual(JSON.parse(new TextDecoder().decode(received.bytes())), committeeProof);
  assert.equal(nativeCalls.committee.length, 1);
  assert.deepEqual(
    nativeCalls.committee[0].map((value) => Buffer.from(value).toString("hex")),
    [
      Buffer.from(jsonBytes(committeeProof)).toString("hex"),
      attestationNetworkId().slice(5, 69).toLowerCase(),
      fixture.identifiers.payload_hex,
    ],
  );

  let fetched = false;
  const missing = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    networkId: attestationNetworkId(),
    nativeVerifier: {},
    async fetchImpl() {
      fetched = true;
      throw new Error("must not fetch");
    },
  });
  await assert.rejects(
    () => missing.getCommitteeProof(
      fixture.identifiers.payload_hex,
      { roleHeaderProvider: roleHeaders },
    ),
    /restricted response verifier is unavailable/u,
  );
  assert.equal(fetched, false);

  const rejecting = acceptingNativeVerifier();
  rejecting.privateSettlementVerifyAuditorCapsuleResponseV1 = () => {
    throw new Error("LEAK_CANARY_NATIVE_RESPONSE");
  };
  const rejected = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    networkId: attestationNetworkId(),
    nativeVerifier: rejecting,
    async fetchImpl(target) {
      return response(fixture.responses.auditor_capsule, target);
    },
  });
  await assert.rejects(
    () => rejected.getAuditorCapsule(
      fixture.identifiers.payload_hex,
      { roleHeaderProvider: roleHeaders },
    ),
    (error) => {
      assert.equal(error.message, "atomic private settlement response is invalid");
      assert.doesNotMatch(String(error), /LEAK_CANARY_NATIVE_RESPONSE/u);
      return true;
    },
  );
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

test("HTTP failures redact bodies and untrusted reject codes", async () => {
  const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    async fetchImpl(target) {
      return response(
        { memo: "LEAK_CANARY", amount: 987654 },
        target,
        {
          ok: false,
          status: 400,
          headers: { "x-iroha-reject-code": "memo=LEAK_CANARY_987654" },
        },
      );
    },
  });
  await assert.rejects(
    () => client.getBundleStatus(fixture.identifiers.bundle_hex),
    (error) => {
      assert.match(error.message, /failed with HTTP 400/u);
      assert.doesNotMatch(error.message, /LEAK_CANARY/u);
      assert.doesNotMatch(error.message, /987654/u);
      return true;
    },
  );

  const validCode = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    async fetchImpl(target) {
      return response(
        { memo: "LEAK_CANARY" },
        target,
        {
          ok: false,
          status: 409,
          headers: { "x-iroha-reject-code": "APS_POLICY_DENIED" },
        },
      );
    },
  });
  await assert.rejects(
    () => validCode.getBundleStatus(fixture.identifiers.bundle_hex),
    /reject_code=APS_POLICY_DENIED/u,
  );
});

test("invalid responses discard secret-bearing parser causes", async () => {
  const canary = "LEAK_CANARY_ACCOUNT_AMOUNT";
  const duplicate = textEncoder.encode(`{"${canary}":1,"${canary}":2}`);
  const client = new AtomicPrivateSettlementToriiClientV1("https://torii.example", {
    async fetchImpl(target) {
      return response(duplicate, target);
    },
  });

  await assert.rejects(
    () => client.getBundleStatus(fixture.identifiers.bundle_hex),
    (error) => {
      assert.equal(error.message, "atomic private settlement response is invalid");
      assert.equal(error.cause, undefined);
      assert.doesNotMatch(String(error.stack ?? error), /LEAK_CANARY_ACCOUNT_AMOUNT/u);
      return true;
    },
  );
});
