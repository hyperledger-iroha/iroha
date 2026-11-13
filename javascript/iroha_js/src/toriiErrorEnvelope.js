// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Canonical decoders for Torii's HTTP error envelope,
 * `iroha_torii_shared::ErrorEnvelope { code, message, details }`.
 *
 * Torii renders a failure in the representation the request negotiated: one
 * canonical Norito frame for `Accept: application/x-norito`, Norito JSON
 * otherwise. These decoders accept exactly those renderings, byte for byte, of
 * an envelope whose optional `details` are absent or carry only the flat
 * fields Torii attaches to admission, throttling and validation failures: the
 * reject code, queue snapshot, retry delay, AXT and fee metadata, and the
 * textual diagnostics (`layer`, `endpoint`, `field`, `expected`, `actual`,
 * `profile`, `chain_discriminant`, `entrypoint_hash`, `tx_hash`,
 * `last_status`, `hint`). They throw for any other body, so each caller keeps
 * its own error for a body that is not such an envelope.
 *
 * Details that carry one of the structured observations (an IVM fault, a
 * pipeline-status, alias, SNS, asset or settlement-receipt absence, a
 * finality-attestation or AMX approval-read failure, or an alias setup report)
 * are outside that subset and are refused rather than partially decoded.
 * TODO: decode those observations here once an SDK route needs them typed;
 * each needs its own canonical codec.
 *
 * Decoded `details` take the Norito JSON shape: snake_case keys, absent
 * options omitted, and each unsigned integer a `number` when it is a safe
 * integer and a `bigint` otherwise.
 */
import { validateNoritoFrame } from "./noritoValueCodecs.js";

/** Rust schema name that binds the Norito error-envelope frame. */
export const TORII_ERROR_ENVELOPE_TYPE_NAME_V1 = "iroha_torii_shared::ErrorEnvelope";

const COMPACT_LEN_FLAG_V1 = 0x02;
const NONCANONICAL_JSON_ERROR_V1 = "non-canonical JSON error envelope";
const MAX_SAFE_INTEGER_BIGINT = BigInt(Number.MAX_SAFE_INTEGER);
// A leading U+FEFF is text, not a byte-order mark to strip.
const STRICT_UTF8 = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
// In a `u` regex a surrogate pair is one astral code point, so this range
// matches only unpaired surrogates.
const LONE_SURROGATE = /[\uD800-\uDFFF]/u;

const TEXT = Object.freeze({ kind: "text" });
const BOOL = Object.freeze({ kind: "bool" });
const U16 = Object.freeze({ kind: "uint", width: 2 });
const U32 = Object.freeze({ kind: "uint", width: 4 });
const U64 = Object.freeze({ kind: "uint", width: 8 });
// A structured observation whose payload schema these decoders do not decode.
const UNDECODED = Object.freeze({ kind: "undecoded" });

const required = (name, type) => Object.freeze({ name, type, optional: false });
// `Option<T>` with `skip_serializing_if = "Option::is_none"`: a tagged Norito
// field, and a JSON key that is omitted when absent.
const optional = (name, type) => Object.freeze({ name, type, optional: true });
const record = (...fields) => Object.freeze({ kind: "record", fields: Object.freeze(fields) });

