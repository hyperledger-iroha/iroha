/** Transport-only ABI-21/manifest-V4 Kagemusha projections.
 *
 * This module deliberately has no native prover or artifact-install surface.
 * Top-up and redemption methods accept canonical Norito archives produced by
 * a supported wallet/prover implementation.
 */

export const KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION = 21;
export const KAGEMUSHA_MANIFEST_VERSION = 4;
export const KAGEMUSHA_MAX_HOPS = 8;
export const KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES = 512 * 1024;
export const KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES = 48 * 1024 * 1024;

const HASH_32 = /^[0-9a-f]{64}$/u;
const OPERATION_ID = /^(?!0{64}$)[0-9a-f]{64}$/u;
const BLOCKER_CODE = /^[a-z0-9_]{1,64}$/u;
const EXACT_JSON_MEDIA_TYPE =
  /^[ \t]*application\/json(?:[ \t]*;[ \t]*[!#$%&'*+\-.^_`|~0-9A-Za-z]+=(?:[!#$%&'*+\-.^_`|~0-9A-Za-z]+|"(?:[ \t!#-\[\]-~\u0080-\u00ff]|\\[ \t!-~\u0080-\u00ff])*"))*[ \t]*$/iu;
const REQUIRED_READINESS_FIELDS = Object.freeze([
  "required_bridge_abi_version",
  "max_hops",
  "asset_definition_id",
  "asset_scale",
  "evaluated_block_height",
  "evaluated_block_hash",
  "active_transfer_verifier",
  "active_topup_shield_verifier",
  "active_unshield_verifier",
  "active_recursive_step_eq_verifier",
  "active_recursive_step_ep_verifier",
  "artifact_set",
  "proof_backend_available",
  "recursive_lineage_supported",
  "ready",
  "blockers",
]);
const VERIFIER_FIELDS = Object.freeze([
  "active_transfer_verifier",
  "active_topup_shield_verifier",
  "active_unshield_verifier",
  "active_recursive_step_eq_verifier",
  "active_recursive_step_ep_verifier",
]);
const VERIFIER_IDENTITIES = Object.freeze({
  active_transfer_verifier: Object.freeze({
    name: "confidential_transfer_v2_verifier_record",
    circuit: "halo2/pasta/ipa/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
  }),
  active_topup_shield_verifier: Object.freeze({
    name: "kagemusha_topup_shield_v2_verifier_record",
    circuit: "halo2/pasta/ipa/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
  }),
  active_unshield_verifier: Object.freeze({
    name: "confidential_unshield_v3_verifier_record",
    circuit: "halo2/pasta/ipa/confidential-unshield-change-merkle16-axiom-poseidon-v4",
  }),
  active_recursive_step_eq_verifier: Object.freeze({
    name: "kagemusha_recursive_step_eq_v4_verifier_record",
    circuit: "kagemusha-recursive-spend-step-eq-compact-layout-v5",
  }),
  active_recursive_step_ep_verifier: Object.freeze({
    name: "kagemusha_recursive_step_ep_v4_verifier_record",
    circuit: "kagemusha-recursive-spend-step-ep-compact-lineage-v5",
  }),
});

function record(value, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function exactFields(value, context, fields) {
  const item = record(value, context);
  const actual = Object.keys(item);
  const expected = new Set(fields);
  if (actual.length !== fields.length || actual.some((field) => !expected.has(field))) {
    throw new TypeError(`${context} contains missing or unknown fields`);
  }
  return item;
}

function exactString(value, context, { maximum = 1024 } = {}) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.length > maximum ||
    value.trim() !== value ||
    /[\u0000-\u001f\u007f]/u.test(value)
  ) {
    throw new TypeError(`${context} must be exact non-empty text`);
  }
  return value;
}

function safeUnsigned(value, context, { positive = false, maximum = Number.MAX_SAFE_INTEGER } = {}) {
  if (!Number.isSafeInteger(value) || value < (positive ? 1 : 0) || value > maximum) {
    throw new TypeError(`${context} must be a${positive ? " positive" : "n"} safe unsigned integer`);
  }
  return value;
}

function boolean(value, context) {
  if (typeof value !== "boolean") {
    throw new TypeError(`${context} must be a boolean`);
  }
  return value;
}

function hash32(value, context, { nonzero = false } = {}) {
  if (typeof value !== "string" || !HASH_32.test(value) || (nonzero && value === "0".repeat(64))) {
    throw new TypeError(`${context} must be ${nonzero ? "non-zero " : ""}lowercase 32-byte hexadecimal`);
  }
  return value;
}

function jsonSnapshot(value) {
  if (typeof structuredClone === "function") {
    return structuredClone(value);
  }
  return JSON.parse(JSON.stringify(value));
}

function normalizeVerifier(value, field, evaluatedBlockHeight) {
  const context = `Kagemusha readiness.${field}`;
  const item = exactFields(value, context, [
    "id",
    "version",
    "circuit_id",
    "commitment",
    "public_inputs_schema_hash",
    "max_proof_bytes",
    "activation_height",
    "withdrawal_height",
  ]);
  const id = exactFields(item.id, `${context}.id`, ["backend", "name"]);
  const identity = VERIFIER_IDENTITIES[field];
  const backend = exactString(id.backend, `${context}.id.backend`, { maximum: 256 });
  const name = exactString(id.name, `${context}.id.name`, { maximum: 256 });
  const circuitId = exactString(item.circuit_id, `${context}.circuit_id`, { maximum: 512 });
  if (backend !== "halo2/ipa" || name !== identity.name || circuitId !== identity.circuit) {
    throw new TypeError(`${context} does not identify its production verifier role`);
  }
  const activationHeight = safeUnsigned(item.activation_height, `${context}.activation_height`);
  const withdrawalHeight = item.withdrawal_height === null
    ? null
    : safeUnsigned(item.withdrawal_height, `${context}.withdrawal_height`, { positive: true });
  if (
    activationHeight > evaluatedBlockHeight ||
    (withdrawalHeight !== null && withdrawalHeight <= evaluatedBlockHeight)
  ) {
    throw new TypeError(`${context} is not active at the evaluated block`);
  }
  return Object.freeze({
    id: Object.freeze({ backend, name }),
    version: safeUnsigned(item.version, `${context}.version`, { positive: true, maximum: 0xffff_ffff }),
    circuit_id: circuitId,
    commitment: hash32(item.commitment, `${context}.commitment`, { nonzero: true }),
    public_inputs_schema_hash: hash32(
      item.public_inputs_schema_hash,
      `${context}.public_inputs_schema_hash`,
      { nonzero: true },
    ),
    max_proof_bytes: safeUnsigned(item.max_proof_bytes, `${context}.max_proof_bytes`, {
      positive: true,
      maximum: 0xffff_ffff,
    }),
    activation_height: activationHeight,
    withdrawal_height: withdrawalHeight,
  });
}

function normalizeArtifactSet(value, evaluatedBlockHeight, assetScale) {
  const context = "Kagemusha readiness.artifact_set";
  const item = exactFields(value, context, [
    "generation",
    "manifest_sha256",
    "release_policy_sha256",
    "release_attestation_sha256",
    "activation_height",
    "withdrawal_height",
    "max_proof_bytes",
    "asset_scale",
  ]);
  const generation = exactString(item.generation, `${context}.generation`, { maximum: 128 });
  if (!/^[A-Za-z0-9](?:[A-Za-z0-9._-]{0,126}[A-Za-z0-9])?$/u.test(generation)) {
    throw new TypeError(`${context}.generation must be a portable artifact identifier`);
  }
  const digests = [
    hash32(item.manifest_sha256, `${context}.manifest_sha256`, { nonzero: true }),
    hash32(item.release_policy_sha256, `${context}.release_policy_sha256`, { nonzero: true }),
    hash32(item.release_attestation_sha256, `${context}.release_attestation_sha256`, { nonzero: true }),
  ];
  if (new Set(digests).size !== digests.length) {
    throw new TypeError(`${context} digests must be pairwise distinct`);
  }
  const activationHeight = safeUnsigned(item.activation_height, `${context}.activation_height`, {
    positive: true,
  });
  const withdrawalHeight = safeUnsigned(item.withdrawal_height, `${context}.withdrawal_height`, {
    positive: true,
  });
  if (
    withdrawalHeight <= activationHeight ||
    activationHeight > evaluatedBlockHeight ||
    withdrawalHeight <= evaluatedBlockHeight
  ) {
    throw new TypeError(`${context} is not active at the evaluated block`);
  }
  const artifactScale = safeUnsigned(item.asset_scale, `${context}.asset_scale`, { maximum: 28 });
  if (artifactScale !== assetScale) {
    throw new TypeError(`${context}.asset_scale must equal the authoritative asset scale`);
  }
  return Object.freeze({
    generation,
    manifest_sha256: digests[0],
    release_policy_sha256: digests[1],
    release_attestation_sha256: digests[2],
    activation_height: activationHeight,
    withdrawal_height: withdrawalHeight,
    max_proof_bytes: safeUnsigned(item.max_proof_bytes, `${context}.max_proof_bytes`, {
      positive: true,
      maximum: 16 * 1024 * 1024,
    }),
    asset_scale: artifactScale,
  });
}

export function normalizeKagemushaAssetSelector(value, context = "assetDefinitionId") {
  return exactString(value, context, { maximum: 512 });
}

export function normalizeKagemushaOperationId(value, context = "operationId") {
  if (typeof value !== "string" || !OPERATION_ID.test(value)) {
    throw new TypeError(`${context} must be non-zero lowercase 32-byte hexadecimal`);
  }
  return value;
}

export function requireKagemushaJsonContentType(value, context) {
  if (typeof value !== "string" || !EXACT_JSON_MEDIA_TYPE.test(value)) {
    throw new TypeError(`${context} must use Content-Type application/json`);
  }
}

export function normalizeKagemushaReadinessV4(payload, requestedAssetSelector) {
  const context = "Kagemusha readiness";
  const requested = normalizeKagemushaAssetSelector(requestedAssetSelector, "requested asset selector");
  const item = exactFields(payload, context, REQUIRED_READINESS_FIELDS);
  if (safeUnsigned(item.required_bridge_abi_version, `${context}.required_bridge_abi_version`, {
    positive: true,
    maximum: 0xffff_ffff,
  }) !== KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION) {
    throw new TypeError(`${context}.required_bridge_abi_version must be 21`);
  }
  if (safeUnsigned(item.max_hops, `${context}.max_hops`, {
    positive: true,
    maximum: 0xffff_ffff,
  }) !== KAGEMUSHA_MAX_HOPS) {
    throw new TypeError(`${context}.max_hops must be 8`);
  }
  const assetDefinitionId = exactString(item.asset_definition_id, `${context}.asset_definition_id`, {
    maximum: 512,
  });
  if (requested.includes("#") && assetDefinitionId !== requested) {
    throw new TypeError(`${context}.asset_definition_id does not match the requested canonical asset`);
  }
  const assetScale = item.asset_scale === null
    ? null
    : safeUnsigned(item.asset_scale, `${context}.asset_scale`, { maximum: 0xffff_ffff });
  const evaluatedBlockHeight = safeUnsigned(
    item.evaluated_block_height,
    `${context}.evaluated_block_height`,
  );
  const verifiers = {};
  for (const field of VERIFIER_FIELDS) {
    verifiers[field] = item[field] === null
      ? null
      : normalizeVerifier(item[field], field, evaluatedBlockHeight);
  }
  const hasEq = verifiers.active_recursive_step_eq_verifier !== null;
  const hasEp = verifiers.active_recursive_step_ep_verifier !== null;
  if (hasEq !== hasEp) {
    throw new TypeError(`${context} must report the ABI-21 V4 Eq/Ep verifier pair atomically`);
  }
  const artifactSet = item.artifact_set === null
    ? null
    : normalizeArtifactSet(item.artifact_set, evaluatedBlockHeight, assetScale);
  if ((artifactSet !== null) !== hasEq) {
    throw new TypeError(`${context}.artifact_set and the ABI-21 V4 Eq/Ep pair must be reported together`);
  }
  if (
    artifactSet !== null &&
    (artifactSet.max_proof_bytes !== verifiers.active_recursive_step_eq_verifier.max_proof_bytes ||
      artifactSet.max_proof_bytes !== verifiers.active_recursive_step_ep_verifier.max_proof_bytes)
  ) {
    throw new TypeError(`${context}.artifact_set.max_proof_bytes must match both recursive verifiers`);
  }
  const proofBackendAvailable = boolean(
    item.proof_backend_available,
    `${context}.proof_backend_available`,
  );
  if (proofBackendAvailable && artifactSet === null) {
    throw new TypeError(`${context}.proof_backend_available requires an authenticated artifact_set`);
  }
  const recursiveLineageSupported = boolean(
    item.recursive_lineage_supported,
    `${context}.recursive_lineage_supported`,
  );
  const expectedLineage = proofBackendAvailable && artifactSet !== null && hasEq && hasEp;
  if (recursiveLineageSupported !== expectedLineage) {
    throw new TypeError(`${context}.recursive_lineage_supported contradicts the ABI-21 runtime conjunction`);
  }
  if (!Array.isArray(item.blockers)) {
    throw new TypeError(`${context}.blockers must be an array`);
  }
  const blockerCodes = new Set();
  const blockers = item.blockers.map((value, index) => {
    const blockerContext = `${context}.blockers[${index}]`;
    const blocker = exactFields(value, blockerContext, ["code", "message"]);
    const code = exactString(blocker.code, `${blockerContext}.code`, { maximum: 64 });
    if (!BLOCKER_CODE.test(code) || blockerCodes.has(code)) {
      throw new TypeError(`${blockerContext}.code must be unique stable lowercase text`);
    }
    blockerCodes.add(code);
    return Object.freeze({
      code,
      message: exactString(blocker.message, `${blockerContext}.message`),
    });
  });
  const ready = boolean(item.ready, `${context}.ready`);
  const expectedReady =
    expectedLineage &&
    assetScale !== null &&
    assetScale <= 28 &&
    verifiers.active_transfer_verifier !== null &&
    verifiers.active_topup_shield_verifier !== null &&
    verifiers.active_unshield_verifier !== null &&
    blockers.length === 0;
  if (ready !== expectedReady) {
    throw new TypeError(`${context}.ready contradicts the complete ABI-21 runtime conjunction`);
  }
  return Object.freeze({
    required_bridge_abi_version: KAGEMUSHA_REQUIRED_BRIDGE_ABI_VERSION,
    max_hops: KAGEMUSHA_MAX_HOPS,
    asset_definition_id: assetDefinitionId,
    asset_scale: assetScale,
    evaluated_block_height: evaluatedBlockHeight,
    evaluated_block_hash: hash32(item.evaluated_block_hash, `${context}.evaluated_block_hash`),
    ...verifiers,
    artifact_set: artifactSet,
    proof_backend_available: proofBackendAvailable,
    recursive_lineage_supported: recursiveLineageSupported,
    ready,
    blockers: Object.freeze(blockers),
  });
}

function normalizeKagemushaNoritoRequestV4(value, maximumBytes, context) {
  const item = exactFields(value, context, ["version", "operationId", "norito"]);
  if (item.version !== KAGEMUSHA_MANIFEST_VERSION) {
    throw new TypeError(`${context}.version must be 4; V3 archives are not upgraded`);
  }
  if (!ArrayBuffer.isView(item.norito) || item.norito.BYTES_PER_ELEMENT !== 1) {
    throw new TypeError(`${context}.norito must be a byte-array view`);
  }
  const archive = new Uint8Array(
    item.norito.buffer,
    item.norito.byteOffset,
    item.norito.byteLength,
  );
  if (
    archive.byteLength < 40 ||
    archive.byteLength > maximumBytes ||
    archive[0] !== 0x4e ||
    archive[1] !== 0x52 ||
    archive[2] !== 0x54 ||
    archive[3] !== 0x30
  ) {
    throw new TypeError(`${context}.norito must be a bounded canonical Norito archive`);
  }
  return Object.freeze({
    version: KAGEMUSHA_MANIFEST_VERSION,
    operationId: normalizeKagemushaOperationId(item.operationId, `${context}.operationId`),
    norito: new Uint8Array(archive),
  });
}

export function normalizeKagemushaTopUpRequestV4(
  value,
  context = "Kagemusha V4 top-up request",
) {
  return normalizeKagemushaNoritoRequestV4(
    value,
    KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES,
    context,
  );
}

export function normalizeKagemushaRedeemRequestV4(
  value,
  context = "Kagemusha V4 redemption request",
) {
  return normalizeKagemushaNoritoRequestV4(
    value,
    KAGEMUSHA_REDEEM_REQUEST_MAX_BYTES,
    context,
  );
}

function taggedKind(value, context) {
  const item = exactFields(value, context, ["kind", "value"]);
  if (item.value !== null || (item.kind !== "top_up" && item.kind !== "redeem")) {
    throw new TypeError(`${context} must be a top_up or redeem unit tag`);
  }
  return item.kind;
}

function taggedPending(value, context) {
  const item = exactFields(value, context, ["state", "value"]);
  if (item.state !== "pending" || item.value !== null) {
    throw new TypeError(`${context} must be a pending unit tag`);
  }
  return "pending";
}

export function normalizeKagemushaOperationReference(
  payload,
  { expectedOperationId, expectedKind, location },
) {
  const context = "Kagemusha operation reference";
  const item = exactFields(payload, context, [
    "operation_id",
    "kind",
    "state",
    "transaction_hash",
    "status_uri",
    "submitted_at_ms",
  ]);
  const operationId = normalizeKagemushaOperationId(item.operation_id, `${context}.operation_id`);
  const kind = taggedKind(item.kind, `${context}.kind`);
  taggedPending(item.state, `${context}.state`);
  const statusUri = `/v1/offline/operations/${operationId}`;
  if (
    operationId !== expectedOperationId ||
    kind !== expectedKind ||
    item.status_uri !== statusUri ||
    location !== statusUri
  ) {
    throw new TypeError(`${context} does not match the submitted V4 command`);
  }
  return Object.freeze({
    operation_id: operationId,
    kind: Object.freeze({ kind, value: null }),
    state: Object.freeze({ state: "pending", value: null }),
    transaction_hash: hash32(item.transaction_hash, `${context}.transaction_hash`),
    status_uri: statusUri,
    submitted_at_ms: safeUnsigned(item.submitted_at_ms, `${context}.submitted_at_ms`),
  });
}

function bytes32(value, context) {
  if (
    !Array.isArray(value) ||
    value.length !== 32 ||
    value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255)
  ) {
    throw new TypeError(`${context} must be an array of 32 bytes`);
  }
  return value;
}

