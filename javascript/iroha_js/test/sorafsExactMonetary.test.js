import { test } from "node:test";
import assert from "node:assert/strict";

import { ToriiClient } from "../src/toriiClient.js";
import {
  buildSignedOrderbookOrderRequest,
  validateOrderbookPayload,
} from "../src/sorafs.js";
import { makeNativeTest } from "./helpers/native.js";

const BASE_URL = "https://localhost:8080";
const PRIVATE_KEY = Buffer.alloc(32, 0xb7);
const OWNER_ACCOUNT = Buffer.from("merchant@paynet", "utf8");
const SIGNED_512_MAX = (1n << 511n) - 1n;
const SIGNED_512_OVERFLOW = (1n << 511n).toString();
const WIDE_XOR = "340282366920938463463374607431768211456.000000001";
const nativeTest = makeNativeTest(test);

function jsonResponse(payload, status = 200) {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function orderRequestFields(pricePerGib) {
  return {
    side: "bid",
    tier: "hot",
    ...(pricePerGib === undefined ? {} : { pricePerGib }),
    quantityGib: "12",
    ownerAccount: OWNER_ACCOUNT,
    expiryUnix: "1700010000",
    nonce: "7",
    makerFeeBps: "25",
    takerFeeBps: "30",
  };
}

function daReceiptFixture(rentQuote) {
  const digest = Array.from({ length: 32 }, (_, index) => index);
  return {
    client_blob_id: [digest],
    lane_id: 1,
    epoch: 2,
    blob_hash: [digest],
    chunk_root: [digest],
    manifest_hash: [digest],
    storage_ticket: [digest],
    pdp_commitment: null,
    queued_at_unix: 1234,
    operator_signature: "aa".repeat(64),
    rent_quote: rentQuote,
  };
}

async function submitDaFixture(rentQuote) {
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () =>
      jsonResponse(
        {
          status: "accepted",
          duplicate: false,
          receipt: daReceiptFixture(rentQuote),
        },
        202,
      ),
  });
  return client.submitDaBlob({
    payload: Buffer.from("car-bytes"),
    codec: "nexus_lane_sidecar",
    laneId: 11,
    epoch: 22,
    sequence: 33,
    submitterPublicKey:
      "ed0120EDF6D7B52C7032D03AEC696F2068BD53101528F3C7B6081BFF05A1662D7FC245",
    signatureHex: "aa".repeat(64),
    clientBlobId: Buffer.alloc(32, 0x11),
  });
}

test("signed orderbook builders reject noncanonical XOR quantities before native work", () => {
  const invalid = [
    [undefined, /pricePerGib is required/i],
    [1, /canonical XOR quantity string/i],
    [1n, /canonical XOR quantity string/i],
    [true, /canonical XOR quantity string/i],
    ["", /canonical/i],
    [" 1", /canonical/i],
    ["1 ", /canonical/i],
    ["+1", /canonical/i],
    ["-1", /canonical/i],
    ["01", /canonical/i],
    ["1.0", /canonical/i],
    ["1e3", /canonical/i],
    ["0", /greater than zero/i],
    ["0.0000000001", /9 fractional/i],
    [SIGNED_512_OVERFLOW, /canonical/i],
  ];
  for (const [value, pattern] of invalid) {
    assert.throws(
      () => buildSignedOrderbookOrderRequest(orderRequestFields(value), PRIVATE_KEY),
      pattern,
      `pricePerGib=${String(value)}`,
    );
  }

  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        { ...orderRequestFields("1"), price_per_gib: "1" },
        PRIVATE_KEY,
      ),
    /exactly once/i,
  );
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        { ...orderRequestFields("1"), pricePerGibMicroXor: "1000000" },
        PRIVATE_KEY,
      ),
    /retired/i,
  );
});

nativeTest("signed orderbook builders accept exact submicro and signed-512 values", () => {
  for (const value of [
    "0.000000001",
    WIDE_XOR,
    SIGNED_512_MAX.toString(),
  ]) {
    const payload = buildSignedOrderbookOrderRequest(orderRequestFields(value), PRIVATE_KEY);
    assert.equal(validateOrderbookPayload("order-request", payload).status, "Ok");
  }
});

test("DA rent quotes expose exact unit-free fields without lossy coercion", async () => {
  const quote = {
    base_rent: WIDE_XOR,
    protocol_reserve: "1.000000001",
    provider_reward: "340282366920938463463374607431768211456",
    pdp_bonus: "0.000000001",
    potr_bonus: "0",
    egress_credit_per_gib: SIGNED_512_MAX.toString(),
  };
  const result = await submitDaFixture(quote);
  assert.deepEqual(result.receipt.rent_quote, quote);
  assert.equal("base_rent_micro" in result.receipt.rent_quote, false);

  const invalid = [
    1,
    true,
    "",
    " 1",
    "1 ",
    "+1",
    "-1",
    "01",
    "1.0",
    "1e3",
    "0.0000000001",
    SIGNED_512_OVERFLOW,
  ];
  for (const value of invalid) {
    await assert.rejects(
      () => submitDaFixture({ ...quote, base_rent: value }),
      /base_rent|canonical|fractional/i,
      `base_rent=${String(value)}`,
    );
  }

  const missing = { ...quote };
  delete missing.base_rent;
  await assert.rejects(() => submitDaFixture(missing), /base_rent/i);
  await assert.rejects(
    () => submitDaFixture({ ...quote, base_rent_micro: "1000000" }),
    /retired monetary field/i,
  );
});
