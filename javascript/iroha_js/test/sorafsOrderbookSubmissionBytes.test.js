// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

import assert from "node:assert/strict";
import test from "node:test";
import {
  SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1,
  snapshotSorafsOrderbookSubmissionBytes,
} from "../src/sorafsOrderbookSubmissionBytes.js";

test("orderbook byte custody copies only the selected exact view", () => {
  for (const makeView of [
    (buffer) => buffer.subarray(1, 4),
    (buffer) => new Uint8Array(buffer.buffer, buffer.byteOffset + 1, 3),
    (buffer) => new DataView(buffer.buffer, buffer.byteOffset + 1, 3),
  ]) {
    const backing = Buffer.from([9, 1, 2, 3, 9]);
    const owned = snapshotSorafsOrderbookSubmissionBytes(makeView(backing), "test");
    backing.fill(0xff);
    assert.deepEqual(owned, Buffer.from([1, 2, 3]));
    owned.fill(0);
    assert.deepEqual(backing, Buffer.alloc(5, 0xff));
  }
  const input = Uint8Array.of(4, 5).buffer;
  const owned = snapshotSorafsOrderbookSubmissionBytes(input, "test");
  new Uint8Array(input).fill(0xff);
  assert.deepEqual(owned, Buffer.from([4, 5]));
});

test("orderbook byte custody preserves the exact transaction byte cap", () => {
  const maximum = Buffer.alloc(SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1, 0x5a);
  const owned = snapshotSorafsOrderbookSubmissionBytes(maximum, "test");
  assert.equal(SORAFS_ORDERBOOK_TRANSACTION_MAX_BYTES_V1, 2 * 1024 * 1024);
  assert.deepEqual(owned, maximum);
  assert.notEqual(owned.buffer, maximum.buffer);
  for (const bytes of [Buffer.alloc(0), Buffer.alloc(maximum.length + 1)]) {
    assert.throws(() => snapshotSorafsOrderbookSubmissionBytes(bytes, "test"),
      { name: "RangeError", message: "test.signedTransaction must contain 1..2097152 bytes" });
  }
});

test("orderbook byte custody rejects coercions and non-byte inputs", () => {
  for (const value of [undefined, null, "010203", [1, 2, 3], { byteLength: 3 },
    { valueOf() { assert.fail("non-byte input must never be coerced"); } }]) {
    assert.throws(() => snapshotSorafsOrderbookSubmissionBytes(value, "test"),
      { name: "TypeError", message: "test.signedTransaction must be exact bytes" });
  }
});
