import assert from "node:assert/strict";
import fs from "node:fs";
import test from "node:test";

import { keccak_256 } from "@noble/hashes/sha3";

import { AccountAddress } from "../src/address.js";
import { blake2b256 } from "../src/blake2b.js";
import {
  SCCP_CODEC_CANONICAL_TEXT,
  SCCP_CODEC_EVM_ADDRESS20,
  SCCP_CODEC_KEYS,
  SCCP_CODEC_TRON_ADDRESS21,
  SCCP_NETWORK_PROFILES,
  SCCP_PAYLOAD_KINDS,
  normalizeBridgeMessageSubmitPayload,
  normalizeBridgeProofSubmitPayload,
  normalizeSccpBridgeSubmitResponse,
  normalizeSccpCapabilities,
  normalizeSccpCodecValue,
  normalizeSccpMessageBundle,
  normalizeSccpProofRequest,
  normalizeSccpRecentMessages,
  normalizeSccpRegistry,
  normalizeSccpRouteGovernanceAction,
  parseSccpBridgeSubmitResponseJson,
  parseSccpJsonObject,
  sccpSourceEventDigest,
} from "../src/sccp.js";
import { ToriiClient } from "../src/toriiClient.js";

const HASH = (byte) => byte.toString(16).padStart(2, "0").repeat(32);
const PREFIX_HASH = (byte) => `0x${HASH(byte)}`;
const UPPER = (byte, length) => byte.toString(16).padStart(2, "0").repeat(length).toUpperCase();
const PUBLIC_KEY = Uint8Array.from([
  0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
  0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
  0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
  0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
]);
const AUTHORITY = AccountAddress.fromAccount({ publicKey: PUBLIC_KEY }).toI105(753);
const MESSAGE_ID = HASH(0x11);

function b64(bytes) {
  return Buffer.from(bytes).toString("base64");
}

function network(profile) {
  return { network: profile.replaceAll("-", "_"), profile: null };
}

function lane(source = "bsc-mainnet") {
  return { source: network(source), target: network("sora-taira") };
}

function g1(x = 1, y = 2) {
  return { x: UPPER(x, 32), y: UPPER(y, 32) };
}

function g2(seed = 3) {
  return {
    x_c0: UPPER(seed, 32),
    x_c1: UPPER(seed + 1, 32),
    y_c0: UPPER(seed + 2, 32),
    y_c1: UPPER(seed + 3, 32),
  };
}

function verifyingKey() {
  const ic = { constant: g1() };
  for (let index = 0; index < 10; index += 1) ic[`signal_${index}`] = g1();
  return { version: 1, alpha1: g1(), beta2: g2(), gamma2: g2(), delta2: g2(), ic };
}

function verifyingKeyBytes(key) {
  const words = [];
  const addG1 = (point) => words.push(point.x, point.y);
  const addG2 = (point) => words.push(point.x_c0, point.x_c1, point.y_c0, point.y_c1);
  addG1(key.alpha1);
  addG2(key.beta2);
  addG2(key.gamma2);
  addG2(key.delta2);
  addG1(key.ic.constant);
  for (let index = 0; index < 10; index += 1) addG1(key.ic[`signal_${index}`]);
  return Uint8Array.from(Buffer.from(words.join(""), "hex"));
}

function keyHash(key) {
  return Buffer.from(keccak_256(verifyingKeyBytes(key))).toString("hex");
}

function capabilities() {
  return {
    version: 1,
    registry_revision: PREFIX_HASH(0x10),
    registry_path: "/v1/sccp/registry",
    message_bundle_path: "/v1/sccp/proofs/message/{message_id}",
    proof_request_path: "/v1/sccp/proof-requests/{message_id}",
    recent_messages_path: "/v1/sccp/messages/recent",
    proof_submit_path: "/v1/bridge/proofs/submit",
    native_message_submit_path: "/v1/bridge/messages",
  };
}

