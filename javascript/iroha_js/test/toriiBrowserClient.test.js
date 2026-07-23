import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { sha256 } from "@noble/hashes/sha2";

import { AccountAddress } from "../src/address.js";
import {
  ToriiBrowserClient,
  ToriiBrowserHttpError,
  ToriiBrowserStreamGapError,
} from "../src/toriiBrowserClient.js";
import * as browserSdk from "../src/browser.js";
import * as browserDistSdk from "../dist/browser.js";

const BASE_URL = "https://localhost:8080/v1/explorer";
const FIXTURE_ALICE_ID = AccountAddress.fromAccount({
  publicKey: Buffer.from(
    "68F4B6017D0F876A55C80A82B8388A54AAD264D367269E2DE8BE079C935B5F96",
    "hex",
  ),
}).toI105();
const FIXTURE_BOB_ID = AccountAddress.fromAccount({
  publicKey: Buffer.from(
    "EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245",
    "hex",
  ),
}).toI105();
const AUTHORITY_FEE_PAYMENT = Object.freeze({
  payer: "authority",
  value: Object.freeze({ charge_limits: Object.freeze([]), gas_limit: null }),
});

function canonicalReadOptions() {
  return {
    authAccountId: FIXTURE_ALICE_ID,
    timestampMs: 1_700_000_000_000,
    nonce: "browser-read-fixture",
    sign: async () => Buffer.alloc(64, 0x11),
  };
}

function jsonResponse(payload, init = {}) {
  return new Response(JSON.stringify(payload), {
    status: init.status ?? 200,
    headers: { "content-type": "application/json", ...(init.headers ?? {}) },
  });
}

function blockProofResponseFixture() {
  const u64 = (value) => {
    const bytes = Buffer.alloc(8);
    bytes.writeBigUInt64LE(BigInt(value));
    return bytes;
  };
  const compact = (value) => {
    const bytes = [];
    let remaining = value;
    do {
      const byte = remaining & 0x7f;
      remaining >>>= 7;
      bytes.push(remaining === 0 ? byte : byte | 0x80);
    } while (remaining !== 0);
    return Buffer.from(bytes);
  };
  const field = (payload) => Buffer.concat([compact(payload.length), payload]);
  const struct = (...fields) => Buffer.concat(fields.map(field));
  const leaf = Buffer.alloc(32, 0x20);
  leaf[31] |= 1;
  const leafIndex = Buffer.alloc(4);
  const receipt = struct(leaf, struct(leafIndex, u64(0)));
  const payload = struct(
    u64(7), leaf, leaf, receipt, Buffer.of(0), Buffer.of(0), u64(0),
  );
  const schemaHash = Buffer.from(
    sha256(Buffer.from(
      "norito:v1:type-name\0iroha_data_model::block::proofs::BlockProofs",
      "utf8",
    )).subarray(0, 16),
  );
  let crc = 0xffff_ffff_ffff_ffffn;
  for (const byte of payload) {
    crc ^= BigInt(byte);
    for (let bit = 0; bit < 8; bit += 1) {
      crc = (crc & 1n) === 0n
        ? crc >> 1n
        : (crc >> 1n) ^ 0xc96c_5795_d787_0f42n;
    }
  }
  crc = BigInt.asUintN(64, crc ^ 0xffff_ffff_ffff_ffffn);
  return Buffer.concat([
    Buffer.from("NRT0", "ascii"),
    Buffer.of(0, 0),
    schemaHash,
    Buffer.of(0),
    u64(payload.length),
    u64(crc),
    Buffer.of(0x02),
    payload,
  ]);
}

