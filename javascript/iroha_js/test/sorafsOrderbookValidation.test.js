import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1,
  SORAFS_ORDERBOOK_PAYLOAD_KINDS,
  buildSignedOrderbookOrderCancel,
  buildSignedOrderbookOrderRequest,
  buildSignedOrderbookSettlementReceipt,
  deriveOrderbookOrderId,
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
const TRADE_EVENT_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/orderbook/trade_event_v1.to",
  import.meta.url,
);
const ORDER_REQUEST_OUTCOME_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/orderbook/order_request_validation_outcome_v1.json",
  import.meta.url,
);
const ORDER_REQUEST_BAD_SIGNATURE_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/orderbook/negative/order_request_bad_signature_v1.to",
  import.meta.url,
);
const ORDER_REQUEST_BAD_SIGNATURE_OUTCOME_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/orderbook/negative/order_request_bad_signature_validation_outcome_v1.json",
  import.meta.url,
);
const ORDER_REQUEST_TRAILING_BYTES_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/orderbook/negative/order_request_trailing_bytes_v1.to",
  import.meta.url,
);
const ORDER_REQUEST_TRAILING_BYTES_OUTCOME_FIXTURE = new URL(
  "../../../fixtures/sorafs_manifest/orderbook/negative/order_request_trailing_bytes_validation_outcome_v1.json",
  import.meta.url,
);
const ORDERBOOK_PRIVATE_KEY = Buffer.alloc(32, 0xb7);
const ORDERBOOK_OWNER_ACCOUNT = Buffer.from("merchant@paynet", "utf8");
const MAX_SCALED_XOR =
  "6703903964971298549787012499102923063739682910296196688861780721860882015036773488400937149083451713845015929093243025426876941405973284973216824.503042047";

function fixed32(byte) {
  return Buffer.alloc(32, byte);
}

function canonicalOutcomeJson(outcome) {
  return `${JSON.stringify(outcome, null, 2)}\n`;
}

test("validateOrderbookPayload accepts canonical order request fixture", () => {
  const outcome = validateOrderbookPayload(
    SORAFS_ORDERBOOK_PAYLOAD_KINDS.ORDER_REQUEST,
    readFileSync(ORDER_REQUEST_FIXTURE),
    {
      label: "order_request_v1.to",
      generatedAtUnix: 123,
    },
  );

  assert.equal(
    canonicalOutcomeJson(outcome),
    readFileSync(ORDER_REQUEST_OUTCOME_FIXTURE, "utf8"),
  );
});

