import assert from "node:assert/strict";
import test from "node:test";

import {
  ToriiBrowserClient,
  ToriiBrowserHttpError,
} from "../src/toriiBrowserClient.js";

function jsonResponse(payload) {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

test("deployment polling waits for exact global persisted Applied finality", async () => {
  const hash = "ab".repeat(32);
  const payloads = [
    {
      hash,
      status: { kind: "Applied", block_height: 17 },
      scope: "global",
      resolved_from: "cache",
    },
    {
      hash,
      status: { kind: "Applied", block_height: 17 },
      scope: "global",
      resolved_from: "state",
    },
  ];
  const urls = [];
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async (url) => {
      urls.push(String(url));
      return jsonResponse(payloads.shift());
    },
  });

  const status = await client.waitForTransactionStatus(hash, {
    intervalMs: 0,
    maxAttempts: 2,
  });

  assert.equal(status.status.kind, "Applied");
  assert.equal(urls.length, 2);
  for (const url of urls) {
    assert.equal(
      url,
      `https://torii.example/v1/pipeline/transactions/status?hash=${hash}&scope=global`,
    );
  }
});

test("deployment polling rejects malformed Applied envelopes", async () => {
  const hash = "cd".repeat(32);
  for (const [payload, pattern] of [
    [
      {
        hash: "ef".repeat(32),
        status: { kind: "Applied", block_height: 1 },
        scope: "global",
        resolved_from: "state",
      },
      /does not match the requested transaction/u,
    ],
    [
      {
        hash,
        status: { kind: "Applied", block_height: 0 },
        scope: "global",
        resolved_from: "state",
      },
      /block_height must be a positive safe integer/u,
    ],
  ]) {
    const client = new ToriiBrowserClient("https://torii.example", {
      fetchImpl: async () => jsonResponse(payload),
    });
    await assert.rejects(
      client.waitForTransactionStatus(hash, { intervalMs: 0, maxAttempts: 1 }),
      pattern,
    );
  }
});

test("deployment polling does not treat nested Committed markers as finality", async () => {
  const hash = "12".repeat(32);
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () =>
      jsonResponse({
        hash,
        status: { kind: "Queued", content: { Committed: true } },
        scope: "global",
        resolved_from: "queue",
      }),
  });

  await assert.rejects(
    client.waitForTransactionStatus(hash, { intervalMs: 0, maxAttempts: 1 }),
    /retired or unsupported fields: content/u,
  );
});

test("deployment polling retries cached failures until state resolution", async () => {
  const hash = "23".repeat(32);
  const payloads = [
    {
      hash,
      status: { kind: "Rejected" },
      scope: "global",
      resolved_from: "cache",
    },
    {
      hash,
      status: { kind: "Rejected" },
      scope: "global",
      resolved_from: "state",
    },
  ];
  let requests = 0;
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => {
      requests += 1;
      return jsonResponse(payloads.shift());
    },
  });

  await assert.rejects(
    client.waitForTransactionStatus(hash, { intervalMs: 0, maxAttempts: 2 }),
    /terminal Rejected status/u,
  );
  assert.equal(requests, 2);
});

test("deployment polling fails closed on unknown status kinds and non-200 status reads", async () => {
  const hash = "34".repeat(32);
  const unknown = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () =>
      jsonResponse({
        hash,
        status: { kind: "Finalized", block_height: 1 },
        scope: "global",
        resolved_from: "state",
      }),
  });
  await assert.rejects(
    unknown.waitForTransactionStatus(hash, { intervalMs: 0, maxAttempts: 1 }),
    /not a current pipeline status/u,
  );

  const accepted = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () =>
      new Response(JSON.stringify({ hash, status: { kind: "Queued" } }), {
        status: 202,
        headers: { "content-type": "application/json" },
      }),
  });
  await assert.rejects(
    accepted.getTransactionStatus(hash),
    (error) => error instanceof ToriiBrowserHttpError && error.status === 202,
  );
});
