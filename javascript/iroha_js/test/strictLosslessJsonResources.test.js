import test from "node:test";
import assert from "node:assert/strict";
import { parseStrictLosslessIntegerJson, parseStrictLosslessJson } from "../src/strictLosslessJson.js";

test("strict integer parsing avoids per-node traversal-path copies", () => {
  const source = `[${"1,".repeat(20_000)}1]`;
  const original = Array.prototype[Symbol.iterator];
  let iteratorCalls = 0;
  let result;
  try {
    Array.prototype[Symbol.iterator] = function () { iteratorCalls++; return original.call(this); };
    result = parseStrictLosslessIntegerJson(source, "large integer page");
  } finally { Array.prototype[Symbol.iterator] = original; }
  assert.equal(result.length, 20_001);
  assert.equal(iteratorCalls, 0, "integer-only decoding must not copy a path per JSON node");
});

test("metadata path stack restores exact siblings and array positions without widening numeric admission", () => {
  const options = { floatingPointPaths: [["manifest", "metadata"]] };
  const parsed = parseStrictLosslessJson('{"manifest":{"metadata":{"rows":[{"n":1.5},2e0]},"height":2},"other":[1,2]}', "metadata", options);
  assert.equal(parsed.manifest.metadata.rows[0].n, 1.5);
  assert.equal(parsed.manifest.metadata.rows[1], 2);
  for (const source of ['{"manifest":{"metadata":{"n":1.5},"height":2.0}}', '{"manifest":{"metadata":{"n":1.5}},"metadata":{"n":2e0}}', '{"manifest":{"metadata":{"n":1.5}},"other":[2.0]}']) {
    assert.throws(() => parseStrictLosslessJson(source, "metadata", options), /canonical integers/);
  }
});