function sseResponse(chunks, { close = true, onCancel } = {}) {
  const encoder = new TextEncoder();
  return new Response(
    new ReadableStream({
      start(controller) {
        for (const chunk of chunks) controller.enqueue(encoder.encode(chunk));
        if (close) controller.close();
      },
      cancel() {
        onCancel?.();
      },
    }),
    { headers: { "content-type": "text/event-stream" } },
  );
}

 test("ToriiBrowserClient strips API suffixes and calls current explorer block routes", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(String(url), "https://localhost:8080/v1/explorer/blocks?page=2&per_page=5");
    assert.equal(init.method, "GET");
    assert.equal(init.headers["x-test-client"], "browser-sdk");
    return jsonResponse({
      pagination: { page: 2, per_page: 5, total_pages: 3, total_items: 11 },
      items: [],
    });
  };
  const client = new ToriiBrowserClient(BASE_URL, {
    fetchImpl,
    defaultHeaders: { "x-test-client": "browser-sdk" },
  });
  const payload = await client.listExplorerBlocks({ page: 2, perPage: 5 });
  assert.equal(payload.pagination.page, 2);
});

test("ToriiBrowserClient exposes exact JSON ledger windows, roots, and state proofs", async () => {
  const calls = [];
  const client = new ToriiBrowserClient(BASE_URL, {
    defaultHeaders: { "x-test-client": "ledger" },
    fetchImpl: async (url, init) => {
      calls.push({ url: String(url), init });
      return jsonResponse({ ok: true });
    },
  });

  await client.listLedgerHeaders({ from: 9, limit: 3 });
  await client.getLedgerStateRoot(9);
  await client.getLedgerStateProof(9);

  assert.equal(
    calls[0].url,
    "https://localhost:8080/v1/ledger/headers?from=9&limit=3",
  );
  assert.equal(calls[1].url, "https://localhost:8080/v1/ledger/state/9");
  assert.equal(calls[2].url, "https://localhost:8080/v1/ledger/state-proof/9");
  for (const call of calls) {
    assert.equal(call.init.headers.Accept, "application/json");
    assert.equal(call.init.headers["x-test-client"], "ledger");
  }
});

test("ToriiBrowserClient fetches ledger BlockProofs only as canonical Norito", async () => {
  const entryHash = "AB".repeat(32);
  let captured;
  const client = new ToriiBrowserClient(BASE_URL, {
    defaultHeaders: { Accept: "application/json", "x-test-client": "ledger" },
    fetchImpl: async (url, init) => {
      captured = { url: String(url), init };
      return new Response(blockProofResponseFixture(), {
        headers: { "content-type": "application/x-norito" },
      });
    },
  });

  const proof = await client.getLedgerBlockProof(7, `0x${entryHash}`);
  assert.equal(
    captured.url,
    `https://localhost:8080/v1/ledger/block/7/proof/${entryHash.toLowerCase()}`,
  );
  assert.equal(captured.init.headers.Accept, "application/x-norito");
  assert.equal(captured.init.headers["x-test-client"], "ledger");
  assert.equal(proof.block_height, "7");
  assert.equal(proof.entry_hash, proof.entry_root);
});

test("ToriiBrowserClient rejects malformed ledger selectors and representations locally", async () => {
  let fetchCalls = 0;
  const client = new ToriiBrowserClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalls += 1;
      return jsonResponse({});
    },
  });

  assert.throws(() => client.listLedgerHeaders({ from: 0 }), /positive safe integer/u);
  assert.throws(() => client.listLedgerHeaders({ offset: 1 }), /unsupported option offset/u);
  assert.throws(() => client.getLedgerStateRoot(0), /positive safe integer/u);
  await assert.rejects(
    client.getLedgerBlockProof(1, "abc"),
    /exactly 32 bytes of hexadecimal/u,
  );
  assert.equal(fetchCalls, 0);

  const wrongContentTypeClient = new ToriiBrowserClient(BASE_URL, {
    fetchImpl: async () => jsonResponse({}),
  });
  await assert.rejects(
    wrongContentTypeClient.getLedgerBlockProof(1, "ab".repeat(32)),
    /must return application\/x-norito/u,
  );
});