test("validateOrderbookPayload matches signature and noncanonical outcome fixtures", () => {
  for (const [payload, label, expected] of [
    [
      ORDER_REQUEST_BAD_SIGNATURE_FIXTURE,
      "order_request_bad_signature_v1.to",
      ORDER_REQUEST_BAD_SIGNATURE_OUTCOME_FIXTURE,
    ],
    [
      ORDER_REQUEST_TRAILING_BYTES_FIXTURE,
      "order_request_trailing_bytes_v1.to",
      ORDER_REQUEST_TRAILING_BYTES_OUTCOME_FIXTURE,
    ],
  ]) {
    const outcome = validateOrderbookPayload(
      SORAFS_ORDERBOOK_PAYLOAD_KINDS.ORDER_REQUEST,
      readFileSync(payload),
      { label, generatedAtUnix: 123 },
    );
    assert.equal(canonicalOutcomeJson(outcome), readFileSync(expected, "utf8"));
  }
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

test("validateOrderbookPayload rejects unknown and retired kind aliases", () => {
  for (const kind of [
    "bad-kind",
    "order",
    "request",
    "order_request",
    "orderbook-order-request",
    "ORDER-REQUEST",
    " order-request ",
    "runtime-snapshot",
  ]) {
    assert.throws(
      () => validateOrderbookPayload(kind, Buffer.alloc(8)),
      /unsupported SoraFS orderbook payload kind/i,
      kind,
    );
  }
});

test("validateOrderbookPayload rejects unsafe generated timestamps", () => {
  assert.throws(
    () =>
      validateOrderbookPayload("order-request", Buffer.alloc(8), {
        generatedAtUnix: Number.MAX_SAFE_INTEGER + 1,
      }),
    /safe integer/i,
  );
  assert.throws(
    () =>
      validateOrderbookPayload("order-request", Buffer.alloc(8), {
        generated_at: 1,
      }),
    /unsupported fields/i,
  );
});

test("signOrderbookPayload deterministically reproduces signed fixtures", () => {
  const cases = [
    ["order-request", ORDER_REQUEST_FIXTURE, "orderbook_order_request"],
    ["order-cancel", ORDER_CANCEL_FIXTURE, "orderbook_order_cancel"],
    ["settlement-receipt", SETTLEMENT_RECEIPT_FIXTURE, "settlement_receipt"],
  ];

  for (const [kind, fixture, inputKind] of cases) {
    const unsigned = readFileSync(fixture);
    const signed = signOrderbookPayload(kind, unsigned, ORDERBOOK_PRIVATE_KEY);
    assert.ok(Buffer.isBuffer(signed));
    assert.deepStrictEqual(signed, unsigned);

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
        "trade-event",
        readFileSync(TRADE_EVENT_FIXTURE),
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
  const orderId = deriveOrderbookOrderId(ORDERBOOK_OWNER_ACCOUNT, "7");
  assert.equal(orderId.length, 32);
  const order = buildSignedOrderbookOrderRequest(
    {
      side: "bid",
      tier: "hot",
      pricePerGib: MAX_SCALED_XOR,
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

  const ask = buildSignedOrderbookOrderRequest(
    {
      side: "ask",
      tier: "hot",
      pricePerGib: "1.25",
      quantityGib: "4",
      ownerAccount: ORDERBOOK_OWNER_ACCOUNT,
      providerId: fixed32(0x72),
      expiryUnix: "1700010000",
      nonce: "8",
      makerFeeBps: "25",
      takerFeeBps: "30",
    },
    ORDERBOOK_PRIVATE_KEY,
  );
  assert.notDeepEqual(ask, order);
  assert.equal(
    validateOrderbookPayload("order-request", ask, {
      generatedAtUnix: 1_700_000_999,
    }).status,
    "Ok",
  );
  const askOtherProvider = buildSignedOrderbookOrderRequest(
    {
      side: "ask",
      tier: "hot",
      pricePerGib: "1.25",
      quantityGib: "4",
      ownerAccount: ORDERBOOK_OWNER_ACCOUNT,
      providerId: fixed32(0x73),
      expiryUnix: "1700010000",
      nonce: "8",
      makerFeeBps: "25",
      takerFeeBps: "30",
    },
    ORDERBOOK_PRIVATE_KEY,
  );
  assert.notDeepEqual(askOtherProvider, ask);

  const cancel = buildSignedOrderbookOrderCancel(
    {
      orderId,
      ownerAccount: ORDERBOOK_OWNER_ACCOUNT,
      reason: "owner-requested",
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
      xorDebited: "340282366920938463463374607431768211456.000000001",
      providerCredit: "340282366920938463463374607431768211456",
      feeAmount: "0.000000001",
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

test("order id derivation matches the cross-SDK golden vector", () => {
  assert.equal(
    deriveOrderbookOrderId(Buffer.from("buyer@sora", "utf8"), 7n).toString("hex"),
    "9d91ad7700ca0c4762e031f9231aa38dd4502c6048c6ffa31d365e3c4e080b69",
  );
  assert.throws(() => deriveOrderbookOrderId(Buffer.alloc(0), 7), /must not be empty/i);
  assert.throws(() => deriveOrderbookOrderId(Buffer.from("buyer@sora"), 0), /greater than zero/i);
});

test("orderbook builders accept owner accounts at the V1 byte ceiling", () => {
  const ownerAccount = Buffer.alloc(ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1, 0x45);
  const orderId = deriveOrderbookOrderId(ownerAccount, 9);
  const order = buildSignedOrderbookOrderRequest(
    {
      side: "bid",
      tier: "hot",
      pricePerGib: "1",
      quantityGib: "1",
      ownerAccount,
      expiryUnix: "1700010000",
      nonce: "9",
      makerFeeBps: 0,
      takerFeeBps: 0,
    },
    ORDERBOOK_PRIVATE_KEY,
  );
  assert.equal(
    validateOrderbookPayload("order-request", order, { generatedAtUnix: 1 }).status,
    "Ok",
  );

  const cancel = buildSignedOrderbookOrderCancel(
    {
      orderId,
      ownerAccount,
      reason: "owner-requested",
      nonce: 10,
    },
    ORDERBOOK_PRIVATE_KEY,
  );
  assert.equal(
    validateOrderbookPayload("order-cancel", cancel, { generatedAtUnix: 1 }).status,
    "Ok",
  );
});

test("orderbook owner-account byte ceiling rejects adversarial oversized inputs", () => {
  const ownerAccount = Buffer.alloc(ORDERBOOK_OWNER_ACCOUNT_MAX_BYTES_V1 + 1, 0x45);
  const expected = /ownerAccount must be at most 256 bytes/i;
  assert.throws(() => deriveOrderbookOrderId(ownerAccount, 9), expected);
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        {
          side: "bid",
          tier: "hot",
          pricePerGib: "1",
          quantityGib: "1",
          ownerAccount,
          expiryUnix: "1700010000",
          nonce: "9",
          makerFeeBps: 0,
          takerFeeBps: 0,
        },
        ORDERBOOK_PRIVATE_KEY,
      ),
    expected,
  );
  assert.throws(
    () =>
      buildSignedOrderbookOrderCancel(
        {
          orderId: fixed32(0x45),
          ownerAccount,
          reason: "owner-requested",
          nonce: 10,
        },
        ORDERBOOK_PRIVATE_KEY,
      ),
    expected,
  );
});

test("field-level orderbook builder rejects noncanonical supplied order ids", () => {
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        {
          orderId: fixed32(0x11),
          side: "bid",
          tier: "hot",
          pricePerGib: "1",
          quantityGib: "12",
          ownerAccount: ORDERBOOK_OWNER_ACCOUNT,
          expiryUnix: "1700010000",
          nonce: "7",
          makerFeeBps: "25",
          takerFeeBps: "30",
        },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /canonical owner-and-nonce derivation/i,
  );
});

test("field-level orderbook builder enforces exact provider binding", () => {
  const common = {
    tier: "hot",
    pricePerGib: "1",
    quantityGib: "1",
    ownerAccount: ORDERBOOK_OWNER_ACCOUNT,
    expiryUnix: "1700010000",
    nonce: "17",
    makerFeeBps: 0,
    takerFeeBps: 0,
  };
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        { ...common, side: "bid", providerId: fixed32(0x72) },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /absent or empty for bid/i,
  );
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        { ...common, side: "ask" },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /exactly 32 bytes for ask/i,
  );
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        { ...common, side: "ask", providerId: Buffer.alloc(32) },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /must not be all zero/i,
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
          xorDebited: "100",
          providerCredit: "91",
          feeAmount: "10",
          issuedAtUnix: "1700000999",
        },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /settlement imbalance/i,
  );
});

test("field-level orderbook builders reject retired micro-XOR fields", () => {
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        {
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
      ),
    /retired/i,
  );
  assert.throws(
    () =>
      buildSignedOrderbookSettlementReceipt(
        {
          xorDebitedMicroXor: "100",
        },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /retired/i,
  );
});

test("field-level orderbook builders require canonical scale-9 XOR quantities", () => {
  const common = {
    side: "bid",
    tier: "hot",
    quantityGib: "12",
    ownerAccount: ORDERBOOK_OWNER_ACCOUNT,
    expiryUnix: "1700010000",
    nonce: "7",
    makerFeeBps: "25",
    takerFeeBps: "30",
  };
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        { ...common, pricePerGib: "1.0" },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /canonical/i,
  );
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        { ...common, pricePerGib: "0.0000000001" },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /9 fractional/i,
  );
  assert.equal(MAX_SCALED_XOR.length, 155);
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        { ...common, pricePerGib: "1".repeat(156) },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /text bound/i,
  );
});

