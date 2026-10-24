//! Asset movement feed: the collection accessor on both clients and the strict row decoder.
import { test } from "node:test";
import assert from "node:assert/strict";

import * as indexSdk from "../src/index.js";
import * as browserSdk from "../src/browser.js";
import * as toriiSdk from "../src/toriiClient.js";
import { ToriiClient } from "../src/toriiClient.js";
import { ToriiBrowserClient } from "../src/toriiBrowserClient.js";

const {
  ASSET_MOVEMENT_KINDS,
  ASSET_MOVEMENT_ROW_FIELDS,
  ListQueryError,
  ToriiCollection,
  ToriiError,
  decodeAssetMovementRow,
  decodeAssetMovementRows,
  field,
} = indexSdk;

const BASE_URL = "https://torii.example";
const DEFINITION = "62Fk4FPcMuLvW5QjDGNF2a4jAmjM";
const TX_HASH = "ab".repeat(32);
const PAYER = "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
const RESERVE = "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB";

function row(overrides = {}) {
  return {
    id: `${TX_HASH}:1`,
    kind: "OFFLINE_LOAD",
    timestamp_ms: 1_761_300_000_000,
    from_account_id: PAYER,
    to_account_id: RESERVE,
    asset_definition_id: DEFINITION,
    amount: "1032.25",
    tx_hash: TX_HASH,
    authority: PAYER,
    block_height: 2_431,
    block_index: 0,
    movement_index: 1,
    result_ok: true,
    ...overrides,
  };
}

function assertInvalidResponse(message) {
  return (error) => {
    assert(error instanceof ToriiError, `${error} is a ToriiError`);
    assert.equal(error.code, "invalid_response");
    assert.match(error.message, message);
    return true;
  };
}

function jsonResponse(body) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

test("every entry point exports the asset movement decoder", () => {
  for (const sdk of [indexSdk, browserSdk, toriiSdk]) {
    assert.deepEqual([...sdk.ASSET_MOVEMENT_KINDS], [
      "TRANSFER",
      "MINT",
      "BURN",
      "OFFLINE_LOAD",
      "OFFLINE_UNLOAD",
      "OFFLINE_FEE",
    ]);
    assert.equal(sdk.ASSET_MOVEMENT_ROW_FIELDS.length, 13);
    for (const name of ["decodeAssetMovementRow", "decodeAssetMovementRows"]) {
      assert.equal(typeof sdk[name], "function", name);
    }
  }
  assert.equal(indexSdk.Torii.decodeAssetMovementRows, decodeAssetMovementRows);
});

test("decodeAssetMovementRow returns exactly the contract fields, frozen", () => {
  const decoded = decodeAssetMovementRow({ ...row(), added_later: 1 });
  assert.deepEqual(Object.keys(decoded), [...ASSET_MOVEMENT_ROW_FIELDS]);
  assert.deepEqual(decoded, row());
  assert(Object.isFrozen(decoded));
  assert.deepEqual(decodeAssetMovementRow(decoded), decoded, "decoding is idempotent");
  const mint = decodeAssetMovementRow(row({ kind: "MINT", from_account_id: null, amount: "0" }));
  assert.equal(mint.from_account_id, null);
  const burn = decodeAssetMovementRow(row({ kind: "BURN", to_account_id: null, result_ok: false }));
  assert.equal(burn.to_account_id, null);
  assert.equal(burn.result_ok, false);
  assert.deepEqual(decodeAssetMovementRows([row(), row({ kind: "TRANSFER" })]).map((item) => item.kind), [
    "OFFLINE_LOAD",
    "TRANSFER",
  ]);
});