test("ToriiBrowserClient account assets use the current asset selector query key", async () => {
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/accounts/test-account/assets");
    assert.equal(parsed.searchParams.get("asset"), "asset-alias");
    assert.equal(parsed.searchParams.get("limit"), "10");
    assert.equal(parsed.searchParams.get("offset"), "20");
    assert.equal(parsed.searchParams.get("count_mode"), "exact");
    return jsonResponse({
      items: [{ asset: "asset-alias", account_id: "test-account", quantity: "7" }],
      total: 1,
    });
  };
  const client = new ToriiBrowserClient(BASE_URL, { fetchImpl });
  const payload = await client.listAccountAssets("test-account", {
    asset: "asset-alias",
    limit: 10,
    offset: 20,
    countMode: " Exact ",
  });
  assert.equal(payload.items[0].asset, "asset-alias");
});

test("ToriiBrowserClient account and contract lists encode only route-specific filters", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url: new URL(url), init });
    const pathname = new URL(url).pathname;
    if (pathname.endsWith("/history")) {
      return jsonResponse({
        items: [],
        total: 0,
        has_more: false,
        count_mode: "exact",
        indexed_height: 7,
        indexed_block_hash: "hash:BLOCK",
        query_source: "account_history_index",
      });
    }
    return jsonResponse({
      items: [],
      has_more: false,
      count_mode: "bounded",
    });
  };
  const client = new ToriiBrowserClient("https://torii.example", { fetchImpl });

  const permissions = await client.listAccountPermissions("alice/account", {
    limit: 5,
    offset: 10,
    countMode: "bounded",
  });
  const history = await client.listAccountHistory("alice/account", {
    assetId: "asset#alice",
    count_mode: "exact",
  });
  await client.listContractActivity({
    authority: "alice",
    contractAddress: "contract-address",
    contractAlias: "router",
    contractEntrypoint: "swap",
    sinceTimestampMs: 100,
    until_timestamp_ms: 200,
    resultOk: true,
    countMode: "bounded",
  });
  await client.listContractEvents({
    authority: "alice",
    contract_address: "contract-address",
    contractAlias: "router",
    module: "swaps",
    eventKind: "fill",
    participant: "bob",
    asset_id: "asset#bob",
    provenance: "emitted",
    since_timestamp_ms: 300,
    untilTimestampMs: 400,
    result_ok: false,
    count_mode: "bounded",
  });

  assert.deepEqual(
    Object.fromEntries(calls[0].url.searchParams),
    { limit: "5", offset: "10", count_mode: "bounded" },
  );
  assert.equal(calls[0].url.pathname, "/v1/accounts/alice%2Faccount/permissions");
  assert.deepEqual(permissions, {
    items: [],
    has_more: false,
    count_mode: "bounded",
  });
  assert.deepEqual(Object.fromEntries(calls[1].url.searchParams), {
    count_mode: "exact",
    asset_id: "asset#alice",
  });
  assert.equal(calls[1].url.pathname, "/v1/accounts/alice%2Faccount/history");
  assert.equal(history.indexed_height, 7);
  assert.deepEqual(Object.fromEntries(calls[2].url.searchParams), {
    count_mode: "bounded",
    authority: "alice",
    contract_address: "contract-address",
    contract_alias: "router",
    contract_entrypoint: "swap",
    since_timestamp_ms: "100",
    until_timestamp_ms: "200",
    result_ok: "true",
  });
  assert.deepEqual(Object.fromEntries(calls[3].url.searchParams), {
    count_mode: "bounded",
    authority: "alice",
    contract_address: "contract-address",
    contract_alias: "router",
    module: "swaps",
    event_kind: "fill",
    participant: "bob",
    asset_id: "asset#bob",
    provenance: "emitted",
    since_timestamp_ms: "300",
    until_timestamp_ms: "400",
    result_ok: "false",
  });
});

test("ToriiBrowserClient rejects unsupported account and contract list options", () => {
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => {
      throw new Error("fetch should not be called for invalid local options");
    },
  });

  assert.throws(
    () => client.listAccountPermissions("alice", { countMode: "full" }),
    /countMode must be bounded or exact/u,
  );
  assert.throws(
    () => client.listAccountHistory("alice", { sort: "timestamp_ms:desc" }),
    /unsupported option sort/u,
  );
  assert.throws(
    () => client.listContractActivity({ filter: { result_ok: true } }),
    /unsupported option filter/u,
  );
  assert.throws(
    () => client.listContractEvents({ provenance: "synthetic" }),
    /provenance must be emitted or derived/u,
  );
});

