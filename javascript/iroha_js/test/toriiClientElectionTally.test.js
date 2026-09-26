import assert from "node:assert/strict";
import test from "node:test";

import { LocalSigningContext, ToriiClient } from "../src/toriiClient.js";
import { NetworkId } from "../src/networkId.js";

const BASE_URL = "https://localhost:8080";
const NETWORK_ID = NetworkId.parse(
  "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0",
);
const AUTH = {
  canonicalAuth: { accountId: "alice@wonderland", privateKey: Buffer.alloc(32, 0x33) },
};
const HASH = "12".repeat(32);
const VALID_JSON = `{"evaluated_block_height":9007199254740993,"evaluated_block_hash":"${HASH}","finalized":true,"tally":[9007199254740993,18446744073709551616]}`;

function clientWithResponse(response, capture = () => {}) {
  return new ToriiClient(BASE_URL, {
    localSigningContext: new LocalSigningContext(NETWORK_ID, 753),
    fetchImpl: async (url, options) => {
      capture(url, options);
      return response;
    },
  });
}

test("getElectionTally signs the exact POST and preserves u128 numeric response tokens", async () => {
  let request;
  const controller = new AbortController();
  const client = clientWithResponse(new Response(VALID_JSON, {
    status: 200, headers: { "content-type": "application/json" },
  }), (url, options) => { request = { url, options }; });
  const tally = await client.getElectionTally("election-1", { ...AUTH, signal: controller.signal });
  assert.equal(request.url, `${BASE_URL}/v1/zk/vote/tally`);
  assert.equal(request.options.method, "POST");
  assert.ok(request.options.signal instanceof AbortSignal);
  assert.equal(request.options.signal.aborted, false);
  assert.deepEqual(JSON.parse(request.options.body), { election_id: "election-1" });
  const headers = new Headers(request.options.headers);
  assert.equal(headers.get("Content-Type"), "application/json");
  assert.equal(headers.get("Accept"), "application/json");
  assert.equal(headers.get("X-Iroha-Account"), "alice@wonderland");
  assert.ok(headers.get("X-Iroha-Signature"));
  assert.ok(headers.get("X-Iroha-Nonce"));
  assert.equal(tally.evaluated_block_height, 9007199254740993n);
  assert.deepEqual(tally.tally, [9007199254740993n, 18446744073709551616n]);
  assert.equal(tally.finalized, true);
});

test("getElectionTally rejects invalid selector and missing auth before dispatch", async () => {
  let dispatched = false;
  const client = clientWithResponse(new Response(null, { status: 404 }), () => {
    dispatched = true;
  });
  await assert.rejects(() => client.getElectionTally(".alias", AUTH), /electionId/u);
  await assert.rejects(() => client.getElectionTally("election-1"), /canonicalAuth is required/u);
  assert.equal(dispatched, false);
  assert.equal(await client.getElectionTally("election-1", AUTH), null);
  assert.equal(dispatched, true);
});

for (const [name, body] of [
  ["duplicate tally field", `{"evaluated_block_height":1,"evaluated_block_hash":"${HASH}","finalized":true,"tally":[0,0],"tally":[1,0]}`],
  ["u128 aggregate overflow", `{"evaluated_block_height":1,"evaluated_block_hash":"${HASH}","finalized":true,"tally":[${(1n << 128n) - 1n},1]}`],
  ["fractional weight", `{"evaluated_block_height":1,"evaluated_block_hash":"${HASH}","finalized":true,"tally":[1.5,0]}`],
  ["missing finalized flag", `{"evaluated_block_height":1,"evaluated_block_hash":"${HASH}","tally":[0,0]}`],
]) {
  test(`getElectionTally rejects ${name}`, async () => {
    const client = clientWithResponse(new Response(body, {
      status: 200, headers: { "content-type": "application/json" },
    }));
    await assert.rejects(() => client.getElectionTally("election-1", AUTH));
  });
}

for (const [name, body, contentType] of [
  ["non-JSON media type", "{}", "text/plain"],
  ["body beyond the V1 response cap", " ".repeat(8193), "application/json"],
]) {
  test(`getElectionTally rejects ${name}`, async () => {
    const client = clientWithResponse(new Response(body, {
      status: 200, headers: { "content-type": contentType },
    }));
    await assert.rejects(() => client.getElectionTally("election-1", AUTH));
  });
}
