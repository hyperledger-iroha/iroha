import { Buffer } from "buffer";

import { computeHashLiteralCrc } from "./hashLiteralCrc.js";
import {
  createNativeRuntime,
  resolveNativeRuntimeBinding,
} from "./nativeRuntime.js";
import {
  parseStrictLosslessIntegerJson,
} from "./strictLosslessJson.js";

const JSON_MEDIA_TYPE = "application/json";
const RESPONSE_SMALL_MAX_BYTES = 1024 * 1024;
const RESPONSE_PUBLIC_BUNDLE_MAX_BYTES = 8 * 1024 * 1024;
const RESPONSE_RESTRICTED_MAX_BYTES = 32 * 1024 * 1024;
const U64_MAX = 0xffffffffffffffffn;

/** Authentication classes for atomic-private-settlement V1 routes. */
export const AtomicPrivateSettlementAuthV1 = Object.freeze({
  SPONSOR: "SPONSOR",
  ROLE_IDENTITY: "ROLE_IDENTITY",
  PUBLIC: "PUBLIC",
});

function operation(path, auth, topLevelFields, maximumRequestBytes) {
  return Object.freeze({
    path,
    auth,
    topLevelFields: Object.freeze([...topLevelFields]),
    maximumRequestBytes,
  });
}

/** Closed mutation-route catalog consumed by native-prepared requests. */
export const AtomicPrivateSettlementOperationV1 = Object.freeze({
  AVAILABILITY_SHARE: operation(
    "/v1/nexus/private-settlements/legs/availability-shares",
    AtomicPrivateSettlementAuthV1.SPONSOR,
    ["material"],
    32 * 1024 * 1024,
  ),
  PREPARE_VOTE: operation(
    "/v1/nexus/private-settlements/phases/prepare-votes",
    AtomicPrivateSettlementAuthV1.SPONSOR,
    ["manifest", "payload_digest"],
    8 * 1024 * 1024,
  ),
  COMMIT_VOTE: operation(
    "/v1/nexus/private-settlements/phases/commit-votes",
    AtomicPrivateSettlementAuthV1.SPONSOR,
    ["payload_digest", "barrier"],
    8 * 1024 * 1024,
  ),
  PHASE_CERTIFICATE: operation(
    "/v1/nexus/private-settlements/phases/certificates",
    AtomicPrivateSettlementAuthV1.SPONSOR,
    ["manifest", "payload_digest", "certificate"],
    8 * 1024 * 1024,
  ),
  LEG_UPLOAD: operation(
    "/v1/nexus/private-settlements/legs",
    AtomicPrivateSettlementAuthV1.SPONSOR,
    ["manifest", "audit_policy", "committee_authority", "payload"],
    32 * 1024 * 1024,
  ),
  AUDIT_APPROVAL: operation(
    "/v1/nexus/private-settlements/legs/{payload_digest}/audit-approvals",
    AtomicPrivateSettlementAuthV1.ROLE_IDENTITY,
    ["approval"],
    2 * 1024 * 1024,
  ),
  BUNDLE_SUBMIT: operation(
    "/v1/nexus/private-settlements/bundles",
    AtomicPrivateSettlementAuthV1.SPONSOR,
    ["transaction"],
    8 * 1024 * 1024,
  ),
});

const OPERATIONS = new Set(Object.values(AtomicPrivateSettlementOperationV1));

function sourceBytes(value, context) {
  if (typeof value === "string") {
    return new TextEncoder().encode(value);
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value.slice(0));
  }
  if (ArrayBuffer.isView(value)) {
    return new Uint8Array(value.buffer, value.byteOffset, value.byteLength).slice();
  }
  throw new TypeError(`${context} must be UTF-8 text or bytes`);
}