function bytesToHex(value) {
  return value.map((byte) => byte.toString(16).padStart(2, "0")).join("");
}

function normalizeAppliedResult(value, operationId, context) {
  const tagged = exactFields(value, context, ["kind", "result"]);
  const result = record(tagged.result, `${context}.result`);
  if (tagged.kind === "redeem") {
    exactFields(result, `${context}.result`, [
      "transaction_hash",
      "finalized_block_height",
      "server_time_ms",
    ]);
    return Object.freeze({
      kind: "redeem",
      result: Object.freeze({
        transaction_hash: hash32(result.transaction_hash, `${context}.result.transaction_hash`),
        finalized_block_height: safeUnsigned(
          result.finalized_block_height,
          `${context}.result.finalized_block_height`,
        ),
        server_time_ms: safeUnsigned(result.server_time_ms, `${context}.result.server_time_ms`),
      }),
    });
  }
  if (tagged.kind !== "top_up") {
    throw new TypeError(`${context}.kind must be top_up or redeem`);
  }
  exactFields(result, `${context}.result`, [
    "transaction_hash",
    "finalized_block_height",
    "server_time_ms",
    "anchor",
    "finality_proof",
  ]);
  const anchor = record(result.anchor, `${context}.result.anchor`);
  const artifactBinding = record(
    anchor.artifact_binding,
    `${context}.result.anchor.artifact_binding`,
  );
  if (anchor.version !== 4 || artifactBinding.version !== 4) {
    throw new TypeError(`${context}.result anchor and artifact binding must use V4`);
  }
  if (bytesToHex(bytes32(anchor.topup_operation_id, `${context}.result.anchor.topup_operation_id`)) !== operationId) {
    throw new TypeError(`${context}.result.anchor.topup_operation_id does not match the operation`);
  }
  const finalityProof = record(result.finality_proof, `${context}.result.finality_proof`);
  const finalityAnchor = record(finalityProof.anchor, `${context}.result.finality_proof.anchor`);
  if (
    finalityProof.version !== 1 ||
    bytesToHex(bytes32(
      finalityAnchor.topup_operation_id,
      `${context}.result.finality_proof.anchor.topup_operation_id`,
    )) !== operationId
  ) {
    throw new TypeError(`${context}.result.finality_proof does not bind the V4 top-up operation`);
  }
  return Object.freeze({
    kind: "top_up",
    result: Object.freeze({
      transaction_hash: hash32(result.transaction_hash, `${context}.result.transaction_hash`),
      finalized_block_height: safeUnsigned(
        result.finalized_block_height,
        `${context}.result.finalized_block_height`,
      ),
      server_time_ms: safeUnsigned(result.server_time_ms, `${context}.result.server_time_ms`),
      anchor: jsonSnapshot(anchor),
      finality_proof: jsonSnapshot(finalityProof),
    }),
  });
}

