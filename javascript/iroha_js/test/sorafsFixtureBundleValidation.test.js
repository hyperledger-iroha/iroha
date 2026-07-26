import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import {
  SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1,
  SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS,
  validateFixtureBundle,
} from "../src/sorafs.js";
import { makeNativeTest } from "./helpers/native.js";

const FIXTURE_ROOT = new URL(
  "../../../fixtures/sorafs_manifest/",
  import.meta.url,
);
const REFERENCE_SDK_OUTCOME_ROOT = new URL("reference_sdk/", FIXTURE_ROOT);
const KINDS = SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS;
const REFERENCE_SDK_GENERATED_AT = 1_700_001_234;
const fixtureBundleNativeTest = makeNativeTest(test, {
  require: "sorafsValidateFixtureBundleJson",
});
const REFERENCE_SDK_BUNDLE_PROFILES = Object.freeze([
  Object.freeze({
    name: "bundle_heterogeneous_positive",
    nowUnix: 1_700_000_001,
    payloads: Object.freeze([
      [KINDS.REPLICATION_ORDER, "replication_order/order_v1.to"],
      [KINDS.PDP_COMMITMENT, "pdp/commitment_v1.to"],
      [KINDS.PDP_CHALLENGE, "pdp/challenge_v1.to"],
      [KINDS.PDP_PROOF, "pdp/proof_v1.to"],
      [KINDS.POR_CHALLENGE, "por/challenge_v1.to"],
      [KINDS.POR_PROOF, "por/proof_v1.to"],
      [KINDS.POTR_RECEIPT, "potr/receipt_v1.to"],
      [KINDS.REPAIR_TASK_RECORD, "repair/task_v1.to"],
      [KINDS.ORDERBOOK_ORDER_REQUEST, "orderbook/order_request_v1.to"],
      [KINDS.ORDERBOOK_ORDER_CANCEL, "orderbook/order_cancel_v1.to"],
      [KINDS.ORDERBOOK_TRADE_EVENT, "orderbook/trade_event_v1.to"],
      [
        KINDS.ORDERBOOK_SETTLEMENT_CHANNEL,
        "orderbook/settlement_channel_v1.to",
      ],
      [
        KINDS.ORDERBOOK_SETTLEMENT_RECEIPT,
        "orderbook/settlement_receipt_v1.to",
      ],
    ]),
  }),
  Object.freeze({
    name: "bundle_orderbook_bad_signature_negative",
    nowUnix: 1_700_000_001,
    payloads: Object.freeze([
      [KINDS.REPLICATION_ORDER, "replication_order/order_v1.to"],
      [KINDS.POR_CHALLENGE, "por/challenge_v1.to"],
      [KINDS.POR_PROOF, "por/proof_v1.to"],
      [
        KINDS.ORDERBOOK_ORDER_REQUEST,
        "orderbook/negative/order_request_bad_signature_v1.to",
      ],
    ]),
  }),
  Object.freeze({
    name: "bundle_orderbook_trailing_bytes_negative",
    nowUnix: 1_700_000_001,
    payloads: Object.freeze([
      [KINDS.REPLICATION_ORDER, "replication_order/order_v1.to"],
      [KINDS.POR_CHALLENGE, "por/challenge_v1.to"],
      [KINDS.POR_PROOF, "por/proof_v1.to"],
      [
        KINDS.ORDERBOOK_ORDER_REQUEST,
        "orderbook/negative/order_request_trailing_bytes_v1.to",
      ],
    ]),
  }),
  Object.freeze({
    name: "bundle_pdp_duplicate_hot_leaf_negative",
    nowUnix: 1_700_000_001,
    payloads: Object.freeze([
      [KINDS.REPLICATION_ORDER, "replication_order/order_v1.to"],
      [KINDS.PDP_COMMITMENT, "pdp/commitment_v1.to"],
      [
        KINDS.PDP_CHALLENGE,
        "pdp/negative/duplicate_hot_leaf_challenge_v1.to",
      ],
    ]),
  }),
  Object.freeze({
    name: "bundle_pdp_missing_signature_negative",
    nowUnix: 1_700_000_001,
    payloads: Object.freeze([
      [KINDS.REPLICATION_ORDER, "replication_order/order_v1.to"],
      [KINDS.PDP_COMMITMENT, "pdp/commitment_v1.to"],
      [KINDS.PDP_CHALLENGE, "pdp/challenge_v1.to"],
      [
        KINDS.PDP_PROOF,
        "pdp/negative/missing_signature_proof_v1.to",
      ],
    ]),
  }),
  Object.freeze({
    name: "bundle_pdp_wrong_provider_negative",
    nowUnix: 1_700_000_001,
    payloads: Object.freeze([
      [KINDS.REPLICATION_ORDER, "replication_order/order_v1.to"],
      [KINDS.PDP_COMMITMENT, "pdp/commitment_v1.to"],
      [KINDS.PDP_CHALLENGE, "pdp/challenge_v1.to"],
      [KINDS.PDP_PROOF, "pdp/negative/wrong_provider_proof_v1.to"],
    ]),
  }),
  Object.freeze({
    name: "bundle_repair_manifest_mismatch_negative",
    nowUnix: 1_700_000_001,
    payloads: Object.freeze([
      [KINDS.REPLICATION_ORDER, "replication_order/order_v1.to"],
      [
        KINDS.REPAIR_TASK_RECORD,
        "repair/negative/task_manifest_mismatch_v1.to",
      ],
    ]),
  }),
  Object.freeze({
    name: "bundle_repair_provider_unassigned_negative",
    nowUnix: 1_700_000_001,
    payloads: Object.freeze([
      [KINDS.REPLICATION_ORDER, "replication_order/order_v1.to"],
      [
        KINDS.REPAIR_TASK_RECORD,
        "repair/negative/task_provider_unassigned_v1.to",
      ],
    ]),
  }),
  Object.freeze({
    name: "bundle_routing_admission_positive",
    nowUnix: 300,
    payloads: Object.freeze([
      [KINDS.PROVIDER_ADVERT, "provider_admission/advert_v1.to"],
      [
        KINDS.PROVIDER_ADMISSION_ENVELOPE,
        "provider_admission/envelope_v1.to",
      ],
    ]),
  }),
]);

