"""Exact source bindings for Native AMX merge-manifest projection."""

from __future__ import annotations

import re
from pathlib import Path


NATIVE_MERGE_MANIFEST_CONTRACT_RELATIVE = Path(
    "scripts/formal/sumeragi_v2_multilane_native_merge_manifest_contract.py"
)
NATIVE_MERGE_MANIFEST_TEST_RELATIVE = Path(
    "pytests/scripts/sumeragi_v2_multilane_native_merge_manifest_test.py"
)
NATIVE_MERGE_MANIFEST_CORRIDOR_RELATIVE = Path(
    "crates/iroha_core/src/sumeragi/tests/"
    "v2_apply_unsealed_01c_historical_recovery.rs"
)
NATIVE_MERGE_MANIFEST_FIXTURE_RELATIVE = Path(
    "crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs"
)
NATIVE_PARTICIPANT_APPLICATION_ROLE_RELATIVE = "crates/iroha_core/src/native_amx.rs"
NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_RELATIVE = (
    "crates/iroha_core/src/native_amx/participant_application_role_tests.rs"
)
NATIVE_PARTICIPANT_APPLICATION_ROLE_BINDINGS = (
    (
        NATIVE_PARTICIPANT_APPLICATION_ROLE_RELATIVE,
        "fn",
        "native_amx_participant_application_role",
        (
            "let prepare = &leg.prepare_qc.body;",
            "let commit = &leg.commit_qc.body;",
            ".participant_settlement",
            ".computed_hash()",
            'map_err(|_| "Native AMX participant settlement cannot be hashed")?',
            "settlement_hash != leg.participant_settlement_hash",
            "Native AMX participant leg identity is internally inconsistent",
            "Native AMX same-route leg differs from the coordinator identity",
            "NativeAmxParticipantApplicationRole::SeparateParticipant",
            "NativeAmxParticipantApplicationRole::Coordinator",
        ),
    ),
    (
        NATIVE_PARTICIPANT_APPLICATION_ROLE_RELATIVE,
        "fn",
        "native_amx_receipt_requires_separate_participant_application_for",
        (
            "let mut matches = false;",
            "for leg in &receipt.legs",
            "native_amx_participant_application_role(receipt, leg)?",
            "NativeAmxParticipantApplicationRole::SeparateParticipant",
            "matches |= descriptor.lane_id == lane_id",
            "descriptor.dataspace_id == dataspace_id",
            "descriptor.lane_incarnation == lane_incarnation",
            "Ok(matches)",
        ),
    ),
)
NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_BINDINGS = tuple(
    (NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_RELATIVE, "fn", symbol, tokens)
    for symbol, tokens in (
        (
            "participant_application_role_classifies_exact_routes_and_incarnations",
            (
                "fixture_receipt()",
                "NativeAmxParticipantApplicationRole::Coordinator",
                "NativeAmxParticipantApplicationRole::SeparateParticipant",
                "native_amx_participant_application_role(&receipt, leg)",
                "Ok(role)",
                "Ok(role == NativeAmxParticipantApplicationRole::SeparateParticipant)",
                '"unknown lane"',
                '"different dataspace"',
                '"stale incarnation"',
                "Ok(false)",
            ),
        ),
        (
            "participant_application_role_keeps_each_route_coordinate_distinct",
            (
                "(receipt.lane_id, receipt.legs[index].dataspace_id)",
                "(receipt.legs[index].lane_id, receipt.dataspace_id)",
                "rebind_participant_identity(leg)",
                "Ok(NativeAmxParticipantApplicationRole::SeparateParticipant)",
                "native_amx_receipt_requires_separate_participant_application_for(",
                "Ok(true)",
            ),
        ),
        (
            "participant_application_role_rejects_independent_prepare_and_commit_identity_drift",
            (
                "for index in 0..receipt.legs.len()",
                "for phase in [NativeAmxPhase::Prepare, NativeAmxPhase::Commit]",
                "for &(label, mutate) in BODY_IDENTITY_MUTATIONS",
                "NativeAmxPhase::Prepare => &mut leg.prepare_qc.body",
                "NativeAmxPhase::Commit => &mut leg.commit_qc.body",
                "mutate(body)",
                "Err(INCONSISTENT_IDENTITY)",
            ),
        ),
        (
            "participant_application_role_rejects_coherent_same_route_coordinator_drift",
            (
                "NativeAmxParticipantApplicationRole::Coordinator",
                "descriptor.lane_incarnation =",
                "descriptor.lane_block_height += 1",
                "descriptor.lane_block_view += 1",
                "descriptor.subject_hash =",
                "mutate(&mut leg.participant_proposal.descriptor)",
                "rebind_participant_identity(leg)",
                "Err(SAME_ROUTE_DRIFT)",
            ),
        ),
        (
            "participant_application_role_rejects_settlement_identity_and_content_tampering",
            (
                "fields.lane_id =",
                "fields.dataspace_id =",
                "fields.lane_incarnation =",
                "fields.participant_lane_block_height += 1",
                "fields.source_ids[0] =",
                "fields.authority_context_height += 1",
                "leg.participant_settlement_hash =",
                "for index in 0..receipt.legs.len()",
                "mutate(&mut altered.legs[index])",
                "Err(INCONSISTENT_IDENTITY)",
            ),
        ),
        (
            "participant_application_lookup_validates_later_legs_after_an_exact_match",
            (
                "receipt.legs.swap(0, matching_index)",
                "let descriptor = &receipt.legs[0].participant_proposal.descriptor",
                "&receipt, route.0, route.1, route.2,",
                "Ok(true)",
                "receipt.legs[1]",
                ".participant_previous_block_height += 1",
                "for lane_id in [route.0, LaneId::new(90)]",
                "&receipt, lane_id, route.1, route.2,",
                "Err(INCONSISTENT_IDENTITY)",
            ),
        ),
    )
)

