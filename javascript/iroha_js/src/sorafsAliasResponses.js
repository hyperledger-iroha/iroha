/**
 * Closed alias inventory projection emitted by Torii's SoraFS registry owner.
 * Attestation is committed-state metadata; parsing it does not verify finality
 * or establish trust in the alias proof's signer set.
 */
import { Buffer } from "node:buffer";
import { rejectType, rejectRange } from "./validationThrow.js";

const U64_MAX = (1n << 64n) - 1n;
const ALIAS_PAGE_MAX_ITEMS = 500;
const ALIAS_PROOF_MAX_BYTES = 1024 * 1024;
const DECISIONS = new Set(["serve", "hold", "refuse"]);
const REASONS = new Set([
  "RefreshWindow", "ExpiredTTL", "HardExpired", "RotationDue", "GovernanceGrace",
  "GovernanceRevoked", "GovernanceFrozen", "GovernanceRotated", "ManifestMissing",
  "LineageDepthExceeded", "LineageCycleDetected", "SuccessorForkResolved",
  "ApprovedSuccessorPending", "ApprovedSuccessorGrace", "ApprovedSuccessor",
  "MissingTimestamp", "PendingSuccessor",
]);
const ANOMALIES = new Set([
  "ManifestMissing", "SuccessorForkResolved", "LineageDepthExceeded", "LineageCycleDetected",
]);
const LABELS = new Set([
  "fresh", "fresh-rotate", "refresh", "refresh-rotate", "expired", "hard-expired",
  "lineage-invalid", "governance-refused", "successor-refused", "refresh-successor",
  "refresh-governance", "pending-successor",
]);

function record(value, required, optional, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value) ||
      ![Object.prototype, null].includes(Object.getPrototypeOf(value))) {
    rejectType(`${context} must be a plain object`);
  }
  const allowed = new Set([...required, ...optional]);
  const result = {};
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (typeof key !== "string" || !allowed.has(key) ||
        !descriptor?.enumerable || !("value" in descriptor)) {
      rejectType(`${context} must contain only its exact enumerable data fields`);
    }
    result[key] = descriptor.value;
  }
  for (const key of required) {
    if (!Object.hasOwn(result, key)) rejectType(`${context}.${key} is required`);
  }
  return result;
}

function integer(value, context, maximum = U64_MAX) {
  const validNumber = typeof value === "number" && Number.isSafeInteger(value) && !Object.is(value, -0);
  if (!validNumber && typeof value !== "bigint") rejectType(`${context} must be an exact unsigned JSON integer`);
  const wide = BigInt(value);
  if (wide < 0n || wide > maximum) rejectRange(`${context} exceeds its unsigned integer bound`);
  return wide <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(wide) : wide;
}

function text(value, context) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    rejectType(`${context} must be a nonempty unpadded string`);
  }
  return value;
}

function timestamp(value, context) {
  if (typeof value !== "string" || !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$/u.test(value)) {
    rejectType(`${context} must be a canonical UTC timestamp`);
  }
  const instant = new Date(value);
  if (!Number.isFinite(instant.getTime()) || instant.toISOString().replace(".000Z", "Z") !== value) {
    rejectType(`${context} must be a canonical UTC timestamp`);
  }
  return value;
}

function boolean(value, context) {
  if (typeof value !== "boolean") rejectType(`${context} must be a boolean`);
  return value;
}

function member(value, choices, context) {
  if (!choices.has(value)) rejectType(`${context} must use an exact first-release variant`);
  return value;
}

function hex(value, context) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)) {
    rejectType(`${context} must be 64 lowercase hexadecimal characters`);
  }
  return value;
}

function nullable(value, normalize, context) {
  return value === null ? null : normalize(value, context);
}

function array(value, normalize, context, maximum = Number.MAX_SAFE_INTEGER) {
  if (!Array.isArray(value) || value.length > maximum || Object.getPrototypeOf(value) !== Array.prototype) {
    rejectType(`${context} must be a bounded array`);
  }
  const keys = Reflect.ownKeys(value);
  if (keys.length !== value.length + 1) rejectType(`${context} must be a dense data array`);
  return Array.from({ length: value.length }, (_, index) => {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (!descriptor?.enumerable || !("value" in descriptor)) rejectType(`${context} must be a dense data array`);
    return normalize(descriptor.value, `${context}[${index}]`);
  });
}

