//! Transport regressions: base-path prefixes, protocol probes, fetch receivers,
//! capability preflight, transaction hashes and event-stream filters/cancellation.
import { test } from "node:test";
import assert from "node:assert/strict";

import { ToriiClient } from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import { NoritoRpcClient } from "../src/noritoRpcClient.js";
import { TORII_TEST_NATIVE_BINDING } from "../src/toriiTestHooks.js";
import { ToriiDataModelMismatchError } from "../src/toriiCompatibility.js";
import { Filter, ListQueryError, ToriiError, ToriiHttpError, ToriiStreamGapError, field } from "../src/index.js";

const SIGNED_TRANSACTION = Uint8Array.of(0x01, 0xaa, 0xbb);
const TRANSACTION_HASH = `${"ab".repeat(31)}cd`;

function jsonResponse(body, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function capabilities(dataModelVersion = 4) {
  return {
    abi_version: 1,
    data_model_version: dataModelVersion,
    crypto: {
      sm: {
        enabled: false,
        default_hash: "sha2_256",
        allowed_signing: ["ed25519"],
        sm2_distid_default: "",
        openssl_preview: false,
        acceleration: { scalar: true, neon_sm3: false, neon_sm4: false, policy: "scalar-only" },
      },
      curves: { registry_version: 1, allowed_curve_ids: [1] },
    },
  };
}

function transactionNative(overrides = {}) {
  return {
    encodeSignedTransactionVersioned: (payload) => Buffer.from(payload),
    hashSignedTransaction: () => Buffer.from(TRANSACTION_HASH, "hex"),
    ...overrides,
  };
}

test("a base URL path prefix is preserved for every request", async () => {
  const urls = [];
  const client = new ToriiClient("https://gateway.example/torii/", {
    fetchImpl: async (url) => {
      urls.push(String(url));
      return jsonResponse({ items: [], next_cursor: null });
    },
  });
  await client.domains.list();
  assert.deepEqual(urls, ["https://gateway.example/torii/v1/domains/query"]);
  const browserUrls = [];
  const browser = new ToriiBrowserClient("https://gateway.example/torii", {
    fetchImpl: async (url) => {
      browserUrls.push(String(url));
      return jsonResponse({ items: [], next_cursor: null });
    },
  });
  await browser.domains.list();
  assert.deepEqual(browserUrls, ["https://gateway.example/torii/v1/domains/query"]);
});

test("getHealth and getMetrics use the protocol probes /health and /metrics", async () => {
  const requests = [];
  const client = new ToriiClient("https://node.example", {
    fetchImpl: async (url, init) => {
      requests.push([String(url), new Headers(init.headers).get("accept")]);
      const text = String(url).endsWith("/health") ? "Healthy" : "# TYPE block_height gauge\nblock_height 7\n";
      return new Response(text, { status: 200, headers: { "content-type": "text/plain" } });
    },
  });
  assert.deepEqual(await client.getHealth(), { status: "Healthy" });
  assert.equal(await client.getMetrics(), "# TYPE block_height gauge\nblock_height 7\n");
  assert.deepEqual(requests, [
    ["https://node.example/health", "text/plain"],
    ["https://node.example/metrics", "text/plain"],
  ]);
  await assert.rejects(() => client.getMetrics({ asText: true }), /unsupported fields: asText/u);
});

test("fetch implementations are never invoked with a client receiver", async () => {
  const receivers = [];
  function strictFetch() {
    receivers.push(this);
    return Promise.resolve(jsonResponse({ items: [], next_cursor: null }));
  }
  await new ToriiClient("https://node.example", { fetchImpl: strictFetch }).domains.list();
  const browser = new ToriiBrowserClient("https://node.example", { fetchImpl: strictFetch });
  await browser.domains.list();
  await browser.getAccountCapabilities().catch(() => {});
  const rpc = new NoritoRpcClient("https://node.example", {
    fetchImpl: function rpcFetch() {
      receivers.push(this);
      return Promise.resolve(new Response(new Uint8Array([1]), { status: 200, headers: { "content-type": "application/x-norito" } }));
    },
  });
  await rpc.call("/v1/pipeline/transactions", new Uint8Array([1])).catch(() => {});
  assert(receivers.length >= 4);
  assert(receivers.every((receiver) => receiver === undefined), "fetch is called as a plain function");
});

test("the transaction preflight reads public capabilities without credentials and never poisons the client", async () => {
  const requests = [];
  let capabilityStatus = 503;
  const client = new ToriiClient("https://node.example", {
    fetchImpl: async (url, init) => {
      requests.push({ url: String(url), headers: new Headers(init.headers), credentials: init.credentials });
      if (String(url).endsWith("/v1/node/capabilities")) {
        return capabilityStatus === 200
          ? jsonResponse(capabilities())
          : jsonResponse({ code: "unavailable", message: "warming up" }, capabilityStatus);
      }
      return new Response(null, { status: 204 });
    },
    maxRetries: 0,
    [TORII_TEST_NATIVE_BINDING]: transactionNative(),
  });
  await assert.rejects(
    () => client.submitTransaction(SIGNED_TRANSACTION),
    (error) => error instanceof ToriiHttpError && error.status === 503,
  );
  capabilityStatus = 200;
  await client.submitTransaction(SIGNED_TRANSACTION);
  assert.deepEqual(
    requests.map((request) => new URL(request.url).pathname),
    ["/v1/node/capabilities", "/v1/node/capabilities", "/v1/pipeline/transactions"],
  );
  const probe = requests[1];
  assert.equal(probe.credentials, "omit");
  assert.equal(probe.headers.get("x-iroha-signature"), null);
  await client.submitTransaction(SIGNED_TRANSACTION);
  assert.equal(requests.length, 4, "a successful probe is cached");
});

test("a data-model mismatch is reported and probed again on the next submission", async () => {
  let version = 9;
  let probes = 0;
  const client = new ToriiClient("https://node.example", {
    fetchImpl: async (url) => {
      if (String(url).endsWith("/v1/node/capabilities")) {
        probes += 1;
        return jsonResponse(capabilities(version));
      }
      return new Response(null, { status: 204 });
    },
    [TORII_TEST_NATIVE_BINDING]: transactionNative(),
  });
  await assert.rejects(
    () => client.submitTransaction(SIGNED_TRANSACTION),
    (error) => error instanceof ToriiDataModelMismatchError && error.actual === 9,
  );
  version = 4;
  await client.submitTransaction(SIGNED_TRANSACTION);
  assert.equal(probes, 2);
});

test("submitTransactionAndWait derives the transaction hash and verifies an asserted one", async () => {
  const statusQueries = [];
  let submissions = 0;
  const fetchImpl = async (url) => {
    const parsed = new URL(String(url));
    if (parsed.pathname === "/v1/node/capabilities") return jsonResponse(capabilities());
    if (parsed.pathname === "/v1/pipeline/transactions") {
      submissions += 1;
      return new Response(null, { status: 204 });
    }
    statusQueries.push(parsed.searchParams.get("hash"));
    return jsonResponse({
      hash: parsed.searchParams.get("hash"),
      status: { kind: "Applied", block_height: 3 },
      scope: "global",
      resolved_from: "state",
    });
  };
  const client = new ToriiClient("https://node.example", {
    fetchImpl,
    [TORII_TEST_NATIVE_BINDING]: transactionNative(),
  });
  const status = await client.submitTransactionAndWait(SIGNED_TRANSACTION, { intervalMs: 0 });
  assert.equal(status.hash, TRANSACTION_HASH);
  assert.deepEqual(statusQueries, [TRANSACTION_HASH]);
  await client.submitTransactionAndWait(SIGNED_TRANSACTION, { hashHex: TRANSACTION_HASH, intervalMs: 0 });
  await assert.rejects(
    () => client.submitTransactionAndWait(SIGNED_TRANSACTION, { hashHex: `${"11".repeat(31)}13` }),
    /does not match the signed transaction hash/u,
  );
  assert.equal(submissions, 2, "a mismatched hash is rejected before submission");
});

function sseResponse(chunks, { onCancel } = {}) {
  const encoder = new TextEncoder();
  return new Response(
    new ReadableStream({
      start(controller) {
        for (const chunk of chunks) controller.enqueue(encoder.encode(chunk));
      },
      cancel() {
        onCancel?.();
      },
    }),
    { status: 200, headers: { "content-type": "text/event-stream" } },
  );
}

test("streamEvents sends the canonical text filter and stops when the caller aborts", async () => {
  let requested;
  let cancelled = 0;
  const client = new ToriiClient("https://node.example", {
    fetchImpl: async (url) => {
      requested = new URL(String(url));
      return sseResponse(['data: {"category":"Other","event":"Time","summary":"t"}\n\n'], { onCancel: () => { cancelled += 1; } });
    },
  });
  const filter = field("tx_hash").eq("abc").and(field("tx_status").in(["Approved", "Rejected"]));
  const controller = new AbortController();
  const events = client.streamEvents({ filter, signal: controller.signal });
  const first = await events.next();
  assert.deepEqual(first.value.data, { category: "Other", event: "Time", summary: "t" });
  assert.equal(requested.pathname, "/v1/events/sse");
  assert.equal(requested.searchParams.get("filter"), 'tx_hash = "abc" and tx_status in ["Approved", "Rejected"]');
  const next = events.next();
  controller.abort(new Error("unsubscribe"));
  await assert.rejects(next, /unsubscribe/u);
  assert.equal(cancelled, 1, "aborting cancels the response body");

  const open = client.streamEvents({ filter: 'block_status = "Committed"' });
  await open.next();
  assert.equal(requested.searchParams.get("filter"), 'block_status = "Committed"');
  await open.return();
  assert.equal(cancelled, 2, "leaving the loop cancels the response body");
});

test("event filters reject the retired object shapes", () => {
  const client = new ToriiClient("https://node.example", { fetchImpl: async () => jsonResponse({}) });
  for (const retired of [{ Pipeline: { Block: {} } }, { VerifyingKey: { id_matcher: {} } }, { Eq: ["a", 1] }]) {
    assert.throws(
      () => client.streamEvents({ filter: retired }),
      (error) => error instanceof ListQueryError && error.code === "invalid_filter",
    );
  }
  assert.throws(
    () => client.streamEvents({ filter: field("metadata.x").eq({ a: 1 }) }),
    /no text form/u,
  );
});

test("ToriiBrowserClient.streamEvents filters with the same grammar and surfaces gaps", async () => {
  let requested;
  const client = new ToriiBrowserClient("https://node.example", {
    fetchImpl: async (url) => {
      requested = new URL(String(url));
      return sseResponse([
        'data: {"category":"Pipeline","event":"Block","status":"Committed"}\n\n',
        'event: stream_error\ndata: {"code":"lagged","message":"dropped 3 events","dropped_messages":3}\n\n',
      ]);
    },
  });
  const seen = [];
  await assert.rejects(
    (async () => {
      for await (const event of client.streamEvents({ filter: Filter.parse("tx_status = 'Approved'") })) {
        seen.push(event.data);
      }
    })(),
    (error) => {
      assert(error instanceof ToriiStreamGapError);
      assert.equal(error.code, "lagged");
      assert.equal(error.droppedMessages, 3);
      return true;
    },
  );
  assert.deepEqual(seen, [{ category: "Pipeline", event: "Block", status: "Committed" }]);
  assert.equal(requested.pathname, "/v1/events/sse");
  assert.equal(requested.searchParams.get("filter"), 'tx_status = "Approved"');
});

test("ToriiBrowserClient.streamEvents stops on abort and on early exit", async () => {
  let cancelled = 0;
  const client = new ToriiBrowserClient("https://node.example", {
    fetchImpl: async () => sseResponse(['data: {"category":"Other","event":"Time","summary":"t"}\n\n'], { onCancel: () => { cancelled += 1; } }),
  });
  const controller = new AbortController();
  const events = client.streamEvents({ filter: 'block_status = "Applied"', signal: controller.signal });
  assert.deepEqual((await events.next()).value.data, { category: "Other", event: "Time", summary: "t" });
  const pending = events.next();
  controller.abort(new Error("unsubscribe"));
  await assert.rejects(pending, /unsubscribe/u);
  assert.equal(cancelled, 1, "aborting cancels the response body");

  const open = client.streamEvents();
  await open.next();
  await open.return();
  assert.equal(cancelled, 2, "leaving the loop cancels the response body");
});

test("a rejected event filter surfaces the server error envelope on both clients", async () => {
  const envelope = {
    code: "invalid_filter",
    message: "invalid `filter`: unknown field `tx_stat` (column 1)",
    details: { field: "filter", actual: "tx_stat", expected: "tx_hash, tx_status, block_height", hint: "tx_status" },
  };
  const fetchImpl = async () => jsonResponse(envelope, 400);
  for (const client of [
    new ToriiClient("https://node.example", { fetchImpl, maxRetries: 0 }),
    new ToriiBrowserClient("https://node.example", { fetchImpl }),
  ]) {
    await assert.rejects(
      client.streamEvents({ filter: 'tx_stat = "Applied"' }).next(),
      (error) => {
        assert(error instanceof ToriiHttpError, client.constructor.name);
        assert.equal(error.status, 400);
        assert.equal(error.code, "invalid_filter");
        assert.equal(error.errorMessage, envelope.message);
        assert.deepEqual(error.details, envelope.details);
        assert.equal(typeof error.details.expected, "string", "expected fields are a comma-separated string");
        return true;
      },
    );
  }
});

test("ToriiBrowserClient.getNodeCapabilities is an unsigned public read", async () => {
  const requests = [];
  const client = new ToriiBrowserClient("https://node.example", {
    fetchImpl: async (url, init) => {
      requests.push({ url: new URL(String(url)), init });
      return jsonResponse(capabilities());
    },
  });
  const advert = await client.getNodeCapabilities();
  assert.equal(advert.data_model_version, 4);
  assert.equal(requests.length, 1);
  assert.equal(requests[0].url.pathname, "/v1/node/capabilities");
  assert.equal(requests[0].init.method, "GET");
  assert.equal(requests[0].init.credentials, "omit");
  assert.equal(new Headers(requests[0].init.headers).get("accept"), "application/json");
  assert.equal(new Headers(requests[0].init.headers).get("x-iroha-signature"), null);
  await assert.rejects(
    () => client.getNodeCapabilities({ sign: () => ({}) }),
    /unsupported/u,
  );
});

const EVENT_FRAMES = [
  'data: {"category":"Pipeline","event":"Transaction","hash":"aa","lane_id":1,"dataspace_id":18446744073709551615,"block_height":9007199254740993,"status":"Rejected","rejection_code":"validation","rejection_reason":"The transaction failed validation."}\n\n',
  'data: {"category":"Pipeline","event":"Block","status":"Rejected","rejection_code":"EmptyBlock"}\n\n',
  'data: {"category":"Data","event":"Hologram","summary":"a kind this SDK does not know"}\n\n',
  'data: {"category":"Interstellar","event":"Ping"}\n\n',
];

test("event payloads decode u64 values exactly and pass unknown kinds through on both clients", async () => {
  const clients = [
    (fetchImpl) => new ToriiClient("https://node.example", { fetchImpl }),
    (fetchImpl) => new ToriiBrowserClient("https://node.example", { fetchImpl }),
  ];
  for (const makeClient of clients) {
    const client = makeClient(async () => sseResponse([...EVENT_FRAMES, "data: [1, 2]\n\n"]));
    const seen = [];
    await assert.rejects(
      (async () => {
        for await (const frame of client.streamEvents()) {
          assert.equal(frame.event, null);
          seen.push(frame.data);
        }
      })(),
      (error) => {
        assert(error instanceof ToriiError, client.constructor.name);
        assert.equal(error.code, "invalid_response");
        assert.match(error.message, /not a JSON object/u);
        return true;
      },
    );
    assert.equal(seen.length, 4, client.constructor.name);
    assert.equal(seen[0].dataspace_id, 18_446_744_073_709_551_615n);
    assert.equal(seen[0].block_height, 9_007_199_254_740_993n);
    assert.equal(seen[0].lane_id, 1);
    assert.equal(seen[0].rejection_code, "validation");
    assert.deepEqual(seen[1], { category: "Pipeline", event: "Block", status: "Rejected", rejection_code: "EmptyBlock" });
    assert.deepEqual(seen[2], { category: "Data", event: "Hologram", summary: "a kind this SDK does not know" });
    assert.deepEqual(seen[3], { category: "Interstellar", event: "Ping" });
  }
});

test("ToriiClient.streamEvents yields the terminal stream_error frame unchanged", async () => {
  const client = new ToriiClient("https://node.example", {
    fetchImpl: async () => sseResponse([
      EVENT_FRAMES[1],
      'event: stream_error\ndata: {"code":"stream_lagged","message":"lost events","dropped_messages":4,"replay_available":false}\n\n',
    ]),
  });
  const events = client.streamEvents();
  assert.equal((await events.next()).value.data.status, "Rejected");
  const gap = (await events.next()).value;
  assert.equal(gap.event, "stream_error");
  assert.deepEqual(gap.data, { code: "stream_lagged", message: "lost events", dropped_messages: 4, replay_available: false });
  await events.return();
});