# Both phase bodies must independently bind the participant and coordinator.
# Keep the whole rejecting disjunction, so a missing comparison or changed
# boolean operator cannot be hidden by another mention of the same field.
NATIVE_PARTICIPANT_APPLICATION_IDENTITY_COMPARISONS = (
    "descriptor.lane_id != leg.lane_id",
    "descriptor.dataspace_id != leg.dataspace_id",
    "prepare.participant_lane_id != leg.lane_id",
    "commit.participant_lane_id != leg.lane_id",
    "prepare.participant_dataspace_id != leg.dataspace_id",
    "commit.participant_dataspace_id != leg.dataspace_id",
    "descriptor.lane_incarnation != prepare.participant_lane_incarnation",
    "descriptor.lane_incarnation != commit.participant_lane_incarnation",
    "descriptor.proposal_height != prepare.authority_context_height",
    "descriptor.proposal_height != commit.authority_context_height",
    "descriptor.previous_lane_block_height != prepare.participant_previous_block_height",
    "descriptor.previous_lane_block_height != commit.participant_previous_block_height",
    "descriptor.previous_lane_block_descriptor_hash != prepare.participant_previous_block_descriptor_hash",
    "descriptor.previous_lane_block_descriptor_hash != commit.participant_previous_block_descriptor_hash",
    "descriptor.lane_block_height != prepare.participant_lane_block_height",
    "descriptor.lane_block_height != commit.participant_lane_block_height",
    "descriptor.lane_block_view != prepare.participant_lane_block_view",
    "descriptor.lane_block_view != commit.participant_lane_block_view",
    "leg.participant_proposal.proposal_hash != prepare.participant_proposal_hash",
    "leg.participant_proposal.proposal_hash != commit.participant_proposal_hash",
    "settlement_hash != leg.participant_settlement_hash",
    "leg.participant_settlement.lane_id() != descriptor.lane_id",
    "leg.participant_settlement.dataspace_id() != descriptor.dataspace_id",
    "leg.participant_settlement.lane_incarnation() != descriptor.lane_incarnation",
    "leg.participant_settlement.participant_lane_block_height() != descriptor.lane_block_height",
    "leg.participant_settlement.authority_context_height() != descriptor.proposal_height",
    "Hash::from(settlement_hash) != prepare.participant_settlement_commitment",
    "Hash::from(settlement_hash) != commit.participant_settlement_commitment",
    "prepare.coordinator_lane_id != receipt.lane_id",
    "commit.coordinator_lane_id != receipt.lane_id",
    "prepare.coordinator_dataspace_id != receipt.dataspace_id",
    "commit.coordinator_dataspace_id != receipt.dataspace_id",
    "prepare.coordinator_lane_incarnation != receipt.lane_incarnation",
    "commit.coordinator_lane_incarnation != receipt.lane_incarnation",
    "prepare.authority_context_height != receipt.authority_context_height",
    "commit.authority_context_height != receipt.authority_context_height",
    "prepare.planned_coordinator_block_height != receipt.lane_block_height",
    "commit.planned_coordinator_block_height != receipt.lane_block_height",
    "prepare.coordinator_lane_block_view != receipt.lane_block_view",
    "commit.coordinator_lane_block_view != receipt.lane_block_view",
    "prepare.coordinator_proposal_hash != receipt.coordinator_proposal_hash",
    "commit.coordinator_proposal_hash != receipt.coordinator_proposal_hash",
)

NATIVE_TYPED_SETTLEMENT_SOURCE_BINDINGS = (('crates/iroha_data_model/src/block/consensus.rs',
  'struct',
  'NativeAmxParticipantSettlement',
  ('lane_id: LaneId',
   'dataspace_id: DataSpaceId',
   'lane_incarnation: Hash',
   'participant_lane_block_height: u64',
   'authority_context_height: u64',
   'previous_native_settlement_hash: Option<HashOf<NativeAmxParticipantSettlement>>',
   'source_ids: Vec<[u8; Hash::LENGTH]>')),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::lane_id',
  ('self.lane_id',)),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::dataspace_id',
  ('self.dataspace_id',)),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::lane_incarnation',
  ('self.lane_incarnation',)),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::participant_lane_block_height',
  ('self.participant_lane_block_height',)),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::authority_context_height',
  ('self.authority_context_height',)),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::previous_native_settlement_hash',
  ('self.previous_native_settlement_hash',)),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::source_ids',
  ('self.source_ids',)),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::computed_hash',
  ('b"iroha:native-amx:participant-settlement:v1"',
   'let bytes = norito::encode_canonical(self)?;',
   'u64::try_from(DOMAIN.len())',
   '.to_le_bytes();',
   '&domain_len,',
   'DOMAIN,',
   '&bytes,')),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::try_new',
  ('participant_lane_block_height == 0',
   'authority_context_height == 0',
   'participant_lane_block_height == 1 && previous_native_settlement_hash.is_some()',
   'source_ids.is_empty() || source_ids.len() > NATIVE_AMX_GROUP_SOURCES_MAX',
   '!native_amx_nonzero(source)',
   'collect::<std::collections::BTreeSet<_>>()',
   '!= source_ids.len()')),
 ('crates/iroha_core/src/native_amx/participant_application_role_tests.rs',
  'fn',
  'mutate_participant_settlement',
  ('mutate(&mut fields);',
   'leg.participant_settlement = NativeAmxParticipantSettlement::try_new(',
   'fields.participant_lane_block_height,',
   'fields.authority_context_height,',
   'settlement.previous_native_settlement_hash(),',
   'fields.source_ids,')),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options',
  ('Self::new_with_options_and_retention(',
   'include_lane_payload,',
   'include_projection_policies,',
   'include_lane_lifecycle,',
   'include_native_lane,',
   'iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,')),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options_and_retention',
  ('(1_u8..=4)',
   'Algorithm::BlsNormal',
   'if include_lane_lifecycle {',
   'locked_lane_work_test_kura(blocks_in_memory)',
   'State::new_with_chain_and_network_id_for_testing(',
   'context.network_id,',
   'install_fixture_validator_authority(&state, &context, &validator_set_pops);',
   'if include_native_lane {',
   'install_fixture_native_lane(&mut state, &mut context);')),
 ('crates/iroha_core/src/kura/native_amx_participant_application_artifacts.rs',
  'struct',
  'NativeAmxParticipantReceiptLatestIndexV2',
  ('version: u8',
   'lane_id: LaneId',
   'dataspace_id: DataSpaceId',
   'lane_incarnation: Hash',
   'lane_block_height: u64',
   'participant_proposal_hash: Hash',
   'participant_settlement_hash:\n'
   '        HashOf<iroha_data_model::block::consensus::NativeAmxParticipantSettlement>',
   'application_block_height: u64',
   'application_block_hash: HashOf<BlockHeader>',
   'executed_block_wire_hash: Hash',
   'finality_artifact_hash: HashOf<V2FinalityArtifact>',
   'manifest_artifact_hash: HashOf<NativeAmxParticipantApplicationManifestArtifactV1>')))

