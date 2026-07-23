import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { ToriiClient } from "../src/toriiClient.js";

const fixtureUrl = new URL(
  "../../../fixtures/sumeragi_v2/native_amx_v2_grouped.json",
  import.meta.url,
);
const fixtureDocument = JSON.parse(readFileSync(fixtureUrl, "utf8"));

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function pointerTokens(pointer) {
  assert.match(pointer, /^\//u);
  return pointer
    .slice(1)
    .split("/")
    .map((token) => token.replaceAll("~1", "/").replaceAll("~0", "~"));
}

function resolvePointer(document, pointer) {
  let target = document;
  for (const token of pointerTokens(pointer)) {
    target = Array.isArray(target) ? target[Number(token)] : target[token];
  }
  return target;
}

function pointerParent(document, pointer) {
  const tokens = pointerTokens(pointer);
  const leaf = tokens.pop();
  let parent = document;
  for (const token of tokens) {
    parent = Array.isArray(parent) ? parent[Number(token)] : parent[token];
  }
  return { parent, leaf };
}

function assignPointer(document, pointer, value) {
  const { parent, leaf } = pointerParent(document, pointer);
  parent[Array.isArray(parent) ? Number(leaf) : leaf] = value;
}

function removePointer(document, pointer) {
  const { parent, leaf } = pointerParent(document, pointer);
  if (Array.isArray(parent)) {
    parent.splice(Number(leaf), 1);
  } else {
    delete parent[leaf];
  }
}

function applyMutation(document, mutation) {
  const { op, path, value } = mutation;
  switch (op) {
    case "replace":
      assignPointer(document, path, clone(value));
      break;
    case "remove":
      removePointer(document, path);
      break;
    case "copy":
      assignPointer(document, path, clone(resolvePointer(document, value.from)));
      break;
    case "swap": {
      const target = resolvePointer(document, path);
      [target[value.left], target[value.right]] = [
        target[value.right],
        target[value.left],
      ];
      break;
    }
    case "repeat": {
      const target = resolvePointer(document, path);
      assignPointer(
        document,
        path,
        Array.from({ length: value.count }, () => clone(target[value.source_index])),
      );
      break;
    }
    default:
      assert.fail(`unsupported fixture mutation operation: ${op}`);
  }
}

function diagnosticsClient(payload) {
  return new ToriiClient("https://fixture.invalid", {
    fetchImpl: async (url) => {
      assert.equal(url, "https://fixture.invalid/v1/sumeragi/diagnostics");
      return new Response(JSON.stringify(payload), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });
}

test("Rust-owned grouped Native AMX v2 golden fixture is accepted", async () => {
  assert.equal(fixtureDocument.format, "iroha-native-amx-v2-grouped");
  assert.equal(fixtureDocument.fixture_version, 1);
  assert.equal(
    fixtureDocument.rust_owner,
    "iroha_data_model::block::consensus",
  );

  const diagnostics = await diagnosticsClient(
    clone(fixtureDocument.golden.expected_diagnostics),
  ).getSumeragiDiagnosticsTyped();
  const group = diagnostics.lane_settlement_commitments[0];
  assert.equal(group.native_amx_receipts.length, 2);
  assert.deepEqual(
    group.native_amx_receipts.map((receipt) => receipt.source_id),
    fixtureDocument.golden.ordered_source_ids,
  );
  for (const receipt of group.native_amx_receipts) {
    assert.equal(receipt.legs.length, 2);
    assert.equal(receipt.lane_block_view, 9);
    for (const leg of receipt.legs) {
      assert.deepEqual(leg.prepare_qc.body.phase, {
        phase: "prepare",
        detail: null,
      });
      assert.deepEqual(leg.commit_qc.body.phase, {
        phase: "commit",
        detail: null,
      });
      assert.equal(leg.prepare_qc.body.round.view, 6);
      assert.equal(leg.prepare_qc.body.coordinator_lane_block_view, 9);
      assert.equal(leg.prepare_qc.validator_set.length, 4);
      assert.ok(
        leg.prepare_qc.validator_set_pops.every((pop) => pop.length === 96),
      );
      assert.equal(leg.prepare_qc.bls_aggregate_signature.length, 96);
      assert.equal(leg.requires_mixed_role_anchor_validation, false);
      assert.deepEqual(
        leg.participant_settlement.receipts.map((entry) => entry.source_id),
        fixtureDocument.golden.ordered_source_ids,
      );
    }
  }
  assert.equal(
    diagnostics.native_amx_participant_applications[0].source_count,
    2,
  );
});

test("grouped Native AMX v2 exposes mixed-role anchor deferral", async () => {
  const diagnosticsPayload = clone(fixtureDocument.golden.expected_diagnostics);
  const descriptor = diagnosticsPayload
    .lane_settlement_commitments[0]
    .native_amx_receipts[0]
    .legs[1]
    .participant_proposal
    .descriptor;
  descriptor.accepted_candidate_indices = [descriptor.accepted_candidate_indices[1]];
  descriptor.accepted_transaction_hashes = [descriptor.accepted_transaction_hashes[1]];

  const diagnostics = await diagnosticsClient(
    diagnosticsPayload,
  ).getSumeragiDiagnosticsTyped();
  assert.equal(
    diagnostics.lane_settlement_commitments[0]
      .native_amx_receipts[0]
      .legs[1]
      .requires_mixed_role_anchor_validation,
    true,
  );
});

test("grouped Native AMX v2 requires the exact ordered outer source group", async () => {
  const diagnosticsPayload = clone(fixtureDocument.golden.expected_diagnostics);
  diagnosticsPayload.lane_settlement_commitments[0].native_amx_receipts.pop();

  await assert.rejects(
    () => diagnosticsClient(diagnosticsPayload).getSumeragiDiagnosticsTyped(),
    /exact ordered source group/,
  );
});

for (const control of fixtureDocument.negative_controls) {
  test(`Rust-owned grouped Native AMX v2 rejects ${control.id}`, async () => {
    assert.equal(control.expectation, "reject");
    const document = clone(fixtureDocument);
    for (const mutation of control.mutations) {
      applyMutation(document, mutation);
    }
    const diagnostics = clone(document.golden.expected_diagnostics);
    diagnostics.lane_settlement_commitments = [
      document.golden.receipt_group,
    ];
    await assert.rejects(() =>
      diagnosticsClient(diagnostics).getSumeragiDiagnosticsTyped(),
    );
  });
}
