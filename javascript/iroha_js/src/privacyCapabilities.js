/**
 * Fail-closed parser for the authoritative `PrivacyCapabilitySnapshotV1` Torii
 * response. This snapshot is the sole first-release privacy catalog contract;
 * only committed, typed protocol state can authorize proof submission.
 */

import { privacyCapabilityTransportV1 } from "./privacyCapabilityTransport.js";

export const PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1 = 1;

export const PRIVACY_PROTOCOL_IDS_V1 = Object.freeze([
  "zk-ace-pq-authorization-v0",
  "anonymous-pgc-k-out-of-n-v1",
  "verange-transparent-range-v1",
  "iroha-zk-ams-v1",
  "vega-existing-credential-zk-v0",
  "iroha-zk-x509-stark-p256-v0",
  "iroha-jindo-polynomial-commitment-v0",
  "iroha-bootle-lantern-anoncred-v1",
  "orchard-halo2-actions-v1",
  "monero-fcmp-plus-plus-v1",
  "iroha-ivm-private-note-stark-v1",
  "pq-masp-stark-v0",
]);

const MAX_SAFE_U64 = Number.MAX_SAFE_INTEGER;
const MAX_U32 = 0xffff_ffff;
const POLICY_DELAY_BLOCKS_V1 = 300;
const PROTOCOL_BINDINGS = Object.freeze({
  "zk-ace-pq-authorization-v0": ["stark-fri-sha256-goldilocks", "native-goldilocks-stark-fri"],
  "anonymous-pgc-k-out-of-n-v1": ["anonymous-pgc-p256", "native-anonymous-pgc-p256"],
  "verange-transparent-range-v1": ["iroha-verange-p256", "native-verange-p256"],
  "iroha-zk-ams-v1": [
    "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512",
    "native-zk-ams-masked-relaxed-spartan-t256-ristretto255",
  ],
  "vega-existing-credential-zk-v0": ["vega-neutron-nova-spartan-hyrax-t256", "native-vega"],
  "iroha-zk-x509-stark-p256-v0": ["stark-fri-sha256-goldilocks", "native-goldilocks-stark-fri"],
  "iroha-jindo-polynomial-commitment-v0": ["jindo-polynomial-commitment", "native-jindo"],
  "iroha-bootle-lantern-anoncred-v1": ["lantern-lnp22-module-linear-norm", "native-lantern-lnp22"],
  "orchard-halo2-actions-v1": ["halo2-ipa-pasta", "native-halo2-orchard"],
  "monero-fcmp-plus-plus-v1": ["fcmp-plus-plus-curve-tree-bulletproofs", "native-fcmp-plus-plus"],
  "iroha-ivm-private-note-stark-v1": ["stark-fri-sha256-goldilocks", "native-goldilocks-stark-fri"],
  "pq-masp-stark-v0": ["stark-fri-sha256-goldilocks", "native-goldilocks-stark-fri"],
});

const CONSENSUS_LIMIT_KEYS = Object.freeze([
  "max_actions_per_transaction",
  "max_actions_per_block",
  "max_proof_bytes_per_action",
  "max_action_bytes",
  "max_privacy_bytes_per_transaction",
  "max_privacy_bytes_per_block",
  "max_statement_and_encrypted_output_bytes_per_transaction",
  "max_nullifiers_per_action",
  "max_commitments_per_action",
  "retained_root_count",
]);
const CONSENSUS_LIMIT_MAXIMA = Object.freeze({
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
});

/** Error raised when a privacy-capability response cannot be trusted. */
export class PrivacyCapabilitySnapshotError extends TypeError {
  constructor(message, path = "privacy capability snapshot") {
    super(`${path}: ${message}`);
    this.name = "PrivacyCapabilitySnapshotError";
    this.path = path;
  }
}

/**
 * Parse and validate the exact first-release Torii privacy capability JSON.
 *
 * The output keeps the server's snake_case wire names so callers can compare
 * governed bindings byte-for-byte without an SDK-specific projection.
 *
 * @param {unknown} payload Torii JSON response body.
 * @returns {Readonly<Record<string, unknown>>} validated immutable snapshot.
 */