test("ToriiBrowserClient streams fragmented multiline CRLF contract events with fetch", async () => {
  const abortController = new AbortController();
  let capturedInit;
  let cancelled = false;
  let fetchCalls = 0;
  const client = new ToriiBrowserClient("https://torii.example", {
    defaultHeaders: {
      Accept: "application/json",
      "Last-Event-ID": "must-not-be-sent",
      "x-test-client": "browser-sdk",
    },
    fetchImpl: async (url, init) => {
      fetchCalls += 1;
      capturedInit = init;
      assert.equal(
        String(url),
        "https://torii.example/v1/contracts/events/sse?module=swaps&event_kind=fill",
      );
      return sseResponse(
        [
          ": heart",
          "beat\r",
          "\n\r\nevent: contract_event\r\nid: event-1\r\nretry: 1500\r\n",
          "data: {\"event_id\":\"event-1\",\r\n",
          "data: \"schema_version\":1}\r\n\r",
          "\n",
        ],
        { close: false, onCancel: () => { cancelled = true; } },
      );
    },
  });

  const iterator = client.streamContractEvents({
    module: "swaps",
    eventKind: "fill",
    signal: abortController.signal,
  });
  const first = await iterator.next();
  assert.equal(first.done, false);
  assert.equal(first.value.event, "contract_event");
  assert.equal(first.value.id, "event-1");
  assert.equal(first.value.retry, 1500);
  assert.deepEqual(first.value.data, { event_id: "event-1", schema_version: 1 });
  assert.equal(
    first.value.raw,
    "{\"event_id\":\"event-1\",\n\"schema_version\":1}",
  );
  assert.equal(capturedInit.signal, abortController.signal);
  const headers = new Headers(capturedInit.headers);
  assert.equal(headers.get("accept"), "text/event-stream");
  assert.equal(headers.get("x-test-client"), "browser-sdk");
  assert.equal(headers.get("last-event-id"), null);
  await iterator.return();
  assert.equal(cancelled, true);
  assert.equal(fetchCalls, 1);
});

test("ToriiBrowserClient forwards aborts into the contract event ReadableStream", async () => {
  const abortController = new AbortController();
  let requestStarted = false;
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async (_url, init) => {
      requestStarted = true;
      return new Response(
        new ReadableStream({
          start(controller) {
            init.signal.addEventListener(
              "abort",
              () => controller.error(init.signal.reason),
              { once: true },
            );
          },
        }),
        { headers: { "content-type": "text/event-stream" } },
      );
    },
  });

  const pending = client.streamContractEvents({ signal: abortController.signal }).next();
  while (!requestStarted) await Promise.resolve();
  const reason = new DOMException("stop streaming", "AbortError");
  abortController.abort(reason);
  await assert.rejects(pending, (error) => error === reason);
});

test("ToriiBrowserClient turns contract stream_error events into typed gaps", async () => {
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => sseResponse([
      "event: stream_error\n",
      "data: {\"code\":\"stream_lagged\",\"message\":\"events were lost\",",
      "\"dropped_messages\":4,\"replay_available\":false}\n\n",
    ]),
  });

  await assert.rejects(
    client.streamContractEvents().next(),
    (error) => {
      assert(error instanceof ToriiBrowserStreamGapError);
      assert.equal(error.code, "stream_lagged");
      assert.equal(error.message, "events were lost");
      assert.equal(error.droppedMessages, 4);
      assert.equal(error.replayAvailable, false);
      assert.equal(error.payload.replay_available, false);
      return true;
    },
  );
});

test("ToriiBrowserClient treats contract stream EOF as a terminal non-replayable gap", async () => {
  let fetchCalls = 0;
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => {
      fetchCalls += 1;
      return sseResponse([]);
    },
  });

  await assert.rejects(
    client.streamContractEvents().next(),
    (error) => {
      assert(error instanceof ToriiBrowserStreamGapError);
      assert.equal(error.code, "stream_unexpected_eof");
      assert.equal(error.droppedMessages, null);
      assert.equal(error.replayAvailable, false);
      return true;
    },
  );
  assert.equal(fetchCalls, 1);
});

