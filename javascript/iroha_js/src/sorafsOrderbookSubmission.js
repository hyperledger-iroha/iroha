import { Buffer } from "node:buffer";

import { parseStrictLosslessIntegerJson } from "./strictLosslessJson.js";

export const SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1 = 2 * 1024 * 1024;
export const SORAFS_ORDERBOOK_RECEIPT_MAX_BYTES_V1 = 1024 * 1024;

export class SorafsOrderbookSubmissionAmbiguousError extends Error {
  constructor(route, identity) {
    super(
      "SoraFS orderbook submission outcome is ambiguous after dispatch; "
      + "do not resubmit automatically, reconcile the expected transaction identity",
    );
    this.name = "SorafsOrderbookSubmissionAmbiguousError";
    Object.defineProperties(this, {
      route: { value: route, enumerable: true },
      expectedIdentity: {
        value: Object.freeze({ ...identity }),
        enumerable: true,
      },
    });
  }
}

const HASH_HEX_PATTERN = /^[0-9a-f]{64}$/u;
const RECEIPT_HASH_LITERAL_PATTERN = /^hash:[0-9A-F]{64}#[0-9A-F]{4}$/u;
const MAX_SIGNATURE_HEX_LENGTH = 2 * 3_309;
const AbortControllerConstructor = globalThis.AbortController;
const abortControllerAbort = AbortControllerConstructor?.prototype?.abort;
const abortControllerSignalGetter = AbortControllerConstructor
  ? Object.getOwnPropertyDescriptor(AbortControllerConstructor.prototype, "signal")?.get
  : null;
const scheduleTimeout = globalThis.setTimeout;
const cancelTimeout = globalThis.clearTimeout;
const FIXED_REQUEST_HEADERS = new Set([
  "accept",
  "accept-encoding",
  "connection",
  "content-encoding",
  "content-length",
  "content-type",
  "expect",
  "host",
  "keep-alive",
  "prefer",
  "proxy-connection",
  "te",
  "trailer",
  "transfer-encoding",
  "upgrade",
  "x-http-method-override",
  "x-method-override",
]);
const IDENTITY_KEYS = ["entrypointHash", "signedTransactionHash"];
const RECEIPT_KEYS = ["payload", "signature"];
const RECEIPT_PAYLOAD_KEYS = [
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

function nativeFunction(native, name) {
  const fn = native?.[name];
  if (typeof fn !== "function") {
    throw new Error(
      `native binding is missing ${name}; rebuild iroha_js_host for this SDK version`,
    );
  }
  return fn.bind(native);
}

function normalizeIdentity(value) {
  const record = requirePlainRecord(value, "native orderbook submission identity");
  requireExactKeys(record, IDENTITY_KEYS, "native orderbook submission identity");
  return Object.freeze({
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

export function sorafsOrderbookHeaderFingerprint(headers) {
  return JSON.stringify(headerEntries(headers)
    .map(([name, value]) => [String(name).toLowerCase(), String(value)])
    .sort(([left], [right]) => left.localeCompare(right)));
}

export function validateSorafsOrderbookSubmissionTransport(
  baseUrl,
  allowInsecure,
  path,
  emitInsecureTelemetry,
  context,
) {
  let base;
  try { base = new URL(baseUrl); } catch { throw new Error(`${context} requires a canonical HTTP(S) Torii base URL`); }
  if (
    base.username || base.password || base.search || base.hash
    || (base.protocol !== "https:" && base.protocol !== "http:")
  ) {
    throw new Error(`${context} requires a canonical HTTP(S) Torii base URL without userinfo, query, or fragment`);
  }
  if (base.protocol === "https:") return;
  if (!allowInsecure) {
    throw new Error(`${context} requires an https Torii base URL unless allowInsecure is true`);
  }
  emitInsecureTelemetry({
    client: "torii", method: "POST", hasCredentials: true, hasSensitiveBody: true,
    hasCanonicalAuth: false, allowInsecure: true, url: new URL(path, `${baseUrl}/`).toString(), baseUrl,
    host: base.host, protocol: base.protocol, pathIsAbsolute: false, originMatches: true,
  });
}

export function createSorafsOrderbookSubmissionDeadline(
  callerSignal,
  timeoutMs,
  context,
  { addAbortListener, removeAbortListener, isAborted },
) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0) {
    throw new TypeError(`${context} requires a positive finite client timeoutMs`);
  }
  if (typeof AbortControllerConstructor !== "function" || typeof abortControllerAbort !== "function" || typeof abortControllerSignalGetter !== "function") {
    throw new Error(`${context} requires AbortController for its bounded operation deadline`);
  }
  const controller = new AbortControllerConstructor();
  const forwardAbort = () => Reflect.apply(abortControllerAbort, controller, []);
  if (callerSignal) addAbortListener(callerSignal, forwardAbort);
  const timer = scheduleTimeout(() => {
    const error = new Error(`${context} exceeded its ${timeoutMs}ms operation deadline`);
    error.name = "TimeoutError";
    Reflect.apply(abortControllerAbort, controller, [error]);
  }, timeoutMs);
  if (callerSignal && isAborted(callerSignal)) forwardAbort();
  return Object.freeze({
    signal: Reflect.apply(abortControllerSignalGetter, controller, []),
    dispose() {
      cancelTimeout(timer);
      if (callerSignal) removeAbortListener(callerSignal, forwardAbort);
    },
  });
}

export function prepareSorafsOrderbookSubmission({
  route,
  signedTransaction,
  expectedNetworkIdBytes,
  expectedChainDiscriminant,
  expectedReceiptSigner,
  native,
  context,
}) {
  const inspect = nativeFunction(native, "inspectSorafsOrderbookSubmissionV1");
  const verifyReceipt = nativeFunction(native, "verifySorafsOrderbookSubmissionReceiptV1");
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
      expectedChainDiscriminant,
      expectedReceiptSigner,
      body,
    ),
  );
  return Object.freeze({
    body,
    expectedReceiptSigner,
    identity,
    verifyReceipt,
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
