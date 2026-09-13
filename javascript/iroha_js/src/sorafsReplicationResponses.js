/** Exact registry.rs replication inventory JSON, preserving current completion evidence. */
import { Buffer } from "node:buffer";
import { SORAFS_REPLICATION_ORDER_CHUNKER_HANDLES_V1 } from "./sorafsReplicationProfiles.js";
import { normalizeSorafsPinU64 as u64 } from "./sorafsPinDetail.js";

function fail(context, message) { throw new TypeError(`${context} ${message}`); }
function record(value, fields, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)
      || ![Object.prototype, null].includes(Object.getPrototypeOf(value))) fail(context, "must be a plain object");
  const allowed = new Set(fields);
  for (const key of Reflect.ownKeys(value)) {
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (typeof key !== "string" || !allowed.has(key) || !descriptor?.enumerable
        || !("value" in descriptor)) fail(context, "contains unknown or non-data fields");
  }
  for (const key of fields) if (!Object.hasOwn(value, key)) fail(`${context}.${key}`, "is required");
  return value;
}
function array(value, maximum, context, normalize) {
  if (!Array.isArray(value) || Object.getPrototypeOf(value) !== Array.prototype
      || value.length > maximum || Reflect.ownKeys(value).length !== value.length + 1) fail(context, "must be a bounded dense array");
  return Array.from({ length: value.length }, (_, index) => {
    const descriptor = Object.getOwnPropertyDescriptor(value, String(index));
    if (!descriptor?.enumerable || !("value" in descriptor)) fail(context, "must contain data values");
    return normalize(descriptor.value, `${context}[${index}]`);
  });
}
function text(value, context) {
  if (typeof value !== "string") fail(context, "must be a string");
  return value;
}
function hex(value, context, nonzero = true) {
  if (typeof value !== "string" || !/^[0-9a-f]{64}$/u.test(value)
      || nonzero && /^0+$/u.test(value)) fail(context, "must be exact non-zero lowercase 32-byte hex");
  return value;
}
function positive(value, context, maximum) {
  const result = u64(value, context, maximum);
  if (result === 0) fail(context, "must be positive");
  return result;
}
function base64(value, context, maximum) {
  if (typeof value !== "string" || value.length === 0
      || value.length > 4 * Math.ceil(maximum / 3)
      || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u.test(value)) fail(context, "must be bounded canonical base64");
  const bytes = Buffer.from(value, "base64");
  if (bytes.length > maximum || bytes.toString("base64") !== value) fail(context, "must be bounded canonical base64");
  return value;
}
function status(value, context) {
  const tag = value?.state;
  if (!["pending", "completed", "expired", "cancelled"].includes(tag)) fail(context, "has an unknown state");
  const fields = record(value, tag === "pending" ? ["state"] : ["state", "epoch"], context);
  return tag === "pending" ? { state: tag } : { state: tag, epoch: u64(fields.epoch, `${context}.epoch`) };
}

