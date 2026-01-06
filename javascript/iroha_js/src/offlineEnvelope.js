import fs from "node:fs/promises";

import { blake2b256 } from "./blake2b.js";
import { hashSignedTransaction } from "./transaction.js";

const DEFAULT_SCHEMA_NAME = "iroha-js.transaction.Payload.v1";
const OFFLINE_ENVELOPE_VERSION = 1;

function toBuffer(value, context) {
  if (
    value instanceof ArrayBuffer ||
    ArrayBuffer.isView(value) ||
    value instanceof Buffer
  ) {
    return Buffer.from(value);
  }
  throw new TypeError(`${context} must be a Buffer, ArrayBuffer, or ArrayBufferView`);
}

function normalizeNonEmptyString(value, context) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new TypeError(`${context} must be a non-empty string`);
  }
  return value.trim();
}

function normalizeMetadata(metadata) {
  if (metadata === undefined || metadata === null) {
    return {};
  }
  if (typeof metadata !== "object" || Array.isArray(metadata)) {
    throw new TypeError("metadata must be an object mapping string keys to string values");
  }
  const normalized = {};
  for (const [key, value] of Object.entries(metadata)) {
    const normalizedKey = normalizeNonEmptyString(key, "metadata key");
    if (typeof value !== "string") {
      throw new TypeError("metadata values must be strings");
    }
    normalized[normalizedKey] = value;
  }
  return normalized;
}

function normalizeHashHex(hashHex, fallback, context) {
  const candidate =
    hashHex !== undefined && hashHex !== null
      ? hashHex
      : typeof fallback === "function"
        ? fallback()
        : fallback;
  if (typeof candidate !== "string") {
    throw new TypeError(`${context} must be a hex string`);
  }
  const normalized = candidate.trim().toLowerCase();
  if (!/^[0-9a-f]+$/u.test(normalized) || normalized.length !== 64) {
    throw new TypeError(`${context} must be a 64-character hex string`);
  }
  return normalized;
}

function normalizeIssuedAt(issuedAtMs) {
  if (issuedAtMs === undefined || issuedAtMs === null) {
    return Date.now();
  }
  const asNumber = Number(issuedAtMs);
  if (!Number.isFinite(asNumber) || asNumber < 0) {
    throw new TypeError("issuedAtMs must be a non-negative number");
  }
  return Math.floor(asNumber);
}

function normalizeOptionalBuffer(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  return toBuffer(value, context);
}

/**
 * Build a deterministic offline envelope that can be persisted and replayed later.
 * @param {{
 *   signedTransaction: ArrayBufferView | ArrayBuffer | Buffer,
 *   hashHex?: string,
 *   schemaName?: string,
 *   keyAlias: string,
 *   issuedAtMs?: number,
 *   metadata?: Record<string, string>,
 *   publicKey?: ArrayBufferView | ArrayBuffer | Buffer | null,
 *   exportedKeyBundle?: ArrayBufferView | ArrayBuffer | Buffer | null,
 * }} input
 * @returns {OfflineEnvelope}
 */
export function buildOfflineEnvelope(input) {
  if (!input || typeof input !== "object") {
    throw new TypeError("buildOfflineEnvelope input must be an object");
  }
  const signedTransaction = toBuffer(
    input.signedTransaction,
    "buildOfflineEnvelope.signedTransaction",
  );
  if (signedTransaction.length === 0) {
    throw new TypeError("buildOfflineEnvelope.signedTransaction must not be empty");
  }
  const schemaName = normalizeNonEmptyString(
    input.schemaName ?? DEFAULT_SCHEMA_NAME,
    "buildOfflineEnvelope.schemaName",
  );
  const keyAlias = normalizeNonEmptyString(input.keyAlias, "buildOfflineEnvelope.keyAlias");
  const issuedAtMs = normalizeIssuedAt(input.issuedAtMs);
  const metadata = normalizeMetadata(input.metadata);
  const publicKey = normalizeOptionalBuffer(input.publicKey, "buildOfflineEnvelope.publicKey");
  const exportedKeyBundle = normalizeOptionalBuffer(
    input.exportedKeyBundle,
    "buildOfflineEnvelope.exportedKeyBundle",
  );
  const hashHex = normalizeHashHex(
    input.hashHex,
    () => computeHashHex(signedTransaction),
    "buildOfflineEnvelope.hashHex",
  );

  return {
    version: OFFLINE_ENVELOPE_VERSION,
    signedTransaction,
    hashHex,
    schemaName,
    keyAlias,
    issuedAtMs,
    metadata,
    publicKey,
    exportedKeyBundle,
  };
}

