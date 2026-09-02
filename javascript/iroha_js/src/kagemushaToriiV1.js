// SPDX-License-Identifier: Apache-2.0

const JSON_MEDIA_TYPE = /^[ \t]*application\/json(?:[ \t]*;.*)?$/iu;

/** Validate the universally compiled Kagemusha V1 readiness projection. */
export function normalizeKagemushaReadinessV1(value) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError("Kagemusha V1 readiness response must be an object");
  }
  const fields = ["kagemusha_handoff_capability", "wire_version", "device_lifecycle_version", "ready"];
  if (Object.keys(value).length !== fields.length || fields.some((field) => !Object.hasOwn(value, field))) {
    throw new TypeError("Kagemusha V1 readiness response contains missing or unknown fields");
  }
  if (value.kagemusha_handoff_capability !== "kagemusha_handoff_v1" || value.wire_version !== 1 || value.device_lifecycle_version !== 1 || typeof value.ready !== "boolean") {
    throw new TypeError("Kagemusha V1 readiness response is incompatible");
  }
  return Object.freeze({
    kagemusha_handoff_capability: "kagemusha_handoff_v1",
    wire_version: 1,
    device_lifecycle_version: 1,
    ready: value.ready,
  });
}

const OPERATION_KINDS = new Set(["top_up", "redemption"]);
const OPERATION_STATES = new Set(["pending", "applied", "rejected"]);
const REJECTION_CODES = new Set([
  "invalid_request",
  "unauthorized",
  "insufficient_online_balance",
  "invalid_proof",
  "hardware_policy_rejected",
  "identity_conflict",
  "reserve_underflow",
  "arithmetic_overflow",
  "internal_failure",
]);
const UNVERIFIED_STATUS = new WeakMap();

/** Return the sole lowercase path/header spelling of a nonzero operation identifier. */
export function kagemushaOperationIdHexV1(value) {
  if (typeof value === "string") {
    if (!/^[0-9a-f]{64}$/u.test(value) || /^0{64}$/u.test(value)) throw new TypeError("Kagemusha V1 operation ID must be 64 lowercase hexadecimal characters");
    return value;
  }
  if (!ArrayBuffer.isView(value) && !(value instanceof ArrayBuffer)) throw new TypeError("Kagemusha V1 operation ID must be binary data or lowercase hexadecimal");
  const bytes = value instanceof ArrayBuffer
    ? new Uint8Array(value)
    : new Uint8Array(value.buffer, value.byteOffset, value.byteLength);
  if (bytes.length !== 32 || bytes.every((byte) => byte === 0)) throw new TypeError("Kagemusha V1 operation ID must be one nonzero 32-byte value");
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

/**
 * Structurally validated operation status whose applied result remains inaccessible until an
 * independently pinned finality verifier authenticates it.
 */
export class UnverifiedKagemushaOperationStatusV1 {
  constructor(operationId, kind, state, rejection, source) {
    UNVERIFIED_STATUS.set(this, {
      operationId: Uint8Array.from(operationId),
      kind,
      state,
      rejection,
      source: cloneJson(source),
    });
    Object.freeze(this);
  }

  get operationId() { return Uint8Array.from(UNVERIFIED_STATUS.get(this).operationId); }
  get kind() { return UNVERIFIED_STATUS.get(this).kind; }
  get state() { return UNVERIFIED_STATUS.get(this).state; }
  get rejection() {
    const value = UNVERIFIED_STATUS.get(this).rejection;
    return value === null ? null : Object.freeze({ code: value.code, detailDigest: Uint8Array.from(value.detailDigest) });
  }

  /** Release a terminal result only through the caller's independently pinned verifier. */
  async verifyAgainst(trustAnchor, verifier) {
    if (trustAnchor === null || trustAnchor === undefined) {
      throw new TypeError("Kagemusha V1 finality trust anchor is required");
    }
    if (typeof verifier !== "function") {
      throw new TypeError("Kagemusha V1 finality verifier must be a function");
    }
    return verifier(cloneJson(UNVERIFIED_STATUS.get(this).source), trustAnchor);
  }
}

/** Strictly decode the closed outer operation-status envelope without trusting its result. */
export function normalizeUnverifiedKagemushaOperationStatusV1(value) {
  exactObject(value, ["version", "operation_id", "kind", "state", "result", "rejection"], "Kagemusha V1 operation status");
  if (value.version !== 1) throw new TypeError("Kagemusha V1 operation status version must be 1");
  const operationId = fixedBytes(value.operation_id, "Kagemusha V1 operation ID");
  const kind = taggedUnit(value.kind, "kind", OPERATION_KINDS, "Kagemusha V1 operation kind");
  const state = taggedUnit(value.state, "state", OPERATION_STATES, "Kagemusha V1 operation state");
  let rejection = null;
  if (state === "pending") {
    if (value.result !== null || value.rejection !== null) throw new TypeError("pending Kagemusha V1 status must not contain a result or rejection");
  } else if (state === "applied") {
    if (value.result === null || value.rejection !== null || typeof value.result !== "object" || Array.isArray(value.result)) {
      throw new TypeError("applied Kagemusha V1 status has an invalid terminal envelope");
    }
  } else {
    if (value.result !== null) throw new TypeError("rejected Kagemusha V1 status must not contain a result");
    exactObject(value.rejection, ["code", "detail_digest"], "Kagemusha V1 rejection");
    const code = taggedUnit(value.rejection.code, "code", REJECTION_CODES, "Kagemusha V1 rejection code");
    const detailDigest = fixedBytes(value.rejection.detail_digest, "Kagemusha V1 rejection detail digest");
    rejection = Object.freeze({ code, detailDigest });
  }
  return new UnverifiedKagemushaOperationStatusV1(operationId, kind, state, rejection, value);
}

/** Require the JSON response family used by the Kagemusha V1 control plane. */
export function requireKagemushaJsonContentTypeV1(value, context) {
  if (typeof value !== "string" || !JSON_MEDIA_TYPE.test(value)) {
    throw new TypeError(`${context} must use application/json`);
  }
}

function exactObject(value, fields, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) throw new TypeError(`${context} must be an object`);
  const actual = Object.keys(value);
  const expected = new Set(fields);
  if (actual.length !== fields.length || actual.some((field) => !expected.has(field))) {
    throw new TypeError(`${context} contains missing or unknown fields`);
  }
}

function fixedBytes(value, context) {
  if (!Array.isArray(value) || value.length !== 32 || value.some((byte) => !Number.isInteger(byte) || byte < 0 || byte > 255) || value.every((byte) => byte === 0)) {
    throw new TypeError(`${context} must be one nonzero 32-byte array`);
  }
  return Uint8Array.from(value);
}

function taggedUnit(value, tag, allowed, context) {
  exactObject(value, [tag, "value"], context);
  if (typeof value[tag] !== "string" || !allowed.has(value[tag]) || value.value !== null) {
    throw new TypeError(`${context} is invalid`);
  }
  return value[tag];
}

function cloneJson(value) {
  if (Array.isArray(value)) return value.map(cloneJson);
  if (value !== null && typeof value === "object") {
    return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, cloneJson(item)]));
  }
  return value;
}
