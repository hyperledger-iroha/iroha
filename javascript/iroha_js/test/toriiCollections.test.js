//! Collection queries (`client.<collection>.list/pages/iterate`) on both Torii clients.
import { test } from "node:test";
import assert from "node:assert/strict";

import {
  LocalSigningContext,
  ToriiClient,
} from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";
import { NetworkId } from "../src/networkId.js";
import {
  ListQuery,
  ListQueryError,
  ToriiCollection,
  ToriiError,
  ToriiHttpError,
  field,
} from "../src/index.js";

const BASE_URL = "https://torii.example";
const NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const ALIAS_ACCOUNT = "alice@wonderland";

function jsonResponse(body, { status = 200, headers = {} } = {}) {
  const text = typeof body === "string" ? body : JSON.stringify(body);
  return new Response(text, {
    status,
    headers: { "content-type": "application/json", ...headers },
  });
}

function recordingFetch(responder) {
  const calls = [];
  const fetchImpl = async (url, init = {}) => {
    const call = {
      url: String(url),
      method: init.method,
      headers: new Headers(init.headers ?? {}),
      body: init.body === undefined ? undefined : String(init.body),
      signal: init.signal,
    };
    calls.push(call);
    return responder(call, calls.length);
  };
  return { calls, fetchImpl };
}

const CLIENTS = [
  ["ToriiClient", (fetchImpl, options = {}) => new ToriiClient(BASE_URL, { fetchImpl, ...options })],
  ["ToriiBrowserClient", (fetchImpl, options = {}) => new ToriiBrowserClient(BASE_URL, { fetchImpl, ...options })],
];

const FIXED_COLLECTIONS = [
  ["domains", "/v1/domains"],
  ["accounts", "/v1/accounts"],
  ["assetDefinitions", "/v1/assets/definitions"],
  ["nfts", "/v1/nfts"],
  ["rwas", "/v1/rwas"],
  ["repoAgreements", "/v1/repo/agreements"],
];
const HISTORY_COLLECTIONS = [
  ["transactions", "/v1/transactions"],
];

