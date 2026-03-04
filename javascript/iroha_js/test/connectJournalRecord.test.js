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

test("ConnectJournalRecord header matches Norito v1 defaults", () => {
  const record = new ConnectJournalRecord({
    direction: ConnectDirection.APP_TO_WALLET,
    sequence: 1n,
    ciphertext: new Uint8Array([0x01]),
    receivedAtMs: 0,
    expiresAtMs: 1,
  });
  const encoded = record.encode();
  assert.equal(encoded[4], 0);
  assert.equal(encoded[5], 0);
  const schemaHex = Buffer.from(encoded.subarray(6, 22)).toString("hex");
  assert.equal(schemaHex, "bcf2f58a15121190bcf2f58a15121190");
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

test("ConnectJournalRecord accepts array-like ciphertext and payload hash", () => {
  const payloadHash = Array.from({ length: 32 }, () => 7);
  const record = new ConnectJournalRecord({
    direction: ConnectDirection.APP_TO_WALLET,
    sequence: 5,
    ciphertext: [9, 8, 7],
    payloadHash,
    receivedAtMs: 10,
    expiresAtMs: 20,
  });
  assert.deepEqual(Array.from(record.ciphertext), [9, 8, 7]);
  assert.equal(record.payloadHash.length, 32);
});

test("ConnectJournalRecord rejects non-byte array payloads", () => {
  assert.throws(
    () =>
      new ConnectJournalRecord({
        direction: ConnectDirection.APP_TO_WALLET,
        sequence: 5,
        ciphertext: [256],
        receivedAtMs: 10,
        expiresAtMs: 20,
      }),
    (error) =>
      error instanceof ConnectJournalError &&
      /must be a byte/i.test(error.message),
  );
});

test("decode accepts array-like payloads", () => {
  const encoded = new ConnectJournalRecord({
    direction: ConnectDirection.APP_TO_WALLET,
    sequence: 2n,
    ciphertext: new Uint8Array([1, 2, 3]),
    receivedAtMs: 100,
    expiresAtMs: 200,
  }).encode();
  const { record } = ConnectJournalRecord.decode(Array.from(encoded));
  assert.equal(record.sequence, 2n);
  assert.deepEqual(Array.from(record.ciphertext), [1, 2, 3]);
});

test("ConnectJournalRecord rejects non-integer inputs", () => {
  assert.throws(
    () =>
      new ConnectJournalRecord({
        direction: ConnectDirection.APP_TO_WALLET,
        sequence: 1.25,
        ciphertext: new Uint8Array([0x01]),
        receivedAtMs: 10,
        expiresAtMs: 20,
      }),
    (error) =>
      error instanceof ConnectJournalError &&
      /sequence must be a non-negative integer/i.test(error.message),
  );
  assert.throws(
    () =>
      new ConnectJournalRecord({
        direction: ConnectDirection.APP_TO_WALLET,
        sequence: 1n,
        ciphertext: new Uint8Array([0x01]),
        receivedAtMs: 10.5,
        expiresAtMs: 20,
      }),
    (error) =>
      error instanceof ConnectJournalError &&
      /receivedAtMs must be a non-negative integer/i.test(error.message),
  );
  assert.throws(
    () =>
      new ConnectJournalRecord({
        direction: ConnectDirection.APP_TO_WALLET,
        sequence: 1n,
        ciphertext: new Uint8Array([0x01]),
        receivedAtMs: 10,
        expiresAtMs: 20.5,
      }),
    (error) =>
      error instanceof ConnectJournalError &&
      /expiresAtMs must be a non-negative integer/i.test(error.message),
  );
});

test("ConnectJournalRecord rejects uint64 overflow sequences", () => {
  assert.throws(
    () =>
      new ConnectJournalRecord({
        direction: ConnectDirection.APP_TO_WALLET,
        sequence: 1n << 64n,
        ciphertext: new Uint8Array([0x01]),
        receivedAtMs: 10,
        expiresAtMs: 20,
      }),
    (error) =>
      error instanceof ConnectJournalError &&
      /uint64/i.test(error.message),
  );
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

test("decode accepts header padding", () => {
  const record = new ConnectJournalRecord({
    direction: ConnectDirection.WALLET_TO_APP,
    sequence: 7n,
    ciphertext: new Uint8Array([0x11, 0x22]),
    receivedAtMs: 100,
    expiresAtMs: 200,
  });
  const encoded = record.encode();
  const padding = new Uint8Array(8);
  const headerLen = 40;
  const padded = new Uint8Array(encoded.length + padding.length);
  padded.set(encoded.subarray(0, headerLen), 0);
  padded.set(padding, headerLen);
  padded.set(encoded.subarray(headerLen), headerLen + padding.length);
  const decoded = ConnectJournalRecord.decode(padded);
  assert.equal(decoded.bytesConsumed, padded.length);
  assert.equal(decoded.record.sequence, record.sequence);
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
