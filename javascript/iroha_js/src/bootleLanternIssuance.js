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
const AUTHORIZATION_MAGIC_V1 = Uint8Array.of(0x49, 0x4c, 0x41, 0x31);
const BLIND_REQUEST_MAGIC_V1 = Uint8Array.of(0x49, 0x4c, 0x51, 0x31);
const RESPONSE_MAGIC_V1 = Uint8Array.of(0x49, 0x4c, 0x52, 0x31);
const WWW_AUTHENTICATE_VALUE_V1 =
  'Bearer realm="iroha-bootle-lantern-issuance"';
const ERROR_ENVELOPE_TYPE_NAME_V1 = "iroha_torii_shared::ErrorEnvelope";
const ERROR_CONTRACT_V1 = Object.freeze({
  400: Object.freeze({ code: "privacy_issuance_invalid_request", mediaType: BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1 }),
  401: Object.freeze({ code: "privacy_issuance_unauthorized", mediaType: BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1, wwwAuthenticate: WWW_AUTHENTICATE_VALUE_V1 }),
  406: Object.freeze({ code: "privacy_issuance_not_acceptable", mediaType: JSON_MEDIA_TYPE_V1 }),
  409: Object.freeze({ code: "privacy_issuance_state_conflict", mediaType: BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1 }),
  413: Object.freeze({ code: "privacy_issuance_payload_too_large", mediaType: BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1 }),
  415: Object.freeze({ code: "privacy_issuance_unsupported_media_type", mediaType: BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1 }),
  429: Object.freeze({ code: "privacy_issuance_capacity_exhausted", mediaType: BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1, retryAfterSeconds: 1 }),
  503: Object.freeze({ code: "privacy_issuance_unavailable", mediaType: BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1 }),
});

const MAX_ENCODED_CREDENTIAL_BYTES =
  Math.ceil(BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 / 3) * 4;
const BASE64URL_ALPHABET =
  "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const BASE64URL_INDEX = new Int16Array(128).fill(-1);
for (let index = 0; index < BASE64URL_ALPHABET.length; index += 1) {
  BASE64URL_INDEX[BASE64URL_ALPHABET.charCodeAt(index)] = index;
}

const credentialState = new WeakMap();

function requireBytes(value, context) {
  if (!(value instanceof Uint8Array)) {
    throw new TypeError(`${context} must be a Uint8Array`);
  }
  return value;
}

function encodeBase64Url(bytes) {
  let encoded = "";
  for (let offset = 0; offset < bytes.length; offset += 3) {
    const remaining = bytes.length - offset;
    const first = bytes[offset];
    const second = remaining > 1 ? bytes[offset + 1] : 0;
    const third = remaining > 2 ? bytes[offset + 2] : 0;
    encoded += BASE64URL_ALPHABET[first >>> 2];
    encoded += BASE64URL_ALPHABET[((first & 0x03) << 4) | (second >>> 4)];
    if (remaining > 1) {
      encoded += BASE64URL_ALPHABET[((second & 0x0f) << 2) | (third >>> 6)];
    }
    if (remaining > 2) {
      encoded += BASE64URL_ALPHABET[third & 0x3f];
    }
  }
  return encoded;
}

