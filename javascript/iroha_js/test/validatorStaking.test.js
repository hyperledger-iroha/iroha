import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { encodeValidatorStakingValueV1 as encode, decodeValidatorStakingValueV1 as decode,
  decodeAccountIdNoritoValue } from "../src/norito.js";
import { AccountAddress } from "../src/address.js";
import { computeHashLiteralCrc } from "../src/hashLiteralCrc.js";
import { NetworkId } from "../src/networkId.js";

const rows = new Map(readFileSync(new URL("../../../fixtures/validator_staking/norito_v1.tsv", import.meta.url), "utf8")
  .split("\n").filter((line) => line && !line.startsWith("#")).map((line) => {
    const [name, hex] = line.split("\t"); return [name, Buffer.from(hex, "hex")];
  }));
const names = {
  authority_generation: "AuthorityGeneration", epoch_authorization: "EpochAuthorization",
  monetary_plan: "MonetaryPlan", monetary_bond_plan: "MonetaryPlan", monetary_unbond_plan: "MonetaryPlan", monetary_slash_plan: "MonetaryPlan",
  reward_claim_plan: "RewardClaimPlan", fee_reward_claim_plan: "RewardClaimPlan",
};
const plan = (name = "monetary_plan") => decode("MonetaryPlan", rows.get(name));
const claim = () => decode("RewardClaimPlan", rows.get("fee_reward_claim_plan"));

test("staking canonical Rust fixtures retain all monetary bindings and generation separation", () => {
  assert.equal(Object.keys(names).length, 8);
  for (const [name, type] of Object.entries(names)) {
    assert.ok(rows.has(name), `missing Rust fixture ${name}`);
    assert.deepEqual(encode(type, decode(type, rows.get(name))), rows.get(name));
  }
  const generation = decode("AuthorityGeneration", rows.get("authority_generation"));
  const epoch = decode("EpochAuthorization", rows.get("epoch_authorization"));
  assert.equal(generation.generation, 0n); assert.equal(generation.validators.length, 4);
  assert.equal(epoch.authority_generation, generation.generation);
  assert.ok(epoch.network_id.equals(generation.network_id));
  const later = { ...epoch, epoch: 42n, first_height: 4201n, last_height: 4300n, decision: { kind: "retain", value: null } };
  assert.equal(decode("EpochAuthorization", encode("EpochAuthorization", later)).authority_generation, 0n);
  for (const [row, kind] of [["monetary_plan", "registration"], ["monetary_bond_plan", "bond"], ["monetary_unbond_plan", "unbond"], ["monetary_slash_plan", "slash"]]) {
    const value = plan(row);
    assert.equal(value.precondition.kind, kind); assert.equal(value.precondition.value.activation_height, 201n);
    assert.equal(value.source_asset.definition, "6TEAJqbb8oEPmLncoNiMRbLEK6tw");
    assert.equal(value.destination_asset.definition, value.source_asset.definition);
    assert.deepEqual(value.source_asset.scope, { kind: "global", value: null });
    assert.equal(value.amount, "1000"); assert.ok(value.network_scope.value.equals(generation.network_id));
  }
  assert.deepEqual(plan("monetary_bond_plan").precondition.value.peer_id, generation.validators[0].validator);
  assert.equal(plan("monetary_slash_plan").precondition.value.slashable_exposure, "1500");
  assert.deepEqual(plan("monetary_unbond_plan").source_asset, plan().destination_asset);
});

test("staking typed preconditions reject old opaque bindings, wrong hashes and malformed key geometry", () => {
  const value = plan("monetary_bond_plan");
  assert.throws(() => encode("MonetaryPlan", { ...value, precondition: { kind: "bond", value: { activation_height: 201n, binding: Buffer.of(1) } } }), /exact native fields/);
  assert.throws(() => encode("MonetaryPlan", { ...value, precondition: { kind: "unknown", value: null } }), /unknown variant/);
  assert.throws(() => encode("MonetaryPlan", { ...value, precondition: { kind: "unbond", value: { activation_height: 201n, request_hash: "hash:00#00" } } }));
  assert.throws(() => encode("MonetaryPlan", { ...value, precondition: { ...value.precondition, value: { ...value.precondition.value, peer_id: { public_key: "00" } } } }));
  const zero = "00".repeat(32);
  const unmarked = `hash:${zero}#${computeHashLiteralCrc("hash", zero)}`;
  assert.throws(() => encode("MonetaryPlan", { ...value, precondition: { kind: "unbond", value: { activation_height: 201n, request_hash: unmarked } } }), /marker/);

  // A tiny genuine AccountController frame advertises an impossible ConstVec count.
  // The shared owner must reject before Buffer.allocUnsafe(count).
  const count = Buffer.alloc(8); count.writeBigUInt64LE(1n << 40n);
  assert.throws(() => decodeAccountIdNoritoValue(Buffer.concat([Buffer.alloc(4), Buffer.of(8), count])), /count exceeds its encoded byte geometry/);
  assert.throws(() => encode("MonetaryPlan", { ...value, valid_until_height: Number.MAX_SAFE_INTEGER + 1 }), /exact unsigned/);
});