export function parsePrivacyCapabilitySnapshotV1(payload) {
  const snapshot = objectWithExactKeys(payload, [
    "version",
    "committed_height",
    "consensus_policy",
    "protocols",
  ], "privacy capability snapshot");
  if (u32(snapshot.version, "privacy capability snapshot.version") !== PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1) {
    fail("version must be exactly 1", "privacy capability snapshot.version");
  }
  const committedHeight = safeU64(snapshot.committed_height, "privacy capability snapshot.committed_height");
  const consensusPolicy = parseConsensusPolicy(snapshot.consensus_policy, committedHeight);
  if (!Array.isArray(snapshot.protocols) || snapshot.protocols.length !== PRIVACY_PROTOCOL_IDS_V1.length) {
    fail("protocols must contain exactly the 12 canonical protocol rows", "privacy capability snapshot.protocols");
  }
  const protocols = snapshot.protocols.map((row, index) => {
    const expected = PRIVACY_PROTOCOL_IDS_V1[index];
    return parseCapabilityRow(row, expected, committedHeight, `privacy capability snapshot.protocols[${index}]`);
  });
  return deepFreeze({
    version: PRIVACY_CAPABILITY_SNAPSHOT_VERSION_V1,
    committed_height: committedHeight,
    consensus_policy: consensusPolicy,
    protocols,
  });
}

/**
 * Fetch and fail-closed validate the authoritative committed privacy
 * capability snapshot from a configured Iroha JS Torii client.
 *
 * @param {unknown} client A package ToriiClient or ToriiBrowserClient.
 * @param {object} [options] Client-specific request options.
 * @returns {Promise<Readonly<Record<string, unknown>>>}
 */
export async function getPrivacyCapabilitiesV1(client, options = {}) {
  if (
    (typeof client !== "object" && typeof client !== "function")
    || client === null
  ) {
    throw new TypeError(
      "getPrivacyCapabilitiesV1 client must be an Iroha JS Torii client",
    );
  }
  const transport = client[privacyCapabilityTransportV1];
  if (typeof transport !== "function") {
    throw new TypeError(
      "getPrivacyCapabilitiesV1 client must be an Iroha JS Torii client",
    );
  }
  const payload = await Reflect.apply(transport, client, [options]);
  return parsePrivacyCapabilitySnapshotV1(payload);
}

function parseConsensusPolicy(value, committedHeight) {
  const policy = objectWithExactKeys(value, ["current_limits", "pending_tightening"], "privacy capability snapshot.consensus_policy");
  const currentLimits = parseConsensusLimits(policy.current_limits, "privacy capability snapshot.consensus_policy.current_limits");
  let pending = null;
  if (policy.pending_tightening !== null) {
    const path = "privacy capability snapshot.consensus_policy.pending_tightening";
    const tightening = objectWithExactKeys(policy.pending_tightening, ["scheduled_at_height", "effective_at_height", "next_limits"], path);
    const scheduled = positiveSafeU64(tightening.scheduled_at_height, `${path}.scheduled_at_height`);
    const effective = positiveSafeU64(tightening.effective_at_height, `${path}.effective_at_height`);
    if (scheduled > MAX_SAFE_U64 - POLICY_DELAY_BLOCKS_V1 || effective <= scheduled || effective < scheduled + POLICY_DELAY_BLOCKS_V1 || scheduled > committedHeight || effective <= committedHeight) {
      fail("has invalid committed-height schedule", path);
    }
    const nextLimits = parseConsensusLimits(tightening.next_limits, `${path}.next_limits`);
    assertStrictTightening(currentLimits, nextLimits, path);
    pending = { scheduled_at_height: scheduled, effective_at_height: effective, next_limits: nextLimits };
  }
  return { current_limits: currentLimits, pending_tightening: pending };
}

function parseConsensusLimits(value, path) {
  const limits = objectWithExactKeys(value, CONSENSUS_LIMIT_KEYS, path);
  const result = {};
  for (const key of CONSENSUS_LIMIT_KEYS) {
    const number = positiveU32(limits[key], `${path}.${key}`);
    if (number > CONSENSUS_LIMIT_MAXIMA[key]) fail("exceeds the first-release hard maximum", `${path}.${key}`);
    result[key] = number;
  }
  if (
    result.max_actions_per_transaction > result.max_actions_per_block
    || result.max_proof_bytes_per_action > result.max_action_bytes
    || result.max_action_bytes > result.max_privacy_bytes_per_transaction
    || result.max_privacy_bytes_per_transaction > result.max_privacy_bytes_per_block
    || result.max_statement_and_encrypted_output_bytes_per_transaction > result.max_action_bytes
  ) fail("violates consensus resource-limit ordering", path);
  return result;
}

