import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { AccountAddress } from "../src/address.js";
import { ToriiBrowserClient, ToriiBrowserHttpError } from "../src/toriiBrowserClient.js";
import * as browserSdk from "../src/browser.js";

const BASE_URL = "https://localhost:8080/v1/explorer";
const FIXTURE_ALICE_ID = AccountAddress.fromAccount({
  publicKey: Buffer.from(
    "B935AAF1F4E44B3DB79E5E5A9BA4569E6F3E2310C219F3DDD56D3277828D5480",
    "hex",
  ),
}).toI105();
const FIXTURE_BOB_ID = AccountAddress.fromAccount({
  publicKey: Buffer.from(
    "641297079357229F295938A4B5A333DE35069BF47B9D0704E45805713D13C201",
    "hex",
  ),
}).toI105();

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
  assert.equal(typeof browserSdk.normalizeAccountAliasFqn, "function");
  assert.equal(typeof browserSdk.noritoEncodeMultisigProposeRequest, "function");
});

test("ToriiBrowserClient does not statically import Node-only Norito code", () => {
  const source = readFileSync(
    new URL("../src/toriiBrowserClient.js", import.meta.url),
    "utf8",
  );
  assert.doesNotMatch(source, /from\s+["']\.\/norito\.js["']/);
});

test("ToriiBrowserClient resolves aliases with JSON body", async () => {
  const fetchImpl = async (url, init) => {
    assert.equal(String(url), "https://torii.example/v1/aliases/resolve");
    assert.equal(init.method, "POST");
    assert.deepEqual(JSON.parse(init.body), { alias: "cbdc@pob.cbsi" });
    return jsonResponse({ resolved: true, account_id: "account" });
  };
  const client = new ToriiBrowserClient("https://torii.example", { fetchImpl });
  const payload = await client.resolveAlias("cbdc@pob.cbsi");
  assert.deepEqual(payload, { resolved: true, account_id: "account" });
});

test("ToriiBrowserClient posts multisig proposal lookups to registered routes", async () => {
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

  await client.listMultisigProposals({
    multisigAccountAlias: "cbdc@banka",
    status: ["COLLECTING_SIGNATURES"],
  });
  await client.getMultisigProposal({
    multisigAccountAlias: "cbdc@banka",
    instructionsHash: "a".repeat(64),
  });

  assert.equal(calls[0].url, "https://torii.example/v1/multisig/proposals/list");
  assert.equal(calls[0].init.method, "POST");
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    multisig_account_alias: "cbdc@banka",
    status: ["COLLECTING_SIGNATURES"],
  });
  assert.equal(calls[1].url, "https://torii.example/v1/multisig/proposals/get");
  assert.equal(calls[1].init.method, "POST");
  assert.deepEqual(JSON.parse(calls[1].init.body), {
    multisig_account_alias: "cbdc@banka",
    instructions_hash: "a".repeat(64),
  });
});

test("ToriiBrowserClient preserves legacy multisig proposal lookup signature", async () => {
  let captured;
  const fetchImpl = async (url, init) => {
    captured = { url: String(url), init };
    return jsonResponse({
      resolved_multisig_account_id: FIXTURE_ALICE_ID,
      proposal_id: "b".repeat(64),
      instructions_hash: "b".repeat(64),
      proposal: { approvals: [], proposed_at_ms: 1 },
    });
  };
  const client = new ToriiBrowserClient("https://torii.example", { fetchImpl });
  await client.getMultisigProposal(FIXTURE_ALICE_ID, "b".repeat(64));

  assert.equal(captured.url, "https://torii.example/v1/multisig/proposals/get");
  assert.deepEqual(JSON.parse(captured.init.body), {
    multisig_account_id: FIXTURE_ALICE_ID,
    proposal_id: "b".repeat(64),
  });
});

test("ToriiBrowserClient posts multisig approvals to registered routes", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url: String(url), init });
    return jsonResponse(
      calls.length === 1
        ? { items: [], next_cursor: null }
        : {
            item: {
              multisig_account_id: FIXTURE_ALICE_ID,
              spec: { signatories: {}, quorum: 1, transaction_ttl_ms: 60000 },
              proposal_id: "c".repeat(64),
              instructions_hash: "c".repeat(64),
              proposal: { approvals: [], proposed_at_ms: 1 },
              operation_type: "TRANSFER",
              status: "COLLECTING_SIGNATURES",
            },
          },
    );
  };
  const client = new ToriiBrowserClient("https://torii.example", { fetchImpl });

  await client.listMultisigApprovals({
    status: ["COLLECTING_SIGNATURES"],
    operationType: ["TRANSFER"],
    requiresMySignature: true,
    limit: 5,
  });
  await client.getMultisigApproval({ proposalId: "c".repeat(64) });

  assert.equal(calls[0].url, "https://torii.example/v1/multisig/approvals/list");
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    status: ["COLLECTING_SIGNATURES"],
    operation_type: ["TRANSFER"],
    requires_my_signature: true,
    limit: 5,
  });
  assert.equal(calls[1].url, "https://torii.example/v1/multisig/approvals/get");
  assert.deepEqual(JSON.parse(calls[1].init.body), {
    proposal_id: "c".repeat(64),
  });
});

test("ToriiBrowserClient maps pending approval compatibility helpers to approval POST routes", async () => {
  const calls = [];
  const fetchImpl = async (url, init) => {
    calls.push({ url: String(url), init });
    return jsonResponse(calls.length === 1 ? { items: [] } : { item: {} });
  };
  const client = new ToriiBrowserClient("https://torii.example", { fetchImpl });

  await client.listPendingMultisigApprovals({ operationType: ["MINT"], limit: 3 });
  await client.getPendingMultisigApproval("d".repeat(64));

  assert.equal(calls[0].url, "https://torii.example/v1/multisig/approvals/list");
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    operation_type: ["MINT"],
    limit: 3,
    status: ["COLLECTING_SIGNATURES"],
  });
  assert.equal(calls[1].url, "https://torii.example/v1/multisig/approvals/get");
  assert.deepEqual(JSON.parse(calls[1].init.body), {
    proposal_id: "d".repeat(64),
  });
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
    instructions: [{ Custom: { payload: { probe: true } } }],
  });
  await client.submitMultisigContractCallPropose({
    multisigAccountAlias: "cbdc@banka",
    signerAccountId: FIXTURE_ALICE_ID,
    contractAddress: "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7",
    entrypoint: "execute",
  });
  await client.submitMultisigContractCallApprove({
    multisigAccountId: FIXTURE_ALICE_ID,
    signerAccountId: FIXTURE_BOB_ID,
    proposalId: "e".repeat(64),
  });

  assert.equal(calls[0].url, "https://torii.example/v1/multisig/propose");
  assert.equal(calls[0].init.headers["Content-Type"], "application/x-norito");
  assert.equal(calls[1].url, "https://torii.example/v1/contracts/call/multisig/propose");
  assert.equal(calls[1].init.headers["Content-Type"], "application/x-norito");
  assert.equal(calls[2].url, "https://torii.example/v1/contracts/call/multisig/approve");
  assert.equal(calls[2].init.headers["Content-Type"], "application/x-norito");
});
