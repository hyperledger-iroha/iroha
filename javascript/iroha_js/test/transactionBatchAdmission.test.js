// Exact per-entry batch admission responses, with injected canonical codec/HTTP owners.
import test from "node:test";
import assert from "node:assert/strict";
import { ToriiClient as SourceToriiClient, TransactionBatchAdmissionAmbiguousError } from "../src/toriiClient.js";
import { TORII_TEST_NATIVE_BINDING } from "../src/toriiTestHooks.js";
// Capability authentication is covered by its own suite. This response-parser
// fixture supplies that prior observation while exercising the actual batch POST.
class ToriiClient extends SourceToriiClient {
  async getNodeCapabilities() { return { dataModelVersion: 4 }; }
}
const BASE_URL = "http://127.0.0.1:8080";
function canonicalTransactionCodecNative(overrides = {}) {
  return {
    encodeSignedTransactionVersioned: (payload) => Buffer.from(payload),
    encodeTransactionPayloadBatch: () => Buffer.from([1]),
    ...overrides,
  };
}
function createResponse({ status, jsonData, headers }) {
  return new Response(jsonData === undefined ? null : JSON.stringify(jsonData), { status, headers });
}
test("submitTransactionBatch exposes exact ordered partial outcomes and rejects substitutions", async () => {
  const payloads = [Buffer.from([1, 0x51]), Buffer.from([1, 0x52])];
  const hashes = ["51".repeat(32), "52".repeat(32)];
  const expected = [
    { signed_transaction_hash: hashes[0], status: 202, reject_code: null },
    { signed_transaction_hash: hashes[1], status: 503, reject_code: "PRTRY:QUEUE_PLAN_JOURNAL_OUTCOME_UNKNOWN" },
  ];
  for (const mutation of ["none", "hash", "order", "count", "status", "missing"]) {
    const outcomes = structuredClone(expected);
    if (mutation === "hash") outcomes[1].signed_transaction_hash = "53".repeat(32);
    if (mutation === "order") outcomes.reverse();
    if (mutation === "status") outcomes[1].status = 200;
    if (mutation === "missing") outcomes.pop();
    let posts = 0;
    const client = new ToriiClient(BASE_URL, {
      fetchImpl: async (url) => {
        assert.equal(url, `${BASE_URL}/v1/pipeline/transactions/batch`);
        posts += 1;
        return createResponse({ status: 207, jsonData: outcomes,
          headers: { "content-type": "application/json", "x-iroha-transactions-accepted": mutation === "count" ? "0" : "1" } });
      },
      [TORII_TEST_NATIVE_BINDING]: canonicalTransactionCodecNative({
        hashSignedTransaction: (payload) => Buffer.alloc(32, payload[1]),
      }),
    });
    if (mutation === "none") {
      assert.deepEqual(await client.submitTransactionBatch(payloads), { acceptedCount: 1, outcomes: expected });
    } else {
      await assert.rejects(() => client.submitTransactionBatch(payloads), TransactionBatchAdmissionAmbiguousError, mutation);
    }
    assert.equal(posts, 1);
  }
});

test("submitTransactionBatch rejects partial counts in an all-accepted response", async () => {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => createResponse({ status: 202, headers: { "x-iroha-transactions-accepted": "0" } }),
    [TORII_TEST_NATIVE_BINDING]: canonicalTransactionCodecNative(),
  });
  await assert.rejects(() => client.submitTransactionBatch([Buffer.from([1, 0x51])]), TransactionBatchAdmissionAmbiguousError);
});

