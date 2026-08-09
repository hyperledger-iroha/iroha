/** Canonical Bootle/Lantern authorization route. */
export const BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1 =
  "/v1/privacy/bootle-lantern/issuance/authorize";

/** Canonical Bootle/Lantern blind-issuance route. */
export const BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1 =
  "/v1/privacy/bootle-lantern/issuance/issue";

/** Sole request and successful-response media type. */
export const BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1 = "application/x-norito";

/** Exact encoded authorization response length. */
export const BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1 = 320;

/** Exact encoded `ILA1 || ILQ1` request length. */
export const BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1 = 71_896;

/** Exact encoded `ILR1` response length. */
export const BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1 = 3_176;

/** Maximum decoded bearer credential length accepted by Torii. */
export const BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 = 4_096;

/** Maximum accepted structured issuance-error body length. */
export const BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1 = 512;

const JSON_MEDIA_TYPE_V1 = "application/json";
const AUTHORIZATION_MAGIC_V1 = "ILA1";
const BLIND_REQUEST_MAGIC_V1 = "ILQ1";
const RESPONSE_MAGIC_V1 = "ILR1";
const WWW_AUTHENTICATE_VALUE_V1 =
  'Bearer realm="iroha-bootle-lantern-issuance"';
const ERROR_ENVELOPE_TYPE_NAME_V1 = "iroha_torii_shared::ErrorEnvelope";
const ISSUANCE_CONTEXT_V1 = "Bootle/Lantern issuance";
const CREDENTIAL_CONTEXT_V1 = `${ISSUANCE_CONTEXT_V1} credential`;
const CANONICAL_CREDENTIAL_ERROR_V1 =
  `${CREDENTIAL_CONTEXT_V1} must be canonical unpadded base64url`;
const REQUEST_HEADER_GUARD_ERROR_V1 =
  "transport cannot enforce canonical request headers";
const NONCANONICAL_JSON_ERROR_V1 = "non-canonical JSON error envelope";
const INVALID_RESPONSE_ERROR_V1 = "response is invalid";
const CONTENT_TYPE_HEADER_V1 = "Content-Type";
const WWW_AUTHENTICATE_HEADER_V1 = "WWW-Authenticate";
const BASE64URL_ENCODING_V1 = "base64url";
const NO_STORE_V1 = "no-store";
const POST_METHOD_V1 = "POST";
const FUNCTION_TYPE = "function";
const ERROR_CODE_BY_STATUS_V1 = Object.freeze({
  400: "privacy_issuance_invalid_request",
  401: "privacy_issuance_unauthorized",
  406: "privacy_issuance_not_acceptable",
  409: "privacy_issuance_state_conflict",
  413: "privacy_issuance_payload_too_large",
  415: "privacy_issuance_unsupported_media_type",
  429: "privacy_issuance_capacity_exhausted",
  503: "privacy_issuance_unavailable",
});

const MAX_ENCODED_CREDENTIAL_BYTES =
  Math.ceil(BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 / 3) * 4;
const credentialState = new WeakMap();

function requireBytes(value, context) {
  if (!(value instanceof Uint8Array)) {
    throw new TypeError(`${context} must be a Uint8Array`);
  }
  return value;
}

function encodeBase64Url(bytes) {
  return Buffer.from(bytes.buffer, bytes.byteOffset, bytes.byteLength).toString(
    BASE64URL_ENCODING_V1,
  );
}

function decodeCanonicalBase64Url(encoded) {
  if (typeof encoded !== "string") {
    throw new TypeError(`${CREDENTIAL_CONTEXT_V1} must be a string`);
  }
  if (
    encoded.length === 0 ||
    encoded.length > MAX_ENCODED_CREDENTIAL_BYTES ||
    encoded.length % 4 === 1
  ) {
    throw new TypeError(CANONICAL_CREDENTIAL_ERROR_V1);
  }

  const decoded = Buffer.from(encoded, BASE64URL_ENCODING_V1);

  if (
    decoded.length === 0 ||
    decoded.length > BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 ||
    encodeBase64Url(decoded) !== encoded
  ) {
    decoded.fill(0);
    throw new TypeError(CANONICAL_CREDENTIAL_ERROR_V1);
  }
  return decoded;
}

