
globalThis.__adapterRecords = new Map();
globalThis.__adapterMode = "normal";
globalThis.__soracloudSharedStateAdapter = {
  get(key) {
    return globalThis.__adapterRecords.has(key)
      ? globalThis.__adapterRecords.get(key)
      : null;
  },
  put(key, value) {
    globalThis.__adapterRecords.set(key, value);
  },
  delete(key) {
    globalThis.__adapterRecords.delete(key);
  },
  putIfAbsent(key, value) {
    if (globalThis.__adapterMode === "bad-put-if-absent") {
      return "true";
    }
    if (globalThis.__adapterRecords.has(key)) {
      return false;
    }
    globalThis.__adapterRecords.set(key, value);
    return true;
  },
  entries(prefix) {
    if (globalThis.__adapterMode === "entries-not-array") {
      return "not-an-array";
    }
    if (globalThis.__adapterMode === "bad-entry-shape") {
      return [["/state/adapter/a"]];
    }
    if (globalThis.__adapterMode === "empty-entry-key") {
      return [["   ", {}]];
    }
    return Array.from(globalThis.__adapterRecords.entries()).concat([
      ["/unrelated/key", { ignored: true }]
    ]);
  }
};