// Each record lists the Rust struct's fields in declaration order.
const QUEUE_ERROR_SNAPSHOT_V1 = record(
  required("state", TEXT),
  required("queued", U64),
  required("capacity", U64),
  required("saturated", BOOL),
);
const AXT_ERROR_DETAILS_V1 = record(
  optional("code", TEXT),
  optional("reason", TEXT),
  optional("snapshot_version", U64),
  optional("dataspace", U64),
  optional("lane", U32),
  optional("active_handle_era", U64),
  optional("next_handle_counter", U64),
);
const FEE_ERROR_DETAILS_V1 = record(
  required("code", TEXT),
  required("retryable", BOOL),
  optional("program_id", TEXT),
  optional("program_revision", U64),
  optional("asset_definition_id", TEXT),
  optional("required", TEXT),
  optional("available", TEXT),
  optional("rule_id", TEXT),
  optional("observation_height", U64),
  optional("remediation", TEXT),
);
const ERROR_DETAILS_V1 = record(
  optional("ivm_fault", UNDECODED),
  optional("layer", TEXT),
  optional("reject_code", TEXT),
  optional("queue", QUEUE_ERROR_SNAPSHOT_V1),
  optional("retry_after_seconds", U64),
  optional("endpoint", TEXT),
  optional("field", TEXT),
  optional("expected", TEXT),
  optional("actual", TEXT),
  optional("profile", TEXT),
  optional("chain_discriminant", U16),
  optional("entrypoint_hash", TEXT),
  optional("tx_hash", TEXT),
  optional("pipeline_transaction_status_not_found", UNDECODED),
  optional("finality_attestation_failure", UNDECODED),
  optional("amx_approval_read_failure", UNDECODED),
  optional("last_status", TEXT),
  optional("hint", TEXT),
  optional("axt", AXT_ERROR_DETAILS_V1),
  optional("fee", FEE_ERROR_DETAILS_V1),
  optional("alias_setup_report", UNDECODED),
  optional("account_alias_not_found", UNDECODED),
  optional("account_aliases_by_account_not_found", UNDECODED),
  optional("sns_registration_not_found", UNDECODED),
  optional("query_asset_not_found", UNDECODED),
  optional("query_settlement_receipt_not_found", UNDECODED),
);
const ERROR_ENVELOPE_V1 = record(
  required("code", TEXT),
  required("message", TEXT),
  optional("details", ERROR_DETAILS_V1),
);

function unsignedValue(value) {
  return value <= MAX_SAFE_INTEGER_BIGINT ? Number(value) : value;
}

function undecodedDetail() {
  return new Error("error details carry an observation these decoders do not decode");
}

function readCompactLength(payload, state) {
  let value = 0n;
  let shift = 0n;
  let count = 0;
  for (;;) {
    if (state.offset >= payload.length || count >= 10) {
      throw new Error("invalid compact length");
    }
    const byte = payload[state.offset];
    state.offset += 1;
    count += 1;
    value |= BigInt(byte & 0x7f) << shift;
    if ((byte & 0x80) === 0) {
      if ((count > 1 && byte === 0) || value > BigInt(Number.MAX_SAFE_INTEGER)) {
        throw new Error("non-canonical compact length");
      }
      return Number(value);
    }
    shift += 7n;
  }
}

// One compact-length-prefixed span: a record field, an option payload or a
// string body.
function readNoritoSpan(payload, state, what) {
  const length = readCompactLength(payload, state);
  const end = state.offset + length;
  if (end > payload.length) {
    throw new Error(`truncated error-envelope ${what}`);
  }
  const span = payload.subarray(state.offset, end);
  state.offset = end;
  return span;
}

// Decode a value that must consume exactly `bytes`, its declared span.
function decodeNoritoValue(type, bytes) {
  switch (type.kind) {
    case "text": {
      const state = { offset: 0 };
      const text = readNoritoSpan(bytes, state, "string");
      if (state.offset !== bytes.length) {
        throw new Error("trailing bytes in error-envelope string field");
      }
      return STRICT_UTF8.decode(text);
    }
    case "bool":
      if (bytes.length !== 1 || bytes[0] > 1) {
        throw new Error("invalid error-envelope bool");
      }
      return bytes[0] === 1;
    case "uint": {
      if (bytes.length !== type.width) {
        throw new Error("invalid error-envelope integer width");
      }
      let value = 0n;
      for (let index = type.width - 1; index >= 0; index -= 1) {
        value = (value << 8n) | BigInt(bytes[index]);
      }
      return unsignedValue(value);
    }
    case "record":
      return decodeNoritoRecord(type, bytes);
    default:
      throw undecodedDetail();
  }
}