/**
 * Opaque, bounded Bootle/Lantern issuer credential.
 *
 * The source is defensively copied. Call {@link destroy} when the credential is
 * no longer needed to overwrite its retained byte buffer.
 */
export class BootleLanternIssuanceCredentialV1 {
  constructor(secret) {
    const bytes = requireBytes(secret, CREDENTIAL_CONTEXT_V1);
    if (
      bytes.length === 0 ||
      bytes.length > BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1
    ) {
      throw new RangeError(
        `${CREDENTIAL_CONTEXT_V1} must contain 1..${BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1} bytes`,
      );
    }
    credentialState.set(this, { bytes: Uint8Array.from(bytes), destroyed: false });
    Object.freeze(this);
  }

  /** Copies and validates opaque credential bytes. */
  static fromOpaqueBytes(secret) {
    return new BootleLanternIssuanceCredentialV1(secret);
  }

  /** Decodes one canonical, unpadded base64url credential without a prefix. */
  static fromCanonicalBase64Url(encoded) {
    const decoded = decodeCanonicalBase64Url(encoded);
    try {
      return new BootleLanternIssuanceCredentialV1(decoded);
    } finally {
      decoded.fill(0);
    }
  }

  /** Overwrites the retained credential buffer. This operation is idempotent. */
  destroy() {
    const state = credentialState.get(this);
    if (state && !state.destroyed) {
      state.bytes.fill(0);
      state.destroyed = true;
    }
  }

  /** Returns a deliberately redacted diagnostic representation. */
  toString() {
    return "BootleLanternIssuanceCredentialV1([REDACTED])";
  }
}

function authorizationHeaderValue(credential) {
  if (!(credential instanceof BootleLanternIssuanceCredentialV1)) {
    throw new TypeError(
      "credential must be a BootleLanternIssuanceCredentialV1",
    );
  }
  const state = credentialState.get(credential);
  if (!state || state.destroyed) {
    throw new TypeError(`${CREDENTIAL_CONTEXT_V1} has been destroyed`);
  }
  return `Bearer ${encodeBase64Url(state.bytes)}`;
}

/** Fail-closed transport or response validation failure. */
export class BootleLanternIssuanceClientErrorV1 extends Error {
  constructor(message, { status = null, code = null, retryAfterSeconds = null } = {}) {
    super(message);
    this.name = "BootleLanternIssuanceClientErrorV1";
    this.status = status;
    this.code = code;
    this.retryAfterSeconds = retryAfterSeconds;
  }
}

function clientError(operation, detail, options) {
  return new BootleLanternIssuanceClientErrorV1(
    `${operation} ${detail}`,
    options,
  );
}

function validateBaseUrl(value) {
  let url;
  try {
    url = value instanceof URL ? new URL(value.href) : new URL(value);
  } catch {
    throw new TypeError(
      `${ISSUANCE_CONTEXT_V1} requires an absolute HTTPS base URL`,
    );
  }
  if (
    url.protocol !== "https:" ||
    url.hostname.length === 0 ||
    url.username.length !== 0 ||
    url.password.length !== 0 ||
    url.search.length !== 0 ||
    url.hash.length !== 0 ||
    url.href.includes("?") ||
    url.href.includes("#") ||
    (url.pathname !== "" && url.pathname !== "/")
  ) {
    throw new TypeError(
      `${ISSUANCE_CONTEXT_V1} requires an origin-only HTTPS base URL`,
    );
  }
  return url.origin;
}

