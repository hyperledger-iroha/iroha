/** Exact PinManifestFinalizedRecordV1 readback over the native model's JSON fields. */
import { Buffer } from "node:buffer";
import { rejectType, rejectRange } from "./validationThrow.js";

const U64_MAX = (1n << 64n) - 1n;

function record(value, required, optional, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value) || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) rejectType(`${context} must be a plain object`);
  const fields = {};
  const allowed = new Set([...required, ...optional]);
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (typeof key !== "string" || !allowed.has(key) || !descriptor?.enumerable || !("value" in descriptor)) rejectType(`${context} contains unknown or non-data fields`);
    fields[key] = descriptor.value;
  }
  for (const key of required) if (!Object.hasOwn(fields, key)) rejectType(`${context}.${key} is required`);
  return fields;
}

/** Native unsigned integer tokens remain numbers or bigints without coercion. */
export function normalizeSorafsPinU64(value, context, maximum = U64_MAX) {
  if (!(typeof value === "number" && Number.isSafeInteger(value) && !Object.is(value, -0)) && typeof value !== "bigint") rejectType(`${context} must be an exact unsigned JSON integer`);
  const integer = BigInt(value);
  if (integer < 0n || integer > maximum) rejectRange(`${context} exceeds its unsigned integer bound`);
  return integer <= BigInt(Number.MAX_SAFE_INTEGER) ? Number(integer) : integer;
}

function string(value, context) {
  if (typeof value !== "string") rejectType(`${context} must be a string`);
  return value;
}

function optional(value, normalize, context) {
  return value === null ? null : normalize(value, context);
}

function metadata(value, context, depth = 0) {
  if (depth > 128) rejectRange(`${context} exceeds its metadata nesting limit`);
  if (value === null || typeof value === "string" || typeof value === "boolean" || typeof value === "bigint") return value;
  if (typeof value === "number") {
    if (!Number.isFinite(value)) rejectType(`${context} must contain finite JSON numbers`);
    return value;
  }
  if (Array.isArray(value)) {
    if (Object.getPrototypeOf(value) !== Array.prototype || Reflect.ownKeys(value).length !== value.length + 1) rejectType(`${context} must be a dense JSON array`);
    return Array.from({ length: value.length }, (_, index) => {
      const field = Object.getOwnPropertyDescriptor(value, String(index));
      if (!field?.enumerable || !("value" in field)) rejectType(`${context} must contain data values`);
      return metadata(field.value, `${context}[${index}]`, depth + 1);
    });
  }
  if (value === undefined || typeof value !== "object" || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) rejectType(`${context} must contain JSON values`);
  const result = {};
  for (const key of Reflect.ownKeys(value)) {
    const field = Object.getOwnPropertyDescriptor(value, key);
    if (typeof key !== "string" || !field?.enumerable || !("value" in field)) rejectType(`${context} must contain exact JSON data fields`);
    Object.defineProperty(result, key, { value: metadata(field.value, `${context}.${key}`, depth + 1), enumerable: true, writable: true, configurable: true });
  }
  return result;
}

