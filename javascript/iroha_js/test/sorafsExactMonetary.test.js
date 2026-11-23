import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { computeDaIngestSigningDigest } from "../src/dataAvailability.js";
import { LocalSigningContext, ToriiClient } from "../src/toriiClient.js";
import { publicKeyFromPrivate } from "../src/crypto.js";
import { AccountAddress } from "../src/address.js";
import { NetworkId } from "../src/networkId.js";
import { parseStrictLosslessIntegerJson, stringifyStrictLosslessIntegerJson } from "../src/strictLosslessJson.js";
import {
  buildSignedOrderbookOrderRequest,
  validateOrderbookPayload,
} from "../src/sorafs.js";
import { makeNativeTest } from "./helpers/native.js";

const BASE_URL = "https://localhost:8080";
const PRIVATE_KEY = Buffer.alloc(32, 0xb7);
const DA_NETWORK_ID = NetworkId.fromBytes(Buffer.alloc(32, 0xa5));
const DA_OWNER = AccountAddress.fromAccount({ publicKey: publicKeyFromPrivate(PRIVATE_KEY) }).toI105(753);
const OWNER_ACCOUNT = Buffer.from("merchant@paynet", "utf8");
const SIGNED_512_MAX = (1n << 511n) - 1n;
const SIGNED_512_OVERFLOW = (1n << 511n).toString();
const WIDE_XOR = "340282366920938463463374607431768211456.000000001";
const nativeTest = makeNativeTest(test);

function jsonResponse(payload, status = 200) {
  return new Response(stringifyStrictLosslessIntegerJson(payload, "test response"), {
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
    stripe_layout: {
      total_stripes: 1,
      shards_per_stripe: 14,
      row_parity_stripes: 0,
    },
    retention_policy: {
      hot_retention_secs: 3600,
      cold_retention_secs: 172800,
      required_replicas: 3,
      storage_class: { type: "Hot", value: null },
      governance_tag: ["da.sidecar"],
    },
    queued_at_unix: 1234,
    operator_signature: "AA".repeat(64),
    rent_quote: rentQuote,
  };
}

async function submitDaFixture(rentQuote, options = {}, changeReceipt = () => {}) {
  const client = new ToriiClient(BASE_URL, {
    localSigningContext: new LocalSigningContext(DA_NETWORK_ID, 753),
    canonicalRequestAuth: { accountId: DA_OWNER, privateKey: PRIVATE_KEY },
    fetchImpl: async (_url, init) => {
      const signed = parseStrictLosslessIntegerJson(init.body, "signed request");
      const receipt = {
        ...daReceiptFixture(rentQuote), client_blob_id: signed.client_blob_id,
        blob_hash: signed.payload_hash, lane_id: signed.lane_id, epoch: signed.epoch,
      };
      changeReceipt(receipt);
      const pin_scope = {
        network_id: signed.network_id, owner: signed.owner, lane_id: signed.lane_id, epoch: signed.epoch, sequence: signed.sequence,
        request_authorization_digest: [Array.from(computeDaIngestSigningDigest(signed))], storage_ticket: receipt.storage_ticket,
        manifest_hash: receipt.manifest_hash, commitment_digest: [Array(32).fill(77)], alias: null,
      };
      return jsonResponse({ status: "pending_pin_authorization", duplicate: false, receipt, pin_scope }, 202);
    },
  });
  return client.submitDaBlob({
    networkId: DA_NETWORK_ID,
    owner: DA_OWNER,
    payload: Buffer.from("car-bytes"),
    codec: "nexus_lane_sidecar",
    laneId: 11,
    epoch: 22,
    sequence: 33,
    privateKey: PRIVATE_KEY,
    clientBlobId: Buffer.alloc(32, 0x11),
    ...options,
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
    /retired/i,
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
  for (const unexpected of ["base_rent_micro", "egress_surcharge"]) {
    await assert.rejects(
      () => submitDaFixture({ ...quote, [unexpected]: "1000000" }),
      { message: new RegExp(`unsupported fields: ${unexpected}`) },
    );
  }
});

test("DA signed requests and persisted receipts preserve the full unsigned wire range", async (t) => {
  const artifactDir = await mkdtemp(join(tmpdir(), "iroha-da-wire-"));
  t.after(() => rm(artifactDir, { recursive: true, force: true }));
  const max = (1n << 64n) - 1n;
  const rent = { base_rent: "0", protocol_reserve: "0", provider_reward: "0",
    pdp_bonus: "0", potr_bonus: "0", egress_credit_per_gib: "0" };
  const options = { artifactDir, laneId: 0xffff_ffff, epoch: max, sequence: max - 1n,
    retentionPolicy: { hotRetentionSecs: max, coldRetentionSecs: 0, governanceTag: "" } };
  const reply = await submitDaFixture(rent, options, (receipt) => {
    receipt.queued_at_unix = max;
    receipt.retention_policy.hot_retention_secs = max;
    receipt.pdp_commitment = "";
  });
  const request = parseStrictLosslessIntegerJson(await readFile(reply.artifactPaths.requestJsonPath, "utf8"), "request");
  const receipt = parseStrictLosslessIntegerJson(await readFile(reply.artifactPaths.receiptJsonPath, "utf8"), "receipt");
  assert.equal(request.epoch, max);
  assert.equal(request.sequence, max - 1n);
  assert.deepEqual(request.retention_policy.governance_tag, [""]);
  assert.equal(reply.pinScope.sequence, max - 1n);
  assert.equal(reply.receipt.epoch, max);
  assert.equal(reply.receipt.lane_id, 0xffff_ffff);
  assert.equal(reply.receipt.retention_policy.hot_retention_secs, max);
  assert.equal(receipt.queued_at_unix, max);
  assert.equal(reply.receipt.pdp_commitment_b64, "");
  assert.deepEqual(reply.receipt.pdp_commitment_bytes, Buffer.alloc(0));
  for (const [field, invalid] of [["laneId", 0x1_0000_0000], ["epoch", max + 1n], ["sequence", -1n]]) {
    await assert.rejects(() => submitDaFixture(rent, { [field]: invalid }), new RegExp(field));
  }
});