// A derived record: every declared field as one length-prefixed span, in
// order, with no trailing bytes. An absent option is the lone tag 0; a present
// one is tag 1 followed by its value's length-prefixed payload.
function decodeNoritoRecord(type, payload) {
  const state = { offset: 0 };
  const value = {};
  for (const field of type.fields) {
    const span = readNoritoSpan(payload, state, "field");
    if (!field.optional) {
      value[field.name] = decodeNoritoValue(field.type, span);
      continue;
    }
    if (span.length === 1 && span[0] === 0) {
      continue;
    }
    if (span.length === 0 || span[0] !== 1) {
      throw new Error("invalid error-envelope option tag");
    }
    const inner = { offset: 1 };
    const present = readNoritoSpan(span, inner, "option payload");
    if (inner.offset !== span.length) {
      throw new Error("trailing bytes in error-envelope option field");
    }
    value[field.name] = decodeNoritoValue(field.type, present);
  }
  if (state.offset !== payload.length) {
    throw new Error("trailing error-envelope payload");
  }
  return value;
}

function nonCanonicalJson() {
  return new Error(NONCANONICAL_JSON_ERROR_V1);
}

function expectJson(reader, character) {
  if (reader.text[reader.offset] !== character) {
    throw nonCanonicalJson();
  }
  reader.offset += 1;
}

// A string token as Norito JSON writes it: `JSON.stringify` escaping (quote,
// backslash, the short control escapes and lowercase `\u00xx` for the other
// controls; everything else literal). A lone surrogate can only arrive as an
// escape, which no UTF-8 string produces.
function readJsonString(reader) {
  const { text } = reader;
  const start = reader.offset;
  if (text[start] !== "\"") {
    throw nonCanonicalJson();
  }
  let index = start + 1;
  for (;;) {
    if (index >= text.length) {
      throw nonCanonicalJson();
    }
    const unit = text.charCodeAt(index);
    if (unit === 0x22) {
      break;
    }
    if (unit < 0x20) {
      throw nonCanonicalJson();
    }
    index += unit === 0x5c ? (text[index + 1] === "u" ? 6 : 2) : 1;
  }
  const token = text.slice(start, index + 1);
  let value;
  try {
    value = JSON.parse(token);
  } catch {
    throw nonCanonicalJson();
  }
  if (typeof value !== "string" || JSON.stringify(value) !== token || LONE_SURROGATE.test(value)) {
    throw nonCanonicalJson();
  }
  reader.offset = index + 1;
  return value;
}

function readJsonUnsigned(reader, width) {
  const { text } = reader;
  const start = reader.offset;
  let end = start;
  while (end < text.length && text.charCodeAt(end) >= 0x30 && text.charCodeAt(end) <= 0x39) {
    end += 1;
  }
  if (end === start || (text[start] === "0" && end - start > 1)) {
    throw nonCanonicalJson();
  }
  const value = BigInt(text.slice(start, end));
  if (value >> BigInt(width * 8) !== 0n) {
    throw nonCanonicalJson();
  }
  reader.offset = end;
  return unsignedValue(value);
}

function decodeJsonValue(type, reader) {
  switch (type.kind) {
    case "text":
      return readJsonString(reader);
    case "bool":
      for (const [literal, value] of [["true", true], ["false", false]]) {
        if (reader.text.startsWith(literal, reader.offset)) {
          reader.offset += literal.length;
          return value;
        }
      }
      throw nonCanonicalJson();
    case "uint":
      return readJsonUnsigned(reader, type.width);
    case "record":
      return decodeJsonRecord(type, reader);
    default:
      throw undecodedDetail();
  }
}

