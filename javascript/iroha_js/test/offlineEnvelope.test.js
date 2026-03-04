import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import { mkdtemp, rm } from "node:fs/promises";
import test from "node:test";

import {
  buildOfflineEnvelope,
  parseOfflineEnvelope,
  readOfflineEnvelopeFile,
  replayOfflineEnvelope,
  serializeOfflineEnvelope,
  writeOfflineEnvelopeFile,
} from "../src/index.js";
import { blake2b256 } from "../src/blake2b.js";

function buildSampleEnvelope() {
  const signedTransaction = Buffer.from("offline-transaction-fixture", "utf8");
  const hashHex = Buffer.from(blake2b256(signedTransaction)).toString("hex");
  const envelope = buildOfflineEnvelope({
    signedTransaction,
    keyAlias: "alice-key",
    metadata: { purpose: "demo" },
    publicKey: Buffer.alloc(32, 0x22),
    hashHex,
  });
  return { envelope, hashHex };
}

test("buildOfflineEnvelope computes hashes and preserves metadata", () => {
  const { envelope, hashHex } = buildSampleEnvelope();
  assert.equal(envelope.hashHex, hashHex);
  assert.equal(envelope.metadata.purpose, "demo");
  assert.equal(envelope.keyAlias, "alice-key");
  assert.equal(envelope.version, 1);
  assert.ok(envelope.signedTransaction.length > 0);
});

test("buildOfflineEnvelope rejects fractional issuedAtMs", () => {
  const signedTransaction = Buffer.from("offline-transaction-fixture", "utf8");
  assert.throws(
    () =>
      buildOfflineEnvelope({
        signedTransaction,
        keyAlias: "alice-key",
        issuedAtMs: 1.5,
      }),
    /issuedAtMs/,
  );
});

test("serialize/parse round-trips offline envelopes", () => {
  const { envelope } = buildSampleEnvelope();
  const serialized = serializeOfflineEnvelope(envelope);
  assert.equal(serialized.hash_hex, envelope.hashHex);
  assert.equal(serialized.key_alias, envelope.keyAlias);
  const parsed = parseOfflineEnvelope(serialized);
  assert.equal(parsed.hashHex, envelope.hashHex);
  assert.equal(parsed.keyAlias, envelope.keyAlias);
  assert.deepEqual(parsed.signedTransaction, envelope.signedTransaction);
});

test("parseOfflineEnvelope rejects invalid base64 payloads", () => {
  const { envelope } = buildSampleEnvelope();
  const serialized = serializeOfflineEnvelope(envelope);
  const bad = { ...serialized, signed_transaction_b64: "not*base64" };
  assert.throws(
    () => parseOfflineEnvelope(bad),
    (error) =>
      error instanceof TypeError &&
      /signed_transaction_b64/.test(error.message) &&
      /base64/.test(error.message),
  );
});

test("write/read offline envelope file round-trips payloads", async () => {
  const tmp = await mkdtemp(path.join(os.tmpdir(), "iroha-offline-"));
  try {
    const { envelope } = buildSampleEnvelope();
    const filePath = path.join(tmp, "envelope.json");
    await writeOfflineEnvelopeFile(filePath, envelope);
    const loaded = await readOfflineEnvelopeFile(filePath);
    assert.equal(loaded.hashHex, envelope.hashHex);
    assert.deepEqual(loaded.metadata, envelope.metadata);
  } finally {
    await rm(tmp, { recursive: true, force: true });
  }
});

test("replayOfflineEnvelope posts payloads and polls status", async () => {
  const { envelope } = buildSampleEnvelope();
  const submitted = [];
  const statuses = [];
  class MockToriiClient {
    async submitTransaction(payload) {
      submitted.push(Buffer.from(payload));
    }

    async waitForTransactionStatus(hash, options) {
      statuses.push({ hash, options });
      return { status: "Committed", hash };
    }
  }

  const client = new MockToriiClient();
  await replayOfflineEnvelope(client, envelope, { intervalMs: 10, timeoutMs: 5_000 });
  assert.equal(submitted.length, 1);
  assert.deepEqual(submitted[0], envelope.signedTransaction);
  assert.equal(statuses[0].hash, envelope.hashHex);
  assert.equal(Buffer.from(blake2b256(submitted[0])).toString("hex"), envelope.hashHex);
});