function governedRoute({ revision = 1, activation = "staged" } = {}) {
  const key = verifyingKey();
  const routeAddress = UPPER(0x31, 20);
  const routeCodeHash = UPPER(0x41, 32);
  return {
    lane_id: lane(),
    route_id: "taira_bsc_xor",
    asset_key: "xor",
    revision,
    activation: { activation, direction: null },
    source_identity: {
      lane: lane(),
      emitter: {
        emitter: "evm",
        identity: {
          address: routeAddress,
          runtime_code_hash: routeCodeHash,
          route_config_hash: UPPER(0x42, 32),
        },
      },
    },
    destination: {
      family: "evm",
      deployment: {
        token_address: UPPER(0x11, 20),
        token_code_hash: UPPER(0x21, 32),
        verifier_address: UPPER(0x12, 20),
        verifier_code_hash: UPPER(0x22, 32),
        verifying_key: key,
        verifier_key_hash: keyHash(key).toUpperCase(),
        route_address: routeAddress,
        route_code_hash: routeCodeHash,
        taira_to_token_multiplier: 1_000_000_000,
      },
    },
    settlement: {
      asset_definition_id: "6TEAJqbb8oEPmLncoNiMRbLEK6tw",
      custody_account_id: AUTHORITY,
      payload_amount_scale: 9,
    },
  };
}

function registry(routes = [governedRoute()]) {
  return {
    version: 1,
    lanes: [{ lane_id: lane(), native_trust_anchor: null, routes }],
  };
}

function messageBundle() {
  return {
    version: 1,
    commitment_root: PREFIX_HASH(0x51),
    commitment: { version: 1 },
    merkle_proof: { steps: [] },
    payload: { Transfer: { amount: "1" } },
    finality_proof: "0x0102",
  };
}

function proofRequest() {
  const key = verifyingKey();
  return {
    version: 1,
    backend: { backend: "evm_groth16_bn254_v1", family: null },
    source_network: network("sora-taira"),
    target_network: network("bsc-mainnet"),
    public_inputs: {
      version: 1,
      message_id: PREFIX_HASH(0x11),
      payload_hash: PREFIX_HASH(0x12),
      target_domain: 2,
      commitment_root: PREFIX_HASH(0x13),
      finality_height: "9",
      finality_block_hash: PREFIX_HASH(0x14),
    },
    verifying_key: key,
    verifier_key_hash: `0x${keyHash(key)}`,
    bundle_bytes: "0x0102",
    statement_hash: PREFIX_HASH(0x61),
    destination_binding_hash: PREFIX_HASH(0x62),
    route_configuration_hash: PREFIX_HASH(0x63),
    request_hash: PREFIX_HASH(0x64),
  };
}

function recentItem(height = 9, id = MESSAGE_ID) {
  return {
    height,
    message_id_hex: id,
    kind: "transfer",
    source_profile: "sora-taira",
    target_profile: "bsc-mainnet",
    destination_binding_hash: PREFIX_HASH(0x71),
    route_configuration_hash: PREFIX_HASH(0x72),
    target_domain: 2,
    asset_id: "xor",
    route_id: "taira_bsc_xor",
    recipient: null,
    amount: "1000",
    payload_projection: null,
    links: {
      bundle_path: `/v1/sccp/proofs/message/${id}`,
      proof_request_path: `/v1/sccp/proof-requests/${id}`,
    },
  };
}

function preparedResponse(overrides = {}) {
  const payload = Uint8Array.of(1, 2, 3, 4);
  const digest = Uint8Array.from(blake2b256(payload));
  digest[31] |= 1;
  return {
    submitted: false,
    payload_kind: "transfer",
    message_id_hex: MESSAGE_ID,
    backend: "bridge/sccp/native/bsc-parlia-v1",
    counterparty_domain: 2,
    counterparty_chain: "bsc-mainnet",
    manifest_hash_hex: HASH(0x31),
    range_start_height: 7,
    range_end_height: 9,
    creation_time_ms: 10,
    tx_hash_hex: null,
    transaction_payload_b64: b64(payload),
    signing_message_b64: b64(digest),
    ...overrides,
  };
}