test("ToriiBrowserClient queryVisibleTransactions posts a browser-safe envelope", async () => {
  let capturedUrl;
  let capturedInit;
  const fetchImpl = async (url, init) => {
    capturedUrl = String(url);
    capturedInit = init;
    return jsonResponse({ items: [], total: 0 });
  };
  const client = new ToriiBrowserClient("https://torii.example/v1", {
    fetchImpl,
    defaultHeaders: { Authorization: "Bearer jwt" },
  });

  const payload = await client.queryVisibleTransactions({
    assetId: "FkLLi7B7cSmSLxwi3cHjB6ZyyEWSXb",
    select: [" entrypoint_hash ", { authority: true }],
    sort: "newest",
    limit: 25,
    fetch_size: 50,
    queryName: "VisibleTransactions",
    countMode: " BOUNDED ",
  });

  assert.equal(capturedUrl, "https://torii.example/v1/transactions/visible/query");
  assert.equal(capturedInit.method, "POST");
  assert.equal(capturedInit.headers.Authorization, "Bearer jwt");
  assert.deepEqual(JSON.parse(capturedInit.body), {
    pagination: { limit: 25 },
    sort: [
      { key: "timestamp_ms", order: "desc" },
      { key: "entrypoint_hash", order: "desc" },
    ],
    filter: {
      op: "eq",
      args: ["asset_id", "FkLLi7B7cSmSLxwi3cHjB6ZyyEWSXb"],
    },
    select: ["entrypoint_hash", { authority: true }],
    fetch_size: 50,
    query: "VisibleTransactions",
    count_mode: "bounded",
  });
  assert.deepEqual(payload, { items: [], total: 0 });
});

test("ToriiBrowserClient rejects adversarial query options before fetch", async () => {
  const fetchImpl = async () => {
    throw new Error("fetch should not be called for invalid local options");
  };
  const client = new ToriiBrowserClient("https://torii.example", { fetchImpl });

  assert.throws(
    () => client.listExplorerBlocks({ page: 0 }),
    /positive safe integer/,
  );
  assert.throws(
    () => client.queryVisibleTransactions({ sort: "timestamp_ms:drop" }),
    /asc or desc/,
  );
  assert.throws(
    () => client.queryVisibleTransactions({ sort: "timestamp_ms:desc:extra" }),
    /key:asc\/key:desc/,
  );
  assert.throws(
    () => client.queryVisibleTransactions({ sort: [{ key: "timestamp ms", order: "desc" }] }),
    /ASCII field name/,
  );
  assert.throws(
    () => client.queryVisibleTransactions({ select: "entrypoint_hash" }),
    /select must be an array/,
  );
  assert.throws(
    () => client.queryVisibleTransactions({ select: ["entrypoint_hash", []] }),
    /select\[1] must be a field-path string or plain object/,
  );
  assert.throws(
    () => client.queryVisibleTransactions({ select: ["entrypoint_hash", " "] }),
    /select\[1] must be a non-empty field path/,
  );
  assert.throws(
    () => client.queryVisibleTransactions({ count_mode: "full" }),
    /countMode must be bounded or exact/,
  );
  assert.throws(
    () => client.listAssetDefinitions({ countMode: "full" }),
    /countMode must be bounded or exact/,
  );
  assert.throws(
    () => client.resolveAlias("  "),
    /alias must not be empty/,
  );
});

test("ToriiBrowserClient preserves error responses for callers", async () => {
  const fetchImpl = async () => new Response("not found", { status: 404 });
  const client = new ToriiBrowserClient("https://localhost:8080", { fetchImpl });
  await assert.rejects(
    () => client.getExplorerRwa("missing$domain"),
    (error) => {
      assert(error instanceof ToriiBrowserHttpError);
      assert.equal(error.status, 404);
      assert.equal(error.bodyText, "not found");
      return true;
    },
  );
});

