// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

// Builders for Torii's `iroha_torii_shared::ErrorEnvelope` error bodies. The
// framing comes from the Rust-authored frames in the bootle-lantern client
// fixture and in `fixtures/torii/error_envelope_details_v1.json`;
// `toriiNoritoErrorEnvelope.test.js` and `toriiErrorEnvelopeDetails.test.js`
// prove the builders reproduce every one of them byte for byte.
import { readFileSync } from "node:fs";
import { crc64Xz } from "../../src/crc64Xz.js";

export const errorEnvelopeContract = JSON.parse(readFileSync(new URL(
  "../../../../fixtures/privacy/bootle_lantern_issuance_client_v1.json", import.meta.url), "utf8")).errors;

/**
 * Rust-authored envelopes whose `details` carry Torii's admission, throttling and
 * observation metadata. An integer beyond the safe range is read as the exact
 * `bigint` the decoders return for it.
 */
export const errorEnvelopeDetailsContract = JSON.parse(
  readFileSync(new URL("../../../../fixtures/torii/error_envelope_details_v1.json", import.meta.url), "utf8"),
  (_key, value, context) =>
    typeof value === "number" && !Number.isSafeInteger(value) ? BigInt(context.source) : value,
);

export function leb128(value) {
  const out = [];
  do { out.push((value & 0x7f) | (value > 0x7f ? 0x80 : 0)); value >>>= 7; } while (value > 0);
  return Buffer.from(out);
}
/** One compact-length-prefixed record field. */
export const noritoField = (bytes) => Buffer.concat([leb128(bytes.length), bytes]);
/** A `String` value: compact length, then UTF-8. */
export const noritoString = (value) => Buffer.concat([leb128(Buffer.byteLength(value)), Buffer.from(value)]);
/** A present `Option<T>`: tag 1, then the value's length-prefixed payload. */
export const noritoSome = (bytes) => Buffer.concat([Buffer.of(1), leb128(bytes.length), bytes]);
/** An absent `Option<T>`. */
export const NORITO_NONE = Buffer.of(0);
/** A little-endian unsigned integer of `width` bytes. */
export function noritoUnsigned(value, width) {
  const bytes = Buffer.alloc(width);
  let remaining = BigInt(value);
  for (let index = 0; index < width; index += 1) {
    bytes[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return bytes;
}

const TEXT = noritoString;
const BOOL = (value) => Buffer.of(value ? 1 : 0);
const U16 = (value) => noritoUnsigned(value, 2);
const U32 = (value) => noritoUnsigned(value, 4);
const U64 = (value) => noritoUnsigned(value, 8);
const UNDECODED = () => {
  throw new Error("pass this observation's field bytes through `rawDetails`");
};
const required = (name, encode) => ({ name, encode, optional: false });
const optional = (name, encode) => ({ name, encode, optional: true });
// Field values in declaration order; `raw` replaces a field's bytes inside its
// length prefix (the option tag included) to craft malformed records.
const record = (...fields) => (value, raw = {}) => Buffer.concat(fields.map(({ name, encode, optional: isOptional }) => {
  if (Object.hasOwn(raw, name)) return noritoField(raw[name]);
  if (!isOptional) return noritoField(encode(value[name]));
  return noritoField(value[name] === undefined ? NORITO_NONE : noritoSome(encode(value[name])));
}));

const QUEUE = record(required("state", TEXT), required("queued", U64), required("capacity", U64),
  required("saturated", BOOL));
const AXT = record(optional("code", TEXT), optional("reason", TEXT), optional("snapshot_version", U64),
  optional("dataspace", U64), optional("lane", U32), optional("active_handle_era", U64),
  optional("next_handle_counter", U64));
const FEE = record(required("code", TEXT), required("retryable", BOOL), optional("program_id", TEXT),
  optional("program_revision", U64), optional("asset_definition_id", TEXT), optional("required", TEXT),
  optional("available", TEXT), optional("rule_id", TEXT), optional("observation_height", U64),
  optional("remediation", TEXT));

/** `iroha_torii_shared::ErrorDetails` field names in declaration order. */
export const ERROR_DETAILS_FIELDS = Object.freeze([
  "ivm_fault", "layer", "reject_code", "queue", "retry_after_seconds", "endpoint", "field", "expected",
  "actual", "profile", "chain_discriminant", "entrypoint_hash", "tx_hash",
  "pipeline_transaction_status_not_found", "finality_attestation_failure", "amx_approval_read_failure",
  "last_status", "hint", "axt", "fee", "alias_setup_report", "account_alias_not_found",
  "account_aliases_by_account_not_found", "sns_registration_not_found", "query_asset_not_found",
  "query_settlement_receipt_not_found",
]);
const DETAIL_ENCODERS = {
  layer: TEXT, reject_code: TEXT, queue: QUEUE, retry_after_seconds: U64, endpoint: TEXT, field: TEXT,
  expected: TEXT, actual: TEXT, profile: TEXT, chain_discriminant: U16, entrypoint_hash: TEXT,
  tx_hash: TEXT, last_status: TEXT, hint: TEXT, axt: AXT, fee: FEE,
};
const DETAILS = record(...ERROR_DETAILS_FIELDS.map((name) => optional(name, DETAIL_ENCODERS[name] ?? UNDECODED)));

/** Encode a details record payload; `raw` maps field names to their bytes. */
export const errorDetailsPayload = (details, raw = {}) => DETAILS(details, raw);

/**
 * `ErrorEnvelope` payload. `details === null` with no `rawDetails` is the
 * absent option; otherwise the details record is present.
 */
export function errorEnvelopePayload(code, message, details = null, rawDetails = undefined) {
  const present = details !== null || rawDetails !== undefined;
  return Buffer.concat([
    noritoField(noritoString(code)),
    noritoField(noritoString(message)),
    noritoField(present ? noritoSome(errorDetailsPayload(details ?? {}, rawDetails ?? {})) : NORITO_NONE),
  ]);
}

/** Frame a payload under the envelope schema with a valid checksum. */
export function errorEnvelopeFrameFromPayload(payload, flags = 0x02) {
  const header = Buffer.alloc(40);
  header.write("NRT0", 0, "ascii");
  Buffer.from(errorEnvelopeContract.norito_envelope.schema_hash_hex, "hex").copy(header, 6);
  header.writeBigUInt64LE(BigInt(payload.length), 23);
  header.writeBigUInt64LE(crc64Xz(payload), 31);
  header[39] = flags;
  return Buffer.concat([header, payload]);
}

/**
 * `norito::to_bytes(&envelope)` for `ErrorEnvelope { code, message, details }`
 * whose details (in their JSON shape) use only the decoded fields.
 */
export function errorEnvelopeFrame(code, message, details = null) {
  return errorEnvelopeFrameFromPayload(errorEnvelopePayload(code, message, details));
}

/** `norito::json::to_json(&ErrorEnvelope::new(code, message))` for escape-free text. */
export const jsonErrorEnvelope = (code, message) => Buffer.from(`{"code":"${code}","message":"${message}"}`);