test("field-level orderbook builders reject retired field-name aliases", () => {
  assert.throws(
    () =>
      buildSignedOrderbookOrderRequest(
        {
          side: "bid",
          tier: "hot",
          pricePerGib: "1",
          price_per_gib: "2",
          quantityGib: "12",
          ownerAccount: ORDERBOOK_OWNER_ACCOUNT,
          expiryUnix: "1700010000",
          nonce: "7",
          makerFeeBps: "25",
          takerFeeBps: "30",
        },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /retired/i,
  );
});

test("field-level orderbook builders reject noncanonical selectors", () => {
  const common = {
    tier: "hot",
    pricePerGib: "1",
    quantityGib: "12",
    ownerAccount: ORDERBOOK_OWNER_ACCOUNT,
    expiryUnix: "1700010000",
    nonce: "7",
    makerFeeBps: "25",
    takerFeeBps: "30",
  };
  for (const side of ["Bid", " bid", "BID"]) {
    assert.throws(
      () =>
        buildSignedOrderbookOrderRequest(
          { ...common, side },
          ORDERBOOK_PRIVATE_KEY,
        ),
      /canonical V1 selector/i,
    );
  }
  assert.throws(
    () =>
      buildSignedOrderbookOrderCancel(
        {
          orderId: fixed32(0x45),
          ownerAccount: ORDERBOOK_OWNER_ACCOUNT,
          reason: "owner_requested",
          nonce: 10,
        },
        ORDERBOOK_PRIVATE_KEY,
      ),
    /canonical V1 selector/i,
  );
});