test("decodeAssetMovementRow rejects every malformed field", () => {
  const cases = [
    [row({ kind: "SWAP" }), /kind must be one of/u],
    [row({ kind: undefined }), /kind must be one of/u],
    [row({ from_account_id: null }), /from_account_id may be null only for MINT/u],
    [row({ kind: "MINT" }), /from_account_id must be null for MINT/u],
    [row({ to_account_id: null }), /to_account_id may be null only for BURN/u],
    [row({ kind: "BURN" }), /to_account_id must be null for BURN/u],
    [row({ from_account_id: "" }), /from_account_id must be a non-empty string/u],
    [row({ amount: undefined }), /amount must be canonical/u],
    [row({ amount: 5 }), /amount must be canonical/u],
    [row({ amount: "1032.250" }), /amount must be canonical/u],
    [row({ amount: "01" }), /amount must be canonical/u],
    [row({ amount: "-1" }), /amount must be canonical/u],
    [row({ amount: "1e3" }), /amount must be canonical/u],
    [row({ amount: ".5" }), /amount must be canonical/u],
    [row({ tx_hash: "AB".repeat(32) }), /tx_hash must be 64 lowercase hex/u],
    [row({ block_height: 0 }), /block_height must be an integer of at least 1/u],
    [row({ block_height: "2431" }), /block_height must be an integer/u],
    [row({ block_index: 1.5 }), /block_index must be an integer/u],
    [row({ movement_index: -1 }), /movement_index must be an integer/u],
    [row({ timestamp_ms: 2 ** 53 }), /timestamp_ms must be an integer/u],
    [row({ id: `${TX_HASH}:0` }), /id must be/u],
    [row({ result_ok: "true" }), /result_ok must be a boolean/u],
    [row({ result_ok: false }), /OFFLINE_LOAD rows come only from successful transactions/u],
    [row({ asset_definition_id: null }), /asset_definition_id must be a non-empty string/u],
    [row({ authority: undefined }), /authority must be a non-empty string/u],
  ];
  for (const [input, message] of cases) {
    assert.throws(() => decodeAssetMovementRow(input), assertInvalidResponse(message), message.source);
  }
  assert.throws(() => decodeAssetMovementRow(null), assertInvalidResponse(/must be an object/u));
  assert.throws(() => decodeAssetMovementRows({}), assertInvalidResponse(/items must be an array/u));
  assert.throws(
    () => decodeAssetMovementRows([row(), row({ kind: "SWAP" })]),
    assertInvalidResponse(/items\[1\]\.kind/u),
  );
});

for (const [name, makeClient] of [
  ["ToriiClient", (fetchImpl) => new ToriiClient(BASE_URL, { fetchImpl })],
  ["ToriiBrowserClient", (fetchImpl) => new ToriiBrowserClient(BASE_URL, { fetchImpl })],
]) {
  test(`${name}: assetMovements is a history collection with decoded rows`, async () => {
    const calls = [];
    const pages = [
      { items: [row()], next_cursor: "q1_older" },
      { items: [{ amount: "1" }], next_cursor: null },
      { items: [row({ kind: "SWAP" })], next_cursor: null },
    ];
    const client = makeClient(async (url, init = {}) => {
      calls.push({ url: String(url), body: String(init.body) });
      return jsonResponse(pages[calls.length - 1]);
    });
    const movements = client.assetMovements(DEFINITION);
    assert(movements instanceof ToriiCollection);
    assert.equal(movements.path, `/v1/assets/${DEFINITION}/movements`);
    assert.equal(movements.history, true);
    const first = await movements.list({ filter: field("block_height").gt(2_400), limit: 500 });
    assert.deepEqual(first.items, [row()]);
    assert(Object.isFrozen(first.items[0]));
    assert.equal(first.nextCursor, "q1_older");
    assert.equal(calls[0].url, `${BASE_URL}/v1/assets/${DEFINITION}/movements/query`);
    assert.equal(calls[0].body, '{"filter":{"op":"gt","args":["block_height",2400]},"limit":500}');
    const selected = await movements.list({ select: ["amount"] });
    assert.deepEqual(selected.items, [{ amount: "1" }], "selected rows are returned as sent");
    await assert.rejects(movements.list(), assertInvalidResponse(/kind must be one of/u));
    for (const [query, code] of [
      [{ sort: "-block_height" }, "invalid_sort"],
      [{ includeTotal: true }, "invalid_include_total"],
    ]) {
      await assert.rejects(movements.list(query), (error) => error instanceof ListQueryError && error.code === code);
    }
    assert.throws(() => client.assetMovements(""), TypeError);
  });
}
