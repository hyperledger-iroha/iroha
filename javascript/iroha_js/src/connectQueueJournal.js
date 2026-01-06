import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";

import {
  ConnectDirection,
  ConnectJournalRecord,
  ConnectJournalError,
} from "./connectJournalRecord.js";

const DEFAULT_MAX_RECORDS = 32;
const DEFAULT_MAX_BYTES = 1 << 20;
const DEFAULT_RETENTION_MS = 24 * 60 * 60 * 1000;
const DEFAULT_DB_NAME = "iroha_connect_journal";
const DEFAULT_DB_VERSION = 1;

export class ConnectQueueJournal {
  constructor(sessionId, options = {}) {
    if (sessionId === undefined || sessionId === null) {
      throw new ConnectJournalError("sessionId must be provided");
    }
    this.#sessionId = normalizeSessionId(sessionId);
    this.#config = {
      maxRecordsPerQueue: normalizePositive(
        options.maxRecordsPerQueue ?? DEFAULT_MAX_RECORDS,
        "maxRecordsPerQueue",
      ),
      maxBytesPerQueue: normalizePositive(
        options.maxBytesPerQueue ?? DEFAULT_MAX_BYTES,
        "maxBytesPerQueue",
      ),
      retentionMs: normalizePositive(options.retentionMs ?? DEFAULT_RETENTION_MS, "retentionMs"),
      indexedDbName: options.indexedDbName ?? DEFAULT_DB_NAME,
      indexedDbVersion: options.indexedDbVersion ?? DEFAULT_DB_VERSION,
      storage: options.storage ?? "auto",
      indexedDbFactory: options.indexedDbFactory ?? globalThis.indexedDB,
    };
    this.#mutex = Promise.resolve();
    this.#ready = this.#initialize();
  }

  async append(direction, sequence, ciphertext, options = {}) {
    await this.#ready;
    const record = ConnectJournalRecord.fromCiphertext({
      direction,
      sequence,
      ciphertext,
      retentionMs: options.ttlMs ?? options.retentionMs ?? this.#config.retentionMs,
      receivedAtMs: options.receivedAtMs,
    });
    const nowMs = options.nowMs ?? record.receivedAtMs ?? currentTimestamp();
    return this.#runLocked(() => this.#store.append(record, nowMs));
  }

  async records(direction, options = {}) {
    await this.#ready;
    const canonical = normalizeDirection(direction);
    return this.#runLocked(() =>
      this.#store.records(canonical, options.nowMs ?? currentTimestamp()),
    );
  }

  async popOldest(direction, count = 1, options = {}) {
    await this.#ready;
    if (!Number.isInteger(count) || count <= 0) {
      throw new ConnectJournalError("pop count must be a positive integer");
    }
    const canonical = normalizeDirection(direction);
    return this.#runLocked(() =>
      this.#store.popOldest(canonical, count, options.nowMs ?? currentTimestamp()),
    );
  }

  get sessionKey() {
    return this.#sessionKey;
  }

  get fallbackError() {
    return this.#fallbackError;
  }

  async #initialize() {
    this.#sessionKey = await hashSessionId(this.#sessionId);
    const { store, fallbackError } = await selectStore(this.#sessionKey, this.#config);
    this.#store = store;
    this.#fallbackError = fallbackError;
  }

  #runLocked(callback) {
    this.#mutex = this.#mutex.then(() => callback()).catch((error) => {
      this.#mutex = Promise.resolve();
      throw error;
    });
    return this.#mutex;
  }

  #sessionId;
  #sessionKey;
  #config;
  #store;
  #fallbackError;
  #ready;
  #mutex;
}

class MemoryJournalStore {
  constructor(config) {
    this.config = config;
    this.queues = {
      [ConnectDirection.APP_TO_WALLET]: [],
      [ConnectDirection.WALLET_TO_APP]: [],
    };
  }

  async append(record, nowMs) {
    const queue = this.queues[record.direction];
    queue.push({
      encoded: record.encode(),
      expiresAtMs: record.expiresAtMs,
    });
    this.#prune(record.direction, nowMs);
  }

  async records(direction, nowMs) {
    this.#prune(direction, nowMs);
    return this.queues[direction].map((entry) =>
      ConnectJournalRecord.decode(entry.encoded).record,
    );
  }

  async popOldest(direction, count, nowMs) {
    this.#prune(direction, nowMs);
    const queue = this.queues[direction];
    const removed = queue.splice(0, Math.min(count, queue.length));
    return removed.map((entry) => ConnectJournalRecord.decode(entry.encoded).record);
  }

