import { Buffer } from "buffer";
import { AccountAddress } from "./address.js";
import { createHash, randomBytes } from "./cryptoHash.js";
import { signEd25519 } from "./crypto.js";
import { NetworkId, networkIdBytes } from "./networkId.js";

export { NetworkId };

const DEFAULT_JSON_HEADERS = Object.freeze({
  "Content-Type": "application/json",
  Accept: "application/json",
});

const CANONICAL_REQUEST_NETWORK_DOMAIN = Buffer.from(
  "iroha.app.request.network.v1\0",
  "utf8",
);

/** Maximum decoded non-empty form pairs in a canonical V1 request. */
export const CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 = 64;
/** Maximum UTF-8 bytes in the raw canonical V1 query. */
export const CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 = 64 * 1024;
/** Maximum UTF-8 bytes in the canonical V1 HTTP method token. */
export const CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 = 32;
/** Maximum UTF-8 bytes in the percent-encoded canonical V1 path. */
export const CANONICAL_REQUEST_MAX_PATH_BYTES_V1 = 64 * 1024;
/** Maximum UTF-8 bytes in a canonical V1 account identity or alias. */
export const CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 = 36 * 1024;

function compareUtf8(left, right) {
  if (left === right) {
    return 0;
  }
  const a = Buffer.from(String(left), "utf8");
  const b = Buffer.from(String(right), "utf8");
  const min = Math.min(a.length, b.length);
  for (let index = 0; index < min; index += 1) {
    const diff = a[index] - b[index];
    if (diff !== 0) {
      return diff;
    }
  }
  return a.length - b.length;
}

function requireExactNonBlankString(value, field, context) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new Error(`${field} is required for ${context}`);
  }
  if (value.trim() !== value) {
    throw new Error(`${field} must not contain surrounding whitespace for ${context}`);
  }
  if (
    field === "nonce" &&
    (Buffer.byteLength(value, "utf8") > 256 ||
      !Array.from(value).every((character) => {
        const code = character.codePointAt(0);
        return code >= 0x21 && code <= 0x7e;
      }))
  ) {
    throw new Error(`nonce must contain 1...256 non-whitespace ASCII bytes for ${context}`);
  }
  return value;
}

const CANONICAL_AUTH_ACCOUNT_ALIAS_PATTERN =
  /^[a-z0-9]+(?:[._-][a-z0-9]+)*@[a-z0-9]+(?:-[a-z0-9]+)*(?:\.[a-z0-9]+(?:-[a-z0-9]+)*)?$/u;

export function requireCanonicalAuthAccount(value, context) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new TypeError(
      `${context} must be an exact canonical I105 account or ASCII account alias`,
    );
  }
  if (Buffer.byteLength(value, "utf8") > CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1) {
    throw new TypeError(
      `${context} exceeds ${CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1} UTF-8 bytes`,
    );
  }
  if (CANONICAL_AUTH_ACCOUNT_ALIAS_PATTERN.test(value)) {
    return value;
  }
  try {
    const parsed = AccountAddress.parseEncoded(value);
    if (parsed.address.toI105(parsed.chainDiscriminant) === value) {
      return value;
    }
  } catch {
    // Use the single stable diagnostic below for every malformed identifier.
  }
  throw new TypeError(
    `${context} must be an exact canonical I105 account or ASCII account alias`,
  );
}

function canonicalAuthAccountHeaderValue(accountId) {
  if (CANONICAL_AUTH_ACCOUNT_ALIAS_PATTERN.test(accountId)) {
    return accountId;
  }
  return AccountAddress.parseEncoded(accountId).address.canonicalHex();
}

/**
 * Canonicalise a raw query string by decoding, sorting, and re-encoding.
 * @param {string | URLSearchParams | undefined | null} raw
 * @returns {string}
 */
export function canonicalQueryString(raw) {
  if (raw === undefined || raw === null) {
    return "";
  }
  const rawText = raw instanceof URLSearchParams ? raw.toString() : String(raw);
  const rawBytes = Buffer.byteLength(rawText, "utf8");
  if (rawBytes > CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1) {
    throw new RangeError(
      `canonical request query exceeds ${CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1} raw UTF-8 bytes`,
    );
  }
  const params = new URLSearchParams(rawText);
  const pairs = Array.from(params.entries()).map(([k, v]) => [k, v]);
  if (pairs.length > CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1) {
    throw new RangeError(
      `canonical request query exceeds ${CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1} pairs`,
    );
  }
  pairs.sort((a, b) => {
    const keyOrder = compareUtf8(a[0], b[0]);
    if (keyOrder !== 0) {
      return keyOrder;
    }
    return compareUtf8(a[1], b[1]);
  });
  const serializer = new URLSearchParams();
  for (const [key, value] of pairs) {
    serializer.append(key, value);
  }
  return serializer.toString();
}

/**
 * Build canonical request bytes for signing.
 * @param {{method: string, path: string, query?: string | URLSearchParams, body?: Buffer | ArrayBuffer | ArrayBufferView | string}} params
 * @returns {Buffer}
 */
