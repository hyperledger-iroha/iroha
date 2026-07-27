import assert from "node:assert/strict";
import test from "node:test";

import {
  VALIDATION_FEE_DS_SCALE,
  normalizeValidationFeePolicyV1,
} from "../src/validationFeePolicy.js";

function policyFixture() {
  return {
    schema_version: 1,
    chain_id: "boi-digital-shekel",
    genesis_hash: "12".repeat(32),
    policy_version: 1,
    previous_policy_hash: null,
    ds_asset_id: "62Fk4FPcMuLvW5QjDGNF2a4jAmjM",
    ds_scale: 2,
    fee: "0.1",
    treasury_account_id: "ed0120AABB",
    charging_mode: {
      charging_mode: "PER_QUALIFYING_TRANSFER_INSTRUCTION",
      value: null,
    },
    effective_from_height: 42,
    expires_after_height: null,
    exemption_classes: [],
    treasury_payout_binding: null,
  };
}

test("validation-fee policy normalizes the exact DS contract losslessly", () => {
  const normalized = normalizeValidationFeePolicyV1(policyFixture());

  assert.equal(VALIDATION_FEE_DS_SCALE, 2);
  assert.equal(normalized.ds_asset_id, "62Fk4FPcMuLvW5QjDGNF2a4jAmjM");
  assert.equal(normalized.ds_scale, 2);
  assert.equal(normalized.policy_version, "1");
  assert.equal(normalized.effective_from_height, "42");
  assert.ok(Object.isFrozen(normalized));
  assert.ok(Object.isFrozen(normalized.charging_mode));
  assert.ok(Object.isFrozen(normalized.exemption_classes));
});

test("validation-fee policy rejects unknown fields and a non-DS scale", () => {
  assert.throws(
    () =>
      normalizeValidationFeePolicyV1({
        ...policyFixture(),
        retired_asset_id: "legacy",
      }),
    /contain exactly/u,
  );
  assert.throws(
    () =>
      normalizeValidationFeePolicyV1({
        ...policyFixture(),
        ds_scale: 6,
      }),
    /ds_scale must be 2/u,
  );
});

test("validation-fee policy binds rollover hashes and validity windows", () => {
  assert.throws(
    () =>
      normalizeValidationFeePolicyV1({
        ...policyFixture(),
        policy_version: 2,
      }),
    /previous_policy_hash/u,
  );
  assert.throws(
    () =>
      normalizeValidationFeePolicyV1({
        ...policyFixture(),
        expires_after_height: 42,
      }),
    /validity window/u,
  );

  const normalized = normalizeValidationFeePolicyV1({
    ...policyFixture(),
    policy_version: 2n,
    previous_policy_hash: "34".repeat(32),
    expires_after_height: "43",
  });
  assert.equal(normalized.policy_version, "2");
  assert.equal(normalized.previous_policy_hash, "34".repeat(32));
  assert.equal(normalized.expires_after_height, "43");
});