function decodeCanonicalBase64Url(encoded) {
  if (typeof encoded !== "string") {
    throw new TypeError("Bootle/Lantern issuance credential must be a string");
  }
  if (
    encoded.length === 0 ||
    encoded.length > MAX_ENCODED_CREDENTIAL_BYTES ||
    encoded.length % 4 === 1
  ) {
    throw new TypeError(
      "Bootle/Lantern issuance credential must be canonical unpadded base64url",
    );
  }

  const decoded = new Uint8Array(Math.floor((encoded.length * 6) / 8));
  let accumulator = 0;
  let bits = 0;
  let outputOffset = 0;
  for (let index = 0; index < encoded.length; index += 1) {
    const code = encoded.charCodeAt(index);
    const value = code < BASE64URL_INDEX.length ? BASE64URL_INDEX[code] : -1;
    if (value < 0) {
      decoded.fill(0);
      throw new TypeError(
        "Bootle/Lantern issuance credential must be canonical unpadded base64url",
      );
    }
    accumulator = (accumulator << 6) | value;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      decoded[outputOffset] = (accumulator >>> bits) & 0xff;
      outputOffset += 1;
      accumulator &= (1 << bits) - 1;
    }
  }

  if (
    outputOffset !== decoded.length ||
    accumulator !== 0 ||
    decoded.length === 0 ||
    decoded.length > BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1 ||
    encodeBase64Url(decoded) !== encoded
  ) {
    decoded.fill(0);
    throw new TypeError(
      "Bootle/Lantern issuance credential must be canonical unpadded base64url",
    );
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
    const bytes = requireBytes(secret, "Bootle/Lantern issuance credential");
    if (
      bytes.length === 0 ||
      bytes.length > BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1
    ) {
      throw new RangeError(
        `Bootle/Lantern issuance credential must contain 1..${BOOTLE_LANTERN_ISSUANCE_CREDENTIAL_MAX_BYTES_V1} bytes`,
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
    throw new TypeError("Bootle/Lantern issuance credential has been destroyed");
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

function validateBaseUrl(value) {
  let url;
  try {
    url = value instanceof URL ? new URL(value.href) : new URL(value);
  } catch {
    throw new TypeError(
      "Bootle/Lantern issuance requires an absolute HTTPS base URL",
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
      "Bootle/Lantern issuance requires an origin-only HTTPS base URL",
    );
  }
  return url.origin;
}

function headerValues(headers, name) {
  if (!headers) {
    return [];
  }
  if (typeof headers.raw === "function") {
    const raw = headers.raw();
    for (const [candidate, values] of Object.entries(raw)) {
      if (candidate.toLowerCase() === name.toLowerCase()) {
        return Array.isArray(values) ? values.map(String) : [String(values)];
      }
    }
    return [];
  }
  if (typeof headers.getAll === "function") {
    const values = headers.getAll(name);
    return Array.from(values ?? [], String);
  }
  if (typeof headers.get === "function") {
    const value = headers.get(name);
    return value === null || value === undefined ? [] : [String(value)];
  }
  const values = [];
  for (const [candidate, value] of Object.entries(headers)) {
    if (candidate.toLowerCase() === name.toLowerCase()) {
      if (Array.isArray(value)) {
        values.push(...value.map(String));
      } else {
        values.push(String(value));
      }
    }
  }
  return values;
}

function validateResponseHeaders(response, operation, expectedBytes) {
  const contentTypes = headerValues(response.headers, "Content-Type");
  if (
    contentTypes.length !== 1 ||
    contentTypes[0] !== BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1
  ) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} response Content-Type must be exactly ${BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1}`,
    );
  }
  if (headerValues(response.headers, "Content-Encoding").length !== 0) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} response must not contain Content-Encoding`,
    );
  }
  if (headerValues(response.headers, "WWW-Authenticate").length !== 0) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} response contains an unexpected WWW-Authenticate`,
    );
  }
  const lengths = headerValues(response.headers, "Content-Length");
  if (lengths.length === 0) {
    return;
  }
  const value = lengths[0];
  if (
    lengths.length !== 1 ||
    value !== String(expectedBytes)
  ) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} response Content-Length must be canonical and exact`,
    );
  }
}

