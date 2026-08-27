import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { pathToFileURL } from "node:url";

import {
  OperatorSigningContext as SourceOperatorSigningContext,
  ToriiClient as SourceToriiClient,
} from "../src/toriiClient.js";
import { NetworkId as SourceNetworkId } from "../src/networkId.js";
import {
  __sumeragiNativeAmxTestHelpers as sourceNativeAmxTestHelpers,
} from "../src/sumeragiTyped.js";
import {
  verifyBlockMerkleProof as sourceVerifyBlockMerkleProof,
} from "../src/norito.js";

const distToriiClientPath =
  process.env.IROHA_JS_NATIVE_AMX_V2_PARITY_DIST_TORII_CLIENT;
const distToriiClientUrl = distToriiClientPath
  ? pathToFileURL(distToriiClientPath)
  : new URL("../dist/toriiClient.js", import.meta.url);
const {
  OperatorSigningContext: DistOperatorSigningContext,
  ToriiClient: DistToriiClient,
} = await import(distToriiClientUrl);
const { NetworkId: DistNetworkId } = await import(
  new URL("./networkId.js", distToriiClientUrl)
);
const {
  __sumeragiNativeAmxTestHelpers: distNativeAmxTestHelpers,
} = await import(new URL("./sumeragiTyped.js", distToriiClientUrl));
const {
  verifyBlockMerkleProof: distVerifyBlockMerkleProof,
} = await import(new URL("./norito.js", distToriiClientUrl));
const merkleProofImplementations = [
  ["source", sourceVerifyBlockMerkleProof],
  ["dist", distVerifyBlockMerkleProof],
];

const fixtureUrl = new URL(
  "../../../fixtures/sumeragi_v2/native_amx_v2_grouped.json",
  import.meta.url,
);
const fixtureDocument = JSON.parse(readFileSync(fixtureUrl, "utf8"));

function clone(value) {
  return JSON.parse(JSON.stringify(value));
}

