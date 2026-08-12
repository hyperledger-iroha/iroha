import { Buffer } from "node:buffer";

import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";

export const SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1 = 2 * 1024 * 1024;
export const SORAFS_ORDERBOOK_RECEIPT_MAX_BYTES_V1 = 1024 * 1024;

const HASH_HEX_PATTERN = /^[0-9a-f]{64}$/u;
const RECEIPT_HASH_LITERAL_PATTERN = /^hash:[0-9A-F]{64}#[0-9A-F]{4}$/u;
const FIXED_REQUEST_HEADERS = new Set([
  "accept",
  "accept-encoding",
  "content-type",
  "prefer",
]);
const IDENTITY_KEYS = ["txHash", "entrypointHash", "signedTransactionHash"];
const RECEIPT_KEYS = ["payload", "signature"];
const RECEIPT_PAYLOAD_KEYS = [
  "tx_hash",
  "entrypoint_hash",
  "signed_transaction_hash",
  "submitted_at_ms",
  "submitted_at_height",
  "signer",
];

function requirePlainRecord(value, context) {
  if (
    value === null
    || typeof value !== "object"
    || Array.isArray(value)
    || ![Object.prototype, null].includes(Object.getPrototypeOf(value))
  ) {
    throw new TypeError(`${context} must be a plain object`);
  }
  return value;
}

function requireExactKeys(record, keys, context) {
  const actual = Reflect.ownKeys(record);
  if (
    actual.some((key) => typeof key !== "string")
    || actual.length !== keys.length
    || keys.some((key) => !actual.includes(key))
  ) {
    throw new TypeError(`${context} must contain exactly ${keys.join(", ")}`);
  }
}

function requireOwnData(record, key, context) {
  const descriptor = Object.getOwnPropertyDescriptor(record, key);
  if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) {
    throw new TypeError(`${context}.${key} must be an enumerable data property`);
  }
  return descriptor.value;
}

function requireHashHex(value, context) {
  if (typeof value !== "string" || !HASH_HEX_PATTERN.test(value)) {
    throw new TypeError(`${context} must be exactly 32 lowercase hexadecimal bytes`);
  }
  return value;
}

function requireNonEmptyString(value, context) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new TypeError(`${context} must be a non-empty exact string`);
  }
  return value;
}

function requireUnsigned(value, context) {
  if (
    (typeof value === "number" && Number.isSafeInteger(value) && value >= 0)
    || (typeof value === "bigint" && value >= 0n)
  ) {
    return value;
  }
  throw new TypeError(`${context} must be a non-negative lossless integer`);
}

function nativeFunction(native, name) {
  const fn = native?.[name];
  if (typeof fn !== "function") {
    throw new Error(
      `native binding is missing ${name}; rebuild iroha_js_host for this SDK version`,
    );
  }
  return fn;
}

function normalizeIdentity(value) {
  const record = requirePlainRecord(value, "native orderbook submission identity");
  requireExactKeys(record, IDENTITY_KEYS, "native orderbook submission identity");
  return Object.freeze({
    txHash: requireHashHex(
      requireOwnData(record, "txHash", "native orderbook submission identity"),
      "native orderbook submission identity.txHash",
    ),
    entrypointHash: requireHashHex(
      requireOwnData(record, "entrypointHash", "native orderbook submission identity"),
      "native orderbook submission identity.entrypointHash",
    ),
    signedTransactionHash: requireHashHex(
      requireOwnData(record, "signedTransactionHash", "native orderbook submission identity"),
      "native orderbook submission identity.signedTransactionHash",
    ),
  });
}

function headerEntries(headers) {
  if (!headers) return [];
  if (typeof Headers === "function" && headers instanceof Headers) {
    return [...headers.entries()];
  }
  if (typeof headers[Symbol.iterator] === "function" && !Array.isArray(headers)) {
    return [...headers];
  }
  return Object.entries(headers);
}

export function assertSorafsOrderbookFixedHeaders(defaultHeaders, context) {
  for (const [rawName, rawValue] of headerEntries(defaultHeaders)) {
    if (rawValue === undefined || rawValue === null) continue;
    const name = String(rawName).toLowerCase();
    if (!FIXED_REQUEST_HEADERS.has(name)) continue;
    if (name === "accept" && String(rawValue) === "application/json") continue;
    throw new TypeError(`${context} forbids overriding ${String(rawName)}`);
  }
}

