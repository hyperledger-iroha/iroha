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
const OFFLINE_OPERATION_BYTES = Array.from({ length: 32 }, () => 0x11);
const OFFLINE_OPERATION_ID = "11".repeat(32);
const OFFLINE_TRANSACTION_HASH = "22".repeat(32);
const OFFLINE_STATUS_URI = `/v1/offline/operations/${OFFLINE_OPERATION_ID}`;
const OFFLINE_CANONICAL_ASSET_DEFINITION_ID = "7EAD8EFYUx1aVKZPUU1fyKvr8dF1";

function browserActiveTransferVerifier() {
  return {
    id: { backend: "halo2-ipa-pasta", name: "transfer-v2" },
    version: 7,
    circuit_id: "confidential-transfer-v2",
    commitment: "44".repeat(32),
    public_inputs_schema_hash: "55".repeat(32),
    max_proof_bytes: 4096,
    activation_height: 1,
    withdrawal_height: null,
  };
}

function jsonResponse(payload, init = {}) {
  return new Response(JSON.stringify(payload), {
    status: init.status ?? 200,
    headers: { "content-type": "application/json", ...(init.headers ?? {}) },
  });
}

function browserOfflineTopUpRequest(overrides = {}) {
  return {
    asset: "xor#sora",
    amount: { atomic_units: 9_007_199_254_740_993n, scale: 4 },
    current_note: { version: 2 },
    record_bundle: { version: 2 },
    pallas_open_envelopes_archive: [1, 2],
    artifact_generation: "generation-1",
    operation_id: [...OFFLINE_OPERATION_BYTES],
    authorization: { operation_id: [...OFFLINE_OPERATION_BYTES] },
    ...overrides,
  };
}

test("ToriiBrowserClient implements the complete first-release Offline JSON flow", async () => {
  const requests = [];
  const fetchImpl = async (url, init) => {
    const parsed = new URL(url);
    requests.push({ parsed, init });
    if (parsed.pathname === "/v1/offline/readiness") {
      return jsonResponse({
        asset_definition_id: OFFLINE_CANONICAL_ASSET_DEFINITION_ID,
        asset_scale: 4,
        evaluated_block_height: 7,
        evaluated_block_hash: "ab".repeat(32),
        active_transfer_verifier: browserActiveTransferVerifier(),
        ready: true,
        blockers: [],
      });
    }
    if (parsed.pathname === "/v1/offline/top-up") {
      return jsonResponse(
        {
          operation_id: OFFLINE_OPERATION_ID,
          kind: { kind: "top_up", value: null },
          state: { state: "pending", value: null },
          transaction_hash: OFFLINE_TRANSACTION_HASH,
          status_uri: OFFLINE_STATUS_URI,
          submitted_at_ms: 10,
        },
        { status: 202, headers: { location: OFFLINE_STATUS_URI } },
      );
    }
    return jsonResponse({
      state: "pending",
      value: {
        operation_id: OFFLINE_OPERATION_ID,
        kind: { kind: "top_up", value: null },
        transaction_hash: OFFLINE_TRANSACTION_HASH,
        submitted_at_ms: 10,
      },
    });
  };
  const client = new ToriiBrowserClient("https://torii.example/v1", { fetchImpl });

  const readiness = await client.getOfflineReadiness("xor#sora");
  assert.equal(readiness.ready, true);
  const reference = await client.submitOfflineTopUp(browserOfflineTopUpRequest());
  assert.equal(reference.operation_id, OFFLINE_OPERATION_ID);
  const status = await client.getOfflineOperationStatus(OFFLINE_OPERATION_ID);
  assert.equal(status.state, "pending");

  assert.equal(requests[0].parsed.pathname, "/v1/offline/readiness");
  assert.equal(requests[0].parsed.searchParams.get("asset_definition_id"), "xor#sora");
  assert.equal(requests[1].parsed.pathname, "/v1/offline/top-up");
  assert.equal(requests[1].init.headers["Idempotency-Key"], OFFLINE_OPERATION_ID);
  assert.equal(requests[1].init.headers["Content-Type"], "application/json");
  assert.match(requests[1].init.body, /"atomic_units":9007199254740993/u);
  assert.equal(requests[2].parsed.pathname, OFFLINE_STATUS_URI);
});

