import test from "node:test";
import assert from "node:assert/strict";

import { ToriiClient, IsoMessageTimeoutError } from "../src/toriiClient.js";

const MIN_ISO_POLL_INTERVAL_MS = 10;

function createClientWithStatusSequence(statusSequence) {
  const pollLog = [];
  const queue = [...statusSequence];
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked in ISO wait helpers tests");
    },
  });
  client.getIsoMessageStatus = async () => {
    const next = queue.length > 0 ? queue.shift() ?? null : null;
    pollLog.push(next);
    return next;
  };
  return { client, pollLog };
}

function isoStatus(status, transactionHash) {
  const record = { status };
  if (transactionHash !== undefined) {
    record.transaction_hash = transactionHash;
  }
  return record;
}

test("getIsoMessageStatus rejects unsupported option fields before fetching", async () => {
  let fetchCalls = 0;
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => {
      fetchCalls += 1;
      return new Response("{}", {
        status: 200,
        headers: { "Content-Type": "application/json" },
      });
    },
  });
  await assert.rejects(
    () => client.getIsoMessageStatus("msg-unsupported", { retryProfile: "ok", extra: true }),
    (error) => {
      assert.match(error.message, /getIsoMessageStatus\.options contains unsupported fields: extra/);
      return true;
    },
  );
  assert.equal(fetchCalls, 0);
});

test("waitForIsoMessageStatus resolves once a committed status is observed", async () => {
  const { client, pollLog } = createClientWithStatusSequence([
    null,
    isoStatus("accepted"),
    isoStatus("committed", "0xabc123"),
  ]);
  const status = await client.waitForIsoMessageStatus("iso-demo", {
    pollIntervalMs: 0,
    maxAttempts: 5,
  });
  assert.equal(status?.status, "committed");
  assert.equal(status?.transaction_hash, "0xabc123");
  assert.equal(pollLog.length, 3);
});

test("waitForIsoMessageStatus rejects sub-minimum non-zero poll intervals", async () => {
  const { client, pollLog } = createClientWithStatusSequence([isoStatus("pending")]);
  await assert.rejects(
    () =>
      client.waitForIsoMessageStatus("iso-too-fast", {
        pollIntervalMs: MIN_ISO_POLL_INTERVAL_MS - 1,
        maxAttempts: 1,
      }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(error.message, /pollIntervalMs must be at least/);
      return true;
    },
  );
  assert.deepEqual(pollLog, []);
});

test("waitForIsoMessageStatus rejects unsupported option fields", async () => {
  const { client, pollLog } = createClientWithStatusSequence([isoStatus("pending")]);
  await assert.rejects(
    () =>
      client.waitForIsoMessageStatus("iso-unsupported", {
        pollIntervalMs: 0,
        unexpected: true,
      }),
    (error) => {
      assert(error instanceof TypeError);
      assert.match(
        error.message,
        /waitForIsoMessageStatus\.options contains unsupported fields: unexpected/,
      );
      return true;
    },
  );
  assert.deepEqual(pollLog, []);
});

test("waitForIsoMessageStatus can resolve on accepted statuses without a transaction hash", async () => {
  const { client } = createClientWithStatusSequence([isoStatus("accepted"), isoStatus("committed")]);
  const status = await client.waitForIsoMessageStatus("iso-accepted", {
    pollIntervalMs: 0,
    maxAttempts: 2,
    resolveOnAcceptedWithoutTransaction: true,
  });
  assert.equal(status?.status, "accepted");
  assert.equal(status?.transaction_hash ?? null, null);
});

test("waitForIsoMessageStatus rejects with IsoMessageTimeoutError when retries are exhausted", async () => {
  const { client } = createClientWithStatusSequence([
    isoStatus("accepted"),
    isoStatus("pending"),
  ]);
  await assert.rejects(
    client.waitForIsoMessageStatus("iso-timeout", {
      pollIntervalMs: 0,
      maxAttempts: 2,
    }),
    (error) => {
      assert.ok(error instanceof IsoMessageTimeoutError);
      assert.equal(error.messageId, "iso-timeout");
      assert.equal(error.attempts, 2);
      assert.equal(error.lastStatus?.status, "pending");
      return true;
    },
  );
});

test("waitForIsoMessageStatus forwards each poll to onPoll observers", async () => {
  const { client } = createClientWithStatusSequence([
    null,
    isoStatus("accepted", "0xfeed"),
  ]);
  const attempts = [];
  const status = await client.waitForIsoMessageStatus("iso-onpol", {
    pollIntervalMs: 0,
    maxAttempts: 3,
    resolveOnAcceptedWithoutTransaction: true,
    onPoll: async ({ attempt, status: current }) => {
      attempts.push({ attempt, status: current });
    },
  });
  assert.equal(status?.transaction_hash, "0xfeed");
  assert.deepEqual(
    attempts.map((entry) => entry.attempt),
    [1, 2],
  );
  assert.deepEqual(
    attempts.map((entry) => entry.status),
    [null, isoStatus("accepted", "0xfeed")],
  );
});

test("waitForIsoMessageStatus forwards retry profile and signal to status polls", async () => {
  const controller = new AbortController();
  const calls = [];
  const client = new ToriiClient("https://example.test", {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked in ISO wait helpers tests");
    },
  });
  client.getIsoMessageStatus = async (messageId, options) => {
    calls.push({ messageId, options });
    if (calls.length === 1) {
      return isoStatus("pending");
    }
    return isoStatus("committed", "0xabc");
  };
  const status = await client.waitForIsoMessageStatus("iso-retry", {
    pollIntervalMs: 0,
    maxAttempts: 3,
    retryProfile: "iso-custom",
    signal: controller.signal,
  });
  assert.equal(status?.transaction_hash, "0xabc");
  assert.deepEqual(
    calls.map((entry) => entry.messageId),
    ["iso-retry", "iso-retry"],
  );
  assert.ok(calls.every((entry) => entry.options?.signal === controller.signal));
  assert.ok(calls.every((entry) => entry.options?.retryProfile === "iso-custom"));
});
