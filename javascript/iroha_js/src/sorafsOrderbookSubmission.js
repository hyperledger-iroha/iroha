import { Buffer } from "node:buffer";

import {
  HASH_HEX_PATTERN,
  requirePlainRecord,
  requireExactKeys,
  requireOwnData,
} from "./sorafsOrderbookPreflight.js";
export {
  assertSorafsOrderbookFixedHeaders,
  createSorafsOrderbookSubmissionDeadline,
  prepareSorafsOrderbookSubmission,
  sorafsOrderbookHeaderFingerprint,
  SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1,
  validateSorafsOrderbookSubmissionTransport,
} from "./sorafsOrderbookPreflight.js";

import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";
export { SorafsOrderbookSubmissionAmbiguousError } from "./sorafsOrderbookAmbiguousError.js";

export const SORAFS_ORDERBOOK_RECEIPT_MAX_BYTES_V1 = 1024 * 1024;

const RECEIPT_HASH_LITERAL_PATTERN = /^hash:[0-9A-F]{63}[13579BDF]#[0-9A-F]{4}$/u;
const MAX_SIGNATURE_HEX_LENGTH = 2 * 3_309;
const RECEIPT_KEYS = ["payload", "signature"];
const RECEIPT_PAYLOAD_KEYS = [
  "entrypoint_hash",
  "signed_transaction_hash",
  "submitted_at_ms",
  "submitted_at_height",
  "signer",
];

function requireUnsigned(value, context) {
  const maximum = (1n << 64n) - 1n;
  if (
    (typeof value === "number" && Number.isSafeInteger(value) && value >= 0)
    || (typeof value === "bigint" && value >= 0n && value <= maximum)
  ) {
    return value;
  }
  throw new TypeError(`${context} must be a non-negative lossless integer`);
}

function requireReceiptHash(value, expected, context) {
  if (typeof value !== "string" || !RECEIPT_HASH_LITERAL_PATTERN.test(value)) {
    throw new TypeError(`${context} is invalid`);
  }
  const body = value.slice(5, 69);
  let crc = 0xffff;
  for (const byte of Buffer.from(`hash:${body}`, "ascii")) {
    crc ^= byte << 8;
    for (let bit = 0; bit < 8; bit += 1) {
      crc = crc & 0x8000 ? ((crc << 1) ^ 0x1021) & 0xffff : (crc << 1) & 0xffff;
    }
  }
  if (value.slice(70) !== crc.toString(16).toUpperCase().padStart(4, "0")) {
    throw new TypeError(`${context} has an invalid checksum`);
  }
  if (body.toLowerCase() !== expected) {
    throw new Error(`${context} changed at the native boundary`);
  }
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
  { contentType, contentEncoding, entrypointHash, signedTransactionHash },
  identity,
) {
  if (contentType !== "application/x-norito") {
    throw new Error("SoraFS orderbook submission response Content-Type must be exactly application/x-norito");
  }
  if (contentEncoding !== null && contentEncoding !== "identity") {
    throw new Error("SoraFS orderbook submission response Content-Encoding must be absent or exactly identity");
  }
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

function normalizeVerifiedReceipt(value, prepared) {
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
  for (const [key, identityKey] of [
    ["entrypoint_hash", "entrypointHash"],
    ["signed_transaction_hash", "signedTransactionHash"],
  ]) {
    requireReceiptHash(
      requireOwnData(payload, key, "verified orderbook submission receipt.payload"),
      prepared.identity[identityKey],
      `verified orderbook submission receipt.payload.${key}`,
    );
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
    !== prepared.expectedReceiptSigner
  ) {
    throw new Error("verified orderbook submission receipt signer changed at the native boundary");
  }
  const signature = requireOwnData(
    receipt,
    "signature",
    "verified orderbook submission receipt",
  );
  if (
    typeof signature !== "string"
    || signature.length > MAX_SIGNATURE_HEX_LENGTH
    || !/^(?:[0-9A-F]{2})+$/u.test(signature)
  ) {
    throw new TypeError("verified orderbook submission receipt.signature is invalid");
  }
  return receipt;
}

export function verifySorafsOrderbookSubmissionReceipt(body, prepared) {
  if (!Buffer.isBuffer(body) || body.length === 0) {
    throw new Error("SoraFS orderbook submission response must contain a non-empty Norito receipt");
  }
  if (body.length > SORAFS_ORDERBOOK_RECEIPT_MAX_BYTES_V1) {
    throw new RangeError("SoraFS orderbook submission receipt exceeds the bounded response limit");
  }
  const json = prepared.verifyReceipt(
    body,
    prepared.identity.entrypointHash,
    prepared.identity.signedTransactionHash,
    prepared.expectedReceiptSigner,
  );
  if (typeof json !== "string") {
    throw new TypeError("native orderbook receipt verifier must return JSON text");
  }
  return normalizeVerifiedReceipt(
    parseStrictLosslessIntegerJson(json, "verified orderbook submission receipt"),
    prepared,
  );
}
