import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { blake2b256 } from "./blake2b.js";

const DEFAULT_ROOT = path.join(os.homedir(), ".iroha", "offline_counters");
const DEFAULT_FILENAME = "journal.json";
const STORAGE_FILE = "file";
const STORAGE_MEMORY = "memory";

export const OfflineCounterPlatform = Object.freeze({
  APPLE_KEY: "apple_key",
  ANDROID_SERIES: "android_series",
});

export class OfflineCounterJournalError extends Error {
  constructor(message, options = {}) {
    super(message);
    this.name = "OfflineCounterJournalError";
    if (options.code !== undefined) {
      this.code = options.code;
    }
    if (options.context !== undefined) {
      this.context = options.context;
    }
    if (options.cause !== undefined) {
      this.cause = options.cause;
    }
  }
}

function defaultStoragePath() {
  return path.join(DEFAULT_ROOT, DEFAULT_FILENAME);
}

function normalizeStorage(value) {
  if (!value || value === STORAGE_FILE) {
    return STORAGE_FILE;
  }
  if (value === STORAGE_MEMORY) {
    return STORAGE_MEMORY;
  }
  throw new OfflineCounterJournalError(`Unknown offline counter storage: ${value}`, {
    code: "invalid_storage",
  });
}

function normalizeRecordTimestamp(value) {
  if (value === undefined || value === null) {
    return Date.now();
  }
  const asNumber = Number(value);
  if (!Number.isFinite(asNumber) || asNumber < 0) {
    throw new OfflineCounterJournalError("recordedAtMs must be a non-negative number", {
      code: "invalid_recorded_at",
    });
  }
  return Math.floor(asNumber);
}

function normalizeNonEmptyString(value, context) {
  if (typeof value !== "string" || value.trim().length === 0) {
    throw new OfflineCounterJournalError(`${context} must be a non-empty string`, {
      code: "invalid_string",
    });
  }
  return value.trim();
}

function normalizeHexId(value, context) {
  let hex = normalizeNonEmptyString(value, context);
  if (hex.startsWith("0x") || hex.startsWith("0X")) {
    hex = hex.slice(2);
  }
  if (!/^[0-9a-fA-F]+$/u.test(hex)) {
    throw new OfflineCounterJournalError(`${context} must be hex`, { code: "invalid_hex" });
  }
  return hex.toLowerCase();
}

function normalizeCounterValue(value, context) {
  if (typeof value === "bigint") {
    if (value < 0n) {
      throw new OfflineCounterJournalError(`${context} must be >= 0`, {
        code: "invalid_counter",
      });
    }
    if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
      throw new OfflineCounterJournalError(`${context} exceeds JS safe integer range`, {
        code: "invalid_counter",
      });
    }
    return Number(value);
  }
  const asNumber = Number(value);
  if (!Number.isFinite(asNumber) || !Number.isSafeInteger(asNumber) || asNumber < 0) {
    throw new OfflineCounterJournalError(`${context} must be a non-negative integer`, {
      code: "invalid_counter",
    });
  }
  return asNumber;
}

function normalizeSummaryItems(input, context) {
  if (Array.isArray(input)) {
    return input;
  }
  if (input && typeof input === "object" && Array.isArray(input.items)) {
    return input.items;
  }
  throw new OfflineCounterJournalError(`${context} must be a summary list`, {
    code: "invalid_summary_list",
  });
}

function normalizeCounterMap(value, context) {
  if (value === undefined || value === null) {
    return {};
  }
  if (typeof value !== "object" || Array.isArray(value)) {
    throw new OfflineCounterJournalError(`${context} must be an object`, {
      code: "invalid_counter_map",
    });
  }
  const result = {};
  for (const [key, raw] of Object.entries(value)) {
    const normalizedKey = normalizeNonEmptyString(key, `${context} key`);
    result[normalizedKey] = normalizeCounterValue(raw, `${context}.${normalizedKey}`);
  }
  return result;
}

function normalizeProofKind(kind, context) {
  if (typeof kind !== "string") {
    throw new OfflineCounterJournalError(`${context}.kind must be a string`, {
      code: "invalid_platform_proof",
    });
  }
  const normalized = kind.trim().toLowerCase();
  switch (normalized) {
    case "apple_app_attest":
      return "apple_app_attest";
    case "android_marker_key":
    case "marker_key":
      return "android_marker_key";
    case "android_provisioned":
    case "provisioned":
      return "android_provisioned";
    default:
      throw new OfflineCounterJournalError(`unsupported platform proof kind ${kind}`, {
        code: "invalid_platform_proof",
      });
  }
}

