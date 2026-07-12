import { test } from "node:test";
import assert from "node:assert/strict";

import {
  normalizeOfflineOperationStatus,
  parseOfflineJson,
} from "../src/offlineApi.js";

const OPERATION_ID = "11".repeat(32);
const TRANSACTION_HASH = "22".repeat(32);

function fixedBytes(byte) {
  return Array(32).fill(byte);
}

function topUpAnchor(overrides = {}) {
  const amount = overrides.amount ?? { atomic_units: 17, scale: 4 };
  const currentNote = overrides.current_note ?? {
    chain_id: "wonderland",
    asset: "xor",
    note_commitment: fixedBytes(0x41),
    spend_nullifier: fixedBytes(0x51),
    amount: { ...amount },
  };
  return {
    version: 2,
    chain_id: "wonderland",
    payer: "alice",
    asset: "xor##alice",
    asset_scale: amount.scale,
    amount,
    initial_root: fixedBytes(0x10),
    finalized_root: fixedBytes(0x20),
    topup_anchor_nullifiers: [fixedBytes(0x31)],
    current_note: currentNote,
    topup_operation_id: fixedBytes(0x11),
    transfer_verifier_id: { backend: "halo2/ipa", name: "offline-transfer" },
    transfer_verifier_commitment: fixedBytes(0x61),
    artifact_generation: "generation-1",
    finalized_height: 12,
    finalized_tx_hash: fixedBytes(0x22),
    anchor_digest: fixedBytes(0x71),
    ...overrides,
  };
}

function appliedTopUpStatus(anchor = topUpAnchor(), overrides = {}) {
  return {
    state: "applied",
    value: {
      operation_id: OPERATION_ID,
      result: {
        kind: "top_up",
        result: {
          transaction_hash: TRANSACTION_HASH,
          finalized_block_height: 12,
          server_time_ms: 13,
          anchor,
          ...overrides,
        },
      },
    },
  };
}

function stringifyWideJson(value) {
  return JSON.stringify(value, (_key, item) =>
    typeof item === "bigint" ? `__wide_integer_${item}` : item)
    .replace(/"__wide_integer_([0-9]+)"/gu, "$1");
}

test("Offline lossless JSON parser preserves wide integer tokens", () => {
  const parsed = parseOfflineJson(
    "{\"safe\":9007199254740991,\"wide\":9007199254740993,"
      + "\"max_u64\":18446744073709551615,"
      + "\"max_u128\":340282366920938463463374607431768211455,"
      + "\"fraction\":1.25,\"negative\":-9007199254740993}",
  );

  assert.equal(parsed.safe, Number.MAX_SAFE_INTEGER);
  assert.equal(parsed.wide, 9_007_199_254_740_993n);
  assert.equal(parsed.max_u64, 18_446_744_073_709_551_615n);
  assert.equal(parsed.max_u128, (1n << 128n) - 1n);
  assert.equal(parsed.fraction, 1.25);
  assert.equal(parsed.negative, -9_007_199_254_740_993n);
});

test("Offline lossless JSON parser rejects ambiguous and malformed inputs", () => {
  for (const input of [
    '{"field":1,"field":2}',
    '{"field":01}',
    '{"field":1e999}',
    '{"field":1} trailing',
    '"\\ud800"',
    '[1,]',
    '{"field":true,}',
  ]) {
    assert.throws(() => parseOfflineJson(input));
  }

  let deep = "0";
  for (let index = 0; index < 130; index += 1) deep = `[${deep}]`;
  assert.throws(() => parseOfflineJson(deep), /maximum JSON nesting depth/u);
});

test("Offline lossless JSON parser does not allow prototype mutation", () => {
  const parsed = parseOfflineJson('{"__proto__":{"polluted":true},"constructor":7}');

  assert.equal(Object.getPrototypeOf(parsed), Object.prototype);
  assert.equal(Object.prototype.polluted, undefined);
  assert.equal(Object.prototype.hasOwnProperty.call(parsed, "__proto__"), true);
  assert.equal(parsed.__proto__.polluted, true);
  assert.equal(parsed.constructor, 7);
});

