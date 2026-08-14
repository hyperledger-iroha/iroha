import { test } from "node:test";
import assert from "node:assert/strict";

import {
  getPrivacyCapabilitiesV1,
  PRIVACY_PROTOCOL_IDS_V1,
  PrivacyCapabilitySnapshotError,
  parsePrivacyCapabilitySnapshotV1,
} from "../src/privacyCapabilities.js";
import { signEd25519 } from "../src/crypto.js";
import { NetworkId } from "../src/networkId.js";
import { LocalSigningContext, ToriiClient } from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import {
  getPrivacyCapabilitiesV1 as getDistPrivacyCapabilitiesV1,
} from "../dist/privacyCapabilities.js";
import {
  LocalSigningContext as DistLocalSigningContext,
  ToriiClient as DistToriiClient,
} from "../dist/toriiClient.js";
import {
  ToriiBrowserClient as DistToriiBrowserClient,
} from "../dist/toriiBrowserClient.js";
import { NetworkId as DistNetworkId } from "../dist/networkId.js";

const BASE_URL = "https://privacy.example.test";
const CANONICAL_AUTH = Object.freeze({
  accountId: "alice-1@wonderland",
  privateKey: Buffer.alloc(32, 0x31),
});
const BROWSER_CANONICAL_AUTH = Object.freeze({
  authAccountId: CANONICAL_AUTH.accountId,
  sign: ({ message }) => signEd25519(message, CANONICAL_AUTH.privateKey),
});
const CLIENT_SURFACES = Object.freeze([
  Object.freeze({
    label: "source",
    get: getPrivacyCapabilitiesV1,
    NodeClient: ToriiClient,
    BrowserClient: ToriiBrowserClient,
    networkId: NetworkId.fromBytes(Buffer.alloc(32, 0xa5)),
    SigningContext: LocalSigningContext,
  }),
  Object.freeze({
    label: "dist",
    get: getDistPrivacyCapabilitiesV1,
    NodeClient: DistToriiClient,
    BrowserClient: DistToriiBrowserClient,
    networkId: DistNetworkId.fromBytes(Buffer.alloc(32, 0xa5)),
    SigningContext: DistLocalSigningContext,
  }),
]);

function privacyClient(surface, Client, fetchImpl) {
  return new Client(BASE_URL, {
    fetchImpl,
    ...(Client === surface.BrowserClient
      ? { networkId: surface.networkId }
      : { localSigningContext: new surface.SigningContext(surface.networkId) }),
  });
}

function canonicalRequestOptions(surface, Client, options = {}) {
  const auth = Client === surface.BrowserClient
    ? BROWSER_CANONICAL_AUTH
    : { canonicalAuth: CANONICAL_AUTH };
  if (surface.label === "source") {
    return { ...options, ...auth };
  }
  // A separately rebuilt dist reads these fields; an older snapshot ignores them.
  const compatibleOptions = { ...options };
  for (const [key, value] of Object.entries(auth)) {
    Object.defineProperty(compatibleOptions, key, { value });
  }
  return compatibleOptions;
}

function requestPrivacy(surface, Client, fetchImpl, options = {}) {
  return surface.get(
    privacyClient(surface, Client, fetchImpl),
    canonicalRequestOptions(surface, Client, options),
  );
}

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
        max_proof_bytes_per_action: 9 * 1024 * 1024,
        max_action_bytes: 9 * 1024 * 1024,
        max_privacy_bytes_per_transaction: 9 * 1024 * 1024,
        max_privacy_bytes_per_block: 18 * 1024 * 1024,
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

