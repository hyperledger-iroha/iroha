
function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertThrows(fn, expectedMessage) {
  try {
    fn();
  } catch (error) {
    if (String(error?.message ?? error).includes(expectedMessage)) {
      return;
    }
    throw new Error(`unexpected error: ${error?.stack ?? String(error)}`);
  }
  throw new Error(`expected error containing: ${expectedMessage}`);
}

assert(SHARED_STATE_ADAPTER === globalThis.__soracloudSharedStateAdapter, "configured shared adapter should be used");
assert(stateGet("/state/adapter/missing") === null, "missing adapter values should read as null");

statePut("/state/adapter/b", { z: 2, a: 1 });
assert(
  JSON.stringify(globalThis.__adapterRecords.get("/state/adapter/b")) === JSON.stringify({ a: 1, z: 2 }),
  "statePut should canonicalize values before calling adapter.put"
);
assert(statePutIfAbsent("/state/adapter/b", { replaced: true }) === false, "existing adapter key should not be replaced");
assert(statePutIfAbsent("/state/adapter/a", { nested: { b: 2, a: 1 } }) === true, "new adapter key should be inserted");
assert(
  JSON.stringify(stateEntries("/state/adapter/").map(([key]) => key)) === JSON.stringify(["/state/adapter/a", "/state/adapter/b"]),
  "adapter entries should be sorted and prefix-filtered"
);
assert(
  JSON.stringify(stateGet("/state/adapter/a")) === JSON.stringify({ nested: { a: 1, b: 2 } }),
  "stateGet should canonicalize adapter-returned values"
);
stateDelete("/state/adapter/b");
assert(stateGet("/state/adapter/b") === null, "stateDelete should call adapter.delete");

globalThis.__adapterMode = "bad-put-if-absent";
assertThrows(
  () => statePutIfAbsent("/state/adapter/bad-insert", {}),
  "shared state adapter putIfAbsent(key, value) must return boolean"
);
globalThis.__adapterMode = "entries-not-array";
assertThrows(
  () => stateEntries("/state/adapter/"),
  "shared state adapter entries(prefix) must return [key, value][]"
);
globalThis.__adapterMode = "bad-entry-shape";
assertThrows(
  () => stateEntries("/state/adapter/"),
  "shared state adapter entries(prefix) must return [key, value][]"
);
globalThis.__adapterMode = "empty-entry-key";
assertThrows(
  () => stateEntries("/state/adapter/"),
  "shared state adapter entry keys must be non-empty strings"
);
