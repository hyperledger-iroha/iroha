import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { signEd25519 } from "./crypto.js";
import { compareUtf16 } from "./ordering.js";

function encodeComponent(value) {
  return encodeURIComponent(value).replace(/%20/gu, "+");
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
    const keyOrder = compareUtf16(a[0], b[0]);
    if (keyOrder !== 0) {
      return keyOrder;
    }
    return compareUtf16(a[1], b[1]);
  });
  return pairs.map(([k, v]) => `${encodeComponent(k)}=${encodeComponent(v)}`).join("&");
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
 * Build canonical signing headers for app-facing Torii endpoints.
 * @param {{accountId: string, method: string, path: string, query?: string | URLSearchParams, body?: Buffer | ArrayBuffer | ArrayBufferView | string, privateKey: Buffer | ArrayBuffer | ArrayBufferView}} params
 * @returns {{ "X-Iroha-Account": string, "X-Iroha-Signature": string }}
 */
export function buildCanonicalRequestHeaders({
  accountId,
  method,
  path,
  query,
  body,
  privateKey,
}) {
  if (!accountId) {
    throw new Error("accountId is required for canonical headers");
  }
  if (!privateKey) {
    throw new Error("privateKey is required for canonical headers");
  }
  const message = canonicalRequestMessage({ method, path, query, body });
  const signature = signEd25519(message, privateKey);
  return {
    "X-Iroha-Account": String(accountId),
    "X-Iroha-Signature": Buffer.from(signature).toString("base64"),
  };
}