export function normalizeKagemushaOperationStatus(payload, expectedOperationId) {
  const context = "Kagemusha operation status";
  const expected = normalizeKagemushaOperationId(expectedOperationId, "expected operation id");
  const item = exactFields(payload, context, ["state", "value"]);
  const value = record(item.value, `${context}.value`);
  const operationId = normalizeKagemushaOperationId(
    value.operation_id,
    `${context}.value.operation_id`,
  );
  if (operationId !== expected) {
    throw new TypeError(`${context}.value.operation_id does not match the requested operation`);
  }
  if (item.state === "pending") {
    exactFields(value, `${context}.value`, [
      "operation_id",
      "kind",
      "transaction_hash",
      "submitted_at_ms",
    ]);
    return Object.freeze({
      state: "pending",
      value: Object.freeze({
        operation_id: operationId,
        kind: Object.freeze({
          kind: taggedKind(value.kind, `${context}.value.kind`),
          value: null,
        }),
        transaction_hash: hash32(value.transaction_hash, `${context}.value.transaction_hash`),
        submitted_at_ms: safeUnsigned(value.submitted_at_ms, `${context}.value.submitted_at_ms`),
      }),
    });
  }
  if (item.state === "applied") {
    exactFields(value, `${context}.value`, ["operation_id", "result"]);
    return Object.freeze({
      state: "applied",
      value: Object.freeze({
        operation_id: operationId,
        result: normalizeAppliedResult(value.result, operationId, `${context}.value.result`),
      }),
    });
  }
  if (item.state !== "rejected") {
    throw new TypeError(`${context}.state must be pending, applied, or rejected`);
  }
  exactFields(value, `${context}.value`, [
    "operation_id",
    "kind",
    "transaction_hash",
    "error",
  ]);
  const error = record(value.error, `${context}.value.error`);
  const normalizedError = {
    code: exactString(error.code, `${context}.value.error.code`, { maximum: 128 }),
    message: exactString(error.message, `${context}.value.error.message`),
  };
  if (error.details !== undefined && error.details !== null) {
    normalizedError.details = jsonSnapshot(record(error.details, `${context}.value.error.details`));
  }
  return Object.freeze({
    state: "rejected",
    value: Object.freeze({
      operation_id: operationId,
      kind: Object.freeze({
        kind: taggedKind(value.kind, `${context}.value.kind`),
        value: null,
      }),
      transaction_hash: hash32(value.transaction_hash, `${context}.value.transaction_hash`),
      error: Object.freeze(normalizedError),
    }),
  });
}