function opaqueString(value, context) {
  if (typeof value !== "string") rejectType(`${context} must be a string`);
  return value;
}

function strings(value, choices, context, sorted) {
  const result = array(value, (entry, path) => choices ? member(entry, choices, path) : opaqueString(entry, path), context);
  if (new Set(result).size !== result.length || (sorted && result.some((entry, i) => i > 0 && Buffer.compare(Buffer.from(result[i - 1], "utf8"), Buffer.from(entry, "utf8")) >= 0))) {
    rejectType(`${context} must contain unique${sorted ? " sorted" : ""} values`);
  }
  return result;
}

function same(actual, expected, context) {
  if (actual !== expected) rejectType(`${context} contradicts its retained projection`);
}

function sameArray(actual, expected, context) {
  if (actual.length !== expected.length || actual.some((entry, index) => entry !== expected[index])) {
    rejectType(`${context} contradicts its retained projection`);
  }
}

function manifestStatus(value, context) {
  const fields = record(value, ["state"], ["epoch"], context);
  if (fields.state === "pending") {
    if (Object.hasOwn(fields, "epoch")) rejectType(`${context}.pending must omit epoch`);
    return { state: "pending" };
  }
  member(fields.state, new Set(["approved", "retired"]), `${context}.state`);
  return { state: fields.state, epoch: integer(fields.epoch, `${context}.epoch`) };
}

function lineageSuccessor(value, context) {
  const fields = record(value, ["digest_hex", "status", "approved_epoch", "approved_at", "status_timestamp_unix"], [], context);
  return {
    digest_hex: hex(fields.digest_hex, `${context}.digest_hex`),
    status: manifestStatus(fields.status, `${context}.status`),
    approved_epoch: nullable(fields.approved_epoch, integer, `${context}.approved_epoch`),
    approved_at: nullable(fields.approved_at, timestamp, `${context}.approved_at`),
    status_timestamp_unix: nullable(fields.status_timestamp_unix, integer, `${context}.status_timestamp_unix`),
  };
}

function lineage(value, context) {
  const fields = record(value, ["successor_of_hex", "head_hex", "depth_to_head", "is_head", "superseded_by", "immediate_successor", "anomalies"], [], context);
  const result = {
    successor_of_hex: nullable(fields.successor_of_hex, hex, `${context}.successor_of_hex`),
    head_hex: hex(fields.head_hex, `${context}.head_hex`),
    depth_to_head: integer(fields.depth_to_head, `${context}.depth_to_head`, 0xffff_ffffn),
    is_head: boolean(fields.is_head, `${context}.is_head`),
    superseded_by: nullable(fields.superseded_by, lineageSuccessor, `${context}.superseded_by`),
    immediate_successor: nullable(fields.immediate_successor, lineageSuccessor, `${context}.immediate_successor`),
    anomalies: strings(fields.anomalies, ANOMALIES, `${context}.anomalies`, false),
  };
  same(result.is_head, result.immediate_successor === null, `${context}.is_head`);
  return result;
}

function successor(value, context) {
  const fields = record(value, ["exists", "head_hex", "approved", "approved_at", "approved_at_unix", "depth_to_head", "anomalies"], [], context);
  return {
    exists: boolean(fields.exists, `${context}.exists`),
    head_hex: nullable(fields.head_hex, hex, `${context}.head_hex`),
    approved: boolean(fields.approved, `${context}.approved`),
    approved_at: nullable(fields.approved_at, timestamp, `${context}.approved_at`),
    approved_at_unix: nullable(fields.approved_at_unix, integer, `${context}.approved_at_unix`),
    depth_to_head: integer(fields.depth_to_head, `${context}.depth_to_head`, 0xffff_ffffn),
    anomalies: strings(fields.anomalies, ANOMALIES, `${context}.anomalies`, false),
  };
}