export function prepareSorafsOrderbookSubmission({
  route,
  signedTransaction,
  expectedNetworkIdBytes,
  expectedReceiptSigner,
  native,
  context,
}) {
  const inspect = nativeFunction(native, "inspectSorafsOrderbookSubmissionV1");
  nativeFunction(native, "verifySorafsOrderbookSubmissionReceiptV1");
  let body;
  if (ArrayBuffer.isView(signedTransaction)) {
    body = Buffer.from(
      signedTransaction.buffer,
      signedTransaction.byteOffset,
      signedTransaction.byteLength,
    );
  } else if (signedTransaction instanceof ArrayBuffer) {
    body = Buffer.from(signedTransaction);
  } else {
    throw new TypeError(`${context}.signedTransaction must be exact bytes`);
  }
  body = Buffer.from(body);
  if (
    body.length === 0
    || body.length > SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1
  ) {
    throw new RangeError(
      `${context}.signedTransaction must contain 1..${SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1} bytes`,
    );
  }
  requireNonEmptyString(expectedReceiptSigner, `${context}.expectedReceiptSigner`);
  const identity = normalizeIdentity(
    inspect(
      route,
      Buffer.from(expectedNetworkIdBytes),
      expectedReceiptSigner,
      body,
    ),
  );
  return Object.freeze({
    body,
    expectedReceiptSigner,
    identity,
    native,
  });
}

function requireMatchingHeader(value, expected, name) {
  if (typeof value !== "string" || !HASH_HEX_PATTERN.test(value)) {
    throw new Error(`${name} must occur exactly once as a lowercase 32-byte hash`);
  }
  if (value !== expected) {
    throw new Error(`${name} does not match the submitted transaction`);
  }
}

export function validateSorafsOrderbookSubmissionHeaders(
  { contentType, contentEncoding, txHash, entrypointHash, signedTransactionHash },
  identity,
) {
  if (contentType !== "application/x-norito") {
    throw new Error("SoraFS orderbook submission response Content-Type must be exactly application/x-norito");
  }
  if (contentEncoding !== null && contentEncoding !== "identity") {
    throw new Error("SoraFS orderbook submission response Content-Encoding must be absent or exactly identity");
  }
  requireMatchingHeader(txHash, identity.txHash, "x-iroha-transaction-hash");
  requireMatchingHeader(
    entrypointHash,
    identity.entrypointHash,
    "x-iroha-entrypoint-hash",
  );
  requireMatchingHeader(
    signedTransactionHash,
    identity.signedTransactionHash,
    "x-iroha-signed-transaction-hash",
  );
}

function normalizeVerifiedReceipt(value, expectedReceiptSigner) {
  const receipt = requirePlainRecord(value, "verified orderbook submission receipt");
  requireExactKeys(receipt, RECEIPT_KEYS, "verified orderbook submission receipt");
  const payload = requirePlainRecord(
    requireOwnData(receipt, "payload", "verified orderbook submission receipt"),
    "verified orderbook submission receipt.payload",
  );
  requireExactKeys(
    payload,
    RECEIPT_PAYLOAD_KEYS,
    "verified orderbook submission receipt.payload",
  );
  for (const key of ["tx_hash", "entrypoint_hash", "signed_transaction_hash"]) {
    const hash = requireOwnData(payload, key, "verified orderbook submission receipt.payload");
    if (typeof hash !== "string" || !RECEIPT_HASH_LITERAL_PATTERN.test(hash)) {
      throw new TypeError(`verified orderbook submission receipt.payload.${key} is invalid`);
    }
  }
  requireUnsigned(
    requireOwnData(payload, "submitted_at_ms", "verified orderbook submission receipt.payload"),
    "verified orderbook submission receipt.payload.submitted_at_ms",
  );
  requireUnsigned(
    requireOwnData(payload, "submitted_at_height", "verified orderbook submission receipt.payload"),
    "verified orderbook submission receipt.payload.submitted_at_height",
  );
  if (
    requireOwnData(payload, "signer", "verified orderbook submission receipt.payload")
    !== expectedReceiptSigner
  ) {
    throw new Error("verified orderbook submission receipt signer changed at the native boundary");
  }
  requireNonEmptyString(
    requireOwnData(receipt, "signature", "verified orderbook submission receipt"),
    "verified orderbook submission receipt.signature",
  );
  return receipt;
}

export function verifySorafsOrderbookSubmissionReceipt(body, prepared) {
  if (!Buffer.isBuffer(body) || body.length === 0) {
    throw new Error("SoraFS orderbook submission response must contain a non-empty Norito receipt");
  }
  if (body.length > SORAFS_ORDERBOOK_RECEIPT_MAX_BYTES_V1) {
    throw new RangeError("SoraFS orderbook submission receipt exceeds the bounded response limit");
  }
  const verify = nativeFunction(
    prepared.native,
    "verifySorafsOrderbookSubmissionReceiptV1",
  );
  const json = verify(
    body,
    prepared.identity.txHash,
    prepared.identity.entrypointHash,
    prepared.identity.signedTransactionHash,
    prepared.expectedReceiptSigner,
  );
  if (typeof json !== "string") {
    throw new TypeError("native orderbook receipt verifier must return JSON text");
  }
  return normalizeVerifiedReceipt(
    parseStrictLosslessIntegerJson(json, "verified orderbook submission receipt"),
    prepared.expectedReceiptSigner,
  );
}