test("closed SCCP inventory exposes only ETH, BSC, TRON and three exact codecs", async () => {
  assert.deepEqual(Object.keys(SCCP_NETWORK_PROFILES), [
    "sora-nexus",
    "sora-taira",
    "ethereum-mainnet",
    "ethereum-sepolia",
    "bsc-mainnet",
    "bsc-testnet",
    "tron-mainnet",
    "tron-nile",
    "tron-shasta",
  ]);
  assert.deepEqual(Object.keys(SCCP_CODEC_KEYS), ["1", "2", "5"]);
  assert.deepEqual(SCCP_PAYLOAD_KINDS, ["transfer"]);
  const exports = await import("../src/sccp.js");
  for (const retired of [
    "SCCP_DOMAIN_SOL",
    "SCCP_DOMAIN_TON",
    "SCCP_CODEC_SOLANA_PUBKEY32",
    "SCCP_CODEC_TON_ACCOUNT36",
    "SCCP_CODEC_SORA_ASSET_ID",
    "normalizeSccpProofManifests",
    "normalizeSccpSourceAdapterEngineDeployment",
  ]) {
    assert.equal(retired in exports, false, retired);
  }
});

test("closed codecs accept exact layouts and reject retired tags and textual aliases", () => {
  assert.deepEqual(normalizeSccpCodecValue(1, "merchant@taira"), new TextEncoder().encode("merchant@taira"));
  assert.equal(normalizeSccpCodecValue(2, new Uint8Array(20).fill(1)).length, 20);
  assert.equal(
    normalizeSccpCodecValue(5, Uint8Array.from([0x41, ...new Uint8Array(20).fill(2)])).length,
    21,
  );
  for (const [tag, value] of [
    [3, new Uint8Array(32).fill(1)],
    [4, new Uint8Array(36).fill(1)],
    [6, Uint8Array.of(1)],
    [2, `0x${"11".repeat(20)}`],
    [2, new Uint8Array(20)],
    [5, Uint8Array.from([0x42, ...new Uint8Array(20).fill(1)])],
    [1, " padded"],
  ]) assert.throws(() => normalizeSccpCodecValue(tag, value));
});

test("source-event digest matches all shared ETH/BSC/TRON vectors", () => {
  const fixture = JSON.parse(
    fs.readFileSync(new URL("../../../fixtures/sccp/native_transfer_event_v1.json", import.meta.url), "utf8"),
  );
  for (const vector of fixture.vectors) {
    assert.equal(
      sccpSourceEventDigest(vector.lane_hash_hex, vector.message_id_hex, vector.payload_hash_hex),
      vector.source_event_digest_hex,
    );
  }
  for (const roles of [
    ["00".repeat(32), HASH(2), HASH(3)],
    [HASH(1), HASH(1), HASH(3)],
    [`0x${HASH(1)}`, HASH(2), HASH(3)],
    ["ab".repeat(32).toUpperCase(), HASH(2), HASH(3)],
  ]) assert.throws(() => sccpSourceEventDigest(...roles));
});

test("capabilities require exact immutable paths and reject all retired discovery fields", () => {
  assert.equal(normalizeSccpCapabilities(capabilities()).proof_request_path, capabilities().proof_request_path);
  const mutations = [
    (value) => { value.registry_path = "/v1/sccp/manifests"; },
    (value) => { value.proof_request_path += "?network=bsc"; },
    (value) => { value.message_bundle_path = "/v1/sccp/proofs/message/{id}"; },
    (value) => { value.proof_artifact_path = "/v1/sccp/artifacts/message/{message_id}"; },
    (value) => { value.proof_job_path = "/v1/sccp/jobs/message/{message_id}"; },
    (value) => { value.outbound = {}; },
    (value) => { value.allow_unready = true; },
    (value) => { value.registry_revision = PREFIX_HASH(0); },
  ];
  for (const mutate of mutations) {
    const value = structuredClone(capabilities());
    mutate(value);
    assert.throws(() => normalizeSccpCapabilities(value));
  }
});