test("Offline status normalization retains wide heights and nested amounts", () => {
  const maxU128 = (1n << 128n) - 1n;
  const maxU64 = (1n << 64n) - 1n;
  const amount = { atomic_units: maxU128, scale: 4 };
  const parsed = parseOfflineJson(stringifyWideJson(appliedTopUpStatus(
    topUpAnchor({
      amount,
      current_note: {
        chain_id: "wonderland",
        asset: "xor",
        note_commitment: fixedBytes(0x41),
        spend_nullifier: fixedBytes(0x51),
        amount,
      },
      finalized_height: maxU64,
    }),
    { finalized_block_height: maxU64, server_time_ms: 9_007_199_254_740_993n },
  )));
  const status = normalizeOfflineOperationStatus(parsed, OPERATION_ID);

  assert.equal(status.value.result.result.finalized_block_height, maxU64);
  assert.equal(status.value.result.result.server_time_ms, 9_007_199_254_740_993n);
  assert.equal(status.value.result.result.anchor.amount.atomic_units, maxU128);
});

test("Offline top-up anchors are decoded into the closed typed contract", () => {
  const anchor = topUpAnchor({ unknown_member: { attacker_controlled: true } });
  const status = normalizeOfflineOperationStatus(appliedTopUpStatus(anchor), OPERATION_ID);
  const normalized = status.value.result.result.anchor;

  assert.equal(normalized.version, 2);
  assert.equal(normalized.asset_scale, 4);
  assert.deepEqual(normalized.transfer_verifier_id, {
    backend: "halo2/ipa",
    name: "offline-transfer",
  });
  assert.deepEqual(normalized.topup_operation_id, fixedBytes(0x11));
  assert.equal(Object.hasOwn(normalized, "unknown_member"), false);
});

test("Offline top-up anchors reject malformed and cross-resource-conflicting fields", () => {
  const invalid = [];
  const missingDigest = topUpAnchor();
  delete missingDigest.anchor_digest;
  invalid.push(
    missingDigest,
    topUpAnchor({ version: 1 }),
    topUpAnchor({ asset_scale: 29 }),
    topUpAnchor({ asset_scale: 3 }),
    topUpAnchor({ finalized_root: fixedBytes(0x10) }),
    topUpAnchor({ topup_anchor_nullifiers: [] }),
    topUpAnchor({
      topup_anchor_nullifiers: [fixedBytes(0x31), fixedBytes(0x32), fixedBytes(0x33)],
    }),
    topUpAnchor({ topup_anchor_nullifiers: [fixedBytes(0x31), fixedBytes(0x31)] }),
    topUpAnchor({ topup_anchor_nullifiers: [fixedBytes(0x32), fixedBytes(0x31)] }),
    topUpAnchor({ topup_anchor_nullifiers: [fixedBytes(0x41)] }),
    topUpAnchor({ topup_operation_id: fixedBytes(0x12) }),
    topUpAnchor({ finalized_height: 11 }),
    topUpAnchor({ finalized_tx_hash: fixedBytes(0x23) }),
    topUpAnchor({ anchor_digest: fixedBytes(0) }),
    topUpAnchor({ transfer_verifier_id: { backend: "", name: "offline-transfer" } }),
    topUpAnchor({ transfer_verifier_id: { backend: "halo2/ipa", name: "v".repeat(257) } }),
    topUpAnchor({ transfer_verifier_commitment: fixedBytes(0) }),
    topUpAnchor({ artifact_generation: "é".repeat(65) }),
    topUpAnchor({
      current_note: {
        chain_id: "wonderland",
        asset: "xor",
        note_commitment: fixedBytes(0x41),
        spend_nullifier: fixedBytes(0x41),
        amount: { atomic_units: 17, scale: 4 },
      },
    }),
    topUpAnchor({
      current_note: {
        chain_id: "other-chain",
        asset: "xor",
        note_commitment: fixedBytes(0x41),
        spend_nullifier: fixedBytes(0x51),
        amount: { atomic_units: 17, scale: 4 },
      },
    }),
    topUpAnchor({
      current_note: {
        chain_id: "wonderland",
        asset: "xor",
        note_commitment: fixedBytes(0x41),
        spend_nullifier: fixedBytes(0x51),
        amount: { atomic_units: 18, scale: 4 },
      },
    }),
  );

  for (const anchor of invalid) {
    assert.throws(() => normalizeOfflineOperationStatus(appliedTopUpStatus(anchor), OPERATION_ID));
  }
});