test("browser aggregate exports reusable browser-safe SDK APIs", () => {
  assert.equal(typeof browserSdk.AccountAddress, "function");
  assert.equal(typeof browserSdk.ToriiBrowserClient, "function");
  assert.equal(typeof browserSdk.ToriiBrowserStreamGapError, "function");
  assert.equal(typeof browserDistSdk.ToriiBrowserStreamGapError, "function");
  assert.equal(typeof browserSdk.normalizeAccountAliasFqn, "function");
  assert.equal(typeof browserSdk.noritoEncodeMultisigProposeRequest, "function");
  assert.equal(typeof browserSdk.noritoDecodeBlockProofs, "function");
  assert.equal(typeof browserSdk.verifyBlockProofs, "function");
  assert.equal(typeof browserSdk.NumericV1?.decodeQuantityJson, "function");
  assert.equal(typeof browserSdk.KotodamaQuantity, "function");
  assert.equal(typeof browserDistSdk.NumericV1?.decodeQuantityJson, "function");
  assert.equal(typeof browserDistSdk.KotodamaQuantity, "function");
  assert.throws(() => browserSdk.NumericV1.decodeQuantityJson("1.0"), {
    code: "invalid_text",
  });
  assert.throws(() => browserDistSdk.NumericV1.decodeQuantityJson("1.0"), {
    code: "invalid_text",
  });
});

test("ToriiBrowserClient rejects noncanonical asset and RWA quantity readbacks", async () => {
  const cases = [
    {
      payload: { items: [{ asset: "asset", quantity: "1.0" }], total: 1 },
      invoke: (client) => client.listAccountAssets("account"),
    },
    {
      payload: { items: [{ account_id: "account", quantity: -1 }], total: 1 },
      invoke: (client) => client.listAssetHolders("asset-definition"),
    },
    {
      payload: { pagination: {}, items: [{ id: "asset", quantity: "01" }] },
      invoke: (client) => client.listExplorerAssets(),
    },
    {
      payload: {
        pagination: {},
        items: [{ id: "rwa", quantity: "1", held_quantity: "0.0" }],
      },
      invoke: (client) => client.listExplorerRwas(),
    },
  ];
  for (const entry of cases) {
    const client = new ToriiBrowserClient("https://torii.example", {
      fetchImpl: async () => jsonResponse(entry.payload),
    });
    await assert.rejects(
      () => entry.invoke(client),
      /canonical (?:non-negative )?Kotodama V1 quantity/u,
    );
  }
});

