/**
 * Fail-closed native admission for the canonical Exact12 capability manifest.
 * Only native-validated canonical bytes from authenticated Torii state can
 * authorize proof submission.
 */

import { getNativeBinding } from "./native.js";
import { NetworkId, networkIdBytes } from "./networkId.js";
import {
  bindPrivacyExact12CapabilityAdmissionV1,
  requirePrivacyExact12CapabilityAdmissionV1,
  requirePrivacyExact12CapabilityTupleV1,
} from "./privacyCapabilityAdmission.js";
import {
  fetchPrivacyExact12CapabilityManifestTransportV1,
  consumePrivacyExact12CapabilityManifestTransportV1,
} from "./privacyCapabilityTransport.js";
import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";

export {
  requirePrivacyExact12CapabilityAdmissionV1,
  requirePrivacyExact12CapabilityTupleV1,
};

/** Canonical public Exact12 capability-manifest version. */
export const PRIVACY_EXACT12_CAPABILITY_MANIFEST_VERSION_V1 = 1;
/** Native validator byte ceiling shared with the Rust decoder. */
export const PRIVACY_EXACT12_CAPABILITY_MANIFEST_MAX_BYTES_V1 = 256 * 1024;

export const PRIVACY_PROTOCOL_IDS_V1 = Object.freeze([
  "zk-ace-pq-authorization-v1",
  "anonymous-pgc-k-out-of-n-v1",
  "verange-transparent-range-v1",
  "iroha-zk-ams-v1",
  "vega-existing-credential-zk-v1",
  "iroha-zk-x509-stark-p256-v1",
  "iroha-jindo-polynomial-commitment-v1",
  "iroha-bootle-lantern-anoncred-v1",
  "orchard-halo2-actions-v1",
  "monero-fcmp-plus-plus-v1",
  "iroha-ivm-private-note-stark-v1",
  "pq-masp-stark-v1",
]);

const MAX_U64 = 0xffff_ffff_ffff_ffffn;
const MAX_U32 = 0xffff_ffff;
const PROTOCOL_BINDINGS = Object.freeze({
  "zk-ace-pq-authorization-v1": ["stark-fri-sha3-384-goldilocks-v1", "native-goldilocks-sha3-384-stark-fri-v1"],
  "anonymous-pgc-k-out-of-n-v1": ["anonymous-pgc-p256", "native-anonymous-pgc-p256"],
  "verange-transparent-range-v1": ["iroha-verange-p256", "native-verange-p256"],
  "iroha-zk-ams-v1": [
    "zk-ams-masked-relaxed-spartan-t256-ristretto255-sha3-512",
    "native-zk-ams-masked-relaxed-spartan-t256-ristretto255",
  ],
  "vega-existing-credential-zk-v1": ["vega-neutron-nova-spartan-hyrax-t256", "native-vega"],
  "iroha-zk-x509-stark-p256-v1": ["stark-fri-sha3-384-goldilocks-v1", "native-goldilocks-sha3-384-stark-fri-v1"],
  "iroha-jindo-polynomial-commitment-v1": ["jindo-polynomial-commitment", "native-jindo"],
  "iroha-bootle-lantern-anoncred-v1": ["lantern-lnp22-module-linear-norm", "native-lantern-lnp22"],
  "orchard-halo2-actions-v1": ["halo2-ipa-pasta", "native-halo2-orchard"],
  "monero-fcmp-plus-plus-v1": ["fcmp-plus-plus-curve-tree-bulletproofs", "native-fcmp-plus-plus"],
  "iroha-ivm-private-note-stark-v1": ["stark-fri-sha3-384-goldilocks-v1", "native-goldilocks-sha3-384-stark-fri-v1"],
  "pq-masp-stark-v1": ["stark-fri-sha3-384-goldilocks-v1", "native-goldilocks-sha3-384-stark-fri-v1"],
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
  max_action_bytes: 9 * 1024 * 1024 + 256 * 1024 + 512,
  max_privacy_bytes_per_transaction: 9 * 1024 * 1024 + 256 * 1024 + 512,
  max_privacy_bytes_per_block: 16 * 1024 * 1024 - 64 * 1024,
  max_statement_and_encrypted_output_bytes_per_transaction: 256 * 1024,
  max_nullifiers_per_action: 8,
  max_commitments_per_action: 8,
  retained_root_count: 2048,
});

