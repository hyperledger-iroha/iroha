import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { sha256 } from "@noble/hashes/sha2";
import { ed25519 } from "@noble/curves/ed25519";

import { AccountAddress } from "../src/address.js";
import { blake2b256 } from "../src/blake2b.js";
import {
  ToriiBrowserClient,
  ToriiBrowserHttpError,
  ToriiBrowserStreamGapError,
} from "../src/toriiBrowserClient.js";
import {
  browserSignedTransactionHashHex,
  browserTransactionPayloadHashHex,
  buildBrowserTransferPayload,
  finalizeBrowserSignedTransaction,
} from "../src/transactionCodec.js";
import { NetworkId } from "../src/networkId.js";
import * as browserSdk from "../src/browser.js";
import * as browserDistSdk from "../dist/browser.js";
import {
  browserSumeragiDiagnosticsFixture,
  browserSumeragiStatusFixture,
} from "./sumeragiBrowserFixtures.js";

const BASE_URL = "https://localhost:8080/v1/explorer";
const QUERY_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const FOREIGN_QUERY_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa7));
const BROWSER_OPERATOR_CONTEXT = new browserSdk.OperatorSigningContext(
  QUERY_NETWORK_ID,
  {
    publicKey:
      "ed012066BE7E332C7A453332BD9D0A7F7DB055F5C5EF1A06ADA66D98B39FB6810C473A",
    sign: async () => Buffer.alloc(64, 0x22),
  },
);
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