/**
 * Convert an offline envelope into a JSON-serialisable record.
 * @param {OfflineEnvelope} envelope
 * @returns {SerializedOfflineEnvelope}
 */
export function serializeOfflineEnvelope(envelope) {
  const normalized = normalizeOfflineEnvelope(envelope, "serializeOfflineEnvelope.envelope");
  return {
    version: normalized.version,
    schema_name: normalized.schemaName,
    key_alias: normalized.keyAlias,
    issued_at_ms: normalized.issuedAtMs,
    hash_hex: normalized.hashHex,
    metadata: normalized.metadata,
    signed_transaction_b64: normalized.signedTransaction.toString("base64"),
    public_key_b64: normalized.publicKey?.toString("base64") ?? null,
    exported_key_bundle_b64: normalized.exportedKeyBundle?.toString("base64") ?? null,
  };
}

/**
 * Parse a serialised envelope (object or JSON string) into a normalised envelope.
 * @param {SerializedOfflineEnvelope | string | object} payload
 * @returns {OfflineEnvelope}
 */
export function parseOfflineEnvelope(payload) {
  const record =
    typeof payload === "string" ? JSON.parse(payload) : ensureRecord(payload, "payload");
  if (record.version !== OFFLINE_ENVELOPE_VERSION) {
    throw new TypeError(`unsupported offline envelope version ${record.version}`);
  }
  const schemaName = normalizeNonEmptyString(record.schema_name, "schema_name");
  const keyAlias = normalizeNonEmptyString(record.key_alias, "key_alias");
  const issuedAtMs = normalizeIssuedAt(record.issued_at_ms);
  const metadata = normalizeMetadata(record.metadata);
  const signedTransaction = decodeBase64Field(record.signed_transaction_b64, "signed_transaction_b64");
  const publicKey = decodeOptionalBase64Field(record.public_key_b64, "public_key_b64");
  const exportedKeyBundle = decodeOptionalBase64Field(
    record.exported_key_bundle_b64,
    "exported_key_bundle_b64",
  );
  const hashHex = normalizeHashHex(
    record.hash_hex,
    () => computeHashHex(signedTransaction),
    "hash_hex",
  );
  return {
    version: OFFLINE_ENVELOPE_VERSION,
    signedTransaction,
    hashHex,
    schemaName,
    keyAlias,
    issuedAtMs,
    metadata,
    publicKey,
    exportedKeyBundle,
  };
}

/**
 * Read an offline envelope from disk.
 * @param {string} filePath
 * @returns {Promise<OfflineEnvelope>}
 */
export async function readOfflineEnvelopeFile(filePath) {
  const payload = await fs.readFile(filePath, "utf8");
  return parseOfflineEnvelope(payload);
}

/**
 * Persist an offline envelope to disk (JSON with base64 payloads).
 * @param {string} filePath
 * @param {OfflineEnvelope} envelope
 * @returns {Promise<void>}
 */
export async function writeOfflineEnvelopeFile(filePath, envelope) {
  const serialized = serializeOfflineEnvelope(envelope);
  const payload = `${JSON.stringify(serialized, null, 2)}\n`;
  await fs.writeFile(filePath, payload, "utf8");
}

/**
 * Submit a stored offline envelope to Torii and optionally wait for status.
 * @param {import("./toriiClient.js").ToriiClient} toriiClient
 * @param {OfflineEnvelope} envelope
 * @param {{
 *   waitForStatus?: boolean,
 *   intervalMs?: number,
 *   timeoutMs?: number | null,
 *   maxAttempts?: number | null,
 * }} [options]
 * @returns {Promise<any>}
 */
export async function replayOfflineEnvelope(toriiClient, envelope, options = {}) {
  if (!toriiClient || typeof toriiClient.submitTransaction !== "function") {
    throw new TypeError("replayOfflineEnvelope requires a ToriiClient instance");
  }
  const normalized = normalizeOfflineEnvelope(envelope, "replayOfflineEnvelope.envelope");
  await toriiClient.submitTransaction(normalized.signedTransaction);
  if (options.waitForStatus === false) {
    return null;
  }
  return toriiClient.waitForTransactionStatus(normalized.hashHex, {
    intervalMs: options.intervalMs,
    timeoutMs: options.timeoutMs,
    maxAttempts: options.maxAttempts,
  });
}