test("Offline applied statuses reject zero finality fields", () => {
  for (const kind of ["top_up", "redeem"]) {
    for (const field of ["finalized_block_height", "server_time_ms"]) {
      for (const zero of [0, 0n]) {
        const result = {
          transaction_hash: TRANSACTION_HASH,
          finalized_block_height: 1,
          server_time_ms: 1,
        };
        result[field] = zero;
        if (kind === "top_up") {
          result.anchor = topUpAnchor({ finalized_height: result.finalized_block_height });
        }
        const status = {
          state: "applied",
          value: {
            operation_id: OPERATION_ID,
            result: { kind, result },
          },
        };

        assert.throws(
          () => normalizeOfflineOperationStatus(status, OPERATION_ID),
          new RegExp(field, "u"),
        );
      }
    }
  }
});

function rejectedStatus(error) {
  return {
    state: "rejected",
    value: {
      operation_id: OPERATION_ID,
      kind: { kind: "redeem", value: null },
      transaction_hash: TRANSACTION_HASH,
      error,
    },
  };
}

test("Offline error codes use the global finite grammar", () => {
  const accepted = normalizeOfflineOperationStatus(
    rejectedStatus({ code: "1_future_code", message: "future rejection" }),
    OPERATION_ID,
  );
  assert.equal(accepted.value.error.code, "1_future_code");

  for (const code of ["", "_leading_underscore", "a".repeat(65)]) {
    assert.throws(() =>
      normalizeOfflineOperationStatus(
        rejectedStatus({ code, message: "invalid code" }),
        OPERATION_ID,
      ),
    );
  }
});

test("Offline error details expose only the closed typed fields", () => {
  const status = normalizeOfflineOperationStatus(
    rejectedStatus({
      code: "offline_operation_rejected",
      message: "rejected",
      unknown_envelope_member: "ignored",
      details: {
        layer: "torii",
        reject_code: "QUEUE_FULL",
        retry_after_seconds: 3,
        endpoint: "/v1/offline/redeem",
        field: "authorization",
        expected: "fresh",
        actual: "replayed",
        profile: "minamoto",
        chain_discriminant: 753,
        tx_hash: TRANSACTION_HASH,
        last_status: "queued",
        hint: "retry later",
        unknown_detail: { attacker_controlled: true },
        queue: {
          state: "saturated",
          queued: 5,
          capacity: 5,
          saturated: true,
          unknown_queue_member: "ignored",
        },
        axt: {
          code: "handle_era_stale",
          reason: "stale handle era",
          snapshot_version: 7,
          dataspace: 8,
          lane: 9,
          next_min_handle_era: 10,
          next_min_sub_nonce: 11,
          unknown_axt_member: "ignored",
        },
      },
    }),
    OPERATION_ID,
  );

  assert.deepEqual(status.value.error.details, {
    layer: "torii",
    reject_code: "QUEUE_FULL",
    endpoint: "/v1/offline/redeem",
    field: "authorization",
    expected: "fresh",
    actual: "replayed",
    profile: "minamoto",
    tx_hash: TRANSACTION_HASH,
    last_status: "queued",
    hint: "retry later",
    retry_after_seconds: 3,
    chain_discriminant: 753,
    queue: { state: "saturated", queued: 5, capacity: 5, saturated: true },
    axt: {
      code: "handle_era_stale",
      reason: "stale handle era",
      snapshot_version: 7,
      dataspace: 8,
      lane: 9,
      next_min_handle_era: 10,
      next_min_sub_nonce: 11,
    },
  });
  assert.equal(Object.hasOwn(status.value.error, "unknown_envelope_member"), false);
});

test("Offline error details reject malformed nested types and ranges", () => {
  const invalidDetails = [
    { queue: { state: "healthy", queued: 0, capacity: 1 } },
    { queue: { state: "healthy", queued: -1, capacity: 1, saturated: false } },
    { queue: { state: "healthy", queued: 0, capacity: 1, saturated: "false" } },
    { retry_after_seconds: -1 },
    { chain_discriminant: 65_536 },
    { axt: { lane: 4_294_967_296n } },
    { axt: { snapshot_version: -1 } },
    { axt: [] },
  ];

  for (const details of invalidDetails) {
    assert.throws(() =>
      normalizeOfflineOperationStatus(
        rejectedStatus({ code: "rejected", message: "no", details }),
        OPERATION_ID,
      ),
    );
  }
});
