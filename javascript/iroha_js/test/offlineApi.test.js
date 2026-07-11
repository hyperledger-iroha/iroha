import { test } from "node:test";
import assert from "node:assert/strict";

import {
  normalizeOfflineOperationStatus,
  parseOfflineJson,
} from "../src/offlineApi.js";

const OPERATION_ID = "11".repeat(32);
const TRANSACTION_HASH = "22".repeat(32);

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
  const parsed = parseOfflineJson(`{
    "state":"applied",
    "value":{
      "operation_id":"${OPERATION_ID}",
      "result":{
        "kind":"top_up",
        "result":{
          "transaction_hash":"${TRANSACTION_HASH}",
          "finalized_block_height":18446744073709551615,
          "server_time_ms":9007199254740993,
          "anchor":{"amount":{"atomic_units":340282366920938463463374607431768211455}}
        }
      }
    }
  }`);
  const status = normalizeOfflineOperationStatus(parsed, OPERATION_ID);

  assert.equal(status.value.result.result.finalized_block_height, (1n << 64n) - 1n);
  assert.equal(status.value.result.result.server_time_ms, 9_007_199_254_740_993n);
  assert.equal(
    status.value.result.result.anchor.amount.atomic_units,
    (1n << 128n) - 1n,
  );
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