  #prune(direction, nowMs) {
    const queue = this.queues[direction];
    while (queue.length > 0 && queue[0].expiresAtMs <= nowMs) {
      queue.shift();
    }
    while (queue.length > this.config.maxRecordsPerQueue) {
      queue.shift();
    }
    let totalBytes = queue.reduce((sum, entry) => sum + entry.encoded.length, 0);
    while (totalBytes > this.config.maxBytesPerQueue && queue.length > 0) {
      const removed = queue.shift();
      totalBytes -= removed.encoded.length;
    }
  }
}

class IndexedDbJournalStore {
  static async open(sessionKey, config) {
    const db = await openIndexedDb(
      config.indexedDbName,
      config.indexedDbVersion,
      config.indexedDbFactory,
    );
    return new IndexedDbJournalStore(db, sessionKey, config);
  }

  constructor(db, sessionKey, config) {
    this.db = db;
    this.sessionKey = sessionKey;
    this.config = config;
  }

  async append(record, nowMs) {
    const encoded = record.encode();
    await this.#withStore(async (store) => {
      await requestToPromise(
        store.add({
          sessionKey: this.sessionKey,
          direction: record.direction,
          encoded,
          encodedLength: encoded.length,
          expiresAtMs: record.expiresAtMs,
        }),
      );
      await this.#collectAndPrune(store, record.direction, nowMs);
    });
  }

  async records(direction, nowMs) {
    return this.#withStore(async (store) => {
      const entries = await this.#collectAndPrune(store, direction, nowMs);
      return entries.map(decodeEntry);
    });
  }

  async popOldest(direction, count, nowMs) {
    return this.#withStore(async (store) => {
      const entries = await this.#collectAndPrune(store, direction, nowMs);
      const removed = entries.splice(0, Math.min(count, entries.length));
      for (const entry of removed) {
        await requestToPromise(store.delete(entry.id));
      }
      return removed.map(decodeEntry);
    });
  }

  async #collectAndPrune(store, direction, nowMs) {
    const entries = await this.#collectEntries(store, direction, nowMs);
    return this.#enforceLimits(store, entries);
  }

  async #collectEntries(store, direction, nowMs) {
    const index = store.index("by_session_direction_id");
    const keyRange = resolveKeyRange();
    const range = keyRange.bound(
      [this.sessionKey, direction, 0],
      [this.sessionKey, direction, Number.MAX_SAFE_INTEGER],
    );
    const entries = [];
    await iterateCursor(index.openCursor(range), async (cursor) => {
      const { encoded, encodedLength, expiresAtMs } = cursor.value;
      if (expiresAtMs <= nowMs) {
        await requestToPromise(cursor.delete());
        return;
      }
      entries.push({ id: cursor.primaryKey, encoded, encodedLength, expiresAtMs });
    });
    return entries;
  }

  async #enforceLimits(store, entries) {
    let totalBytes = entries.reduce((sum, entry) => sum + entry.encodedLength, 0);
    while (entries.length > this.config.maxRecordsPerQueue) {
      const removed = entries.shift();
      if (removed) {
        totalBytes -= removed.encodedLength;
        await requestToPromise(store.delete(removed.id));
      }
    }
    while (totalBytes > this.config.maxBytesPerQueue && entries.length > 0) {
      const removed = entries.shift();
      if (!removed) break;
      totalBytes -= removed.encodedLength;
      await requestToPromise(store.delete(removed.id));
    }
    return entries;
  }

  async #withStore(callback) {
    const tx = this.db.transaction("records", "readwrite");
    const store = tx.objectStore("records");
    const result = await callback(store);
    await transactionComplete(tx);
    return result;
  }
}

async function selectStore(sessionKey, config) {
  if (config.storage !== "memory" && hasIndexedDbSupport(config.indexedDbFactory)) {
    try {
      const store = await IndexedDbJournalStore.open(sessionKey, config);
      return { store, fallbackError: undefined };
    } catch (error) {
      return { store: new MemoryJournalStore(config), fallbackError: error };
    }
  }
  return { store: new MemoryJournalStore(config), fallbackError: undefined };
}

function normalizeSessionId(input) {
  if (input instanceof Uint8Array) {
    return new Uint8Array(input);
  }
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }
  if (input instanceof ArrayBuffer) {
    return new Uint8Array(input);
  }
  if (typeof input === "string") {
    return decodeBase64Url(input.trim());
  }
  throw new ConnectJournalError("sessionId must be binary data or base64url string");
}

