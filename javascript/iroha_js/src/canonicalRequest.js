import { Buffer } from "node:buffer";
import { createHash, randomBytes } from "node:crypto";
import { signEd25519 } from "./crypto.js";

const DEFAULT_JSON_HEADERS = Object.freeze({
  "Content-Type": "application/json",
  Accept: "application/json",
});

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

/**
 * Canonicalise a raw query string by decoding, sorting, and re-encoding.
 * @param {string | URLSearchParams | undefined | null} raw
 * @returns {string}
 */
export function canonicalQueryString(raw) {
  if (raw === undefined || raw === null) {
    return "";
  }
  const params = raw instanceof URLSearchParams ? raw : new URLSearchParams(String(raw));
  const pairs = Array.from(params.entries()).map(([k, v]) => [k, v]);
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
  const upperMethod = String(method ?? "").toUpperCase();
  const canonicalQuery = canonicalQueryString(query);
  const bodyBuffer = body === undefined ? Buffer.alloc(0) : Buffer.from(body);
  const bodyHash = createHash("sha256").update(bodyBuffer).digest("hex");
  const rendered = `${upperMethod}\n${path}\n${canonicalQuery}\n${bodyHash}`;
  return Buffer.from(rendered, "utf8");
}

/**
 * Build canonical request bytes for signature verification with freshness metadata.
 * @param {{method: string, path: string, query?: string | URLSearchParams, body?: Buffer | ArrayBuffer | ArrayBufferView | string, timestampMs: number, nonce: string}} params
 * @returns {Buffer}
 */
export function canonicalRequestSignatureMessage({
  method,
  path,
  query,
  body,
  timestampMs,
  nonce,
}) {
  const base = canonicalRequestMessage({ method, path, query, body });
  return Buffer.from(
    `${base.toString("utf8")}\n${String(timestampMs)}\n${String(nonce)}`,
    "utf8",
  );
}

/**
 * Build canonical signing headers for app-facing Torii endpoints.
 * @param {{accountId: string, method: string, path: string, query?: string | URLSearchParams, body?: Buffer | ArrayBuffer | ArrayBufferView | string, privateKey: Buffer | ArrayBuffer | ArrayBufferView, timestampMs?: number, nonce?: string}} params
 * @returns {{ "X-Iroha-Account": string, "X-Iroha-Signature": string, "X-Iroha-Timestamp-Ms": string, "X-Iroha-Nonce": string }}
 */
export function buildCanonicalRequestHeaders({
  accountId,
  method,
  path,
  query,
  body,
  privateKey,
  timestampMs = Date.now(),
  nonce = randomBytes(16).toString("hex"),
}) {
  if (!accountId) {
    throw new Error("accountId is required for canonical headers");
  }
  if (!privateKey) {
    throw new Error("privateKey is required for canonical headers");
  }
  if (!Number.isFinite(timestampMs)) {
    throw new Error("timestampMs must be a finite number");
  }
  if (!nonce || typeof nonce !== "string") {
    throw new Error("nonce is required for canonical headers");
  }
  const normalizedTimestampMs = Math.trunc(timestampMs);
  const message = canonicalRequestSignatureMessage({
    method,
    path,
    query,
    body,
    timestampMs: normalizedTimestampMs,
    nonce,
  });
  const signature = signEd25519(message, privateKey);
  return {
    "X-Iroha-Account": String(accountId),
    "X-Iroha-Signature": Buffer.from(signature).toString("base64"),
    "X-Iroha-Timestamp-Ms": String(normalizedTimestampMs),
    "X-Iroha-Nonce": nonce,
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
 *
 * @param {{accountId: string, method?: string, path: string, baseUrl?: string, query?: string | URLSearchParams, body?: unknown, headers?: Headers | Array<[string, string]> | Record<string, string>, privateKey?: Buffer | ArrayBuffer | ArrayBufferView, sign?: (input: {message: Buffer, messageBase64: string, method: string, path: string, query?: string | URLSearchParams, body: string, timestampMs: number, nonce: string}) => Promise<Buffer | ArrayBuffer | ArrayBufferView | string> | Buffer | ArrayBuffer | ArrayBufferView | string, timestampMs?: number, nonce?: string}} params
 * @returns {Promise<{method: string, headers: Record<string, string>, body: string}>}
 */
export async function buildCanonicalJsonRequest({
  accountId,
  method = "POST",
  path,
  baseUrl,
  query,
  body = {},
  headers,
  privateKey,
  sign,
  timestampMs = Date.now(),
  nonce = randomBytes(16).toString("hex"),
}) {
  if (!accountId) {
    throw new Error("accountId is required for canonical JSON requests");
  }
  if (!path) {
    throw new Error("path is required for canonical JSON requests");
  }
  if (!Number.isFinite(timestampMs)) {
    throw new Error("timestampMs must be a finite number");
  }
  if (!nonce || typeof nonce !== "string") {
    throw new Error("nonce is required for canonical JSON requests");
  }
  if (!privateKey && typeof sign !== "function") {
    throw new Error("privateKey or sign is required for canonical JSON requests");
  }
  const methodUpper = String(method).toUpperCase();
  const normalizedTimestampMs = Math.trunc(timestampMs);
  const canonicalTarget = canonicalTargetFromPath({ path, query, baseUrl });
  const bodyJson = JSON.stringify(body);
  const message = canonicalRequestSignatureMessage({
    method: methodUpper,
    path: canonicalTarget.path,
    query: canonicalTarget.query,
    body: bodyJson,
    timestampMs: normalizedTimestampMs,
    nonce,
  });
  const signatureBase64 = privateKey
    ? Buffer.from(signEd25519(message, privateKey)).toString("base64")
    : normalizeSignatureBase64(
        await sign({
          message,
          messageBase64: message.toString("base64"),
          method: methodUpper,
          path: canonicalTarget.path,
          query: canonicalTarget.query,
          body: bodyJson,
          timestampMs: normalizedTimestampMs,
          nonce,
        }),
      );
  return {
    method: methodUpper,
    headers: {
      ...DEFAULT_JSON_HEADERS,
      ...normalizeHeadersInit(headers),
      "X-Iroha-Account": String(accountId),
      "X-Iroha-Signature": signatureBase64,
      "X-Iroha-Timestamp-Ms": String(normalizedTimestampMs),
      "X-Iroha-Nonce": nonce,
    },
    body: bodyJson,
  };
}