function strictObject(bytes, context) {
  let text;
  try {
    text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch (error) {
    throw new TypeError(`${context} must be exact UTF-8`, { cause: error });
  }
  const value = parseStrictLosslessIntegerJson(text, context);
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be one strict JSON object`);
  }
  return value;
}

function exactFields(value, expected, context) {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (
    actual.length !== wanted.length
    || actual.some((field, index) => field !== wanted[index])
  ) {
    throw new TypeError(`${context} has unexpected public fields`);
  }
}

/** Exact 32-byte marked Iroha hash used in settlement paths and responses. */
export class AtomicPrivateSettlementIdentifierV1 {
  #pathComponent;
  #jsonLiteral;

  constructor(value) {
    if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
      throw new TypeError("settlement identifier must be exact non-empty text");
    }
    let body;
    let literal;
    if (/^[0-9A-Fa-f]{64}$/u.test(value)) {
      body = value.toUpperCase();
      if ((Number.parseInt(body.slice(-2), 16) & 1) === 0) {
        throw new TypeError("settlement identifier must set the Iroha hash marker bit");
      }
      literal = `hash:${body}#${computeHashLiteralCrc("hash", body)}`;
    } else {
      const match = /^hash:([0-9A-F]{64})#([0-9A-F]{4})$/u.exec(value);
      if (match === null) {
        throw new TypeError(
          "settlement identifier must be raw hex or a canonical Norito hash literal",
        );
      }
      [, body] = match;
      if (
        match[2] !== computeHashLiteralCrc("hash", body)
        || (Number.parseInt(body.slice(-2), 16) & 1) === 0
      ) {
        throw new TypeError("settlement identifier checksum or marker is invalid");
      }
      literal = value;
    }
    this.#pathComponent = body.toLowerCase();
    this.#jsonLiteral = literal;
    Object.freeze(this);
  }

  get pathComponent() {
    return this.#pathComponent;
  }

  get jsonLiteral() {
    return this.#jsonLiteral;
  }

  get bytes() {
    return Uint8Array.from(Buffer.from(this.#pathComponent, "hex"));
  }

  toString() {
    return this.#pathComponent;
  }
}

/** Bounded, operation-tagged request produced by the native coordinator. */
export class AtomicPrivateSettlementPreparedRequestV1 {
  #operation;
  #body;
  #closed = false;

  constructor(operationValue, nativePreparedJson) {
    if (!OPERATIONS.has(operationValue)) {
      throw new TypeError("settlement operation is not a V1 catalog member");
    }
    const bytes = sourceBytes(nativePreparedJson, "prepared settlement request");
    if (bytes.byteLength === 0 || bytes.byteLength > operationValue.maximumRequestBytes) {
      throw new RangeError("prepared settlement request is empty or exceeds its route bound");
    }
    const parsed = strictObject(bytes, "prepared settlement request");
    exactFields(parsed, operationValue.topLevelFields, "prepared settlement request");
    this.#operation = operationValue;
    this.#body = bytes;
  }

  get operation() {
    return this.#operation;
  }

  bytes() {
    if (this.#closed) throw new TypeError("prepared settlement request is closed");
    return this.#body.slice();
  }

  close() {
    if (!this.#closed) {
      this.#body.fill(0);
      this.#closed = true;
    }
  }

  toString() {
    const name = Object.entries(AtomicPrivateSettlementOperationV1)
      .find(([, candidate]) => candidate === this.#operation)?.[0] ?? "UNKNOWN";
    return `AtomicPrivateSettlementPreparedRequestV1(operation=${name}, body=[REDACTED])`;
  }
}

/** Opaque bounded response suitable for the native wallet/auditor decoder. */
export class AtomicPrivateSettlementJsonResponseV1 {
  #route;
  #body;
  #closed = false;

  constructor(route, body) {
    this.#route = route;
    this.#body = body.slice();
  }

  get route() {
    return this.#route;
  }

  bytes() {
    if (this.#closed) throw new TypeError("settlement response is closed");
    return this.#body.slice();
  }

  close() {
    if (!this.#closed) {
      this.#body.fill(0);
      this.#closed = true;
    }
  }

  toString() {
    return `AtomicPrivateSettlementJsonResponseV1(route=${this.#route}, body=[REDACTED])`;
  }
}

/** Redacted settlement transport or response-validation failure. */
export class AtomicPrivateSettlementToriiErrorV1 extends Error {
  constructor(message, options) {
    super(message, options);
    this.name = "AtomicPrivateSettlementToriiErrorV1";
  }
}

const RESPONSE_FIELDS = Object.freeze([
  ["/availability-shares", ["bundle_id", "payload_digest", "leg_ordinal", "disposition", "share"]],
  ["/prepare-votes", ["bundle_id", "payload_digest", "leg_ordinal", "vote"]],
  ["/commit-votes", ["bundle_id", "payload_digest", "leg_ordinal", "vote"]],
  [
    "/phase-certificates",
    [
      "bundle_id",
      "payload_digest",
      "leg_ordinal",
      "lifecycle",
      "prepare_certificate",
      "commit_certificate",
    ],
  ],
  ["/certificates", ["bundle_id", "payload_digest", "leg_ordinal", "phase", "lifecycle"]],
  [
    "/audit-approvals",
    [
      "authoritative_height",
      "bundle_id",
      "payload_digest",
      "leg_ordinal",
      "committee_authority",
      "collected",
      "required",
      "newly_recorded",
      "lifecycle",
      "responder_attestation",
    ],
  ],
  [
    "/status",
    [
      "bundle_id",
      "payload_digest",
      "leg_ordinal",
      "route",
      "stored_at_height",
      "lifecycle_height",
      "expiry_height",
      "lifecycle",
    ],
  ],
  [
    "/committee-proof",
    [
      "manifest",
      "audit_policy",
      "committee_authority",
      "statement",
      "proof",
      "delta",
      "audit_approvals",
      "audit_capsule_digest",
      "availability",
      "lifecycle",
    ],
  ],
  [
    "/audit-capsule",
    [
      "authoritative_height",
      "manifest",
      "audit_policy",
      "committee_authority",
      "statement",
      "delta",
      "audit_capsule",
      "availability",
      "lifecycle",
      "responder_attestation",
    ],
  ],
  ["/receipt", ["status", "value"]],
]);

function responseFields(route) {
  for (const [suffix, fields] of RESPONSE_FIELDS) {
    if (route.endsWith(suffix)) return fields;
  }
  if (route.endsWith("/legs")) {
    return ["bundle_id", "payload_digest", "leg_ordinal", "disposition", "lifecycle"];
  }
  if (route.endsWith("/bundles")) {
    return ["bundle_id", "accepted_at_height", "carrier_id"];
  }
  if (route.includes("/bundles/")) {
    return ["manifest", "lifecycle", "finalized_height"];
  }
  throw new TypeError("unknown atomic private settlement route");
}

function validateBundleAdmission(value) {
  for (const field of ["bundle_id", "carrier_id"]) {
    if (typeof value[field] !== "string") {
      throw new TypeError(`settlement bundle admission ${field} must be a hash literal`);
    }
    const parsed = new AtomicPrivateSettlementIdentifierV1(value[field]);
    if (parsed.jsonLiteral !== value[field]) {
      throw new TypeError(`settlement bundle admission ${field} must be canonical`);
    }
  }
  const height = value.accepted_at_height;
  const validNumber = typeof height === "number" && Number.isSafeInteger(height) && height >= 0;
  const validBigInt = typeof height === "bigint" && height >= 0n && height <= 0xffffffffffffffffn;
  if (!validNumber && !validBigInt) {
    throw new TypeError("settlement bundle admission accepted_at_height must be a u64");
  }
}

function canonicalAttestationHash(value, context) {
  if (typeof value !== "string") {
    throw new TypeError(`${context} must be a canonical hash literal`);
  }
  const parsed = new AtomicPrivateSettlementIdentifierV1(value);
  if (parsed.jsonLiteral !== value) {
    throw new TypeError(`${context} must be a canonical hash literal`);
  }
  return parsed;
}

function isU8(value, nonzero = false) {
  return typeof value === "number"
    && Number.isInteger(value)
    && value >= (nonzero ? 1 : 0)
    && value <= 0xff;
}

function isLegOrdinal(value) {
  return isU8(value) && value < 0xff;
}

function isU64(value, nonzero = false) {
  const minimum = nonzero ? 1n : 0n;
  return (typeof value === "number"
      && Number.isSafeInteger(value)
      && value >= Number(minimum))
    || (typeof value === "bigint" && value >= minimum && value <= U64_MAX);
}

function u64BigInt(value) {
  return typeof value === "bigint" ? value : BigInt(value);
}

function validateBlsNormalSignatureLiteral(value, context) {
  // TODO: Verify the responder PoP and signature once the JavaScript SDK
  // exports the typed Norito attestation preimages and BLS-Normal PoP boundary.
  if (typeof value !== "string" || !/^[A-Za-z0-9+/]{128}$/u.test(value)) {
    throw new TypeError(`${context} must be exact standard base64 for 96 bytes`);
  }
  const decoded = Buffer.from(value, "base64");
  if (decoded.byteLength !== 96 || decoded.toString("base64") !== value) {
    throw new TypeError(`${context} must be exact standard base64 for 96 bytes`);
  }
}

function auditApprovalRequestContext(request) {
  const bytes = request.bytes();
  try {
    const parsed = strictObject(bytes, "prepared settlement audit approval");
    const approval = parsed.approval;
    if (approval === null || typeof approval !== "object" || Array.isArray(approval)) {
      throw new TypeError("prepared settlement audit approval is invalid");
    }
    exactFields(
      approval,
      ["body", "signature"],
      "prepared settlement audit approval",
    );
    if (approval.signature === null || approval.signature === undefined) {
      throw new TypeError("prepared settlement audit approval signature is missing");
    }
    const body = approval.body;
    if (body === null || typeof body !== "object" || Array.isArray(body)) {
      throw new TypeError("prepared settlement audit approval body is invalid");
    }
    exactFields(
      body,
      [
        "version",
        "network_id",
        "bundle_id",
        "leg_ordinal",
        "dataspace_id",
        "auditor_id",
        "audit_policy_digest",
        "audit_key_epoch",
        "proof_digest",
        "capsule_digest",
        "delta_digest",
        "old_root",
        "new_root",
        "expiry_height",
      ],
      "prepared settlement audit approval body",
    );
    const networkId = canonicalAttestationHash(
      body.network_id,
      "prepared settlement approval network_id",
    );
    const bundleId = canonicalAttestationHash(
      body.bundle_id,
      "prepared settlement approval bundle_id",
    );
    for (const field of [
      "audit_policy_digest",
      "proof_digest",
      "capsule_digest",
      "delta_digest",
    ]) {
      canonicalAttestationHash(
        body[field],
        `prepared settlement approval ${field}`,
      );
    }
    if (body.version !== 1) {
      throw new TypeError("prepared settlement approval version must be one");
    }
    if (!isLegOrdinal(body.leg_ordinal)) {
      throw new TypeError("prepared settlement approval leg_ordinal must be in 0..=254");
    }
    if (!isU64(body.dataspace_id)) {
      throw new TypeError("prepared settlement approval dataspace_id must be a u64");
    }
    if (!isU64(body.expiry_height, true)) {
      throw new TypeError("prepared settlement approval expiry_height must be a nonzero u64");
    }
    return Object.freeze({
      networkId,
      bundleId,
      legOrdinal: body.leg_ordinal,
      dataspaceId: body.dataspace_id,
      expiryHeight: body.expiry_height,
    });
  } finally {
    bytes.fill(0);
  }
}

function validateAuditorCapsuleAttestation(value, identity) {
  const attestation = value.responder_attestation;
  if (attestation === null || typeof attestation !== "object" || Array.isArray(attestation)) {
    throw new TypeError("settlement auditor capsule responder attestation must be an object");
  }
  exactFields(attestation, ["body", "signature"], "settlement auditor capsule attestation");
  const body = attestation.body;
  if (body === null || typeof body !== "object" || Array.isArray(body)) {
    throw new TypeError("settlement auditor capsule attestation body must be an object");
  }
  exactFields(
    body,
    [
      "version",
      "network_id",
      "payload_digest",
      "view_digest",
      "authority_digest",
      "lifecycle_code",
      "authoritative_height",
      "responder",
    ],
    "settlement auditor capsule attestation body",
  );
  const lifecycleCodes = new Map([
    ["collecting", 0],
    ["audited", 1],
    ["prepared", 2],
    ["commit_certified", 3],
    ["finalized", 4],
    ["aborted", 5],
    ["expired", 6],
  ]);
  const expectedCode = lifecycleCodes.get(value.lifecycle?.status);
  if (
    !isU8(body.version, true)
    || body.version !== 1
    || !isU64(body.authoritative_height, true)
    || body.authoritative_height !== value.authoritative_height
    || !isU8(body.lifecycle_code)
    || body.lifecycle_code !== expectedCode
    || typeof body.responder !== "string"
    || body.responder.length === 0
    || body.responder.trim() !== body.responder
  ) {
    throw new TypeError("settlement auditor capsule responder attestation is invalid");
  }
  for (const field of ["network_id", "payload_digest", "view_digest", "authority_digest"]) {
    canonicalAttestationHash(body[field], "settlement auditor capsule attestation digest");
  }
  if (
    body.network_id !== identity.network.jsonLiteral
    || body.payload_digest !== identity.value.jsonLiteral
  ) {
    throw new TypeError("settlement auditor capsule attestation binding is invalid");
  }
  if (value.manifest === null || typeof value.manifest !== "object" || Array.isArray(value.manifest)) {
    throw new TypeError("settlement auditor capsule manifest is invalid");
  }
  if (
    Object.prototype.hasOwnProperty.call(value.manifest, "network_id")
    && canonicalAttestationHash(
      value.manifest.network_id,
      "settlement auditor capsule manifest network_id",
    ).jsonLiteral !== identity.network.jsonLiteral
  ) {
    throw new TypeError("settlement auditor capsule network binding is invalid");
  }
  validateBlsNormalSignatureLiteral(
    attestation.signature,
    "settlement auditor capsule responder signature",
  );
}

function validateAuditApprovalAcknowledgementAttestation(value, identity) {
  const attestation = value.responder_attestation;
  if (attestation === null || typeof attestation !== "object" || Array.isArray(attestation)) {
    throw new TypeError("settlement approval acknowledgement responder attestation must be an object");
  }
  exactFields(attestation, ["body", "signature"], "settlement approval acknowledgement attestation");
  const body = attestation.body;
  if (body === null || typeof body !== "object" || Array.isArray(body)) {
    throw new TypeError("settlement approval acknowledgement attestation body must be an object");
  }
  exactFields(
    body,
    [
      "version",
      "network_id",
      "payload_digest",
      "approval_digest",
      "acknowledgement_digest",
      "authority_digest",
      "lifecycle_code",
      "authoritative_height",
      "responder",
    ],
    "settlement approval acknowledgement attestation body",
  );
  const expectedCode = new Map([["collecting", 0], ["audited", 1]])
    .get(value.lifecycle?.status);
  const height = value.authoritative_height;
  const validHeight = isU64(height, true);
  const collected = value.collected;
  const required = value.required;
  if (
    !validHeight
    || u64BigInt(height) > u64BigInt(identity.approvalContext.expiryHeight)
    || !isU8(body.version, true)
    || body.version !== 1
    || !isU64(body.authoritative_height, true)
    || body.authoritative_height !== height
    || body.payload_digest !== value.payload_digest
    || !isU8(body.lifecycle_code)
    || body.lifecycle_code !== expectedCode
    || typeof body.responder !== "string"
    || body.responder.length === 0
    || body.responder.trim() !== body.responder
    || !isLegOrdinal(value.leg_ordinal)
    || !isU8(collected, true)
    || !isU8(required, true)
    || collected > required
    || typeof value.newly_recorded !== "boolean"
    || value.lifecycle?.status !== (collected < required ? "collecting" : "audited")
    || value.leg_ordinal !== identity.approvalContext.legOrdinal
  ) {
    throw new TypeError("settlement approval acknowledgement responder attestation is invalid");
  }
  for (const field of [
    "network_id",
    "payload_digest",
    "approval_digest",
    "acknowledgement_digest",
    "authority_digest",
  ]) {
    canonicalAttestationHash(
      body[field],
      "settlement approval acknowledgement attestation digest",
    );
  }
  const payloadDigest = canonicalAttestationHash(
    value.payload_digest,
    "settlement approval acknowledgement payload_digest",
  );
  const bundleId = canonicalAttestationHash(
    value.bundle_id,
    "settlement approval acknowledgement bundle_id",
  );
  if (
    body.network_id !== identity.network.jsonLiteral
    || body.network_id !== identity.approvalContext.networkId.jsonLiteral
    || payloadDigest.jsonLiteral !== identity.value.jsonLiteral
    || body.payload_digest !== identity.value.jsonLiteral
    || bundleId.jsonLiteral !== identity.approvalContext.bundleId.jsonLiteral
  ) {
    throw new TypeError("settlement approval acknowledgement binding is invalid");
  }
  if (
    value.committee_authority === null
    || typeof value.committee_authority !== "object"
    || Array.isArray(value.committee_authority)
    || value.committee_authority.route === null
    || typeof value.committee_authority.route !== "object"
    || Array.isArray(value.committee_authority.route)
    || !isU64(value.committee_authority.route.dataspace_id)
    || u64BigInt(value.committee_authority.route.dataspace_id)
      !== u64BigInt(identity.approvalContext.dataspaceId)
  ) {
    throw new TypeError("settlement approval acknowledgement authority is invalid");
  }
  validateBlsNormalSignatureLiteral(
    attestation.signature,
    "settlement approval acknowledgement responder signature",
  );
}

function identifier(value) {
  return value instanceof AtomicPrivateSettlementIdentifierV1
    ? value
    : new AtomicPrivateSettlementIdentifierV1(value);
}

function requireProvider(provider, context) {
  if (typeof provider !== "function") {
    throw new TypeError(`${context} requires an exact-request header provider`);
  }
  return provider;
}

function exactAuthHeaders(value, expectedNames, context) {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must return one exact header object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${context} must return one plain header object`);
  }
  const normalized = {};
  for (const name of Reflect.ownKeys(value)) {
    if (typeof name !== "string") {
      throw new TypeError(`${context} returned a non-string header name`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(value, name);
    if (!descriptor?.enumerable) continue;
    if (!("value" in descriptor)) {
      throw new TypeError(`${context} returned an accessor header`);
    }
    const headerValue = descriptor.value;
    if (
      typeof headerValue !== "string"
      || headerValue.length === 0
      || /[\r\n]/u.test(headerValue)
    ) {
      throw new TypeError(`${context} returned an invalid header value`);
    }
    const normalizedName = name.toLowerCase();
    if (Object.prototype.hasOwnProperty.call(normalized, normalizedName)) {
      throw new TypeError(`${context} returned a duplicate header name`);
    }
    normalized[normalizedName] = headerValue;
  }
  exactFields(normalized, expectedNames, `${context} result`);
  return normalized;
}

const SPONSOR_HEADER_NAMES = Object.freeze([
  "x-iroha-account",
  "x-iroha-signature",
  "x-iroha-timestamp-ms",
  "x-iroha-nonce",
]);
const ROLE_HEADER_NAMES = Object.freeze([
  "x-iroha-operator-public-key",
  "x-iroha-operator-timestamp-ms",
  "x-iroha-operator-nonce",
  "x-iroha-operator-signature",
]);

async function cancelBody(response) {
  try {
    await response?.body?.cancel?.();
  } catch {
    // Best-effort cancellation follows a terminal redacted error.
  }
}

async function boundedResponseBytes(response, maximumBytes) {
  const contentLength = response.headers.get("content-length");
  let declaredLength = null;
  if (contentLength !== null) {
    if (!/^(?:0|[1-9][0-9]*)$/u.test(contentLength)) {
      await cancelBody(response);
      throw new TypeError("settlement response Content-Length is not canonical");
    }
    if (BigInt(contentLength) > BigInt(maximumBytes)) {
      await cancelBody(response);
      throw new RangeError("settlement response exceeds its byte bound");
    }
    declaredLength = Number(contentLength);
  }
  const reader = response.body?.getReader?.();
  if (reader === undefined) {
    await cancelBody(response);
    throw new TypeError("settlement response requires a readable byte stream");
  }
  const chunks = [];
  let total = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      if (!(value instanceof Uint8Array)) {
        throw new TypeError("settlement response yielded a non-byte chunk");
      }
      if (value.byteLength > maximumBytes - total) {
        throw new RangeError("settlement response exceeds its byte bound");
      }
      chunks.push(value);
      total += value.byteLength;
    }
  } catch (error) {
    try {
      await reader.cancel();
    } catch {
      // Preserve the primary bounded-read failure.
    }
    throw error;
  }
  if (total === 0) throw new TypeError("settlement response must not be empty");
  if (declaredLength !== null && declaredLength !== total) {
    throw new TypeError("settlement response length does not match Content-Length");
  }
  const result = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return result;
}

function requestOptions(options, allowed, context) {
  if (options === undefined) return {};
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    throw new TypeError(`${context} options must be a plain object`);
  }
  const prototype = Object.getPrototypeOf(options);
  if (prototype !== Object.prototype && prototype !== null) {
    throw new TypeError(`${context} options must be a plain object`);
  }
  for (const key of Reflect.ownKeys(options)) {
    if (typeof key !== "string") {
      throw new TypeError(`${context} option names must be strings`);
    }
    const descriptor = Object.getOwnPropertyDescriptor(options, key);
    if (!descriptor?.enumerable) continue;
    if (!("value" in descriptor)) {
      throw new TypeError(`${context} option ${key} must not be an accessor`);
    }
    if (!allowed.has(key)) throw new TypeError(`${context} option ${key} is unsupported`);
  }
  return options;
}

/**
 * Witness-free exact-route client for prepared-leg, audit, coordination, and
 * redacted query workflows. Header providers receive the final method, path,
 * URL, and a defensive body copy and must return exactly one authentication
 * quartet; token and caller-supplied ambient headers are not accepted.
 */
export class AtomicPrivateSettlementToriiClientV1 {
  #baseUrl;
  #fetch;
  #sponsorHeaderProvider;
  #networkId;
  #nativeRuntime;

  constructor(
    baseUrl,
    {
      fetchImpl = globalThis.fetch,
      sponsorHeaderProvider,
      networkId,
      nativeVerifier,
    } = {},
  ) {
    const parsed = new URL(String(baseUrl));
    if (
      !["http:", "https:"].includes(parsed.protocol)
      || parsed.username
      || parsed.password
      || parsed.search
      || parsed.hash
    ) {
      throw new TypeError("settlement Torii base URL must be an exact HTTP(S) base");
    }
    if (typeof fetchImpl !== "function") {
      throw new TypeError("settlement Torii client requires fetch");
    }
    this.#baseUrl = parsed.href.replace(/\/+$/u, "");
    this.#fetch = fetchImpl;
    this.#sponsorHeaderProvider = sponsorHeaderProvider;
    this.#networkId = networkId === undefined || networkId === null
      ? null
      : identifier(networkId);
    this.#nativeRuntime = createNativeRuntime(nativeVerifier);
  }

  #requireAttestationNetwork(context) {
    if (this.#networkId === null) {
      throw new TypeError(`${context} requires a configured settlement networkId`);
    }
    return this.#networkId;
  }

  #nativeFunction(name) {
    try {
      const binding = resolveNativeRuntimeBinding(this.#nativeRuntime);
      const verifier = binding[name];
      if (typeof verifier !== "function") {
        throw new TypeError("native response verifier is unavailable");
      }
      return verifier;
    } catch {
      throw new AtomicPrivateSettlementToriiErrorV1(
        "atomic private settlement restricted response verifier is unavailable",
      );
    }
  }

  async #request(method, route, body, headerProvider, headerNames, maximumBytes, identity) {
    const target = new URL(`${this.#baseUrl}${route}`);
    const headers = {
      Accept: JSON_MEDIA_TYPE,
      "Accept-Encoding": "identity",
      "Cache-Control": "no-store",
      Pragma: "no-cache",
    };
    if (body.byteLength > 0) headers["Content-Type"] = JSON_MEDIA_TYPE;
    if (headerProvider !== null) {
      const authBody = body.slice();
      let generated;
      try {
        generated = await headerProvider({
          method,
          path: route,
          url: target.href,
          body: authBody,
        });
      } finally {
        authBody.fill(0);
      }
      const generatedHeaders = exactAuthHeaders(
        generated,
        headerNames,
        "settlement auth provider",
      );
      Object.assign(headers, generatedHeaders);
      if (identity?.auditorKeyFromRoleHeader === true) {
        identity = {
          ...identity,
          auditorSigningKey: generatedHeaders["x-iroha-operator-public-key"],
        };
      }
    }

    const transportBody = body.byteLength > 0 ? body.slice() : null;
    let response;
    try {
      response = await this.#fetch(target.href, {
        method,
        headers,
        body: transportBody,
        redirect: "error",
        credentials: "omit",
        cache: "no-store",
        referrerPolicy: "no-referrer",
        signal: identity?.signal,
      });
    } catch (error) {
      throw new AtomicPrivateSettlementToriiErrorV1(
        "atomic private settlement request failed",
        { cause: error },
      );
    } finally {
      transportBody?.fill(0);
    }

    if (response.redirected !== false || response.url !== target.href) {
      await cancelBody(response);
      throw new AtomicPrivateSettlementToriiErrorV1(
        "atomic private settlement response provenance is invalid",
      );
    }
    if (!response.ok) {
      const rejectCode = response.headers.get("x-iroha-reject-code");
      await cancelBody(response);
      const suffix = /^[A-Za-z0-9_.:-]{1,128}$/u.test(rejectCode ?? "")
        ? `; reject_code=${rejectCode}`
        : "";
      throw new AtomicPrivateSettlementToriiErrorV1(
        `atomic private settlement request failed with HTTP ${response.status}${suffix}`,
      );
    }
    const expectedStatus = route.endsWith("/bundles") ? 202 : 200;
    if (response.status !== expectedStatus) {
      await cancelBody(response);
      throw new AtomicPrivateSettlementToriiErrorV1(
        "atomic private settlement response status is invalid",
      );
    }
    if (response.headers.get("content-type") !== JSON_MEDIA_TYPE) {
      await cancelBody(response);
      throw new AtomicPrivateSettlementToriiErrorV1(
        "atomic private settlement response content type is invalid",
      );
    }
    const contentEncoding = response.headers.get("content-encoding");
    if (contentEncoding !== null && contentEncoding !== "identity") {
      await cancelBody(response);
      throw new AtomicPrivateSettlementToriiErrorV1(
        "atomic private settlement response encoding is invalid",
      );
    }

    try {
      const responseBytes = await boundedResponseBytes(response, maximumBytes);
      const parsedBody = strictObject(responseBytes, "atomic private settlement response");
      exactFields(parsedBody, responseFields(route), "atomic private settlement response");
      if (route.endsWith("/bundles")) validateBundleAdmission(parsedBody);
      if (route.endsWith("/audit-capsule")) {
        const height = parsedBody.authoritative_height;
        const validNumber = typeof height === "number" && Number.isSafeInteger(height) && height > 0;
        const validBigInt = typeof height === "bigint" && height > 0n && height <= 0xffffffffffffffffn;
        if (!validNumber && !validBigInt) {
          throw new TypeError("settlement auditor capsule authoritative_height must be a nonzero u64");
        }
        if (identity?.network === undefined || identity?.value === undefined) {
          throw new TypeError("settlement auditor capsule request binding is missing");
        }
        validateAuditorCapsuleAttestation(parsedBody, identity);
      }
      if (route.endsWith("/audit-approvals")) {
        if (
          identity?.network === undefined
          || identity?.value === undefined
          || identity?.approvalContext === undefined
        ) {
          throw new TypeError("settlement approval acknowledgement request binding is missing");
        }
        validateAuditApprovalAcknowledgementAttestation(parsedBody, identity);
      }
      if (identity?.field !== undefined && parsedBody[identity.field] !== identity.value.jsonLiteral) {
        throw new TypeError("settlement response identifier is substituted");
      }
      if (route.endsWith("/phase-certificates")) {
        for (const field of ["prepare_certificate", "commit_certificate"]) {
          const certificate = parsedBody[field];
          if (
            certificate !== null
            && (typeof certificate !== "object" || Array.isArray(certificate))
          ) {
            throw new TypeError("settlement phase certificate must be null or an opaque object");
          }
        }
      }
      if (route.endsWith("/receipt")) {
        if (!["pending", "finalized", "aborted"].includes(parsedBody.status)) {
          throw new TypeError("settlement receipt has an unknown tag");
        }
        if (
          parsedBody.value === null
          || typeof parsedBody.value !== "object"
          || Array.isArray(parsedBody.value)
          || parsedBody.value.bundle_id !== identity.value.jsonLiteral
        ) {
          throw new TypeError("settlement receipt identifier is substituted");
        }
      } else if (route.includes("/bundles/") && parsedBody.manifest !== null) {
        if (
          typeof parsedBody.manifest !== "object"
          || Array.isArray(parsedBody.manifest)
          || parsedBody.manifest.bundle_id !== identity.value.jsonLiteral
        ) {
          throw new TypeError("settlement bundle status is substituted");
        }
      }
      if (identity?.nativeVerification !== undefined) {
        const { kind, verify } = identity.nativeVerification;
        if (kind === "committee") {
          await verify(responseBytes, identity.network.bytes, identity.value.bytes);
        } else if (kind === "capsule") {
          await verify(
            responseBytes,
            identity.network.bytes,
            identity.value.bytes,
            identity.auditorSigningKey,
          );
        } else if (kind === "approval") {
          await verify(
            responseBytes,
            body,
            identity.network.bytes,
            identity.value.bytes,
            identity.auditorSigningKey,
          );
        } else {
          throw new TypeError("settlement native verification mode is invalid");
        }
      }
      return new AtomicPrivateSettlementJsonResponseV1(route, responseBytes);
    } catch (error) {
      if (error instanceof AtomicPrivateSettlementToriiErrorV1) throw error;
      throw new AtomicPrivateSettlementToriiErrorV1(
        "atomic private settlement response is invalid",
      );
    }
  }

  async #sponsorMutation(request, expectedOperation, options) {
    if (!(request instanceof AtomicPrivateSettlementPreparedRequestV1)) {
      throw new TypeError("settlement request must be native-prepared");
    }
    if (request.operation !== expectedOperation) {
      throw new TypeError("prepared settlement request is bound to another operation");
    }
    const normalized = requestOptions(
      options,
      new Set(["sponsorHeaderProvider", "signal"]),
      "settlement sponsor mutation",
    );
    const provider = requireProvider(
      normalized.sponsorHeaderProvider ?? this.#sponsorHeaderProvider,
      "settlement sponsor mutation",
    );
    return this.#request(
      "POST",
      expectedOperation.path,
      request.bytes(),
      provider,
      SPONSOR_HEADER_NAMES,
      RESPONSE_RESTRICTED_MAX_BYTES,
      { signal: normalized.signal },
    );
  }

  requestAvailabilityShare(request, options) {
    return this.#sponsorMutation(
      request,
      AtomicPrivateSettlementOperationV1.AVAILABILITY_SHARE,
      options,
    );
  }

  requestPrepareVote(request, options) {
    return this.#sponsorMutation(
      request,
      AtomicPrivateSettlementOperationV1.PREPARE_VOTE,
      options,
    );
  }

  requestCommitVote(request, options) {
    return this.#sponsorMutation(
      request,
      AtomicPrivateSettlementOperationV1.COMMIT_VOTE,
      options,
    );
  }

  persistPhaseCertificate(request, options) {
    return this.#sponsorMutation(
      request,
      AtomicPrivateSettlementOperationV1.PHASE_CERTIFICATE,
      options,
    );
  }

  uploadLeg(request, options) {
    return this.#sponsorMutation(
      request,
      AtomicPrivateSettlementOperationV1.LEG_UPLOAD,
      options,
    );
  }

  submitBundle(request, options) {
    return this.#sponsorMutation(
      request,
      AtomicPrivateSettlementOperationV1.BUNDLE_SUBMIT,
      options,
    );
  }

  async submitAuditApproval(payloadDigest, request, options) {
    if (!(request instanceof AtomicPrivateSettlementPreparedRequestV1)) {
      throw new TypeError("settlement request must be native-prepared");
    }
    if (request.operation !== AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL) {
      throw new TypeError("prepared settlement request is bound to another operation");
    }
    const normalized = requestOptions(
      options,
      new Set(["roleHeaderProvider", "signal"]),
      "settlement auditor approval",
    );
    const digest = identifier(payloadDigest);
    const network = this.#requireAttestationNetwork("settlement auditor approval");
    const approvalContext = auditApprovalRequestContext(request);
    if (approvalContext.networkId.jsonLiteral !== network.jsonLiteral) {
      throw new TypeError(
        "prepared settlement approval network differs from the configured networkId",
      );
    }
    const nativeVerifier = this.#nativeFunction(
      "privateSettlementVerifyAuditApprovalResponseV1",
    );
    const route = request.operation.path.replace("{payload_digest}", digest.pathComponent);
    return this.#request(
      "POST",
      route,
      request.bytes(),
      requireProvider(normalized.roleHeaderProvider, "settlement auditor approval"),
      ROLE_HEADER_NAMES,
      RESPONSE_RESTRICTED_MAX_BYTES,
      {
        field: "payload_digest",
        value: digest,
        network,
        approvalContext,
        auditorKeyFromRoleHeader: true,
        nativeVerification: { kind: "approval", verify: nativeVerifier },
        signal: normalized.signal,
      },
    );
  }

  async getLegStatus(payloadDigest, options) {
    const normalized = requestOptions(
      options,
      new Set(["sponsorHeaderProvider", "signal"]),
      "settlement leg status",
    );
    const digest = identifier(payloadDigest);
    const route = `/v1/nexus/private-settlements/legs/${digest.pathComponent}/status`;
    return this.#request(
      "GET",
      route,
      new Uint8Array(),
      requireProvider(
        normalized.sponsorHeaderProvider ?? this.#sponsorHeaderProvider,
        "settlement leg status",
      ),
      SPONSOR_HEADER_NAMES,
      RESPONSE_SMALL_MAX_BYTES,
      { field: "payload_digest", value: digest, signal: normalized.signal },
    );
  }

  async getPhaseCertificates(payloadDigest, options) {
    const normalized = requestOptions(
      options,
      new Set(["sponsorHeaderProvider", "signal"]),
      "settlement phase-certificate recovery",
    );
    const digest = identifier(payloadDigest);
    const route =
      `/v1/nexus/private-settlements/legs/${digest.pathComponent}/phase-certificates`;
    return this.#request(
      "GET",
      route,
      new Uint8Array(),
      requireProvider(
        normalized.sponsorHeaderProvider ?? this.#sponsorHeaderProvider,
        "settlement phase-certificate recovery",
      ),
      SPONSOR_HEADER_NAMES,
      RESPONSE_SMALL_MAX_BYTES,
      { field: "payload_digest", value: digest, signal: normalized.signal },
    );
  }

  async getCommitteeProof(payloadDigest, options) {
    return this.#roleGet(payloadDigest, options, "committee-proof", "settlement committee proof");
  }

  async getAuditorCapsule(payloadDigest, options) {
    return this.#roleGet(payloadDigest, options, "audit-capsule", "settlement auditor capsule");
  }

  async #roleGet(payloadDigest, options, suffix, context) {
    const normalized = requestOptions(
      options,
      new Set(["roleHeaderProvider", "signal"]),
      context,
    );
    const digest = identifier(payloadDigest);
    const network = this.#requireAttestationNetwork(context);
    const capsule = suffix === "audit-capsule";
    const nativeVerifier = this.#nativeFunction(
      capsule
        ? "privateSettlementVerifyAuditorCapsuleResponseV1"
        : "privateSettlementVerifyCommitteeProofResponseV1",
    );
    const route = `/v1/nexus/private-settlements/legs/${digest.pathComponent}/${suffix}`;
    return this.#request(
      "GET",
      route,
      new Uint8Array(),
      requireProvider(normalized.roleHeaderProvider, context),
      ROLE_HEADER_NAMES,
      RESPONSE_RESTRICTED_MAX_BYTES,
      {
        value: digest,
        network,
        auditorKeyFromRoleHeader: capsule,
        nativeVerification: {
          kind: capsule ? "capsule" : "committee",
          verify: nativeVerifier,
        },
        signal: normalized.signal,
      },
    );
  }

  async getBundleStatus(bundleId, options) {
    return this.#publicBundleRead(bundleId, options, false);
  }

  async getBundleReceipt(bundleId, options) {
    return this.#publicBundleRead(bundleId, options, true);
  }

  async #publicBundleRead(bundleId, options, receipt) {
    const normalized = requestOptions(
      options,
      new Set(["signal"]),
      "settlement public bundle read",
    );
    const digest = identifier(bundleId);
    const suffix = receipt ? "/receipt" : "";
    const route = `/v1/nexus/private-settlements/bundles/${digest.pathComponent}${suffix}`;
    return this.#request(
      "GET",
      route,
      new Uint8Array(),
      null,
      [],
      receipt ? RESPONSE_RESTRICTED_MAX_BYTES : RESPONSE_PUBLIC_BUNDLE_MAX_BYTES,
      { value: digest, signal: normalized.signal },
    );
  }
}
