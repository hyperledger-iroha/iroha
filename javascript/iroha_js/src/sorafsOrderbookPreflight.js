import { Buffer } from "node:buffer";
import { snapshotBoundedBytes } from "./boundedByteSnapshot.js";

// Request custody must be captured synchronously, before optional receipt code loads.
export const SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1 = 2 * 1024 * 1024;

export const HASH_HEX_PATTERN = /^[0-9a-f]{63}[13579bdf]$/u;

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

export function requirePlainRecord(value, context) {
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

export function requireExactKeys(record, keys, context) {
  const actual = Reflect.ownKeys(record);
  if (
    actual.some((key) => typeof key !== "string")
    || actual.length !== keys.length
    || keys.some((key) => !actual.includes(key))
  ) {
    throw new TypeError(`${context} must contain exactly ${keys.join(", ")}`);
  }
}

export function requireOwnData(record, key, context) {
  const descriptor = Object.getOwnPropertyDescriptor(record, key);
  if (!descriptor || !("value" in descriptor) || !descriptor.enumerable) {
    throw new TypeError(`${context}.${key} must be an enumerable data property`);
  }
  return descriptor.value;
}

function requireHashHex(value, context) {
  if (typeof value !== "string" || !HASH_HEX_PATTERN.test(value)) {
    throw new TypeError(`${context} must be an exact canonical lowercase 32-byte Iroha hash`);
  }
  return value;
}

function requireNonEmptyString(value, context) {
  if (typeof value !== "string" || value.length === 0 || value.trim() !== value) {
    throw new TypeError(`${context} must be a non-empty exact string`);
  }
  return value;
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
  { addAbortListener, removeAbortListener, isAborted, abortReason },
) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0) {
    throw new TypeError(`${context} requires a positive finite client timeoutMs`);
  }
  if (typeof AbortControllerConstructor !== "function" || typeof abortControllerAbort !== "function" || typeof abortControllerSignalGetter !== "function") {
    throw new Error(`${context} requires AbortController for its bounded operation deadline`);
  }
  const controller = new AbortControllerConstructor();
  const forwardAbort = () => Reflect.apply(
    abortControllerAbort,
    controller,
    [abortReason(callerSignal)],
  );
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
  const inspect = nativeFunction(
    native,
    "inspectSorafsOrderbookSubmissionForDiscriminantV1",
  );
  const verifyReceipt = nativeFunction(native, "verifySorafsOrderbookSubmissionReceiptV1");
  const body = snapshotBoundedBytes(
    signedTransaction, `${context}.signedTransaction`, SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1, RangeError,
  );
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