test("ToriiBrowserClient does not statically import Node-only Norito code", () => {
  const source = readFileSync(
    new URL("../src/toriiBrowserClient.js", import.meta.url),
    "utf8",
  );
  assert.doesNotMatch(source, /from\s+["']\.\/norito\.js["']/);
});

test("ToriiBrowserClient source and dist use only first-release multisig proposal routes", () => {
  for (const relativePath of [
    "../src/toriiBrowserClient.js",
    "../dist/toriiBrowserClient.js",
  ]) {
    const source = readFileSync(new URL(relativePath, import.meta.url), "utf8");
    assert.doesNotMatch(
      source,
      /["']\/v1\/multisig\/proposals\/(?:list|get|search|lookup)["']/,
      `${relativePath} must not retain retired multisig proposal paths`,
    );
    assert.doesNotMatch(
      source,
      /\b(?:listMultisigProposals|getMultisigProposal)\b/,
      `${relativePath} must not retain retired multisig proposal methods`,
    );
    assert.match(source, /["']\/v1\/multisig\/proposals\/query["']/);
    assert.match(source, /["']\/v1\/multisig\/proposals\/resolve["']/);
    assert.match(source, /\bqueryMultisigProposals\b/);
    assert.match(source, /\bresolveMultisigProposal\b/);
  }
});

test("ToriiBrowserClient resolves aliases with JSON body", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(String(url), "https://torii.example/v1/aliases/resolve");
    assert.equal(init.method, "POST");
    assert.deepEqual(JSON.parse(init.body), { alias: "treasury@boi.is2" });
    return jsonResponse({ resolved: true, account_id: "account" });
  };
  const client = new ToriiBrowserClient("https://torii.example", { fetchImpl });
  const payload = await client.resolveAlias("treasury@boi.is2");
  assert.deepEqual(payload, { resolved: true, account_id: "account" });
});

test("ToriiBrowserClient posts selector-explicit multisig proposal reads", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url: String(url), init });
    return jsonResponse(
      calls.length === 1
        ? { resolved_multisig_account_id: FIXTURE_ALICE_ID, proposals: [] }
        : {
            resolved_multisig_account_id: FIXTURE_ALICE_ID,
            proposal_id: "a".repeat(64),
            instructions_hash: "a".repeat(64),
            proposal: { approvals: [], proposed_at_ms: 1 },
          },
    );
  };
  const client = new ToriiBrowserClient("https://torii.example", { fetchImpl });

  await client.queryMultisigProposals(
    {
      multisigAccountAlias: "cbdc@banka",
      status: ["collecting_signatures"],
      cursor: "page-1",
      limit: 25,
    },
    canonicalReadOptions(),
  );
  await client.resolveMultisigProposal(
    {
      multisigAccountAlias: "cbdc@banka",
      instructionsHash: "a".repeat(64),
    },
    canonicalReadOptions(),
  );

  assert.equal(calls[0].url, "https://torii.example/v1/multisig/proposals/query");
  assert.notEqual(calls[0].url, "https://torii.example/v1/multisig/proposals/list");
  assert.equal(calls[0].init.method, "POST");
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    multisig_account_alias: "cbdc@banka",
    status: ["COLLECTING_SIGNATURES"],
    cursor: "page-1",
    limit: 25,
  });
  assert.equal(calls[1].url, "https://torii.example/v1/multisig/proposals/resolve");
  assert.notEqual(calls[1].url, "https://torii.example/v1/multisig/proposals/get");
  assert.equal(calls[1].init.method, "POST");
  assert.deepEqual(JSON.parse(calls[1].init.body), {
    multisig_account_alias: "cbdc@banka",
    instructions_hash: "a".repeat(64),
  });
});

test("ToriiBrowserClient rejects unsupported multisig proposal statuses before fetch", () => {
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => {
      throw new Error("fetch should not be invoked");
    },
  });

  assert.throws(
    () =>
      client.queryMultisigProposals({
        multisigAccountAlias: "cbdc@banka",
        status: ["READY_TO_SUBMIT"],
      }),
    /must be one of COLLECTING_SIGNATURES, FINALIZED, CANCELED, EXPIRED/,
  );
});

test("ToriiBrowserClient rejects implicit and ambiguous multisig selectors before fetch", () => {
  let calls = 0;
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => {
      calls += 1;
      throw new Error("fetch should not be invoked");
    },
  });

  assert.throws(() => client.queryMultisigProposals({}), /requires exactly one/);
  assert.throws(
    () =>
      client.queryMultisigProposals({
        multisigAccountId: FIXTURE_ALICE_ID,
        multisigAccountAlias: "cbdc@banka",
      }),
    /requires exactly one/,
  );
  assert.throws(
    () => client.resolveMultisigProposal(FIXTURE_ALICE_ID, "b".repeat(64)),
    /must be an object/,
  );
  assert.throws(
    () =>
      client.resolveMultisigProposal({
        multisigAccountId: FIXTURE_ALICE_ID,
        proposalId: "b".repeat(64),
        instructionsHash: "b".repeat(64),
      }),
    /requires exactly one/,
  );
  assert.equal(calls, 0);
});

