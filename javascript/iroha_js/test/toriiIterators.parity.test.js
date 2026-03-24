import { test } from "node:test";
import assert from "node:assert/strict";

import { ToriiClient, ToriiHttpError } from "../src/toriiClient.js";
import { ValidationError, ValidationErrorCode } from "../src/index.js";

const BASE_URL = "http://localhost:8080";
const FIXTURE_ACCOUNT_ID = "6cmzPVPX8e5qQsHdB57DhqFT9wp2MiMoXsvt9LYUtypj1nx96bF5s8W";

function createResponse({ status, jsonData = {}, headers, textBody }) {
  const headerMap = new Map();
  if (headers) {
    for (const [key, value] of Object.entries(headers)) {
      headerMap.set(key.toLowerCase(), value);
    }
  }
  return {
    status,
    headers: {
      get(name) {
        return headerMap.get(name.toLowerCase()) ?? null;
      },
    },
    async json() {
      return jsonData;
    },
    async text() {
      if (textBody !== undefined && textBody !== null) {
        return String(textBody);
      }
      return typeof jsonData === "string" ? jsonData : JSON.stringify(jsonData);
    },
  };
}

test("listNfts forwards pagination/sort/filter and validates filter payloads", async () => {
  const requests = [];
  const fetchImpl = async (url) => {
    requests.push(url);
    return createResponse({
      status: 200,
      jsonData: { items: [{ id: "5Pz9SwdN9eXPbiXPX9HRCpzCcE3o#0001" }], total: 1 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const payload = await client.listNfts({
    limit: 2,
    sort: [{ key: "id", order: "asc" }],
    filter: { Eq: ["id.definition_id", "5Pz9SwdN9eXPbiXPX9HRCpzCcE3o"] },
  });
  assert.equal(payload.items[0].id, "5Pz9SwdN9eXPbiXPX9HRCpzCcE3o#0001");
  assert.equal(requests.length, 1);
  const parsed = new URL(requests[0]);
  assert.equal(parsed.pathname, "/v1/nfts");
  assert.equal(parsed.searchParams.get("limit"), "2");
  assert.equal(parsed.searchParams.get("sort"), "id:asc");
  const parsedFilter = parsed.searchParams.get("filter");
  assert.ok(parsedFilter);
  assert.deepEqual(JSON.parse(parsedFilter), { Eq: ["id.definition_id", "5Pz9SwdN9eXPbiXPX9HRCpzCcE3o"] });

  const badClient = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => badClient.listNfts({ filter: "   " }),
    (error) =>
      error instanceof ValidationError &&
      error.code === ValidationErrorCode.INVALID_STRING &&
      error.path === "filter",
  );
  assert.equal(requests.length, 1, "invalid filter must not issue fetch calls");
});

test("iterateAccountAssets paginates with maxItems", async () => {
  let callCount = 0;
  const expectedPath = `/v1/accounts/${encodeURIComponent(FIXTURE_ACCOUNT_ID)}/assets`;
  const firstAssetId = "norito:deadbeef";
  const secondAssetId = "norito:cafebabe";
  const fetchImpl = async (url) => {
    const parsed = new URL(url);
    assert.equal(parsed.pathname, expectedPath);
    assert.equal(parsed.searchParams.get("canonical_i105"), null);
    const offset = Number(parsed.searchParams.get("offset") ?? 0);
    const items =
      offset === 0
        ? [{ asset_id: firstAssetId, quantity: "5" }]
        : [{ asset_id: secondAssetId, quantity: "1" }];
    callCount += 1;
    return createResponse({
      status: 200,
      jsonData: { items, total: 3 },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  const seen = [];
  for await (const holding of client.iterateAccountAssets(FIXTURE_ACCOUNT_ID, {
    pageSize: 1,
    maxItems: 2,
  })) {
    seen.push(holding.asset_id);
  }
  assert.deepEqual(seen, [firstAssetId, secondAssetId]);
  assert.equal(callCount, 2);
});

test("queryAccountAssets surfaces permission errors with context", async () => {
  let calls = 0;
  const fetchImpl = async () => {
    calls += 1;
    return createResponse({
      status: 403,
      jsonData: { code: "FORBIDDEN", message: "account missing ReadAssets" },
      headers: { "content-type": "application/json" },
    });
  };
  const client = new ToriiClient(BASE_URL, { fetchImpl });
  await assert.rejects(
    () => client.queryAccountAssets(FIXTURE_ACCOUNT_ID, { filter: { Eq: ["quantity", 1] } }),
    (error) => {
      assert(error instanceof ToriiHttpError);
      assert.equal(error.status, 403);
      assert.equal(error.code, "FORBIDDEN");
      assert.equal(error.errorMessage, "account missing ReadAssets");
      assert.match(error.message, /HTTP 403/);
      return true;
    },
  );
  assert.equal(calls, 1);
});
