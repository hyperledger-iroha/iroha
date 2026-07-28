import { test } from "node:test";
import assert from "node:assert/strict";

import {
  PRIVACY_PROTOCOL_IDS_V1,
  PrivacyCapabilitySnapshotError,
  parsePrivacyCapabilitySnapshotV1,
} from "../src/privacyCapabilities.js";
import { ToriiClient } from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";

const BASE_URL = "https://privacy.example.test";

function tagged(protocol) {
  return { protocol, value: null };
}

function snapshot() {
  return {
    version: 1,
    committed_height: 42,
    consensus_policy: {
      current_limits: {
        max_actions_per_transaction: 1,
        max_actions_per_block: 2,
        max_proof_bytes_per_action: 8 * 1024 * 1024,
        max_action_bytes: 8 * 1024 * 1024,
        max_privacy_bytes_per_transaction: 8 * 1024 * 1024,
        max_privacy_bytes_per_block: 16 * 1024 * 1024,
        max_statement_and_encrypted_output_bytes_per_transaction: 256 * 1024,
        max_nullifiers_per_action: 8,
        max_commitments_per_action: 8,
        retained_root_count: 2048,
      },
      pending_tightening: null,
    },
    protocols: PRIVACY_PROTOCOL_IDS_V1.map((protocol) => ({
      protocol_id: tagged(protocol),
      compiled_profile: {
        status: "unavailable",
        value: { reason: "engine-unavailable", detail: null },
      },
      activation: null,
    })),
  };
}

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function jsonResponse(payload) {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

test("parses the exact canonical privacy capability snapshot and freezes it", () => {
  const parsed = parsePrivacyCapabilitySnapshotV1(snapshot());
  assert.equal(parsed.version, 1);
  assert.equal(parsed.committed_height, 42);
  assert.deepEqual(parsed.protocols.map((row) => row.protocol_id.protocol), PRIVACY_PROTOCOL_IDS_V1);
  assert.ok(Object.isFrozen(parsed));
  assert.ok(Object.isFrozen(parsed.protocols));
  assert.ok(Object.isFrozen(parsed.protocols[0].compiled_profile));
});

test("rejects snapshot structure, aliases, non-canonical order, and unsafe integers", () => {
  const cases = [
    (value) => { value.unknown = true; },
    (value) => { value.version = 2; },
    (value) => { value.protocols.pop(); },
    (value) => { value.protocols.reverse(); },
    (value) => { value.protocols[0].protocol_id.protocol = "ZK-ACE"; },
    (value) => { value.protocols[0].protocol_id.value = {}; },
    (value) => { value.committed_height = Number.MAX_SAFE_INTEGER + 1; },
    (value) => { value.consensus_policy.current_limits.max_actions_per_transaction = 1.5; },
    (value) => { value.consensus_policy.current_limits.max_actions_per_transaction = 2; },
  ];
  for (const mutate of cases) {
    const hostile = snapshot();
    mutate(hostile);
    assert.throws(() => parsePrivacyCapabilitySnapshotV1(hostile), PrivacyCapabilitySnapshotError);
  }
});

test("rejects nested unknown fields, malformed fixed bindings, and cross-protocol substitutions", () => {
  const profile = {
    protocol_id: tagged("anonymous-pgc-k-out-of-n-v1"),
    proof_system_id: { proof_system: "anonymous-pgc-p256", value: null },
    engine_id: { engine: "native-anonymous-pgc-p256", value: null },
    parameter_id: Array.from({ length: 32 }, () => 1),
    parameter_digest: Array.from({ length: 32 }, () => 2),
    verifier_digest: Array.from({ length: 32 }, () => 3),
    statement_schema_digest: Array.from({ length: 32 }, () => 4),
    engine_manifest_digest: Array.from({ length: 32 }, () => 5),
    protocol_limits: {
      protocol: "anonymous-pgc-k-out-of-n-v1",
      limits: { max_anonymity_set_size: 64, max_recipient_count: 8 },
    },
  };
  const cases = [
    (value) => { value.protocols[1].compiled_profile = { status: "available", value: { ...profile, extra: true } }; },
    (value) => { value.protocols[1].compiled_profile = { status: "available", value: { ...profile, parameter_id: [1] } }; },
    (value) => { value.protocols[1].compiled_profile = { status: "available", value: { ...profile, verifier_digest: Array(32).fill(0) } }; },
    (value) => { value.protocols[1].compiled_profile = { status: "available", value: { ...profile, proof_system_id: { proof_system: "iroha-verange-p256", value: null } } }; },
    (value) => { value.protocols[1].compiled_profile = { status: "available", value: { ...profile, protocol_limits: { protocol: "iroha-zk-ams-v1", limits: null } } }; },
    (value) => { value.protocols[1].compiled_profile = { status: "unavailable", value: { reason: "engine-unavailable", detail: null, extra: true } }; },
  ];
  for (const mutate of cases) {
    const hostile = snapshot();
    mutate(hostile);
    assert.throws(() => parsePrivacyCapabilitySnapshotV1(hostile), PrivacyCapabilitySnapshotError);
  }
});

test("accepts a fully bound active profile only when all governed bindings match", () => {
  const valid = snapshot();
  const profile = {
    protocol_id: tagged("anonymous-pgc-k-out-of-n-v1"),
    proof_system_id: { proof_system: "anonymous-pgc-p256", value: null },
    engine_id: { engine: "native-anonymous-pgc-p256", value: null },
    parameter_id: Array(32).fill(1), parameter_digest: Array(32).fill(2),
    verifier_digest: Array(32).fill(3), statement_schema_digest: Array(32).fill(4),
    engine_manifest_digest: Array(32).fill(5),
    protocol_limits: { protocol: "anonymous-pgc-k-out-of-n-v1", limits: { max_anonymity_set_size: 64, max_recipient_count: 8 } },
  };
  valid.protocols[1].compiled_profile = { status: "available", value: profile };
  valid.protocols[1].activation = {
    ...clone(profile),
    lifecycle: { state: "active", record: { proposed_at_height: 1, activated_at_height: 2, state_since_height: 2 } },
    pending_protocol_limits_tightening: null,
    assurance: { assurance: "experimental", value: null },
  };
  const parsed = parsePrivacyCapabilitySnapshotV1(valid);
  assert.equal(parsed.protocols[1].compiled_profile.status, "available");
  assert.equal(parsed.protocols[1].activation.lifecycle.state, "active");
});

test("rejects forged governance activation and malformed delayed transitions", () => {
  const hostile = snapshot();
  const profile = {
    protocol_id: tagged("anonymous-pgc-k-out-of-n-v1"),
    proof_system_id: { proof_system: "anonymous-pgc-p256", value: null },
    engine_id: { engine: "native-anonymous-pgc-p256", value: null },
    parameter_id: Array(32).fill(1), parameter_digest: Array(32).fill(2),
    verifier_digest: Array(32).fill(3), statement_schema_digest: Array(32).fill(4),
    engine_manifest_digest: Array(32).fill(5),
    protocol_limits: { protocol: "anonymous-pgc-k-out-of-n-v1", limits: { max_anonymity_set_size: 64, max_recipient_count: 8 } },
  };
  hostile.protocols[1].compiled_profile = { status: "available", value: profile };
  hostile.protocols[1].activation = {
    ...clone(profile),
    parameter_digest: Array(32).fill(9),
    lifecycle: { state: "active", record: { proposed_at_height: 1, activated_at_height: 2, state_since_height: 2 } },
    pending_protocol_limits_tightening: null,
    assurance: { assurance: "experimental", value: null },
  };
  assert.throws(() => parsePrivacyCapabilitySnapshotV1(hostile), /does not match compiled profile/);

  const schedule = snapshot();
  schedule.consensus_policy.pending_tightening = {
    scheduled_at_height: 42,
    effective_at_height: 342,
    next_limits: clone(schedule.consensus_policy.current_limits),
  };
  assert.throws(() => parsePrivacyCapabilitySnapshotV1(schedule), /strict tightening/);
});

test("node and browser clients request and validate the authoritative route", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url: String(url), accept: new Headers(init?.headers).get("accept") });
    return jsonResponse(snapshot());
  };
  const node = new ToriiClient(BASE_URL, { fetchImpl });
  const browser = new ToriiBrowserClient(BASE_URL, { fetchImpl });
  assert.equal((await node.getPrivacyCapabilitiesV1()).version, 1);
  assert.equal((await browser.getPrivacyCapabilitiesV1({
    headers: { Accept: "application/problem+json" },
  })).version, 1);
  assert.deepEqual(calls, [
    { url: `${BASE_URL}/v1/privacy/capabilities`, accept: "application/json" },
    { url: `${BASE_URL}/v1/privacy/capabilities`, accept: "application/json" },
  ]);
});

test("node and browser clients reject hostile privacy response transport", async () => {
  const clientsFor = (responseFactory) => [
    new ToriiClient(BASE_URL, { fetchImpl: async () => responseFactory() }),
    new ToriiBrowserClient(BASE_URL, { fetchImpl: async () => responseFactory() }),
  ];

  for (const client of clientsFor(() => new Response(JSON.stringify(snapshot()), {
    status: 200,
    headers: { "content-type": "application/problem+json" },
  }))) {
    await assert.rejects(
      client.getPrivacyCapabilitiesV1(),
      /application\/json media type/,
    );
  }

  const oversizedBody = " ".repeat(256 * 1024 + 1);
  for (const client of clientsFor(() => new Response(oversizedBody, {
    status: 200,
    headers: { "content-type": "application/json" },
  }))) {
    await assert.rejects(
      client.getPrivacyCapabilitiesV1(),
      /262144-byte response (?:limit|limit|bytes)|262144-byte size bound/,
    );
  }
});
