// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Canonical decoders for Torii's HTTP error envelope,
 * `iroha_torii_shared::ErrorEnvelope { code, message, details }`.
 *
 * Torii renders a failure in the representation the request negotiated: one
 * canonical Norito frame for `Accept: application/x-norito`, Norito JSON
 * otherwise. These decoders accept exactly those renderings of an envelope
 * whose optional `details` are absent and throw for any other body, so each
 * caller keeps its own error for a body that is not such an envelope.
 */
import { validateNoritoFrame } from "./noritoValueCodecs.js";

/** Rust schema name that binds the Norito error-envelope frame. */
export const TORII_ERROR_ENVELOPE_TYPE_NAME_V1 = "iroha_torii_shared::ErrorEnvelope";

const COMPACT_LEN_FLAG_V1 = 0x02;
const NONCANONICAL_JSON_ERROR_V1 = "non-canonical JSON error envelope";

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

function readNoritoField(payload, state, decodeString = false) {
  const length = readCompactLength(payload, state);
  const end = state.offset + length;
  if (end > payload.length) {
    throw new Error("truncated error-envelope field");
  }
  const field = payload.subarray(state.offset, end);
  state.offset = end;
  if (!decodeString) {
    return field;
  }
  const fieldState = { offset: 0 };
  const stringLength = readCompactLength(field, fieldState);
  const stringEnd = fieldState.offset + stringLength;
  if (stringEnd > field.length) {
    throw new Error("truncated string");
  }
  if (stringEnd !== field.length) {
    throw new Error("trailing bytes in error-envelope string field");
  }
  return new TextDecoder("utf-8", { fatal: true }).decode(
    field.subarray(fieldState.offset, stringEnd),
  );
}

function requireEnvelopeCode(code) {
  // Torii never emits an empty code; such a body is not a typed refusal.
  if (code.length === 0) {
    throw new Error("error envelope code must be non-empty");
  }
}

/**
 * Decode one canonical, unpadded Norito `ErrorEnvelope` frame whose `details`
 * are absent.
 *
 * @param {Uint8Array} body
 * @param {string} [context] diagnostic label for frame validation failures
 * @returns {{code: string, message: string}}
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
  const state = { offset: 0 };
  const code = readNoritoField(frame.payload, state, true);
  const message = readNoritoField(frame.payload, state, true);
  const details = readNoritoField(frame.payload, state);
  if (details.length !== 1 || details[0] !== 0) {
    throw new Error("error details must be absent");
  }
  if (state.offset !== frame.payload.length) {
    throw new Error("trailing error-envelope payload");
  }
  requireEnvelopeCode(code);
  return { code, message };
}

/**
 * Decode Torii's Norito JSON rendering of an `ErrorEnvelope` whose `details`
 * are absent: exactly `{"code":…,"message":…}` in strict UTF-8, with Norito's
 * string escaping and no insignificant whitespace.
 *
 * @param {Uint8Array} body
 * @returns {{code: string, message: string}}
 */
export function decodeCanonicalJsonErrorEnvelope(body) {
  const envelope = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(body));
  if (envelope === null || typeof envelope !== "object" || Array.isArray(envelope)) {
    throw new Error(NONCANONICAL_JSON_ERROR_V1);
  }
  const keys = Object.keys(envelope);
  const { code, message } = envelope;
  if (
    keys.length !== 2 ||
    keys[0] !== "code" ||
    keys[1] !== "message" ||
    typeof code !== "string" ||
    typeof message !== "string"
  ) {
    throw new Error(NONCANONICAL_JSON_ERROR_V1);
  }
  // `JSON.stringify` escapes strings exactly as Norito JSON does: quote,
  // backslash, the short control escapes and lowercase `\u00xx` for the rest.
  const canonical = new TextEncoder().encode(JSON.stringify({ code, message }));
  if (canonical.length !== body.length) {
    throw new Error(NONCANONICAL_JSON_ERROR_V1);
  }
  for (let index = 0; index < canonical.length; index += 1) {
    if (canonical[index] !== body[index]) {
      throw new Error(NONCANONICAL_JSON_ERROR_V1);
    }
  }
  requireEnvelopeCode(code);
  return { code, message };
}

/**
 * Classify a non-success response body as Torii's canonical error envelope.
 *
 * The declared media type selects the representation as the Rust client does:
 * `application/x-norito` is Norito; an absent type, `application/json` or an
 * `application/*+json` type is JSON. Any other media type, and any body that is
 * not exactly one canonical envelope, returns `null`.
 *
 * @param {string | null | undefined} contentType
 * @param {Uint8Array} body
 * @returns {{code: string, message: string, format: "norito" | "json"} | null}
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