async function hashSessionId(bytes) {
  const subtle = globalThis.crypto?.subtle;
  if (subtle) {
    const digest = await subtle.digest("SHA-256", bytes);
    return toHex(new Uint8Array(digest));
  }
  const hash = createHash("sha256");
  hash.update(Buffer.from(bytes));
  return hash.digest("hex");
}

function decodeBase64Url(input) {
  if (!input) {
    throw new ConnectJournalError("sessionId must not be empty");
  }
  const trimmed = input.trim();
  if (!trimmed) {
    throw new ConnectJournalError("sessionId must not be empty");
  }
  const normalized = trimmed.replace(/-/g, "+").replace(/_/g, "/");
  let padded = normalized;
  const paddingIndex = normalized.indexOf("=");
  if (paddingIndex !== -1) {
    const head = normalized.slice(0, paddingIndex);
    const padding = normalized.slice(paddingIndex);
    if (!/^[0-9A-Za-z+/]*$/.test(head) || !/^={1,2}$/.test(padding)) {
      throw new ConnectJournalError("sessionId must be a valid base64url string");
    }
    if (normalized.length % 4 !== 0) {
      throw new ConnectJournalError("sessionId must be a valid base64url string");
    }
  } else {
    if (!/^[0-9A-Za-z+/]+$/.test(normalized) || normalized.length % 4 === 1) {
      throw new ConnectJournalError("sessionId must be a valid base64url string");
    }
    const padLength = (4 - (normalized.length % 4)) % 4;
    padded = normalized + "=".repeat(padLength);
  }
  const decoded = Buffer.from(padded, "base64");
  if (decoded.toString("base64") !== padded) {
    throw new ConnectJournalError("sessionId must be a valid base64url string");
  }
  return new Uint8Array(decoded);
}

function hasIndexedDbSupport(factory) {
  return !!factory && typeof factory.open === "function";
}

function resolveKeyRange() {
  const keyRange =
    typeof globalThis !== "undefined" && globalThis.IDBKeyRange
      ? globalThis.IDBKeyRange
      : null;
  if (keyRange) {
    return keyRange;
  }
  throw new ConnectJournalError("IndexedDB key range is unavailable");
}

async function openIndexedDb(name, version, factory) {
  if (!factory) {
    throw new ConnectJournalError("IndexedDB factory is unavailable");
  }
  return new Promise((resolve, reject) => {
    const request = factory.open(name, version);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains("records")) {
        const store = db.createObjectStore("records", { keyPath: "id", autoIncrement: true });
        store.createIndex("by_session_direction_id", ["sessionKey", "direction", "id"]);
      }
    };
    request.onerror = () => reject(request.error ?? new Error("failed to open IndexedDB"));
    request.onsuccess = () => resolve(request.result);
  });
}

function normalizeDirection(direction) {
  const key = typeof direction === "string" ? direction.trim().toLowerCase() : direction;
  switch (key) {
    case ConnectDirection.APP_TO_WALLET:
    case "app-to-wallet":
    case "app_to_wallet":
    case "app":
      return ConnectDirection.APP_TO_WALLET;
    case ConnectDirection.WALLET_TO_APP:
    case "wallet-to-app":
    case "wallet_to_app":
    case "wallet":
      return ConnectDirection.WALLET_TO_APP;
    default:
      throw new ConnectJournalError(`Unknown Connect queue direction: ${direction}`);
  }
}

function normalizePositive(value, name) {
  const number = Number(value);
  if (!Number.isFinite(number) || number <= 0) {
    throw new ConnectJournalError(`${name} must be a positive number`);
  }
  return Math.trunc(number);
}

function requestToPromise(request) {
  return new Promise((resolve, reject) => {
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error("IndexedDB request failed"));
  });
}

function transactionComplete(tx) {
  return new Promise((resolve, reject) => {
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error ?? new Error("IndexedDB transaction failed"));
    tx.onabort = () => reject(tx.error ?? new Error("IndexedDB transaction aborted"));
  });
}

async function iterateCursor(request, callback) {
  return new Promise((resolve, reject) => {
    request.onerror = () => reject(request.error ?? new Error("IndexedDB cursor error"));
    request.onsuccess = async (event) => {
      const cursor = event.target.result;
      if (!cursor) {
        resolve();
        return;
      }
      try {
        await callback(cursor);
        cursor.continue();
      } catch (error) {
        reject(error);
      }
    };
  });
}

function decodeEntry(entry) {
  const { record } = ConnectJournalRecord.decode(new Uint8Array(entry.encoded));
  return record;
}

function currentTimestamp() {
  return Date.now();
}

function toHex(bytes) {
  return Array.from(bytes, (byte) => byte.toString(16).padStart(2, "0")).join("");
}
