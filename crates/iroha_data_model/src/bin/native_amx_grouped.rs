//! Rust-owned grouped Native AMX v2 JSON fixture generation.
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, MerkleTree, Signature};
use iroha_data_model::{
    NetworkId,
    block::{
        Header as BlockHeader,
        consensus::{
            LaneBlockCommitment, LaneBlockDescriptorV1, LaneBlockProposalV1, LaneSettlementReceipt,
            NATIVE_AMX_BLS_PROOF_BYTES, NATIVE_AMX_GROUP_SOURCES_MAX,
            NATIVE_AMX_PARTICIPANT_LEGS_MAX, NATIVE_AMX_VALIDATORS_MAX, NativeAmxAttestationBodyV2,
            NativeAmxAttestationQcV2, NativeAmxLegRecordV2, NativeAmxPhase, NativeAmxReceipt,
            SumeragiDiagnosticsStatus, SumeragiNativeAmxParticipantApplication,
            SumeragiNativeAmxParticipantApplicationState, SumeragiPipelineExecutionStatus,
        },
        consensus_v2::{
            ConsensusRound, ExecutionCommitment, HeightContext, HeightContextId,
            MERGE_CARRIER_COMMITMENT_VERSION_V1, MergeCarrierCommitmentV1,
            NATIVE_AMX_APPLICATION_MANIFEST_VERSION, NativeAmxApplicationManifestLeafV1,
            NativeAmxApplicationManifestMemberV1,
        },
    },
    consensus::VALIDATOR_SET_HASH_VERSION_V1,
    merge::MergeLedgerEntry,
    nexus::{DataSpaceId, LaneId, compute_settlement_hash},
    peer::PeerId,
    transaction::{TransactionEntrypoint, TransactionResult},
};
use iroha_primitives::numeric::Quantity;
use norito::json::{self, Value};
use std::{collections::BTreeSet, error::Error, fs, path::Path};
pub const FIXTURE_BASENAME: &str = "native_amx_v2_grouped.json";
const GROUP_SOURCE_COUNT: usize = 2;
const VALIDATOR_COUNT: usize = 4;
const MIN_QUORUM: usize = VALIDATOR_COUNT - (VALIDATOR_COUNT - 1) / 3;
const APPLICATION_MANIFEST_LEAF_COUNT: u32 = 1;
const EXECUTED_BLOCK_WIRE_FIXTURE: &[u8] = b"native-amx-v2-grouped-fixture-executed-block-wire";
#[derive(Clone)]
struct ParticipantFixture {
    proposal: LaneBlockProposalV1,
    settlement: LaneBlockCommitment,
    settlement_hash: HashOf<LaneBlockCommitment>,
}
struct FixtureContext {
    keypairs: Vec<KeyPair>,
    validators: Vec<PeerId>,
    validator_set_pops: Vec<Vec<u8>>,
    validator_set_hash: HashOf<Vec<PeerId>>,
    round: ConsensusRound,
    epoch: u64,
    network_id: NetworkId,
    plan_digest: Hash,
    coordinator_lane_id: LaneId,
    coordinator_dataspace_id: DataSpaceId,
    coordinator_lane_incarnation: Hash,
    authority_context_height: u64,
    coordinator_lane_block_height: u64,
    coordinator_lane_block_view: u64,
    coordinator_proposal_hash: Hash,
    sources: [[u8; 32]; GROUP_SOURCE_COUNT],
    entrypoints: [HashOf<TransactionEntrypoint>; GROUP_SOURCE_COUNT],
}
fn fixture_context() -> Result<FixtureContext, Box<dyn Error>> {
    let mut keyed_validators = (1_u8..=u8::try_from(VALIDATOR_COUNT)?)
        .map(|seed| {
            let keypair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)?;
            let peer = PeerId::new(keypair.public_key().clone());
            Ok((peer, keypair))
        })
        .collect::<Result<Vec<_>, iroha_crypto::Error>>()?;
    keyed_validators.sort_by(|left, right| left.0.cmp(&right.0));
    let validators = keyed_validators
        .iter()
        .map(|(peer, _)| peer.clone())
        .collect::<Vec<_>>();
    let keypairs = keyed_validators
        .into_iter()
        .map(|(_, keypair)| keypair)
        .collect::<Vec<_>>();
    let validator_set_pops = keypairs
        .iter()
        .map(|keypair| iroha_crypto::bls_normal_pop_prove(keypair.private_key()))
        .collect::<Result<Vec<_>, _>>()?;
    let validator_set_hash = HashOf::new(&validators);
    Ok(FixtureContext {
        keypairs,
        validators,
        validator_set_pops,
        validator_set_hash,
        round: ConsensusRound {
            context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
                Hash::new(b"native-amx-v2-grouped-fixture-context"),
            )),
            height: 40,
            view: 6,
        },
        epoch: 3,
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            // Stable synthetic genesis-header hash seed used by the checked-in fixture.
            Hash::new(b"native-amx-v2-grouped-fixture-chain"),
        )),
        plan_digest: Hash::new(b"native-amx-v2-grouped-fixture-plan"),
        coordinator_lane_id: LaneId::new(7),
        coordinator_dataspace_id: DataSpaceId::new(11),
        coordinator_lane_incarnation: Hash::new(
            b"native-amx-v2-grouped-fixture-coordinator-incarnation",
        ),
        authority_context_height: 40,
        coordinator_lane_block_height: 42,
        // Deliberately independent from the global round view.
        coordinator_lane_block_view: 9,
        coordinator_proposal_hash: Hash::new(b"native-amx-v2-grouped-fixture-coordinator-proposal"),
        sources: [[0xAB; 32], [0xCD; 32]],
        entrypoints: [
            HashOf::from_untyped_unchecked(Hash::prehashed([0x61; 32])),
            HashOf::from_untyped_unchecked(Hash::prehashed([0x63; 32])),
        ],
    })
}
fn participant_incarnation(
    context: &FixtureContext,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> Hash {
    if lane_id == context.coordinator_lane_id && dataspace_id == context.coordinator_dataspace_id {
        context.coordinator_lane_incarnation
    } else {
        Hash::new(
            [
                b"native-amx-v2-grouped-fixture-participant-incarnation:".as_slice(),
                &lane_id.as_u32().to_be_bytes(),
                &dataspace_id.as_u64().to_be_bytes(),
            ]
            .concat(),
        )
    }
}
fn grouped_settlement(
    context: &FixtureContext,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
) -> Result<LaneBlockCommitment, Box<dyn Error>> {
    let receipts = context
        .sources
        .iter()
        .copied()
        .map(|source_id| LaneSettlementReceipt {
            source_id,
            local_amount: Quantity::zero(),
            xor_due: Quantity::zero(),
            xor_after_haircut: Quantity::zero(),
            xor_variance: Quantity::zero(),
            timestamp_ms: context.authority_context_height,
        })
        .collect::<Vec<_>>();
    Ok(LaneBlockCommitment {
        block_height: context.coordinator_lane_block_height,
        lane_id,
        lane_incarnation,
        dataspace_id,
        tx_count: u64::try_from(receipts.len())?,
        total_local_amount: Quantity::zero(),
        total_xor_due: Quantity::zero(),
        total_xor_after_haircut: Quantity::zero(),
        total_xor_variance: Quantity::zero(),
        swap_metadata: None,
        receipts,
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    })
}
fn participant_fixture(
    context: &FixtureContext,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_view: u64,
    lane_seed: u8,
) -> Result<ParticipantFixture, Box<dyn Error>> {
    let lane_incarnation = participant_incarnation(context, lane_id, dataspace_id);
    let settlement = grouped_settlement(context, lane_id, dataspace_id, lane_incarnation)?;
    let settlement_hash = compute_settlement_hash(&settlement)?;
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id,
        dataspace_id,
        lane_incarnation,
        proposal_height: context.authority_context_height,
        previous_lane_block_height: context.coordinator_lane_block_height - 1,
        previous_lane_block_descriptor_hash: Some(Hash::new(
            [
                b"native-amx-v2-grouped-fixture-predecessor:".as_slice(),
                &[lane_seed],
            ]
            .concat(),
        )),
        lane_block_height: context.coordinator_lane_block_height,
        lane_block_view,
        subject_hash: Hash::new(
            [
                b"native-amx-v2-grouped-fixture-subject:".as_slice(),
                &[lane_seed],
            ]
            .concat(),
        ),
        payload_ownership_hash: Hash::new(
            [
                b"native-amx-v2-grouped-fixture-ownership:".as_slice(),
                &[lane_seed],
            ]
            .concat(),
        ),
        rbc_instance_hash: Hash::new(
            [
                b"native-amx-v2-grouped-fixture-rbc:".as_slice(),
                &[lane_seed],
            ]
            .concat(),
        ),
        accepted_candidate_indices: vec![0, 1],
        accepted_transaction_hashes: context
            .entrypoints
            .iter()
            .copied()
            .map(Hash::from)
            .collect(),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: context.validator_set_hash,
        validator_set: context.validators.clone(),
        validator_count: u32::try_from(context.validators.len())?,
        min_quorum: u32::try_from(MIN_QUORUM)?,
        qc_mode_tag: "permissioned:native-amx-v2".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    Ok(ParticipantFixture {
        proposal,
        settlement,
        settlement_hash,
    })
}
fn body(
    context: &FixtureContext,
    participant: &ParticipantFixture,
    source_index: usize,
    phase: NativeAmxPhase,
) -> Result<NativeAmxAttestationBodyV2, Box<dyn Error>> {
    let descriptor = &participant.proposal.descriptor;
    Ok(NativeAmxAttestationBodyV2 {
        round: context.round,
        epoch: context.epoch,
        network_id: context.network_id,
        source_id: context.sources[source_index],
        tx_entrypoint_hash: context.entrypoints[source_index],
        plan_digest: context.plan_digest,
        phase,
        coordinator_lane_id: context.coordinator_lane_id,
        coordinator_dataspace_id: context.coordinator_dataspace_id,
        coordinator_lane_incarnation: context.coordinator_lane_incarnation,
        participant_lane_id: descriptor.lane_id,
        participant_dataspace_id: descriptor.dataspace_id,
        participant_lane_incarnation: descriptor.lane_incarnation,
        participant_previous_block_height: descriptor.previous_lane_block_height,
        participant_previous_block_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
        participant_lane_block_height: descriptor.lane_block_height,
        participant_lane_block_view: descriptor.lane_block_view,
        participant_proposal_hash: participant.proposal.proposal_hash,
        participant_settlement_commitment: Hash::from(participant.settlement_hash),
        participant_validator_set_hash: context.validator_set_hash,
        participant_validator_count: u32::try_from(context.validators.len())?,
        participant_min_quorum: u32::try_from(MIN_QUORUM)?,
        authority_context_height: context.authority_context_height,
        planned_coordinator_block_height: context.coordinator_lane_block_height,
        coordinator_lane_block_view: context.coordinator_lane_block_view,
        coordinator_proposal_hash: context.coordinator_proposal_hash,
    })
}
#[expect(
    clippy::large_types_passed_by_value,
    reason = "the QC takes ownership of the attestation body and stores it without a large clone"
)]
fn qc(
    context: &FixtureContext,
    body: NativeAmxAttestationBodyV2,
) -> Result<NativeAmxAttestationQcV2, Box<dyn Error>> {
    let signatures = context
        .keypairs
        .iter()
        .take(MIN_QUORUM)
        .map(|keypair| Signature::try_new(keypair.private_key(), &body.signature_preimage()))
        .collect::<Result<Vec<_>, _>>()?;
    let signature_payloads = signatures
        .iter()
        .map(Signature::payload)
        .collect::<Vec<_>>();
    let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_payloads)?;
    let mut signers_bitmap = vec![0_u8; context.validators.len().div_ceil(8)];
    for index in 0..MIN_QUORUM {
        signers_bitmap[index / 8] |= 1_u8 << (index % 8);
    }
    Ok(NativeAmxAttestationQcV2::try_new(
        body,
        VALIDATOR_SET_HASH_VERSION_V1,
        context.validator_set_hash,
        context.validators.clone(),
        context.validator_set_pops.clone(),
        signers_bitmap,
        aggregate_signature,
    )?)
}
#[expect(
    clippy::large_types_passed_by_value,
    reason = "the rebuilt attestation body is consumed directly by the isolated control QC"
)]
fn control_qc(
    context: &FixtureContext,
    body: NativeAmxAttestationBodyV2,
    validator_set: Vec<PeerId>,
    validator_set_pops: Vec<Vec<u8>>,
    signer_keys: &[(usize, usize)],
) -> Result<NativeAmxAttestationQcV2, Box<dyn Error>> {
    let signatures = signer_keys
        .iter()
        .map(|(_, key_index)| -> Result<Signature, Box<dyn Error>> {
            let keypair = context
                .keypairs
                .get(*key_index)
                .ok_or("Native AMX control signer key index is out of bounds")?;
            Ok(Signature::try_new(
                keypair.private_key(),
                &body.signature_preimage(),
            )?)
        })
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
    let signature_payloads = signatures
        .iter()
        .map(Signature::payload)
        .collect::<Vec<_>>();
    let aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_payloads)?;
    let mut signers_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
    for (validator_index, _) in signer_keys {
        if *validator_index >= validator_set.len() {
            return Err("Native AMX control signer index is out of bounds".into());
        }
        signers_bitmap[*validator_index / 8] |= 1_u8 << (*validator_index % 8);
    }
    let validator_set_hash = HashOf::new(&validator_set);
    Ok(NativeAmxAttestationQcV2::try_new(
        body,
        VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash,
        validator_set,
        validator_set_pops,
        signers_bitmap,
        aggregate_signature,
    )?)
}
fn rebuild_control_leg_committee(
    context: &FixtureContext,
    source: &NativeAmxLegRecordV2,
    validator_set: Vec<PeerId>,
    validator_set_pops: Vec<Vec<u8>>,
    min_quorum: usize,
    signer_keys: &[(usize, usize)],
) -> Result<NativeAmxLegRecordV2, Box<dyn Error>> {
    let validator_count = u32::try_from(validator_set.len())?;
    let min_quorum = u32::try_from(min_quorum)?;
    let validator_set_hash = HashOf::new(&validator_set);
    let mut leg = source.clone();
    leg.participant_proposal.descriptor.validator_set_hash = validator_set_hash;
    leg.participant_proposal.descriptor.validator_set = validator_set.clone();
    leg.participant_proposal.descriptor.validator_count = validator_count;
    leg.participant_proposal.descriptor.min_quorum = min_quorum;
    leg.participant_proposal.descriptor.descriptor_hash = leg
        .participant_proposal
        .descriptor
        .computed_descriptor_hash();
    leg.participant_proposal.proposal_hash = leg.participant_proposal.computed_proposal_hash();
    let proposal_hash = leg.participant_proposal.proposal_hash;
    let mut prepare_body = leg.prepare_qc.body;
    prepare_body.participant_validator_set_hash = validator_set_hash;
    prepare_body.participant_validator_count = validator_count;
    prepare_body.participant_min_quorum = min_quorum;
    prepare_body.participant_proposal_hash = proposal_hash;
    let mut commit_body = leg.commit_qc.body;
    commit_body.participant_validator_set_hash = validator_set_hash;
    commit_body.participant_validator_count = validator_count;
    commit_body.participant_min_quorum = min_quorum;
    commit_body.participant_proposal_hash = proposal_hash;
    leg.prepare_qc = control_qc(
        context,
        prepare_body,
        validator_set.clone(),
        validator_set_pops.clone(),
        signer_keys,
    )?;
    leg.commit_qc = control_qc(
        context,
        commit_body,
        validator_set,
        validator_set_pops,
        signer_keys,
    )?;
    Ok(leg)
}
fn leg(
    context: &FixtureContext,
    participant: &ParticipantFixture,
    source_index: usize,
) -> Result<NativeAmxLegRecordV2, Box<dyn Error>> {
    Ok(NativeAmxLegRecordV2 {
        lane_id: participant.proposal.descriptor.lane_id,
        dataspace_id: participant.proposal.descriptor.dataspace_id,
        participant_proposal: participant.proposal.clone(),
        participant_settlement: participant.settlement.clone(),
        participant_settlement_hash: participant.settlement_hash,
        prepare_qc: qc(
            context,
            body(context, participant, source_index, NativeAmxPhase::Prepare)?,
        )?,
        commit_qc: qc(
            context,
            body(context, participant, source_index, NativeAmxPhase::Commit)?,
        )?,
    })
}
fn receipt(
    context: &FixtureContext,
    participants: &[ParticipantFixture],
    source_index: usize,
) -> Result<NativeAmxReceipt, Box<dyn Error>> {
    Ok(NativeAmxReceipt {
        version: 2,
        source_id: context.sources[source_index],
        network_id: context.network_id,
        plan_digest: context.plan_digest,
        lane_id: context.coordinator_lane_id,
        dataspace_id: context.coordinator_dataspace_id,
        lane_incarnation: context.coordinator_lane_incarnation,
        authority_context_height: context.authority_context_height,
        lane_block_height: context.coordinator_lane_block_height,
        lane_block_view: context.coordinator_lane_block_view,
        coordinator_proposal_hash: context.coordinator_proposal_hash,
        legs: participants
            .iter()
            .map(|participant| leg(context, participant, source_index))
            .collect::<Result<Vec<_>, _>>()?,
    })
}
fn golden_commitment() -> Result<(LaneBlockCommitment, ParticipantFixture), Box<dyn Error>> {
    let mut context = fixture_context()?;
    let coordinator = participant_fixture(
        &context,
        context.coordinator_lane_id,
        context.coordinator_dataspace_id,
        context.coordinator_lane_block_view,
        0x71,
    )?;
    context.coordinator_proposal_hash = coordinator.proposal.proposal_hash;
    let remote = participant_fixture(&context, LaneId::new(8), DataSpaceId::new(12), 0, 0x81)?;
    let participants = vec![coordinator, remote];
    let native_amx_receipts = (0..GROUP_SOURCE_COUNT)
        .map(|source_index| receipt(&context, &participants, source_index))
        .collect::<Result<Vec<_>, _>>()?;
    let commitment = LaneBlockCommitment {
        block_height: context.coordinator_lane_block_height,
        lane_id: context.coordinator_lane_id,
        lane_incarnation: context.coordinator_lane_incarnation,
        dataspace_id: context.coordinator_dataspace_id,
        tx_count: u64::try_from(GROUP_SOURCE_COUNT)?,
        total_local_amount: Quantity::zero(),
        total_xor_due: Quantity::zero(),
        total_xor_after_haircut: Quantity::zero(),
        total_xor_variance: Quantity::zero(),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts,
    };
    Ok((commitment, participants[1].clone()))
}
fn diagnostics(
    commitment: LaneBlockCommitment,
    remote: &ParticipantFixture,
) -> SumeragiDiagnosticsStatus {
    SumeragiDiagnosticsStatus {
        pipeline_execution: SumeragiPipelineExecutionStatus::default(),
        tx_queue_depth: 0,
        tx_queue_capacity: 1,
        tx_queue_retained_bytes: 0,
        tx_queue_max_retained_bytes: 1,
        tx_queue_saturated: false,
        tx_queue_saturated_by_count: false,
        tx_queue_saturated_by_bytes: false,
        tx_queue_saturated_by_age: false,
        tx_queue_oldest_queued_age_ms: 0,
        npos: None,
        lane_commitments: Vec::new(),
        dataspace_commitments: Vec::new(),
        lane_settlement_commitments: vec![commitment],
        lane_relay_envelopes: Vec::new(),
        lane_payload_ownerships: Vec::new(),
        committed_lane_blocks: Vec::new(),
        lane_block_sessions: Vec::new(),
        lane_governance_sealed_total: 0,
        lane_governance_sealed_aliases: Vec::new(),
        lane_governance: Vec::new(),
        native_amx_participant_applications: vec![participant_application(remote)],
        autonomous_lane_executions: Vec::new(),
    }
}
fn participant_application(remote: &ParticipantFixture) -> SumeragiNativeAmxParticipantApplication {
    let descriptor = &remote.proposal.descriptor;
    SumeragiNativeAmxParticipantApplication {
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
        participant_height: descriptor.lane_block_height,
        participant_view: descriptor.lane_block_view,
        predecessor_height: descriptor.previous_lane_block_height,
        predecessor_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
        descriptor_hash: descriptor.descriptor_hash,
        proposal_hash: remote.proposal.proposal_hash,
        settlement_hash: remote.settlement_hash,
        source_count: GROUP_SOURCE_COUNT as u64,
        application_block_height: Some(42),
        application_block_hash: Some(application_block_hash()),
        state: SumeragiNativeAmxParticipantApplicationState::DurablyApplied,
    }
}
fn same_participant_fixture(left: &ParticipantFixture, right: &ParticipantFixture) -> bool {
    left.proposal == right.proposal
        && left.settlement == right.settlement
        && left.settlement_hash == right.settlement_hash
}
fn application_block_hash() -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(Hash::new(
        b"native-amx-v2-grouped-fixture-application-block",
    ))
}
fn executed_block_wire_hash() -> Hash {
    Hash::new(EXECUTED_BLOCK_WIRE_FIXTURE)
}
fn executed_block_wire_len() -> u64 {
    u64::try_from(EXECUTED_BLOCK_WIRE_FIXTURE.len()).expect("grouped fixture wire length fits u64")
}
fn application_evidence(
    context: &FixtureContext,
    remote: &ParticipantFixture,
) -> Result<Value, Box<dyn Error>> {
    let descriptor = &remote.proposal.descriptor;
    let members = context
        .sources
        .iter()
        .copied()
        .zip(context.entrypoints.iter().copied())
        .enumerate()
        .map(
            |(index, (source_id, entrypoint_hash))| NativeAmxApplicationManifestMemberV1 {
                entrypoint_index: descriptor.accepted_candidate_indices[index],
                source_id,
                entrypoint_hash,
                result_hash: HashOf::<TransactionResult>::from_untyped_unchecked(Hash::new(
                    [
                        b"native-amx-v2-grouped-fixture-result:".as_slice(),
                        &[u8::try_from(index).expect("fixture member index fits u8")],
                    ]
                    .concat(),
                )),
            },
        )
        .collect::<Vec<_>>();
    let leaf = NativeAmxApplicationManifestLeafV1 {
        version: NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
        participant_height: descriptor.lane_block_height,
        participant_view: descriptor.lane_block_view,
        predecessor_height: descriptor.previous_lane_block_height,
        predecessor_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
        descriptor_hash: descriptor.descriptor_hash,
        proposal_hash: remote.proposal.proposal_hash,
        settlement_hash: remote.settlement_hash,
        members,
        application_block_height: 42,
        application_block_hash: application_block_hash(),
        executed_block_wire_hash: executed_block_wire_hash(),
    };
    leaf.validate()?;
    let leaf_hash = HashOf::new(&leaf);
    let tree = [leaf_hash].into_iter().collect::<MerkleTree<_>>();
    let manifest_commitment = tree
        .commitment()
        .ok_or("singleton Native AMX manifest must have a commitment")?;
    if manifest_commitment.leaf_count().get() != u64::from(APPLICATION_MANIFEST_LEAF_COUNT) {
        return Err("Native AMX manifest tree count differs from the execution commitment".into());
    }
    let manifest_root = Hash::from(*manifest_commitment.root());
    let proof = tree
        .get_proof(0)
        .ok_or("singleton Native AMX manifest must have a proof")?;
    if !proof.verify(&leaf_hash, &manifest_commitment) {
        return Err("generated Native AMX manifest proof does not verify".into());
    }
    let execution_commitment =
        ExecutionCommitment::new_with_native_amx_application_manifest_and_merge_carrier(
            Hash::new(b"native-amx-v2-grouped-fixture-parent-state"),
            Hash::new(b"native-amx-v2-grouped-fixture-post-state"),
            Hash::new(b"native-amx-v2-grouped-fixture-ordinary-writes"),
            None,
            0,
            NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            manifest_root,
            APPLICATION_MANIFEST_LEAF_COUNT,
            Some(MergeCarrierCommitmentV1::new(
                HashOf::<MergeLedgerEntry>::from_untyped_unchecked(Hash::new(
                    b"native-amx-v2-grouped-fixture-merge-carrier",
                )),
            )),
            executed_block_wire_len(),
            executed_block_wire_hash(),
        )?;
    let carrier_entrypoint_hashes = context
        .entrypoints
        .iter()
        .copied()
        .map(Hash::from)
        .collect::<Vec<_>>();
    Ok(norito::json!({
        "active_lane_incarnations": [{
            "lane_id": (descriptor.lane_id),
            "dataspace_id": (descriptor.dataspace_id),
            "lane_incarnation": (descriptor.lane_incarnation),
        }],
        "carrier_entrypoint_hashes": (carrier_entrypoint_hashes),
        "execution_commitment": (execution_commitment),
        "manifest_artifacts": [{
            "version": 1,
            "leaf": (leaf),
            "leaf_hash": (Hash::from(leaf_hash)),
            "leaf_index": 0,
            "proof": (proof),
            "manifest_root": (manifest_root),
            "manifest_leaf_count": (APPLICATION_MANIFEST_LEAF_COUNT),
        }],
    }))
}
fn mutation(op: &str, path: &str, value: Option<Value>) -> Value {
    value.map_or_else(
        || {
            norito::json!({
                "op": (op),
                "path": (path),
            })
        },
        |value| {
            norito::json!({
            "op": (op),
            "path": (path),
            "value": (value),
            })
        },
    )
}
#[expect(
    clippy::needless_pass_by_value,
    reason = "the fixture helper owns the mutation vector so callers do not clone its JSON values"
)]
fn controls(id: &str, validator: &str, mutations: Vec<Value>) -> Value {
    norito::json!({
        "id": (id),
        "expectation": "reject",
        "validator": (validator),
        "mutations": (mutations),
    })
}
fn control(id: &str, mutation: Value) -> Value {
    controls(id, "receipt_group", vec![mutation])
}
fn evidence_control(id: &str, mutations: Vec<Value>) -> Value {
    controls(id, "application_evidence", mutations)
}
#[expect(
    clippy::too_many_lines,
    reason = "both coherent committee controls rebuild every grouped source and evidence projection together"
)]
fn committee_consistency_controls(
    context: &FixtureContext,
    commitment: &LaneBlockCommitment,
) -> Result<Vec<Value>, Box<dyn Error>> {
    let first_receipt = commitment
        .native_amx_receipts
        .first()
        .ok_or("Native AMX committee controls require a grouped receipt")?;
    let mut first_remote_legs = first_receipt.legs.iter().filter(|leg| {
        leg.lane_id != commitment.lane_id || leg.dataspace_id != commitment.dataspace_id
    });
    let first_remote_leg = first_remote_legs
        .next()
        .ok_or("Native AMX committee controls require a remote participant leg")?;
    if first_remote_legs.next().is_some() {
        return Err("Native AMX committee controls require one remote participant route".into());
    }
    let remote_route = (first_remote_leg.lane_id, first_remote_leg.dataspace_id);
    let canonical_validators = first_remote_leg.prepare_qc.validator_set().to_vec();
    let canonical_pops = first_remote_leg.prepare_qc.validator_set_pops().to_vec();
    if canonical_validators.len() != VALIDATOR_COUNT || canonical_pops.len() != VALIDATOR_COUNT {
        return Err("Native AMX committee-control fixture geometry changed".into());
    }
    let mut duplicate_validators = canonical_validators.clone();
    duplicate_validators[1] = duplicate_validators[0].clone();
    let mut duplicate_pops = canonical_pops.clone();
    duplicate_pops[1] = duplicate_pops[0].clone();
    let duplicate_signer_keys = (0..MIN_QUORUM)
        .map(|validator_index| {
            let key_index = if validator_index == 1 {
                0
            } else {
                validator_index
            };
            (validator_index, key_index)
        })
        .collect::<Vec<_>>();
    let over_quorum_signer_keys = (0..VALIDATOR_COUNT)
        .map(|index| (index, index))
        .collect::<Vec<_>>();
    let mut duplicate_mutations = Vec::with_capacity(commitment.native_amx_receipts.len() + 2);
    let mut over_quorum_mutations = Vec::with_capacity(commitment.native_amx_receipts.len() + 2);
    let mut duplicate_commitment = commitment.clone();
    let mut over_quorum_commitment = commitment.clone();
    let mut duplicate_remote: Option<ParticipantFixture> = None;
    let mut over_quorum_remote: Option<ParticipantFixture> = None;
    for (receipt_index, receipt) in commitment.native_amx_receipts.iter().enumerate() {
        let mut matching_legs = receipt
            .legs
            .iter()
            .enumerate()
            .filter(|(_, leg)| (leg.lane_id, leg.dataspace_id) == remote_route);
        let (leg_index, remote_leg) = matching_legs
            .next()
            .ok_or("Native AMX grouped receipt is missing the remote participant leg")?;
        if matching_legs.next().is_some() {
            return Err("Native AMX grouped receipt repeats the remote participant leg".into());
        }
        if remote_leg.prepare_qc.body.source_id != receipt.source_id
            || remote_leg.commit_qc.body.source_id != receipt.source_id
            || remote_leg.prepare_qc.validator_set() != canonical_validators.as_slice()
            || remote_leg.commit_qc.validator_set() != canonical_validators.as_slice()
            || remote_leg.prepare_qc.validator_set_pops() != canonical_pops.as_slice()
            || remote_leg.commit_qc.validator_set_pops() != canonical_pops.as_slice()
        {
            return Err("Native AMX grouped remote committee is not source-coherent".into());
        }
        let duplicate_leg = rebuild_control_leg_committee(
            context,
            remote_leg,
            duplicate_validators.clone(),
            duplicate_pops.clone(),
            MIN_QUORUM,
            &duplicate_signer_keys,
        )?;
        let over_quorum_leg = rebuild_control_leg_committee(
            context,
            remote_leg,
            canonical_validators.clone(),
            canonical_pops.clone(),
            MIN_QUORUM + 1,
            &over_quorum_signer_keys,
        )?;
        let duplicate_participant = ParticipantFixture {
            proposal: duplicate_leg.participant_proposal.clone(),
            settlement: duplicate_leg.participant_settlement.clone(),
            settlement_hash: duplicate_leg.participant_settlement_hash,
        };
        let over_quorum_participant = ParticipantFixture {
            proposal: over_quorum_leg.participant_proposal.clone(),
            settlement: over_quorum_leg.participant_settlement.clone(),
            settlement_hash: over_quorum_leg.participant_settlement_hash,
        };
        if duplicate_remote
            .as_ref()
            .is_some_and(|expected| !same_participant_fixture(expected, &duplicate_participant))
            || over_quorum_remote.as_ref().is_some_and(|expected| {
                !same_participant_fixture(expected, &over_quorum_participant)
            })
        {
            return Err("Native AMX committee control diverged across grouped sources".into());
        }
        if duplicate_remote.is_none() {
            duplicate_remote = Some(duplicate_participant);
        }
        if over_quorum_remote.is_none() {
            over_quorum_remote = Some(over_quorum_participant);
        }
        duplicate_commitment.native_amx_receipts[receipt_index].legs[leg_index] =
            duplicate_leg.clone();
        over_quorum_commitment.native_amx_receipts[receipt_index].legs[leg_index] =
            over_quorum_leg.clone();
        let remote_leg_path =
            format!("/golden/receipt_group/native_amx_receipts/{receipt_index}/legs/{leg_index}");
        duplicate_mutations.push(mutation(
            "replace",
            &remote_leg_path,
            Some(json::to_value(&duplicate_leg)?),
        ));
        over_quorum_mutations.push(mutation(
            "replace",
            &remote_leg_path,
            Some(json::to_value(&over_quorum_leg)?),
        ));
    }
    let duplicate_remote =
        duplicate_remote.ok_or("Native AMX duplicate-committee control has no participant")?;
    let over_quorum_remote =
        over_quorum_remote.ok_or("Native AMX over-quorum control has no participant")?;
    duplicate_mutations.push(mutation(
        "replace",
        "/golden/application_evidence",
        Some(application_evidence(context, &duplicate_remote)?),
    ));
    duplicate_mutations.push(mutation(
        "replace",
        "/golden/expected_diagnostics",
        Some(json::to_value(&diagnostics(
            duplicate_commitment,
            &duplicate_remote,
        ))?),
    ));
    over_quorum_mutations.push(mutation(
        "replace",
        "/golden/application_evidence",
        Some(application_evidence(context, &over_quorum_remote)?),
    ));
    over_quorum_mutations.push(mutation(
        "replace",
        "/golden/expected_diagnostics",
        Some(json::to_value(&diagnostics(
            over_quorum_commitment,
            &over_quorum_remote,
        ))?),
    ));
    Ok(vec![
        controls(
            "coherent_duplicate_validator_set",
            "receipt_group",
            duplicate_mutations,
        ),
        controls(
            "coherent_over_quorum_requirement",
            "receipt_group",
            over_quorum_mutations,
        ),
    ])
}
#[expect(
    clippy::too_many_lines,
    reason = "the five coherent hash-consistency controls stay together so their linked descriptor, proposal, and quorum-certificate mutations remain auditable"
)]
fn hash_consistency_controls(commitment: &LaneBlockCommitment) -> Vec<Value> {
    let remote_leg = &commitment.native_amx_receipts[0].legs[1];
    let leg = "/golden/receipt_group/native_amx_receipts/0/legs/1";
    let descriptor = format!("{leg}/participant_proposal/descriptor");
    let proposal = format!("{leg}/participant_proposal");
    let prepare = format!("{leg}/prepare_qc");
    let commit = format!("{leg}/commit_qc");
    let forged_validator_set_hash = HashOf::<Vec<PeerId>>::from_untyped_unchecked(Hash::new(
        b"native-amx-v2-negative-forged-validator-set-hash",
    ));
    let mut validator_hash_leg = remote_leg.clone();
    validator_hash_leg
        .participant_proposal
        .descriptor
        .validator_set_hash = forged_validator_set_hash;
    validator_hash_leg
        .participant_proposal
        .descriptor
        .descriptor_hash = validator_hash_leg
        .participant_proposal
        .descriptor
        .computed_descriptor_hash();
    validator_hash_leg.participant_proposal.proposal_hash = validator_hash_leg
        .participant_proposal
        .computed_proposal_hash();
    for qc in [
        &mut validator_hash_leg.prepare_qc,
        &mut validator_hash_leg.commit_qc,
    ] {
        qc.validator_set_hash = forged_validator_set_hash;
        qc.body.participant_validator_set_hash = forged_validator_set_hash;
        qc.body.participant_proposal_hash = validator_hash_leg.participant_proposal.proposal_hash;
    }
    let mut stale_descriptor_leg = remote_leg.clone();
    stale_descriptor_leg
        .participant_proposal
        .descriptor
        .subject_hash = Hash::new(b"native-amx-v2-negative-stale-descriptor-subject");
    stale_descriptor_leg.participant_proposal.proposal_hash = stale_descriptor_leg
        .participant_proposal
        .computed_proposal_hash();
    for qc in [
        &mut stale_descriptor_leg.prepare_qc,
        &mut stale_descriptor_leg.commit_qc,
    ] {
        qc.body.participant_proposal_hash = stale_descriptor_leg.participant_proposal.proposal_hash;
    }
    let forged_proposal_hash = Hash::new(b"native-amx-v2-negative-forged-proposal-hash");
    let retired_plain_settlement_hash = HashOf::new(&remote_leg.participant_settlement);
    vec![
        controls(
            "coherent_forged_validator_set_hash",
            "receipt_group",
            vec![
                mutation(
                    "replace",
                    &format!("{descriptor}/validator_set_hash"),
                    Some(
                        json::to_value(
                            &validator_hash_leg
                                .participant_proposal
                                .descriptor
                                .validator_set_hash,
                        )
                        .expect("validator-set hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{descriptor}/descriptor_hash"),
                    Some(
                        json::to_value(
                            &validator_hash_leg
                                .participant_proposal
                                .descriptor
                                .descriptor_hash,
                        )
                        .expect("descriptor hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{proposal}/proposal_hash"),
                    Some(
                        json::to_value(&validator_hash_leg.participant_proposal.proposal_hash)
                            .expect("proposal hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{prepare}/validator_set_hash"),
                    Some(
                        json::to_value(&validator_hash_leg.prepare_qc.validator_set_hash)
                            .expect("prepare validator-set hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{prepare}/body/participant_validator_set_hash"),
                    Some(
                        json::to_value(
                            &validator_hash_leg
                                .prepare_qc
                                .body
                                .participant_validator_set_hash,
                        )
                        .expect("prepare body validator-set hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{prepare}/body/participant_proposal_hash"),
                    Some(
                        json::to_value(
                            &validator_hash_leg.prepare_qc.body.participant_proposal_hash,
                        )
                        .expect("prepare body proposal hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{commit}/validator_set_hash"),
                    Some(
                        json::to_value(&validator_hash_leg.commit_qc.validator_set_hash)
                            .expect("commit validator-set hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{commit}/body/participant_validator_set_hash"),
                    Some(
                        json::to_value(
                            &validator_hash_leg
                                .commit_qc
                                .body
                                .participant_validator_set_hash,
                        )
                        .expect("commit body validator-set hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{commit}/body/participant_proposal_hash"),
                    Some(
                        json::to_value(
                            &validator_hash_leg.commit_qc.body.participant_proposal_hash,
                        )
                        .expect("commit body proposal hash serializes"),
                    ),
                ),
            ],
        ),
        controls(
            "coherent_stale_descriptor_hash",
            "receipt_group",
            vec![
                mutation(
                    "replace",
                    &format!("{descriptor}/subject_hash"),
                    Some(
                        json::to_value(
                            &stale_descriptor_leg
                                .participant_proposal
                                .descriptor
                                .subject_hash,
                        )
                        .expect("descriptor subject hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{proposal}/proposal_hash"),
                    Some(
                        json::to_value(&stale_descriptor_leg.participant_proposal.proposal_hash)
                            .expect("stale-descriptor proposal hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{prepare}/body/participant_proposal_hash"),
                    Some(
                        json::to_value(
                            &stale_descriptor_leg
                                .prepare_qc
                                .body
                                .participant_proposal_hash,
                        )
                        .expect("stale-descriptor prepare proposal hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{commit}/body/participant_proposal_hash"),
                    Some(
                        json::to_value(
                            &stale_descriptor_leg
                                .commit_qc
                                .body
                                .participant_proposal_hash,
                        )
                        .expect("stale-descriptor commit proposal hash serializes"),
                    ),
                ),
            ],
        ),
        controls(
            "coherent_stale_proposal_hash",
            "receipt_group",
            vec![
                mutation(
                    "replace",
                    &format!("{proposal}/proposal_hash"),
                    Some(
                        json::to_value(&forged_proposal_hash)
                            .expect("forged proposal hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{prepare}/body/participant_proposal_hash"),
                    Some(
                        json::to_value(&forged_proposal_hash)
                            .expect("forged prepare proposal hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{commit}/body/participant_proposal_hash"),
                    Some(
                        json::to_value(&forged_proposal_hash)
                            .expect("forged commit proposal hash serializes"),
                    ),
                ),
            ],
        ),
        controls(
            "coherent_stale_settlement_hash",
            "receipt_group",
            vec![
                mutation(
                    "replace",
                    &format!("{leg}/participant_settlement_hash"),
                    Some(
                        json::to_value(&retired_plain_settlement_hash)
                            .expect("retired plain settlement hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{prepare}/body/participant_settlement_commitment"),
                    Some(
                        json::to_value(&Hash::from(retired_plain_settlement_hash))
                            .expect("retired plain prepare settlement hash serializes"),
                    ),
                ),
                mutation(
                    "replace",
                    &format!("{commit}/body/participant_settlement_commitment"),
                    Some(
                        json::to_value(&Hash::from(retired_plain_settlement_hash))
                            .expect("retired plain commit settlement hash serializes"),
                    ),
                ),
            ],
        ),
        controls(
            "non_canonical_validator_peer_id",
            "receipt_group",
            vec![
                mutation(
                    "replace",
                    &format!("{descriptor}/validator_set/3"),
                    Some(norito::json!("not-a-canonical-bls-peer-id")),
                ),
                mutation(
                    "replace",
                    &format!("{prepare}/validator_set/3"),
                    Some(norito::json!("not-a-canonical-bls-peer-id")),
                ),
                mutation(
                    "replace",
                    &format!("{commit}/validator_set/3"),
                    Some(norito::json!("not-a-canonical-bls-peer-id")),
                ),
            ],
        ),
    ]
}
#[expect(
    clippy::too_many_lines,
    reason = "the compact negative-control corpus is easier to audit as one ordered list"
)]
fn negative_controls(
    context: &FixtureContext,
    commitment: &LaneBlockCommitment,
) -> Result<Vec<Value>, Box<dyn Error>> {
    let receipt = "/golden/receipt_group/native_amx_receipts";
    let first = format!("{receipt}/0");
    let second = format!("{receipt}/1");
    let first_leg = format!("{first}/legs/0");
    let prepare = format!("{first_leg}/prepare_qc");
    let commit = format!("{first_leg}/commit_qc");
    let settlement = format!("{first_leg}/participant_settlement");
    let same_route_descriptor = format!("{first_leg}/participant_proposal/descriptor");
    let stale_incarnation = json::to_value(&Hash::new(
        b"native-amx-v2-negative-stale-participant-incarnation",
    ))
    .expect("hash serializes to JSON");
    let foreign_entrypoint = json::to_value(&Hash::new(
        b"native-amx-v2-negative-unanchored-mixed-role-entrypoint",
    ))
    .expect("hash serializes to JSON");
    let forged_hash = json::to_value(&Hash::new(b"native-amx-v2-negative-forged-manifest-value"))
        .expect("hash serializes to JSON");
    let coordinator_incarnation = json::to_value(&Hash::new(
        b"native-amx-v2-grouped-fixture-coordinator-incarnation",
    ))
    .expect("hash serializes to JSON");
    let mut controls = vec![
        control(
            "flattened_phase",
            mutation(
                "replace",
                &format!("{prepare}/body/phase"),
                Some(norito::json!("prepare")),
            ),
        ),
        control(
            "wrong_prepare_phase",
            mutation(
                "replace",
                &format!("{prepare}/body/phase"),
                Some(norito::json!({"phase": "commit", "detail": null})),
            ),
        ),
        control(
            "source_entrypoint_drift",
            mutation(
                "replace",
                &format!("{commit}/body/tx_entrypoint_hash"),
                Some(norito::json!(
                    Hash::prehashed([0xE1; Hash::LENGTH]).to_string()
                )),
            ),
        ),
        control(
            "round_context_drift",
            mutation(
                "replace",
                &format!("{commit}/body/round/context_id"),
                Some(norito::json!(vec![
                    Hash::new(b"native-amx-v2-negative-context").to_string()
                ])),
            ),
        ),
        control(
            "unordered_validator_set",
            mutation(
                "swap",
                &format!("{prepare}/validator_set"),
                Some(norito::json!({"left": 0, "right": 1})),
            ),
        ),
        controls(
            "coherent_unordered_validator_set",
            "receipt_group",
            vec![
                mutation(
                    "swap",
                    &format!("{prepare}/validator_set"),
                    Some(norito::json!({"left": 0, "right": 1})),
                ),
                mutation(
                    "swap",
                    &format!("{prepare}/validator_set_pops"),
                    Some(norito::json!({"left": 0, "right": 1})),
                ),
                mutation(
                    "swap",
                    &format!("{commit}/validator_set"),
                    Some(norito::json!({"left": 0, "right": 1})),
                ),
                mutation(
                    "swap",
                    &format!("{commit}/validator_set_pops"),
                    Some(norito::json!({"left": 0, "right": 1})),
                ),
                mutation(
                    "swap",
                    &format!("{same_route_descriptor}/validator_set"),
                    Some(norito::json!({"left": 0, "right": 1})),
                ),
            ],
        ),
        control(
            "under_quorum_bitmap",
            mutation(
                "replace",
                &format!("{prepare}/signers_bitmap"),
                Some(norito::json!([3])),
            ),
        ),
        control(
            "out_of_range_bitmap",
            mutation(
                "replace",
                &format!("{prepare}/signers_bitmap"),
                Some(norito::json!([135])),
            ),
        ),
        control(
            "short_pop",
            mutation(
                "replace",
                &format!("{prepare}/validator_set_pops/0"),
                Some(norito::json!(vec![
                    0x5A_u64;
                    NATIVE_AMX_BLS_PROOF_BYTES - 1
                ])),
            ),
        ),
        controls(
            "zero_pop",
            "receipt_group",
            vec![
                mutation(
                    "replace",
                    &format!("{prepare}/validator_set_pops/0/0"),
                    Some(norito::json!(0)),
                ),
                mutation(
                    "repeat",
                    &format!("{prepare}/validator_set_pops/0"),
                    Some(norito::json!({
                        "source_index": 0,
                        "count": (NATIVE_AMX_BLS_PROOF_BYTES)
                    })),
                ),
            ],
        ),
        control(
            "long_pop",
            mutation(
                "repeat",
                &format!("{prepare}/validator_set_pops/0"),
                Some(norito::json!({
                    "source_index": 0,
                    "count": (NATIVE_AMX_BLS_PROOF_BYTES + 1)
                })),
            ),
        ),
        control(
            "short_aggregate_signature",
            mutation(
                "replace",
                &format!("{prepare}/bls_aggregate_signature"),
                Some(norito::json!(vec![
                    0x5A_u64;
                    NATIVE_AMX_BLS_PROOF_BYTES - 1
                ])),
            ),
        ),
        control(
            "zero_aggregate_signature",
            mutation(
                "replace",
                &format!("{prepare}/bls_aggregate_signature"),
                Some(norito::json!(vec![0_u64; NATIVE_AMX_BLS_PROOF_BYTES])),
            ),
        ),
        control(
            "long_aggregate_signature",
            mutation(
                "repeat",
                &format!("{prepare}/bls_aggregate_signature"),
                Some(norito::json!({
                    "source_index": 0,
                    "count": (NATIVE_AMX_BLS_PROOF_BYTES + 1)
                })),
            ),
        ),
        control(
            "duplicate_participant_leg",
            mutation(
                "copy",
                &format!("{first}/legs/1"),
                Some(norito::json!({"from": (format!("{first}/legs/0"))})),
            ),
        ),
        control(
            "empty_participant_legs",
            mutation("replace", &format!("{first}/legs"), Some(norito::json!([]))),
        ),
        control(
            "participant_leg_overflow",
            mutation(
                "repeat",
                &format!("{first}/legs"),
                Some(norito::json!({
                    "source_index": 0,
                    "count": (NATIVE_AMX_PARTICIPANT_LEGS_MAX + 1)
                })),
            ),
        ),
        control(
            "duplicate_group_source",
            mutation(
                "copy",
                &format!("{settlement}/receipts/1/source_id"),
                Some(norito::json!({"from": (format!("{settlement}/receipts/0/source_id"))})),
            ),
        ),
        control(
            "missing_current_source",
            mutation(
                "replace",
                &format!("{settlement}/receipts/0/source_id"),
                Some(norito::json!(
                    "EFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEF"
                )),
            ),
        ),
        control(
            "group_tx_count_mismatch",
            mutation(
                "replace",
                &format!("{settlement}/tx_count"),
                Some(norito::json!(1)),
            ),
        ),
        control(
            "group_source_overflow",
            mutation(
                "repeat",
                &format!("{settlement}/receipts"),
                Some(norito::json!({
                    "source_index": 0,
                    "count": (NATIVE_AMX_GROUP_SOURCES_MAX + 1)
                })),
            ),
        ),
        control(
            "participant_timestamp_drift",
            mutation(
                "replace",
                &format!("{settlement}/receipts/1/timestamp_ms"),
                Some(norito::json!(41)),
            ),
        ),
        control(
            "nonzero_participant_effect",
            mutation(
                "replace",
                &format!("{settlement}/total_local_amount"),
                Some(norito::json!("1")),
            ),
        ),
        control(
            "nested_native_receipt",
            mutation(
                "replace",
                &format!("{settlement}/native_amx_receipts"),
                Some(norito::json!([{}])),
            ),
        ),
        control(
            "nested_fee_receipt",
            mutation(
                "replace",
                &format!("{settlement}/nexus_fee_receipts"),
                Some(norito::json!([{}])),
            ),
        ),
        control(
            "predecessor_hash_missing",
            mutation(
                "remove",
                &format!(
                    "{first_leg}/participant_proposal/descriptor/previous_lane_block_descriptor_hash"
                ),
                None,
            ),
        ),
        control(
            "receipt_source_duplicate",
            mutation(
                "copy",
                &format!("{receipt}/1/source_id"),
                Some(norito::json!({"from": (format!("{receipt}/0/source_id"))})),
            ),
        ),
        control(
            "outer_group_source_reorder",
            mutation(
                "swap",
                receipt,
                Some(norito::json!({"left": 0, "right": 1})),
            ),
        ),
        controls(
            "outer_group_source_substitution",
            "receipt_group",
            vec![
                mutation(
                    "replace",
                    &format!("{second}/source_id"),
                    Some(norito::json!(
                        "EFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEF"
                    )),
                ),
                mutation(
                    "replace",
                    &format!("{second}/legs/0/prepare_qc/body/source_id"),
                    Some(norito::json!(
                        "EFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEF"
                    )),
                ),
                mutation(
                    "replace",
                    &format!("{second}/legs/0/commit_qc/body/source_id"),
                    Some(norito::json!(
                        "EFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEF"
                    )),
                ),
                mutation(
                    "replace",
                    &format!("{second}/legs/1/prepare_qc/body/source_id"),
                    Some(norito::json!(
                        "EFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEF"
                    )),
                ),
                mutation(
                    "replace",
                    &format!("{second}/legs/1/commit_qc/body/source_id"),
                    Some(norito::json!(
                        "EFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEFEF"
                    )),
                ),
            ],
        ),
        control(
            "unsupported_receipt_version",
            mutation(
                "replace",
                &format!("{first}/version"),
                Some(norito::json!(1)),
            ),
        ),
        control(
            "lowercase_source_id",
            mutation(
                "replace",
                &format!("{first}/source_id"),
                Some(norito::json!(
                    "abababababababababababababababababababababababababababababababab"
                )),
            ),
        ),
        controls(
            "source_id_substituted_for_entrypoint_hash",
            "receipt_group",
            vec![
                mutation(
                    "copy",
                    &format!("{prepare}/body/tx_entrypoint_hash"),
                    Some(norito::json!({"from": (format!("{first}/source_id"))})),
                ),
                mutation(
                    "copy",
                    &format!("{commit}/body/tx_entrypoint_hash"),
                    Some(norito::json!({"from": (format!("{first}/source_id"))})),
                ),
            ],
        ),
        controls(
            "entrypoint_hash_substituted_for_source_id",
            "receipt_group",
            vec![
                mutation(
                    "copy",
                    &format!("{first}/source_id"),
                    Some(norito::json!({"from": (format!("{prepare}/body/tx_entrypoint_hash"))})),
                ),
                mutation(
                    "copy",
                    &format!("{prepare}/body/source_id"),
                    Some(norito::json!({"from": (format!("{prepare}/body/tx_entrypoint_hash"))})),
                ),
                mutation(
                    "copy",
                    &format!("{commit}/body/source_id"),
                    Some(norito::json!({"from": (format!("{prepare}/body/tx_entrypoint_hash"))})),
                ),
            ],
        ),
        controls(
            "wrong_entrypoint_hash_checksum",
            "receipt_group",
            vec![
                mutation(
                    "replace",
                    &format!("{prepare}/body/tx_entrypoint_hash"),
                    Some(norito::json!(
                        "hash:1111111111111111111111111111111111111111111111111111111111111111#0000"
                    )),
                ),
                mutation(
                    "replace",
                    &format!("{commit}/body/tx_entrypoint_hash"),
                    Some(norito::json!(
                        "hash:1111111111111111111111111111111111111111111111111111111111111111#0000"
                    )),
                ),
            ],
        ),
        controls(
            "wrong_entrypoint_hash_marker",
            "receipt_group",
            vec![
                mutation(
                    "replace",
                    &format!("{prepare}/body/tx_entrypoint_hash"),
                    Some(norito::json!(
                        "hash:E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2#4F70"
                    )),
                ),
                mutation(
                    "replace",
                    &format!("{commit}/body/tx_entrypoint_hash"),
                    Some(norito::json!(
                        "hash:E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2E2#4F70"
                    )),
                ),
            ],
        ),
        controls(
            "stale_same_route_incarnation",
            "receipt_group",
            vec![
                mutation(
                    "replace",
                    &format!("{same_route_descriptor}/lane_incarnation"),
                    Some(stale_incarnation.clone()),
                ),
                mutation(
                    "replace",
                    &format!("{settlement}/lane_incarnation"),
                    Some(stale_incarnation.clone()),
                ),
                mutation(
                    "replace",
                    &format!("{prepare}/body/participant_lane_incarnation"),
                    Some(stale_incarnation.clone()),
                ),
                mutation(
                    "replace",
                    &format!("{commit}/body/participant_lane_incarnation"),
                    Some(stale_incarnation.clone()),
                ),
            ],
        ),
        control(
            "same_route_coordinator_view_drift",
            mutation(
                "replace",
                &format!("{same_route_descriptor}/lane_block_view"),
                Some(norito::json!(10)),
            ),
        ),
        control(
            "same_route_mixed_role_deferral",
            mutation(
                "replace",
                &format!("{same_route_descriptor}/accepted_transaction_hashes/0"),
                Some(foreign_entrypoint.clone()),
            ),
        ),
        evidence_control(
            "execution_commitment_merge_carrier_wrong_version",
            vec![mutation(
                "replace",
                "/golden/application_evidence/execution_commitment/merge_carrier/version",
                Some(norito::json!(
                    MERGE_CARRIER_COMMITMENT_VERSION_V1.saturating_add(1)
                )),
            )],
        ),
        evidence_control(
            "execution_commitment_missing_merge_carrier_field",
            vec![mutation(
                "remove",
                "/golden/application_evidence/execution_commitment/merge_carrier",
                None,
            )],
        ),
        evidence_control(
            "stale_participant_application_incarnation",
            vec![mutation(
                "replace",
                "/golden/application_evidence/manifest_artifacts/0/leaf/lane_incarnation",
                Some(stale_incarnation),
            )],
        ),
        evidence_control(
            "same_route_participant_application_marker",
            vec![
                mutation(
                    "replace",
                    "/golden/application_evidence/manifest_artifacts/0/leaf/lane_id",
                    Some(norito::json!(7)),
                ),
                mutation(
                    "replace",
                    "/golden/application_evidence/manifest_artifacts/0/leaf/dataspace_id",
                    Some(norito::json!(11)),
                ),
                mutation(
                    "replace",
                    "/golden/application_evidence/manifest_artifacts/0/leaf/lane_incarnation",
                    Some(coordinator_incarnation),
                ),
            ],
        ),
        evidence_control(
            "unanchored_mixed_role_participant",
            vec![mutation(
                "replace",
                "/golden/receipt_group/native_amx_receipts/0/legs/1/participant_proposal/descriptor/accepted_transaction_hashes/0",
                Some(foreign_entrypoint),
            )],
        ),
        evidence_control(
            "manifest_root_tampering",
            vec![mutation(
                "replace",
                "/golden/application_evidence/manifest_artifacts/0/manifest_root",
                Some(forged_hash.clone()),
            )],
        ),
        evidence_control(
            "manifest_leaf_hash_tampering",
            vec![mutation(
                "replace",
                "/golden/application_evidence/manifest_artifacts/0/leaf_hash",
                Some(forged_hash.clone()),
            )],
        ),
        evidence_control(
            "manifest_proof_path_tampering",
            vec![mutation(
                "replace",
                "/golden/application_evidence/manifest_artifacts/0/proof/audit_path",
                Some(Value::Array(vec![forged_hash.clone()])),
            )],
        ),
        evidence_control(
            "manifest_proof_position_tampering",
            vec![mutation(
                "replace",
                "/golden/application_evidence/manifest_artifacts/0/proof/leaf_index",
                Some(norito::json!(1)),
            )],
        ),
        evidence_control(
            "application_block_substitution",
            vec![mutation(
                "replace",
                "/golden/application_evidence/manifest_artifacts/0/leaf/application_block_hash",
                Some(forged_hash),
            )],
        ),
    ];
    controls.extend(hash_consistency_controls(commitment));
    controls.extend(committee_consistency_controls(context, commitment)?);
    Ok(controls)
}
fn validate_golden(diagnostics: &SumeragiDiagnosticsStatus) -> Result<(), Box<dyn Error>> {
    diagnostics
        .validate_native_amx_participant_applications()
        .map_err(|reason| format!("invalid Native AMX diagnostics row: {reason}"))?;
    let [commitment] = diagnostics.lane_settlement_commitments.as_slice() else {
        return Err("golden fixture must contain exactly one lane settlement".into());
    };
    if commitment.native_amx_receipts.len() != GROUP_SOURCE_COUNT {
        return Err("golden fixture must contain two grouped source receipts".into());
    }
    let expected_sources = commitment
        .native_amx_receipts
        .iter()
        .map(|receipt| receipt.source_id)
        .collect::<Vec<_>>();
    if expected_sources.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("grouped source receipts must be strictly ordered".into());
    }
    for receipt in &commitment.native_amx_receipts {
        if receipt.version != 2 || receipt.legs.len() != 2 {
            return Err("every grouped source must carry two Native AMX v2 legs".into());
        }
        let same_route = receipt
            .legs
            .iter()
            .find(|leg| leg.lane_id == receipt.lane_id && leg.dataspace_id == receipt.dataspace_id)
            .ok_or("every grouped source must carry its same-route coordinator leg")?;
        let same_route_descriptor = &same_route.participant_proposal.descriptor;
        if same_route_descriptor.lane_incarnation != receipt.lane_incarnation
            || same_route_descriptor.lane_block_height != receipt.lane_block_height
            || same_route_descriptor.lane_block_view != receipt.lane_block_view
            || same_route.participant_proposal.proposal_hash != receipt.coordinator_proposal_hash
        {
            return Err(
                "same-route participant proposal must be the exact coordinator identity".into(),
            );
        }
        for leg in &receipt.legs {
            let settlement_sources = leg
                .participant_settlement
                .receipts
                .iter()
                .map(|settlement_receipt| settlement_receipt.source_id)
                .collect::<Vec<_>>();
            if settlement_sources != expected_sources
                || settlement_sources.len() > NATIVE_AMX_GROUP_SOURCES_MAX
                || settlement_sources
                    .iter()
                    .filter(|source| **source == receipt.source_id)
                    .count()
                    != 1
            {
                return Err(
                    "participant settlement does not bind the exact grouped sources".into(),
                );
            }
            for qc in [&leg.prepare_qc, &leg.commit_qc] {
                if qc.validator_set().windows(2).any(|pair| pair[0] >= pair[1])
                    || qc.validator_set_pops().len() != VALIDATOR_COUNT
                    || qc
                        .validator_set_pops()
                        .iter()
                        .any(|pop| pop.len() != NATIVE_AMX_BLS_PROOF_BYTES)
                    || qc.bls_aggregate_signature.len() != NATIVE_AMX_BLS_PROOF_BYTES
                {
                    return Err("Native AMX QC proof geometry is malformed".into());
                }
            }
        }
    }
    Ok(())
}
fn document() -> Result<Value, Box<dyn Error>> {
    let context = fixture_context()?;
    let (commitment, remote) = golden_commitment()?;
    let diagnostics = diagnostics(commitment.clone(), &remote);
    validate_golden(&diagnostics)?;
    let application_evidence = application_evidence(&context, &remote)?;
    let controls = negative_controls(&context, &commitment)?;
    let mut ids = BTreeSet::new();
    for control in &controls {
        let Some(id) = control
            .as_object()
            .and_then(|object| object.get("id"))
            .and_then(Value::as_str)
        else {
            return Err("negative control is missing its string id".into());
        };
        if !ids.insert(id.to_owned()) {
            return Err(format!("duplicate negative-control id `{id}`").into());
        }
    }
    let ordered_source_ids = commitment
        .native_amx_receipts
        .iter()
        .map(|receipt| hex::encode_upper(receipt.source_id))
        .collect::<Vec<_>>();
    let receipt_group = json::to_value(&commitment)?;
    let expected_diagnostics = json::to_value(&diagnostics)?;
    Ok(norito::json!({
        "format": "iroha-native-amx-v2-grouped",
        "fixture_version": 1,
        "rust_owner": "iroha_data_model::block::consensus",
        "bounds": {
            "group_sources_min": 1,
            "group_sources_max": (NATIVE_AMX_GROUP_SOURCES_MAX as u64),
            "participant_legs_max": (NATIVE_AMX_PARTICIPANT_LEGS_MAX as u64),
            "validators_max": (NATIVE_AMX_VALIDATORS_MAX as u64),
            "validator_pop_bytes": (NATIVE_AMX_BLS_PROOF_BYTES as u64),
            "aggregate_signature_bytes": (NATIVE_AMX_BLS_PROOF_BYTES as u64),
        },
        "golden": {
            "ordered_source_ids": (ordered_source_ids),
            "receipt_group": (receipt_group),
            "expected_diagnostics": (expected_diagnostics),
            "application_evidence": (application_evidence),
        },
        "negative_controls": (controls),
    }))
}
pub fn write_fixture(path: &Path, check_only: bool) -> Result<(), Box<dyn Error>> {
    let rendered = format!("{}\n", json::to_string_pretty(&document()?)?);
    if check_only {
        let existing = fs::read_to_string(path)?;
        if existing != rendered {
            return Err(format!(
                "fixture {} is stale; run cargo run --locked --offline -p iroha_data_model --features dev-tools --bin sumeragi_v2_wire_fixtures",
                path.display(),
            )
            .into());
        }
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, rendered)?;
    Ok(())
}
