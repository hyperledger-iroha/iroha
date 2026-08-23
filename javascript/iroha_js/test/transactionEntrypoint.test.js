import test from "node:test";
import assert from "node:assert/strict";

import { submitTransactionEntrypoint } from "../src/transaction.js";
import { ToriiClient } from "../src/toriiClient.js";

test("submitTransactionEntrypoint waits through Committed until Applied", async () => {
  const client = new ToriiClient("https://example.test");
  const submitted = [];
  const polled = [];
  client.submitTransaction = async (payload) => {
    submitted.push(Buffer.from(payload));
    return { accepted: true };
  };
  client.getTransactionStatus = async (hashHex, options = {}) => {
    polled.push({ hashHex, options });
    return authoritativePipelineStatus(
      hashHex,
      polled.length > 1 ? "Applied" : "Committed",
    );
  };

  const result = await submitTransactionEntrypoint(
    client,
    Buffer.from([9, 8, 7]),
    {
      hashHex: "ab".repeat(32),
      waitForCommit: true,
      pollIntervalMs: 0,
      timeoutMs: 100,
    },
  );

  assert.deepEqual(Array.from(submitted[0]), [9, 8, 7]);
  assert.equal(polled[0].hashHex, "ab".repeat(32));
  assert.equal(polled[0].options.scope, "global");
  assert.equal(result.hash, "ab".repeat(32));
  assert.equal(result.status.status.kind, "Applied");
  assert.equal(polled.length, 2);
});

test("submitTransactionEntrypoint rejects removed scope before submission", async () => {
  const client = new ToriiClient("https://example.test");
  let submissions = 0;
  client.submitTransaction = async () => {
    submissions += 1;
    throw new Error("removed scope must fail before submission");
  };

  await assert.rejects(
    () =>
      submitTransactionEntrypoint(client, Buffer.from([1]), {
        hashHex: "ac".repeat(32),
        waitForCommit: true,
        scope: "global",
      }),
    /scope is unsupported; finality waits always use global scope/u,
  );
  assert.equal(submissions, 0);
});

function authoritativePipelineStatus(hashHex, kind) {
  const status = kind === "Applied" ? { kind, block_height: 7 } : { kind };
  return {
    hash: hashHex,
    status,
    scope: "global",
    resolved_from: kind === "Applied" ? "state" : "cache",
  };
}
