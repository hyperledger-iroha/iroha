import { test } from "node:test";
import assert from "node:assert/strict";

import { ConnectRetryPolicy } from "../src/connectRetryPolicy.js";

const ZERO_SEED = new Uint8Array(32);
const SEQ_SEED = Uint8Array.from(Array.from({ length: 32 }, (_, i) => i));

test("connect retry cap saturates", () => {
  const policy = new ConnectRetryPolicy();
  assert.equal(policy.capMillis(0), 5_000);
  assert.equal(policy.capMillis(1), 10_000);
  assert.equal(policy.capMillis(2), 20_000);
  assert.equal(policy.capMillis(3), 40_000);
  assert.equal(policy.capMillis(4), 60_000);
  assert.equal(policy.capMillis(42), 60_000);
});

test("connect retry deterministic series for zero seed", () => {
  const policy = new ConnectRetryPolicy();
  const expected = [2_236, 4_203, 9_051, 15_827, 44_159, 3_907];
  const actual = expected.map((_, attempt) =>
    policy.delayMillis(attempt, ZERO_SEED),
  );
  assert.deepStrictEqual(actual, expected);
});

test("connect retry deterministic series for sequential seed", () => {
  const policy = new ConnectRetryPolicy();
  const expected = [4_133, 1_579, 16_071, 30_438, 7_169, 20_790];
  const actual = expected.map((_, attempt) =>
    policy.delayMillis(attempt, SEQ_SEED),
  );
  assert.deepStrictEqual(actual, expected);
});

test("connect retry delay bounded by cap", () => {
  const policy = new ConnectRetryPolicy();
  const seeds = [ZERO_SEED, SEQ_SEED];
  for (let attempt = 0; attempt < 12; attempt += 1) {
    const cap = policy.capMillis(attempt);
    for (const seed of seeds) {
      const value = policy.delayMillis(attempt, seed);
      assert.ok(value <= cap, `attempt ${attempt} exceeded cap`);
    }
  }
});

test("connect retry accepts array-like seeds", () => {
  const policy = new ConnectRetryPolicy();
  const buffer = ZERO_SEED.buffer.slice(0);
  const view = new DataView(buffer);
  const arraySeed = Array.from(ZERO_SEED);
  const values = [
    policy.delayMillis(0, buffer),
    policy.delayMillis(0, view),
    policy.delayMillis(0, arraySeed),
  ];
  for (const value of values) {
    assert.ok(value >= 0);
  }
});

test("connect retry rejects non-byte seeds", () => {
  const policy = new ConnectRetryPolicy();
  assert.throws(
    () => policy.delayMillis(0, [256]),
    (error) => error instanceof TypeError && /seed\[0\]/i.test(error.message),
  );
});