test("ToriiBrowserClient preserves wide Offline response integers", async () => {
  const fetchImpl = async () => new Response(
    `{"asset_definition_id":"${OFFLINE_CANONICAL_ASSET_DEFINITION_ID}",`
      + `"asset_scale":4,"evaluated_block_height":18446744073709551615,"evaluated_block_hash":"${"ab".repeat(32)}",`
      + `"active_transfer_verifier":${JSON.stringify(browserActiveTransferVerifier())},"ready":true,"blockers":[]}`,
    { status: 200, headers: { "content-type": "application/json" } },
  );
  const client = new ToriiBrowserClient("https://torii.example", { fetchImpl });

  const readiness = await client.getOfflineReadiness("xor#sora");
  assert.equal(readiness.evaluated_block_height, (1n << 64n) - 1n);
});

test("ToriiBrowserClient rejects adversarial Offline inputs before fetch", async () => {
  let fetchCount = 0;
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => {
      fetchCount += 1;
      throw new Error("must not fetch");
    },
  });
  await assert.rejects(() => client.getOfflineReadiness(" xor#sora"), /assetDefinitionId/);
  await assert.rejects(
    () => client.submitOfflineTopUp(browserOfflineTopUpRequest({ operation_id: Array(32).fill(0) })),
    /all zero/,
  );
  await assert.rejects(
    () => client.submitOfflineTopUp(browserOfflineTopUpRequest({
      authorization: { operation_id: Array(32).fill(0x12) },
    })),
    /must match/,
  );
  await assert.rejects(
    () => client.submitOfflineTopUp(browserOfflineTopUpRequest({
      amount: { atomic_units: 1, scale: 29 },
    })),
    /scale/,
  );
  await assert.rejects(
    () => client.submitOfflineTopUp(browserOfflineTopUpRequest({
      artifact_generation: "é".repeat(65),
    })),
    /128/,
  );
  await assert.rejects(
    () => client.getOfflineOperationStatus("AB".repeat(32)),
    /lowercase/,
  );
  await assert.rejects(
    () => client.getOfflineReadiness("xor#sora", { headers: {} }),
    /unsupported fields/,
  );
  assert.equal(fetchCount, 0);
});

test("ToriiBrowserClient requires the canonical Location on Offline acceptance", async () => {
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => jsonResponse(
      {
        operation_id: OFFLINE_OPERATION_ID,
        kind: { kind: "top_up", value: null },
        state: { state: "pending", value: null },
        transaction_hash: OFFLINE_TRANSACTION_HASH,
        status_uri: OFFLINE_STATUS_URI,
        submitted_at_ms: 10,
      },
      { status: 202 },
    ),
  });
  await assert.rejects(
    () => client.submitOfflineTopUp(browserOfflineTopUpRequest()),
    /Location header/,
  );

  const wrongMediaTypeClient = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => new Response(JSON.stringify({
      asset_definition_id: "xor#sora",
      evaluated_block_height: 1,
      evaluated_block_hash: "ab".repeat(32),
      ready: true,
      blockers: [],
    }), { status: 200, headers: { "content-type": "text/plain" } }),
  });
  await assert.rejects(
    () => wrongMediaTypeClient.getOfflineReadiness("xor#sora"),
    /Content-Type application\/json/,
  );
});

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

  await client.listMultisigProposals({
    multisigAccountAlias: "cbdc@banka",
    status: ["collecting_signatures"],
    cursor: "page-1",
    limit: 25,
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
    cursor: "page-1",
    limit: 25,
  });
  assert.equal(calls[1].url, "https://torii.example/v1/multisig/proposals/get");
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
      client.listMultisigProposals({
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

  assert.throws(() => client.listMultisigProposals({}), /requires exactly one/);
  assert.throws(
    () =>
      client.listMultisigProposals({
        multisigAccountId: FIXTURE_ALICE_ID,
        multisigAccountAlias: "cbdc@banka",
      }),
    /requires exactly one/,
  );
  assert.throws(
    () => client.getMultisigProposal(FIXTURE_ALICE_ID, "b".repeat(64)),
    /must be an object/,
  );
  assert.throws(
    () =>
      client.getMultisigProposal({
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