export function canonicalRequestMessage({ method, path, query, body }) {
  const methodText = String(method ?? "");
  const pathText = String(path);
  if (Buffer.byteLength(methodText, "utf8") > CANONICAL_REQUEST_MAX_METHOD_BYTES_V1) {
    throw new RangeError(
      `canonical request method exceeds ${CANONICAL_REQUEST_MAX_METHOD_BYTES_V1} UTF-8 bytes`,
    );
  }
  if (Buffer.byteLength(pathText, "utf8") > CANONICAL_REQUEST_MAX_PATH_BYTES_V1) {
    throw new RangeError(
      `canonical request path exceeds ${CANONICAL_REQUEST_MAX_PATH_BYTES_V1} UTF-8 bytes`,
    );
  }
  const upperMethod = methodText.toUpperCase();
  const canonicalQuery = canonicalQueryString(query);
  const bodyBuffer = body === undefined ? Buffer.alloc(0) : Buffer.from(body);
  const bodyHash = createHash("sha256").update(bodyBuffer).digest("hex");
  const rendered = `${upperMethod}\n${pathText}\n${canonicalQuery}\n${bodyHash}`;
  return Buffer.from(rendered, "utf8");
}

/**
 * Build canonical request bytes for signature verification, bound to one exact
 * genesis-derived network and freshness metadata.
 * @param {{networkId: import("./networkId.js").NetworkId, method: string, path: string, query?: string | URLSearchParams, body?: Buffer | ArrayBuffer | ArrayBufferView | string, timestampMs: number, nonce: string}} params
 * @returns {Buffer}
 */
export function canonicalRequestSignatureMessage({
  networkId,
  method,
  path,
  query,
  body,
  timestampMs,
  nonce,
}) {
  if (!Number.isSafeInteger(timestampMs) || timestampMs < 0) {
    throw new RangeError("timestampMs must be a non-negative safe integer");
  }
  const checkedNonce = requireExactNonBlankString(
    nonce,
    "nonce",
    "canonical exact-network signatures",
  );
  const network = Buffer.from(networkIdBytes(networkId, "networkId"));
  const base = canonicalRequestMessage({ method, path, query, body });
  return Buffer.concat([
    CANONICAL_REQUEST_NETWORK_DOMAIN,
    network,
    base,
    Buffer.from(`\n${String(timestampMs)}\n${checkedNonce}`, "utf8"),
  ]);
}

/**
 * Build canonical signing headers for app-facing Torii endpoints.
 * `accountId` is an exact canonical I105 account or active canonical ASCII
 * alias. I105 is rendered as lowercase canonical hex in `X-Iroha-Account`;
 * ASCII aliases are carried unchanged.
 * @param {{accountId: string, networkId: import("./networkId.js").NetworkId, method: string, path: string, query?: string | URLSearchParams, body?: Buffer | ArrayBuffer | ArrayBufferView | string, privateKey: Buffer | ArrayBuffer | ArrayBufferView, timestampMs?: number, nonce?: string}} params
 * @returns {{ "X-Iroha-Account": string, "X-Iroha-Signature": string, "X-Iroha-Timestamp-Ms": string, "X-Iroha-Nonce": string }}
 */
export function buildCanonicalRequestHeaders({
  accountId,
  networkId,
  method,
  path,
  query,
  body,
  privateKey,
  timestampMs = Date.now(),
  nonce = randomBytes(16).toString("hex"),
}) {
  const checkedAccount = requireCanonicalAuthAccount(
    accountId,
    "accountId",
  );
  if (!privateKey) {
    throw new Error("privateKey is required for canonical headers");
  }
  const checkedNonce = requireExactNonBlankString(nonce, "nonce", "canonical headers");
  const signatureInput = {
    method,
    path,
    query,
    body,
    timestampMs,
    nonce: checkedNonce,
  };
  const message = canonicalRequestSignatureMessage({ networkId, ...signatureInput });
  const signature = signEd25519(message, privateKey);
  return {
    "X-Iroha-Account": canonicalAuthAccountHeaderValue(checkedAccount),
    "X-Iroha-Signature": Buffer.from(signature).toString("base64"),
    "X-Iroha-Timestamp-Ms": String(timestampMs),
    "X-Iroha-Nonce": checkedNonce,
  };
}

function normalizeHeadersInit(headers) {
  if (headers === undefined || headers === null) {
    return {};
  }
  const normalized = {};
  if (typeof Headers !== "undefined" && headers instanceof Headers) {
    for (const [key, value] of headers.entries()) {
      normalized[key] = value;
    }
    return normalized;
  }
  if (Array.isArray(headers)) {
    for (const [key, value] of headers) {
      normalized[String(key)] = String(value);
    }
    return normalized;
  }
  if (typeof headers[Symbol.iterator] === "function") {
    for (const [key, value] of headers) {
      normalized[String(key)] = String(value);
    }
    return normalized;
  }
  if (typeof headers === "object") {
    for (const [key, value] of Object.entries(headers)) {
      if (value !== undefined && value !== null) {
        normalized[key] = String(value);
      }
    }
    return normalized;
  }
  throw new Error("headers must be a Headers, iterable, or plain object");
}

