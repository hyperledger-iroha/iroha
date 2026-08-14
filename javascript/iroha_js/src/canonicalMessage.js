import { Buffer } from "buffer";

import { createHash } from "./cryptoHash.js";

/** Maximum decoded non-empty form pairs in a canonical V1 request. */
export const CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 = 64;
/** Maximum UTF-8 bytes in the raw canonical V1 query. */
export const CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 = 64 * 1024;
/** Maximum UTF-8 bytes in the canonical V1 HTTP method token. */
export const CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 = 32;
/** Maximum UTF-8 bytes in the percent-encoded canonical V1 path. */
export const CANONICAL_REQUEST_MAX_PATH_BYTES_V1 = 64 * 1024;

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

function hasValidCanonicalPathPercentEscapes(path) {
  for (let index = 0; index < path.length; index += 1) {
    if (path[index] !== "%") continue;
    if (
      index + 2 >= path.length ||
      !/^[0-9A-Fa-f]{2}$/u.test(path.slice(index + 1, index + 3))
    ) {
      return false;
    }
    index += 2;
  }
  return true;
}

function hasCanonicalPathDotSegment(path) {
  return path.split("/").some((segment) => {
    const decodedDots = segment.replace(/%2e/giu, ".");
    return decodedDots === "." || decodedDots === "..";
  });
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
  const pairs = Array.from(params.entries());
  if (pairs.length > CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1) {
    throw new RangeError(
      `canonical request query exceeds ${CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1} pairs`,
    );
  }
  pairs.sort((a, b) => compareUtf8(a[0], b[0]) || compareUtf8(a[1], b[1]));
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
  if (!/^[!#$%&'*+\-.^_`|~0-9A-Za-z]+$/u.test(methodText)) {
    throw new TypeError("canonical request method must be a non-empty ASCII HTTP token");
  }
  if (Buffer.byteLength(methodText, "utf8") > CANONICAL_REQUEST_MAX_METHOD_BYTES_V1) {
    throw new RangeError(
      `canonical request method exceeds ${CANONICAL_REQUEST_MAX_METHOD_BYTES_V1} UTF-8 bytes`,
    );
  }
  if (
    !pathText.startsWith("/") ||
    pathText.startsWith("//") ||
    pathText.includes("?") ||
    pathText.includes("#") ||
    !/^\/(?:[!$&'()*+,\-./0-9:;=@A-Z_a-z~%])*$/u.test(pathText) ||
    !hasValidCanonicalPathPercentEscapes(pathText) ||
    hasCanonicalPathDotSegment(pathText)
  ) {
    throw new TypeError(
      "canonical request path must be an exact root-relative ASCII path without query or fragment",
    );
  }
  if (Buffer.byteLength(pathText, "utf8") > CANONICAL_REQUEST_MAX_PATH_BYTES_V1) {
    throw new RangeError(
      `canonical request path exceeds ${CANONICAL_REQUEST_MAX_PATH_BYTES_V1} UTF-8 bytes`,
    );
  }
  const canonicalQuery = canonicalQueryString(query);
  const bodyBuffer = body === undefined ? Buffer.alloc(0) : Buffer.from(body);
  const bodyHash = createHash("sha256").update(bodyBuffer).digest("hex");
  return Buffer.from(
    `${methodText.toUpperCase()}\n${pathText}\n${canonicalQuery}\n${bodyHash}`,
    "utf8",
  );
}
