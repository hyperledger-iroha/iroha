import test from "node:test";
import assert from "node:assert/strict";

import { ConnectQueueJournal } from "../src/connectQueueJournal.js";
import { ConnectDirection } from "../src/connectJournalRecord.js";

class FakeIndexedDbFactory {
  constructor() {
    this.databases = new Map();
  }

  open(name, _version) {
    const request = new FakeRequest();
    queueMicrotask(() => {
      let db = this.databases.get(name);
      if (!db) {
        db = new FakeDatabase();
        this.databases.set(name, db);
        request.result = db;
        request.onupgradeneeded?.({ target: request });
      }
      request.result = db;
      request.onsuccess?.({ target: request });
    });
    return request;
  }
}

class FakeDatabase {
  constructor() {
    this.stores = new Map();
  }

  get objectStoreNames() {
    return {
      contains: (name) => this.stores.has(name),
    };
  }

  createObjectStore(name) {
    const store = new FakeObjectStore();
    this.stores.set(name, store);
    return store;
  }

  transaction(name) {
    const store = this.stores.get(name);
    if (!store) {
      throw new Error(`unknown object store ${name}`);
    }
    return new FakeTransaction(store);
  }
}

class FakeTransaction {
  constructor(store) {
    this.store = store;
    this._completed = false;
    queueMicrotask(() => {
      this._completed = true;
      this._oncomplete?.();
    });
  }

  set oncomplete(handler) {
    this._oncomplete = handler;
    if (this._completed && handler) {
      queueMicrotask(() => handler());
    }
  }

  get oncomplete() {
    return this._oncomplete;
  }

  objectStore() {
    return this.store;
  }
}

class FakeObjectStore {
  constructor() {
    this.entries = [];
    this.autoIncrement = 1;
  }

  add(value) {
    const request = new FakeRequest();
    queueMicrotask(() => {
      this.entries.push({ id: this.autoIncrement, value: { ...value } });
      request.result = this.autoIncrement;
      this.autoIncrement += 1;
      request.onsuccess?.({ target: request });
    });
    return request;
  }

  delete(id) {
    const request = new FakeRequest();
    queueMicrotask(() => {
      const index = this.entries.findIndex((entry) => entry.id === id);
      if (index >= 0) {
        this.entries.splice(index, 1);
      }
      request.onsuccess?.({ target: request });
    });
    return request;
  }

  createIndex() {
    return new FakeIndex(this.entries);
  }

  index() {
    return new FakeIndex(this.entries);
  }
}

class FakeIndex {
  constructor(entries) {
    this.entries = entries;
  }

  openCursor(range) {
    const [sessionKey, direction] = range.lower;
    const matches = this.entries
      .filter(
        (entry) =>
          entry.value.sessionKey === sessionKey && entry.value.direction === direction,
      )
      .sort((a, b) => a.id - b.id);
    return new FakeCursorRequest(matches);
  }
}

class FakeCursorRequest {
  constructor(entries) {
    this.entries = entries;
    this.index = 0;
    queueMicrotask(() => this.#dispatch());
  }

  #dispatch() {
    if (this.index >= this.entries.length) {
      this.onsuccess?.({ target: { result: null } });
      return;
    }
    const entry = this.entries[this.index];
    const cursor = new FakeCursor(
      entry,
      () => {
        this.index += 1;
        this.#dispatch();
      },
      () => {
        const idx = this.entries.indexOf(entry);
        if (idx >= 0) {
          this.entries.splice(idx, 1);
        }
      },
    );
    this.onsuccess?.({ target: { result: cursor } });
  }
}

class FakeCursor {
  constructor(entry, advance, remove) {
    this.primaryKey = entry.id;
    this.value = entry.value;
    this._advance = advance;
    this._remove = remove;
  }

  continue() {
    this._advance();
  }

  delete() {
    this._remove();
    return new FakeRequest().resolve(undefined);
  }
}

class FakeRequest {
  resolve(result) {
    this.result = result;
    queueMicrotask(() => this.onsuccess?.({ target: this }));
    return this;
  }
}

globalThis.IDBKeyRange ??= {
  bound(lower, upper) {
    return { lower, upper };
  },
};

test("memory journal round-trip", async () => {
  const journal = new ConnectQueueJournal("AQIDBA", {
    storage: "memory",
    retentionMs: 1_000,
  });
  await journal.append(ConnectDirection.APP_TO_WALLET, 1, new Uint8Array([1, 2, 3]), {
    receivedAtMs: 100,
  });
  const records = await journal.records(ConnectDirection.APP_TO_WALLET, { nowMs: 150 });
  assert.equal(records.length, 1);
  assert.equal(records[0].sequence, 1n);
  assert.equal(records[0].ciphertext.length, 3);
});