function headerValues(headers, name) {
  if (!headers) {
    return [];
  }
  if (typeof headers.getAll === FUNCTION_TYPE) {
    const values = headers.getAll(name);
    return Array.from(values ?? [], String);
  }
  if (typeof headers.get === FUNCTION_TYPE) {
    const value = headers.get(name);
    return value === null || value === undefined ? [] : [String(value)];
  }
  const record = typeof headers.raw === FUNCTION_TYPE ? headers.raw() : headers;
  const normalizedName = name.toLowerCase();
  const values = [];
  for (const [candidate, value] of Object.entries(record)) {
    if (candidate.toLowerCase() === normalizedName) {
      if (Array.isArray(value)) {
        values.push(...value.map(String));
      } else {
        values.push(String(value));
      }
    }
  }
  return values;
}

function validateResponseHeaders(response, operation, expectedBytes, error = false) {
  const status = response.status;
  const mediaType =
    error && status === 406
      ? JSON_MEDIA_TYPE_V1
      : BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1;
  const contentTypes = headerValues(response.headers, CONTENT_TYPE_HEADER_V1);
  if (contentTypes.length !== 1 || contentTypes[0] !== mediaType) {
    throw clientError(
      operation,
      error
        ? "error response Content-Type is invalid"
        : `response Content-Type must be exactly ${mediaType}`,
    );
  }
  if (headerValues(response.headers, "Content-Encoding").length !== 0) {
    throw clientError(
      operation,
      `${error ? "error response" : "response"} must not contain Content-Encoding`,
    );
  }
  if (!error) {
    if (headerValues(response.headers, WWW_AUTHENTICATE_HEADER_V1).length !== 0) {
      throw clientError(
        operation,
        "response contains an unexpected WWW-Authenticate",
      );
    }
  }
  const lengths = headerValues(response.headers, "Content-Length");
  if (
    lengths.length !== 0 &&
    (lengths.length !== 1 || lengths[0] !== String(expectedBytes))
  ) {
    throw clientError(
      operation,
      error
        ? "error response Content-Length is invalid"
        : "response Content-Length must be canonical and exact",
    );
  }
  if (!error) {
    return;
  }
  const retryAfter = headerValues(response.headers, "Retry-After");
  if (status === 429) {
    if (retryAfter.length !== 1 || retryAfter[0] !== "1") {
      throw clientError(operation, "error response Retry-After is invalid");
    }
  } else if (retryAfter.length !== 0) {
    throw clientError(
      operation,
      "error response contains an unexpected Retry-After",
    );
  }
  const wwwAuthenticate = headerValues(response.headers, WWW_AUTHENTICATE_HEADER_V1);
  if (status === 401) {
    if (
      wwwAuthenticate.length !== 1 ||
      wwwAuthenticate[0] !== WWW_AUTHENTICATE_VALUE_V1
    ) {
      throw clientError(
        operation,
        "error response WWW-Authenticate is invalid",
      );
    }
  } else if (wwwAuthenticate.length !== 0) {
    throw clientError(
      operation,
      "error response contains an unexpected WWW-Authenticate",
    );
  }
}

function hasExactMagic(bytes, expectedMagic, offset = 0) {
  for (let index = 0; index < 4; index += 1) {
    if (bytes[offset + index] !== expectedMagic.charCodeAt(index)) {
      return false;
    }
  }
  return true;
}

function requireCanonicalRequestHeaderGuard(target, headers, operation) {
  let probe;
  try {
    probe = new Request(target, { method: POST_METHOD_V1, headers });
  } catch {
    throw clientError(
      operation,
      REQUEST_HEADER_GUARD_ERROR_V1,
    );
  }
  for (const [name, expected] of Object.entries(headers)) {
    if (probe.headers.get(name) !== expected) {
      throw clientError(
        operation,
        REQUEST_HEADER_GUARD_ERROR_V1,
      );
    }
  }
}