function normalizeSignatureBase64(signature, context = "signature") {
  if (typeof signature === "string") {
    const trimmed = signature.trim();
    if (!trimmed) {
      throw new Error(`${context} must not be empty`);
    }
    return trimmed;
  }
  if (signature === undefined || signature === null) {
    throw new Error(`${context} must be returned by the canonical request signer`);
  }
  return Buffer.from(signature).toString("base64");
}

function splitPathAndQuery(path, query) {
  const pathText = String(path);
  const queryIndex = pathText.indexOf("?");
  if (query !== undefined && query !== null) {
    return {
      path: queryIndex < 0 ? pathText : pathText.slice(0, queryIndex),
      query,
    };
  }
  if (queryIndex < 0) {
    return { path: pathText, query: undefined };
  }
  return {
    path: pathText.slice(0, queryIndex),
    query: pathText.slice(queryIndex + 1),
  };
}

function canonicalTargetFromPath({ path, query, baseUrl }) {
  const absoluteUrlPattern = /^[a-z][a-z0-9+.-]*:\/\//i;
  if (absoluteUrlPattern.test(String(path))) {
    const url = new URL(String(path));
    return splitPathAndQuery(url.pathname + url.search, query);
  }

  const target = splitPathAndQuery(path, query);
  if (!baseUrl) {
    return target;
  }

  const base = new URL(String(baseUrl));
  const basePath = base.pathname.replace(/\/+$/, "");
  const requestPath = String(target.path || "").replace(/^\/+/, "");
  const joinedPath = [basePath, requestPath].filter(Boolean).join("/");
  return {
    path: joinedPath ? `/${joinedPath}`.replace(/\/{2,}/g, "/") : "/",
    query: target.query,
  };
}

/**
 * Build fetch-compatible JSON request options signed with Torii canonical auth.
 *
 * The returned body string is the exact JSON payload covered by the signature.
 * Callers with private key bytes can pass `privateKey`; browser keystores can
 * pass an async `sign` callback and keep private keys out of application code.
 * `accountId` must be the exact canonical I105 account or active canonical
 * ASCII alias used as the auth header.
 *
 * @param {{accountId: string, networkId: import("./networkId.js").NetworkId, method?: string, path: string, baseUrl?: string, query?: string | URLSearchParams, body?: unknown, headers?: Headers | Array<[string, string]> | Record<string, string>, privateKey?: Buffer | ArrayBuffer | ArrayBufferView, sign?: (input: {message: Buffer, messageBase64: string, networkId: import("./networkId.js").NetworkId, method: string, path: string, query?: string | URLSearchParams, body: string, timestampMs: number, nonce: string}) => Promise<Buffer | ArrayBuffer | ArrayBufferView | string> | Buffer | ArrayBuffer | ArrayBufferView | string, timestampMs?: number, nonce?: string}} params
 * @returns {Promise<{method: string, headers: Record<string, string>, body: string}>}
 */
export async function buildCanonicalJsonRequest({
  accountId,
  networkId,
  method = "POST",
  path,
  baseUrl,
  query,
  body,
  headers,
  privateKey,
  sign,
  timestampMs = Date.now(),
  nonce = randomBytes(16).toString("hex"),
}) {
  const checkedAccount = requireCanonicalAuthAccount(
    accountId,
    "accountId",
  );
  if (!path) {
    throw new Error("path is required for canonical JSON requests");
  }
  const checkedNonce = requireExactNonBlankString(
    nonce,
    "nonce",
    "canonical JSON requests",
  );
  if (!privateKey && typeof sign !== "function") {
    throw new Error("privateKey or sign is required for canonical JSON requests");
  }
  const methodUpper = String(method).toUpperCase();
  const canonicalTarget = canonicalTargetFromPath({ path, query, baseUrl });
  const bodyJson = body === undefined ? "" : JSON.stringify(body);
  const message = canonicalRequestSignatureMessage({
    networkId,
    method: methodUpper,
    path: canonicalTarget.path,
    query: canonicalTarget.query,
    body: bodyJson,
    timestampMs,
    nonce: checkedNonce,
  });
  const signatureBase64 = privateKey
    ? Buffer.from(signEd25519(message, privateKey)).toString("base64")
    : normalizeSignatureBase64(
        await sign({
          message,
          messageBase64: message.toString("base64"),
          networkId,
          method: methodUpper,
          path: canonicalTarget.path,
          query: canonicalTarget.query,
          body: bodyJson,
          timestampMs,
          nonce: checkedNonce,
        }),
      );
  return {
    method: methodUpper,
    headers: {
      ...DEFAULT_JSON_HEADERS,
      ...normalizeHeadersInit(headers),
      "X-Iroha-Account": canonicalAuthAccountHeaderValue(checkedAccount),
      "X-Iroha-Signature": signatureBase64,
      "X-Iroha-Timestamp-Ms": String(timestampMs),
      "X-Iroha-Nonce": checkedNonce,
    },
    body: bodyJson,
  };
}
