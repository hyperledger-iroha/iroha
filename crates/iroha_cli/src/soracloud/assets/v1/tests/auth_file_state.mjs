
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

assert(
  JSON.stringify(readAuthStateSnapshot()) === JSON.stringify({ schema_version: AUTH_STATE_SCHEMA_VERSION, records: {} }),
  "missing state file should decode as an empty auth snapshot"
);

statePut("/state/test/z", {
  z: 1,
  a: { d: 4, c: 3 },
  list: [{ b: 2, a: 1 }]
});
const storedRaw = fs.readFileSync(STATE_FILE_PATH, "utf8");
const stored = JSON.parse(storedRaw);
const expectedStoredValue = { a: { c: 3, d: 4 }, list: [{ a: 1, b: 2 }], z: 1 };
assert(
  JSON.stringify(stored.records["/state/test/z"]) === JSON.stringify(expectedStoredValue),
  `statePut should canonicalize nested values: ${storedRaw}`
);
assert(
  JSON.stringify(stateGet("/state/test/z")) === JSON.stringify(expectedStoredValue),
  "stateGet should return the canonicalized value"
);
assert(statePutIfAbsent("/state/test/z", { replaced: true }) === false, "existing key must not be replaced");
assert(statePutIfAbsent("/state/test/a", { value: 1 }) === true, "new key must be inserted");
assert(
  JSON.stringify(stateEntries("/state/test/").map(([key]) => key)) === JSON.stringify(["/state/test/a", "/state/test/z"]),
  "stateEntries should be sorted and prefix-filtered"
);
stateDelete("/state/test/z");
assert(stateGet("/state/test/z") === null, "stateDelete should remove existing records");

fs.mkdirSync(STATE_FILE_LOCK_DIR, { recursive: true });
const staleLockTime = new Date(Date.now() - STATE_FILE_LOCK_STALE_MS - 1000);
fs.utimesSync(STATE_FILE_LOCK_DIR, staleLockTime, staleLockTime);
statePut("/state/test/stale-lock", { ok: true });
assert(
  stateGet("/state/test/stale-lock")?.ok === true,
  "statePut should recover from stale file locks"
);
assert(!fs.existsSync(STATE_FILE_LOCK_DIR), "state lock should be released after mutation");

fs.writeFileSync(STATE_FILE_PATH, "  ");
assert(
  JSON.stringify(readAuthStateSnapshot()) === JSON.stringify({ schema_version: AUTH_STATE_SCHEMA_VERSION, records: {} }),
  "empty state file should decode as an empty auth snapshot"
);
fs.writeFileSync(STATE_FILE_PATH, JSON.stringify({ schema_version: "wrong", records: {} }));
assertThrows(() => readAuthStateSnapshot(), "invalid auth state snapshot shape");