/** Bind scalar and identity validation to the existing Torii owners. */
export function createSorafsPinDetailNormalizer({ bytes, account, asset, quantity }) {
  const hash = (value, context) => bytes(value, context, { exactLength: 32 });
  const nonzeroHash = (value, context) => {
    const result = hash(value, context);
    if (result.every((byte) => byte === 0)) rejectType(`${context} must be non-zero`);
    return result;
  };
  const exact = (value, normalize, context) => {
    const result = normalize(value, context);
    if (result !== value) rejectType(`${context} must use its exact canonical identity`);
    return result;
  };
  function cursor(value, context) {
    const fields = record(value, ["height", "block_hash"], [], context);
    const height = normalizeSorafsPinU64(fields.height, `${context}.height`);
    if (height === 0) rejectType(`${context}.height must be positive`);
    return { height, block_hash: nonzeroHash(fields.block_hash, `${context}.block_hash`) };
  }
  function status(value, context) {
    const fields = record(value, ["status", "value"], [], context);
    if (!["Pending", "Approved", "Retired"].includes(fields.status)) rejectType(`${context}.status must be Pending, Approved, or Retired`);
    if (fields.status === "Pending") {
      if (fields.value !== null) rejectType(`${context}.value must be null for Pending`);
      return { status: "Pending", value: null };
    }
    return { status: fields.status, value: normalizeSorafsPinU64(fields.value, `${context}.value`) };
  }
  function alias(value, context) {
    const fields = record(value, ["name", "namespace", "proof"], [], context);
    const proof = string(fields.proof, `${context}.proof`);
    if (proof.length > 4 * Math.ceil(1024 * 1024 / 3) || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(proof) || Buffer.from(proof, "base64").length > 1024 * 1024 || Buffer.from(proof, "base64").toString("base64") !== proof) rejectType(`${context}.proof must contain exact bounded base64`);
    return { name: string(fields.name, `${context}.name`), namespace: string(fields.namespace, `${context}.namespace`), proof };
  }
  function fee(value, context) {
    const fields = record(value, ["paid_by", "fee_asset_id", "treasury_account_id", "amount"], [], context);
    return { paid_by: exact(fields.paid_by, account, `${context}.paid_by`), fee_asset_id: exact(fields.fee_asset_id, asset, `${context}.fee_asset_id`), treasury_account_id: exact(fields.treasury_account_id, account, `${context}.treasury_account_id`), amount: exact(fields.amount, quantity, `${context}.amount`) };
  }
  function manifest(value, context) {
    const required = ["digest", "root_cid", "chunker", "chunk_digest_sha3_256", "por_root", "content_length", "policy", "submitted_by", "submitted_epoch", "approved_epoch", "alias", "metadata", "status", "council_envelope_digest"];
    const fields = record(value, required, ["successor_of", "retirement_reason", "pin_fee_payment"], context);
    const chunker = record(fields.chunker, ["profile_id", "namespace", "name", "semver", "multihash_code"], [], `${context}.chunker`);
    const policy = record(fields.policy, ["min_replicas", "storage_class", "retention_epoch"], [], `${context}.policy`);
    const storage = record(policy.storage_class, ["type", "value"], [], `${context}.policy.storage_class`);
    if (!["Hot", "Warm", "Cold"].includes(storage.type) || storage.value !== null) rejectType(`${context}.policy.storage_class must be an exact native unit variant`);
    const rootCid = bytes(fields.root_cid, `${context}.root_cid`, { exactLength: 36 });
    if (![1, 0x71, 0x1f, 32].every((byte, index) => rootCid[index] === byte) || rootCid.subarray(4).every((byte) => byte === 0)) rejectType(`${context}.root_cid must be a canonical CIDv1/dag-cbor/BLAKE3-256 root`);
    if (fields.metadata === null || typeof fields.metadata !== "object" || Array.isArray(fields.metadata)) rejectType(`${context}.metadata must be an object`);
    const result = {
      digest: nonzeroHash(fields.digest, `${context}.digest`), root_cid: rootCid,
      chunker: { profile_id: normalizeSorafsPinU64(chunker.profile_id, `${context}.chunker.profile_id`, 0xffff_ffffn), namespace: string(chunker.namespace, `${context}.chunker.namespace`), name: string(chunker.name, `${context}.chunker.name`), semver: string(chunker.semver, `${context}.chunker.semver`), multihash_code: normalizeSorafsPinU64(chunker.multihash_code, `${context}.chunker.multihash_code`) },
      chunk_digest_sha3_256: hash(fields.chunk_digest_sha3_256, `${context}.chunk_digest_sha3_256`), por_root: hash(fields.por_root, `${context}.por_root`),
      content_length: normalizeSorafsPinU64(fields.content_length, `${context}.content_length`),
      policy: { min_replicas: normalizeSorafsPinU64(policy.min_replicas, `${context}.policy.min_replicas`, 0xffffn), storage_class: { type: storage.type, value: null }, retention_epoch: normalizeSorafsPinU64(policy.retention_epoch, `${context}.policy.retention_epoch`) },
      submitted_by: exact(fields.submitted_by, account, `${context}.submitted_by`), submitted_epoch: normalizeSorafsPinU64(fields.submitted_epoch, `${context}.submitted_epoch`), approved_epoch: optional(fields.approved_epoch, normalizeSorafsPinU64, `${context}.approved_epoch`),
      alias: optional(fields.alias, alias, `${context}.alias`), metadata: metadata(fields.metadata, `${context}.metadata`), status: status(fields.status, `${context}.status`), council_envelope_digest: optional(fields.council_envelope_digest, hash, `${context}.council_envelope_digest`),
    };
    if (Object.hasOwn(fields, "successor_of")) result.successor_of = nonzeroHash(fields.successor_of, `${context}.successor_of`);
    if (Object.hasOwn(fields, "retirement_reason")) result.retirement_reason = string(fields.retirement_reason, `${context}.retirement_reason`);
    if (Object.hasOwn(fields, "pin_fee_payment")) result.pin_fee_payment = fee(fields.pin_fee_payment, `${context}.pin_fee_payment`);
    // Same retained-history invariant as Core validate_stored_pin_approval_history.
    const submitted = BigInt(result.submitted_epoch);
    const approved = result.approved_epoch === null ? null : BigInt(result.approved_epoch);
    const retention = BigInt(result.policy.retention_epoch);
    const hasRetirementReason = Object.hasOwn(result, "retirement_reason");
    let validHistory;
    switch (result.status.status) {
      case "Pending":
        validHistory = approved === null
          && result.council_envelope_digest === null && !hasRetirementReason;
        break;
      case "Approved":
        validHistory = approved === BigInt(result.status.value)
          && approved >= submitted && approved < retention && !hasRetirementReason;
        break;
      case "Retired": {
        const retired = BigInt(result.status.value);
        validHistory = retired >= submitted
          && (approved === null || approved >= submitted
            && approved <= retired && approved < retention)
          && (approved !== null || result.council_envelope_digest === null);
        break;
      }
    }
    if (!validHistory) rejectType(`${context} has inconsistent retained approval lifecycle`);
    return result;
  }
  function normalize(value, expected = {}, context = "sorafs pin manifest response") {
    const fields = record(value, ["finalized_cursor", "manifest"], [], context);
    const result = { finalized_cursor: cursor(fields.finalized_cursor, `${context}.finalized_cursor`), manifest: manifest(fields.manifest, `${context}.manifest`) };
    if (expected.digestHex !== undefined && Buffer.from(result.manifest.digest).toString("hex") !== expected.digestHex) rejectType(`${context}.manifest.digest differs from the requested digest`);
    if (expected.height !== undefined && (BigInt(result.finalized_cursor.height) !== BigInt(expected.height) || Buffer.from(result.finalized_cursor.block_hash).toString("hex") !== expected.blockHashHex)) rejectType(`${context}.finalized_cursor differs from the requested finalized anchor`);
    return result;
  }
  return Object.freeze({ normalize, cursor, status });
}