function governance(value, context) {
  const fields = record(value, ["ref_ids", "revoked", "frozen", "rotated", "flags", "effective_at_unix", "effective_at"], [], context);
  const flags = record(fields.flags, ["revoked", "frozen", "rotated"], [], `${context}.flags`);
  const result = {
    ref_ids: strings(fields.ref_ids, null, `${context}.ref_ids`, true),
    flags: {},
    effective_at_unix: nullable(fields.effective_at_unix, integer, `${context}.effective_at_unix`),
    effective_at: nullable(fields.effective_at, timestamp, `${context}.effective_at`),
  };
  for (const key of ["revoked", "frozen", "rotated"]) {
    result[key] = boolean(fields[key], `${context}.${key}`);
    result.flags[key] = boolean(flags[key], `${context}.flags.${key}`);
    same(result[key], result.flags[key], `${context}.flags.${key}`);
  }
  return result;
}

function evaluation(value, context) {
  const fields = record(value, ["decision", "reasons", "ttl_expires_at", "ttl_expires_at_unix", "serve_until", "serve_until_unix", "successor", "governance", "policy_successor_grace_secs", "policy_governance_grace_secs"], [], context);
  return {
    decision: member(fields.decision, DECISIONS, `${context}.decision`),
    reasons: strings(fields.reasons, REASONS, `${context}.reasons`, true),
    ttl_expires_at: nullable(fields.ttl_expires_at, timestamp, `${context}.ttl_expires_at`),
    ttl_expires_at_unix: integer(fields.ttl_expires_at_unix, `${context}.ttl_expires_at_unix`),
    serve_until: nullable(fields.serve_until, timestamp, `${context}.serve_until`),
    serve_until_unix: nullable(fields.serve_until_unix, integer, `${context}.serve_until_unix`),
    successor: successor(fields.successor, `${context}.successor`),
    governance: governance(fields.governance, `${context}.governance`),
    policy_successor_grace_secs: integer(fields.policy_successor_grace_secs, `${context}.policy_successor_grace_secs`),
    policy_governance_grace_secs: integer(fields.policy_governance_grace_secs, `${context}.policy_governance_grace_secs`),
  };
}

const ALIAS_INTEGER_FIELDS = [
  "bound_epoch", "expiry_epoch", "cache_age_seconds", "proof_generated_at_unix", "proof_expires_at_unix",
  "policy_positive_ttl_secs", "policy_refresh_window_secs", "policy_hard_expiry_secs",
  "policy_rotation_max_age_secs", "policy_successor_grace_secs", "policy_governance_grace_secs",
];
const ALIAS_REQUIRED_FIELDS = [
  "alias", "namespace", "name", "manifest_digest_hex", "bound_by", "proof_b64", "cache_state", "status_label",
  "cache_rotation_due", "cache_evaluation", "cache_decision", "cache_reasons", "lineage", ...ALIAS_INTEGER_FIELDS,
];