function parseCapabilityRow(value, expectedProtocol, committedHeight, path) {
  const row = objectWithExactKeys(value, ["protocol_id", "compiled_profile", "activation"], path);
  const protocol = taggedUnit(row.protocol_id, "protocol", "value", PRIVACY_PROTOCOL_IDS_V1, `${path}.protocol_id`);
  if (protocol !== expectedProtocol) fail(`must be canonical protocol ${expectedProtocol}`, `${path}.protocol_id`);
  const compiled = parseCompiledProfile(row.compiled_profile, protocol, `${path}.compiled_profile`);
  const activation = row.activation === null ? null : parseActivation(row.activation, protocol, compiled, committedHeight, `${path}.activation`);
  if (activation !== null && compiled.status !== "available") {
    fail("cannot activate an unavailable compiled profile", `${path}.activation`);
  }
  return { protocol_id: tagged(protocol, "protocol", "value"), compiled_profile: compiled, activation };
}

function parseCompiledProfile(value, protocol, path) {
  const result = objectWithExactKeys(value, ["status", "value"], path);
  if (result.status === "available") {
    return { status: "available", value: parseProfileBindings(result.value, protocol, `${path}.value`) };
  }
  if (result.status !== "unavailable") fail("status must be available or unavailable", `${path}.status`);
  return { status: "unavailable", value: parseUnavailableReason(result.value, `${path}.value`) };
}

function parseUnavailableReason(value, path) {
  const reason = objectWithExactKeys(value, ["reason", "detail"], path);
  if (reason.reason === "engine-unavailable" || reason.reason === "profile-initialization-failed") {
    if (reason.detail !== null) fail("unit unavailable reason must have null detail", `${path}.detail`);
    return { reason: reason.reason, detail: null };
  }
  if (reason.reason !== "statement-schema-invalid") fail("unknown unavailable reason", `${path}.reason`);
  const detail = taggedUnit(reason.detail, "schema_error", "detail", ["conflicting-stable-type-id", "missing-type-reference"], `${path}.detail`);
  return { reason: "statement-schema-invalid", detail: tagged(detail, "schema_error", "detail") };
}

function parseProfileBindings(value, protocol, path) {
  const profile = objectWithExactKeys(value, [
    "protocol_id", "proof_system_id", "engine_id", "parameter_id", "parameter_digest",
    "verifier_digest", "statement_schema_digest", "engine_manifest_digest", "protocol_limits",
  ], path);
  const bindings = parseBindings(profile, protocol, path);
  const limits = parseProtocolLimits(profile.protocol_limits, protocol, `${path}.protocol_limits`);
  return { ...bindings, protocol_limits: limits };
}

function parseActivation(value, protocol, compiled, committedHeight, path) {
  const record = objectWithExactKeys(value, [
    "protocol_id", "proof_system_id", "engine_id", "parameter_id", "parameter_digest",
    "verifier_digest", "statement_schema_digest", "engine_manifest_digest", "lifecycle",
    "protocol_limits", "pending_protocol_limits_tightening", "assurance",
  ], path);
  const bindings = parseBindings(record, protocol, path);
  if (compiled.status === "available") assertEqualBindings(bindings, compiled.value, path);
  const protocolLimits = parseProtocolLimits(record.protocol_limits, protocol, `${path}.protocol_limits`);
  if (compiled.status === "available") assertLimitsAtMost(protocolLimits, compiled.value.protocol_limits, `${path}.protocol_limits`);
  const lifecycle = parseLifecycle(record.lifecycle, committedHeight, `${path}.lifecycle`);
  const pending = parseProtocolTightening(record.pending_protocol_limits_tightening, protocolLimits, committedHeight, `${path}.pending_protocol_limits_tightening`);
  const assurance = taggedUnit(record.assurance, "assurance", "value", ["experimental"], `${path}.assurance`);
  return { ...bindings, lifecycle, protocol_limits: protocolLimits, pending_protocol_limits_tightening: pending, assurance: tagged(assurance, "assurance", "value") };
}

function parseBindings(value, protocol, path) {
  const protocolId = taggedUnit(value.protocol_id, "protocol", "value", PRIVACY_PROTOCOL_IDS_V1, `${path}.protocol_id`);
  if (protocolId !== protocol) fail("does not match its row protocol", `${path}.protocol_id`);
  const [expectedProof, expectedEngine] = PROTOCOL_BINDINGS[protocol];
  const proof = taggedUnit(value.proof_system_id, "proof_system", "value", [expectedProof], `${path}.proof_system_id`);
  const engine = taggedUnit(value.engine_id, "engine", "value", [expectedEngine], `${path}.engine_id`);
  return {
    protocol_id: tagged(protocol, "protocol", "value"),
    proof_system_id: tagged(proof, "proof_system", "value"),
    engine_id: tagged(engine, "engine", "value"),
    parameter_id: fixedBytes(value.parameter_id, `${path}.parameter_id`),
    parameter_digest: fixedBytes(value.parameter_digest, `${path}.parameter_digest`),
    verifier_digest: fixedBytes(value.verifier_digest, `${path}.verifier_digest`),
    statement_schema_digest: fixedBytes(value.statement_schema_digest, `${path}.statement_schema_digest`),
    engine_manifest_digest: fixedBytes(value.engine_manifest_digest, `${path}.engine_manifest_digest`),
  };
}