test("memory journal pop oldest removes entries", async () => {
  const journal = new ConnectQueueJournal("AgMEBQ", {
    storage: "memory",
    retentionMs: 5_000,
  });
  for (let seq = 1; seq <= 3; seq += 1) {
    await journal.append(ConnectDirection.APP_TO_WALLET, seq, new Uint8Array([seq]), {
      receivedAtMs: 5 * seq,
    });
  }
  const removed = await journal.popOldest(ConnectDirection.APP_TO_WALLET, 2, { nowMs: 50 });
  assert.deepEqual(
    removed.map((record) => Number(record.sequence)),
    [1, 2],
  );
  const remaining = await journal.records(ConnectDirection.APP_TO_WALLET, { nowMs: 60 });
  assert.equal(remaining.length, 1);
  assert.equal(Number(remaining[0].sequence), 3);
});

test("memory journal enforces limits", async () => {
  const journal = new ConnectQueueJournal("AQQDAQ", {
    storage: "memory",
    retentionMs: 5_000,
    maxRecordsPerQueue: 2,
    maxBytesPerQueue: 512,
  });
  for (let seq = 1; seq <= 4; seq += 1) {
    await journal.append(ConnectDirection.WALLET_TO_APP, seq, new Uint8Array([seq, seq]), {
      receivedAtMs: 10 * seq,
    });
  }
  const records = await journal.records(ConnectDirection.WALLET_TO_APP, { nowMs: 100 });
  assert.equal(records.length, 2);
  assert.deepEqual(
    records.map((entry) => Number(entry.sequence)),
    [3, 4],
  );
});

test("indexeddb journal persists entries", async () => {
  const journal = new ConnectQueueJournal("AQIDBAUG", {
    retentionMs: 1_000,
    maxRecordsPerQueue: 4,
    indexedDbFactory: new FakeIndexedDbFactory(),
  });
  await journal.append(ConnectDirection.APP_TO_WALLET, 1, new Uint8Array([9, 9]), {
    receivedAtMs: 25,
  });
  await journal.append(ConnectDirection.APP_TO_WALLET, 2, new Uint8Array([8, 8]), {
    receivedAtMs: 35,
  });
  const records = await journal.records(ConnectDirection.APP_TO_WALLET, { nowMs: 50 });
  assert.equal(records.length, 2);
  assert.deepEqual(
    records.map((record) => Number(record.sequence)),
    [1, 2],
  );
});

test("connect journal rejects invalid session id strings", () => {
  assert.throws(
    () => new ConnectQueueJournal("AQIDB*"),
    (error) => error?.name === "ConnectJournalError",
  );
});

test("connect journal rejects empty binary session ids", () => {
  assert.throws(
    () => new ConnectQueueJournal(new Uint8Array()),
    (error) => error?.name === "ConnectJournalError",
  );
});

test("connect journal rejects non-byte session ids", () => {
  assert.throws(
    () => new ConnectQueueJournal([256]),
    (error) => error?.name === "ConnectJournalError",
  );
});

test("connect journal accepts array-like session ids", async () => {
  const journal = new ConnectQueueJournal([1, 2, 3], { storage: "memory" });
  const records = await journal.records(ConnectDirection.APP_TO_WALLET);
  assert.equal(records.length, 0);
});

test("connect journal rejects fractional retention", () => {
  assert.throws(
    () =>
      new ConnectQueueJournal("AQIDBA", {
        retentionMs: 1.5,
      }),
    (error) => error?.name === "ConnectJournalError" && /retentionMs/.test(error.message),
  );
});

test("indexeddb journal drops corrupted entries and keeps valid records", async () => {
  const factory = new FakeIndexedDbFactory();
  const journal = new ConnectQueueJournal("AQIDBAUGBwg", {
    retentionMs: 5_000,
    maxRecordsPerQueue: 8,
    indexedDbFactory: factory,
    indexedDbName: "iroha_connect_corrupted_records",
  });
  await journal.append(ConnectDirection.APP_TO_WALLET, 1, new Uint8Array([0x10]), {
    receivedAtMs: 10,
  });
  await journal.append(ConnectDirection.APP_TO_WALLET, 2, new Uint8Array([0x20]), {
    receivedAtMs: 20,
  });

  const db = factory.databases.get("iroha_connect_corrupted_records");
  assert.ok(db, "indexeddb test database should exist");
  const store = db.stores.get("records");
  assert.ok(store, "records object store should exist");
  store.entries[0].value.encoded = new Uint8Array([0x00, 0x01, 0x02]);
  store.entries[0].value.encodedLength = 3;

  const records = await journal.records(ConnectDirection.APP_TO_WALLET, { nowMs: 100 });
  assert.deepEqual(
    records.map((record) => Number(record.sequence)),
    [2],
  );

  const popped = await journal.popOldest(ConnectDirection.APP_TO_WALLET, 2, { nowMs: 150 });
  assert.deepEqual(
    popped.map((record) => Number(record.sequence)),
    [2],
  );
});
