import { test } from "node:test";
import assert from "node:assert/strict";

import {
  assertNonBlankString,
  normalizeStatusSet,
  normalizeTransactionStatusScope,
  readHeaderValue,
} from "../src/toriiClientPrimitives.js";

test("Torii client primitives normalize status options", () => {
  assert.equal(assertNonBlankString(" value ", "value"), "value");
  assert.throws(() => assertNonBlankString(" ", "value"), /non-empty string/u);
  assert.deepEqual([...normalizeStatusSet(null, ["Rejected"])], ["Rejected"]);
  assert.deepEqual([...normalizeStatusSet(["Applied"], [])], ["Applied"]);
  assert.equal(normalizeTransactionStatusScope(undefined, "scope"), "global");
  assert.equal(normalizeTransactionStatusScope("local", "scope"), "local");
  assert.throws(
    () => normalizeTransactionStatusScope("peer", "scope"),
    /local, global/u,
  );
});

test("readHeaderValue supports Fetch and plain header collections", () => {
  assert.equal(readHeaderValue(new Headers({ "x-test": "fetch" }), "X-Test"), "fetch");
  assert.equal(readHeaderValue(new Map([["x-test", "map"]]), "X-Test"), "map");
  assert.equal(readHeaderValue({ "x-test": "object" }, "X-Test"), "object");
  assert.equal(readHeaderValue(null, "X-Test"), null);
});