test("staking fee claims require explicit presence and retain independent receipt and source bounds", () => {
  const value = claim(), fee = value.fee_claim;
  assert.equal(fee.beneficiary_revision, 4n); assert.equal(fee.expected_claim_sequence, 5n); assert.equal(fee.amount, "7");
  assert.deepEqual(fee.source_asset, plan().destination_asset); assert.deepEqual(fee.destination_asset, plan().source_asset);
  const { fee_claim: removed, ...absent } = value;
  assert.throws(() => encode("RewardClaimPlan", absent), /exact native fields/);
  assert.throws(() => encode("RewardClaimPlan", { ...value, fee_claim: undefined }), /explicit null/);
  assert.equal(decode("RewardClaimPlan", rows.get("reward_claim_plan")).fee_claim, null);
  assert.throws(() => encode("RewardClaimPlan", { ...value, records: [...value.records, value.records[0]] }), /advance/);
  assert.throws(() => encode("RewardClaimPlan", { ...value, sources: [...value.sources, value.sources[0]] }), /strict AssetId order/);
  assert.throws(() => encode("RewardClaimPlan", { ...value, records: Array(65).fill(value.records[0]) }), /exceeds 64/);
  assert.throws(() => encode("RewardClaimPlan", { ...value, fee_claim: { ...fee, lifecycle_seal: Buffer.alloc(32) } }), /fee reward custody/);
  assert.throws(() => encode("RewardClaimPlan", { ...value, fee_claim: { ...fee, amount: "0" } }), /fee reward custody/);
  assert.throws(() => encode("RewardClaimPlan", { ...value, fee_claim: { ...fee, destination_asset: fee.source_asset } }), /changes recipient/);
  assert.throws(() => encode("RewardClaimPlan", { ...value, fee_claim: { ...fee, source_asset: { ...fee.source_asset, scope: { kind: "dataspace", value: 1n } } } }), /fee reward custody/);
  assert.throws(() => encode("RewardClaimPlan", { ...value, sources: [{ ...value.sources[0], expected_accrued: "0" }] }), /accrual/);
  const dust = { ...value, sources: [{ ...value.sources[0], payout: "0" }] };
  assert.equal(decode("RewardClaimPlan", encode("RewardClaimPlan", dust)).sources[0].payout, "0");
});

test("staking exact layout rejects truncation, trailing data and superseded optional layout", () => {
  for (const [name, type] of Object.entries(names)) {
    const bytes = rows.get(name);
    assert.throws(() => decode(type, bytes.subarray(0, bytes.length - 1)), name);
    assert.throws(() => decode(type, Buffer.concat([bytes, Buffer.of(0)])), name);
  }
  // The current no-fee row ends with a required one-byte None field.
  const reward = rows.get("reward_claim_plan"); assert.deepEqual(reward.subarray(-2), Buffer.of(1, 0));
  assert.throws(() => decode("RewardClaimPlan", reward.subarray(0, -2)));
  const bytes = rows.get("monetary_plan");
  assert.equal(bytes[0], 37);
  assert.throws(() => decode("MonetaryPlan", Buffer.concat([Buffer.of(0xa5, 0), bytes.subarray(1)])), /varint is not minimally encoded/);
});

test("staking authority geometry and epoch units are exact without certifying observations", () => {
  const authority = decode("AuthorityGeneration", rows.get("authority_generation"));
  assert.throws(() => encode("AuthorityGeneration", { ...authority, validators: authority.validators.slice(0, 3) }), /geometry/);
  assert.throws(() => encode("AuthorityGeneration", { ...authority, validators: Array(32).fill(authority.validators[0]) }), /exceeds 31/);
  assert.throws(() => encode("AuthorityGeneration", { ...authority, version: 2 }), /geometry/);
  const epoch = decode("EpochAuthorization", rows.get("epoch_authorization"));
  assert.throws(() => encode("EpochAuthorization", { ...epoch, first_height: 0 }), /authorization/);
  assert.throws(() => encode("EpochAuthorization", { ...epoch, decision: { kind: "retain", value: 0 } }), /explicit null/);
  assert.throws(() => NetworkId.fromBytes(Buffer.alloc(32)));
  assert.throws(() => decode("toString", Buffer.alloc(0)), /unknown staking value type/);
});


test("staking scoped and multisig assets preserve canonical account controllers and source ordering", () => {
  const value = plan(), controller = AccountAddress.fromI105(value.source_asset.account).controllerInfo();
  const multisig = new AccountAddress(
    { version: 0, classId: 1, normVersion: 1, extFlag: false },
    { tag: 1, version: 1, threshold: 1, members: [{ curve: controller.curve, publicKey: controller.publicKey, weight: 1 }] },
  ).toI105();
  const exact = { ...value, source_asset: { ...value.source_asset, account: multisig } };
  assert.equal(decode("MonetaryPlan", encode("MonetaryPlan", exact)).source_asset.account, multisig);
  const scope = { kind: "dataspace", value: 18446744073709551615n };
  const scoped = { ...value, source_asset: { ...value.source_asset, scope }, destination_asset: { ...value.destination_asset, scope } };
  assert.equal(decode("MonetaryPlan", encode("MonetaryPlan", scoped)).source_asset.scope.value, scope.value);
  assert.throws(() => encode("MonetaryPlan", { ...scoped, destination_asset: value.destination_asset }), /invalid staking/);
  const reward = claim(), first = reward.sources[0];
  // Actual fixture keys are 0x5b... then 0xe2..., in Rust PublicKey order.
  const second = { ...first, source_asset: first.destination_asset, expected_accrued: null };
  const ordered = { ...reward, sources: [first, second] };
  assert.deepEqual(decode("RewardClaimPlan", encode("RewardClaimPlan", ordered)).sources, ordered.sources);
  assert.throws(() => encode("RewardClaimPlan", { ...reward, sources: [second, first] }), /strict AssetId order/);
});