/** Bind canonical account identity to the existing SDK owner. */
export function createSorafsReplicationResponseNormalizer({ account }) {
  function identity(value, context) {
    const normalized = account(value, context);
    if (normalized !== value) fail(context, "must use the exact canonical identity");
    return normalized;
  }
  function attestation(value, context) {
    const fields = record(value, ["block_height", "block_hash_hex", "chain_id"], context);
    const height = u64(fields.block_height, `${context}.block_height`);
    const hash = fields.block_hash_hex === null ? null : hex(fields.block_hash_hex, `${context}.block_hash_hex`);
    if ((height === 0) !== (hash === null)) fail(context, "has inconsistent committed height and hash");
    return { block_height: height, block_hash_hex: hash, chain_id: text(fields.chain_id, `${context}.chain_id`) };
  }
  function assignment(value, context) {
    const fields = record(value, ["provider_id_hex", "slice_gib", "lane"], context);
    if (fields.lane !== null && (typeof fields.lane !== "string" || !/^[a-z0-9._-]{1,64}$/u.test(fields.lane))) fail(`${context}.lane`, "must use its canonical native lane spelling");
    return { provider_id_hex: hex(fields.provider_id_hex, `${context}.provider_id_hex`),
      slice_gib: positive(fields.slice_gib, `${context}.slice_gib`),
      lane: fields.lane === null ? null : text(fields.lane, `${context}.lane`) };
  }
  function order(value, context) {
    const fields = record(value, ["version", "order_id_hex", "manifest_cid_b64", "manifest_digest_hex", "chunking_profile", "target_replicas", "assignments", "issued_at", "deadline_at", "sla", "metadata"], context);
    if (fields.version !== 1) fail(`${context}.version`, "must equal 1");
    const cid = base64(fields.manifest_cid_b64, `${context}.manifest_cid_b64`, 36);
    const cidBytes = Buffer.from(cid, "base64");
    if (cidBytes.length !== 36 || ![1, 0x71, 0x1f, 32].every((byte, index) => cidBytes[index] === byte)
        || cidBytes.subarray(4).every(byte => byte === 0)) fail(context, "has an invalid native manifest CID");
    if (!SORAFS_REPLICATION_ORDER_CHUNKER_HANDLES_V1.includes(fields.chunking_profile)) fail(`${context}.chunking_profile`, "must use an exact canonical registered profile handle");
    const sla = record(fields.sla, ["ingest_deadline_secs", "min_availability_percent_milli", "min_por_success_percent_milli"], `${context}.sla`);
    const result = {
      version: 1, order_id_hex: hex(fields.order_id_hex, `${context}.order_id_hex`),
      manifest_cid_b64: cid, manifest_digest_hex: hex(fields.manifest_digest_hex, `${context}.manifest_digest_hex`),
      chunking_profile: text(fields.chunking_profile, `${context}.chunking_profile`),
      target_replicas: positive(fields.target_replicas, `${context}.target_replicas`, 0xffffn),
      assignments: array(fields.assignments, 1024, `${context}.assignments`, assignment),
      issued_at: u64(fields.issued_at, `${context}.issued_at`), deadline_at: u64(fields.deadline_at, `${context}.deadline_at`),
      sla: { ingest_deadline_secs: positive(sla.ingest_deadline_secs, `${context}.sla.ingest_deadline_secs`, 0xffff_ffffn),
        min_availability_percent_milli: positive(sla.min_availability_percent_milli, `${context}.sla.min_availability_percent_milli`, 100_000n),
        min_por_success_percent_milli: positive(sla.min_por_success_percent_milli, `${context}.sla.min_por_success_percent_milli`, 100_000n) },
      metadata: array(fields.metadata, 64, `${context}.metadata`, (entry, label) => {
        const item = record(entry, ["key", "value"], label);
        const key = text(item.key, `${label}.key`), value = text(item.value, `${label}.value`);
        if (!/^[a-z0-9_.-]{1,128}$/u.test(key) || value.length === 0 || /^\p{White_Space}|\p{White_Space}$/u.test(value)
            || Buffer.byteLength(value, "utf8") > 4096 || /[\u0000-\u001f\u007f-\u009f]/u.test(value)) fail(label, "must retain canonical bounded metadata strings");
        return { key, value };
      }),
    };
    if (result.assignments.length < result.target_replicas
        || BigInt(result.deadline_at) <= BigInt(result.issued_at)
        || BigInt(result.sla.ingest_deadline_secs) > BigInt(result.deadline_at) - BigInt(result.issued_at)) fail(context, "has inconsistent target or deadline fields");
    for (let i = 1; i < result.assignments.length; i++) {
      if (result.assignments[i - 1].provider_id_hex >= result.assignments[i].provider_id_hex) fail(context, "assignments must be strictly provider-ordered");
    }
    if (result.metadata.reduce((size, item) => size + Buffer.byteLength(item.key, "utf8") + Buffer.byteLength(item.value, "utf8"), 0) > 64 * 1024) fail(context, "metadata exceeds the native aggregate byte bound");
    if (new Set(result.metadata.map(item => item.key)).size !== result.metadata.length) fail(context, "metadata keys must be unique");
    return result;
  }
  function completion(value, context) {
    const fields = record(value, ["provider_hex", "completed_by", "completion_epoch", "assignment_revision", "completion_authority", "finalized_anchor"], context);
    const authority = record(fields.completion_authority, ["provider_owner", "signer_policy"], `${context}.completion_authority`);
    const policy = record(authority.signer_policy, ["policy_id_hex", "revision", "predecessor_digest_hex", "policy_digest_hex"], `${context}.completion_authority.signer_policy`);
    const anchor = record(fields.finalized_anchor, ["height", "block_hash_hex"], `${context}.finalized_anchor`);
    const revision = positive(policy.revision, `${context}.completion_authority.signer_policy.revision`);
    const predecessor = policy.predecessor_digest_hex === null ? null : hex(policy.predecessor_digest_hex, `${context}.completion_authority.signer_policy.predecessor_digest_hex`);
    if ((revision === 1) !== (predecessor === null)) fail(context, "has inconsistent signer-policy predecessor");
    const result = {
      provider_hex: hex(fields.provider_hex, `${context}.provider_hex`), completed_by: identity(fields.completed_by, `${context}.completed_by`),
      completion_epoch: u64(fields.completion_epoch, `${context}.completion_epoch`), assignment_revision: positive(fields.assignment_revision, `${context}.assignment_revision`),
      completion_authority: { provider_owner: identity(authority.provider_owner, `${context}.completion_authority.provider_owner`),
        signer_policy: { policy_id_hex: hex(policy.policy_id_hex, `${context}.completion_authority.signer_policy.policy_id_hex`), revision,
          predecessor_digest_hex: predecessor, policy_digest_hex: hex(policy.policy_digest_hex, `${context}.completion_authority.signer_policy.policy_digest_hex`) } },
      finalized_anchor: { height: positive(anchor.height, `${context}.finalized_anchor.height`), block_hash_hex: hex(anchor.block_hash_hex, `${context}.finalized_anchor.block_hash_hex`) },
    };
    if (result.completed_by !== result.completion_authority.provider_owner) fail(context, "must retain the provider owner's completion authority");
    return result;
  }
  function orderRecord(value, context) {
    const fields = record(value, ["order_id_hex", "manifest_digest_hex", "issued_by", "issued_epoch", "deadline_epoch", "status", "canonical_order_b64", "assignment_revision", "order", "provider_completions", "providers"], context);
    const result = {
      order_id_hex: hex(fields.order_id_hex, `${context}.order_id_hex`), manifest_digest_hex: hex(fields.manifest_digest_hex, `${context}.manifest_digest_hex`),
      issued_by: identity(fields.issued_by, `${context}.issued_by`), issued_epoch: u64(fields.issued_epoch, `${context}.issued_epoch`), deadline_epoch: u64(fields.deadline_epoch, `${context}.deadline_epoch`),
      status: status(fields.status, `${context}.status`), canonical_order_b64: base64(fields.canonical_order_b64, `${context}.canonical_order_b64`, 256 * 1024),
      assignment_revision: positive(fields.assignment_revision, `${context}.assignment_revision`), order: order(fields.order, `${context}.order`),
      provider_completions: array(fields.provider_completions, 1024, `${context}.provider_completions`, completion),
      providers: array(fields.providers, 1024, `${context}.providers`, hex),
    };
    if (result.order_id_hex !== result.order.order_id_hex || result.manifest_digest_hex !== result.order.manifest_digest_hex
        || BigInt(result.issued_epoch) !== BigInt(result.order.issued_at) || BigInt(result.deadline_epoch) !== BigInt(result.order.deadline_at)
        || result.providers.length !== result.order.assignments.length
        || result.providers.some((provider, index) => provider !== result.order.assignments[index].provider_id_hex)) fail(context, "disagrees with its order projection");
    const seen = new Set();
    let previous;
    for (const item of result.provider_completions) {
      const epoch = BigInt(item.completion_epoch);
      if (seen.has(item.provider_hex) || !result.providers.includes(item.provider_hex)
          || BigInt(item.assignment_revision) !== BigInt(result.assignment_revision)
          || epoch < BigInt(result.issued_epoch) || epoch > BigInt(result.deadline_epoch)
          || previous !== undefined && epoch < previous) fail(context, "contains inconsistent provider completions");
      seen.add(item.provider_hex); previous = epoch;
    }
    const count = result.provider_completions.length, target = result.order.target_replicas;
    let valid = count <= target;
    switch (result.status.state) {
      case "pending": valid &&= count < target; break;
      case "completed": valid &&= count === target && previous === BigInt(result.status.epoch); break;
      case "cancelled": valid &&= count < target && BigInt(result.status.epoch) >= BigInt(result.issued_epoch) && BigInt(result.status.epoch) <= BigInt(result.deadline_epoch); break;
      case "expired": valid &&= count < target && BigInt(result.status.epoch) > BigInt(result.deadline_epoch); break;
    }
    if (!valid) fail(context, "has inconsistent completion lifecycle");
    return result;
  }
  function normalize(value, request = {}, context = "sorafs replication list response") {
    const fields = record(value, ["attestation", "total_count", "returned_count", "offset", "limit", "replication_orders"], context);
    const result = { attestation: attestation(fields.attestation, `${context}.attestation`),
      total_count: u64(fields.total_count, `${context}.total_count`), returned_count: u64(fields.returned_count, `${context}.returned_count`),
      offset: u64(fields.offset, `${context}.offset`, 0xffff_ffffn), limit: positive(fields.limit, `${context}.limit`, 500n),
      replication_orders: array(fields.replication_orders, 500, `${context}.replication_orders`, orderRecord) };
    if (result.returned_count !== result.replication_orders.length || result.returned_count > result.limit
        || BigInt(result.offset) + BigInt(result.returned_count) > BigInt(result.total_count)) fail(context, "has inconsistent pagination");
    const available = BigInt(result.total_count) - BigInt(result.offset);
    const expectedReturned = available < BigInt(result.limit) ? Number(available) : result.limit;
    if (result.returned_count !== expectedReturned) fail(context, "does not contain the complete native page");
    const expectedOffset = BigInt(request.offset ?? 0) < BigInt(result.total_count) ? request.offset ?? 0 : result.total_count;
    if (BigInt(result.offset) !== BigInt(expectedOffset) || result.limit !== (request.limit ?? 50)) fail(context, "does not match the requested pagination");
    let previous;
    for (const item of result.replication_orders) {
      if (previous !== undefined && previous >= item.order_id_hex) fail(context, "orders must be strictly identifier-ordered");
      previous = item.order_id_hex;
      // Core accepts completion anchors only from the committed prefix. The
      // page attests this same world generation; older anchors remain valid.
      for (const completion of item.provider_completions) {
        const height = BigInt(completion.finalized_anchor.height);
        const tip = BigInt(result.attestation.block_height);
        if (height > tip || height === tip
            && completion.finalized_anchor.block_hash_hex !== result.attestation.block_hash_hex) fail(context, "completion anchor is inconsistent with the attested committed prefix");
      }
      if (request.status !== undefined && item.status.state !== request.status
          || request.manifest_digest !== undefined && item.manifest_digest_hex !== request.manifest_digest) fail(context, "violates the requested filter");
    }
    return result;
  }
  return Object.freeze({ normalize, orderRecord });
}