for (const [name, makeClient] of CLIENTS) {
  test(`${name}: every collection posts the canonical body to <path>/query`, async () => {
    const { calls, fetchImpl } = recordingFetch(() =>
      jsonResponse({ items: [{ id: "a" }], next_cursor: null }),
    );
    const client = makeClient(fetchImpl);
    const scoped = [
      [client.accountAssets("sorauﾛ1Pacct"), "/v1/accounts/sorau%EF%BE%9B1Pacct/assets"],
      [client.assetHolders("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), "/v1/assets/62Fk4FPcMuLvW5QjDGNF2a4jAmjM/holders"],
    ];
    const collections = [
      ...FIXED_COLLECTIONS.map(([property, path]) => [client[property], path]),
      ...scoped,
    ];
    for (const [collection, path] of collections) {
      assert(collection instanceof ToriiCollection);
      assert.equal(collection.path, path);
      assert.equal(collection.history, false);
      const page = await collection.list({
        filter: field("owned_by").eq(ALIAS_ACCOUNT).and(field("quantity").gt(1)),
        sort: "-quantity,id",
        select: ["id", "quantity"],
        limit: 25,
        includeTotal: true,
      });
      assert.deepEqual(page, { items: [{ id: "a" }], nextCursor: null, total: undefined });
      const call = calls.at(-1);
      assert.equal(call.method, "POST");
      assert.equal(call.url, `${BASE_URL}${path}/query`);
      assert.equal(call.headers.get("content-type"), "application/json");
      assert.equal(
        call.body,
        '{"filter":{"op":"and","args":[{"op":"eq","args":["owned_by","alice@wonderland"]},{"op":"gt","args":["quantity",1]}]},"sort":["-quantity","id"],"select":["id","quantity"],"limit":25,"include_total":true}',
      );
    }
    assert.equal(client.domains, client.domains, "collection handles are stable");
  });

  test(`${name}: transaction history collections post filters, projections and cursors`, async () => {
    const { calls, fetchImpl } = recordingFetch(() =>
      jsonResponse({ items: [{ entrypoint_hash: "aa" }], next_cursor: "q1_older" }),
    );
    const client = makeClient(fetchImpl);
    const collections = [
      ...HISTORY_COLLECTIONS.map(([property, path]) => [client[property], path]),
      [client.accountTransactions("bob@wonderland"), "/v1/accounts/bob%40wonderland/transactions"],
    ];
    for (const [collection, path] of collections) {
      assert(collection instanceof ToriiCollection);
      assert.equal(collection.path, path);
      assert.equal(collection.history, true);
      const page = await collection.list({
        filter: field("block_height").gte(1200).and(field("result_ok").eq(true)),
        select: ["entrypoint_hash", "block_height"],
        limit: 50,
        cursor: "q1_newer",
      });
      assert.deepEqual(page, { items: [{ entrypoint_hash: "aa" }], nextCursor: "q1_older", total: undefined });
      const call = calls.at(-1);
      assert.equal(call.method, "POST");
      assert.equal(call.url, `${BASE_URL}${path}/query`);
      assert.equal(
        call.body,
        '{"filter":{"op":"and","args":[{"op":"gte","args":["block_height",1200]},{"op":"eq","args":["result_ok",true]}]},"select":["entrypoint_hash","block_height"],"limit":50,"cursor":"q1_newer"}',
      );
    }
  });

  test(`${name}: transaction history rejects sort, totals and aggregates before any request`, async () => {
    const { calls, fetchImpl } = recordingFetch(() => jsonResponse({ items: [], next_cursor: null }));
    const client = makeClient(fetchImpl);
    for (const collection of [client.transactions, client.accountTransactions("bob@wonderland")]) {
      for (const [query, code] of [
        [{ sort: "-block_height" }, "invalid_sort"],
        [{ includeTotal: true }, "invalid_include_total"],
        [{ aggregate: { metrics: [{ alias: "n", fn: "count" }] } }, "invalid_aggregate"],
      ]) {
        await assert.rejects(
          collection.list(query),
          (error) => error instanceof ListQueryError && error.code === code,
        );
        assert.throws(
          () => collection.iterate(query),
          (error) => error instanceof ListQueryError && error.code === code,
        );
      }
    }
    assert.equal(calls.length, 0);
  });

  test(`${name}: history iteration follows next_cursor across short and empty pages`, async () => {
    const pages = [
      { items: [], next_cursor: "q1_a" },
      { items: [{ entrypoint_hash: "01" }], next_cursor: "q1_b" },
      { items: [], next_cursor: "q1_c" },
      { items: [{ entrypoint_hash: "02" }, { entrypoint_hash: "03" }], next_cursor: null },
    ];
    const { calls, fetchImpl } = recordingFetch((_call, index) => jsonResponse(pages[index - 1]));
    const client = makeClient(fetchImpl);
    const hashes = [];
    for await (const row of client.transactions.iterate({ filter: "result_ok = true", limit: 2 })) {
      hashes.push(row.entrypoint_hash);
    }
    assert.deepEqual(hashes, ["01", "02", "03"]);
    assert.deepEqual(
      calls.map((call) => JSON.parse(call.body).cursor ?? null),
      [null, "q1_a", "q1_b", "q1_c"],
    );
  });

  test(`${name}: list() sends text filters as-is and returns the total`, async () => {
    const { calls, fetchImpl } = recordingFetch(() =>
      jsonResponse({ items: [], next_cursor: "q1_next", total: 7 }),
    );
    const client = makeClient(fetchImpl);
    const page = await client.assetDefinitions.list({
      filter: 'alias_binding.status = "permanent"',
      includeTotal: true,
    });
    assert.deepEqual(page, { items: [], nextCursor: "q1_next", total: 7 });
    assert.equal(calls[0].body, '{"filter":"alias_binding.status = \\"permanent\\"","include_total":true}');
    await client.domains.list();
    assert.equal(calls[1].body, "{}");
  });

  test(`${name}: iterate() follows next_cursor until null`, async () => {
    const pages = [
      { items: [{ id: "a" }, { id: "b" }], next_cursor: "c1" },
      { items: [{ id: "c" }], next_cursor: "c2" },
      { items: [], next_cursor: null },
    ];
    const { calls, fetchImpl } = recordingFetch((_call, index) => jsonResponse(pages[index - 1]));
    const client = makeClient(fetchImpl);
    const ids = [];
    for await (const item of client.nfts.iterate({ filter: field("owned_by").eq("x"), limit: 2 })) {
      ids.push(item.id);
    }
    assert.deepEqual(ids, ["a", "b", "c"]);
    assert.deepEqual(
      calls.map((call) => JSON.parse(call.body)),
      [
        { filter: { op: "eq", args: ["owned_by", "x"] }, limit: 2 },
        { filter: { op: "eq", args: ["owned_by", "x"] }, limit: 2, cursor: "c1" },
        { filter: { op: "eq", args: ["owned_by", "x"] }, limit: 2, cursor: "c2" },
      ],
    );
    const paged = makeClient(recordingFetch((_call, index) => jsonResponse(pages[index - 1])).fetchImpl);
    const seen = [];
    for await (const page of paged.nfts.pages({ limit: 2 })) {
      seen.push([page.items.length, page.nextCursor]);
    }
    assert.deepEqual(seen, [[2, "c1"], [1, "c2"], [0, null]]);
  });

  test(`${name}: leaving the loop or aborting stops paging`, async () => {
    const { calls, fetchImpl } = recordingFetch((_call, index) =>
      jsonResponse({ items: [{ id: `item${index}` }], next_cursor: `c${index}` }),
    );
    const client = makeClient(fetchImpl);
    for await (const item of client.accounts.iterate()) {
      assert.equal(item.id, "item1");
      break;
    }
    assert.equal(calls.length, 1);
    const controller = new AbortController();
    const seen = [];
    await assert.rejects(
      (async () => {
        for await (const item of client.accounts.iterate({}, { signal: controller.signal })) {
          seen.push(item.id);
          if (seen.length === 2) controller.abort(new Error("stop paging"));
        }
      })(),
      /stop paging/u,
    );
    assert.deepEqual(seen, ["item2", "item3"]);
    assert.equal(calls.length, 3);
  });

  test(`${name}: a page that repeats its cursor is rejected instead of looping`, async () => {
    const { fetchImpl } = recordingFetch(() => jsonResponse({ items: [{ id: "a" }], next_cursor: "same" }));
    const client = makeClient(fetchImpl);
    await assert.rejects(
      (async () => {
        for await (const _item of client.rwas.iterate({ cursor: "same" })) {
          // drain
        }
      })(),
      (error) => error instanceof ToriiError && error.code === "invalid_response",
    );
  });

  test(`${name}: Torii error envelopes become ToriiHttpError with code, message and details`, async () => {
    const envelope = {
      code: "invalid_filter",
      message: "invalid `filter`: use the keyword `and` instead of `&` or `&&` (column 17)",
      details: { field: "filter", hint: "write `and`" },
    };
    const { fetchImpl } = recordingFetch(() => jsonResponse(envelope, { status: 400 }));
    const client = makeClient(fetchImpl);
    await assert.rejects(
      () => client.accounts.list({ filter: 'owned_by == "x" && quantity > 1' }),
      (error) => {
        assert(error instanceof ToriiHttpError);
        assert(error instanceof ToriiError);
        assert.equal(error.status, 400);
        assert.equal(error.code, "invalid_filter");
        assert.equal(error.errorMessage, envelope.message);
        assert.deepEqual(error.details, envelope.details);
        assert.deepEqual(error.bodyJson, envelope);
        assert.match(error.message, /HTTP 400.*invalid_filter/u);
        return true;
      },
    );
    const rejecting = makeClient(recordingFetch(() =>
      jsonResponse({ code: "admission_rejected", message: "nope" }, {
        status: 409,
        headers: { "x-iroha-reject-code": "PRTRY:QUEUE_FULL" },
      }),
    ).fetchImpl);
    await assert.rejects(
      () => rejecting.domains.list(),
      (error) => error instanceof ToriiHttpError && error.rejectCode === "PRTRY:QUEUE_FULL" && error.code === "PRTRY:QUEUE_FULL",
    );
  });

  test(`${name}: invalid queries fail before any request`, async () => {
    const { calls, fetchImpl } = recordingFetch(() => jsonResponse({ items: [], next_cursor: null }));
    const client = makeClient(fetchImpl);
    await assert.rejects(
      () => client.domains.list({ limit: 0 }),
      (error) => error instanceof ListQueryError && error.code === "invalid_limit",
    );
    assert.throws(
      () => client.domains.iterate({ offset: 5 }),
      (error) => error instanceof ListQueryError && error.code === "invalid_query",
    );
    await assert.rejects(() => client.domains.list({}, { bogus: true }), /bogus/u);
    assert.throws(() => client.accountAssets(" padded "), TypeError);
    assert.throws(() => client.assetHolders(""), TypeError);
    assert.equal(calls.length, 0);
  });

  test(`${name}: wide integers stay exact in requests and responses`, async () => {
    const { calls, fetchImpl } = recordingFetch(() =>
      new Response('{"items":[{"id":"a","nonce":18446744073709551615,"ratio":0.5,"q":"12345678901234567890"}],"next_cursor":null}', {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    const client = makeClient(fetchImpl);
    const page = await client.accounts.list({ filter: field("nonce").eq(18_446_744_073_709_551_615n) });
    assert.equal(calls[0].body, '{"filter":{"op":"eq","args":["nonce",18446744073709551615]}}');
    assert.deepEqual(page.items, [{ id: "a", nonce: 18_446_744_073_709_551_615n, ratio: 0.5, q: "12345678901234567890" }]);
    assert.equal(Object.getPrototypeOf(page.items[0]), Object.prototype);
  });

  test(`${name}: malformed pages are protocol errors`, async () => {
    for (const body of ['{"items":{}}', "[]", '{"items":[],"next_cursor":5}', "not json", ""]) {
      const client = makeClient(recordingFetch(() =>
        new Response(body, { status: 200, headers: { "content-type": "application/json" } }),
      ).fetchImpl);
      await assert.rejects(
        () => client.domains.list(),
        (error) => error instanceof ToriiError && error.code === "invalid_response",
        body,
      );
    }
  });

  test(`${name}: a stalled body read honours the caller's abort signal`, async () => {
    const encoder = new TextEncoder();
    let cancelled = false;
    const fetchImpl = async () =>
      new Response(
        new ReadableStream({
          start(controller) {
            controller.enqueue(encoder.encode('{"items":['));
          },
          cancel() {
            cancelled = true;
          },
        }),
        { status: 200, headers: { "content-type": "application/json" } },
      );
    const client = makeClient(fetchImpl);
    const controller = new AbortController();
    const pending = client.domains.list({}, { signal: controller.signal });
    setTimeout(() => controller.abort(new Error("caller gave up")), 20);
    await assert.rejects(pending, /caller gave up/u);
    assert.equal(cancelled, true, "the stalled body is cancelled");
  });
}

test("ToriiClient signs collection queries with configured credentials and never requires them", async () => {
  const { calls, fetchImpl } = recordingFetch(() => jsonResponse({ items: [], next_cursor: null }));
  const signed = new ToriiClient(BASE_URL, {
    fetchImpl,
    localSigningContext: new LocalSigningContext(NETWORK_ID, 753),
    canonicalRequestAuth: { accountId: ALIAS_ACCOUNT, privateKey: Buffer.alloc(32, 7) },
  });
  await signed.domains.list({ limit: 1 });
  assert.equal(calls[0].headers.get("x-iroha-account"), ALIAS_ACCOUNT);
  assert.match(calls[0].headers.get("x-iroha-signature"), /^[A-Za-z0-9+/]+=*$/u);
  assert.ok(calls[0].headers.get("x-iroha-nonce"));
  await signed.domains.list({ limit: 1 }, { canonicalAuth: null });
  assert.equal(calls[1].headers.get("x-iroha-signature"), null, "canonicalAuth: null sends unsigned");
  const anonymous = new ToriiClient(BASE_URL, { fetchImpl });
  await anonymous.domains.list({ limit: 1 });
  assert.equal(calls[2].headers.get("x-iroha-signature"), null);
});

test("ToriiBrowserClient signs collection queries through its sign callback", async () => {
  const { calls, fetchImpl } = recordingFetch(() => jsonResponse({ items: [], next_cursor: null }));
  const messages = [];
  const client = new ToriiBrowserClient(BASE_URL, {
    fetchImpl,
    networkId: NETWORK_ID,
    canonicalRequestAuth: {
      accountId: ALIAS_ACCOUNT,
      sign: async ({ body, method, path }) => {
        messages.push({ body, method, path });
        return Buffer.alloc(64, 1).toString("base64");
      },
    },
  });
  await client.accountAssets(ALIAS_ACCOUNT).list({ limit: 3 });
  assert.deepEqual(messages, [
    { body: '{"limit":3}', method: "POST", path: "/v1/accounts/alice%40wonderland/assets/query" },
  ]);
  assert.equal(calls[0].body, '{"limit":3}');
  assert.equal(calls[0].headers.get("x-iroha-account"), ALIAS_ACCOUNT);
});

test("ListQuery instances can be reused across clients", async () => {
  const query = ListQuery.from({ filter: field("owned_by").eq("x"), limit: 5 });
  for (const [, makeClient] of CLIENTS) {
    const { calls, fetchImpl } = recordingFetch(() => jsonResponse({ items: [], next_cursor: null }));
    await makeClient(fetchImpl).repoAgreements.list(query);
    assert.equal(calls[0].body, '{"filter":{"op":"eq","args":["owned_by","x"]},"limit":5}');
  }
});
