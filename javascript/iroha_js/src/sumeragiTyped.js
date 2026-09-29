import { Buffer } from "buffer";
import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";
import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import { getCurveEntryByPublicKeyMulticodec } from "./curveRegistry.js";
import { AccountAddress } from "./address.js";
import { SUMERAGI_STATUS_TYPED_JSON_MAX_BYTES } from "./sumeragiTypedLimits.js";

export { SUMERAGI_STATUS_TYPED_JSON_MAX_BYTES };
const FIELDS = "protocol_version config_fingerprint beacon_horizon instance height view stage leader proxy_tail high_qc_view level start_level t_retx_ms committed_height applied_height awaiting signer unanchored abstaining halted footprint".split(" ");
const FOOTPRINT = "votes timeouts blocks exec_entries wants pending_apply sync_entries sync_bytes peers recent_headers configs cert_cache evidence_keys probe".split(" ");
const HORIZON = "epoch_length_blocks next_required_pulse_height active_session_id session_covers_next_pulse local_provider_ready".split(" ");
const HEIGHT_REASONS = new Set(["safety_violation", "apply_diverged", "publication_recovery_required"]);
const UNIT_REASONS = new Set(["safety_record_corrupt", "safety_record_inconsistent", "driver_anomaly"]);

function record(value, fields, label) {
  if (value === null || typeof value !== "object" || Array.isArray(value) ||
      Object.keys(value).length !== fields.length || fields.some((key) => !Object.hasOwn(value, key))) {
    throw new TypeError(`${label} requires exactly its native fields`);
  }
  return value;
}
function uint(value, bits = 64) {
  if ((typeof value !== "number" || !Number.isSafeInteger(value) || Object.is(value, -0)) && typeof value !== "bigint") {
    throw new TypeError("native status requires exact unsigned integers");
  }
  const integer = BigInt(value);
  if (integer < 0n || integer >= (1n << BigInt(bits))) throw new RangeError("native status integer exceeds its bound");
  return bits < 64 ? Number(integer) : value;
}
function optionalUint(value) { return value === null ? null : uint(value); }
function bool(value) {
  if (typeof value !== "boolean") throw new TypeError("native status requires booleans");
  return value;
}
function hash(value) {
  if (typeof value !== "string" || !/^hash:[0-9A-F]{64}#[0-9A-F]{4}$/.test(value)) throw new TypeError("native fingerprint must be a canonical hash");
  const body = value.slice(5, 69);
  if ((Number.parseInt(body.slice(-2), 16) & 1) !== 1 || computeHashLiteralCrc("hash", body) !== value.slice(70)) {
    throw new TypeError("native fingerprint marker or checksum is invalid");
  }
  return value;
}
function publicKey(value) {
  if (value === null) return null;
  if (typeof value !== "string" || value.length % 2 || !/^[0-9a-fA-F]+$/.test(value)) throw new TypeError("native status requires canonical public keys");
  const bytes = Buffer.from(value, "hex");
  let cursor = 0;
  function varint() {
    let result = 0n;
    for (let used = 0; used < 10 && cursor < bytes.length; used += 1) {
      const byte = bytes[cursor++];
      if (used === 9 && (byte & 0xfe) !== 0) throw new TypeError("public-key varint overflows");
      result |= BigInt(byte & 0x7f) << BigInt(used * 7);
      if ((byte & 0x80) === 0) {
        if ((used !== 0 && byte === 0) || result > BigInt(Number.MAX_SAFE_INTEGER)) throw new TypeError("public-key varint is not canonical");
        return Number(result);
      }
    }
    throw new TypeError("public-key varint is truncated");
  }
  const code = varint();
  const length = varint();
  const key = bytes.subarray(cursor);
  const curve = getCurveEntryByPublicKeyMulticodec(code);
  if (!curve || length !== key.length || value !== bytes.subarray(0, cursor).toString("hex") + key.toString("hex").toUpperCase()) throw new TypeError("public key has wrong algorithm, length or canonical spelling");
  // Reuse the SDK's actual controller/public-key admission boundary.
  AccountAddress.fromAccount({ publicKey: key, algorithm: curve.algorithm });
  return value;
}
function horizon(value) {
  if (value === null) return null;
  const r = record(value, HORIZON, "native beacon horizon");
  const session = r.active_session_id;
  if (session !== null && (typeof session !== "string" || !/^[0-9A-F]{64}$/.test(session))) throw new TypeError("native beacon session must be uppercase 32-byte hex");
  const next = optionalUint(r.next_required_pulse_height);
  const covers = bool(r.session_covers_next_pulse);
  const ready = bool(r.local_provider_ready);
  if ((covers && (session === null || next === null)) || (ready && session === null)) throw new TypeError("native beacon readiness requires its exact session and demand");
  return Object.freeze({ epoch_length_blocks: uint(r.epoch_length_blocks), next_required_pulse_height: next, active_session_id: session, session_covers_next_pulse: covers, local_provider_ready: ready });
}
function halt(value) {
  if (value === null) return null;
  const r = record(value, ["reason", "details"], "native halt reason");
  if (HEIGHT_REASONS.has(r.reason)) return Object.freeze({ reason: r.reason, details: uint(r.details) });
  if (UNIT_REASONS.has(r.reason) && r.details === null) return Object.freeze({ reason: r.reason, details: null });
  throw new TypeError("invalid native halt reason or details");
}
/** Validate an immutable protocol-8 observation, never a finality proof. */
export function parseSumeragiStatusPayload(payload) {
  const r = record(payload, FIELDS, "native status");
  if (uint(r.protocol_version, 16) !== 8 || uint(r.stage, 16) > 2) throw new TypeError("unsupported native status protocol or stage");
  if (typeof r.instance !== "string" || !/^[0-9a-f]{64}$/.test(r.instance)) throw new TypeError("native instance must be lowercase 32-byte hex");
  const f = record(r.footprint, FOOTPRINT, "native footprint");
  return Object.freeze({
    protocol_version: 8, config_fingerprint: hash(r.config_fingerprint), beacon_horizon: horizon(r.beacon_horizon),
    instance: r.instance, height: uint(r.height), view: uint(r.view), stage: Number(r.stage),
    leader: publicKey(r.leader), proxy_tail: publicKey(r.proxy_tail), high_qc_view: optionalUint(r.high_qc_view),
    level: uint(r.level, 32), start_level: uint(r.start_level, 32), t_retx_ms: uint(r.t_retx_ms),
    committed_height: uint(r.committed_height), applied_height: uint(r.applied_height), awaiting: bool(r.awaiting),
    signer: publicKey(r.signer), unanchored: bool(r.unanchored), abstaining: bool(r.abstaining), halted: halt(r.halted),
    footprint: Object.freeze(Object.fromEntries(FOOTPRINT.map((field) => [field, uint(f[field])]))),
  });
}
/** Bounded strict JSON preserves all unsigned bits and rejects duplicate/numeric-token drift. */
export function parseSumeragiStatusJson(text, context = "native status") {
  if (typeof text !== "string" || !text.length || Buffer.byteLength(text, "utf8") > SUMERAGI_STATUS_TYPED_JSON_MAX_BYTES) throw new TypeError(`${context} is empty or exceeds its byte bound`);
  return parseSumeragiStatusPayload(parseStrictLosslessIntegerJson(text, context));
}