test("registry validates complete typed route identity and immutable key hash", () => {
  const parsed = normalizeSccpRegistry(registry());
  assert.equal(parsed.lanes.length, 1);
  assert.equal(Object.isFrozen(parsed.lanes[0]), true);
  const badHash = registry();
  badHash.lanes[0].routes[0].destination.deployment.verifier_key_hash = UPPER(0x99, 32);
  assert.throws(() => normalizeSccpRegistry(badHash), /verifier_key_hash/u);
  const alias = registry();
  alias.lanes[0].routes[0].destination.deployment.verifier_address =
    alias.lanes[0].routes[0].destination.deployment.token_address;
  assert.throws(() => normalizeSccpRegistry(alias), /reuses/u);
});

test("registry rejects retired families, browser metadata, duplicate lanes, and revision gaps", () => {
  const retired = registry();
  retired.lanes[0].lane_id.source = { network: "solana_mainnet_beta", profile: null };
  assert.throws(() => normalizeSccpRegistry(retired), /retired/u);
  const browser = registry();
  browser.lanes[0].routes[0].destination_browser_prover = { module_url: "https://invalid" };
  assert.throws(() => normalizeSccpRegistry(browser), /unknown or retired/u);
  const duplicate = registry();
  duplicate.lanes.push(structuredClone(duplicate.lanes[0]));
  assert.throws(() => normalizeSccpRegistry(duplicate), /duplicate lane/u);
  const gap = registry([governedRoute({ revision: 2 })]);
  assert.throws(() => normalizeSccpRegistry(gap), /start at one/u);
  const doubleLive = registry([
    governedRoute({ revision: 1, activation: "bidirectional" }),
    governedRoute({ revision: 2, activation: "bidirectional" }),
  ]);
  assert.throws(() => normalizeSccpRegistry(doubleLive), /multiple revisions/u);
});

test("route governance accepts only closed atomic actions and exact field names", () => {
  const remove = {
    action: "Remove",
    route: {
      lane_id: lane(),
      route_id: "taira_bsc_xor",
      asset_key: "xor",
      revision: 1,
    },
  };
  assert.equal(normalizeSccpRouteGovernanceAction(remove).action, "Remove");
  for (const value of [
    { ...remove, manifest: {} },
    { action: "UpsertManifest", route: {} },
    { action: "Remove", route: { ...remove.route, routeId: "alias" } },
    {
      action: "SetActivation",
      route: {
        key: remove.route,
        expected_current: { activation: "staged", direction: null },
        next: { activation: "paused", direction: null },
      },
    },
  ]) assert.throws(() => normalizeSccpRouteGovernanceAction(value));
});

test("recent discovery contains only exact bundle and proof-request links", () => {
  const parsed = normalizeSccpRecentMessages({ items: [recentItem(9), recentItem(8, HASH(0x12))] });
  assert.deepEqual(parsed.items.map(({ height }) => height), [9, 8]);
  const retired = recentItem();
  retired.links.artifact_path = `/v1/sccp/artifacts/message/${MESSAGE_ID}`;
  assert.throws(() => normalizeSccpRecentMessages({ items: [retired] }), /retired/u);
  const mismatch = recentItem();
  mismatch.links.proof_request_path = `/v1/sccp/proof-requests/${HASH(0x12)}`;
  assert.throws(() => normalizeSccpRecentMessages({ items: [mismatch] }), /exact message/u);
  const injection = recentItem();
  injection.links.bundle_path += "?allow_unready=true";
  assert.throws(() => normalizeSccpRecentMessages({ items: [injection] }));
  assert.throws(() => normalizeSccpRecentMessages({ items: [recentItem(8), recentItem(9)] }));
});