// A record object: its keys in declaration order with no whitespace, every
// required field present, absent options omitted (never `null`) and no other
// key.
function decodeJsonRecord(type, reader) {
  expectJson(reader, "{");
  const value = {};
  let next = 0;
  while (reader.text[reader.offset] !== "}") {
    // Every key read so far advanced `next` past its field.
    if (next > 0) {
      expectJson(reader, ",");
    }
    const key = readJsonString(reader);
    expectJson(reader, ":");
    let field;
    while (next < type.fields.length) {
      const candidate = type.fields[next];
      next += 1;
      if (candidate.name === key) {
        field = candidate;
        break;
      }
      if (!candidate.optional) {
        throw nonCanonicalJson();
      }
    }
    if (field === undefined) {
      throw nonCanonicalJson();
    }
    value[key] = decodeJsonValue(field.type, reader);
  }
  expectJson(reader, "}");
  for (; next < type.fields.length; next += 1) {
    if (!type.fields[next].optional) {
      throw nonCanonicalJson();
    }
  }
  return value;
}

function finishEnvelope({ code, message, details }) {
  // Torii never emits an empty code; such a body is not a typed refusal.
  if (code.length === 0) {
    throw new Error("error envelope code must be non-empty");
  }
  return { code, message, details: details ?? null };
}

/**
 * Decode one canonical, unpadded Norito `ErrorEnvelope` frame.
 *
 * @param {Uint8Array} body
 * @param {string} [context] diagnostic label for frame validation failures
 * @returns {{code: string, message: string, details: Record<string, unknown> | null}}
 */
export function decodeCanonicalNoritoErrorEnvelope(body, context = "Torii error envelope") {
  const frame = validateNoritoFrame(body, {
    context,
    expectedTypeName: TORII_ERROR_ENVELOPE_TYPE_NAME_V1,
    expectedPaddingLength: 0,
    requireNonEmptyPayload: true,
  });
  if (frame.flags !== COMPACT_LEN_FLAG_V1) {
    throw new Error("non-canonical error envelope flags");
  }
  return finishEnvelope(decodeNoritoRecord(ERROR_ENVELOPE_V1, frame.payload));
}

/**
 * Decode Torii's Norito JSON rendering of an `ErrorEnvelope`: exactly
 * `{"code":…,"message":…}` or `{"code":…,"message":…,"details":{…}}` in strict
 * UTF-8, with Norito's key order and string escaping and no insignificant
 * whitespace.
 *
 * @param {Uint8Array} body
 * @returns {{code: string, message: string, details: Record<string, unknown> | null}}
 */
export function decodeCanonicalJsonErrorEnvelope(body) {
  let text;
  try {
    text = STRICT_UTF8.decode(body);
  } catch {
    throw nonCanonicalJson();
  }
  const reader = { text, offset: 0 };
  const envelope = decodeJsonRecord(ERROR_ENVELOPE_V1, reader);
  if (reader.offset !== text.length) {
    throw nonCanonicalJson();
  }
  return finishEnvelope(envelope);
}

/**
 * Classify a non-success response body as Torii's canonical error envelope.
 *
 * The declared media type selects the representation as the Rust client does:
 * `application/x-norito` is Norito; an absent type, `application/json` or an
 * `application/*+json` type is JSON. Any other media type, and any body that is
 * not exactly one canonical envelope these decoders accept, returns `null`.
 *
 * @param {string | null | undefined} contentType
 * @param {Uint8Array} body
 * @returns {{code: string, message: string, details: Record<string, unknown> | null, format: "norito" | "json"} | null}
 */
export function decodeTypedToriiErrorEnvelope(contentType, body) {
  const mediaType =
    contentType === null || contentType === undefined
      ? null
      : String(contentType).split(";", 1)[0].trim().toLowerCase();
  let format;
  if (mediaType === "application/x-norito") {
    format = "norito";
  } else if (
    mediaType === null ||
    mediaType === "application/json" ||
    (mediaType.startsWith("application/") && mediaType.endsWith("+json"))
  ) {
    format = "json";
  } else {
    return null;
  }
  try {
    const envelope =
      format === "norito"
        ? decodeCanonicalNoritoErrorEnvelope(body)
        : decodeCanonicalJsonErrorEnvelope(body);
    return { ...envelope, format };
  } catch {
    return null;
  }
}