function parseProtocolLimits(value, protocol, path) {
  const limit = objectWithExactKeys(value, ["protocol", "limits"], path);
  if (typeof limit.protocol !== "string" || !PRIVACY_PROTOCOL_IDS_V1.includes(limit.protocol)) {
    fail("has an unknown or non-canonical protocol tag", `${path}.protocol`);
  }
  const id = limit.protocol;
  if (id !== protocol) fail("does not match the protocol binding", `${path}.protocol`);
  const fields = limitFieldsFor(protocol);
  if (fields.length === 0) {
    if (limit.limits !== null) fail("fixed protocol limits must be null", `${path}.limits`);
    return { protocol, limits: null };
  }
  const limits = objectWithExactKeys(limit.limits, fields.map(([key]) => key), `${path}.limits`);
  const normalized = {};
  for (const [key, max, permitted] of fields) {
    const number = positiveU32(limits[key], `${path}.limits.${key}`);
    if (number > max || (permitted && !permitted.includes(number))) fail("is outside the closed first-release limit set", `${path}.limits.${key}`);
    normalized[key] = number;
  }
  return { protocol, limits: normalized };
}

function limitFieldsFor(protocol) {
  switch (protocol) {
    case "anonymous-pgc-k-out-of-n-v1": return [["max_anonymity_set_size", 64, [16, 32, 64]], ["max_recipient_count", 8]];
    case "verange-transparent-range-v1": return [["max_aggregation_count", 8]];
    case "iroha-zk-ams-v1": return [["max_batch_size", 8], ["max_ring_size", 64, [16, 32, 64]]];
    case "iroha-jindo-polynomial-commitment-v0": return [["max_polynomial_count", 4]];
    case "orchard-halo2-actions-v1": return [["max_action_count", 2]];
    case "monero-fcmp-plus-plus-v1": return [["max_input_count", 2], ["max_output_count", 4]];
    case "iroha-ivm-private-note-stark-v1":
    case "pq-masp-stark-v0": return [["max_input_count", 2], ["max_output_count", 2]];
    default: return [];
  }
}

function parseLifecycle(value, committedHeight, path) {
  const lifecycle = objectWithExactKeys(value, ["state", "record"], path);
  const state = lifecycle.state;
  if (!new Set(["proposed", "active", "suspended", "retired"]).has(state)) fail("unknown lifecycle state", `${path}.state`);
  const keys = state === "proposed"
    ? ["proposed_at_height", "activate_at_height"]
    : ["proposed_at_height", "activated_at_height", "state_since_height"];
  const record = objectWithExactKeys(lifecycle.record, keys, `${path}.record`);
  const proposed = positiveSafeU64(record.proposed_at_height, `${path}.record.proposed_at_height`);
  let normalized;
  if (state === "proposed") {
    const activate = positiveSafeU64(record.activate_at_height, `${path}.record.activate_at_height`);
    if (activate <= proposed || proposed > committedHeight || activate <= committedHeight) fail("has invalid proposed lifecycle heights", path);
    normalized = { proposed_at_height: proposed, activate_at_height: activate };
  } else {
    const activated = state === "retired" && record.activated_at_height === null ? null : positiveSafeU64(record.activated_at_height, `${path}.record.activated_at_height`);
    const since = positiveSafeU64(record.state_since_height, `${path}.record.state_since_height`);
    if (proposed > committedHeight || since > committedHeight || (activated !== null && activated > committedHeight)) fail("claims a state after committed height", path);
    if (activated === null ? state !== "retired" || since <= proposed : activated <= proposed || (state === "active" ? since < activated : since <= activated)) fail("has invalid lifecycle ordering", path);
    normalized = { proposed_at_height: proposed, activated_at_height: activated, state_since_height: since };
  }
  return { state, record: normalized };
}