NATIVE_TYPED_SETTLEMENT_NORMALIZED_RELATIONS = (('crates/iroha_data_model/src/block/consensus.rs',
  'struct',
  'NativeAmxParticipantSettlement',
  'pub struct NativeAmxParticipantSettlement {\n'
  '    lane_id: LaneId,\n'
  '    dataspace_id: DataSpaceId,\n'
  '    lane_incarnation: Hash,\n'
  '    participant_lane_block_height: u64,\n'
  '    authority_context_height: u64,\n'
  '    previous_native_settlement_hash: Option<HashOf<NativeAmxParticipantSettlement>>,\n'
  '    source_ids: Vec<[u8; Hash::LENGTH]>,\n'
  '}'),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::lane_id',
  '    pub const fn lane_id(&self) -> LaneId {\n        self.lane_id\n    }'),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::dataspace_id',
  '    pub const fn dataspace_id(&self) -> DataSpaceId {\n        self.dataspace_id\n    }'),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::lane_incarnation',
  '    pub const fn lane_incarnation(&self) -> Hash {\n        self.lane_incarnation\n    }'),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::participant_lane_block_height',
  '    pub const fn participant_lane_block_height(&self) -> u64 {\n'
  '        self.participant_lane_block_height\n'
  '    }'),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::authority_context_height',
  '    pub const fn authority_context_height(&self) -> u64 {\n'
  '        self.authority_context_height\n'
  '    }'),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::previous_native_settlement_hash',
  '    pub const fn previous_native_settlement_hash(&self) -> Option<HashOf<Self>> {\n'
  '        self.previous_native_settlement_hash\n'
  '    }'),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::source_ids',
  '    pub fn source_ids(&self) -> &[[u8; Hash::LENGTH]] {\n        &self.source_ids\n    }'),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::computed_hash',
  '    pub fn computed_hash(&self) -> Result<HashOf<Self>, norito::Error> {\n'
  '        const DOMAIN: &[u8] = b"iroha:native-amx:participant-settlement:v1";\n'
  '        let bytes = norito::encode_canonical(self)?;\n'
  '        let domain_len = u64::try_from(DOMAIN.len())\n'
  '            .expect("protocol-defined hash domain length fits u64")\n'
  '            .to_le_bytes();\n'
  '        Ok(HashOf::from_untyped_unchecked(Hash::new_from_chunks(&[\n'
  '            &domain_len,\n'
  '            DOMAIN,\n'
  '            &bytes,\n'
  '        ])))\n'
  '    }'),
 ('crates/iroha_data_model/src/block/consensus.rs',
  'method',
  'NativeAmxParticipantSettlement::try_new',
  '    pub fn try_new(\n'
  '        lane_id: LaneId,\n'
  '        dataspace_id: DataSpaceId,\n'
  '        lane_incarnation: Hash,\n'
  '        participant_lane_block_height: u64,\n'
  '        authority_context_height: u64,\n'
  '        previous_native_settlement_hash: Option<HashOf<Self>>,\n'
  '        source_ids: Vec<[u8; Hash::LENGTH]>,\n'
  "    ) -> Result<Self, &'static str> {\n"
  '        if lane_incarnation == Hash::prehashed([0; Hash::LENGTH])\n'
  '            || participant_lane_block_height == 0\n'
  '            || authority_context_height == 0\n'
  '        {\n'
  '            return Err("Native AMX participant settlement authority must be nonzero");\n'
  '        }\n'
  '        if (participant_lane_block_height == 1 && '
  'previous_native_settlement_hash.is_some())\n'
  '            || previous_native_settlement_hash\n'
  '                .is_some_and(|hash| Hash::from(hash) == Hash::prehashed([0; '
  'Hash::LENGTH]))\n'
  '        {\n'
  '            return Err("Native AMX previous settlement link is invalid");\n'
  '        }\n'
  '        if source_ids.is_empty() || source_ids.len() > NATIVE_AMX_GROUP_SOURCES_MAX {\n'
  '            return Err("Native AMX participant source group is out of bounds");\n'
  '        }\n'
  '        if source_ids.iter().any(|source| !native_amx_nonzero(source)) {\n'
  '            return Err("Native AMX participant sources must be nonzero");\n'
  '        }\n'
  '        if source_ids\n'
  '            .iter()\n'
  '            .copied()\n'
  '            .collect::<std::collections::BTreeSet<_>>()\n'
  '            .len()\n'
  '            != source_ids.len()\n'
  '        {\n'
  '            return Err("Native AMX participant source group must be unique");\n'
  '        }\n'
  '        Ok(Self {\n'
  '            lane_id,\n'
  '            dataspace_id,\n'
  '            lane_incarnation,\n'
  '            participant_lane_block_height,\n'
  '            authority_context_height,\n'
  '            previous_native_settlement_hash,\n'
  '            source_ids,\n'
  '        })\n'
  '    }'),
 ('crates/iroha_core/src/native_amx/participant_application_role_tests.rs',
  'fn',
  'participant_application_role_rejects_settlement_identity_and_content_tampering',
  '    let mutations: &[Mutation] = &[\n'
  '        ("lane", |leg| {\n'
  '            mutate_participant_settlement(leg, |fields| fields.lane_id = LaneId::new(90))\n'
  '        }),\n'
  '        ("dataspace", |leg| {\n'
  '            mutate_participant_settlement(leg, |fields| fields.dataspace_id = '
  'DataSpaceId::new(90))\n'
  '        }),\n'
  '        ("incarnation", |leg| {\n'
  '            mutate_participant_settlement(leg, |fields| {\n'
  '                fields.lane_incarnation = Hash::new(b"settlement-incarnation-drift")\n'
  '            })\n'
  '        }),\n'
  '        ("height", |leg| {\n'
  '            mutate_participant_settlement(leg, |fields| '
  'fields.participant_lane_block_height += 1)\n'
  '        }),\n'
  '        ("source", |leg| {\n'
  '            mutate_participant_settlement(leg, |fields| fields.source_ids[0] = [0x11; '
  'Hash::LENGTH])\n'
  '        }),\n'
  '        ("authority height", |leg| {\n'
  '            mutate_participant_settlement(leg, |fields| fields.authority_context_height += '
  '1)\n'
  '        }),\n'
  '        ("advertised hash", |leg| {\n'
  '            leg.participant_settlement_hash =\n'
  '                HashOf::from_untyped_unchecked(Hash::new(b"settlement-hash-drift"))\n'
  '        }),\n'
  '    ];\n'),
 ('crates/iroha_core/src/native_amx/participant_application_role_tests.rs',
  'fn',
  'mutate_participant_settlement',
  'fn mutate_participant_settlement(\n'
  '    leg: &mut NativeAmxLegRecordV2,\n'
  '    mutate: impl FnOnce(&mut ParticipantSettlementFields),\n'
  ') {\n'
  '    let settlement = &leg.participant_settlement;\n'
  '    let mut fields = ParticipantSettlementFields {\n'
  '        lane_id: settlement.lane_id(),\n'
  '        dataspace_id: settlement.dataspace_id(),\n'
  '        lane_incarnation: settlement.lane_incarnation(),\n'
  '        participant_lane_block_height: settlement.participant_lane_block_height(),\n'
  '        authority_context_height: settlement.authority_context_height(),\n'
  '        source_ids: settlement.source_ids().to_vec(),\n'
  '    };\n'
  '    mutate(&mut fields);\n'
  '    leg.participant_settlement = NativeAmxParticipantSettlement::try_new(\n'
  '        fields.lane_id,\n'
  '        fields.dataspace_id,\n'
  '        fields.lane_incarnation,\n'
  '        fields.participant_lane_block_height,\n'
  '        fields.authority_context_height,\n'
  '        settlement.previous_native_settlement_hash(),\n'
  '        fields.source_ids,\n'
  '    )\n'
  '    .expect("tampered control still satisfies intrinsic constructor invariants");\n'
  '}'),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options',
  '    fn new_with_options(\n'
  '        include_lane_payload: bool,\n'
  '        include_projection_policies: bool,\n'
  '        include_lane_lifecycle: bool,\n'
  '        include_native_lane: bool,\n'
  '    ) -> Self {\n'
  '        Self::new_with_options_and_retention(\n'
  '            include_lane_payload,\n'
  '            include_projection_policies,\n'
  '            include_lane_lifecycle,\n'
  '            include_native_lane,\n'
  '            iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,\n'
  '        )\n'
  '    }'),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options_and_retention',
  'let kura = if include_lane_lifecycle { '
  'crate::sumeragi::v2_lane_work::tests::locked_lane_work_test_kura(blocks_in_memory) } else { '
  'Kura::blank_kura_for_testing_with_blocks_in_memory(blocks_in_memory) };'),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options_and_retention',
  'install_fixture_validator_authority(&state, &context, &validator_set_pops); if '
  'include_native_lane { install_fixture_native_lane(&mut state, &mut context); }'),
 ('crates/iroha_core/src/kura/native_amx_participant_application_artifacts.rs',
  'struct',
  'NativeAmxParticipantReceiptLatestIndexV2',
  'struct NativeAmxParticipantReceiptLatestIndexV2 {\n'
  '    version: u8,\n'
  '    lane_id: LaneId,\n'
  '    dataspace_id: DataSpaceId,\n'
  '    lane_incarnation: Hash,\n'
  '    lane_block_height: u64,\n'
  '    participant_proposal_hash: Hash,\n'
  '    participant_settlement_hash:\n'
  '        HashOf<iroha_data_model::block::consensus::NativeAmxParticipantSettlement>,\n'
  '    application_block_height: u64,\n'
  '    application_block_hash: HashOf<BlockHeader>,\n'
  '    executed_block_wire_hash: Hash,\n'
  '    finality_artifact_hash: HashOf<V2FinalityArtifact>,\n'
  '    manifest_artifact_hash: HashOf<NativeAmxParticipantApplicationManifestArtifactV1>,\n'
  '}'),
 ('crates/iroha_core/src/sumeragi/exec.rs',
  'fn',
  'from_result_bearing_block_and_merge_entry',
  '                if descriptor.proposal_height != authority_context_height\n'
  '                    || (!source.finality_bound_merge\n'
  '                        && authority_context_height != application_block_height)\n'
  '                    || (source.finality_bound_merge\n'
  '                        && authority_context_height > application_block_height)\n'
  '                    || prepare.source_id != source.receipt.source_id\n'
  '                    || commit.source_id != source.receipt.source_id\n'
  '                    || prepare.tx_entrypoint_hash != source.entrypoint_hash\n'
  '                    || commit.tx_entrypoint_hash != source.entrypoint_hash\n'
  '                    || prepare.participant_proposal_hash != '
  'leg.participant_proposal.proposal_hash\n'
  '                    || commit.participant_proposal_hash != '
  'leg.participant_proposal.proposal_hash\n'
  '                    || prepare.participant_settlement_commitment\n'
  '                        != Hash::from(leg.participant_settlement_hash)\n'
  '                    || commit.participant_settlement_commitment\n'
  '                        != Hash::from(leg.participant_settlement_hash)\n'
  '                {\n'
  '                    return Err(\n'
  '                        "Native AMX participant QCs do not bind the canonical '
  'source/entrypoint"\n'
  '                            .to_owned(),\n'
  '                    );\n'
  '                }\n'
  '                let computed_settlement_hash =\n'
  '                    leg.participant_settlement.computed_hash().map_err(|_| {\n'
  '                        "Native AMX participant control settlement cannot be '
  'hashed".to_owned()\n'
  '                    })?;\n'
  '                if computed_settlement_hash != leg.participant_settlement_hash {\n'
  '                    return Err(\n'
  '                        "Native AMX participant control settlement hash '
  'mismatch".to_owned()\n'
  '                    );\n'
  '                }\n'
  '                let settlement = &leg.participant_settlement;\n'
  '                if settlement.lane_id() != descriptor.lane_id\n'
  '                    || settlement.dataspace_id() != descriptor.dataspace_id\n'
  '                    || settlement.lane_incarnation() != descriptor.lane_incarnation\n'
  '                    || settlement.participant_lane_block_height() != '
  'descriptor.lane_block_height\n'
  '                    || settlement.authority_context_height() != authority_context_height\n'
  '                {\n'
  '                    return Err(\n'
  '                        "Native AMX participant settlement differs from its application '
  'context"\n'
  '                            .to_owned(),\n'
  '                    );\n'
  '                }\n'),
 ('crates/iroha_core/src/sumeragi/exec.rs',
  'fn',
  'from_result_bearing_block_and_merge_entry',
  '                if group.participant_proposal != leg.participant_proposal\n'
  '                    || group.participant_settlement != *settlement\n'
  '                    || group.participant_settlement_hash != '
  'leg.participant_settlement_hash\n'
  '                {\n'
  '                    return Err(\n'
  '                        "Native AMX participant route carries conflicting proposal/control '
  'claims"\n'
  '                            .to_owned(),\n'
  '                    );\n'
  '                }\n'
  '                if group\n'
  '                    .members\n'
  '                    .iter()\n'
  '                    .any(|member| member.source_id == source.receipt.source_id)\n'
  '                {\n'
  '                    return Err(\n'
  '                        "Native AMX participant control repeats a source '
  'transaction".to_owned(),\n'
  '                    );\n'
  '                }\n'),
 ('crates/iroha_core/src/sumeragi/exec.rs',
  'fn',
  'from_result_bearing_block_and_merge_entry',
  '            if source_ids != group.settlement_source_ids\n'
  '                || source_ids.iter().copied().collect::<BTreeSet<_>>().len() != '
  'source_ids.len()\n'
  '            {\n'
  '                return Err(\n'
  '                    "Native AMX grouped participant settlement does not exactly cover block '
  'sources"\n'
  '                        .to_owned(),\n'
  '                );\n'
  '            }\n'))