function normalizeSummaryHash(raw) {
  const trimmed = normalizeNonEmptyString(raw, "summary_hash_hex");
  if (trimmed.toLowerCase().startsWith("hash:")) {
    const match = /^hash:([0-9A-Fa-f]{64})#([0-9A-Fa-f]{4})$/u.exec(trimmed);
    if (!match) {
      throw new OfflineCounterJournalError("summary_hash_hex must be hash:<HEX>#<CRC>", {
        code: "invalid_summary_hash",
      });
    }
    const body = match[1].toUpperCase();
    const checksum = match[2].toUpperCase();
    const expected = crc16("hash", body).toString(16).toUpperCase().padStart(4, "0");
    if (checksum !== expected) {
      throw new OfflineCounterJournalError("summary_hash_hex checksum mismatch", {
        code: "invalid_summary_hash",
      });
    }
    return body.toLowerCase();
  }
  let hex = trimmed;
  if (hex.startsWith("0x") || hex.startsWith("0X")) {
    hex = hex.slice(2);
  }
  if (!/^[0-9A-Fa-f]{64}$/u.test(hex)) {
    throw new OfflineCounterJournalError("summary_hash_hex must be 64 hex chars", {
      code: "invalid_summary_hash",
    });
  }
  return hex.toLowerCase();
}

function crc16(tag, body) {
  let crc = 0xffff;
  const processByte = (byte) => {
    crc ^= (byte & 0xff) << 8;
    for (let i = 0; i < 8; i += 1) {
      if ((crc & 0x8000) !== 0) {
        crc = ((crc << 1) ^ 0x1021) & 0xffff;
      } else {
        crc = (crc << 1) & 0xffff;
      }
    }
  };

  for (const byte of Buffer.from(tag, "utf8")) {
    processByte(byte);
  }
  processByte(":".charCodeAt(0));
  for (const byte of Buffer.from(body, "utf8")) {
    processByte(byte);
  }
  return crc & 0xffff;
}

function encodeU64LE(value) {
  const buffer = Buffer.alloc(8);
  let remaining = BigInt(value);
  for (let index = 0; index < 8; index += 1) {
    buffer[index] = Number(remaining & 0xffn);
    remaining >>= 8n;
  }
  return buffer;
}

function encodeString(value) {
  const bytes = Buffer.from(value, "utf8");
  return Buffer.concat([encodeU64LE(bytes.length), bytes]);
}

function encodeCounterMap(map) {
  const entries = Object.entries(map ?? {});
  entries.sort(([a], [b]) => a.localeCompare(b));
  const chunks = [encodeU64LE(entries.length)];
  for (const [key, rawValue] of entries) {
    const value = normalizeCounterValue(rawValue, `counter.${key}`);
    const keyPayload = encodeString(key);
    const valuePayload = encodeU64LE(value);
    chunks.push(encodeU64LE(keyPayload.length), keyPayload);
    chunks.push(encodeU64LE(valuePayload.length), valuePayload);
  }
  return Buffer.concat(chunks);
}

function irohaHash(payload) {
  const digest = Buffer.from(blake2b256(payload));
  digest[digest.length - 1] |= 1;
  return digest;
}

function computeSummaryHashBytes(appleKeyCounters, androidSeriesCounters) {
  const applePayload = encodeCounterMap(appleKeyCounters);
  const androidPayload = encodeCounterMap(androidSeriesCounters);
  const writer = Buffer.concat([
    encodeU64LE(applePayload.length),
    applePayload,
    encodeU64LE(androidPayload.length),
    androidPayload,
  ]);
  return irohaHash(writer);
}

function cloneValue(value) {
  if (typeof structuredClone === "function") {
    return structuredClone(value);
  }
  return JSON.parse(JSON.stringify(value));
}

export class OfflineCounterJournal {
  constructor(options = {}) {
    this.#storage = normalizeStorage(options.storage);
    this.#storagePath =
      this.#storage === STORAGE_FILE
        ? path.resolve(options.storagePath ?? defaultStoragePath())
        : null;
    this.#entries = new Map();
    this.#mutex = Promise.resolve();
    this.#ready = this.#initialize();
  }

  static computeSummaryHashHex(appleKeyCounters, androidSeriesCounters) {
    return computeSummaryHashBytes(appleKeyCounters, androidSeriesCounters).toString("hex");
  }

  static computeSummaryHash(appleKeyCounters, androidSeriesCounters) {
    return Buffer.from(computeSummaryHashBytes(appleKeyCounters, androidSeriesCounters));
  }

  get storage() {
    return this.#storage;
  }

  get storagePath() {
    return this.#storagePath;
  }