function validateErrorResponseHeaders(response, contract, bodyBytes, operation) {
  const contentTypes = headerValues(response.headers, "Content-Type");
  if (contentTypes.length !== 1 || contentTypes[0] !== contract.mediaType) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} error response Content-Type is invalid`,
    );
  }
  if (headerValues(response.headers, "Content-Encoding").length !== 0) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} error response must not contain Content-Encoding`,
    );
  }
  const lengths = headerValues(response.headers, "Content-Length");
  if (lengths.length !== 0 && (lengths.length !== 1 || lengths[0] !== String(bodyBytes))) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} error response Content-Length is invalid`,
    );
  }
  const retryAfter = headerValues(response.headers, "Retry-After");
  if (contract.retryAfterSeconds === 1) {
    if (retryAfter.length !== 1 || retryAfter[0] !== "1") {
      throw new BootleLanternIssuanceClientErrorV1(
        `${operation} error response Retry-After is invalid`,
      );
    }
  } else if (retryAfter.length !== 0) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} error response contains an unexpected Retry-After`,
    );
  }
  const wwwAuthenticate = headerValues(response.headers, "WWW-Authenticate");
  if (contract.wwwAuthenticate === WWW_AUTHENTICATE_VALUE_V1) {
    if (
      wwwAuthenticate.length !== 1 ||
      wwwAuthenticate[0] !== WWW_AUTHENTICATE_VALUE_V1
    ) {
      throw new BootleLanternIssuanceClientErrorV1(
        `${operation} error response WWW-Authenticate is invalid`,
      );
    }
  } else if (wwwAuthenticate.length !== 0) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} error response contains an unexpected WWW-Authenticate`,
    );
  }
}

function hasExactMagic(bytes, expectedMagic, offset = 0) {
  return expectedMagic.every(
    (byte, index) => bytes[offset + index] === byte,
  );
}

function requireCanonicalRequestHeaderGuard(target, headers, operation) {
  let probe;
  try {
    probe = new Request(target, { method: "POST", headers });
  } catch {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} transport cannot enforce canonical request headers`,
    );
  }
  for (const [name, expected] of Object.entries(headers)) {
    if (probe.headers.get(name) !== expected) {
      throw new BootleLanternIssuanceClientErrorV1(
        `${operation} transport cannot enforce canonical request headers`,
      );
    }
  }
}

async function readExactBody(response, expectedBytes, operation) {
  const reader = response.body?.getReader?.();
  if (!reader) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} response body is not a bounded byte stream`,
    );
  }
  const result = new Uint8Array(expectedBytes);
  let offset = 0;
  try {
    for (;;) {
      const chunk = await reader.read();
      if (chunk.done) {
        break;
      }
      const bytes = requireBytes(chunk.value, `${operation} response chunk`);
      if (offset + bytes.length > expectedBytes) {
        throw new BootleLanternIssuanceClientErrorV1(
          `${operation} response must be exactly ${expectedBytes} bytes`,
        );
      }
      result.set(bytes, offset);
      offset += bytes.length;
    }
  } catch (error) {
    result.fill(0);
    throw error;
  } finally {
    reader.releaseLock?.();
  }
  if (offset !== expectedBytes) {
    result.fill(0);
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} response must be exactly ${expectedBytes} bytes`,
    );
  }
  return result;
}

async function readBoundedBody(response, maximumBytes, operation) {
  const reader = response.body?.getReader?.();
  if (!reader) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} response body is not a bounded byte stream`,
    );
  }
  const chunks = [];
  let length = 0;
  try {
    for (;;) {
      const chunk = await reader.read();
      if (chunk.done) {
        break;
      }
      const bytes = requireBytes(chunk.value, `${operation} response chunk`);
      if (length + bytes.length > maximumBytes) {
        throw new BootleLanternIssuanceClientErrorV1(
          `${operation} error response exceeds its byte bound`,
        );
      }
      chunks.push(Uint8Array.from(bytes));
      length += bytes.length;
    }
  } finally {
    reader.releaseLock?.();
  }
  if (length === 0) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} error response body is empty`,
    );
  }
  const result = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    result.set(chunk, offset);
    offset += chunk.length;
    chunk.fill(0);
  }
  return result;
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

function readNoritoString(payload, state) {
  const length = readCompactLength(payload, state);
  const end = state.offset + length;
  if (end > payload.length) {
    throw new Error("truncated string");
  }
  const value = new TextDecoder("utf-8", { fatal: true }).decode(
    payload.subarray(state.offset, end),
  );
  state.offset = end;
  return value;
}

function readNoritoField(payload, state) {
  const length = readCompactLength(payload, state);
  const end = state.offset + length;
  if (end > payload.length) {
    throw new Error("truncated error-envelope field");
  }
  const field = payload.subarray(state.offset, end);
  state.offset = end;
  return field;
}

function readNoritoStringField(payload, state) {
  const field = readNoritoField(payload, state);
  const fieldState = { offset: 0 };
  const value = readNoritoString(field, fieldState);
  if (fieldState.offset !== field.length) {
    throw new Error("trailing bytes in error-envelope string field");
  }
  return value;
}