async function readResponseBody(response, byteLimit, operation, exact) {
  const reader = response.body?.getReader?.();
  if (!reader) {
    throw clientError(
      operation,
      "response body is not a bounded byte stream",
    );
  }
  const result = new Uint8Array(byteLimit);
  let offset = 0;
  try {
    for (;;) {
      const chunk = await reader.read();
      if (chunk.done) {
        break;
      }
      const bytes = requireBytes(chunk.value, `${operation} response chunk`);
      if (offset + bytes.length > byteLimit) {
        throw clientError(
          operation,
          exact
            ? `response must be exactly ${byteLimit} bytes`
            : "error response exceeds its byte bound",
        );
      }
      result.set(bytes, offset);
      offset += bytes.length;
    }
    if (exact ? offset !== byteLimit : offset === 0) {
      const detail = exact
        ? `response must be exactly ${byteLimit} bytes`
        : "error response body is empty";
      throw clientError(operation, detail);
    }
    if (exact) {
      return result;
    }
    const bounded = result.slice(0, offset);
    result.fill(0);
    return bounded;
  } catch (error) {
    result.fill(0);
    throw error;
  } finally {
    reader.releaseLock?.();
  }
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

function decodeCanonicalNoritoErrorEnvelope(body) {
  const frame = validateNoritoFrame(body, {
    context: `${ISSUANCE_CONTEXT_V1} error envelope`,
    expectedTypeName: ERROR_ENVELOPE_TYPE_NAME_V1,
    expectedPaddingLength: 0,
    requireNonEmptyPayload: true,
  });
  if (frame.flags !== 0x02) {
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
  return { code, message };
}

function decodeCanonicalJsonErrorEnvelope(body, expectedCode) {
  const expected = new TextEncoder().encode(
    `{"code":"${expectedCode}","message":"${expectedCode}"}`,
  );
  if (body.length !== expected.length) {
    throw new Error(NONCANONICAL_JSON_ERROR_V1);
  }
  for (let index = 0; index < expected.length; index += 1) {
    if (body[index] !== expected[index]) {
      throw new Error(NONCANONICAL_JSON_ERROR_V1);
    }
  }
  return { code: expectedCode, message: expectedCode };
}

async function decodeErrorResponse(response, operation) {
  const code = ERROR_CODE_BY_STATUS_V1[response.status];
  if (!code) {
    throw clientError(operation, "returned an unsupported error response");
  }
  const body = await readResponseBody(
    response,
    BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1,
    operation,
    false,
  );
  try {
    validateResponseHeaders(response, operation, body.length, true);
    const envelope = response.status === 406
      ? decodeCanonicalJsonErrorEnvelope(body, code)
      : decodeCanonicalNoritoErrorEnvelope(body);
    if (
      envelope.code !== code ||
      envelope.message !== code ||
      Object.hasOwn(envelope, "details")
    ) {
      throw new Error("error envelope does not match its HTTP status");
    }
    return clientError(
      operation,
      `returned HTTP ${response.status}: ${code}`,
      {
        status: response.status,
        code,
        retryAfterSeconds: response.status === 429 ? 1 : null,
      },
    );
  } catch (error) {
    if (error instanceof BootleLanternIssuanceClientErrorV1) {
      throw error;
    }
    throw clientError(operation, "returned an invalid error response");
  } finally {
    body.fill(0);
  }
}

/**
 * Exact, single-attempt client for first-release native Bootle/Lantern issuance.
 */
export class BootleLanternIssuanceClientV1 {
  #baseUrl;

  #fetch;

  constructor({ baseUrl, fetch: fetchImplementation = globalThis.fetch } = {}) {
    this.#baseUrl = validateBaseUrl(baseUrl);
    if (typeof fetchImplementation !== FUNCTION_TYPE) {
      throw new TypeError(`${ISSUANCE_CONTEXT_V1} requires a fetch implementation`);
    }
    this.#fetch = fetchImplementation;
  }

  /** Requests one exact 320-byte `ILA1` authorization. */
  async authorize(credential) {
    return this.#executeExact(
      `${ISSUANCE_CONTEXT_V1} authorization`,
      BOOTLE_LANTERN_ISSUANCE_AUTHORIZE_PATH_V1,
      credential,
      new Uint8Array(0),
      BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1,
      AUTHORIZATION_MAGIC_V1,
    );
  }

  /** Submits exact `ILA1 || ILQ1` and returns an exact `ILR1` response. */
  async issue(credential, canonicalRequest) {
    const body = requireBytes(
      canonicalRequest,
      "Bootle/Lantern issue request",
    );
    if (body.length !== BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1) {
      throw new RangeError(
        `Bootle/Lantern issue request must be exactly ${BOOTLE_LANTERN_ISSUE_REQUEST_BYTES_V1} bytes`,
      );
    }
    if (
      !hasExactMagic(body, AUTHORIZATION_MAGIC_V1) ||
      !hasExactMagic(
        body,
        BLIND_REQUEST_MAGIC_V1,
        BOOTLE_LANTERN_AUTHORIZATION_BYTES_V1,
      )
    ) {
      throw new RangeError(
        "Bootle/Lantern issue request must contain canonical ILA1 || ILQ1 magics",
      );
    }
    return this.#executeExact(
      "Bootle/Lantern blind issuance",
      BOOTLE_LANTERN_ISSUANCE_ISSUE_PATH_V1,
      credential,
      Uint8Array.from(body),
      BOOTLE_LANTERN_ISSUE_RESPONSE_BYTES_V1,
      RESPONSE_MAGIC_V1,
    );
  }

  async #executeExact(
    operation,
    path,
    credential,
    body,
    expectedBytes,
    expectedMagic,
  ) {
    const authorization = authorizationHeaderValue(credential);
    const target = `${this.#baseUrl}${path}`;
    const headers = {
      Authorization: authorization,
      [CONTENT_TYPE_HEADER_V1]: BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
      Accept: BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
      "Accept-Encoding": "identity",
      "Cache-Control": NO_STORE_V1,
      Pragma: "no-cache",
    };
    requireCanonicalRequestHeaderGuard(target, headers, operation);
    let response;
    try {
      response = await this.#fetch(target, {
        method: POST_METHOD_V1,
        headers,
        body,
        redirect: "manual",
        cache: NO_STORE_V1,
        credentials: "omit",
      });
    } catch {
      throw clientError(operation, "request failed");
    }

    try {
      if (!response || typeof response.status !== "number") {
        throw clientError(operation, INVALID_RESPONSE_ERROR_V1);
      }
      if (response.redirected === true || response.type === "opaqueredirect") {
        throw clientError(operation, "response redirected");
      }
      if (response.url) {
        let responseUrl;
        try {
          responseUrl = new URL(response.url).href;
        } catch {
          throw clientError(operation, "response URL is invalid");
        }
        if (responseUrl !== new URL(target).href) {
          throw clientError(
            operation,
            "response URL does not match the request",
          );
        }
      }
      if (response.status !== 200) {
        throw await decodeErrorResponse(response, operation);
      }
      validateResponseHeaders(response, operation, expectedBytes);
      const result = await readResponseBody(
        response,
        expectedBytes,
        operation,
        true,
      );
      if (!hasExactMagic(result, expectedMagic)) {
        result.fill(0);
        throw clientError(operation, "response wire magic is invalid");
      }
      return result;
    } catch (error) {
      try {
        await response?.body?.cancel?.();
      } catch {
        // Preserve the canonical validation error and discard transport details.
      }
      if (error instanceof BootleLanternIssuanceClientErrorV1) {
        throw error;
      }
      throw clientError(operation, INVALID_RESPONSE_ERROR_V1);
    }
  }
}
import { Buffer } from "buffer";

import { validateNoritoFrame } from "./norito.js";