test("bundle and proof-request JSON enforce the closed transfer/Groth16 schema", () => {
  assert.equal(normalizeSccpMessageBundle(messageBundle()).version, 1);
  assert.equal(normalizeSccpProofRequest(proofRequest()).public_inputs.target_domain, 2);
  const retiredPayload = messageBundle();
  retiredPayload.payload = { Burn: {} };
  assert.throws(() => normalizeSccpMessageBundle(retiredPayload), /retired/u);
  const retiredBackend = proofRequest();
  retiredBackend.backend.backend = "solana_recursive_v1";
  assert.throws(() => normalizeSccpProofRequest(retiredBackend), /retired/u);
  const wrongFamily = proofRequest();
  wrongFamily.target_network = network("tron-mainnet");
  wrongFamily.public_inputs.target_domain = 5;
  assert.throws(() => normalizeSccpProofRequest(wrongFamily), /backend/u);
  const alias = proofRequest();
  alias.route_configuration_hash = alias.destination_binding_hash;
  assert.throws(() => normalizeSccpProofRequest(alias), /role-separated/u);
  const wrongKey = proofRequest();
  wrongKey.verifier_key_hash = PREFIX_HASH(0x99);
  assert.throws(() => normalizeSccpProofRequest(wrongKey), /does not match/u);
  const selector = proofRequest();
  selector.allow_unready = true;
  assert.throws(() => normalizeSccpProofRequest(selector), /retired/u);
});

test("submit DTOs expose only authority, optional signature, artifact, and positive timestamp", () => {
  const proof = normalizeBridgeProofSubmitPayload({
    authority: AUTHORITY,
    signature_b64: "AQ==",
    destination_proof_b64: "Ag==",
    creation_time_ms: 10,
  });
  assert.deepEqual(Object.keys(proof), [
    "authority",
    "signature_b64",
    "destination_proof_b64",
    "creation_time_ms",
  ]);
  assert.deepEqual(Object.keys(normalizeBridgeMessageSubmitPayload({
    authority: AUTHORITY,
    native_proof_b64: "Aw==",
  })), ["authority", "native_proof_b64"]);
});

test("submit DTOs reject redundant signers, caller-selected routes, bad base64, and bad time", () => {
  const proof = { authority: AUTHORITY, destination_proof_b64: "AQ==" };
  for (const [field, value] of [
    ["public_key_hex", HASH(1)],
    ["message_bundle_b64", "AQ=="],
    ["proof_bytes_hex", "01"],
    ["network_id_hex", HASH(2)],
    ["manifest_hash", HASH(3)],
    ["deployment", {}],
    ["allow_unready", true],
    ["signature", "AQ=="],
  ]) assert.throws(() => normalizeBridgeProofSubmitPayload({ ...proof, [field]: value }));
  for (const artifact of ["AQ", " AQ==", "AQ==\n", "", "====", "A==="]) {
    assert.throws(() => normalizeBridgeProofSubmitPayload({ ...proof, destination_proof_b64: artifact }));
  }
  for (const creation_time_ms of [0, -1, 1.5, Number.MAX_SAFE_INTEGER + 1, "1"]) {
    assert.throws(() => normalizeBridgeProofSubmitPayload({ ...proof, creation_time_ms }));
  }
});

test("bridge response and JSON parser reject contradictions, aliases, and duplicate fields", () => {
  assert.equal(normalizeSccpBridgeSubmitResponse(preparedResponse()).submitted, false);
  assert.equal(normalizeSccpBridgeSubmitResponse({
    ...preparedResponse(),
    submitted: true,
    tx_hash_hex: HASH(0x55),
    transaction_payload_b64: null,
    signing_message_b64: null,
  }).submitted, true);
  for (const value of [
    { ...preparedResponse(), payload_kind: "burn" },
    { ...preparedResponse(), counterparty_chain: "solana-mainnet-beta" },
    { ...preparedResponse(), proof_artifact_hash: HASH(3) },
    { ...preparedResponse(), creation_time_ms: 0 },
    { ...preparedResponse(), tx_hash_hex: HASH(4) },
    { ...preparedResponse(), signing_message_b64: b64(new Uint8Array(32).fill(9)) },
  ]) assert.throws(() => normalizeSccpBridgeSubmitResponse(value));
  const json = JSON.stringify(preparedResponse());
  assert.equal(parseSccpBridgeSubmitResponseJson(json).submitted, false);
  assert.throws(() => parseSccpBridgeSubmitResponseJson(json.replace("{", '{"submitted":false,')), /duplicate/u);
  assert.throws(() => parseSccpJsonObject(`${json}{}`), /trailing/u);
});

