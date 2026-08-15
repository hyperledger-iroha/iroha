import { Buffer } from "buffer";
import { randomBytes } from "node:crypto";

import { canonicalRequestMessage } from "./canonicalMessage.js";
import { preparedTransportQuery } from "./canonicalTransport.js";
import { networkIdBytes } from "./networkId.js";

const OPERATOR_REQUEST_DOMAIN_V1 = Buffer.from(
  "iroha.operator.http-request.network.v1\0",
  "utf8",
);

const ISO_RETIRED_AUTH_HEADERS = new Set([
  "authorization",
  "x-api-token",
  "x-iroha-account",
  "x-iroha-signature",
  "x-iroha-timestamp-ms",
  "x-iroha-nonce",
  "x-iroha-witness",
  "x-iroha-iso-profile",
  "x-iroha-operator-public-key",
  "x-iroha-operator-timestamp-ms",
  "x-iroha-operator-nonce",
  "x-iroha-operator-signature",
]);

function requireExactAscii(value, context, maximumLength = 512) {
  if (
    typeof value !== "string" ||
    value.length === 0 ||
    value.trim() !== value ||
    value.length > maximumLength ||
    /[^\x21-\x7e]/u.test(value)
  ) {
    throw new TypeError(`${context} must be exact non-empty printable ASCII`);
  }
  return value;
}

function signatureBytes(value) {
  if (Buffer.isBuffer(value)) {
    return Buffer.from(value);
  }
  if (ArrayBuffer.isView(value)) {
    return Buffer.from(value.buffer, value.byteOffset, value.byteLength);
  }
  if (value instanceof ArrayBuffer) {
    return Buffer.from(value);
  }
  throw new TypeError("operator signer must return signature bytes");
}

/** Immutable exact-network signer used only for Torii operator requests. */
export class OperatorSigningContext {
  #networkId;
  #publicKey;
  #signer;

  /**
   * @param {import("./networkId.js").NetworkId} networkId
   * @param {{publicKey: string, sign: (message: Buffer) => Promise<ArrayBuffer | ArrayBufferView | Buffer> | ArrayBuffer | ArrayBufferView | Buffer}} signer
   */
  constructor(networkId, signer) {
    networkIdBytes(networkId, "OperatorSigningContext.networkId");
    if (signer === null || typeof signer !== "object") {
      throw new TypeError("OperatorSigningContext.signer must be an object");
    }
    const publicKey = requireExactAscii(
      signer.publicKey,
      "OperatorSigningContext.signer.publicKey",
    );
    if (typeof signer.sign !== "function") {
      throw new TypeError("OperatorSigningContext.signer.sign must be a function");
    }
    this.#networkId = networkId;
    this.#publicKey = publicKey;
    this.#signer = signer.sign.bind(signer);
    Object.freeze(this);
  }

  get networkId() {
    return this.#networkId;
  }

  get publicKey() {
    return this.#publicKey;
  }

  async sign(message) {
    return signatureBytes(await this.#signer(Buffer.from(message)));
  }
}

/** Normalize an optional operator signing context without accepting lookalikes. */
export function resolveOperatorSigningContext(value, context = "operatorSigningContext") {
  if (value === undefined || value === null) {
    return null;
  }
  if (!(value instanceof OperatorSigningContext)) {
    throw new TypeError(`${context} must be an OperatorSigningContext`);
  }
  return value;
}

/** Require the immutable exact-network context used by an operator-only API. */
export function requireOperatorSigningContext(value, context = "operator request") {
  const resolved = resolveOperatorSigningContext(value, `${context} operatorSigningContext`);
  if (resolved === null) {
    throw new TypeError(`${context} requires an immutable OperatorSigningContext`);
  }
  return resolved;
}

/** Install the immutable context slot used by operator-backed client methods. */
export function installOperatorSigningContext(target, value) {
  Object.defineProperty(target, "_operatorSigningContext", {
    value: resolveOperatorSigningContext(
      value,
      "ToriiClient options.operatorSigningContext",
    ),
    writable: false,
    configurable: false,
    enumerable: false,
  });
}

/** Reject all token, app-auth, legacy profile, and precomputed operator headers. */
export function rejectRetiredIsoAuthHeaders(headers, context) {
  for (const name of Object.keys(headers ?? {})) {
    if (ISO_RETIRED_AUTH_HEADERS.has(name.toLowerCase())) {
      throw new TypeError(
        `${context} requires generated operator signing; header ${name} is not accepted`,
      );
    }
  }
}

/** Require fresh generated operator auth for an ISO 20022 request. */
export function requireIsoOperatorSigningContext(value, headers) {
  rejectRetiredIsoAuthHeaders(headers, "ISO 20022 request");
  return requireOperatorSigningContext(value, "ISO 20022 request");
}

/** Keep application-account and operator request domains unambiguous. */
export function rejectMixedRequestAuth(canonicalAuth, operatorSigningContext) {
  if (canonicalAuth !== null && operatorSigningContext !== null) {
    throw new TypeError(
      "ToriiClient: canonical account auth and operator auth are mutually exclusive",
    );
  }
}

/** Build fresh exact-network operator signature headers for one request. */
export async function buildOperatorRequestHeaders({
  signingContext,
  method,
  path,
  query,
  body,
  timestampMs = Date.now(),
  nonce = randomBytes(12).toString("base64url"),
}) {
  if (!(signingContext instanceof OperatorSigningContext)) {
    throw new TypeError("operatorSigningContext is required");
  }
  if (!Number.isSafeInteger(timestampMs) || timestampMs < 0) {
    throw new TypeError("operator timestampMs must be a non-negative safe integer");
  }
  const checkedNonce = requireExactAscii(nonce, "operator nonce", 256);
  const request = canonicalRequestMessage({
    method,
    path,
    query: preparedTransportQuery(query),
    body,
  });
  const message = Buffer.concat([
    OPERATOR_REQUEST_DOMAIN_V1,
    Buffer.from(networkIdBytes(signingContext.networkId, "operatorSigningContext.networkId")),
    request,
    Buffer.from(`\n${timestampMs}\n${checkedNonce}`, "utf8"),
  ]);
  const signature = await signingContext.sign(message);
  if (signature.length === 0) {
    throw new TypeError("operator signer returned an empty signature");
  }
  return {
    "X-Iroha-Operator-Public-Key": signingContext.publicKey,
    "X-Iroha-Operator-Timestamp-Ms": String(timestampMs),
    "X-Iroha-Operator-Nonce": checkedNonce,
    "X-Iroha-Operator-Signature": signature.toString("base64"),
  };
}

/** Add fresh generated operator headers to the exact request header object. */
export async function applyOperatorRequestHeaders(
  headers,
  signingContext,
  method,
  url,
  body,
) {
  if (signingContext === null) {
    return;
  }
  Object.assign(
    headers,
    await buildOperatorRequestHeaders({
      signingContext,
      method,
      path: url.pathname,
      query: url.search.startsWith("?") ? url.search.slice(1) : url.search,
      body,
    }),
  );
}