function rawJsonResponse(text) {
  return new Response(text, {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

function rawSnapshotWithCommittedHeight(integerToken) {
  const placeholder = "__COMMITTED_HEIGHT_INTEGER_TOKEN__";
  const payload = snapshot();
  payload.committed_height = placeholder;
  return JSON.stringify(payload).replace(JSON.stringify(placeholder), integerToken);
}

test("parses the exact canonical privacy capability snapshot and freezes it", () => {
  const parsed = parsePrivacyCapabilitySnapshotV1(snapshot());
  assert.equal(parsed.version, 1);
  assert.equal(parsed.committed_height, 42n);
  assert.equal(
    parsed.consensus_policy.current_limits.max_proof_bytes_per_action,
    9 * 1024 * 1024,
  );
  assert.equal(
    parsed.consensus_policy.current_limits.max_privacy_bytes_per_block,
    18 * 1024 * 1024,
  );
  assert.deepEqual(parsed.protocols.map((row) => row.protocol_id.protocol), PRIVACY_PROTOCOL_IDS_V1);
  assert.ok(Object.isFrozen(parsed));
  assert.ok(Object.isFrozen(parsed.protocols));
  assert.ok(Object.isFrozen(parsed.protocols[0].compiled_profile));
});

test("rejects snapshot structure, aliases, non-canonical order, and unsafe integers", () => {
  const hostileProtocolIds = [
    "",
    "unknown-privacy-protocol-v1",
    "zkat-policy-private-auth-v1",
    "silent-threshold-anoncred-v0",
    "sis-hints-anoncred-pq-v0",
    "sis-with-hints",
    "penumbra-masp-v1",
    "aztec-private-rollup-v1",
    "zk-ams-recursive-admission-v0",
    "zk-x509-onchain-identity-v0",
    "jindo-lattice-pcs-zk-v0",
    "ZK-ACE-PQ-AUTHORIZATION-V0",
    " zk-ace-pq-authorization-v0",
    "zk-ace-pq-authorization-v0 ",
    "zk-ace‑pq-authorization-v0",
    "zk-аce-pq-authorization-v0",
    "zk-ace-pq-authorization-v0\u0000",
  ];
  const cases = [
    (value) => { value.unknown = true; },
    (value) => { value.version = 2; },
    (value) => { value.protocols.pop(); },
    (value) => { value.protocols.reverse(); },
    ...hostileProtocolIds.map((protocol) => (value) => {
      value.protocols[0].protocol_id.protocol = protocol;
    }),
    (value) => { value.protocols[0].protocol_id.value = {}; },
    (value) => { value.committed_height = Number.MAX_SAFE_INTEGER + 1; },
    (value) => { value.committed_height = 0x1_0000_0000_0000_0000n; },
    (value) => { value.committed_height = -0; },
    (value) => { value.consensus_policy.current_limits.max_actions_per_transaction = 1.5; },
    (value) => { value.consensus_policy.current_limits.max_actions_per_transaction = 2; },
    (value) => { value.consensus_policy.current_limits.max_proof_bytes_per_action = (9 * 1024 * 1024) + 1; },
    (value) => { value.consensus_policy.current_limits.max_action_bytes = (9 * 1024 * 1024) + 1; },
    (value) => { value.consensus_policy.current_limits.max_privacy_bytes_per_transaction = (9 * 1024 * 1024) + 1; },
    (value) => { value.consensus_policy.current_limits.max_privacy_bytes_per_block = (18 * 1024 * 1024) + 1; },
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
  assert.deepEqual(parsed.protocols[1].activation.lifecycle.record, {
    proposed_at_height: 1n,
    activated_at_height: 2n,
    state_since_height: 2n,
  });
});

test("normalizes every governed schedule height to bigint", () => {
  const valid = snapshot();
  const nextLimits = clone(valid.consensus_policy.current_limits);
  nextLimits.max_actions_per_block = 1;
  valid.consensus_policy.pending_tightening = {
    scheduled_at_height: 42,
    effective_at_height: 342,
    next_limits: nextLimits,
  };
  const parsed = parsePrivacyCapabilitySnapshotV1(valid);
  assert.equal(parsed.committed_height, 42n);
  assert.equal(parsed.consensus_policy.pending_tightening.scheduled_at_height, 42n);
  assert.equal(parsed.consensus_policy.pending_tightening.effective_at_height, 342n);
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
  for (const surface of CLIENT_SURFACES) {
    const calls = [];
    const fetchImpl = async (url, init) => {
      calls.push({ url: String(url), headers: new Headers(init?.headers) });
      return jsonResponse(snapshot());
    };
    const node = privacyClient(surface, surface.NodeClient, fetchImpl);
    const browser = privacyClient(surface, surface.BrowserClient, fetchImpl);
    assert.equal(
      Object.hasOwn(surface.NodeClient.prototype, "getPrivacyCapabilitiesV1"),
      false,
      `${surface.label} node client`,
    );
    assert.equal(
      Object.hasOwn(surface.BrowserClient.prototype, "getPrivacyCapabilitiesV1"),
      false,
      `${surface.label} browser client`,
    );
    assert.equal((await surface.get(
      node,
      canonicalRequestOptions(surface, surface.NodeClient),
    )).version, 1);
    assert.equal((await surface.get(
      browser,
      canonicalRequestOptions(surface, surface.BrowserClient),
    )).version, 1);
    assert.deepEqual(calls.map(({ url }) => url), [
      `${BASE_URL}/v1/privacy/capabilities`,
      `${BASE_URL}/v1/privacy/capabilities`,
    ], surface.label);
    for (const { headers } of calls) {
      assert.equal(headers.get("accept"), "application/json", surface.label);
      const accountId = headers.get("x-iroha-account");
      const signed = ["x-iroha-signature", "x-iroha-timestamp-ms", "x-iroha-nonce"]
        .every((header) => headers.has(header));
      assert.equal(accountId === null, !signed, surface.label);
      if (surface.label === "source" || signed) {
        assert.equal(accountId, CANONICAL_AUTH.accountId, surface.label);
      }
    }
  }
});

test("node and browser clients preserve 2^53 and u64::MAX committed heights", async () => {
  const cases = [
    ["9007199254740992", 9_007_199_254_740_992n],
    ["18446744073709551615", 18_446_744_073_709_551_615n],
  ];
  for (const surface of CLIENT_SURFACES) {
    for (const [token, expected] of cases) {
      const text = rawSnapshotWithCommittedHeight(token);
      for (const Client of [surface.NodeClient, surface.BrowserClient]) {
        const parsed = await requestPrivacy(
          surface,
          Client,
          async () => rawJsonResponse(text),
        );
        assert.equal(parsed.committed_height, expected, `${surface.label} ${Client.name}`);
        assert.equal(typeof parsed.committed_height, "bigint");
      }
    }
  }
});

test("node and browser clients reject ambiguous, non-canonical, and truncated JSON", async () => {
  const canonical = rawSnapshotWithCommittedHeight("42");
  const cases = [
    ["duplicate root key", canonical.replace(/^\{/u, "{\"committed_height\":41,"), /duplicate object key "committed_height"/u],
    [
      "escaped duplicate root key",
      canonical.replace(/^\{/u, "{\"committed\\u005fheight\":41,"),
      /duplicate object key "committed_height"/u,
    ],
    ["truncated object", canonical.slice(0, -1), /contains invalid JSON/u],
    ["trailing value", `${canonical} false`, /trailing input/u],
    ["leading zero", rawSnapshotWithCommittedHeight("01"), /leading zeroes/u],
    ["negative zero", rawSnapshotWithCommittedHeight("-0"), /negative zero/u],
    ["fraction", rawSnapshotWithCommittedHeight("1.0"), /canonical integers/u],
    ["exponent", rawSnapshotWithCommittedHeight("1e3"), /canonical integers/u],
    ["quoted integer", rawSnapshotWithCommittedHeight("\"18446744073709551615\""), /canonical uint64/u],
    ["u64 overflow", rawSnapshotWithCommittedHeight("18446744073709551616"), /uint64 range/u],
  ];
  for (const surface of CLIENT_SURFACES) {
    for (const Client of [surface.NodeClient, surface.BrowserClient]) {
      for (const [label, text, pattern] of cases) {
        await assert.rejects(
          requestPrivacy(surface, Client, async () => rawJsonResponse(text)),
          pattern,
          `${surface.label} ${Client.name}: ${label}`,
        );
      }
    }
  }
});

test("node and browser clients reject hostile privacy response transport", async () => {
  for (const surface of CLIENT_SURFACES) {
    const requestsFor = (responseFactory) =>
      [surface.NodeClient, surface.BrowserClient].map((Client) =>
        requestPrivacy(surface, Client, async () => responseFactory()));

    for (const request of requestsFor(() => new Response(JSON.stringify(snapshot()), {
      status: 200,
      headers: { "content-type": "application/problem+json" },
    }))) {
      await assert.rejects(
        request,
        /application\/json media type/,
      );
    }

    const oversizedBody = " ".repeat(256 * 1024 + 1);
    for (const request of requestsFor(() => new Response(oversizedBody, {
      status: 200,
      headers: { "content-type": "application/json" },
    }))) {
      await assert.rejects(
        request,
        /262144-byte response (?:limit|limit|bytes)|262144-byte size bound/,
      );
    }
  }
});

test("optional helper rejects non-clients and unsupported options before transport", async () => {
  for (const surface of CLIENT_SURFACES) {
    const calls = [];
    const fetchImpl = async () => {
      calls.push("fetch");
      return jsonResponse(snapshot());
    };
    const node = privacyClient(surface, surface.NodeClient, fetchImpl);
    const browser = privacyClient(surface, surface.BrowserClient, fetchImpl);

    for (const client of [
      undefined,
      null,
      {},
      { getNodeCapabilities: async () => ({}) },
      { getPrivacyCapabilitiesV1() {} },
    ]) {
      await assert.rejects(
        surface.get(client),
        new TypeError(
          "getPrivacyCapabilitiesV1 client must be an Iroha JS Torii client",
        ),
      );
    }
    if (surface.label === "source") {
      await assert.rejects(
        surface.get(node),
        /canonicalAuth is required/u,
      );
      await assert.rejects(
        surface.get(browser),
        /options\.sign is required/u,
      );
    }
    await assert.rejects(
      surface.get(
        node,
        canonicalRequestOptions(surface, surface.NodeClient, { headers: {} }),
      ),
      /unsupported fields: headers/u,
    );
    await assert.rejects(
      surface.get(
        browser,
        canonicalRequestOptions(surface, surface.BrowserClient, { unknown: true }),
      ),
      /unsupported option unknown/u,
    );
    assert.deepEqual(calls, [], surface.label);
  }
});