test("ToriiBrowserClient submits multisig Norito payloads to registered routes", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url: String(url), init });
    return jsonResponse({
      ok: true,
      resolved_multisig_account_id: FIXTURE_ALICE_ID,
      submitted: false,
    });
  };
  const client = new ToriiBrowserClient("https://torii.example", { fetchImpl });

  await client.submitMultisigPropose({
    multisigAccountAlias: "cbdc@banka",
    signerAccountId: FIXTURE_ALICE_ID,
    feePayment: AUTHORITY_FEE_PAYMENT,
    instructions: [{ Custom: { payload: { probe: true } } }],
  });
  await client.submitMultisigContractCallPropose({
    multisigAccountAlias: "cbdc@banka",
    signerAccountId: FIXTURE_ALICE_ID,
    contractAddress: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
    entrypoint: "execute",
    feePayment: AUTHORITY_FEE_PAYMENT,
  });
  await client.submitMultisigContractCallApprove({
    multisigAccountId: FIXTURE_ALICE_ID,
    signerAccountId: FIXTURE_BOB_ID,
    proposalId: "e".repeat(64),
    feePayment: AUTHORITY_FEE_PAYMENT,
  });

  assert.equal(calls[0].url, "https://torii.example/v1/multisig/propose");
  assert.equal(calls[0].init.headers["Content-Type"], "application/x-norito");
  assert.equal(calls[1].url, "https://torii.example/v1/contracts/call/multisig/propose");
  assert.equal(calls[1].init.headers["Content-Type"], "application/x-norito");
  assert.equal(calls[2].url, "https://torii.example/v1/contracts/call/multisig/approve");
  assert.equal(calls[2].init.headers["Content-Type"], "application/x-norito");
});

test("ToriiBrowserClient waits for exact global persisted Applied finality", async () => {
  const hash = "ab".repeat(32);
  const payloads = [
    {
      hash,
      status: { kind: "Applied", block_height: 17 },
      summary: "Applied",
      scope: "global",
      resolved_from: "cache",
    },
    {
      hash,
      status: { kind: "Applied", block_height: 17 },
      summary: "Applied",
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

test("ToriiBrowserClient rejects malformed Applied envelopes", async () => {
  const hash = "cd".repeat(32);
  for (const [payload, pattern] of [
    [
      {
        hash: "ef".repeat(32),
        status: { kind: "Applied", block_height: 1 },
        summary: "Applied",
        scope: "global",
        resolved_from: "state",
      },
      /does not match the requested transaction/u,
    ],
    [
      {
        hash,
        status: { kind: "Applied", block_height: 0 },
        summary: "Applied",
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

test("ToriiBrowserClient does not treat nested Committed markers as finality", async () => {
  const hash = "12".repeat(32);
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () =>
      jsonResponse({
        hash,
        status: { kind: "Queued", content: { Committed: true } },
        summary: "Queued",
        scope: "global",
        resolved_from: "queue",
      }),
  });

  await assert.rejects(
    client.waitForTransactionStatus(hash, { intervalMs: 0, maxAttempts: 1 }),
    /did not reach persisted Applied status/u,
  );
});

test("ToriiBrowserClient keeps diagnostic scopes separate from global-only waits", async () => {
  const hash = "56".repeat(32);
  const urls = [];
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async (url) => {
      urls.push(String(url));
      return new Response("", { status: 404 });
    },
  });

  assert.equal(await client.getTransactionStatus(hash), null);
  assert.equal(
    await client.getTransactionStatus(hash, { scope: undefined }),
    null,
  );
  assert.equal(await client.getTransactionStatus(hash, { scope: "local" }), null);
  assert.deepEqual(urls, [
    `https://torii.example/v1/pipeline/transactions/status?hash=${hash}&scope=global`,
    `https://torii.example/v1/pipeline/transactions/status?hash=${hash}&scope=global`,
    `https://torii.example/v1/pipeline/transactions/status?hash=${hash}&scope=local`,
  ]);
  for (const scope of [null, "", "auto"]) {
    await assert.rejects(
      client.getTransactionStatus(hash, { scope }),
      /must be local or global/u,
    );
  }
  for (const scope of [undefined, null, "global"]) {
    await assert.rejects(
      client.waitForTransactionStatus(hash, { scope }),
      /scope is not supported/u,
    );
  }

  let submissions = 0;
  const submittingClient = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => {
      submissions += 1;
      return new Response("", { status: 204 });
    },
  });
  for (const scope of [undefined, null, "global"]) {
    await assert.rejects(
      submittingClient.submitTransactionAndWait(Uint8Array.from([1, 2]), {
        scope,
      }),
      /scope is not supported/u,
    );
  }
  assert.equal(submissions, 0);
});