function fixture(path) {
  return readFileSync(new URL(path, FIXTURE_ROOT));
}

function assertExactReferenceSdkOutcome(outcome, profileName) {
  const expectedText = readFileSync(
    new URL(`${profileName}_validation_outcome_v1.json`, REFERENCE_SDK_OUTCOME_ROOT),
    "utf8",
  );
  assert.deepEqual(outcome, JSON.parse(expectedText));
  assert.equal(`${JSON.stringify(outcome, null, 2)}\n`, expectedText);
}

test("fixture-bundle selectors preserve the canonical V1 order", () => {
  assert.deepEqual(
    Object.values(SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS),
    [
      "provider-advert",
      "provider-admission-envelope",
      "replication-order",
      "por-challenge",
      "por-proof",
      "potr-receipt",
      "repair-evidence",
      "repair-report",
      "repair-task-record",
      "repair-slash-proposal",
      "repair-task-event",
      "orderbook-order-request",
      "orderbook-order-cancel",
      "orderbook-trade-event",
      "orderbook-settlement-channel",
      "orderbook-settlement-receipt",
      "pdp-commitment",
      "pdp-challenge",
      "pdp-proof",
    ],
  );
});

fixtureBundleNativeTest("validateFixtureBundle accepts linked replication and PoR fixtures", () => {
  const outcome = validateFixtureBundle(
    [
      {
        kind: SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS.REPLICATION_ORDER,
        bytes: fixture("replication_order/order_v1.to"),
        label: "replication-order.to",
      },
      {
        kind: SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS.POR_PROOF,
        payload: fixture("por/proof_v1.to"),
        label: "por-proof.to",
      },
    ],
    {
      nowUnix: 1_700_000_001,
      generatedAtUnix: 1_700_001_238,
    },
  );

  assert.equal(outcome.status, "Ok");
  assert.equal(outcome.code, "SFS-OK-000");
  assert.equal(outcome.generated_at, 1_700_001_238);
  assert.deepEqual(
    outcome.inputs.map((input) => input.kind),
    ["replication_order", "por_proof"],
  );
});

fixtureBundleNativeTest("fixture-bundle wrapper matches all nine release-wide outcome goldens byte-for-byte", async (t) => {
  assert.equal(REFERENCE_SDK_BUNDLE_PROFILES.length, 9);
  for (const profile of REFERENCE_SDK_BUNDLE_PROFILES) {
    await t.test(profile.name, () => {
      const outcome = validateFixtureBundle(
        profile.payloads.map(([kind, path]) => ({
          kind,
          bytes: fixture(path),
          label: path,
        })),
        {
          nowUnix: profile.nowUnix,
          generatedAtUnix: REFERENCE_SDK_GENERATED_AT,
        },
      );
      assertExactReferenceSdkOutcome(outcome, profile.name);
    });
  }
});

fixtureBundleNativeTest("validateFixtureBundle snapshots payload bytes before label access", () => {
  const order = fixture("replication_order/order_v1.to");
  const outcome = validateFixtureBundle(
    [
      {
        kind: SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS.REPLICATION_ORDER,
        bytes: order,
        get label() {
          order.fill(0);
          return "replication-order.to";
        },
      },
      {
        kind: SORAFS_FIXTURE_BUNDLE_PAYLOAD_KINDS.POR_PROOF,
        bytes: fixture("por/proof_v1.to"),
        label: "por-proof.to",
      },
    ],
    {
      nowUnix: 1_700_000_001,
      generatedAtUnix: 1_700_001_238,
    },
  );

  assert.equal(outcome.status, "Ok");
});

fixtureBundleNativeTest("validateFixtureBundle returns canonical negative outcomes", () => {
  const outcome = validateFixtureBundle(
    [
      {
        kind: "replication-order",
        bytes: fixture("replication_order/order_v1.to"),
      },
      { kind: "por-proof", bytes: Buffer.alloc(8) },
    ],
    { now_unix: 1_700_000_001, generated_at: 1_700_001_239 },
  );

  assert.equal(outcome.status, "Error");
  assert.equal(outcome.category, "norito");
  assert.equal(outcome.generated_at, 1_700_001_239);
});

test("validateFixtureBundle rejects aliases and unbounded input before native dispatch", () => {
  assert.throws(
    () =>
      validateFixtureBundle([
        { kind: "por_proof", bytes: Buffer.from([0]) },
      ]),
    /unsupported.*payload kind/i,
  );
  assert.throws(() => validateFixtureBundle([]), /1\.\.=64 entries/i);
  assert.throws(
    () =>
      validateFixtureBundle(
        Array.from(
          { length: SORAFS_FIXTURE_BUNDLE_MAX_PAYLOADS_V1 + 1 },
          () => ({ kind: "por-proof", bytes: Buffer.from([0]) }),
        ),
    ),
    /1\.\.=64 entries/i,
  );
  assert.throws(
    () =>
      validateFixtureBundle([
        {
          kind: "por-proof",
          bytes: Buffer.from([0]),
          label: "\ud800",
        },
      ]),
    /valid Unicode text/i,
  );
});