NATIVE_MERGE_SOURCE_BINDINGS = (
    *NATIVE_TYPED_SETTLEMENT_SOURCE_BINDINGS,
    *NATIVE_PARTICIPANT_APPLICATION_ROLE_BINDINGS,
    *NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_BINDINGS,
    (
        "crates/iroha_core/src/kura/prune_commit_merge_support.rs",
        "enum",
        "NativeAmxMergeAssociation",
        (
            "Live(Option<&'a MergeLedgerEntry>)",
            "Startup(Option<&'a MergeLedgerEntry>)",
            "CommittedOnly",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/exec.rs",
        "fn",
        "merge_native_amx_application_sources",
        (
            "reference.matches_entry(entry)",
            "entry.merge_qc.carrier_height != block.header().height().get()",
            "block.header().prev_block_hash() != Some(entry.merge_qc.carrier_parent_hash)",
            "entry.merge_qc.view != block.header().view_change_index()",
            "let Some(batch) = entry.execution_batch.as_ref() else",
            "return ordinary_native_amx_application_sources(block);",
            "!bundle.external.is_empty()",
            "block.external_entrypoints_cloned().next().is_some()",
            "merge_application_header_from_carrier(&block.header())",
            "merge_execution_batch_commitments_match(batch)",
            "execution.native_amx_receipts.len() != execution.entrypoints.len()",
            "Hash::from(canonical_entrypoint_hash) != *expected_entrypoint_hash",
            "Hash::from(result.hash()) != *expected_result_hash",
            "finality_bound_merge: true",
            "entrypoint_index != batch.entrypoint_count",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/exec.rs",
        "fn",
        "canonical_native_amx_application_sources",
        (
            "merge_entry: Option<&MergeLedgerEntry>",
            "ordinary_native_amx_application_sources(block)",
            "merge_native_amx_application_sources(block, entry)",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "planned_merge_entries_by_carrier",
        (
            "repairs: &[FinalizedMergeCarrierRepair]",
            "BTreeMap<(u64, HashOf<BlockHeader>), &MergeLedgerEntry>",
            "let key = (carrier_height, carrier_hash)",
            "entries.insert(key, repair.entry()).is_some()",
            "more than one finalized merge repair names carrier",
        ),
    ),
)

NATIVE_APPLICATION_MANIFEST_BINDING = (
    "crates/iroha_core/src/sumeragi/exec.rs",
    "fn",
    "from_result_bearing_block_and_merge_entry",
    (
        "merge_entry: Option<&MergeLedgerEntry>",
        "executed_block_wire_len",
        "executed_block_wire_hash",
        "canonical_native_amx_application_sources(block, merge_entry)?",
        "native_amx_participant_application_role",
        "NativeAmxParticipantApplicationRole::Coordinator",
        "NativeAmxParticipantApplicationRole::SeparateParticipant",
        "!source.finality_bound_merge",
        "source.finality_bound_merge",
        "authority_context_height > application_block_height",
        "prepare.source_id != source.receipt.source_id",
        "commit.source_id != source.receipt.source_id",
        "prepare.tx_entrypoint_hash != source.entrypoint_hash",
        "commit.tx_entrypoint_hash != source.entrypoint_hash",
        "leg.participant_settlement.computed_hash().map_err(|_|",
        "computed_settlement_hash != leg.participant_settlement_hash",
        "settlement.lane_id() != descriptor.lane_id",
        "settlement.dataspace_id() != descriptor.dataspace_id",
        "settlement.lane_incarnation() != descriptor.lane_incarnation",
        "settlement.participant_lane_block_height() != descriptor.lane_block_height",
        "settlement.authority_context_height() != authority_context_height",
        "let settlement_source_ids = settlement.source_ids().to_vec();",
        "source_ids.iter().copied().collect::<BTreeSet<_>>().len() != source_ids.len()",
        "BTreeMap::<(LaneId, DataSpaceId, Hash), u64>",
        "BTreeMap::<(LaneId, DataSpaceId, Hash, u64)",
        "route_heights",
        "descriptor.lane_block_height",
        "Native AMX participant route carries more than one height in one application block",
        "source_ids != group.settlement_source_ids",
        "MAX_NATIVE_AMX_APPLICATION_MANIFEST_LEAVES",
        "NativeAmxApplicationManifestLeafV1",
    ),
)

NATIVE_APPLICATION_MANIFEST_CLASSIFIER_MATCH_RELATION = (
    "crates/iroha_core/src/sumeragi/exec.rs",
    "fn",
    "from_result_bearing_block_and_merge_entry",
    (
        "match crate::native_amx::native_amx_participant_application_role( "
        "&source.receipt, leg, ) { "
        "Ok(crate::native_amx::NativeAmxParticipantApplicationRole::Coordinator) "
        "=> { continue; } "
        "Ok( crate::native_amx::"
        "NativeAmxParticipantApplicationRole::SeparateParticipant, ) => {} "
        "Err(error) => { return Err(format!( "
        '"Native AMX participant application identity is invalid: {error}" '
        ")); } }"
    ),
)

NATIVE_APPLICATION_MANIFEST_CLASSIFIER_ORDERED_SOURCE_CHECK = (
    "crates/iroha_core/src/sumeragi/exec.rs",
    "fn",
    "from_result_bearing_block_and_merge_entry",
    (
        "canonical_native_amx_application_sources(block, merge_entry)?",
        "for source in sources",
        "for leg in &source.receipt.legs",
        "match crate::native_amx::native_amx_participant_application_role(",
        "validate_lane_block_proposal(&leg.participant_proposal)",
        ".entry(key)",
        "source_ids != group.settlement_source_ids",
    ),
)

NATIVE_PARTICIPANT_APPLICATION_CLASSIFIER_MATCH_RE = re.compile(
    r"match\s+crate::native_amx::"
    r"native_amx_participant_application_role\s*"
    r"\(\s*(?:receipt|&source\.receipt)\s*,\s*leg\s*,?\s*\)"
)
NATIVE_PARTICIPANT_APPLICATION_ROLE_TOKENS = (
    "NativeAmxParticipantApplicationRole::Coordinator",
    "NativeAmxParticipantApplicationRole::SeparateParticipant",
)

NATIVE_MERGE_MANIFEST_CALLER_BINDINGS = (
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_candidate",
        (
            "from_result_bearing_block_and_merge_entry",
            "state_block.staged_merge_entry()",
            "execution_commitment_from_validated_block",
            "validate_native_amx_participant_application_evidence_byte_budget",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "native_amx_participant_frontier_markers_and_merge_entry",
        (
            "merge_entry: Option<&MergeLedgerEntry>",
            "from_result_bearing_block_and_merge_entry",
            "block,",
            "merge_entry,",
            ".entries()",
            "application_block_hash: leaf.application_block_hash",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "stage_native_amx_participant_frontiers",
        (
            "native_amx_participant_frontier_markers_and_merge_entry",
            "self.staged_merge_entry()",
            "encode_native_amx_participant_frontier_marker",
            "stage_merge_lane_frontier_markers",
        ),
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "replay_blocks_from_kura_range_inner",
        (
            "from_result_bearing_block_and_merge_entry",
            "state_block.staged_merge_entry()",
            "execution_commitment_from_validated_block",
            "replayed_execution_commitment != finality.commit_qc.execution_commitment",
        ),
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "lane_artifact_required_bytes_for_block",
        (
            "merge_entry: Option<&MergeLedgerEntry>",
            "merge_lane_application_artifact_required_bytes_for_block(block, merge_entry)?",
            "from_result_bearing_block_and_merge_entry",
            "block,",
            "merge_entry,",
            "native_amx_participant_application_artifacts",
            "NativeAmxParticipantReceiptLatestIndexV2::from_receipt",
            "native_prune_intent_routes.insert",
            "native_amx_evidence_prune_intent_max_bytes",
        ),
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "native_amx_manifest_for_committed_block",
        (
            "merge_association: NativeAmxMergeAssociation<'_>",
            "finality: &V2FinalityArtifact",
            "associated_merge_entry_for_block(block)?",
            "let planned_merge_entry = match merge_association",
            "NativeAmxMergeAssociation::Live(staged)",
            "NativeAmxMergeAssociation::Startup(staged)",
            "NativeAmxMergeAssociation::CommittedOnly => None",
            "if let Some(planned) = planned_merge_entry",
            "carrier_record_for_block_entry(block, planned)?",
            "validate_merge_carrier_finality_projection(",
            "committed != planned",
            "planned merge entry differs from its committed association",
            "let merge_entry = match merge_association",
            "live Native AMX merge publication lacks its staged association witness",
            "live Native AMX merge publication lacks its committed association",
            "live Native AMX staged merge entry differs from its committed association",
            "NativeAmxMergeAssociation::Startup(planned)",
            "committed_merge_entry.as_ref().or(planned)",
            "Self::block_merge_reference(block).is_some() && merge_entry.is_none()",
            "lacks its committed merge association",
            "from_result_bearing_block_and_merge_entry",
            "merge_entry,",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "native_amx_participant_application_evidence_for_block_under_publication_guard",
        (
            "merge_association: NativeAmxMergeAssociation<'_>",
            "v2_finality_artifact_with_archive_under_prune_guard",
            "native_amx_manifest_for_committed_block(",
            "merge_association",
            "&finality",
            "native_amx_application_manifest_root",
            "executed_block_wire_hash",
            "finality_artifact_hash",
            "native_amx_participant_receipt_matches_manifest_leaf",
        ),
    ),
)

NATIVE_MERGE_MANIFEST_NORMALIZED_RELATIONS = (
    *NATIVE_TYPED_SETTLEMENT_NORMALIZED_RELATIONS,
    (
        NATIVE_PARTICIPANT_APPLICATION_ROLE_RELATIVE,
        "fn",
        "native_amx_participant_application_role",
        "let descriptor = &leg.participant_proposal.descriptor; "
        "let prepare = &leg.prepare_qc.body; let commit = &leg.commit_qc.body; "
        "let settlement_hash = leg.participant_settlement.computed_hash() "
        '.map_err(|_| "Native AMX participant settlement cannot be hashed")?; '
        "if " + " || ".join(NATIVE_PARTICIPANT_APPLICATION_IDENTITY_COMPARISONS)
        + ' { return Err("Native AMX participant leg identity is internally inconsistent"); } '
        "let same_route = descriptor.lane_id == receipt.lane_id "
        "&& descriptor.dataspace_id == receipt.dataspace_id; "
        "if !same_route { return Ok(NativeAmxParticipantApplicationRole::SeparateParticipant); } "
        "if descriptor.lane_incarnation != receipt.lane_incarnation "
        "|| descriptor.proposal_height != receipt.authority_context_height "
        "|| descriptor.lane_block_height != receipt.lane_block_height "
        "|| descriptor.lane_block_view != receipt.lane_block_view "
        "|| leg.participant_proposal.proposal_hash != receipt.coordinator_proposal_hash "
        '{ return Err("Native AMX same-route leg differs from the coordinator identity"); } '
        "Ok(NativeAmxParticipantApplicationRole::Coordinator)",
    ),
    (
        NATIVE_PARTICIPANT_APPLICATION_ROLE_RELATIVE,
        "fn",
        "native_amx_receipt_requires_separate_participant_application_for",
        "let mut matches = false; for leg in &receipt.legs { "
        "match native_amx_participant_application_role(receipt, leg)? { "
        "NativeAmxParticipantApplicationRole::Coordinator => {} "
        "NativeAmxParticipantApplicationRole::SeparateParticipant => { "
        "let descriptor = &leg.participant_proposal.descriptor; "
        "matches |= descriptor.lane_id == lane_id "
        "&& descriptor.dataspace_id == dataspace_id "
        "&& descriptor.lane_incarnation == lane_incarnation; } } } Ok(matches)",
    ),
    (
        NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_RELATIVE,
        "fn",
        "participant_application_role_rejects_independent_prepare_and_commit_identity_drift",
        "let body = match phase { "
        "NativeAmxPhase::Prepare => &mut leg.prepare_qc.body, "
        "NativeAmxPhase::Commit => &mut leg.commit_qc.body, }; mutate(body); "
        "assert_eq!( native_amx_participant_application_role(&altered, &altered.legs[index]), "
        'Err(INCONSISTENT_IDENTITY), "leg {index}, {phase:?}: {label} drift must fail closed", );',
    ),
    (
        NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_RELATIVE,
        "fn",
        "participant_application_role_rejects_coherent_same_route_coordinator_drift",
        "mutate(&mut leg.participant_proposal.descriptor); rebind_participant_identity(leg); "
        "assert_eq!( native_amx_participant_application_role(&altered, &altered.legs[index]), "
        'Err(SAME_ROUTE_DRIFT), "coherent participant-side {label} drift must not become '
        'a separate application", );',
    ),
    (
        NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_RELATIVE,
        "fn",
        "participant_application_role_rejects_settlement_identity_and_content_tampering",
        "mutate(&mut altered.legs[index]); "
        "assert_eq!( native_amx_participant_application_role(&altered, &altered.legs[index]), "
        'Err(INCONSISTENT_IDENTITY), "leg {index}: settlement {label} tampering must fail closed", );',
    ),
    (
        NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_RELATIVE,
        "fn",
        "participant_application_lookup_validates_later_legs_after_an_exact_match",
        "assert_eq!( native_amx_receipt_requires_separate_participant_application_for( "
        "&receipt, route.0, route.1, route.2, ), Ok(true), ); "
        "receipt.legs[1].commit_qc.body.participant_previous_block_height += 1; "
        "for lane_id in [route.0, LaneId::new(90)] { "
        "assert_eq!( native_amx_receipt_requires_separate_participant_application_for( "
        "&receipt, lane_id, route.1, route.2, ), Err(INCONSISTENT_IDENTITY), "
        '"a malformed later leg must fail lookup even if the queried route matched '
        'or is absent", ); }',
    ),
    (
        NATIVE_MERGE_MANIFEST_FIXTURE_RELATIVE.as_posix(),
        "method",
        "ApplyFixture::new_for_production_recovered_decision_apply_with_native_lane_lifecycle",
        "Self::new_with_options(false, false, true, true)",
    ),
    (
        "crates/iroha_core/src/sumeragi/exec.rs",
        "fn",
        "canonical_native_amx_application_sources",
        "merge_entry.map_or_else( || ordinary_native_amx_application_sources(block), "
        "|entry| merge_native_amx_application_sources(block, entry), )",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_candidate",
        "let native_amx_manifest = crate::sumeragi::exec::"
        "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry( "
        "valid.as_ref(), state_block.staged_merge_entry(), )",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_and_apply",
        "let native_amx_manifest = crate::sumeragi::exec::"
        "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry( "
        "valid_block.as_ref(), state_block.staged_merge_entry(), )",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_and_apply",
        "self.kura .prepublish_native_amx_participant_application_evidence( "
        "committed_block.as_ref(), state_block.staged_merge_entry(), )",
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "native_amx_participant_frontier_markers_and_merge_entry",
        "let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::"
        "from_result_bearing_block_and_merge_entry( block, merge_entry, )",
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "stage_native_amx_participant_frontiers",
        "let markers = State::native_amx_participant_frontier_markers_and_merge_entry( "
        "block, self.staged_merge_entry(), )?;",
    ),
    (
        "crates/iroha_core/src/state.rs",
        "fn",
        "replay_blocks_from_kura_range_inner",
        "let native_amx_manifest = crate::sumeragi::exec::"
        "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry( "
        "valid_block.as_ref(), state_block.staged_merge_entry(), )",
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "lane_artifact_required_bytes_for_block",
        "let native_manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::"
        "from_result_bearing_block_and_merge_entry( block, merge_entry, )",
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "native_amx_manifest_for_committed_block",
        "let committed_merge_entry = self.associated_merge_entry_for_block(block)?;",
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "native_amx_manifest_for_committed_block",
        "if let Some(planned) = planned_merge_entry { let record = "
        "Self::carrier_record_for_block_entry(block, planned)?; "
        "Self::validate_merge_carrier_finality_projection( record, planned, "
        "&block.header(), finality, )?;",
    ),
    (
        "crates/iroha_core/src/kura/lane_artifact_budget.rs",
        "fn",
        "native_amx_manifest_for_committed_block",
        "NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry( "
        "block, merge_entry, )",
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "prepublish_native_amx_participant_application_evidence",
        "let plan = self "
        ".native_amx_participant_application_evidence_for_block_under_publication_guard( "
        "block, false, NativeAmxMergeAssociation::Live(staged_merge_entry), )?;",
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "repair_native_amx_participant_application_evidence",
        "let plan=self.native_amx_participant_application_evidence_for_block_under_publication_guard("
        "block,true,NativeAmxMergeAssociation::CommittedOnly)?;",
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "fn",
        "native_amx_participant_application_evidence_for_block_under_publication_guard",
        "let native_manifest = self.native_amx_manifest_for_committed_block("
        "block, merge_association, &finality)?;",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "build_canonical_executed_block_response",
        "kura.read_block_body(height).map_err(CanonicalRecoveryReadError::storage)?",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "plan_lane_application_evidence_repair",
        "kura.read_block_body(height)"
        ".map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "plan_lane_application_evidence_repair",
        "let planned_merge_entries = "
        "planned_merge_entries_by_carrier(&merge_carriers)?;",
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "apply_lane_application_evidence_repair",
        "summary.merge_carriers = kura .apply_finalized_merge_carrier_repairs( "
        "&plan.merge_carriers, plan.merge_carrier_repair_authorizations, )",
    ),
)

NATIVE_MERGE_MANIFEST_ORDERED_RELATIONS = (
    *(
        (relative, kind, symbol, tokens)
        for relative, kind, symbol, tokens in NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_BINDINGS
        if symbol in {
            "participant_application_role_rejects_independent_prepare_and_commit_identity_drift",
            "participant_application_role_rejects_settlement_identity_and_content_tampering",
            "participant_application_lookup_validates_later_legs_after_an_exact_match",
        }
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::new_with_output_guard_and_transport_inner",
        (
            ".begin_fail_stop_operation()",
            ".prune_finalized_pending_certified_merge_entries(finalized_cleanup_height)",
            "adapter.ensure_globally_applied_lane_receipts_durable()?;",
            "construction.complete();",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "V2LaneWorkAdapter::activate_after_lane_drain_queue_install",
        (
            "if !Arc::ptr_eq(installed_queue, queue)",
            ".begin_fail_stop_operation()",
            "self.hydrate_canonical_lane_artifacts()?;",
            "self.revalidate_hydrated_autonomous_queue_owners(installed_queue.as_ref())?;",
            "self.startup_activation_complete = true;",
            "self.drive_lane_sessions();",
            "activation.complete();",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "method",
        "CanonicalExecutedBlockRecovery::reconcile_cached_front",
        (
            "self.kura.read_block_body(height)",
            "validate_canonical_executed_block_need(",
            "!canonical_executed_block_matches_need(&block, &finality, need)",
            ".preflight_cached_finalized_merge_carrier_reconstruction(&block)",
            "self.needs.pop_front();",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "plan_lane_application_evidence_repair",
        (
            "let planned_merge_entries = "
            "planned_merge_entries_by_carrier(&merge_carriers)?;",
            "let Some(block) = kura",
            ".read_block_body(height)",
            ".map_err(|error| V2LaneWorkError::Persistence(error.to_string()))?",
            "let planned_merge_entry = planned_merge_entries",
            ".get(&(application_block_height, application_block_hash))",
            "preflight_native_amx_participant_application_evidence_repair(",
            "planned_merge_entry,",
            "drop(planned_merge_entries);",
            "if !needs.is_empty()",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work/"
        "canonical_executed_block_application_repair.rs",
        "fn",
        "apply_lane_application_evidence_repair",
        (
            "let planned_merge_entries = "
            "planned_merge_entries_by_carrier(&plan.merge_carriers)?;",
            "preflight_native_amx_participant_application_evidence_repair(",
            "planned_merge_entries",
            "drop(planned_merge_entries);",
            "preflight_finalized_merge_carrier_repairs(",
            "summary.merge_carriers = kura",
            ".apply_finalized_merge_carrier_repairs(",
            "for carrier in &plan.native_carriers",
            ".repair_native_amx_participant_application_evidence_for_markers(",
        ),
    ),
)

NATIVE_MERGE_MANIFEST_RAW_TEST_CHECKS = (
    (
        NATIVE_MERGE_MANIFEST_CORRIDOR_RELATIVE,
        "historical_autonomous_recovery_reaches_exactly_once_canonical_merge_application",
        (
            "ApplyFixture::new_for_production_recovered_decision_apply_with_native_lane_lifecycle()",
            "lane_work.merge_execution_full_validation_checks_for_test(),\n"
            "            0",
            "for _ in 0..4 {",
            ".validate_merge_execution_candidate_for_test(&candidate, &parent_header, 0)",
            '"the locally built execution candidate must not be fully reexecuted by the adapter"',
            "lane_work.merge_execution_full_validation_checks_for_test(),\n"
            "            1",
            "fail_next_native_amx_prepublication_for_tests",
            '"pre-WSV Native AMX participant evidence publication"',
            '"failed live Native prepublication must not stage WSV"',
            "prepublish_native_amx_participant_application_evidence(",
            "durable_carrier.as_ref(), None)",
            '"live merge prepublication requires its exact staged witness"',
            "durable_carrier.as_ref(),\n                Some(&entry),",
            "live_prepublication.authenticates_state_frontiers",
            "remove_latest_native_amx_participant_manifest_for_testing",
            '"remove only the exact latest Native manifest"',
            "remove_merge_carrier_record_for_testing",
            "read_structural_native_amx_participant_application_receipt(",
            '"manifest loss must retain the exact structural Native receipt"',
            "read_native_amx_participant_application_receipt(",
            ".is_none()",
            '"the authoritative reader must reject a receipt without its manifest"',
            "preflight_native_amx_participant_application_evidence_repair(",
            "std::slice::from_ref(&native_marker),\n                None,",
            '"startup Native repair requires a committed or planned association"',
            "std::slice::from_ref(&native_marker),\n                Some(&entry),",
            '"planned merge association authorizes exact Native startup repair"',
            "plan_lane_application_evidence_repair(",
            "apply_lane_application_evidence_repair(",
            "native_carriers: 1",
            "native_routes: 1",
            "merge_carriers: 1",
            '"startup repair must reproduce the exact retained receipt bytes"',
            '"startup evidence repair must not mutate canonical WSV"',
            "assert!(empty_plan.is_empty())",
        ),
    ),
)

NATIVE_MERGE_MANIFEST_SOURCE_RELATIVES = (
    NATIVE_MERGE_MANIFEST_CONTRACT_RELATIVE,
    NATIVE_MERGE_MANIFEST_TEST_RELATIVE,
    Path("pytests/scripts/sumeragi_v2_multilane_native_settlement_test.py"),
    NATIVE_MERGE_MANIFEST_CORRIDOR_RELATIVE,
    NATIVE_MERGE_MANIFEST_FIXTURE_RELATIVE,
    Path(NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_RELATIVE),
)


def _validate_native_merge_manifest_raw_tests(
    root: Path, errors: list[str]
) -> None:
    for relative, test_name, required_tokens in NATIVE_MERGE_MANIFEST_RAW_TEST_CHECKS:
        path = root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{path}: cannot read Native corridor macro test: {error}")
            continue
        declaration = re.compile(
            r"v2_apply_test!\(\s*" + re.escape(test_name) + r"\s*,"
        )
        matches = list(declaration.finditer(source))
        if len(matches) != 1:
            errors.append(
                f"{path}: Native corridor macro test {test_name} must occur "
                f"exactly once, found {len(matches)}"
            )
            continue
        start = matches[0].start()
        next_test = re.search(r"v2_apply_test!\(", source[matches[0].end() :])
        end = (
            matches[0].end() + next_test.start()
            if next_test is not None
            else len(source)
        )
        item = source[start:end]
        cursor = -1
        for token in required_tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{path}: Native corridor macro test {test_name} is "
                    f"missing or reorders token {token!r}"
                )
                break
            cursor = position


def _normalize_rust_relation(source: str) -> str:
    """Ignore layout and optional trailing commas, but preserve token order."""

    compact = re.sub(r"\s+", "", source)
    return re.sub(r",(?=[)\]}])", "", compact)


def validate_native_merge_manifest_relations(
    root: Path,
    binding_items: dict[tuple[str, str, str], str],
    errors: list[str],
    rust_binding_item=None,
) -> None:
    """Require every consumer to use its staged, durable, or planned entry."""

    _validate_native_merge_manifest_raw_tests(root, errors)

    for relative, kind, symbol, expected_relation in (
        NATIVE_MERGE_MANIFEST_NORMALIZED_RELATIONS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None and rust_binding_item is not None:
            item = rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "Native merge-manifest relation",
                errors,
            )
        if item is None:
            continue
        normalized = _normalize_rust_relation(item)
        count = normalized.count(_normalize_rust_relation(expected_relation))
        if count != 1:
            errors.append(
                f"{root / relative}: Native merge-manifest relation {symbol} "
                "must bind the exact reviewed Native relation once, "
                f"found {count}"
            )

    for relative, kind, symbol, ordered_tokens in (
        NATIVE_MERGE_MANIFEST_ORDERED_RELATIONS
    ):
        item = binding_items.get((relative, kind, symbol))
        if item is None and rust_binding_item is not None:
            item = rust_binding_item(
                root,
                relative,
                kind,
                symbol,
                "Native merge-manifest corridor",
                errors,
            )
        if item is None:
            continue
        cursor = -1
        for token in ordered_tokens:
            position = item.find(token, cursor + 1)
            if position < 0:
                errors.append(
                    f"{root / relative}: Native merge-manifest corridor "
                    f"{symbol} is missing or reorders token {token!r}"
                )
                break
            cursor = position