/** Create the projection reader with the caller's canonical account owner. */
export function createSorafsAliasResponseNormalizers({ requireAccountId }) {
  if (typeof requireAccountId !== "function") rejectType("canonical account reader is required");
  function normalizeSorafsAliasRecord(payload, context) {
    const fields = record(payload, ALIAS_REQUIRED_FIELDS, ["proof_expires_in_seconds"], context);
    const result = {};
    for (const key of ["alias", "namespace", "name"]) result[key] = text(fields[key], `${context}.${key}`);
    same(result.alias, `${result.namespace}/${result.name}`, `${context}.alias`);
    result.manifest_digest_hex = hex(fields.manifest_digest_hex, `${context}.manifest_digest_hex`);
    result.bound_by = requireAccountId(fields.bound_by, `${context}.bound_by`);
    same(result.bound_by, fields.bound_by, `${context}.bound_by`);
    const proof = text(fields.proof_b64, `${context}.proof_b64`);
    if (proof.length > 4 * Math.ceil(ALIAS_PROOF_MAX_BYTES / 3) ||
        !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(proof)) {
      rejectType(`${context}.proof_b64 must contain canonical bounded base64`);
    }
    const bytes = Buffer.from(proof, "base64");
    if (bytes.length === 0 || bytes.length > ALIAS_PROOF_MAX_BYTES || bytes.toString("base64") !== proof) {
      rejectType(`${context}.proof_b64 must contain canonical bounded base64`);
    }
    result.proof_b64 = proof;
    for (const key of ALIAS_INTEGER_FIELDS) result[key] = integer(fields[key], `${context}.${key}`);
    if (Object.hasOwn(fields, "proof_expires_in_seconds")) {
      result.proof_expires_in_seconds = integer(fields.proof_expires_in_seconds, `${context}.proof_expires_in_seconds`);
    }
    result.cache_state = member(fields.cache_state, LABELS, `${context}.cache_state`);
    result.status_label = member(fields.status_label, LABELS, `${context}.status_label`);
    same(result.cache_state, result.status_label, `${context}.cache_state`);
    result.cache_rotation_due = boolean(fields.cache_rotation_due, `${context}.cache_rotation_due`);
    result.cache_evaluation = evaluation(fields.cache_evaluation, `${context}.cache_evaluation`);
    result.cache_decision = member(fields.cache_decision, DECISIONS, `${context}.cache_decision`);
    result.cache_reasons = strings(fields.cache_reasons, REASONS, `${context}.cache_reasons`, true);
    result.lineage = lineage(fields.lineage, `${context}.lineage`);
    same(result.cache_decision, result.cache_evaluation.decision, `${context}.cache_decision`);
    sameArray(result.cache_reasons, result.cache_evaluation.reasons, `${context}.cache_reasons`);
    same(result.proof_expires_at_unix, result.cache_evaluation.ttl_expires_at_unix, `${context}.proof_expires_at_unix`);
    for (const key of ["policy_successor_grace_secs", "policy_governance_grace_secs"]) {
      same(result[key], result.cache_evaluation[key], `${context}.${key}`);
    }
    const next = result.cache_evaluation.successor;
    same(next.head_hex, result.lineage.head_hex, `${context}.cache_evaluation.successor.head_hex`);
    same(next.depth_to_head, result.lineage.depth_to_head, `${context}.cache_evaluation.successor.depth_to_head`);
    sameArray(next.anomalies, result.lineage.anomalies, `${context}.cache_evaluation.successor.anomalies`);
    same(next.exists, result.lineage.immediate_successor !== null, `${context}.cache_evaluation.successor.exists`);
    same(next.approved, result.lineage.superseded_by !== null, `${context}.cache_evaluation.successor.approved`);
    return result;
  }

  function normalizeSorafsAliasListResponse(payload, context = "sorafs alias list response") {
    const fields = record(payload, ["attestation", "total_count", "returned_count", "offset", "limit", "aliases"], [], context);
    const attestation = record(fields.attestation, ["block_height", "block_hash_hex", "chain_id"], [], `${context}.attestation`);
    const result = {
      attestation: {
        block_height: integer(attestation.block_height, `${context}.attestation.block_height`),
        block_hash_hex: nullable(attestation.block_hash_hex, hex, `${context}.attestation.block_hash_hex`),
        chain_id: text(attestation.chain_id, `${context}.attestation.chain_id`),
      },
      total_count: integer(fields.total_count, `${context}.total_count`, BigInt(Number.MAX_SAFE_INTEGER)),
      returned_count: integer(fields.returned_count, `${context}.returned_count`, BigInt(ALIAS_PAGE_MAX_ITEMS)),
      offset: integer(fields.offset, `${context}.offset`, BigInt(Number.MAX_SAFE_INTEGER)),
      limit: integer(fields.limit, `${context}.limit`, BigInt(ALIAS_PAGE_MAX_ITEMS)),
      aliases: array(fields.aliases, normalizeSorafsAliasRecord, `${context}.aliases`, ALIAS_PAGE_MAX_ITEMS),
    };
    if (result.limit === 0 || result.returned_count !== result.aliases.length || result.returned_count > result.limit ||
        result.offset > result.total_count || result.returned_count > result.total_count - result.offset) {
      rejectType(`${context} has inconsistent pagination counts`);
    }
    return result;
  }
  return Object.freeze({ normalizeSorafsAliasRecord, normalizeSorafsAliasListResponse });
}