function parseConsensusPolicy(value, committedHeight, path) {
  const policy = objectWithExactKeys(value, ["current_limits", "pending_update"], path);
  const currentLimits = parseConsensusLimits(policy.current_limits, `${path}.current_limits`);
  let pending = null;
  if (policy.pending_update !== null) {
    const pendingPath = `${path}.pending_update`;
    const update = objectWithExactKeys(policy.pending_update, ["scheduled_at_height", "effective_at_height", "next_limits"], pendingPath);
    const scheduled = positiveU64(update.scheduled_at_height, `${pendingPath}.scheduled_at_height`);
    const effective = positiveU64(update.effective_at_height, `${pendingPath}.effective_at_height`);
    if (effective <= scheduled || scheduled > committedHeight || effective <= committedHeight) {
      fail("has invalid committed-height schedule", pendingPath);
    }
    const nextLimits = parseConsensusLimits(update.next_limits, `${pendingPath}.next_limits`);
    assertUpdateChanged(currentLimits, nextLimits, pendingPath);
    pending = { scheduled_at_height: scheduled, effective_at_height: effective, next_limits: nextLimits };
  }
  return { current_limits: currentLimits, pending_update: pending };
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
  const row = objectWithExactKeys(value, ["protocol_id", "compiled_profile", "policy"], path);
  const protocol = taggedUnit(row.protocol_id, "protocol", "value", PRIVACY_PROTOCOL_IDS_V1, `${path}.protocol_id`);
  if (protocol !== expectedProtocol) fail(`must be canonical protocol ${expectedProtocol}`, `${path}.protocol_id`);
  const compiled = parseCompiledProfile(row.compiled_profile, protocol, `${path}.compiled_profile`);
  const policy = row.policy === null ? null : parseProtocolPolicy(row.policy, protocol, committedHeight, `${path}.policy`);
  return { protocol_id: tagged(protocol, "protocol", "value"), compiled_profile: compiled, policy };
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
  if (reason.reason === "engine-unavailable" || reason.reason === "profile-initialization-failed" || reason.reason === "joint-security-reduction-incomplete") {
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

function parseProtocolPolicy(value, protocol, committedHeight, path) {
  const policy = objectWithExactKeys(value, ["protocol_id", "current_limits", "pending_update"], path);
  const id = taggedUnit(policy.protocol_id, "protocol", "value", PRIVACY_PROTOCOL_IDS_V1, `${path}.protocol_id`);
  if (id !== protocol) fail("does not match its row protocol", `${path}.protocol_id`);
  const current = parseProtocolLimits(policy.current_limits, protocol, `${path}.current_limits`);
  return {
    protocol_id: tagged(protocol, "protocol", "value"),
    current_limits: current,
    pending_update: parseProtocolUpdate(policy.pending_update, current, committedHeight, `${path}.pending_update`),
  };
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
    case "iroha-jindo-polynomial-commitment-v1": return [["max_polynomial_count", 4]];
    case "orchard-halo2-actions-v1": return [["max_action_count", 2]];
    case "monero-fcmp-plus-plus-v1": return [["max_input_count", 2], ["max_output_count", 4]];
    case "iroha-ivm-private-note-stark-v1":
    case "pq-masp-stark-v1": return [["max_input_count", 2], ["max_output_count", 2]];
    default: return [];
  }
}

function parseProtocolUpdate(value, current, committedHeight, path) {
  if (value === null) return null;
  const update = objectWithExactKeys(value, ["scheduled_at_height", "effective_at_height", "next_limits"], path);
  const scheduled = positiveU64(update.scheduled_at_height, `${path}.scheduled_at_height`);
  const effective = positiveU64(update.effective_at_height, `${path}.effective_at_height`);
  if (effective <= scheduled || scheduled > committedHeight || effective <= committedHeight) fail("has invalid committed-height schedule", path);
  const next = parseProtocolLimits(update.next_limits, current.protocol, `${path}.next_limits`);
  if (sameJson(next, current)) fail("must change at least one limit", path);
  return { scheduled_at_height: scheduled, effective_at_height: effective, next_limits: next };
}

function assertUpdateChanged(current, next, path) {
  if (!CONSENSUS_LIMIT_KEYS.some((key) => next[key] !== current[key])) {
    fail("must change at least one limit", path);
  }
}

function taggedUnit(value, tagKey, contentKey, permitted, path) {
  const taggedValue = objectWithExactKeys(value, [tagKey, contentKey], path);
  if (typeof taggedValue[tagKey] !== "string" || !permitted.includes(taggedValue[tagKey])) fail("has an unknown or non-canonical tag", `${path}.${tagKey}`);
  if (taggedValue[contentKey] !== null) fail("unit enum content must be null", `${path}.${contentKey}`);
  return taggedValue[tagKey];
}

function tagged(value, tagKey, contentKey) { return { [tagKey]: value, [contentKey]: null }; }

function fixedBytes(value, path) {
  const bytes = fixedBytesWithLength(value, 32, path);
  if (bytes.every((byte) => byte === 0)) fail("must not be all zero", path);
  return bytes;
}

function fixedBytesWithLength(value, length, path) {
  if (!Array.isArray(value) || value.length !== length) {
    fail(`must be exactly ${length} bytes`, path);
  }
  const bytes = value.map((byte, index) => {
    if (!Number.isInteger(byte) || byte < 0 || byte > 255) fail("must contain only uint8 values", `${path}[${index}]`);
    return byte;
  });
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
function u64(value, path) {
  let result;
  if (typeof value === "bigint") {
    result = value;
  } else if (Number.isSafeInteger(value) && !Object.is(value, -0)) {
    result = BigInt(value);
  } else {
    fail("must be an exact canonical uint64 integer", path);
  }
  if (result < 0n || result > MAX_U64) fail("must be within the uint64 range", path);
  return result;
}
function positiveU64(value, path) { const result = u64(value, path); if (result === 0n) fail("must be non-zero", path); return result; }
function sameJson(left, right) { return JSON.stringify(left) === JSON.stringify(right); }
function fail(message, path) { manifestFailV1(message, path); }
function deepFreeze(value) { if (value && typeof value === "object" && !Object.isFrozen(value)) { Object.freeze(value); for (const item of Object.values(value)) deepFreeze(item); } return value; }

const PRIVACY_EXACT12_CAPABILITY_MANIFEST_JSON_MAX_BYTES_V1 = 2 * 1024 * 1024;
const PRIVACY_EXACT12_MANIFEST_CONSTRUCTOR = Symbol("Exact12 manifest constructor");
const privacyExact12ManifestState = new WeakMap();
const PRIVACY_EXACT12_OPERATION_TUPLES_V1 = Object.freeze([
  Object.freeze(["zk_ace_authorization_action_v1", "authorization_action", 0]),
  Object.freeze(["anonymous_pgc_payment_action_v1", "payment_action", 6]),
  Object.freeze(["verange_range_proof_v1", "component", 1]),
  Object.freeze(["zk_ams_admission_and_provisioning_v1", "admission_action", 2]),
  Object.freeze(["vega_credential_presentation_v1", "presentation_action", 2]),
  Object.freeze(["zk_x509_identity_presentation_v1", "presentation_action", 2]),
  Object.freeze(["jindo_polynomial_evaluation_v1", "component", 0]),
  Object.freeze(["bootle_lantern_credential_presentation_v1", "presentation_action", 2]),
  Object.freeze(["orchard_note_action_v1", "note_action", 7]),
  Object.freeze(["fcmp_membership_payment_v1", "payment_action", 2]),
  Object.freeze(["ivm_private_note_action_v1", "note_action", 7]),
  Object.freeze(["pq_masp_note_action_v1", "note_action", 31]),
]);

const PRIVACY_EXACT12_NATIVE_METHODS_V1 = Object.freeze([
  "privacyCompiledProfileCatalogV1",
  "privacyValidateCompiledProfileCatalogV1",
  "privacyValidateExact12CapabilityManifestV1",
  "privacyExact12CapabilityManifestJsonV1",
  "privacyRequireExact12CapabilityTupleV1",
]);

/** Error raised when canonical Exact12 bytes or native admission fail closed. */
export class PrivacyExact12CapabilityManifestError extends TypeError {
  constructor(message, path = "Exact12 capability manifest", options = undefined) {
    super(`${path}: ${message}`, options);
    this.name = "PrivacyExact12CapabilityManifestError";
    this.path = path;
  }
}

/**
 * Immutable model created only from native-validated canonical manifest bytes.
 *
 * The public fields preserve the exact seven-field rows and manifest digest.
 * The self-digest identifies content; it does not authenticate an untrusted
 * producer. Use authenticated Torii transport.
 */
export class PrivacyExact12CapabilityManifestV1 {
  constructor(token, canonicalArchive, projection) {
    if (token !== PRIVACY_EXACT12_MANIFEST_CONSTRUCTOR) {
      throw new TypeError(
        "PrivacyExact12CapabilityManifestV1 has no public constructor; decode canonical Torii bytes",
      );
    }
    this.version = projection.version;
    this.committed_height = projection.committed_height;
    this.consensus_policy = projection.consensus_policy;
    this.network_id = projection.network_id;
    this.protocols = projection.protocols;
    this.manifest_digest = projection.manifest_digest;
    privacyExact12ManifestState.set(this, {
      canonicalArchive: Uint8Array.from(canonicalArchive),
    });
    Object.freeze(this);
  }

  /** Return a defensive copy of the exact native-validated Torii bytes. */
  canonicalBytes() {
    const state = privacyExact12ManifestState.get(this);
    if (!state) {
      throw new TypeError("invalid Exact12 capability manifest receiver");
    }
    return Uint8Array.from(state.canonicalArchive);
  }
}

/**
 * Return this N-API binary's canonical compiled-profile catalog bytes.
 *
 * This local catalog has no committed height or committed policy and therefore
 * never authorizes network construction by itself.
 */
export function compiledProfileCatalogV1() {
  const native = requirePrivacyExact12NativeV1();
  return compiledProfileCatalogFromNativeV1(native);
}

function compiledProfileCatalogFromNativeV1(native) {
  const archive = callPrivacyExact12NativeV1(
    native,
    "privacyCompiledProfileCatalogV1",
    [],
  );
  const bytes = copyPrivacyExact12ArchiveV1(
    archive,
    "native compiled-profile catalog",
  );
  const status = callPrivacyExact12NativeV1(
    native,
    "privacyValidateCompiledProfileCatalogV1",
    [bytes],
  );
  if (status !== 0) {
    manifestFailV1(
      `native local compiled-profile catalog validation returned status ${String(status)}`,
      "compiled profile catalog",
    );
  }
  return bytes;
}

/** Decode canonical Exact12 bytes for inspection. An archive never grants admission. */
export function decodePrivacyExact12CapabilityManifestV1(canonicalArchive) {
  const bytes = copyPrivacyExact12ArchiveV1(canonicalArchive, "canonical archive");
  const native = requirePrivacyExact12NativeV1();
  const status = callPrivacyExact12NativeV1(
    native,
    "privacyValidateExact12CapabilityManifestV1",
    [bytes],
  );
  if (status !== 0) {
    manifestFailV1(
      `native canonical manifest validation returned status ${String(status)}`,
      "canonical archive",
    );
  }
  const jsonText = callPrivacyExact12NativeV1(
    native,
    "privacyExact12CapabilityManifestJsonV1",
    [bytes],
  );
  if (typeof jsonText !== "string") {
    manifestFailV1("native decoder returned a non-string projection", "native projection");
  }
  if (jsonText.length > PRIVACY_EXACT12_CAPABILITY_MANIFEST_JSON_MAX_BYTES_V1) {
    manifestFailV1(
      `native projection exceeds ${PRIVACY_EXACT12_CAPABILITY_MANIFEST_JSON_MAX_BYTES_V1} bytes`,
      "native projection",
    );
  }
  let payload;
  try {
    payload = parseStrictLosslessIntegerJson(
      jsonText,
      "native Exact12 capability manifest projection",
    );
  } catch (cause) {
    throw new PrivacyExact12CapabilityManifestError(
      "native decoder returned invalid lossless JSON",
      "native projection",
      { cause },
    );
  }
  const projection = parsePrivacyExact12ManifestProjectionV1(payload);
  return new PrivacyExact12CapabilityManifestV1(
    PRIVACY_EXACT12_MANIFEST_CONSTRUCTOR,
    bytes,
    projection,
  );
}

/**
 * Fetch Torii's canonical Norito manifest and validate it with the required
 * N-API binding. Browser-only clients and JSON/mock transports fail closed.
 */
export async function getPrivacyExact12CapabilityManifestV1(client, options) {
  requirePrivacyExact12NativeV1();
  if (
    (typeof client !== "object" && typeof client !== "function")
    || client === null
  ) {
    throw new TypeError(
      "getPrivacyExact12CapabilityManifestV1 requires the N-API Torii client",
    );
  }
  const receipt = await fetchPrivacyExact12CapabilityManifestTransportV1(client, options);
  const transport = consumePrivacyExact12CapabilityManifestTransportV1(receipt);
  const manifest = decodePrivacyExact12CapabilityManifestV1(transport.archive);
  const state = privacyExact12ManifestState.get(manifest);
  state.expectedNetworkId = Uint8Array.from(transport.expectedNetworkId);
  state.origin = transport.origin;
  const actualNetworkId = manifest.network_id.toBytes();
  if (!state.expectedNetworkId.every((byte, index) => byte === actualNetworkId[index])) {
    manifestFailV1("capability manifest belongs to a different network", "transport");
  }
  bindPrivacyExact12CapabilityAdmissionV1(
    manifest,
    (protocolId) => admitPrivacyExact12CapabilityTupleV1(manifest, protocolId),
  );
  return manifest;
}

/**
 * Require one available row and exact native-local compiled tuple.
 *
 * Transaction builders must call this guard with the validated manifest at
 * construction time. A local catalog, digest shell, or caller-created object
 * is never accepted as admission.
 */
function admitPrivacyExact12CapabilityTupleV1(manifest, protocolId) {
  const state = privacyExact12ManifestState.get(manifest);
  if (!state) manifestFailV1("lost its native validation state", "admission");
  const actualNetworkId = manifest.network_id.toBytes();
  if (!state.expectedNetworkId || !state.expectedNetworkId.every((byte, index) => byte === actualNetworkId[index])) {
    manifestFailV1("capability manifest belongs to a different network", "admission");
  }
  const index = requirePrivacyExact12ProtocolIdV1(protocolId);
  const row = manifest.protocols[index];
  const readiness = row.readiness.readiness;
  if (
    readiness !== "available"
    || row.compiled_profile.status !== "available"
  ) {
    manifestFailV1(
      `protocol ${protocolId} is not available in committed state: ${row.compiled_profile.value.reason}`,
      `protocols[${index}]`,
    );
  }
  const native = requirePrivacyExact12NativeV1();
  // The native guard checks only the selected implementation, exact tuple and
  // effective policy. Unrelated catalog entries are not admission prerequisites.
  const admitted = callPrivacyExact12NativeV1(
    native,
    "privacyRequireExact12CapabilityTupleV1",
    [Uint8Array.from(state.canonicalArchive), protocolId, Uint8Array.from(state.expectedNetworkId)],
  );
  if (admitted !== true) {
    manifestFailV1(
      "native tuple admission did not return the sole success value",
      `protocols[${index}]`,
    );
  }
  return deepFreeze({
    manifest_digest: Array.from(manifest.manifest_digest),
    network_id: Array.from(state.expectedNetworkId),
    torii_origin: state.origin,
    committed_height: manifest.committed_height,
    protocol_id: protocolId,
    operation_schema: row.operation_schema.operation_schema,
    execution_mode: row.execution_mode.execution_mode,
    privacy_feature_mask: row.privacy_feature_mask,
    readiness,
    compiled_profile: row.compiled_profile.value,
  });
}

function requirePrivacyExact12NativeV1() {
  let native;
  try {
    native = getNativeBinding();
  } catch (cause) {
    throw new PrivacyExact12CapabilityManifestError(
      "authenticated iroha_js_host native binding is required; no browser or mock fallback is permitted",
      "native binding",
      { cause },
    );
  }
  let abiVersion;
  try {
    abiVersion = native?.connectNoritoBridgeAbiVersion?.();
  } catch (cause) {
    throw new PrivacyExact12CapabilityManifestError(
      "could not read the native bridge ABI version",
      "native binding",
      { cause },
    );
  }
  if (abiVersion !== 33) {
    manifestFailV1("requires exact ABI33", "native binding");
  }
  for (const method of PRIVACY_EXACT12_NATIVE_METHODS_V1) {
    if (typeof native?.[method] !== "function") {
      manifestFailV1(`is missing ${method}`, "native binding");
    }
  }
  return native;
}

function callPrivacyExact12NativeV1(native, method, args) {
  try {
    return Reflect.apply(native[method], native, args);
  } catch (cause) {
    throw new PrivacyExact12CapabilityManifestError(
      `${method} failed`,
      "native binding",
      { cause },
    );
  }
}

function copyPrivacyExact12ArchiveV1(value, path) {
  if (typeof value === "string") {
    manifestFailV1("must be canonical Norito bytes, not text", path);
  }
  let bytes;
  if (value instanceof ArrayBuffer) {
    bytes = new Uint8Array(value.slice(0));
  } else if (ArrayBuffer.isView(value)) {
    if (!(value.buffer instanceof ArrayBuffer)) {
      manifestFailV1("must not use shared memory", path);
    }
    bytes = Uint8Array.from(
      new Uint8Array(value.buffer, value.byteOffset, value.byteLength),
    );
  } else {
    manifestFailV1("must be an ArrayBuffer or contiguous byte view", path);
  }
  if (bytes.length === 0) manifestFailV1("must not be empty", path);
  if (bytes.length > PRIVACY_EXACT12_CAPABILITY_MANIFEST_MAX_BYTES_V1) {
    manifestFailV1(
      `exceeds ${PRIVACY_EXACT12_CAPABILITY_MANIFEST_MAX_BYTES_V1} bytes`,
      path,
    );
  }
  return bytes;
}

function parsePrivacyExact12ManifestProjectionV1(payload) {
  const manifest = exactManifestObjectV1(payload, [
    "version",
    "committed_height",
    "consensus_policy",
    "network_id",
    "protocols",
    "manifest_digest",
  ], "Exact12 capability manifest");
  if (
    u32(manifest.version, "Exact12 capability manifest.version")
    !== PRIVACY_EXACT12_CAPABILITY_MANIFEST_VERSION_V1
  ) {
    manifestFailV1("version must be exactly 1", "Exact12 capability manifest.version");
  }
  const committedHeight = u64(
    manifest.committed_height,
    "Exact12 capability manifest.committed_height",
  );
  const consensusPolicy = parseConsensusPolicy(
    manifest.consensus_policy,
    committedHeight,
    "Exact12 capability manifest.consensus_policy",
  );
  let networkId;
  try {
    networkId = NetworkId.parse(manifest.network_id);
  } catch (cause) {
    throw new PrivacyExact12CapabilityManifestError(
      "must be a canonical NetworkId",
      "Exact12 capability manifest.network_id",
      { cause },
    );
  }
  if (
    !Array.isArray(manifest.protocols)
    || manifest.protocols.length !== PRIVACY_PROTOCOL_IDS_V1.length
  ) {
    manifestFailV1(
      "protocols must contain exactly the 12 canonical protocol rows",
      "Exact12 capability manifest.protocols",
    );
  }
  const protocols = manifest.protocols.map((rawRow, index) => {
    const path = `Exact12 capability manifest.protocols[${index}]`;
    const row = exactManifestObjectV1(rawRow, [
      "protocol_id",
      "operation_schema",
      "execution_mode",
      "privacy_feature_mask",
      "compiled_profile",
      "readiness",
      "policy",
    ], path);
    const base = parseCapabilityRow(
      {
        protocol_id: row.protocol_id,
        compiled_profile: row.compiled_profile,
        policy: row.policy,
      },
      PRIVACY_PROTOCOL_IDS_V1[index],
      committedHeight,
      path,
    );
    const [operationSchema, executionMode, featureMask] =
      PRIVACY_EXACT12_OPERATION_TUPLES_V1[index];
    const operation = manifestTaggedUnitV1(
      row.operation_schema,
      "operation_schema",
      "value",
      operationSchema,
      `${path}.operation_schema`,
    );
    const execution = manifestTaggedUnitV1(
      row.execution_mode,
      "execution_mode",
      "value",
      executionMode,
      `${path}.execution_mode`,
    );
    if (row.privacy_feature_mask !== featureMask) {
      manifestFailV1(
        `must equal the closed feature mask ${featureMask}`,
        `${path}.privacy_feature_mask`,
      );
    }
    const readiness = parsePrivacyExact12ReadinessV1(
      row.readiness,
      base,
      `${path}.readiness`,
    );
    return {
      protocol_id: base.protocol_id,
      operation_schema: operation,
      execution_mode: execution,
      privacy_feature_mask: featureMask,
      compiled_profile: base.compiled_profile,
      readiness,
      policy: base.policy,
    };
  });
  return deepFreeze({
    version: PRIVACY_EXACT12_CAPABILITY_MANIFEST_VERSION_V1,
    committed_height: committedHeight,
    consensus_policy: consensusPolicy,
    network_id: networkId,
    protocols,
    manifest_digest: manifestFixed32V1(
      manifest.manifest_digest,
      "Exact12 capability manifest.manifest_digest",
    ),
  });
}

function parsePrivacyExact12ReadinessV1(value, row, path) {
  const readiness = exactManifestObjectV1(value, ["readiness", "detail"], path);
  let expectedReadiness;
  let expectedReason = null;
  let expectedReasonDetail = null;
  if (row.compiled_profile.status === "unavailable") {
    expectedReadiness = "unavailable";
    expectedReason = "compiled-profile";
    expectedReasonDetail = row.compiled_profile.value;
  } else {
    expectedReadiness = "available";
  }
  if (readiness.readiness !== expectedReadiness) {
    manifestFailV1(
      `must equal evidence-derived ${expectedReadiness}`,
      `${path}.readiness`,
    );
  }
  if (expectedReadiness === "available") {
    if (readiness.detail !== null) {
      manifestFailV1("must have null detail", `${path}.detail`);
    }
    return { readiness: "available", detail: null };
  }
  const detail = exactManifestObjectV1(
    readiness.detail,
    ["reason", "detail"],
    `${path}.detail`,
  );
  if (detail.reason !== expectedReason) {
    manifestFailV1(
      `must equal evidence-derived ${expectedReason}`,
      `${path}.detail.reason`,
    );
  }
  if (!manifestValuesEqualV1(detail.detail, expectedReasonDetail)) {
    manifestFailV1(
      "does not match the evidence-derived unavailable detail",
      `${path}.detail.detail`,
    );
  }
  return {
    readiness: "unavailable",
    detail: {
      reason: expectedReason,
      detail: expectedReasonDetail,
    },
  };
}

function requirePrivacyExact12ProtocolIdV1(value) {
  if (typeof value !== "string") {
    throw new TypeError("protocolId must be an exact retained Exact12 identifier");
  }
  const index = PRIVACY_PROTOCOL_IDS_V1.indexOf(value);
  if (index < 0) {
    throw new TypeError(
      "protocolId must be one exact retained Exact12 identifier; aliases and retired identifiers are rejected",
    );
  }
  return index;
}

function exactManifestObjectV1(value, expectedKeys, path) {
  const prototype = value === null || typeof value !== "object"
    ? undefined
    : Object.getPrototypeOf(value);
  if (
    value === null
    || typeof value !== "object"
    || Array.isArray(value)
    || (prototype !== Object.prototype && prototype !== null)
  ) {
    manifestFailV1("must be a plain JSON object", path);
  }
  const keys = Object.keys(value).sort();
  const expected = [...expectedKeys].sort();
  if (
    keys.length !== expected.length
    || keys.some((key, index) => key !== expected[index])
  ) {
    manifestFailV1(`must contain exactly: ${expected.join(", ")}`, path);
  }
  return value;
}

function manifestTaggedUnitV1(value, tagKey, contentKey, expected, path) {
  const taggedValue = exactManifestObjectV1(value, [tagKey, contentKey], path);
  if (taggedValue[tagKey] !== expected || taggedValue[contentKey] !== null) {
    manifestFailV1(`must be the exact ${expected} unit variant`, path);
  }
  return { [tagKey]: expected, [contentKey]: null };
}

function manifestFixed32V1(value, path) {
  if (!Array.isArray(value) || value.length !== 32) {
    manifestFailV1("must contain exactly 32 bytes", path);
  }
  const bytes = value.map((byte, index) => {
    if (!Number.isInteger(byte) || byte < 0 || byte > 255) {
      manifestFailV1("must contain only uint8 values", `${path}[${index}]`);
    }
    return byte;
  });
  if (bytes.every((byte) => byte === 0)) {
    manifestFailV1("must be non-zero", path);
  }
  return bytes;
}

function manifestValuesEqualV1(left, right) {
  if (Object.is(left, right)) return true;
  if (typeof left !== typeof right || left === null || right === null) return false;
  if (Array.isArray(left) || Array.isArray(right)) {
    return Array.isArray(left)
      && Array.isArray(right)
      && left.length === right.length
      && left.every((item, index) => manifestValuesEqualV1(item, right[index]));
  }
  if (typeof left !== "object") return false;
  const leftKeys = Object.keys(left).sort();
  const rightKeys = Object.keys(right).sort();
  return leftKeys.length === rightKeys.length
    && leftKeys.every(
      (key, index) => key === rightKeys[index]
        && manifestValuesEqualV1(left[key], right[key]),
    );
}

function manifestFailV1(message, path) {
  throw new PrivacyExact12CapabilityManifestError(message, path);
}

/** @internal Copy authenticated original bytes for a native construction owner.
 * This establishes transport origin and exact network only; the native producer
 * must enforce the actual selected profile, request tuple and committed policy.
 */
export function privacyExact12AuthenticatedConstructionArchiveV1(manifest, expectedNetworkId) {
  const state = privacyExact12ManifestState.get(manifest);
  if (!state?.origin || !state.expectedNetworkId) {
    manifestFailV1("construction requires an authenticated Torii-origin manifest", "construction");
  }
  const expected = networkIdBytes(expectedNetworkId, "expectedNetworkId");
  if (!state.expectedNetworkId.every((byte, index) => byte === expected[index])) {
    manifestFailV1("construction belongs to a different network", "construction");
  }
  return Uint8Array.from(state.canonicalArchive);
}