function resealNativeAmxLeg(leg, helpers) {
  const descriptor = leg.participant_proposal.descriptor;
  descriptor.validator_set_hash = helpers.computeValidatorSetHash(
    descriptor.validator_set,
  );
  descriptor.descriptor_hash = helpers.computeDescriptorHash(descriptor);
  leg.participant_proposal.proposal_hash =
    helpers.computeProposalHash(descriptor);
  leg.participant_settlement_hash =
    helpers.computeParticipantSettlementHash(leg.participant_settlement);
  for (const qc of [leg.prepare_qc, leg.commit_qc]) {
    qc.validator_set_hash = descriptor.validator_set_hash;
    qc.body.participant_validator_set_hash = descriptor.validator_set_hash;
    qc.body.participant_proposal_hash =
      leg.participant_proposal.proposal_hash;
    qc.body.participant_settlement_commitment =
      leg.participant_settlement_hash;
  }
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

function validateApplicationEvidence(document) {
  const { golden } = document;
  const group = golden.receipt_group;
  const evidence = golden.application_evidence;
  const execution = evidence.execution_commitment;
  const artifacts = evidence.manifest_artifacts;
  assert.equal(execution.native_amx_application_manifest_version, 1);
  assert.equal(execution.lane_finality_manifest, null);
  assert.equal(
    typeof execution.merge_carrier,
    "object",
    "merge carrier must be an object",
  );
  assert.notEqual(execution.merge_carrier, null);
  assert.equal(Array.isArray(execution.merge_carrier), false);
  assert.deepEqual(
    Object.keys(execution.merge_carrier).sort(),
    ["entry_hash", "version"],
  );
  assert.equal(execution.merge_carrier.version, 1);
  assert.match(
    execution.merge_carrier.entry_hash,
    /^hash:[0-9A-F]{64}#[0-9A-F]{4}$/u,
  );
  assert.equal(execution.native_amx_application_manifest_count, artifacts.length);
  assert.equal(artifacts.length, 1);

  const artifact = artifacts[0];
  const { leaf, proof } = artifact;
  assert.equal(artifact.version, 1);
  assert.equal(leaf.version, 1);
  assert.equal(artifact.leaf_index, 0);
  assert.equal(proof.leaf_index, 0);
  assert.deepEqual(proof.audit_path, []);
  assert.equal(
    artifact.manifest_leaf_count,
    execution.native_amx_application_manifest_count,
  );
  assert.equal(
    artifact.manifest_root,
    execution.native_amx_application_manifest_root,
  );
  for (const [implementation, verifyMerkleProof] of merkleProofImplementations) {
    assert.equal(
      verifyMerkleProof(artifact.leaf_hash, proof, {
        root: artifact.manifest_root,
        leaf_count: artifact.manifest_leaf_count,
      }),
      true,
      `${implementation} must authenticate the singleton manifest commitment`,
    );
  }
  assert.equal(
    leaf.executed_block_wire_hash,
    execution.executed_block_wire_hash,
  );
  assert.equal(Number.isSafeInteger(execution.executed_block_wire_len), true);
  assert.equal(execution.executed_block_wire_len, 49);
  assert.equal(leaf.predecessor_height + 1, leaf.participant_height);
  assert.deepEqual(evidence.active_lane_incarnations, [{
    lane_id: leaf.lane_id,
    dataspace_id: leaf.dataspace_id,
    lane_incarnation: leaf.lane_incarnation,
  }]);
  assert.notDeepEqual(
    [leaf.lane_id, leaf.dataspace_id],
    [group.lane_id, group.dataspace_id],
  );

  const members = leaf.members;
  assert.ok(members.length >= 1 && members.length <= 4096);
  assert.deepEqual(
    members.map((member) => member.source_id),
    group.native_amx_receipts.map((receipt) => receipt.source_id),
  );
  assert.equal(
    new Set(members.map((member) => member.source_id)).size,
    members.length,
  );
  assert.ok(
    members.every(
      (member, index) =>
        index === 0 ||
        members[index - 1].entrypoint_index < member.entrypoint_index,
    ),
  );
  const carrierEntrypoints = new Set(evidence.carrier_entrypoint_hashes);
  for (const [index, receipt] of group.native_amx_receipts.entries()) {
    const member = members[index];
    const leg = receipt.legs.find(
      (candidate) =>
        candidate.lane_id === leaf.lane_id &&
        candidate.dataspace_id === leaf.dataspace_id,
    );
    assert.ok(leg);
    const descriptor = leg.participant_proposal.descriptor;
    assert.equal(descriptor.lane_incarnation, leaf.lane_incarnation);
    assert.equal(descriptor.lane_block_height, leaf.participant_height);
    assert.equal(descriptor.lane_block_view, leaf.participant_view);
    assert.equal(
      descriptor.previous_lane_block_height,
      leaf.predecessor_height,
    );
    assert.equal(
      descriptor.previous_lane_block_descriptor_hash ?? null,
      leaf.predecessor_descriptor_hash ?? null,
    );
    assert.equal(descriptor.descriptor_hash, leaf.descriptor_hash);
    assert.equal(leg.participant_proposal.proposal_hash, leaf.proposal_hash);
    assert.equal(leg.participant_settlement_hash, leaf.settlement_hash);
    assert.equal(leg.prepare_qc.body.source_id, member.source_id);
    assert.equal(
      leg.prepare_qc.body.tx_entrypoint_hash,
      member.entrypoint_hash,
    );
    assert.ok(
      descriptor.accepted_candidate_indices.includes(member.entrypoint_index),
    );
    assert.ok(
      descriptor.accepted_transaction_hashes.every((hash) =>
        carrierEntrypoints.has(hash)),
    );
  }

  const row = golden.expected_diagnostics.native_amx_participant_applications[0];
  for (const field of [
    "lane_id",
    "dataspace_id",
    "lane_incarnation",
    "participant_height",
    "participant_view",
    "predecessor_height",
    "predecessor_descriptor_hash",
    "descriptor_hash",
    "proposal_hash",
    "settlement_hash",
    "application_block_height",
    "application_block_hash",
  ]) {
    assert.equal(row[field] ?? null, leaf[field] ?? null, field);
  }
  assert.equal(row.source_count, members.length);
}

const clientImplementations = [
  ["source", SourceToriiClient],
  ["dist", DistToriiClient],
];

const operatorSigningContexts = new Map([
  [
    SourceToriiClient,
    new SourceOperatorSigningContext(
      SourceNetworkId.fromBytes(Buffer.alloc(32, 0xa5)),
      {
        publicKey:
          "ed012066BE7E332C7A453332BD9D0A7F7DB055F5C5EF1A06ADA66D98B39FB6810C473A",
        sign: () => Buffer.of(1),
      },
    ),
  ],
  [
    DistToriiClient,
    new DistOperatorSigningContext(
      DistNetworkId.fromBytes(Buffer.alloc(32, 0xa5)),
      {
        publicKey:
          "ed012066BE7E332C7A453332BD9D0A7F7DB055F5C5EF1A06ADA66D98B39FB6810C473A",
        sign: () => Buffer.of(1),
      },
    ),
  ],
]);

function diagnosticsClient(payload, Client = SourceToriiClient) {
  return new Client("https://fixture.invalid", {
    operatorSigningContext: operatorSigningContexts.get(Client),
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
  const negativeControlIds = new Set(
    fixtureDocument.negative_controls.map((control) => control.id),
  );
  assert.ok(negativeControlIds.has("coherent_duplicate_validator_set"));
  assert.ok(negativeControlIds.has("coherent_over_quorum_requirement"));
  assert.ok(
    negativeControlIds.has(
      "execution_commitment_merge_carrier_wrong_version",
    ),
  );
  assert.ok(
    negativeControlIds.has(
      "execution_commitment_missing_merge_carrier_field",
    ),
  );
  const declarations = readFileSync(
    new URL("../index.d.ts", import.meta.url),
    "utf8",
  );
  const legDeclaration = declarations.match(
    /export interface ToriiNativeAmxLeg \{([\s\S]*?)\n\}/u,
  );
  assert.ok(legDeclaration, "missing ToriiNativeAmxLeg declaration");
  assert.match(
    legDeclaration[1],
    /readonly requires_mixed_role_anchor_validation: boolean;/u,
  );
  const expectedSettlementHashes = new Map([
    [
      "7/11",
      "hash:C6B18DBE6BEC468DB021B79604233F3CB9E2D6CDF3384C491CE7A6DA89747825#9D72",
    ],
    [
      "8/12",
      "hash:40C7FCA7AA143B323B473A9958B96F49896C03C3547B83DD340FAE2FC1A85D29#B452",
    ],
  ]);
  const vectorLegs =
    fixtureDocument.golden.receipt_group.native_amx_receipts[0].legs;
  for (const leg of vectorLegs) {
    const expected = expectedSettlementHashes.get(
      `${leg.lane_id}/${leg.dataspace_id}`,
    );
    assert.ok(expected);
    assert.equal(leg.participant_settlement_hash, expected);
    assert.equal(
      sourceNativeAmxTestHelpers.computeParticipantSettlementHash(
        leg.participant_settlement,
      ),
      expected,
    );
    assert.equal(
      distNativeAmxTestHelpers.computeParticipantSettlementHash(
        leg.participant_settlement,
      ),
      expected,
    );
  }

  for (const [implementation, Client] of clientImplementations) {
    const diagnostics = await diagnosticsClient(
      clone(fixtureDocument.golden.expected_diagnostics),
      Client,
    ).getSumeragiDiagnosticsTyped();
    const group = diagnostics.lane_settlement_commitments[0];
    assert.equal(group.native_amx_receipts.length, 2, implementation);
    assert.deepEqual(
      group.native_amx_receipts.map((receipt) => receipt.source_id),
      fixtureDocument.golden.ordered_source_ids,
      implementation,
    );
    for (const receipt of group.native_amx_receipts) {
      assert.equal(receipt.legs.length, 2, implementation);
      assert.equal(receipt.lane_block_view, 9, implementation);
      for (const leg of receipt.legs) {
        assert.deepEqual(leg.prepare_qc.body.phase, {
          phase: "prepare",
          detail: null,
        }, implementation);
        assert.deepEqual(leg.commit_qc.body.phase, {
          phase: "commit",
          detail: null,
        }, implementation);
        assert.equal(leg.prepare_qc.body.round.view, 6, implementation);
        assert.equal(
          leg.prepare_qc.body.coordinator_lane_block_view,
          9,
          implementation,
        );
        assert.equal(leg.prepare_qc.validator_set.length, 4, implementation);
        assert.ok(
          leg.prepare_qc.validator_set_pops.every((pop) => pop.length === 96),
          implementation,
        );
        assert.equal(
          leg.prepare_qc.bls_aggregate_signature.length,
          96,
          implementation,
        );
        assert.equal(
          leg.requires_mixed_role_anchor_validation,
          false,
          implementation,
        );
        assert.equal(
          leg.participant_proposal.payload_block_hint,
          null,
          implementation,
        );
        assert.deepEqual(
          leg.participant_settlement.receipts.map((entry) => entry.source_id),
          fixtureDocument.golden.ordered_source_ids,
          implementation,
        );
      }
    }
    assert.equal(
      diagnostics.native_amx_participant_applications[0].source_count,
      2,
      implementation,
    );
  }
  validateApplicationEvidence(fixtureDocument);
});

test("grouped Native AMX v2 exposes mixed-role anchor deferral", async () => {
  for (const [implementation, Client] of clientImplementations) {
    const diagnosticsPayload = clone(
      fixtureDocument.golden.expected_diagnostics,
    );
    const leg = diagnosticsPayload
      .lane_settlement_commitments[0]
      .native_amx_receipts[0]
      .legs[1];
    const descriptor = leg.participant_proposal.descriptor;
    descriptor.accepted_candidate_indices = [
      descriptor.accepted_candidate_indices[1],
    ];
    descriptor.accepted_transaction_hashes = [
      descriptor.accepted_transaction_hashes[1],
    ];
    resealNativeAmxLeg(
      leg,
      implementation === "source"
        ? sourceNativeAmxTestHelpers
        : distNativeAmxTestHelpers,
    );

    const diagnostics = await diagnosticsClient(
      diagnosticsPayload,
      Client,
    ).getSumeragiDiagnosticsTyped();
    assert.equal(
      diagnostics.lane_settlement_commitments[0]
        .native_amx_receipts[0]
        .legs[1]
        .requires_mixed_role_anchor_validation,
      true,
      implementation,
    );
  }
});

test("grouped Native AMX v2 rejects noncanonical validator PeerIds", async () => {
  const invalidValidators = [
    " not-a-canonical-bls-peer-id",
    `ed0120${"AA".repeat(32)}`,
    `ea013080${"00".repeat(47)}`,
    `EA0130${"AA".repeat(48)}`,
  ];
  for (const validator of invalidValidators) {
    for (const [implementation, Client] of clientImplementations) {
      const diagnostics = clone(
        fixtureDocument.golden.expected_diagnostics,
      );
      diagnostics.lane_settlement_commitments[0]
        .native_amx_receipts[0]
        .legs[0]
        .prepare_qc
        .validator_set[0] = validator;
      await assert.rejects(
        () => diagnosticsClient(
          diagnostics,
          Client,
        ).getSumeragiDiagnosticsTyped(),
        implementation,
      );
    }
  }
});

test("grouped Native AMX v2 requires the exact ordered outer source group", async () => {
  const diagnosticsPayload = clone(fixtureDocument.golden.expected_diagnostics);
  diagnosticsPayload.lane_settlement_commitments[0].native_amx_receipts.pop();

  await assert.rejects(
    () => diagnosticsClient(diagnosticsPayload).getSumeragiDiagnosticsTyped(),
    /exact ordered source group/,
  );
});

test("Rust-owned grouped Native AMX v2 corpus includes required controls", () => {
  const identifiers = new Set(
    fixtureDocument.negative_controls.map((control) => control.id),
  );
  const required = [
    "coherent_forged_validator_set_hash",
    "coherent_stale_descriptor_hash",
    "coherent_stale_proposal_hash",
    "coherent_stale_settlement_hash",
    "manifest_leaf_hash_tampering",
    "non_canonical_validator_peer_id",
  ];
  assert.deepEqual(
    required.filter((identifier) => !identifiers.has(identifier)),
    [],
  );
});

for (const control of fixtureDocument.negative_controls) {
  test(`Rust-owned grouped Native AMX v2 rejects ${control.id}`, async () => {
    assert.equal(control.expectation, "reject");
    const document = clone(fixtureDocument);
    for (const mutation of control.mutations) {
      applyMutation(document, mutation);
    }
    if (control.validator === "application_evidence") {
      assert.throws(() => validateApplicationEvidence(document));
      return;
    }
    assert.equal(control.validator, "receipt_group");
    const diagnostics = clone(document.golden.expected_diagnostics);
    diagnostics.lane_settlement_commitments = [
      document.golden.receipt_group,
    ];
    for (const [implementation, Client] of clientImplementations) {
      await assert.rejects(
        () => diagnosticsClient(diagnostics, Client).getSumeragiDiagnosticsTyped(),
        implementation,
      );
    }
  });
}