function response(value, { contentType = "application/json", bytes } = {}) {
  const body = bytes ?? Buffer.from(JSON.stringify(value), "utf8");
  return {
    status: 200,
    headers: new Headers({ "content-type": contentType }),
    async text() { return Buffer.from(body).toString("utf8"); },
    async arrayBuffer() { return body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength); },
  };
}

test("Torii exact client constructs fixed query-free endpoints and content negotiation", async () => {
  const observed = [];
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async (url, init) => {
      observed.push({ url: String(url), accept: init.headers.Accept });
      const path = new URL(url).pathname;
      if (path === "/v1/sccp/capabilities") return response(capabilities());
      if (path === "/v1/sccp/registry") return response({ version: 1, lanes: [] });
      if (path.includes("proof-requests")) {
        return init.headers.Accept === "application/x-norito"
          ? response(null, { contentType: "application/x-norito", bytes: Buffer.from([7, 8]) })
          : response(proofRequest());
      }
      if (path.includes("proofs/message")) return response(messageBundle());
      return response({ items: [] });
    },
  });
  assert.equal((await client.getSccpCapabilities()).version, 1);
  assert.equal((await client.getSccpRegistry()).version, 1);
  assert.equal((await client.getSccpMessageBundle(MESSAGE_ID)).version, 1);
  assert.deepEqual(await client.getSccpProofRequest(MESSAGE_ID, { format: "norito" }), Uint8Array.of(7, 8));
  assert.deepEqual((await client.getSccpRecentMessages({ from: 9, limit: 0 })).items, []);
  assert.deepEqual(observed.map(({ url }) => url), [
    "https://example.invalid/v1/sccp/capabilities",
    "https://example.invalid/v1/sccp/registry",
    `https://example.invalid/v1/sccp/proofs/message/${MESSAGE_ID}`,
    `https://example.invalid/v1/sccp/proof-requests/${MESSAGE_ID}`,
    "https://example.invalid/v1/sccp/messages/recent?from=9&limit=0",
  ]);
});

test("Torii exact client rejects path/query injection and retired option aliases before fetch", async () => {
  let calls = 0;
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async () => { calls += 1; return response({}); },
  });
  for (const id of [
    `0x${MESSAGE_ID}`,
    "ab".repeat(32).toUpperCase(),
    `${MESSAGE_ID}?network=bsc`,
    `${MESSAGE_ID}/../registry`,
    "00".repeat(32),
  ]) await assert.rejects(() => client.getSccpProofRequest(id));
  for (const options of [
    { network: "bsc-mainnet" },
    { allowUnready: true },
    { proofBytes: "01" },
    { format: "JSON" },
    { format: "artifact" },
  ]) await assert.rejects(() => client.getSccpProofRequest(MESSAGE_ID, options));
  for (const options of [
    { cursor: 1 },
    { from: -1 },
    { from: "1" },
    { limit: -1 },
    { limit: 51 },
  ]) await assert.rejects(() => client.getSccpRecentMessages(options));
  assert.equal(calls, 0);
  assert.equal(typeof client.getSccpProofManifests, "undefined");
});

test("Torii proof submit sends only the closed destination artifact DTO", async () => {
  let observed;
  const client = new ToriiClient("https://example.invalid", {
    fetchImpl: async (url, init) => {
      observed = { url: String(url), body: JSON.parse(init.body) };
      return response(preparedResponse({ creation_time_ms: 42 }));
    },
  });
  await client.submitBridgeProof({
    authority: AUTHORITY,
    destination_proof_b64: "AQ==",
    creation_time_ms: 42,
  });
  assert.deepEqual(observed, {
    url: "https://example.invalid/v1/bridge/proofs/submit",
    body: { authority: AUTHORITY, destination_proof_b64: "AQ==", creation_time_ms: 42 },
  });
});