function parseProtocolTightening(value, current, committedHeight, path) {
  if (value === null) return null;
  const tightening = objectWithExactKeys(value, ["scheduled_at_height", "effective_at_height", "next_limits"], path);
  const scheduled = positiveSafeU64(tightening.scheduled_at_height, `${path}.scheduled_at_height`);
  const effective = positiveSafeU64(tightening.effective_at_height, `${path}.effective_at_height`);
  if (scheduled > MAX_SAFE_U64 - POLICY_DELAY_BLOCKS_V1 || effective <= scheduled || effective < scheduled + POLICY_DELAY_BLOCKS_V1 || scheduled > committedHeight || effective <= committedHeight) fail("has invalid committed-height schedule", path);
  const next = parseProtocolLimits(tightening.next_limits, current.protocol, `${path}.next_limits`);
  assertLimitsAtMost(next, current, `${path}.next_limits`);
  if (sameJson(next, current)) fail("must be a strict tightening", path);
  return { scheduled_at_height: scheduled, effective_at_height: effective, next_limits: next };
}

function assertEqualBindings(actual, expected, path) {
  for (const key of ["protocol_id", "proof_system_id", "engine_id", "parameter_id", "parameter_digest", "verifier_digest", "statement_schema_digest", "engine_manifest_digest"]) {
    if (!sameJson(actual[key], expected[key])) fail(`does not match compiled profile ${key}`, `${path}.${key}`);
  }
}

function assertLimitsAtMost(actual, ceiling, path) {
  if (actual.protocol !== ceiling.protocol || (actual.limits === null) !== (ceiling.limits === null)) fail("protocol-limit tag differs from compiled ceiling", path);
  if (actual.limits !== null) for (const key of Object.keys(actual.limits)) if (actual.limits[key] > ceiling.limits[key]) fail("exceeds the compiled profile ceiling", `${path}.${key}`);
}

function assertStrictTightening(current, next, path) {
  let changed = false;
  for (const key of CONSENSUS_LIMIT_KEYS) {
    if (next[key] > current[key]) fail("cannot increase a consensus limit", `${path}.next_limits.${key}`);
    changed ||= next[key] !== current[key];
  }
  if (!changed) fail("must be a strict tightening", path);
}

function taggedUnit(value, tagKey, contentKey, permitted, path) {
  const taggedValue = objectWithExactKeys(value, [tagKey, contentKey], path);
  if (typeof taggedValue[tagKey] !== "string" || !permitted.includes(taggedValue[tagKey])) fail("has an unknown or non-canonical tag", `${path}.${tagKey}`);
  if (taggedValue[contentKey] !== null) fail("unit enum content must be null", `${path}.${contentKey}`);
  return taggedValue[tagKey];
}

function tagged(value, tagKey, contentKey) { return { [tagKey]: value, [contentKey]: null }; }

function fixedBytes(value, path) {
  if (!Array.isArray(value) || value.length !== 32) fail("must be exactly 32 bytes", path);
  const bytes = value.map((byte, index) => {
    if (!Number.isInteger(byte) || byte < 0 || byte > 255) fail("must contain only uint8 values", `${path}[${index}]`);
    return byte;
  });
  if (bytes.every((byte) => byte === 0)) fail("must not be all zero", path);
  return bytes;
}

function objectWithExactKeys(value, expectedKeys, path) {
  const prototype = value === null || typeof value !== "object"
    ? undefined
    : Object.getPrototypeOf(value);
  if (
    value === null
    || typeof value !== "object"
    || Array.isArray(value)
    || (prototype !== Object.prototype && prototype !== null)
  ) fail("must be a plain JSON object", path);
  const keys = Object.keys(value).sort();
  const expected = [...expectedKeys].sort();
  if (keys.length !== expected.length || keys.some((key, index) => key !== expected[index])) fail(`must contain exactly: ${expected.join(", ")}`, path);
  return value;
}

function u32(value, path) {
  if (!Number.isSafeInteger(value) || value < 0 || value > MAX_U32) fail("must be a safe uint32 integer", path);
  return value;
}
function positiveU32(value, path) { const result = u32(value, path); if (result === 0) fail("must be non-zero", path); return result; }
function safeU64(value, path) { if (!Number.isSafeInteger(value) || value < 0 || value > MAX_SAFE_U64) fail("must be a safe uint64 integer", path); return value; }
function positiveSafeU64(value, path) { const result = safeU64(value, path); if (result === 0) fail("must be non-zero", path); return result; }
function sameJson(left, right) { return JSON.stringify(left) === JSON.stringify(right); }
function fail(message, path) { throw new PrivacyCapabilitySnapshotError(message, path); }
function deepFreeze(value) { if (value && typeof value === "object" && !Object.isFrozen(value)) { Object.freeze(value); for (const item of Object.values(value)) deepFreeze(item); } return value; }
