import assert from "node:assert/strict";
import { test } from "node:test";

import { __sumeragiNativeAmxTestHelpers as helpers } from "../src/sumeragiTyped.js";
import { computeHashLiteralCrc } from "../src/hashLiteralCrc.js";

function settlement() {
  const body = "01".repeat(32);
  return {
    lane_id: 0,
    dataspace_id: 0,
    lane_incarnation: `hash:${body}#${computeHashLiteralCrc("hash", body)}`,
    participant_lane_block_height: 1,
    authority_context_height: 2,
    previous_native_settlement_hash: null,
    source_ids: ["F0".repeat(32), "10".repeat(32)],
  };
}

test("Native participant settlement retains FIFO identity and accepts zero route coordinates", () => {
  const value = settlement();
  const parsed = helpers.parseParticipantSettlement(value, "participant");
  assert.deepEqual(parsed, value);
  assert.ok(Object.isFrozen(parsed));
  assert.ok(Object.isFrozen(parsed.source_ids));
  const original = helpers.computeParticipantSettlementHash(parsed);
  assert.notEqual(original, helpers.computeParticipantSettlementHash({
    ...value, source_ids: [...value.source_ids].reverse(),
  }));
  for (const field of ["participant_lane_block_height", "authority_context_height"]) {
    assert.notEqual(original, helpers.computeParticipantSettlementHash({ ...value, [field]: 3 }));
  }
});

test("Native participant settlement rejects retired economic and nested fields", () => {
  for (const field of ["block_height", "tx_count", "receipts", "total_local_amount",
    "total_xor_due", "total_xor_after_haircut", "total_xor_variance", "swap_metadata",
    "nexus_fee_receipts", "native_amx_receipts"]) {
    const value = { ...settlement(), [field]: null };
    assert.throws(() => helpers.parseParticipantSettlement(value, "participant"), /unknown field/u);
    assert.throws(() => helpers.computeParticipantSettlementHash(value), /unknown field/u);
  }
});

test("Native participant settlement enforces exact source and numeric bounds", () => {
  for (const update of [
    { source_ids: [] }, { source_ids: ["00".repeat(32)] },
    { source_ids: ["F0".repeat(32), "F0".repeat(32)] },
    { source_ids: Array(4097).fill("01".repeat(32)) },
    { source_ids: ["f0".repeat(32)] }, { source_ids: [Array(32).fill(1)] },
    { participant_lane_block_height: 0 }, { authority_context_height: 0 },
    { lane_id: 1n << 32n }, { dataspace_id: 1n << 64n },
    { lane_id: -1 }, { authority_context_height: true },
  ]) {
    assert.throws(() => helpers.computeParticipantSettlementHash({ ...settlement(), ...update }));
  }
  const zeroBody = "00".repeat(31) + "01";
  const markedZero = `hash:${zeroBody}#${computeHashLiteralCrc("hash", zeroBody)}`;
  assert.throws(() => helpers.computeParticipantSettlementHash({ ...settlement(), lane_incarnation: markedZero }), /marked zero/u);
  const sources = Array.from({ length: 4096 }, (_, index) =>
    (index + 1).toString(16).toUpperCase().padStart(64, "0"));
  const maximum = (1n << 64n) - 1n;
  assert.equal(helpers.parseParticipantSettlement({ ...settlement(), source_ids: sources,
    dataspace_id: maximum, participant_lane_block_height: maximum,
    authority_context_height: maximum }, "participant").source_ids.length, 4096);
  for (const field of Object.keys(settlement())) {
    const value = settlement();
    delete value[field];
    assert.throws(() => helpers.computeParticipantSettlementHash(value), /missing required/u);
  }
});

test("Native participant settlement requires and binds its exact Native history link", () => {
  const value = settlement();
  assert.equal(helpers.computeParticipantSettlementHash(value),
    "hash:350CB3C0D8728E39820775AC522B345C84631FA81BA164F72FB70043657012CF#EB51");
  const later = { ...value, participant_lane_block_height: 2 };
  assert.equal(helpers.computeParticipantSettlementHash(later),
    "hash:C3196EEEB6B5795424F82CDCCA551495E9F459E75EE274EE957FEC51EEE69393#EC1E");
  const linked = { ...later, previous_native_settlement_hash: value.lane_incarnation };
  assert.equal(helpers.computeParticipantSettlementHash(linked),
    "hash:1F71F0A536D50BB281A9C0FC3A9BF7AA5070F8860BF24173671FFB00D500DED5#1CF3");
  assert.equal(helpers.parseParticipantSettlement(linked, "participant").previous_native_settlement_hash,
    value.lane_incarnation);
  assert.throws(() => helpers.computeParticipantSettlementHash({ ...value,
    previous_native_settlement_hash: value.lane_incarnation }), /null at participant height one/u);
  const oldSixField = { ...later };
  delete oldSixField.previous_native_settlement_hash;
  assert.throws(() => helpers.computeParticipantSettlementHash(oldSixField), /missing required/u);
  const zeroBody = "00".repeat(31) + "01";
  const markedZero = `hash:${zeroBody}#${computeHashLiteralCrc("hash", zeroBody)}`;
  for (const previous of [markedZero, undefined, "", true, {}, value.lane_incarnation.slice(0, -1) + "0"]) {
    assert.throws(() => helpers.computeParticipantSettlementHash({ ...later,
      previous_native_settlement_hash: previous }));
  }
});
