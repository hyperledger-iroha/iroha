import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  SORAFS_ORDERBOOK_PAYLOAD_KINDS,
  buildSignedOrderbookOrderCancel,
  buildSignedOrderbookOrderRequest,
  buildSignedOrderbookSettlementReceipt,
  signOrderbookPayload,
  validateOrderbookPayload,
} from "../src/sorafs.js";

const ORDER_REQUEST_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/orderbook/order_request_v1.to",
  import.meta.url,
);
const ORDER_CANCEL_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/orderbook/order_cancel_v1.to",
  import.meta.url,
);
const SETTLEMENT_RECEIPT_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/orderbook/settlement_receipt_v1.to",
  import.meta.url,
);
const RUNTIME_SNAPSHOT_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/orderbook/runtime_snapshot_v1.to",
  import.meta.url,
);
const ORDERBOOK_PRIVATE_KEY = Buffer.alloc(32, 0xb7);
const ORDERBOOK_OWNER_ACCOUNT = Buffer.from("merchant@paynet", "utf8");

function fixed32(byte) {
  return Buffer.alloc(32, byte);
}

test("validateOrderbookPayload accepts canonical order request fixture", () => {
  const outcome = validateOrderbookPayload(
    "order",
    readFileSync(ORDER_REQUEST_FIXTURE),
    {
      label: "fixtures/sorafs_manifest/orderbook/order_request_v1.to",
      generatedAtUnix: 1_700_000_123,
    },
  );

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-OK-000");
  assert.equal(outcome.category, "validation");
  assert.equal(outcome.generated_at, 1_700_000_123);
  assert.equal(outcome.inputs[0]?.kind, "orderbook_order_request");
  assert.equal(
    outcome.inputs[0]?.path,
    "fixtures/sorafs_manifest/orderbook/order_request_v1.to",
  );
});

test("validateOrderbookPayload accepts runtime snapshot fixture", () => {
  const outcome = validateOrderbookPayload(
    SORAFS_ORDERBOOK_PAYLOAD_KINDS.RUNTIME_SNAPSHOT,
    readFileSync(RUNTIME_SNAPSHOT_FIXTURE),
    { generated_at: 1_700_000_456 },
  );

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-OK-000");
  assert.equal(outcome.inputs[0]?.kind, "orderbook_runtime_snapshot");
  assert.equal(outcome.generated_at, 1_700_000_456);
});

test("validateOrderbookPayload reports malformed payloads as reference outcomes", () => {
  const outcome = validateOrderbookPayload("settlement-receipt", Buffer.alloc(8), {
    generatedAtUnix: 1_700_000_789,
  });

  assert.equal(outcome.status, "Error");
  assert.match(outcome.code, /^SFS-/);
  assert.equal(outcome.category, "norito");
  assert.equal(outcome.inputs[0]?.kind, "settlement_receipt");
});

test("validateOrderbookPayload rejects unknown kinds before native validation", () => {
  assert.throws(
    () => validateOrderbookPayload("bad-kind", Buffer.alloc(8)),
    /unsupported SoraFS orderbook payload kind/i,
  );
});

test("validateOrderbookPayload rejects unsafe generated timestamps", () => {
  assert.throws(
    () =>
      validateOrderbookPayload("order-request", Buffer.alloc(8), {
        generatedAtUnix: Number.MAX_SAFE_INTEGER + 1,
      }),
    /safe integer/i,
  );
});

test("signOrderbookPayload signs mutable orderbook fixture payloads", () => {
  const cases = [
    ["order", ORDER_REQUEST_FIXTURE, "orderbook_order_request"],
    ["order-cancel", ORDER_CANCEL_FIXTURE, "orderbook_order_cancel"],
    ["settlement-receipt", SETTLEMENT_RECEIPT_FIXTURE, "settlement_receipt"],
  ];

  for (const [kind, fixture, inputKind] of cases) {
    const unsigned = readFileSync(fixture);
    const signed = signOrderbookPayload(kind, unsigned, ORDERBOOK_PRIVATE_KEY);
    assert.ok(Buffer.isBuffer(signed));
    assert.notDeepStrictEqual(signed, unsigned);

    const outcome = validateOrderbookPayload(kind, signed, {
      generatedAtUnix: 1_700_000_999,
    });
    assert.equal(outcome.status, "Ok");
    assert.equal(outcome.inputs[0]?.kind, inputKind);
  }
});

test("signOrderbookPayload rejects non-signable orderbook payload kinds", () => {
  assert.throws(
    () =>
      signOrderbookPayload(
        "runtime-snapshot",
        readFileSync(RUNTIME_SNAPSHOT_FIXTURE),
        ORDERBOOK_PRIVATE_KEY,
      ),
    /cannot be signed/i,
  );
});

test("signOrderbookPayload rejects malformed private keys", () => {
  assert.throws(
    () =>
      signOrderbookPayload(
        "order-request",
        readFileSync(ORDER_REQUEST_FIXTURE),
        Buffer.alloc(31, 0xb7),
      ),
    /32 bytes/i,
  );
});

test("field-level orderbook builders emit valid signed payloads", () => {
  const order = buildSignedOrderbookOrderRequest(
    {
      orderId: fixed32(0x11),
      side: "bid",
      tier: "hot",
      pricePerGibMicroXor: "1000000",
      quantityGib: "12",
      ownerAccount: ORDERBOOK_OWNER_ACCOUNT,
      expiryUnix: "1700010000",
      nonce: "7",
      makerFeeBps: "25",
      takerFeeBps: "30",
    },
    ORDERBOOK_PRIVATE_KEY,
  );
  assert.equal(
    validateOrderbookPayload("order-request", order, {
      generatedAtUnix: 1_700_000_999,
    }).status,
    "Ok",
  );

  const cancel = buildSignedOrderbookOrderCancel(
    {
      order_id: fixed32(0x11),
      owner_account: ORDERBOOK_OWNER_ACCOUNT,
      reason: "owner_requested",
      nonce: 8n,
    },
    ORDERBOOK_PRIVATE_KEY,
  );
  assert.equal(
    validateOrderbookPayload("order-cancel", cancel, {
      generatedAtUnix: 1_700_000_999,
    }).status,
    "Ok",
  );

  const receipt = buildSignedOrderbookSettlementReceipt(
    {
      receiptId: fixed32(0x21),
      channelId: fixed32(0x22),
      tradeId: fixed32(0x23),
      rangeStart: "0",
      rangeEnd: "4096",
      chunkHash: fixed32(0x24),
      bytesDelivered: "4096",
      xorDebitedMicroXor: "100",
      providerCreditMicroXor: "90",
      feeAmountMicroXor: "10",
      issuedAtUnix: "1700000999",
    },
    ORDERBOOK_PRIVATE_KEY,
  );
  assert.equal(
    validateOrderbookPayload("settlement-receipt", receipt, {
      generatedAtUnix: 1_700_000_999,
    }).status,
    "Ok",
  );
});

test("field-level settlement receipt builder rejects imbalanced amounts", () => {
  assert.throws(
    () =>
      buildSignedOrderbookSettlementReceipt(
        {
          receiptId: fixed32(0x31),
          channelId: fixed32(0x32),
          tradeId: fixed32(0x33),
          rangeStart: "0",
          rangeEnd: "4096",
          chunkHash: fixed32(0x34),
          bytesDelivered: "4096",
          xorDebitedMicroXor: "100",
          providerCreditMicroXor: "91",
          feeAmountMicroXor: "10",
          issuedAtUnix: "1700000999",
        },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /settlement imbalance/i,
  );
});
