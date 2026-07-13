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

function signatureFixture() {
  return {
    algorithm: "Ed25519",
    public_key_hex: "aa".repeat(32),
    signature_hex: "bb".repeat(64),
  };
}

function orderbookFixture() {
  const orderId = "11".repeat(32);
  const tradeId = "22".repeat(32);
  const channelId = "33".repeat(32);
  const receiptId = "44".repeat(32);
  const providerId = "55".repeat(32);
  const order = {
    version: 1,
    order_id_hex: orderId,
    side: "bid",
    tier: "hot",
    price_per_gib: WIDE_XOR,
    quantity_gib: 2,
    remaining_gib: 1,
    owner_account_hex: "cafe",
    expiry_unix: 1_800_000_000,
    nonce: 7,
    maker_fee_bps: 25,
    taker_fee_bps: 35,
    signature: signatureFixture(),
  };
  const trade = {
    version: 1,
    trade_id_hex: tradeId,
    maker_order_id_hex: orderId,
    taker_order_id_hex: "66".repeat(32),
    tier: "hot",
    price_per_gib: WIDE_XOR,
    filled_gib: 1,
    maker_fee: "0.000000001",
    taker_fee: "1.000000001",
    timestamp_unix: 1_700_000_100,
  };
  const channel = {
    version: 1,
    channel_id_hex: channelId,
    trade_id_hex: tradeId,
    buyer_account_hex: "face",
    provider_id_hex: providerId,
    total_bytes: 1024,
    remaining_bytes: 512,
    xor_locked: WIDE_XOR,
    status: "open",
    opened_at_unix: 1_700_000_101,
    updated_at_unix: 1_700_000_102,
  };
  const receipt = {
    version: 1,
    receipt_id_hex: receiptId,
    channel_id_hex: channelId,
    trade_id_hex: tradeId,
    range: { start: 0, end: 512 },
    chunk_hash_hex: "77".repeat(32),
    bytes_delivered: 512,
    xor_debited: WIDE_XOR,
    provider_credit: "1.000000001",
    fee_amount: "0.000000001",
    issued_at_unix: 1_700_000_103,
    settlement_signature: signatureFixture(),
  };
  return {
    schema: "sorafs.orderbook.local.v1",
    source: "local",
    generated_at_unix: 1_700_000_000,
    next_sequence: 2,
    open_order_count: 1,
    trade_count: 1,
    settlement_channel_count: 1,
    settlement_receipt_count: 1,
    depth: {
      hot_bid_gib: 1,
      hot_ask_gib: 0,
      warm_bid_gib: 0,
      warm_ask_gib: 0,
      archive_bid_gib: 0,
      archive_ask_gib: 0,
    },
    open_orders: [{ sequence: 1, order }],
    trades: [trade],
    settlement_channels: [channel],
    settlement_receipts: [receipt],
    expired_order_ids_hex: [],
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

test("orderbook responses preserve exact quantities and reject adversarial spellings", async () => {
  const valid = orderbookFixture();
  const client = new ToriiClient(BASE_URL, {
    fetchImpl: async () => jsonResponse(valid),
  });
  const normalized = await client.getSorafsOrderbook();
  assert.equal(normalized.open_orders[0].order.price_per_gib, WIDE_XOR);
  assert.equal(normalized.trades[0].maker_fee, "0.000000001");
  assert.equal(normalized.settlement_channels[0].xor_locked, WIDE_XOR);
  assert.equal(normalized.settlement_receipts[0].fee_amount, "0.000000001");

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
    const payload = orderbookFixture();
    payload.open_orders[0].order.price_per_gib = value;
    const rejectingClient = new ToriiClient(BASE_URL, {
      fetchImpl: async () => jsonResponse(payload),
    });
    await assert.rejects(
      () => rejectingClient.getSorafsOrderbook(),
      /price_per_gib|canonical|fractional/i,
      `price_per_gib=${String(value)}`,
    );
  }

  const missing = orderbookFixture();
  delete missing.open_orders[0].order.price_per_gib;
  await assert.rejects(
    () =>
      new ToriiClient(BASE_URL, {
        fetchImpl: async () => jsonResponse(missing),
      }).getSorafsOrderbook(),
    /price_per_gib/i,
  );

  for (const mutate of [
    (payload) => {
      payload.open_orders[0].order.price_per_gib_micro_xor = "1";
    },
    (payload) => {
      payload.trades[0].maker_fee_micro_xor = "1";
    },
    (payload) => {
      payload.settlement_channels[0].xor_locked_micro = "1";
    },
    (payload) => {
      payload.settlement_receipts[0].provider_credit_micro = "1";
    },
  ]) {
    const payload = orderbookFixture();
    mutate(payload);
    await assert.rejects(
      () =>
        new ToriiClient(BASE_URL, {
          fetchImpl: async () => jsonResponse(payload),
        }).getSorafsOrderbook(),
      /retired monetary field/i,
    );
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