  async upsert(summaries, options = {}) {
    await this.#ready;
    const items = normalizeSummaryItems(summaries, "summaries");
    const recordedAtMs = normalizeRecordTimestamp(options.recordedAtMs);
    const allowMismatch = Boolean(options.allowSummaryHashMismatch);
    return this.#runLocked(async () => {
      const inserted = [];
      for (const entry of items) {
        const record = entry && typeof entry === "object" ? entry : {};
        const certificateIdHex = normalizeHexId(
          record.certificate_id_hex,
          "certificate_id_hex",
        );
        const controllerId = normalizeNonEmptyString(
          record.controller_id,
          "controller_id",
        );
        const controllerDisplay = record.controller_display ?? null;
        const summaryHashRaw = record.summary_hash_hex ?? null;
        const appleCounters = normalizeCounterMap(
          record.apple_key_counters,
          "apple_key_counters",
        );
        const androidCounters = normalizeCounterMap(
          record.android_series_counters,
          "android_series_counters",
        );
        const computedHash = OfflineCounterJournal.computeSummaryHashHex(
          appleCounters,
          androidCounters,
        ).toLowerCase();
        if (summaryHashRaw !== null && summaryHashRaw !== undefined) {
          const expected = normalizeSummaryHash(summaryHashRaw);
          if (!allowMismatch && expected.toLowerCase() !== computedHash) {
            throw new OfflineCounterJournalError("summary_hash_hex mismatch", {
              code: "summary_hash_mismatch",
              certificateIdHex,
              expected,
              actual: computedHash,
            });
          }
        }
        const checkpoint = {
          certificateIdHex,
          controllerId,
          controllerDisplay,
          summaryHashHex: computedHash,
          appleKeyCounters: appleCounters,
          androidSeriesCounters: androidCounters,
          recordedAtMs,
        };
        this.#entries.set(certificateIdHex, checkpoint);
        inserted.push(cloneValue(checkpoint));
      }
      await this.#persistLocked();
      return inserted;
    });
  }

  async updateCounter(params) {
    await this.#ready;
    const certificateIdHex = normalizeHexId(params.certificateIdHex, "certificateIdHex");
    const controllerId = normalizeNonEmptyString(params.controllerId, "controllerId");
    const controllerDisplay = params.controllerDisplay ?? null;
    const platform = normalizePlatform(params.platform);
    const scope = normalizeNonEmptyString(params.scope, "scope");
    const counter = normalizeCounterValue(params.counter, "counter");
    const recordedAtMs = normalizeRecordTimestamp(params.recordedAtMs);
    return this.#runLocked(async () => {
      const existing = this.#entries.get(certificateIdHex) ?? null;
      const apple = { ...(existing?.appleKeyCounters ?? {}) };
      const android = { ...(existing?.androidSeriesCounters ?? {}) };
      const previous = platform === OfflineCounterPlatform.APPLE_KEY ? apple[scope] : android[scope];
      if (previous !== undefined) {
        const expected = previous + 1;
        if (counter !== expected) {
          throw new OfflineCounterJournalError("counter jump detected", {
            code: "counter_jump",
            certificateIdHex,
            scope,
            expected,
            actual: counter,
          });
        }
      }
      if (platform === OfflineCounterPlatform.APPLE_KEY) {
        apple[scope] = counter;
      } else {
        android[scope] = counter;
      }
      const computedHash = OfflineCounterJournal.computeSummaryHashHex(apple, android).toLowerCase();
      const resolvedControllerId = existing?.controllerId?.length
        ? existing.controllerId
        : controllerId;
      const resolvedControllerDisplay =
        existing?.controllerDisplay ?? controllerDisplay ?? null;
      const checkpoint = {
        certificateIdHex,
        controllerId: resolvedControllerId,
        controllerDisplay: resolvedControllerDisplay,
        summaryHashHex: computedHash,
        appleKeyCounters: apple,
        androidSeriesCounters: android,
        recordedAtMs,
      };
      this.#entries.set(certificateIdHex, checkpoint);
      await this.#persistLocked();
      return cloneValue(checkpoint);
    });
  }

  async advanceCounterFromProof(params) {
    await this.#ready;
    const certificateIdHex = normalizeHexId(params.certificateIdHex, "certificateIdHex");
    const controllerId = normalizeNonEmptyString(params.controllerId, "controllerId");
    const controllerDisplay = params.controllerDisplay ?? null;
    const proof = params.platformProof ?? {};
    if (!proof || typeof proof !== "object") {
      throw new OfflineCounterJournalError("platformProof must be an object", {
        code: "invalid_platform_proof",
      });
    }
    const kind = normalizeProofKind(
      proof.kind ?? proof.platform_kind ?? proof.platformKind,
      "platformProof",
    );
    let scope;
    let counter;
    let platform;
    if (kind === "apple_app_attest") {
      scope = normalizeNonEmptyString(
        proof.key_id ?? proof.keyId ?? proof.keyID,
        "platformProof.key_id",
      );
      counter = normalizeCounterValue(proof.counter, "platformProof.counter");
      platform = OfflineCounterPlatform.APPLE_KEY;
    } else if (kind === "android_marker_key") {
      scope = normalizeNonEmptyString(proof.series, "platformProof.series");
      counter = normalizeCounterValue(proof.counter, "platformProof.counter");
      platform = OfflineCounterPlatform.ANDROID_SERIES;
    } else {
      const schema = normalizeNonEmptyString(
        proof.manifest_schema ?? proof.manifestSchema,
        "platformProof.manifest_schema",
      );
      const deviceId = normalizeNonEmptyString(
        proof.device_id ?? proof.deviceId,
        "platformProof.device_id",
      );
      scope = `provisioned::${schema}::${deviceId}`;
      counter = normalizeCounterValue(proof.counter, "platformProof.counter");
      platform = OfflineCounterPlatform.ANDROID_SERIES;
    }
    return this.updateCounter({
      certificateIdHex,
      controllerId,
      controllerDisplay,
      platform,
      scope,
      counter,
      recordedAtMs: params.recordedAtMs,
    });
  }

  async checkpoint(certificateIdHex) {
    await this.#ready;
    const normalized = normalizeHexId(certificateIdHex, "certificateIdHex");
    const entry = this.#entries.get(normalized) ?? null;
    return entry ? cloneValue(entry) : null;
  }

  async snapshot() {
    await this.#ready;
    return Array.from(this.#entries.values(), (entry) => cloneValue(entry));
  }

  async clear() {
    await this.#ready;
    return this.#runLocked(async () => {
      this.#entries.clear();
      await this.#persistLocked();
    });
  }

  async refresh() {
    await this.#ready;
    return this.#runLocked(async () => {
      await this.#loadFromDisk();
    });
  }

  async #initialize() {
    if (this.#storage === STORAGE_FILE) {
      await this.#loadFromDisk();
    }
  }

  async #loadFromDisk() {
    if (this.#storage !== STORAGE_FILE || !this.#storagePath) {
      this.#entries = new Map();
      return;
    }
    try {
      const data = await fs.readFile(this.#storagePath, "utf8");
      if (!data.trim()) {
        this.#entries = new Map();
        return;
      }
      const parsed = JSON.parse(data);
      if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
        throw new OfflineCounterJournalError("offline counter journal must be a JSON object", {
          code: "invalid_storage",
        });
      }
      const entries = new Map();
      for (const [key, value] of Object.entries(parsed)) {
        if (!value || typeof value !== "object") {
          continue;
        }
        const certificateIdHex = normalizeHexId(key, "certificateIdHex");
        entries.set(certificateIdHex, value);
      }
      this.#entries = entries;
    } catch (error) {
      if (error?.code === "ENOENT") {
        this.#entries = new Map();
        return;
      }
      if (error instanceof OfflineCounterJournalError) {
        throw error;
      }
      throw new OfflineCounterJournalError("failed to load offline counter journal", {
        code: "invalid_storage",
        cause: error,
      });
    }
  }

  async #persistLocked() {
    if (this.#storage !== STORAGE_FILE || !this.#storagePath) {
      return;
    }
    const dir = path.dirname(this.#storagePath);
    await fs.mkdir(dir, { recursive: true });
    const serialized = {};
    const keys = Array.from(this.#entries.keys()).sort();
    for (const key of keys) {
      serialized[key] = this.#entries.get(key);
    }
    const payload = `${JSON.stringify(serialized, null, 2)}\n`;
    await fs.writeFile(this.#storagePath, payload, "utf8");
  }

  #runLocked(callback) {
    this.#mutex = this.#mutex.then(() => callback()).catch((error) => {
      this.#mutex = Promise.resolve();
      throw error;
    });
    return this.#mutex;
  }

  #storage;
  #storagePath;
  #entries;
  #mutex;
  #ready;
}

function normalizePlatform(value) {
  const normalized =
    typeof value === "string" ? value.trim().toLowerCase() : value;
  if (normalized === OfflineCounterPlatform.APPLE_KEY) {
    return OfflineCounterPlatform.APPLE_KEY;
  }
  if (normalized === OfflineCounterPlatform.ANDROID_SERIES) {
    return OfflineCounterPlatform.ANDROID_SERIES;
  }
  throw new OfflineCounterJournalError(`invalid counter platform ${value}`, {
    code: "invalid_platform",
  });
}
