"""Exact source bindings for Native AMX merge-manifest projection."""

from __future__ import annotations

import re
from pathlib import Path

import sumeragi_v2_multilane_native_preparation_contract as native_preparation


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
  ('Self::new_with_options_and_retention_and_genesis(',
   'include_lane_payload,', 'include_projection_policies,',
   'include_lane_lifecycle,', 'include_native_lane,', 'blocks_in_memory,', 'false,')),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options_and_retention_and_genesis',
  ('Self::new_with_options_and_retention_and_genesis_and_archival_kura(',
   'include_lane_payload,', 'include_projection_policies,',
   'include_lane_lifecycle,', 'include_native_lane,', 'blocks_in_memory,',
   'seed_genesis_domain,', 'None,')),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options_and_retention_and_genesis_and_archival_kura',
  ('(1_u8..=4)',
   'Algorithm::BlsNormal',
   'if let Some(kura) = archival_kura {',
   'assert!(include_native_lane && include_lane_lifecycle);',
   'else if include_lane_lifecycle {',
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
  'Self::new_with_options_and_retention_and_genesis(include_lane_payload, '
  'include_projection_policies, include_lane_lifecycle, include_native_lane, blocks_in_memory, false,)'),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options_and_retention_and_genesis',
  'Self::new_with_options_and_retention_and_genesis_and_archival_kura(include_lane_payload, '
  'include_projection_policies, include_lane_lifecycle, include_native_lane, blocks_in_memory, seed_genesis_domain, None,)'),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options_and_retention_and_genesis_and_archival_kura',
  'let kura = if let Some(kura) = archival_kura { '
  'assert!(include_native_lane && include_lane_lifecycle); kura '
  '} else if include_lane_lifecycle { '
  'crate::sumeragi::v2_lane_work::tests::locked_lane_work_test_kura(blocks_in_memory) } else { '
  'Kura::blank_kura_for_testing_with_blocks_in_memory(blocks_in_memory) };'),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options_and_retention_and_genesis_and_archival_kura',
  'let mut state = if include_lane_lifecycle { '
  'crate::sumeragi::v2_lane_work::tests::authenticated_lane_work_state_for_testing('
  'world, Arc::clone(&kura), chain_id.clone(), context.network_id,) } else { '
  'State::new_with_chain_and_network_id_for_testing(world, Arc::clone(&kura), '
  'LiveQueryStore::start_test(), chain_id.clone(), context.network_id,) };'),
 ('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_00.rs',
  'method',
  'ApplyFixture::new_with_options_and_retention_and_genesis_and_archival_kura',
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

# Executable publication uses native Decisions and their actual captured outputs.
# A certified MergeQC remains an adverse source-control fixture; it cannot stand
# in for native participant publication or application-evidence repair coverage.
NATIVE_CANONICAL_PUBLICATION_FIXTURE_BINDINGS = (('crates/iroha_core/src/sumeragi/tests/v2_apply_certified_source_rejection.rs',
  'fn',
  'assert_retired_merge_candidate_rejected',
  ('let before_state = crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref())',
   'let before_ledger = fixture.state.merge_ledger.snapshot();',
   'let before_height = fixture.state.committed_height();',
   'let before_tree = completed_secondary_tree(&fixture.kura.store_root());',
   'for _ in 0..2 {',
   '.validate_candidate(context, body)',
   '.expect_err("MergeQC cannot substitute for the canonical native Decision source")',
   'reason.contains("ordinary output source contains a competing native owner")',
   'assert_eq!(fixture.state.committed_height(), before_height);',
   'crate::snapshot::canonical_state_snapshot_hash(fixture.state.as_ref()).unwrap(),\n'
   '            before_state',
   'assert_eq!(fixture.state.merge_ledger.snapshot(), before_ledger);',
   'assert_eq!(fixture.state.has_committed_entrypoint(*hash), *committed);',
   'completed_secondary_tree(&fixture.kura.store_root()),\n            before_tree')),
 ('crates/iroha_core/src/state/native_publication_test_support.rs',
  'fn',
  'prepared_native_publication_for_test',
  ('let mut executed = carrier.clone();',
   'ValidBlock::execute_native_block_and_capture_for_test(',
   '&mut executed, state, &context',
   'let (committed, witness) =\n        finalize_native_execution_for_test(',
   'state, executed, &mut overlay, context',
   'overlay\n        .authorize_execution_output_publication(&committed, &witness)',
   'overlay\n        .apply_without_execution_with_verified_v2_finality(&committed)',
   'assert!(\n'
   '        overlay\n'
   '            .native_output_publication_identity()\n'
   '            .unwrap()\n'
   '            .is_some()\n'
   '    );',
   'assert!(matches!(\n'
   '        overlay.execution_output_plan,\n'
   '        Some(super::output_capacity::ExecutionOutputPlanState::Finalized(\n'
   '            _\n'
   '        ))\n'
   '    ));',
   '(overlay, committed)')),
 ('crates/iroha_core/src/state/native_execution_finality_test_support.rs',
  'fn',
  'finalize_native_execution_for_test',
  ('context.validate()',
   'assert_eq!(context.network_id, state.network_id);',
   'assert_eq!(context.height, block.header().height().get());',
   'context.roster.len(),\n        4',
   'assert_eq!(validator.power, 1',
   'let witness = overlay\n        .take_exec_witness()',
   'let casting = overlay\n        .take_parliament_timed_ovn_casting_bindings()',
   'NativeAmxApplicationManifestV1::from_result_bearing_block_and_merge_entry(',
   'LaneFinalityManifestV1::from_result_bearing_block(&block)',
   'execution_commitment_from_validated_block(&witness, &native, &lanes, &block)',
   'let preimage = iroha_data_model::block::consensus_v2::Vote {',
   'let signatures = keys\n        .iter()\n        .take(3)',
   'signers: vec![0, 1, 2]',
   'crate::block::VerifiedV2FinalityArtifact::verify(artifact.clone())',
   '.commit_with_verified_v2_artifact(verified, execution_commitment)',
   '.stage_kagemusha_finality_sidecar(',
   '&witness,\n            execution_commitment,\n            &casting',
   '.store_block(committed.clone())',
   '.store_v2_finality_artifact(&artifact)',
   'assert_eq!(receipt.artifact_hash(), HashOf::new(&artifact));',
   'assert_eq!(receipt.block_hash(), committed.as_ref().hash());',
   '(committed, witness)')),
 ('crates/iroha_core/src/state/native_execution_finality_test_support.rs',
  'fn',
  'promote_native_execution_finality_for_test',
  ('.verified_v2_finality_artifact()',
   '.store_v2_finality_artifact(artifact)',
   '.promote_kagemusha_finality_sidecar(artifact, &receipt)')),
 ('crates/iroha_core/src/state/native_completed_history_tests.rs',
  'fn',
  'publish_next_native_group_for_test',
  ('state\n        .kura\n        .get_block(',
   'state\n        .kura\n        .v2_finality_artifact(',
   'build_successor_height_context(',
   'state\n        .prepare_lane_decision_batch(std::slice::from_ref(group))',
   'BlockExecutionContextBundle::default().with_native_lane_decisions(batch)',
   'prepared_native_publication_for_test(state, &carrier, context)',
   'overlay\n        .commit()',
   'promote_native_execution_finality_for_test(state, &committed)')),
 ('crates/iroha_core/src/state/native_completed_history_tests.rs',
  'fn',
  'cold_restore_completed_native_history_for_test',
  ('for entry in std::fs::read_dir(source)',
   'if kind.is_dir() {',
   'copy_durable_tree(&source, &destination);',
   'assert!(\n                    kind.is_file()',
   'std::fs::write(&destination, &bytes)',
   'assert_eq!(std::fs::read(&destination).unwrap(), bytes);',
   'assert_eq!(\n'
   '                    std::fs::read(&source).unwrap(),\n'
   '                    bytes',
   'let cold_root = tempfile::tempdir()',
   'copy_durable_tree(&state.kura.store_root(), cold_root.path());',
   'strict_kura_config_for_testing(cold_root.path().to_path_buf())',
   'Kura::new_with_configured_lane_catalog(',
   'kura: cold_kura',
   'lane_manifests: state.lane_manifests.read().clone()',
   '.into_state_from_json(norito::json::to_value(state).unwrap())',
   'restored\n        .kura\n        .bind_lane_storage_network(restored.network_id)',
   'restored\n'
   '        '
   '.prepare_restored_configured_primary_geometry_anchor(&nexus.configured_lane_catalog)',
   'restored\n        .restore_kura_lane_segments_from_nexus()',
   'restored\n        .set_nexus_from_config(nexus)',
   'restored.install_lane_compliance_engine(state.lane_compliance_engine())',
   'restored.execution_policy_digest_v1().unwrap()',
   'state.execution_policy_digest_v1().unwrap()',
   'crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap()',
   'crate::snapshot::canonical_state_snapshot_hash(state).unwrap()',
   '(cold_root, restored)')),
 ('crates/iroha_core/src/state/native_completed_history_tests.rs',
  'fn',
  'assert_completed_native_history_for_test',
  ('crate::snapshot::canonical_state_snapshot_hash(state).unwrap()',
   'state.kura.exact_durable_blocks_count().unwrap()',
   'blocks.iter().zip(original_groups)',
   'let expected_wire = signed.encode_wire().unwrap()',
   'state.kura.get_block(height).unwrap().encode_wire().unwrap(),\n            expected_wire',
   'block.verified_v2_finality_artifact().unwrap()',
   'let finality_bytes = std::fs::read(&path).unwrap()',
   'state\n'
   '                .kura\n'
   '                .v2_finality_artifact(height.get() as u64)\n'
   '                .unwrap()\n'
   '                .as_ref(),\n'
   '            Some(original_finality)',
   'NativeLaneBatchCarrierReadV1::Ready(included)',
   'state\n'
   '            .read_finalized_native_lane_batch(height, signed.hash())\n'
   '            .unwrap()',
   'assert_eq!(included.batch().groups, vec![group.to_wire()])',
   'assert_eq!(\n'
   '            state.view().transactions.get(&entrypoint),\n'
   '            Some(height)',
   'assert!(\n'
   '            state\n'
   '                .replay_finalized_native_lane_batch(&included, &[])\n'
   '                .is_err()',
   'state\n'
   '            .preexecute_lane_decision_groups(successor.header(), '
   'std::slice::from_ref(group))\n'
   '            .err()',
   'if reason == expected_reason',
   'assert_eq!(\n            std::fs::read(&path).unwrap(),\n            finality_bytes',
   'assert_eq!(\n'
   '            state.kura.get_block(height).unwrap().encode_wire().unwrap(),\n'
   '            expected_wire\n'
   '        )',
   'assert_eq!(\n'
   '        state.kura.exact_durable_blocks_count().unwrap(),\n'
   '        durable_height\n'
   '    )',
   'assert_eq!(\n'
   '        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),\n'
   '        before')),
 ('crates/iroha_core/src/state/native_completed_history_tests.rs',
  'fn',
  'native_completed_history_rejects_reapplication_after_second_economic_commit_impl',
  ('native_publication_fixture_for_test(&[\n'
   '        NativeEconomicCase::Transfer(25),\n'
   '        NativeEconomicCase::Transfer(15),\n'
   '    ])',
   'assert_eq!(original_groups.len(), 2)',
   'for original in &original_groups {',
   'publish_next_native_group_for_test(&fixture, current)',
   'assert_eq!(block.as_ref().network_entrypoint_count(), 1)',
   'assert!(\n'
   '            block\n'
   '                .as_ref()\n'
   '                .network_output_at(0)\n'
   '                .unwrap()\n'
   '                .1\n'
   '                .result\n'
   '                .is_ok()\n'
   '        )',
   'assert!(block.as_ref().output_results().all(|result| result.is_ok()))',
   'assert_eq!(state.committed_height(), admission_height + committed.len())',
   'assert!(\n            current_total > total',
   'assert_completed_native_history_for_test(\n'
   '            state,\n'
   '            &committed,\n'
   '            &original_groups[..committed.len()],\n'
   '        )',
   'assert_eq!(total, Quantity::from(40_u32))',
   'assert_eq!(\n'
   '        state.world.assets.view().get(&fixture.source).unwrap().0,\n'
   '        Quantity::from(60_u32)\n'
   '    )',
   'assert_eq!(\n'
   '        committed[1].as_ref().header().prev_block_hash(),\n'
   '        Some(committed[0].as_ref().hash())\n'
   '    )',
   'state\n'
   '            .verified_lane_consensus_contexts()\n'
   '            .unwrap()\n'
   '            .unwrap()\n'
   '            .contexts()\n'
   '            .is_empty()',
   'let (_cold_root, restored) = cold_restore_completed_native_history_for_test(state)',
   'restored\n'
   '            .kura\n'
   '            .get_block(admission_height)\n'
   '            .unwrap()\n'
   '            .encode_wire()\n'
   '            .unwrap(),\n'
   '        admission_wire',
   'restored\n'
   '            .kura\n'
   '            .v2_finality_artifact(admission_height.get() as u64)\n'
   '            .unwrap(),\n'
   '        admission_finality',
   'assert_completed_native_history_for_test(&restored, &committed, &original_groups)',
   'assert!(\n        restored\n            .read_finalized_native_lane_batch(',
   'committed[0].as_ref().hash(),\n            )\n            .is_err()',
   'assert_eq!(\n'
   '        restored.world.assets.view().get(&fixture.source).unwrap().0,\n'
   '        Quantity::from(60_u32)\n'
   '    )',
   'assert_eq!(\n'
   '        restored\n'
   '            .world\n'
   '            .assets\n'
   '            .view()\n'
   '            .get(&fixture.destination)\n'
   '            .unwrap()\n'
   '            .0,\n'
   '        Quantity::from(40_u32)\n'
   '    )',
   'for change_instance in [false, true] {',
   'copied.base_state_height = restored.committed_height() as u64;',
   'copied.base_state_hash = restored.lane_execution_state_hash().unwrap();',
   'copied.groups[0].payload.descriptor.slots[0].instance_id =',
   'copied.groups[0].payload.descriptor.slots[0].lane_incarnation =',
   'assert!(\n'
   '            restored\n'
   '                .prepare_proposed_native_lane_batch_source(&candidate, &[])\n'
   '                .is_err()',
   'assert_eq!(\n'
   '            crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap(),\n'
   '            before\n'
   '        )',
   'assert_completed_native_history_for_test(&restored, &committed, &original_groups)')))

NATIVE_MERGE_MANIFEST_CALLER_BINDINGS = (
('crates/iroha_core/src/sumeragi/tests/v2_apply_unsealed_01c_historical_recovery.rs',
 'fn',
 'run_autonomous_merge_frontier_fixture',
 ('ApplyFixture::new_for_production_recovered_decision_apply_with_native_lane_lifecycle()',
  'for _ in 0..4 {',
  '.validate_merge_execution_candidate_for_test(&candidate, &parent_header, 0)',
  '"the locally built execution candidate must not be fully reexecuted by the adapter"',
  'authenticate_merge_entry_for_height_context(',
  'if frontier_case != MergeFrontierFixtureCase::SuccessfulApply {',
  'assert_retired_merge_candidate_rejected(',
  'assert_eq!(queue.live_lane_reservations(), reservations);',
  'assert!(queue.lane_reservation_commit_barriers().is_empty());',
  'assert!(queue.lane_reservation_release_barriers().is_empty());',
  'assert_eq!(autonomous_balance(), None);',
  'assert!(!participant_metadata_is_committed());',
  'assert!(\n'
  '                expected_fifo\n'
  '                    .iter()\n'
  '                    .all(|hash| !fixture.state.has_committed_entrypoint(*hash))\n'
  '            );',
  'fixture\n            .kura\n            .durable_autonomous_lane_merge_source(',
  'assert!(queue.has_durable_plan_claim_for_test(key.entrypoint_hash));',
  'assert_cold_merge_registry_replay_boundary(',
  'return;')),
    *NATIVE_CANONICAL_PUBLICATION_FIXTURE_BINDINGS,
    (
        "crates/iroha_core/src/sumeragi/v2_apply.rs",
        "method",
        "V2ApplyService::validate_candidate",
        native_preparation.CANDIDATE_TOKENS,
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
        native_preparation.ORDINARY_TOKENS,
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
        "Ok(prepared.execution_prefix_commitment())",
    ),
    (
        native_preparation.PREFIX,
        "method",
        "PrefixPreparation::capture",
        "let manifest = exec::NativeAmxApplicationManifestV1::"
        "from_result_bearing_block_and_merge_entry( block, None, )?;",
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
        "let mut total = self.merge_lane_application_artifact_required_bytes_for_block(block, merge_entry)?;",
    ),
    (
        native_preparation.CAPACITY,
        "method",
        "Kura::native_amx_publication_plan_for_storage_under_prune_and_canonical_guards",
        "let manifest = crate::sumeragi::exec::NativeAmxApplicationManifestV1::"
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
        "historical_certified_source_recovery_cannot_authorize_retired_merge_execution",
        ("run_autonomous_merge_frontier_fixture(MergeFrontierFixtureCase::HistoricalRecovery);",),
    ),
)
NATIVE_MERGE_MANIFEST_CORRIDOR_HELPER_BINDING = NATIVE_MERGE_MANIFEST_CALLER_BINDINGS[0]

NATIVE_MERGE_MANIFEST_SOURCE_RELATIVES = (
    NATIVE_MERGE_MANIFEST_CONTRACT_RELATIVE,
    NATIVE_MERGE_MANIFEST_TEST_RELATIVE,
    Path("pytests/scripts/sumeragi_v2_multilane_native_fixture_delegation_test.py"),
    Path("pytests/scripts/sumeragi_v2_multilane_native_settlement_test.py"),
    NATIVE_MERGE_MANIFEST_CORRIDOR_RELATIVE,
    NATIVE_MERGE_MANIFEST_FIXTURE_RELATIVE,
    *(Path(path) for path, _, _, _ in NATIVE_CANONICAL_PUBLICATION_FIXTURE_BINDINGS),
    Path(NATIVE_PARTICIPANT_APPLICATION_ROLE_TEST_RELATIVE),
)


def _fixture_body(source: str, declaration: re.Pattern, label: str, errors: list[str]) -> str | None:
    """Read one exact braced fixture body; comments/literals cannot change its extent."""
    from sumeragi_v2_multilane_reviewed_rust_source import _mask_rust_comments

    masked = _mask_rust_comments(source)
    matches = list(declaration.finditer(masked))
    if len(matches) != 1:
        errors.append(f"{label} must occur exactly once, found {len(matches)}")
        return None
    start = masked.find("{", matches[0].end())
    depth = 0
    for index in range(start, len(masked)):
        depth += (masked[index] == "{") - (masked[index] == "}")
        if depth == 0:
            return source[start:index + 1]
    errors.append(f"{label} has no complete body")
    return None


def _validate_native_merge_manifest_raw_tests(root: Path, errors: list[str]) -> None:
    from sumeragi_v2_multilane_geometry_evidence_contract import _code

    for relative, test_name, required_tokens in NATIVE_MERGE_MANIFEST_RAW_TEST_CHECKS:
        path = root / relative
        label = f"{path}: Native corridor macro test {test_name}"
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{label}: cannot read source: {error}")
            continue
        wrapper = _fixture_body(source, re.compile(
            r"v2_apply_test!\(\s*" + re.escape(test_name) + r"\s*,"), label, errors)
        if wrapper is not None and _code(wrapper) != _code("{" + required_tokens[0] + "}"):
            errors.append(f"{label} must dispatch exactly {required_tokens[0]!r}")
        _, _, helper, helper_tokens = NATIVE_MERGE_MANIFEST_CORRIDOR_HELPER_BINDING
        body = _fixture_body(source, re.compile(r"\bfn\s+" + helper + r"\s*\([^)]*\)"),
                             f"{label} helper {helper}", errors)
        if body is None:
            continue
        normalized = _normalize_rust_relation(body)
        cursor = -1
        for token in helper_tokens:
            position = normalized.find(_normalize_rust_relation(token).rstrip(","), cursor + 1)
            if position < 0:
                errors.append(f"{label} helper {helper} is missing or reorders token {token!r}")
                break
            cursor = position
        code = _code(body)
        dispatch = (
            "let fixture = if frontier_case == MergeFrontierFixtureCase::StartupRegistryBoundaries { "
            "ApplyFixture::new_for_cold_merge_registry_replay() } else { "
            "ApplyFixture::new_for_production_recovered_decision_apply_with_native_lane_lifecycle() };"
        )
        if _code(dispatch) not in code:
            errors.append(f"{label} helper {helper} changes its exact Native fixture dispatch")
        # Every historical source must reach the exact rejection/custody branch.
        # A special branch must not skip its retained assertions.
        if "MergeFrontierFixtureCase::HistoricalRecovery" in code:
            errors.append(f"{label} helper {helper} intercepts HistoricalRecovery fallthrough")
        if _code("if false {") in code:
            errors.append(f"{label} helper {helper} hides a required source check")

    history_path = root / "crates/iroha_core/src/state/native_completed_history_tests.rs"
    try:
        history_source = history_path.read_text(encoding="utf-8")
    except (OSError, UnicodeDecodeError) as error:
        errors.append(f"{history_path}: canonical native publication wrapper is unavailable: {error}")
    else:
        wrapper = (
            "state_test!(consensus_stack "
            "native_completed_history_rejects_reapplication_after_second_economic_commit "
            "native_completed_history_rejects_reapplication_after_second_economic_commit_impl(););"
        )
        if _code(history_source).count(_code(wrapper)) != 1:
            errors.append(f"{history_path}: canonical native publication wrapper must dispatch exactly once")

    for relative, _, symbol, tokens in NATIVE_CANONICAL_PUBLICATION_FIXTURE_BINDINGS:
        path = root / relative
        label = f"{path}: canonical native publication fixture {symbol}"
        try:
            source = path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(f"{label}: cannot read source: {error}")
            continue
        body = _fixture_body(source, re.compile(
            r"\bfn\s+" + re.escape(symbol) + r"(?:<[^>]*>)?\s*\([^)]*\)"),
            label, errors)
        if body is None:
            continue
        normalized = _normalize_rust_relation(body)
        cursor = -1
        for token in tokens:
            position = normalized.find(_normalize_rust_relation(token).rstrip(","), cursor + 1)
            if position < 0:
                errors.append(f"{label} is missing or reorders {token!r}")
                break
            cursor = position
        if _code("if false {") in _code(body):
            errors.append(f"{label} hides a required publication assertion")


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
