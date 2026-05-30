import { test } from "node:test";
import assert from "node:assert/strict";

import { ToriiBrowserClient, ToriiBrowserHttpError } from "../src/toriiBrowserClient.js";

const BASE_URL = "https://localhost:8080/v1/explorer";

function jsonResponse(payload, init = {}) {
  return new Response(JSON.stringify(payload), {
    status: init.status ?? 200,
    headers: { "content-type": "application/json", ...(init.headers ?? {}) },
  });
}

test("ToriiBrowserClient strips API suffixes and calls current explorer block routes", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(String(url), "https://localhost:8080/v1/explorer/blocks?page=2&per_page=5");
    assert.equal(init.method, "GET");
    assert.equal(init.headers["x-iroha-api-version"], "1.1");
    return jsonResponse({
      pagination: { page: 2, per_page: 5, total_pages: 3, total_items: 11 },
      items: [],
    });
  };
  const client = new ToriiBrowserClient(BASE_URL, {
    fetchImpl,
    defaultHeaders: { "x-iroha-api-version": "1.1" },
  });
  const payload = await client.listExplorerBlocks({ page: 2, perPage: 5 });
  assert.equal(payload.pagination.page, 2);
});

test("ToriiBrowserClient account assets use the current asset selector query key", async () => {
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, "/v1/accounts/test-account/assets");
    assert.equal(parsed.searchParams.get("asset"), "asset-alias");
    assert.equal(parsed.searchParams.get("limit"), "10");
    assert.equal(parsed.searchParams.get("offset"), "20");
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
  });
  assert.equal(payload.items[0].asset, "asset-alias");
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
    sort: "newest",
    limit: 25,
    queryName: "VisibleTransactions",
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
    query: "VisibleTransactions",
  });
  assert.deepEqual(payload, { items: [], total: 0 });
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