function decodeCanonicalNoritoErrorEnvelope(body) {
  const frame = validateNoritoFrame(body, {
    context: "Bootle/Lantern issuance error envelope",
    expectedTypeName: ERROR_ENVELOPE_TYPE_NAME_V1,
    expectedPaddingLength: 0,
    requireNonEmptyPayload: true,
  });
  if (frame.flags !== 0x02) {
    throw new Error("non-canonical error envelope flags");
  }
  const state = { offset: 0 };
  const code = readNoritoStringField(frame.payload, state);
  const message = readNoritoStringField(frame.payload, state);
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
    throw new Error("non-canonical JSON error envelope");
  }
  for (let index = 0; index < expected.length; index += 1) {
    if (body[index] !== expected[index]) {
      throw new Error("non-canonical JSON error envelope");
    }
  }
  const decoded = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(body));
  return decoded;
}

async function decodeErrorResponse(response, operation) {
  const contract = ERROR_CONTRACT_V1[response.status];
  if (!contract) {
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} returned an unsupported error response`,
    );
  }
  const body = await readBoundedBody(
    response,
    BOOTLE_LANTERN_ISSUANCE_ERROR_RESPONSE_MAX_BYTES_V1,
    operation,
  );
  try {
    validateErrorResponseHeaders(response, contract, body.length, operation);
    const envelope = response.status === 406
      ? decodeCanonicalJsonErrorEnvelope(body, contract.code)
      : decodeCanonicalNoritoErrorEnvelope(body);
    if (
      envelope.code !== contract.code ||
      envelope.message !== contract.code ||
      Object.hasOwn(envelope, "details")
    ) {
      throw new Error("error envelope does not match its HTTP status");
    }
    return new BootleLanternIssuanceClientErrorV1(
      `${operation} returned HTTP ${response.status}: ${contract.code}`,
      {
        status: response.status,
        code: contract.code,
        retryAfterSeconds: contract.retryAfterSeconds ?? null,
      },
    );
  } catch (error) {
    if (error instanceof BootleLanternIssuanceClientErrorV1) {
      throw error;
    }
    throw new BootleLanternIssuanceClientErrorV1(
      `${operation} returned an invalid error response`,
    );
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
    if (typeof fetchImplementation !== "function") {
      throw new TypeError("Bootle/Lantern issuance requires a fetch implementation");
    }
    this.#fetch = fetchImplementation;
  }

  /** Requests one exact 320-byte `ILA1` authorization. */
  async authorize(credential) {
    return this.#executeExact(
      "Bootle/Lantern issuance authorization",
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
      "Content-Type": BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
      Accept: BOOTLE_LANTERN_ISSUANCE_MEDIA_TYPE_V1,
      "Accept-Encoding": "identity",
      "Cache-Control": "no-store",
      Pragma: "no-cache",
    };
    requireCanonicalRequestHeaderGuard(target, headers, operation);
    let response;
    try {
      response = await this.#fetch(target, {
        method: "POST",
        headers,
        body,
        redirect: "manual",
        cache: "no-store",
        credentials: "omit",
      });
    } catch {
      throw new BootleLanternIssuanceClientErrorV1(`${operation} request failed`);
    }

    try {
      if (!response || typeof response.status !== "number") {
        throw new BootleLanternIssuanceClientErrorV1(
          `${operation} response is invalid`,
        );
      }
      if (response.redirected === true || response.type === "opaqueredirect") {
        throw new BootleLanternIssuanceClientErrorV1(
          `${operation} response redirected`,
        );
      }
      if (response.url) {
        let responseUrl;
        try {
          responseUrl = new URL(response.url).href;
        } catch {
          throw new BootleLanternIssuanceClientErrorV1(
            `${operation} response URL is invalid`,
          );
        }
        if (responseUrl !== new URL(target).href) {
          throw new BootleLanternIssuanceClientErrorV1(
            `${operation} response URL does not match the request`,
          );
        }
      }
      if (response.status !== 200) {
        throw await decodeErrorResponse(response, operation);
      }
      validateResponseHeaders(response, operation, expectedBytes);
      const result = await readExactBody(response, expectedBytes, operation);
      if (!hasExactMagic(result, expectedMagic)) {
        result.fill(0);
        throw new BootleLanternIssuanceClientErrorV1(
          `${operation} response wire magic is invalid`,
        );
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
      throw new BootleLanternIssuanceClientErrorV1(
        `${operation} response is invalid`,
      );
    }
  }
}
import { validateNoritoFrame } from "./norito.js";
