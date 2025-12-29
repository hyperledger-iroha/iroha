import test from "node:test";
import assert from "node:assert/strict";

import {
  ConnectDirection,
  ConnectJournalRecord,
  ConnectJournalError,
} from "../src/connectJournalRecord.js";

test("ConnectJournalRecord encode/decode roundtrip", () => {
  const ciphertext = new Uint8Array([0xde, 0xad, 0xbe, 0xef]);
  const record = new ConnectJournalRecord({
    direction: ConnectDirection.APP_TO_WALLET,
    sequence: 42n,
    ciphertext,
    receivedAtMs: 1_700_000_000_000,
    expiresAtMs: 1_700_000_100_000,
  });
  const encoded = record.encode();
  assert.equal(encoded.length, record.encodedLength);
  const { record: decoded, bytesConsumed } = ConnectJournalRecord.decode(encoded);
  assert.equal(bytesConsumed, encoded.length);
  assert.equal(decoded.direction, record.direction);
  assert.equal(decoded.sequence, record.sequence);
  assert.equal(decoded.receivedAtMs, record.receivedAtMs);
  assert.equal(decoded.expiresAtMs, record.expiresAtMs);
  assert.deepEqual(new Uint8Array(decoded.ciphertext), ciphertext);
  assert.equal(decoded.payloadHash.length, 32);
});

test("fromCiphertext applies retention automatically", () => {
  const start = Date.now();
  const record = ConnectJournalRecord.fromCiphertext({
    direction: ConnectDirection.WALLET_TO_APP,
    sequence: "123",
    ciphertext: new Uint8Array([1, 2, 3]),
    receivedAtMs: start,
    retentionMs: 5_000,
  });
  assert.equal(record.direction, ConnectDirection.WALLET_TO_APP);
  assert.equal(record.sequence, 123n);
  assert.equal(record.receivedAtMs, start);
  assert.equal(record.expiresAtMs, start + 5_000);
});

test("decode rejects tampered payloads", () => {
  const ciphertext = new Uint8Array([0x01, 0x02]);
  const encoded = new ConnectJournalRecord({
    direction: ConnectDirection.APP_TO_WALLET,
    sequence: 1n,
    ciphertext,
    receivedAtMs: 100,
    expiresAtMs: 200,
  }).encode();
  encoded[encoded.length - 1] ^= 0xff;
  assert.throws(
    () => ConnectJournalRecord.decode(encoded),
    (error) =>
      error instanceof ConnectJournalError &&
      /checksum mismatch/i.test(error.message),
  );
});

test("decode rejects schema mismatch", () => {
  const ciphertext = new Uint8Array([0xaa, 0xbb]);
  const record = new ConnectJournalRecord({
    direction: ConnectDirection.APP_TO_WALLET,
    sequence: 9n,
    ciphertext,
    receivedAtMs: 10,
    expiresAtMs: 20,
  }).encode();
  record[10] ^= 0xff;
  assert.throws(
    () => ConnectJournalRecord.decode(record),
    (error) =>
      error instanceof ConnectJournalError &&
      /schema hash mismatch/i.test(error.message),
  );
});