function ensureRecord(value, context) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new TypeError(`${context} must be an object`);
  }
  return value;
}

function decodeBase64Strict(value) {
  const compact = value.replace(/\s+/g, "");
  if (compact.length === 0) {
    throw new Error("payload is empty");
  }
  let padded = compact;
  const paddingIndex = compact.indexOf("=");
  if (paddingIndex !== -1) {
    const head = compact.slice(0, paddingIndex);
    const padding = compact.slice(paddingIndex);
    if (!/^[0-9A-Za-z+/]*$/.test(head) || !/^={1,2}$/.test(padding)) {
      throw new Error("invalid base64 payload");
    }
    if (compact.length % 4 !== 0) {
      throw new Error("invalid base64 payload");
    }
  } else {
    if (!/^[0-9A-Za-z+/]+$/.test(compact) || compact.length % 4 === 1) {
      throw new Error("invalid base64 payload");
    }
    const padLength = (4 - (compact.length % 4)) % 4;
    padded = compact + "=".repeat(padLength);
  }
  const decoded = Buffer.from(padded, "base64");
  if (decoded.toString("base64") !== padded) {
    throw new Error("invalid base64 payload");
  }
  return decoded;
}

function decodeBase64Field(value, context) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new TypeError(`${context} must be a non-empty base64 string`);
  }
  try {
    return decodeBase64Strict(value);
  } catch (error) {
    const reason = error instanceof Error ? error.message : "unknown error";
    throw new TypeError(`${context} must be valid base64 (${reason})`);
  }
}

function decodeOptionalBase64Field(value, context) {
  if (value === undefined || value === null) {
    return null;
  }
  return decodeBase64Field(value, context);
}

function computeHashHex(payload) {
  try {
    const hash = hashSignedTransaction(payload, { encoding: "hex" });
    return typeof hash === "string" ? hash.toLowerCase() : Buffer.from(hash).toString("hex");
  } catch {
    return Buffer.from(blake2b256(payload)).toString("hex");
  }
}

function normalizeOfflineEnvelope(envelope, context) {
  const record = ensureRecord(envelope, context);
  if (record.version !== OFFLINE_ENVELOPE_VERSION) {
    throw new TypeError(`${context}.version must equal ${OFFLINE_ENVELOPE_VERSION}`);
  }
  const signedTransaction = toBuffer(record.signedTransaction, `${context}.signedTransaction`);
  if (signedTransaction.length === 0) {
    throw new TypeError(`${context}.signedTransaction must not be empty`);
  }
  return {
    version: OFFLINE_ENVELOPE_VERSION,
    signedTransaction,
    hashHex: normalizeHashHex(record.hashHex, null, `${context}.hashHex`),
    schemaName: normalizeNonEmptyString(record.schemaName, `${context}.schemaName`),
    keyAlias: normalizeNonEmptyString(record.keyAlias, `${context}.keyAlias`),
    issuedAtMs: normalizeIssuedAt(record.issuedAtMs),
    metadata: normalizeMetadata(record.metadata),
    publicKey: normalizeOptionalBuffer(record.publicKey, `${context}.publicKey`),
    exportedKeyBundle: normalizeOptionalBuffer(
      record.exportedKeyBundle,
      `${context}.exportedKeyBundle`,
    ),
  };
}

/**
 * @typedef {object} OfflineEnvelope
 * @property {number} version
 * @property {Buffer} signedTransaction
 * @property {string} hashHex
 * @property {string} schemaName
 * @property {string} keyAlias
 * @property {number} issuedAtMs
 * @property {Record<string, string>} metadata
 * @property {Buffer | null} publicKey
 * @property {Buffer | null} exportedKeyBundle
 */

/**
 * @typedef {object} SerializedOfflineEnvelope
 * @property {number} version
 * @property {string} schema_name
 * @property {string} key_alias
 * @property {number} issued_at_ms
 * @property {string} hash_hex
 * @property {Record<string, string>} metadata
 * @property {string} signed_transaction_b64
 * @property {string | null} public_key_b64
 * @property {string | null} exported_key_bundle_b64
 */