function compactHashSignedTransactionFixture() {
  const privateKey = Buffer.alloc(32, 7);
  const publicKey = Buffer.from(ed25519.getPublicKey(privateKey));
  const networkId = NetworkId.parse(
    "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
  );
  const authority = AccountAddress.fromAccount({ publicKey }).toI105();
  const payload = buildBrowserTransferPayload({
    networkId,
    authority,
    sourceAssetHoldingId: `62Fk4FPcMuLvW5QjDGNF2a4jAmjM#${authority}`,
    quantity: "1",
    destinationAccountId: FIXTURE_BOB_ID,
    feePayment: { payer: "authority", chargeLimits: [] },
    creationTimeMs: 1_700_000_000_000,
    ttlMs: 5_000,
    nonce: 42,
  });
  const payloadHashHex = browserTransactionPayloadHashHex(payload);
  const signature = Buffer.from(
    ed25519.sign(Buffer.from(payloadHashHex, "hex"), privateKey),
  );
  return finalizeBrowserSignedTransaction(
    { networkId, payloadBytes: payload, payloadHashHex, authority, signingPublicKey: publicKey },
    { algorithm: "ed25519", signature },
    publicKey,
  ).signedTransaction;
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
  const entryHash = Buffer.alloc(32, 0x20);
  entryHash[31] |= 1;
  const resultHash = Buffer.alloc(32, 0x50);
  resultHash[31] |= 1;
  const blockHash = Buffer.alloc(32, 0x30);
  blockHash[31] |= 1;
  const executedBlockWireHash = Buffer.alloc(32, 0x40);
  executedBlockWireHash[31] |= 1;
  const entryRoot = Buffer.from(
    blake2b256(
      Buffer.concat([
        Buffer.from("iroha:merkle:leaf:v1\0", "utf8"),
        entryHash,
      ]),
    ),
  );
  entryRoot[31] |= 1;
  const resultRoot = Buffer.from(
    blake2b256(
      Buffer.concat([
        Buffer.from("iroha:merkle:leaf:v1\0", "utf8"),
        resultHash,
      ]),
    ),
  );
  resultRoot[31] |= 1;
  const leafIndex = Buffer.alloc(4);
  const entryCommitment = struct(entryRoot, u64(1));
  const receipt = struct(entryHash, struct(leafIndex, u64(0)));
  const resultCommitment = struct(resultRoot, u64(1));
  const resultReceipt = struct(resultHash, struct(leafIndex, u64(0)));
  const payload = struct(
    u64(7),
    blockHash,
    executedBlockWireHash,
    entryHash,
    entryCommitment,
    receipt,
    resultCommitment,
    resultReceipt,
    u64(0),
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

function authenticatedBlockProofAnchorFixture() {
  const entryHash = Buffer.alloc(32, 0x20);
  entryHash[31] |= 1;
  const resultHash = Buffer.alloc(32, 0x50);
  resultHash[31] |= 1;
  const blockHash = Buffer.alloc(32, 0x30);
  blockHash[31] |= 1;
  const executedBlockWireHash = Buffer.alloc(32, 0x40);
  executedBlockWireHash[31] |= 1;
  const merkleLeafDomain = Buffer.from("iroha:merkle:leaf:v1\0", "utf8");
  const entryRoot = Buffer.from(
    blake2b256(Buffer.concat([merkleLeafDomain, entryHash])),
  );
  entryRoot[31] |= 1;
  const resultRoot = Buffer.from(
    blake2b256(Buffer.concat([merkleLeafDomain, resultHash])),
  );
  resultRoot[31] |= 1;
  return {
    block_height: "7",
    block_hash: blockHash.toString("hex"),
    executed_block_wire_hash: executedBlockWireHash.toString("hex"),
    entry_hash: entryHash.toString("hex"),
    entry_index: 0,
    entry_commitment: { root: entryRoot.toString("hex"), leaf_count: "1" },
    result_commitment: { root: resultRoot.toString("hex"), leaf_count: "1" },
    fastpq_transcripts: {},
  };
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

test("ToriiBrowserClient uses opaque cursors for world Explorer lists", async () => {
  const cursor = "ZXhwbG9yZXItY3Vyc29y";
  const nextCursor = "bmV4dA";
  const expectedPaths = [
    ["/v1/explorer/accounts", (client) => client.listExplorerAccounts({ cursor, limit: 10 })],
    ["/v1/explorer/domains", (client) => client.listExplorerDomains({ cursor, limit: 10 })],
    [
      "/v1/explorer/asset-definitions",
      (client) => client.listExplorerAssetDefinitions({ cursor, limit: 10 }),
    ],
    ["/v1/explorer/assets", (client) => client.listExplorerAssets({ cursor, limit: 10 })],
    ["/v1/explorer/nfts", (client) => client.listExplorerNfts({ cursor, limit: 10 })],
    ["/v1/explorer/rwas", (client) => client.listExplorerRwas({ cursor, limit: 10 })],
  ];
  const seen = [];
  const client = new ToriiBrowserClient("https://localhost:8080", {
    fetchImpl: async (url) => {
      const parsed = new URL(url);
      seen.push(parsed.pathname);
      assert.equal(parsed.searchParams.get("cursor"), cursor);
      assert.equal(parsed.searchParams.get("limit"), "10");
      assert.equal(parsed.searchParams.get("page"), null);
      assert.equal(parsed.searchParams.get("per_page"), null);
      return jsonResponse({
        pagination: { limit: 10, next_cursor: nextCursor, has_more: true },
        items: [],
      });
    },
  });

  for (const [path, invoke] of expectedPaths) {
    const page = await invoke(client);
    assert.equal(seen.at(-1), path);
    assert.deepEqual(page.pagination, {
      limit: 10,
      next_cursor: nextCursor,
      has_more: true,
    });
  }
});

test("ToriiBrowserClient uses explicit asset-definition ownership fields", async () => {
  const item = {
    id: "11111111-1111-4111-8111-111111111111",
    owning_domain: null,
    mintable: "Infinitely",
    logo: null,
    metadata: {},
    owned_by: FIXTURE_ALICE_ID,
    assets: 0,
    total_quantity: "0",
    locked_quantity: null,
    circulating_quantity: null,
  };
  const client = new ToriiBrowserClient("https://localhost:8080", {
    fetchImpl: async (url) => {
      const parsed = new URL(url);
      assert.equal(parsed.searchParams.get("owning_domain"), "treasury.universal");
      assert.equal(parsed.searchParams.get("domain"), null);
      return jsonResponse({
        pagination: { limit: 10, next_cursor: null, has_more: false },
        items: [item],
      });
    },
  });

  const page = await client.listExplorerAssetDefinitions({
    limit: 10,
    owningDomain: "treasury.universal",
  });
  assert.equal(page.items[0].owning_domain, null);

  const missingOwnership = { ...item };
  delete missingOwnership.owning_domain;
  const invalidClient = new ToriiBrowserClient("https://localhost:8080", {
    fetchImpl: async () =>
      jsonResponse({
        pagination: { limit: 10, next_cursor: null, has_more: false },
        items: [missingOwnership],
      }),
  });
  await assert.rejects(
    invalidClient.listExplorerAssetDefinitions({ limit: 10 }),
    /missing or unsupported fields/u,
  );
});

test("ToriiBrowserClient rejects invalid world Explorer cursor contracts", async () => {
  let fetchCalls = 0;
  const localClient = new ToriiBrowserClient("https://localhost:8080", {
    fetchImpl: async () => {
      fetchCalls += 1;
      throw new Error("must not fetch");
    },
  });
  assert.throws(
    () => localClient.listExplorerAccounts({ page: 2 }),
    /page is not supported; use cursor and limit/u,
  );
  assert.throws(
    () => localClient.listExplorerNfts({ cursor: "padded==" }),
    /canonical base64url without padding/u,
  );
  assert.throws(
    () => localClient.listExplorerAccounts({ cursor: "AB" }),
    /canonical base64url without padding/u,
  );
  assert.throws(
    () => localClient.listExplorerRwas({ limit: 101 }),
    /limit must be between 1 and 100/u,
  );
  assert.equal(fetchCalls, 0);

  const malformedClient = new ToriiBrowserClient("https://localhost:8080", {
    fetchImpl: async () => jsonResponse({
      pagination: { limit: 25, next_cursor: null, has_more: true },
      items: [],
    }),
  });
  await assert.rejects(
    () => malformedClient.listExplorerDomains(),
    /has_more must match next_cursor availability/u,
  );

  const unknownFieldClient = new ToriiBrowserClient("https://localhost:8080", {
    fetchImpl: async () => jsonResponse({
      pagination: { limit: 25, next_cursor: null, has_more: false },
      items: [],
      total_items: 0,
    }),
  });
  await assert.rejects(
    () => unknownFieldClient.listExplorerAccounts(),
    /contains unknown field total_items/u,
  );

  const oversizedPageClient = new ToriiBrowserClient("https://localhost:8080", {
    fetchImpl: async () => jsonResponse({
      pagination: { limit: 1, next_cursor: null, has_more: false },
      items: [{}, {}],
    }),
  });
  await assert.rejects(
    () => oversizedPageClient.listExplorerAssets(),
    /items must not exceed pagination\.limit/u,
  );
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
  assert.deepEqual(Object.keys(proof), [
    "block_height",
    "block_hash",
    "executed_block_wire_hash",
    "entry_hash",
    "entry_commitment",
    "entry_proof",
    "result_commitment",
    "result_proof",
    "fastpq_transcripts",
  ]);
  assert.equal(proof.block_height, "7");
  assert.match(proof.block_hash, /^hash:[0-9A-F]{64}#[0-9A-F]{4}$/u);
  assert.match(
    proof.executed_block_wire_hash,
    /^hash:[0-9A-F]{64}#[0-9A-F]{4}$/u,
  );
  assert.equal(proof.entry_proof.leaf, proof.entry_hash);
  assert.deepEqual(proof.entry_proof.proof, { leaf_index: 0, audit_path: [] });
  assert.equal(proof.entry_commitment.leaf_count, "1");
  assert.notEqual(proof.entry_commitment.root, proof.entry_hash);
  assert.equal(proof.result_commitment.leaf_count, "1");
  assert.equal(proof.result_proof.proof.leaf_index, 0);
  assert.notEqual(proof.result_commitment.root, proof.result_proof.leaf);
  assert.deepEqual(proof.fastpq_transcripts, {});

  assert.equal(
    browserSdk.verifyBlockProofs(
      proof,
      authenticatedBlockProofAnchorFixture(),
    ).valid,
    true,
  );
  assert.equal(browserSdk.verifyBlockProofs(proof).valid, false);
});

test("ToriiBrowserClient fetches the bounded exact executed block wire", async () => {
  const expectedWire = Buffer.from([1, 0x4e, 0x52, 0x54, 0x30, 0xaa]);
  let captured;
  const client = new ToriiBrowserClient(BASE_URL, {
    defaultHeaders: { Accept: "application/json", "x-test-client": "ledger" },
    fetchImpl: async (url, init) => {
      captured = { url: String(url), init };
      return new Response(expectedWire, {
        headers: { "content-type": "application/x-norito" },
      });
    },
  });

  const actualWire = await client.getLedgerExecutedBlockWire(7);
  assert.equal(captured.url, "https://localhost:8080/v1/ledger/block/7");
  assert.equal(captured.init.headers.Accept, "application/x-norito");
  assert.equal(captured.init.headers["x-test-client"], "ledger");
  assert.deepEqual(actualWire, expectedWire);

  const maximumHeightWire = await client.getLedgerExecutedBlockWire(
    "18446744073709551615",
  );
  assert.equal(
    captured.url,
    "https://localhost:8080/v1/ledger/block/18446744073709551615",
  );
  assert.deepEqual(maximumHeightWire, expectedWire);
});

test("ToriiBrowserClient rejects malformed ledger selectors and representations locally", async () => {
  let fetchCalls = 0;
  const client = new ToriiBrowserClient(BASE_URL, {
    fetchImpl: async () => {
      fetchCalls += 1;
      return jsonResponse({});
    },
  });

  assert.throws(() => client.listLedgerHeaders({ from: 0 }), /positive decimal integer/u);
  assert.throws(() => client.listLedgerHeaders({ offset: 1 }), /unsupported option offset/u);
  assert.throws(() => client.getLedgerStateRoot(0), /positive decimal integer/u);
  await assert.rejects(
    client.getLedgerExecutedBlockWire(0),
    /positive decimal integer/u,
  );
  await assert.rejects(
    client.getLedgerExecutedBlockWire(1n << 64n),
    /must not exceed 18446744073709551615/u,
  );
  await assert.rejects(
    client.getLedgerExecutedBlockWire(1, { offset: 1 }),
    /unsupported option offset/u,
  );
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
  await assert.rejects(
    wrongContentTypeClient.getLedgerExecutedBlockWire(1),
    /must return application\/x-norito/u,
  );

  const emptyClient = new ToriiBrowserClient(BASE_URL, {
    fetchImpl: async () => new Response(new Uint8Array(), {
      headers: { "content-type": "application/x-norito" },
    }),
  });
  await assert.rejects(
    emptyClient.getLedgerExecutedBlockWire(1),
    /must not be empty/u,
  );

  const oversizedClient = new ToriiBrowserClient(BASE_URL, {
    fetchImpl: async () => new Response(Uint8Array.of(1), {
      headers: {
        "content-type": "application/x-norito",
        "content-length": String(
          browserSdk.AUTHENTICATED_BLOCK_PROOFS_MAX_BLOCK_WIRE_BYTES_V1 + 1,
        ),
      },
    }),
  });
  await assert.rejects(
    oversizedClient.getLedgerExecutedBlockWire(1),
    /33554432-byte response limit/u,
  );

  const lengthMismatchClient = new ToriiBrowserClient(BASE_URL, {
    fetchImpl: async () => new Response(Uint8Array.of(1), {
      headers: {
        "content-type": "application/x-norito",
        "content-length": "2",
      },
    }),
  });
  await assert.rejects(
    lengthMismatchClient.getLedgerExecutedBlockWire(1),
    /Content-Length does not match/u,
  );

  const fragmentedClient = new ToriiBrowserClient(BASE_URL, {
    fetchImpl: async () => new Response(new ReadableStream({
      start(controller) {
        for (let index = 0; index <= 16_384; index += 1) {
          controller.enqueue(Uint8Array.of(index & 0xff));
        }
        controller.close();
      },
    }), {
      headers: { "content-type": "application/x-norito" },
    }),
  });
  await assert.rejects(
    fragmentedClient.getLedgerExecutedBlockWire(1),
    /too many fragmented response chunks/u,
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
    networkId: QUERY_NETWORK_ID,
  });

  const payload = await client.queryVisibleTransactions({
    ...canonicalReadOptions(),
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
  assert.equal(capturedInit.redirect, "error");
  assert.equal(capturedInit.headers.Authorization, "Bearer jwt");
  assert.equal(
    Buffer.from(capturedInit.headers["X-Iroha-Account"], "latin1").toString("utf8"),
    FIXTURE_ALICE_ID,
  );
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

test("ToriiBrowserClient transaction queries bind exact genesis, path, and body", async () => {
  const messages = [];
  const fetchImpl = async () => jsonResponse({ items: [], total: 0 });
  const sign = async ({ message }) => {
    messages.push(Buffer.from(message));
    return Buffer.alloc(64, messages.length);
  };
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl,
    networkId: QUERY_NETWORK_ID,
  });
  const foreign = new ToriiBrowserClient("https://torii.example", {
    fetchImpl,
    networkId: FOREIGN_QUERY_NETWORK_ID,
  });
  const auth = { authAccountId: FIXTURE_ALICE_ID, sign, timestampMs: 1_700_000_000_000, nonce: "query-binding" };

  await client.queryAccountTransactions(FIXTURE_ALICE_ID, { ...auth, limit: 1 });
  await client.queryAccountTransactions(FIXTURE_BOB_ID, { ...auth, limit: 1 });
  await client.queryTransactions({ ...auth, limit: 2 });
  await foreign.queryTransactions({ ...auth, limit: 2 });

  assert.notDeepEqual(messages[0], messages[1], "the substituted account path must be signed");
  assert.notDeepEqual(messages[1], messages[2], "the final query body must be signed");
  assert.notDeepEqual(messages[2], messages[3], "a foreign genesis must change the signature message");
});

test("ToriiBrowserClient transaction queries are one-shot and reject legacy auth shapes", async () => {
  let fetchCalls = 0;
  const client = new ToriiBrowserClient("https://torii.example", {
    networkId: QUERY_NETWORK_ID,
    fetchImpl: async (_url, init) => {
      fetchCalls += 1;
      assert.equal(init.redirect, "error");
      return jsonResponse({ error: "unavailable" }, { status: 503 });
    },
  });
  await assert.rejects(
    client.queryTransactions({ ...canonicalReadOptions(), limit: 1 }),
    (error) => error instanceof ToriiBrowserHttpError && error.status === 503,
  );
  assert.equal(fetchCalls, 1);

  const noFetch = new ToriiBrowserClient("https://torii.example", {
    networkId: QUERY_NETWORK_ID,
    fetchImpl: async () => {
      throw new Error("invalid authentication must fail before fetch");
    },
  });
  assert.throws(() => noFetch.queryTransactions({ limit: 1 }), /authAccountId/);
  assert.throws(
    () => noFetch.queryTransactions({ ...canonicalReadOptions(), privateKey: "inline" }),
    /unsupported option privateKey/,
  );
  assert.throws(
    () => noFetch.queryTransactions({
      ...canonicalReadOptions(),
      headers: { "X-Iroha-Signature": "precomputed" },
    }),
    /cannot be precomputed/,
  );
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
      payload: {
        pagination: { limit: 25, next_cursor: null, has_more: false },
        items: [{ id: "asset", value: "01" }],
      },
      invoke: (client) => client.listExplorerAssets(),
    },
    {
      payload: {
        pagination: { limit: 25, next_cursor: null, has_more: false },
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

test("ToriiBrowserClient statically imports only named browser-safe Norito APIs", () => {
  const source = readFileSync(
    new URL("../src/toriiBrowserClient.js", import.meta.url),
    "utf8",
  );
  assert.match(
    source,
    /import\s*\{[^}]*noritoDecodeBlockProofs[^}]*noritoEncodeMultisigProposeRequest[^}]*\}\s*from\s*["']\.\/norito\.js["']/su,
  );
  assert.doesNotMatch(source, /import\s*\(\s*["']\.\/norito\.js["']\s*\)/u);
  assert.doesNotMatch(source, /import\s+\*\s+as\s+\w+\s+from\s+["']\.\/norito\.js["']/u);
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
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl,
    networkId: QUERY_NETWORK_ID,
  });

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
  assert.equal(calls[0].init.redirect, "error");
  assert.deepEqual(JSON.parse(calls[0].init.body), {
    multisig_account_alias: "cbdc@banka",
    status: ["COLLECTING_SIGNATURES"],
    cursor: "page-1",
    limit: 25,
  });
  assert.equal(calls[1].url, "https://torii.example/v1/multisig/proposals/resolve");
  assert.notEqual(calls[1].url, "https://torii.example/v1/multisig/proposals/get");
  assert.equal(calls[1].init.method, "POST");
  assert.equal(calls[1].init.redirect, "error");
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
    contractAddress: "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw",
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

test("ToriiBrowserClient requires equal raw lowercase receipt identities", async () => {
  const signedTransaction = compactHashSignedTransactionFixture();
  const entrypointHash = browserSignedTransactionHashHex(signedTransaction);

  const correctHeaders = {
    "x-iroha-entrypoint-hash": entrypointHash,
    "x-iroha-transaction-hash": entrypointHash,
    "x-iroha-signed-transaction-hash": entrypointHash,
  };
  let acceptedInit;
  const acceptedClient = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async (_url, init) => {
      acceptedInit = init;
      return jsonResponse(
        { accepted: true },
        { status: 202, headers: correctHeaders },
      );
    },
  });
  assert.deepEqual(await acceptedClient.submitTransaction(signedTransaction), {
    accepted: true,
  });
  assert.equal(acceptedInit.redirect, "error");

  const forgedHash = "01".repeat(32);
  for (const [label, override, expectedHeader, message] of [
    [
      "forged entrypoint hash",
      { "x-iroha-entrypoint-hash": forgedHash },
      "x-iroha-entrypoint-hash",
      "does not match",
    ],
    [
      "forged compatibility hash",
      { "x-iroha-transaction-hash": forgedHash },
      "x-iroha-transaction-hash",
      "does not match",
    ],
    [
      "forged signed transaction hash",
      { "x-iroha-signed-transaction-hash": forgedHash },
      "x-iroha-signed-transaction-hash",
      "does not match",
    ],
    [
      "coalesced signed transaction hash",
      { "x-iroha-signed-transaction-hash": `${entrypointHash}, ${entrypointHash}` },
      "x-iroha-signed-transaction-hash",
      "exactly once",
    ],
    [
      "uppercase transaction hash",
      { "x-iroha-transaction-hash": entrypointHash.toUpperCase() },
      "x-iroha-transaction-hash",
      "exactly once",
    ],
  ]) {
    const client = new ToriiBrowserClient("https://torii.example", {
      fetchImpl: async () =>
        jsonResponse(
          { accepted: true },
          { status: 202, headers: { ...correctHeaders, ...override } },
        ),
    });
    await assert.rejects(
      client.submitTransaction(signedTransaction),
      new RegExp(`${expectedHeader}.*${message}`, "u"),
      label,
    );
  }

  for (const missing of Object.keys(correctHeaders)) {
    const headers = { ...correctHeaders };
    delete headers[missing];
    const client = new ToriiBrowserClient("https://torii.example", {
      fetchImpl: async () => jsonResponse({ accepted: true }, { status: 202, headers }),
    });
    await assert.rejects(
      client.submitTransaction(signedTransaction),
      new RegExp(`${missing}.*exactly once`, "u"),
    );
  }
});

for (const redirectStatus of [307, 308]) {
  test(`ToriiBrowserClient rejects transaction ${redirectStatus} without redirecting`, async () => {
    const signedTransaction = compactHashSignedTransactionFixture();
    let attempts = 0;
    const client = new ToriiBrowserClient("https://torii.example", {
      fetchImpl: async (_url, init) => {
        attempts += 1;
        assert.equal(init.redirect, "error");
        return jsonResponse(
          { redirect: true },
          {
            status: redirectStatus,
            headers: { location: "https://redirect.example/replayed" },
          },
        );
      },
    });

    await assert.rejects(
      () => client.submitTransaction(signedTransaction),
      (error) =>
        error instanceof ToriiBrowserHttpError &&
        error.status === redirectStatus,
    );
    assert.equal(attempts, 1);
  });
}

test("ToriiBrowserClient rejects redirects for caller-supplied nonce headers", async () => {
  let attempts = 0;
  const client = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async (_url, init) => {
      attempts += 1;
      assert.equal(init.redirect, "error");
      return jsonResponse(
        { redirect: true },
        {
          status: 307,
          headers: { location: "https://redirect.example/replayed-query" },
        },
      );
    },
  });

  await assert.rejects(
    () =>
      client._json("POST", "/query", {
        body: { query: "signed" },
        headers: { "X-Iroha-Nonce": "caller-generated-nonce" },
      }),
    (error) => error instanceof ToriiBrowserHttpError && error.status === 307,
  );
  assert.equal(attempts, 1);
});

test("ToriiBrowserClient waits for exact global persisted Applied finality", async () => {
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

test("ToriiBrowserClient rejects malformed Applied envelopes", async () => {
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

test("ToriiBrowserClient rejects retired nested status details", async () => {
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

const typedSumeragiBrowserClients = Object.freeze([
  ["source", ToriiBrowserClient],
  ["dist", browserDistSdk.ToriiBrowserClient],
]);

test("ToriiBrowserClient typed Sumeragi methods use fixed JSON routes and preserve u64 tokens", async (t) => {
  for (const [label, Client] of typedSumeragiBrowserClients) {
    await t.test(label, async () => {
      const diagnosticsText = JSON.stringify(
        browserSumeragiDiagnosticsFixture(),
      )
        .replace(
          '"tx_queue_retained_bytes":4096',
          '"tx_queue_retained_bytes":9007199254740993',
        )
        .replace(
          '"tx_queue_max_retained_bytes":65536',
          '"tx_queue_max_retained_bytes":9007199254740994',
        );
      const requests = [];
      const client = new Client("https://torii.example", {
        operatorSigningContext: BROWSER_OPERATOR_CONTEXT,
        fetchImpl: async (url, init) => {
          requests.push([String(url), init]);
          if (String(url).endsWith("/v1/sumeragi/status")) {
            return new Response(JSON.stringify(browserSumeragiStatusFixture()), {
              headers: { "content-type": "application/json; charset=utf-8" },
            });
          }
          return new Response(diagnosticsText, {
            headers: { "content-type": "application/json" },
          });
        },
      });

      const status = await client.getSumeragiStatusTyped();
      const diagnostics = await client.getSumeragiDiagnosticsTyped();
      assert.equal(status.protocol_version, 4);
      assert.equal(status.height, 10);
      assert.equal(diagnostics.tx_queue_retained_bytes, 9007199254740993n);
      assert.equal(
        diagnostics.tx_queue_max_retained_bytes,
        9007199254740994n,
      );
      assert.deepEqual(
        requests.map(([url, init]) => [url, init.method, init.headers.Accept]),
        [
          [
            "https://torii.example/v1/sumeragi/status",
            "GET",
            "application/json",
          ],
          [
            "https://torii.example/v1/sumeragi/diagnostics",
            "GET",
            "application/json",
          ],
        ],
      );
    });
  }
});

test("ToriiBrowserClient typed Sumeragi methods reject ambiguous JSON and non-JSON media", async (t) => {
  for (const [label, Client] of typedSumeragiBrowserClients) {
    await t.test(label, async () => {
      const validStatus = JSON.stringify(browserSumeragiStatusFixture());
      const responses = [
        new Response(`{"protocol_version":4,${validStatus.slice(1)}`, {
          headers: { "content-type": "application/json" },
        }),
        new Response(`${validStatus} trailing`, {
          headers: { "content-type": "application/json" },
        }),
        new Response("", {
          headers: { "content-type": "application/json" },
        }),
        new Response(validStatus, {
          headers: { "content-type": "text/plain" },
        }),
      ];
      const client = new Client("https://torii.example", {
        operatorSigningContext: BROWSER_OPERATOR_CONTEXT,
        fetchImpl: async () => responses.shift(),
      });

      await assert.rejects(
        client.getSumeragiStatusTyped(),
        /duplicate object key/u,
      );
      await assert.rejects(
        client.getSumeragiStatusTyped(),
        /trailing input/u,
      );
      await assert.rejects(
        client.getSumeragiStatusTyped(),
        /unexpected end of input/u,
      );
      await assert.rejects(
        client.getSumeragiStatusTyped(),
        /application\/json media type/u,
      );
      assert.throws(
        () => client.getSumeragiStatusTyped({ headers: {} }),
        /unsupported option headers/u,
      );
    });
  }
});

test("ToriiBrowserClient typed Sumeragi methods enforce endpoint-specific byte bounds", async (t) => {
  for (const [label, Client] of typedSumeragiBrowserClients) {
    await t.test(label, async () => {
      const declaredLengths = [1024 * 1024 + 1, 16 * 1024 * 1024 + 1];
      const client = new Client("https://torii.example", {
        operatorSigningContext: BROWSER_OPERATOR_CONTEXT,
        fetchImpl: async () => new Response("{}", {
          headers: {
            "content-length": String(declaredLengths.shift()),
            "content-type": "application/json",
          },
        }),
      });

      await assert.rejects(
        client.getSumeragiStatusTyped(),
        /1048576-byte response limit/u,
      );
      await assert.rejects(
        client.getSumeragiDiagnosticsTyped(),
        /16777216-byte response limit/u,
      );
    });
  }
});

test("ToriiBrowserClient keeps raw and typed Sumeragi methods distinct", async () => {
  const payload = { operational_note: "raw payload" };
  const client = new ToriiBrowserClient("https://torii.example", {
    operatorSigningContext: BROWSER_OPERATOR_CONTEXT,
    fetchImpl: async () => jsonResponse(payload),
  });

  assert.deepEqual(await client.getSumeragiStatus(), payload);
  assert.deepEqual(await client.getSumeragiDiagnostics(), payload);
  await assert.rejects(client.getSumeragiStatusTyped(), /unknown field/u);
  await assert.rejects(client.getSumeragiDiagnosticsTyped(), /unknown field/u);
});

test("ToriiBrowserClient Sumeragi reads require fresh operator auth before dispatch", async () => {
  let calls = 0;
  const missing = new ToriiBrowserClient("https://torii.example", {
    fetchImpl: async () => {
      calls += 1;
      return jsonResponse({});
    },
  });
  assert.throws(
    () => missing.getSumeragiStatus(),
    /requires an immutable OperatorSigningContext/u,
  );
  assert.equal(calls, 0);

  const signedCalls = [];
  const signed = new ToriiBrowserClient("https://torii.example", {
    operatorSigningContext: BROWSER_OPERATOR_CONTEXT,
    fetchImpl: async (url, init) => {
      signedCalls.push([String(url), init]);
      return jsonResponse({});
    },
  });
  await signed.getSumeragiTelemetry();
  assert.equal(signedCalls.length, 1);
  assert.equal(signedCalls[0][0], "https://torii.example/v1/sumeragi/telemetry");
  assert.equal(signedCalls[0][1].redirect, "error");
  assert.equal(signedCalls[0][1].body, undefined);
  assert.ok(signedCalls[0][1].headers["X-Iroha-Operator-Signature"]);

  const fallback = new ToriiBrowserClient("https://torii.example", {
    operatorSigningContext: BROWSER_OPERATOR_CONTEXT,
    defaultHeaders: { Authorization: "Bearer retired" },
    fetchImpl: async () => {
      calls += 1;
      return jsonResponse({});
    },
  });
  await assert.rejects(
    fallback.getSumeragiStatus(),
    /generated signing/u,
  );
  assert.equal(calls, 0);
});
