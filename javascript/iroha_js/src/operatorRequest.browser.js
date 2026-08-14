import { Buffer } from "buffer";

import { canonicalRequestMessage } from "./canonicalMessage.js";
import { networkIdBytes } from "./networkId.js";

const OPERATOR_REQUEST_DOMAIN_V1 = Buffer.from(
  "iroha.operator.http-request.network.v1\0",
  "utf8",
);
const FORBIDDEN_AUTH_HEADERS = new Set([
  "authorization",
  "x-api-token",
  "x-iroha-account",
  "x-iroha-signature",
  "x-iroha-timestamp-ms",
  "x-iroha-nonce",
  "x-iroha-witness",
  "x-iroha-operator-public-key",
  "x-iroha-operator-timestamp-ms",
  "x-iroha-operator-nonce",
  "x-iroha-operator-signature",
]);

function exactAscii(value, context) {
  if (
    typeof value !== "string"
    || value.length === 0
    || value.trim() !== value
    || value.length > 512
    || !/^[\x21-\x7e]+$/u.test(value)
  ) {
    throw new TypeError(`${context} must be exact non-empty printable ASCII`);
  }
  return value;
}

function freshNonce() {
  const bytes = new Uint8Array(16);
  if (typeof globalThis.crypto?.getRandomValues !== "function") {
    throw new TypeError("operator request requires crypto.getRandomValues");
  }
  globalThis.crypto.getRandomValues(bytes);
  return Buffer.from(bytes).toString("base64url");
}

/** Immutable browser-safe exact-network operator signing context. */
export class OperatorSigningContext {
  constructor(networkId, signer) {
    networkIdBytes(networkId, "OperatorSigningContext.networkId");
    if (signer === null || typeof signer !== "object") {
      throw new TypeError("OperatorSigningContext signer must be an object");
    }
    const publicKey = exactAscii(signer.publicKey, "operator publicKey");
    if (typeof signer.sign !== "function") {
      throw new TypeError("OperatorSigningContext signer.sign must be a function");
    }
    Object.defineProperties(this, {
      networkId: { value: networkId, enumerable: true },
      publicKey: { value: publicKey, enumerable: true },
      sign: { value: signer.sign.bind(signer) },
    });
    Object.freeze(this);
  }
}

export function requireOperatorSigningContext(value, context) {
  if (!(value instanceof OperatorSigningContext)) {
    throw new TypeError(
      `${context} requires an immutable OperatorSigningContext`,
    );
  }
  return value;
}

/** Install fresh exact-request operator headers after the URL and body are final. */
export async function applyOperatorGetHeaders(headers, signingContext, url) {
  for (const name of Object.keys(headers)) {
    if (FORBIDDEN_AUTH_HEADERS.has(name.toLowerCase())) {
      throw new TypeError(
        `operator GET requires generated signing; header ${name} is not accepted`,
      );
    }
  }
  const timestampMs = Date.now();
  const nonce = freshNonce();
  const canonical = canonicalRequestMessage({
    method: "GET",
    path: url.pathname,
    query: url.search.startsWith("?") ? url.search.slice(1) : url.search,
    body: Buffer.alloc(0),
  });
  const message = Buffer.concat([
    OPERATOR_REQUEST_DOMAIN_V1,
    Buffer.from(networkIdBytes(signingContext.networkId)),
    canonical,
    Buffer.from(`\n${timestampMs}\n${nonce}`, "utf8"),
  ]);
  const signature = Buffer.from(await signingContext.sign(message));
  if (signature.length === 0) {
    throw new TypeError("operator signer returned an empty signature");
  }
  Object.assign(headers, {
    "X-Iroha-Operator-Public-Key": signingContext.publicKey,
    "X-Iroha-Operator-Timestamp-Ms": String(timestampMs),
    "X-Iroha-Operator-Nonce": nonce,
    "X-Iroha-Operator-Signature": signature.toString("base64"),
  });
}
