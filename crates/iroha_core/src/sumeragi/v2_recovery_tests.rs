use super::{
    BlockSignaturePolicy, RecoveredCompleteTipActivationAuthority,
    RecoveredLifecycleStorageMintPermitV1, RecoveredSuccessorActivationAuthority,
    V2RecoveryError, V2StartupReplayError, authenticate_v2_snapshot_replay_boundary,
    authenticate_v2_snapshot_startup, authenticated_v2_snapshot_startup_mode,
    build_verified_successor, committed_execution_policy_hash,
    committed_nexus_amx_context_hash, plan_v2_startup_replay, recover_active_height,
    recover_active_height_with_plan, successor_proofs_of_possession,
};
use crate::{
    block::{CommittedBlock, ValidBlock},
    kura::{CommitManifest, Kura},
    query::store::LiveQueryStore,
    snapshot::AuthenticatedSnapshotBootstrapPayload,
    state::{State, World},
    sumeragi::{
        network_topology::Topology,
        v2::{RecoveredLifecycleStorageAuthorityV1, VerifiedHeightContext},
        v2_context_store::{PersistedHeightContext, V2ContextStore},
    },
};
use iroha_config::parameters::actual::LaneConfig;
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::{
        BlockExecutionContextBundle, BlockHeader, ExternalExecutionContext, SignedBlock,
        builder::BlockBuilder, consensus::SumeragiLanePayloadOwnership, consensus_v2 as wire,
    },
    consensus::{ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus},
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope},
    peer::PeerId,
    transaction::{TransactionBuilder, signed::TransactionResultInner},
    trigger::DataTriggerSequence,
};
use std::{
    io::Write,
    num::{NonZeroU16, NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
fn verified_keys() -> Vec<KeyPair> {
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic BLS key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    keys
}
fn verified_context_for_policy_state(
    policy_state: &State,
    network_id: iroha_data_model::NetworkId,
    keys: &[KeyPair],
) -> VerifiedHeightContext {
    let roster = keys
        .iter()
        .map(|key| wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let context = wire::HeightContext {
        network_id,
        protocol_version: wire::PROTOCOL_VERSION,
        height: 1,
        epoch: 0,
        epoch_end_height: u64::MAX,
        next_epoch_snapshot: None,
        mode: wire::ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
        roster,
        nexus_amx_context_hash: Hash::new(b"recovery fixture Nexus/AMX"),
        execution_policy_hash: committed_execution_policy_hash(policy_state)
            .expect("derive fixture execution policy"),
        da_layout: wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4096,
            max_chunk_count: 8,
        },
        leader_seed: [0x31; 32],
    };
    let proofs = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("BLS proof of possession")
        })
        .collect();
    VerifiedHeightContext::genesis(context, proofs).expect("verified context")
}
fn verified_context() -> (VerifiedHeightContext, Vec<KeyPair>) {
    let keys = verified_keys();
    let network_id = crate::sumeragi::synthetic_network_id("sumeragi-v2-recovery-test");
    let policy_kura = Kura::blank_kura_for_testing();
    let policy_state = state_for(&policy_kura, network_id);
    (
        verified_context_for_policy_state(&policy_state, network_id, &keys),
        keys,
    )
}
#[test]
fn lifecycle_storage_mint_permit_binds_kura_context_and_policy() {
    let (verified, keys) = verified_context();
    let kura = Kura::blank_kura_for_testing();
    let foreign_kura = Kura::blank_kura_for_testing();
    let policy = BlockSignaturePolicy::RotatingLeader;
    let genesis_account = AccountId::new(keys[0].public_key().clone());
    let foreign_genesis_account = AccountId::new(keys[1].public_key().clone());
    let exact = RecoveredLifecycleStorageMintPermitV1::new(
        kura.as_ref(),
        &verified,
        &policy,
        &genesis_account,
    );
    let _authority = RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height(
        kura.as_ref(),
        &verified,
        &policy,
        &genesis_account,
        exact,
    );
    let foreign = RecoveredLifecycleStorageMintPermitV1::new(
        kura.as_ref(),
        &verified,
        &policy,
        &genesis_account,
    );
    assert!(!foreign.authorizes(foreign_kura.as_ref(), &verified, &policy, &genesis_account,));
    let wrong_policy = BlockSignaturePolicy::GenesisAuthority(keys[0].public_key().clone());
    let substituted = RecoveredLifecycleStorageMintPermitV1::new(
        kura.as_ref(),
        &verified,
        &policy,
        &genesis_account,
    );
    assert!(
        !substituted.authorizes(kura.as_ref(), &verified, &wrong_policy, &genesis_account,)
    );
    let substituted = RecoveredLifecycleStorageMintPermitV1::new(
        kura.as_ref(),
        &verified,
        &policy,
        &genesis_account,
    );
    assert!(!substituted.authorizes(
        kura.as_ref(),
        &verified,
        &policy,
        &foreign_genesis_account,
    ));
}
fn state_for(kura: &Arc<Kura>, network_id: iroha_data_model::NetworkId) -> State {
    State::new_with_chain_and_network_id_for_testing(
        World::new(),
        Arc::clone(kura),
        LiveQueryStore::start_test(),
        ChainId::from("sumeragi-v2-recovery-display-name"),
        network_id,
    )
}
fn world_with_consensus_keys(keys: &[KeyPair]) -> World {
    let mut world = World::new();
    for (index, key) in keys.iter().enumerate() {
        let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, format!("validator{index}"));
        let record = ConsensusKeyRecord {
            id: id.clone(),
            public_key: key.public_key().clone(),
            pop: Some(
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("BLS proof of possession"),
            ),
            activation_height: 0,
            expiry_height: None,
            hsm: None,
            replaces: None,
            status: ConsensusKeyStatus::Active,
        };
        world.consensus_keys.insert(id.clone(), record.clone());
        world
            .consensus_keys_by_pk
            .insert(record.public_key.to_string(), vec![id]);
    }
    world
}
fn state_with_consensus_keys(
    kura: &Arc<Kura>,
    network_id: iroha_data_model::NetworkId,
    keys: &[KeyPair],
) -> State {
    State::new_with_chain_and_network_id_for_testing(
        world_with_consensus_keys(keys),
        Arc::clone(kura),
        LiveQueryStore::start_test(),
        ChainId::from("sumeragi-v2-recovery-display-name"),
        network_id,
    )
}
fn dummy_block(
    key: &KeyPair,
    height: u64,
    parent: Option<HashOf<BlockHeader>>,
) -> CommittedBlock {
    dummy_block_with_time(key, height, parent, height)
}
fn dummy_block_with_time(
    key: &KeyPair,
    height: u64,
    parent: Option<HashOf<BlockHeader>>,
    creation_time_ms: u64,
) -> CommittedBlock {
    let mut valid = ValidBlock::new_dummy_and_modify_header(key.private_key(), |header| {
        header.set_height(NonZeroU64::new(height).expect("non-zero height"));
        header.set_prev_block_hash(parent);
        header.creation_time_ms = creation_time_ms;
        header.merkle_root = None;
    });
    valid
        .as_mut()
        .set_transaction_results(Vec::new(), &[], Vec::new())
        .expect("attach required empty block result metadata");
    valid.commit_unchecked().unpack(|_| {})
}
fn autonomous_lane_carrier_block_for_recovery(
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    context: &wire::HeightContext,
    keys: &[KeyPair],
    parent: Option<HashOf<BlockHeader>>,
) -> CommittedBlock {
    let envelope = crate::lane_consensus::autonomous_lane_payload_envelope(
        payload,
        context.network_id,
        context.epoch,
    )
    .expect("encode exact autonomous startup carrier envelope");
    let header = BlockHeader::new(
        NonZeroU64::new(context.height).expect("non-zero carrier height"),
        parent,
        None,
        None,
        context.height,
        0,
    );
    let mut builder = BlockBuilder::new(header);
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new())
            .with_autonomous_lane_payloads(vec![envelope]),
    ));
    let leader = usize::try_from(context.leader(0)).expect("leader index fits usize");
    let signed = builder.build_with_signature(
        u64::try_from(leader).expect("leader index fits u64"),
        keys[leader].private_key(),
    );
    ValidBlock::new_unverified_for_tests(signed)
        .commit_unchecked()
        .unpack(|_| {})
}
fn lane_owned_block_for_recovery(
    state: &State,
    context: &wire::HeightContext,
    keys: &[KeyPair],
) -> SignedBlock {
    let lane_id = LaneId::SINGLE;
    let dataspace_id = DataSpaceId::UNIVERSAL;
    let lane_incarnation = state
        .lane_incarnation_at_height(lane_id, context.height)
        .expect("canonical lane incarnation is active");
    let transaction_key =
        KeyPair::try_from_seed(vec![0xD3; 32], Algorithm::Ed25519).expect("transaction key");
    let transaction = TransactionBuilder::new(
        context.network_id,
        AccountId::new(transaction_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(transaction_key.private_key());
    let entrypoint_hash = transaction.hash_as_entrypoint();
    let validators = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let validator_count =
        u32::try_from(validators.len()).expect("fixture validator count fits u32");
    let min_quorum = u32::try_from(
        crate::sumeragi::network_topology::commit_quorum_from_len(validators.len()).max(1),
    )
    .expect("fixture quorum fits u32");
    let base_mode_tag = match context.mode {
        wire::ConsensusMode::Permissioned => wire::PERMISSIONED_TAG,
        wire::ConsensusMode::Npos => wire::NPOS_TAG,
    };
    let context_mode_tag = format!(
        "{base_mode_tag}::height-context:{}::epoch:{}",
        hex::encode(context.id().0.as_ref()),
        context.epoch
    );
    let mut ownership = SumeragiLanePayloadOwnership {
        proposal_height: context.height,
        proposal_view: 0,
        lane_id,
        dataspace_id,
        lane_incarnation,
        lane_block_height: 1,
        lane_block_view: 0,
        subject_hash: Hash::prehashed([0; Hash::LENGTH]),
        qc_mode_tag: LaneRelayEnvelope::lane_qc_mode_tag_for(
            lane_id,
            dataspace_id,
            &context_mode_tag,
        ),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![Hash::from(entrypoint_hash)],
        previous_lane_block_height: 0,
        previous_lane_block_descriptor_hash: None,
        lane_block_descriptor_hash: Some(Hash::prehashed([0; Hash::LENGTH])),
        lane_block_descriptor_validator_set: validators,
        lane_block_descriptor_validator_count: validator_count,
        lane_block_descriptor_min_quorum: min_quorum,
        payload_ownership_hash: Hash::prehashed([0; Hash::LENGTH]),
        rbc_instance_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    let replay = ownership
        .compute_replay_hashes()
        .expect("fixture ownership replay material is canonical");
    ownership.subject_hash = replay.subject_hash;
    ownership.payload_ownership_hash = replay.payload_ownership_hash;
    ownership.rbc_instance_hash = replay.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
    let header = BlockHeader::new(
        NonZeroU64::new(context.height).expect("non-zero fixture height"),
        None,
        None,
        None,
        context.height,
        0,
    );
    let leader = usize::try_from(context.leader(0)).expect("leader index fits usize");
    let mut builder = BlockBuilder::new(header);
    builder.push_transaction(transaction);
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(vec![ExternalExecutionContext::new(
            entrypoint_hash,
            lane_id,
            dataspace_id,
        )])
        .with_lane_payload_ownerships(vec![ownership]),
    ));
    let mut block = builder.build_with_signature(
        u64::try_from(leader).expect("leader index fits u64"),
        keys[leader].private_key(),
    );
    block
        .set_transaction_results(
            Vec::new(),
            &[entrypoint_hash],
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("attach canonical transaction result");
    block
}
fn commit_to_state(state: &State, block: &CommittedBlock, context: &wire::HeightContext) {
    let topology = Topology::new(context.roster.iter().map(|entry| entry.validator.clone()));
    let mut state_block = state.block(block.as_ref().header());
    let _events = state_block.apply_without_execution(block, topology.as_ref().to_owned());
    state_block.commit().expect("commit synthetic state block");
}
fn execution_commitment(seed: u8) -> wire::ExecutionCommitment {
    wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new([seed, 1]),
        Hash::new([seed, 2]),
        Hash::new([seed, 3]),
        1,
        Hash::new([seed, 4]),
    )
}
fn authenticated_artifact_for(
    context: wire::HeightContext,
    block: &SignedBlock,
    keys: &[KeyPair],
) -> wire::finality::V2FinalityArtifact {
    let subject = wire::BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block
            .canonical_proposal_wire_hash()
            .expect("canonical proposal block wire"),
    };
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let mut exact_execution_commitment = execution_commitment(0xB6);
    exact_execution_commitment.executed_block_wire_len = u64::try_from(
        block
            .encode_wire()
            .expect("canonical executed block wire")
            .len(),
    )
    .expect("canonical executed block wire length fits u64");
    exact_execution_commitment.executed_block_wire_hash = block
        .executed_block_wire_hash()
        .expect("canonical executed block wire");
    let unsigned_vote = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: exact_execution_commitment,
        signer: 0,
        signature: Vec::new(),
    };
    let preimage = unsigned_vote.signature_preimage();
    let shares = keys[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let commit_qc = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: exact_execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
            .expect("aggregate CommitQC"),
    };
    let validator_set_pops = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("fixture validator PoP")
        })
        .collect();
    wire::finality::V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops)
}
fn persist_checkpoint_and_manifest(
    kura: &Kura,
    state: &State,
    artifact: &wire::finality::V2FinalityArtifact,
) {
    artifact.verify().expect("authenticated fixture artifact");
    let checkpoint = crate::snapshot::canonical_state_snapshot_hash(state);
    kura.store_wsv_checkpoint(artifact.height, artifact.block_hash, checkpoint)
        .expect("persist WSV checkpoint");
    kura.store_commit_manifest(
        CommitManifest::new(
            artifact.height,
            artifact.block_hash,
            None,
            None,
            checkpoint,
            None,
        )
        .with_authenticated_v2_commit_authority(artifact),
    )
    .expect("persist checkpoint-bound v2 commit manifest");
}
fn persist_complete_height(
    kura: &Kura,
    state: &State,
    artifact: &wire::finality::V2FinalityArtifact,
) {
    persist_checkpoint_and_manifest(kura, state, artifact);
    let _commit_receipt = kura
        .store_v2_finality_artifact(artifact)
        .expect("persist authenticated v2 finality");
}
#[cfg(feature = "bls")]
pub(super) fn production_empty_genesis_complete_tip_fixture() -> (
    Arc<Kura>,
    Arc<State>,
    VerifiedHeightContext,
    RecoveredLifecycleStorageAuthorityV1,
    KeyPair,
    crate::sumeragi::v2_lifecycle_coordinator::RetiredRecoveredCompleteTipActivationAuthorityV1,
) {
    let (verified_genesis, keys) = verified_context();
    let context = verified_genesis.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = Arc::new(state_with_consensus_keys(&kura, context.network_id, &keys));
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist production-shaped signed genesis block");
    commit_to_state(state.as_ref(), &block, &context);
    let artifact = authenticated_artifact_for(context, block.as_ref(), &keys);
    persist_complete_height(kura.as_ref(), state.as_ref(), &artifact);
    let context_store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    context_store
        .persist(&PersistedHeightContext::from_verified(&verified_genesis))
        .expect("persist signed genesis context");
    let recovered = recover_active_height(
        kura.as_ref(),
        state.as_ref(),
        None,
        keys[0].public_key().clone(),
    )
    .expect("recover the exact Kura height-one CompleteTip");
    let (
        verified_successor,
        _context_store,
        signature_policy,
        lifecycle_storage,
        authenticated_genesis,
        activation,
        staged_genesis,
    ) = recovered.into_parts();
    assert!(authenticated_genesis.is_none());
    assert!(staged_genesis.is_none());
    assert!(matches!(
        signature_policy,
        BlockSignaturePolicy::RotatingLeader
    ));
    let Some(RecoveredSuccessorActivationAuthority::CompleteTip(complete_tip)) = activation
    else {
        panic!("a complete signed genesis tip must recover CompleteTip authority")
    };
    let predecessor_frame = complete_tip
        .lifecycle_storage
        .predecessor
        .root
        .join("lifecycle-ledger-v1.norito");
    assert!(
        !predecessor_frame.exists(),
        "the production-shaped predecessor lifecycle must begin genuinely empty"
    );
    let retirement = complete_tip
        .into_canonical_predecessor_storage(&keys[0])
        .and_then(
            crate::sumeragi::v2_lifecycle_coordinator::AuthenticatedCompleteTipPredecessorStorageV1::retire,
        )
        .expect("retire the empty signed-genesis predecessor");
    (
        kura,
        state,
        verified_successor,
        lifecycle_storage,
        keys[0].clone(),
        retirement,
    )
}
fn hash_only_snapshot_boundary(
    anchor_height: u64,
    install_record: bool,
) -> (
    Arc<Kura>,
    State,
    wire::SnapshotV2BootstrapRecord,
    Vec<KeyPair>,
) {
    assert!(anchor_height > 0);
    let (genesis_context, keys) = verified_context();
    let kura = Kura::blank_kura_for_testing();
    let mut state =
        state_with_consensus_keys(&kura, genesis_context.context().network_id, &keys);
    let mut parent = None;
    for height in 1..=anchor_height {
        let block = dummy_block(&keys[0], height, parent);
        parent = Some(block.as_ref().hash());
        commit_to_state(&state, &block, genesis_context.context());
    }
    let hashes = state.committed_block_hashes_snapshot();
    let record = snapshot_record_for_state(&state, &genesis_context, &keys, anchor_height);
    let payload =
        AuthenticatedSnapshotBootstrapPayload::for_testing(record.clone(), hashes.clone());
    kura.install_authenticated_snapshot_prefix_for_testing(&payload)
        .expect("publish authenticated hash-only snapshot tail");
    for height in 1..=anchor_height {
        let index = NonZeroUsize::new(usize::try_from(height).expect("fixture height fits"))
            .expect("fixture height is non-zero");
        assert!(kura.is_hash_only_block_height(index));
    }
    assert_eq!(
        state.commit_topology_snapshot(),
        record
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>()
    );
    if install_record {
        state.set_authenticated_snapshot_v2_bootstrap_for_testing(record.clone());
    }
    (kura, state, record, keys)
}
fn snapshot_record_for_state(
    state: &State,
    genesis_context: &VerifiedHeightContext,
    keys: &[KeyPair],
    anchor_height: u64,
) -> wire::SnapshotV2BootstrapRecord {
    let mut context = genesis_context.context().clone();
    context.height = anchor_height + 1;
    context.parent_commit_qc = None;
    context.snapshot_bootstrap = Some(wire::SnapshotBootstrapAnchor {
        snapshot_height: anchor_height,
        snapshot_block_hash: state
            .latest_block_hash_fast()
            .expect("non-empty snapshot has a tip"),
        snapshot_block_creation_time_ms: anchor_height,
        snapshot_state_hash: crate::snapshot::canonical_state_snapshot_hash(&state),
    });
    context.nexus_amx_context_hash = committed_nexus_amx_context_hash(&state);
    context.execution_policy_hash =
        committed_execution_policy_hash(state).expect("derive snapshot execution policy");
    let record = wire::SnapshotV2BootstrapRecord {
        version: wire::SnapshotV2BootstrapRecord::VERSION,
        context,
        validator_set_pops: keys
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key())
                    .expect("fixture validator PoP")
            })
            .collect(),
    };
    VerifiedHeightContext::snapshot_bootstrap(&record)
        .expect("fixture snapshot bootstrap is valid");
    record
}
fn complete_first_post_snapshot_height(
    kura: &Kura,
    state: &State,
    record: &wire::SnapshotV2BootstrapRecord,
    keys: &[KeyPair],
) -> CommittedBlock {
    let anchor = record
        .context
        .snapshot_bootstrap
        .as_ref()
        .expect("fixture anchor");
    let block = dummy_block(
        &keys[0],
        record.context.height,
        Some(anchor.snapshot_block_hash),
    );
    kura.store_block(block.clone())
        .expect("persist first post-snapshot block");
    commit_to_state(state, &block, &record.context);
    let artifact = authenticated_artifact_for(record.context.clone(), block.as_ref(), keys);
    persist_complete_height(kura, state, &artifact);
    block
}
fn store_context(kura: &Kura, height: u64) -> PersistedHeightContext {
    V2ContextStore::open(kura.sumeragi_v2_storage_root())
        .expect("open context store")
        .load(height)
        .expect("read context store")
        .expect("persisted context exists")
}
fn model_successful_snapshot_finalization(
    kura: &Kura,
    record: &wire::SnapshotV2BootstrapRecord,
) {
    let verified = VerifiedHeightContext::snapshot_bootstrap(record)
        .expect("fixture snapshot bootstrap is valid");
    V2ContextStore::open(kura.sumeragi_v2_storage_root())
        .expect("open context store after authentication")
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("publish the exact token-owned first-height context");
}
fn storage_tree(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    fn visit(root: &Path, directory: &Path, entries: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
        let Ok(read_dir) = std::fs::read_dir(directory) else {
            return;
        };
        let mut paths = read_dir
            .map(|entry| entry.expect("read storage tree entry").path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let relative = path
                .strip_prefix(root)
                .expect("walk remains below storage root")
                .to_owned();
            if path.is_dir() {
                entries.push((relative, None));
                visit(root, &path, entries);
            } else {
                entries.push((
                    relative,
                    Some(std::fs::read(&path).expect("read storage tree file")),
                ));
            }
        }
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries
}
fn primary_lane_blocks_dir(kura: &Kura) -> PathBuf {
    LaneConfig::default()
        .primary()
        .blocks_dir(kura.store_root())
}
#[test]
fn empty_chain_retry_binds_current_lane_auxiliary_storage() {
    let kura = Kura::blank_kura_for_testing();
    kura.finish_v2_startup_finality_verification();
    kura.reset_startup_replay_historical_payload_reads_for_test();
    let plan = plan_v2_startup_replay(kura.as_ref())
        .expect("empty-chain retry must bind current lane auxiliary storage");
    assert_eq!(plan.durable_height(), 0);
    assert_eq!(plan.complete_prefix_height(), 0);
    assert_eq!(plan.pending_tip_height(), None);
    plan.validate_exact_kura_boundary(kura.as_ref())
        .expect("empty-chain plan must retain its exact storage binding");
    assert_eq!(
        kura.startup_replay_historical_payload_reads_for_test(),
        0,
        "empty-chain refresh must not perform historical payload reads"
    );
}
#[test]
fn all_hash_only_snapshot_recovers_exact_authenticated_successor() {
    let (kura, state, record, keys) = hash_only_snapshot_boundary(3, true);
    let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan snapshot import");
    let authorization = authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan)
        .expect("authenticate snapshot startup")
        .expect("snapshot startup mints an authorization");
    assert_eq!(authorization.mode(), record.context.mode);
    model_successful_snapshot_finalization(kura.as_ref(), &record);
    let recovered =
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
            .expect("authenticated all-hash-only snapshot must open its first context");
    assert_eq!(recovered.verified_context().context(), &record.context);
    assert_eq!(
        recovered.verified_context().proofs_of_possession(),
        record.validator_set_pops
    );
    assert!(recovered.pending_kura_apply().is_none());
    match recovered
        .successor_activation()
        .expect("snapshot recovery retains typed activation authority")
    {
        RecoveredSuccessorActivationAuthority::SnapshotBootstrap(authority) => {
            assert_eq!(authority.snapshot_anchor_height(), 3);
            assert_eq!(authority.successor_context_id(), record.context.id());
            assert_eq!(authority.record_hash, HashOf::new(&record));
            assert_eq!(
                authority.snapshot_block_hash,
                record
                    .context
                    .snapshot_bootstrap
                    .as_ref()
                    .expect("snapshot record retains anchor")
                    .snapshot_block_hash
            );
        }
        RecoveredSuccessorActivationAuthority::CompleteTip(_) => {
            panic!("snapshot bootstrap must not masquerade as durable CommitQC authority")
        }
    }
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    let persisted = store
        .load(record.context.height)
        .expect("load context")
        .expect("bootstrap context was persisted before ingress");
    assert_eq!(persisted.context(), &record.context);
    assert_eq!(persisted.proofs_of_possession(), record.validator_set_pops);
}
#[test]
fn audited_snapshot_prefix_classifies_retained_legacy_bodies_without_sidecars() {
    let (genesis_context, keys) = verified_context();
    let kura = Kura::blank_kura_for_testing();
    let mut state =
        state_with_consensus_keys(&kura, genesis_context.context().network_id, &keys);
    let mut parent = None;
    for height in 1..=3 {
        let block = dummy_block(&keys[0], height, parent);
        parent = Some(block.as_ref().hash());
        if height <= 2 {
            kura.store_block(block.clone())
                .expect("retain legacy snapshot body");
        }
        commit_to_state(&state, &block, genesis_context.context());
    }
    let retained_body_path = primary_lane_blocks_dir(kura.as_ref()).join("blocks.data");
    let retained_body_bytes =
        std::fs::read(&retained_body_path).expect("read exact retained legacy body journal");
    assert!(!retained_body_bytes.is_empty());
    let record = snapshot_record_for_state(&state, &genesis_context, &keys, 3);
    let payload = AuthenticatedSnapshotBootstrapPayload::for_testing(
        record.clone(),
        state.committed_block_hashes_snapshot(),
    );
    kura.install_authenticated_snapshot_prefix_for_testing(&payload)
        .expect("publish mixed retained/hash-only audited prefix");
    state.set_authenticated_snapshot_v2_bootstrap_for_testing(record.clone());
    assert_eq!(
        std::fs::read(&retained_body_path).expect("reread retained legacy body journal"),
        retained_body_bytes,
        "typed import publication must preserve every exact retained body byte"
    );
    for height in 1..=3 {
        assert!(
            kura.get_block(NonZeroUsize::new(height).expect("non-zero height"))
                .is_none(),
            "typed imported history is never exposed for executable replay even when exact legacy bytes remain retained"
        );
    }
    let plan = plan_v2_startup_replay(kura.as_ref())
        .expect("the complete typed import is exempt from executable sidecars");
    assert_eq!(plan.audited_bootstrap_prefix_height(), 3);
    assert_eq!(plan.complete_prefix_height(), 3);
    assert_eq!(plan.pending_tip_height(), None);
    authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan)
        .expect("authenticate mixed imported prefix")
        .expect("snapshot startup requires finalization");
    model_successful_snapshot_finalization(kura.as_ref(), &record);
    recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
        .expect("retained bodies inside the typed import are historical, not executable");
}
#[test]
fn untyped_zero_length_placeholder_is_never_a_replay_exemption() {
    let kura = Kura::blank_kura_for_testing();
    let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xC4; 32]));
    kura.extend_hash_only_suffix_from_verified_snapshot(&[hash])
        .expect("publish local snapshot placeholder without audited import authority");
    let height = NonZeroUsize::new(1).expect("non-zero height");
    assert!(kura.is_hash_only_block_height(height));
    assert!(!kura.is_audited_snapshot_import_height(height));
    assert!(matches!(
        plan_v2_startup_replay(kura.as_ref()),
        Err(V2StartupReplayError::InvalidReplayMetadata { height: 1, .. })
    ));
}
#[test]
fn all_hash_only_snapshot_without_authenticated_record_fails_closed() {
    let (kura, state, _record, _keys) = hash_only_snapshot_boundary(2, false);
    let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan snapshot import");
    let storage_root = kura.sumeragi_v2_storage_root();
    let tree_before = storage_tree(&storage_root);
    assert!(matches!(
        authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan),
        Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
    ));
    assert_eq!(
        storage_tree(&storage_root),
        tree_before,
        "failed token minting must leave the complete storage tree unchanged"
    );
}
#[test]
fn arbitrary_self_signed_first_roster_is_rejected_before_state_or_context_mutation() {
    let (kura, state, record, _keys) = hash_only_snapshot_boundary(2, true);
    let before_height = state.committed_height();
    let before_wsv = crate::snapshot::canonical_state_snapshot_hash(&state);
    let anchor = record
        .context
        .snapshot_bootstrap
        .as_ref()
        .expect("fixture anchor");
    let mut attacker_keys = (81_u8..=84)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic attacker BLS key")
        })
        .collect::<Vec<_>>();
    attacker_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let attacker_roster = attacker_keys
        .iter()
        .map(|key| wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let mut attacker_context = record.context.clone();
    attacker_context.roster = attacker_roster;
    attacker_context.quorum = wire::DualQuorum::from_roster(&attacker_context.roster)
        .expect("attacker roster is internally valid");
    let block = dummy_block(
        &attacker_keys[0],
        record.context.height,
        Some(anchor.snapshot_block_hash),
    );
    kura.store_block(block.clone())
        .expect("persist attacker first full body");
    let attacker_artifact =
        authenticated_artifact_for(attacker_context, block.as_ref(), &attacker_keys);
    persist_complete_height(kura.as_ref(), &state, &attacker_artifact);
    let plan = plan_v2_startup_replay(kura.as_ref())
        .expect("self-signed artifact is structurally complete but not snapshot-authorized");
    let storage_root = kura.sumeragi_v2_storage_root();
    let tree_before = storage_tree(&storage_root);
    assert!(matches!(
        authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan),
        Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
    ));
    assert_eq!(state.committed_height(), before_height);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state),
        before_wsv
    );
    assert_eq!(
        storage_tree(&storage_root),
        tree_before,
        "attacker artifact must be rejected before any storage publication"
    );
}
#[test]
fn startup_plan_rejects_poisoned_height_two_that_ignores_npos_transition() {
    let (verified, current_keys) = verified_context();
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_consensus_keys(&kura, verified.context().network_id, &current_keys);
    let mut transitioned_keys = (21_u8..=24)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic transitioned BLS key")
        })
        .collect::<Vec<_>>();
    transitioned_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let transitioned_roster = transitioned_keys
        .iter()
        .map(|key| wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let transitioned_pops = transitioned_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("transitioned validator PoP")
        })
        .collect::<Vec<_>>();
    let transitioned_quorum =
        wire::DualQuorum::from_roster(&transitioned_roster).expect("transitioned quorum");
    let transitioned_leader_seed = [0x62; 32];
    let mut parent_context = verified.context().clone();
    parent_context.mode = wire::ConsensusMode::Npos;
    parent_context.epoch_end_height = 1;
    parent_context.next_epoch_snapshot = Some(wire::finality::FinalizedNextEpochSnapshot {
        epoch: 1,
        epoch_end_height: 10,
        mode: wire::ConsensusMode::Npos,
        roster: transitioned_roster,
        validator_set_pops: transitioned_pops,
        quorum: transitioned_quorum,
        leader_seed: transitioned_leader_seed,
    });
    let block_one = dummy_block(&current_keys[0], 1, None);
    kura.store_block(block_one.clone())
        .expect("persist canonical parent block");
    commit_to_state(&state, &block_one, &parent_context);
    let parent_artifact =
        authenticated_artifact_for(parent_context.clone(), block_one.as_ref(), &current_keys);
    persist_complete_height(kura.as_ref(), &state, &parent_artifact);
    let mut attacker_keys = (81_u8..=84)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic attacker BLS key")
        })
        .collect::<Vec<_>>();
    attacker_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let attacker_roster = attacker_keys
        .iter()
        .map(|key| wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let attacker_quorum =
        wire::DualQuorum::from_roster(&attacker_roster).expect("attacker quorum");
    let child_context = wire::HeightContext {
        network_id: parent_context.network_id,
        protocol_version: parent_context.protocol_version,
        height: 2,
        epoch: 1,
        epoch_end_height: 10,
        next_epoch_snapshot: None,
        mode: wire::ConsensusMode::Npos,
        parent_commit_qc: Some(parent_artifact.commit_qc.clone()),
        snapshot_bootstrap: None,
        quorum: attacker_quorum,
        roster: attacker_roster,
        nexus_amx_context_hash: parent_context.nexus_amx_context_hash,
        execution_policy_hash: parent_context.execution_policy_hash,
        da_layout: parent_context.da_layout,
        leader_seed: transitioned_leader_seed,
    };
    let block_two = dummy_block(&attacker_keys[0], 2, Some(block_one.as_ref().hash()));
    kura.store_block(block_two.clone())
        .expect("persist poisoned child block");
    commit_to_state(&state, &block_two, &child_context);
    let child_artifact =
        authenticated_artifact_for(child_context, block_two.as_ref(), &attacker_keys);
    persist_complete_height(kura.as_ref(), &state, &child_artifact);
    assert!(matches!(
        plan_v2_startup_replay(kura.as_ref()),
        Err(V2StartupReplayError::FinalityAuthorityLineageMismatch { height: 2 })
    ));
}
#[test]
fn anchor_snapshot_reopens_pending_first_full_block_without_parent_finality() {
    let (kura, state, record, keys) = hash_only_snapshot_boundary(2, true);
    let anchor = record
        .context
        .snapshot_bootstrap
        .as_ref()
        .expect("fixture anchor");
    let all_hash_only_plan =
        plan_v2_startup_replay(kura.as_ref()).expect("plan hash-only snapshot");
    authenticate_v2_snapshot_startup(kura.as_ref(), &state, &all_hash_only_plan)
        .expect("authenticate first executable context")
        .expect("snapshot startup requires finalization");
    model_successful_snapshot_finalization(kura.as_ref(), &record);
    let block = dummy_block(
        &keys[0],
        record.context.height,
        Some(anchor.snapshot_block_hash),
    );
    kura.store_block(block.clone())
        .expect("persist first post-snapshot block");
    let recovered =
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
            .expect("anchor-height snapshot must reopen its exact pending first block");
    assert_eq!(recovered.verified_context().context(), &record.context);
    let pending = recovered
        .pending_kura_apply()
        .expect("missing finality sidecar must reopen exact Apply pipeline");
    assert_eq!(pending.height(), record.context.height);
    assert_eq!(pending.context_id(), record.context.id());
    assert_eq!(pending.block_hash(), block.as_ref().hash());
    assert_eq!(
        state.committed_height(),
        usize::try_from(anchor.snapshot_height).expect("fixture height fits usize")
    );
}
#[test]
fn later_snapshot_before_first_full_finality_is_rejected_without_mutation() {
    let (kura, state, record, keys) = hash_only_snapshot_boundary(2, true);
    let anchor = record
        .context
        .snapshot_bootstrap
        .as_ref()
        .expect("fixture anchor");
    let all_hash_only_plan =
        plan_v2_startup_replay(kura.as_ref()).expect("plan hash-only snapshot");
    authenticate_v2_snapshot_startup(kura.as_ref(), &state, &all_hash_only_plan)
        .expect("authenticate first executable context")
        .expect("snapshot startup requires finalization");
    model_successful_snapshot_finalization(kura.as_ref(), &record);
    let block = dummy_block(
        &keys[0],
        record.context.height,
        Some(anchor.snapshot_block_hash),
    );
    kura.store_block(block.clone())
        .expect("persist first post-snapshot block");
    commit_to_state(&state, &block, &record.context);
    let artifact = authenticated_artifact_for(record.context.clone(), block.as_ref(), &keys);
    persist_checkpoint_and_manifest(kura.as_ref(), &state, &artifact);
    let state_hash_before = crate::snapshot::canonical_state_snapshot_hash(&state);
    let hashes_before = state.committed_block_hashes_snapshot();
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    let context_before = store
        .load(record.context.height)
        .expect("read immutable context")
        .expect("authenticated context exists");
    assert!(matches!(
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone()),
        Err(V2RecoveryError::StartupReplay(
            V2StartupReplayError::SnapshotBootstrapAuthentication { .. }
        ))
    ));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state),
        state_hash_before,
        "rejected lineage must not mutate WSV"
    );
    assert_eq!(state.committed_block_hashes_snapshot(), hashes_before);
    assert_eq!(
        store
            .load(record.context.height)
            .expect("reload immutable context")
            .expect("context remains present"),
        context_before
    );
    assert!(
        kura.v2_finality_artifact(record.context.height)
            .expect("read finality")
            .is_none(),
        "failed startup authentication must not publish missing finality"
    );
}
#[test]
fn later_snapshot_requires_retained_original_bootstrap_lineage() {
    let (kura, state, record, keys) = hash_only_snapshot_boundary(2, false);
    let verified = VerifiedHeightContext::snapshot_bootstrap(&record)
        .expect("fixture bootstrap context is valid");
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist original boundary context");
    complete_first_post_snapshot_height(kura.as_ref(), &state, &record, &keys);
    let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan complete first height");
    assert!(matches!(
        authenticate_v2_snapshot_replay_boundary(kura.as_ref(), &state, &plan),
        Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
    ));
    assert!(matches!(
        authenticated_v2_snapshot_startup_mode(kura.as_ref(), &state, &plan),
        Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
    ));
}
#[test]
fn later_signed_lineage_without_immutable_first_context_fails_closed_read_only() {
    let (kura, state, record, keys) = hash_only_snapshot_boundary(2, true);
    complete_first_post_snapshot_height(kura.as_ref(), &state, &record, &keys);
    assert!(
        V2ContextStore::load_from_root_read_only(
            kura.sumeragi_v2_storage_root(),
            record.context.height,
        )
        .expect("read context store")
        .is_none(),
        "fixture starts without node-local immutable context"
    );
    let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan complete first height");
    let storage_root = kura.sumeragi_v2_storage_root();
    let tree_before = storage_tree(&storage_root);
    assert!(matches!(
        authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan),
        Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
    ));
    assert_eq!(storage_tree(&storage_root), tree_before);
    assert!(
        V2ContextStore::load_from_root_read_only(storage_root, record.context.height)
            .expect("read context store after failed authentication")
            .is_none(),
        "failed reauthentication must not publish an immutable first-height context"
    );
}
#[test]
fn finalized_later_snapshot_rejects_a_missing_immutable_first_height_context() {
    let (kura, state, record, keys) = hash_only_snapshot_boundary(2, true);
    model_successful_snapshot_finalization(kura.as_ref(), &record);
    assert!(
        !kura.provisional_snapshot_bootstrap_pending(),
        "fixture must exercise the post-finalization trust boundary"
    );
    let mut parent = complete_first_post_snapshot_height(kura.as_ref(), &state, &record, &keys);
    let context_store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    for _ in 0..3 {
        let parent_height = parent.as_ref().header().height().get();
        let (parent_artifact, parent_receipt) = kura
            .v2_finality_artifact_with_receipt(parent_height)
            .expect("read parent finality")
            .expect("parent finality exists");
        let verified =
            build_verified_successor(&state, &context_store, &parent_artifact, &parent_receipt)
                .expect("derive exact post-snapshot successor context");
        let context = verified.context().clone();
        let block = dummy_block(&keys[0], context.height, Some(parent.as_ref().hash()));
        kura.store_block(block.clone())
            .expect("persist later full post-snapshot block");
        commit_to_state(&state, &block, &context);
        let artifact = authenticated_artifact_for(context, block.as_ref(), &keys);
        persist_complete_height(kura.as_ref(), &state, &artifact);
        parent = block;
    }
    kura.publish_exact_commit_marker_for_tests()
        .expect("publish the exact full-height test commit marker");
    assert_eq!(
        kura.exact_durable_blocks_count()
            .expect("exact durable count"),
        usize::try_from(record.context.height + 3).expect("fixture height fits usize")
    );
    let context_path = kura
        .sumeragi_v2_storage_root()
        .join("contexts")
        .join(format!("{:020}.norito", record.context.height));
    std::fs::remove_file(&context_path)
        .expect("remove the immutable context to model post-finalization loss");
    let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan complete first height");
    let state_hash_before = crate::snapshot::canonical_state_snapshot_hash(&state);
    let hashes_before = state.committed_block_hashes_snapshot();
    let storage_root = kura.store_root();
    let storage_before = storage_tree(&storage_root);
    assert!(matches!(
        authenticate_v2_snapshot_replay_boundary(kura.as_ref(), &state, &plan),
        Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
    ));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state),
        state_hash_before,
        "missing immutable context rejection must not mutate WSV"
    );
    assert_eq!(state.committed_block_hashes_snapshot(), hashes_before);
    assert_eq!(
        storage_tree(&storage_root),
        storage_before,
        "post-eviction missing immutable context rejection must keep all Kura bytes read-only"
    );
}
#[test]
fn later_snapshot_rejects_lineage_changed_from_immutable_first_height() {
    let (kura, mut state, record, keys) = hash_only_snapshot_boundary(2, true);
    let initial_plan =
        plan_v2_startup_replay(kura.as_ref()).expect("plan initial hash-only snapshot");
    authenticate_v2_snapshot_startup(kura.as_ref(), &state, &initial_plan)
        .expect("authenticate original boundary context")
        .expect("snapshot startup requires finalization");
    model_successful_snapshot_finalization(kura.as_ref(), &record);
    complete_first_post_snapshot_height(kura.as_ref(), &state, &record, &keys);
    let mut substituted = record.clone();
    substituted.context.leader_seed[0] ^= 0x80;
    VerifiedHeightContext::snapshot_bootstrap(&substituted)
        .expect("substituted lineage is internally self-consistent");
    state.set_authenticated_snapshot_v2_bootstrap_for_testing(substituted);
    let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan complete first height");
    assert!(matches!(
        authenticate_v2_snapshot_replay_boundary(kura.as_ref(), &state, &plan),
        Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
    ));
    assert_eq!(
        store_context(kura.as_ref(), record.context.height).context(),
        &record.context,
        "conflicting signed lineage must not replace the immutable original"
    );
}
#[test]
fn later_snapshot_uses_historical_lineage_not_current_topology_or_anchor_wsv() {
    let (kura, mut state, record, keys) = hash_only_snapshot_boundary(2, true);
    let initial_plan =
        plan_v2_startup_replay(kura.as_ref()).expect("plan initial hash-only snapshot");
    authenticate_v2_snapshot_startup(kura.as_ref(), &state, &initial_plan)
        .expect("authenticate original boundary context")
        .expect("snapshot startup requires finalization");
    model_successful_snapshot_finalization(kura.as_ref(), &record);
    complete_first_post_snapshot_height(kura.as_ref(), &state, &record, &keys);
    let changed_topology = (91_u8..=94)
        .map(|seed| {
            let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic replacement BLS key");
            PeerId::new(key.public_key().clone())
        })
        .collect::<Vec<_>>();
    {
        let mut topology = state.commit_topology.block();
        topology.clear();
        topology.extend(changed_topology.clone());
        topology.commit();
    }
    assert_ne!(
        state.commit_topology_snapshot(),
        record
            .context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>()
    );
    assert_ne!(
        crate::snapshot::canonical_state_snapshot_hash(&state),
        record
            .context
            .snapshot_bootstrap
            .as_ref()
            .expect("fixture anchor")
            .snapshot_state_hash,
        "fixture must model a later WSV, not the original anchor WSV"
    );
    state.set_authenticated_snapshot_v2_bootstrap_for_testing(record.clone());
    let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan complete first height");
    authenticate_v2_snapshot_replay_boundary(kura.as_ref(), &state, &plan)
        .expect("historical lineage is authenticated by its first full finality");
    assert_eq!(
        authenticated_v2_snapshot_startup_mode(kura.as_ref(), &state, &plan)
            .expect("derive retained signed mode"),
        Some(record.context.mode)
    );
}
#[test]
fn hash_only_snapshot_rejects_an_intermediate_hash_vector_substitution() {
    let (genesis_context, keys) = verified_context();
    let kura = Kura::blank_kura_for_testing();
    let mut state =
        state_with_consensus_keys(&kura, genesis_context.context().network_id, &keys);
    let mut parent = None;
    for height in 1..=3 {
        let block = dummy_block(&keys[0], height, parent);
        parent = Some(block.as_ref().hash());
        commit_to_state(&state, &block, genesis_context.context());
    }
    let mut substituted_hashes = state.committed_block_hashes_snapshot();
    substituted_hashes[0] = HashOf::from_untyped_unchecked(Hash::prehashed([0xE1; 32]));
    assert_eq!(
        substituted_hashes.last(),
        state.committed_block_hashes_snapshot().last(),
        "adversarial vector preserves the signed tip"
    );
    let record = snapshot_record_for_state(&state, &genesis_context, &keys, 3);
    let payload = AuthenticatedSnapshotBootstrapPayload::for_testing(
        record.clone(),
        substituted_hashes.clone(),
    );
    kura.install_authenticated_snapshot_prefix_for_testing(&payload)
        .expect("publish adversarial hash-only vector fixture");
    state.set_authenticated_snapshot_v2_bootstrap_for_testing(record.clone());
    let plan = plan_v2_startup_replay(kura.as_ref()).expect("plan hash-only snapshot");
    let storage_root = kura.sumeragi_v2_storage_root();
    let tree_before = storage_tree(&storage_root);
    assert!(matches!(
        authenticate_v2_snapshot_startup(kura.as_ref(), &state, &plan),
        Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
    ));
    assert_eq!(
        storage_tree(&storage_root),
        tree_before,
        "hash-vector substitution must fail before any storage publication"
    );
}
const STARTUP_FINALITY_PLANNER_CHILD_ENV: &str =
    "IROHA_STARTUP_FINALITY_PLANNER_DEADLOCK_CHILD";
const STARTUP_FINALITY_PLANNER_TIMEOUT: Duration = Duration::from_secs(30);

fn run_startup_finality_planner_deadlock_child() {
    let keys = verified_keys();
    let state = State::new_with_pre_genesis_nexus_for_testing(
        world_with_consensus_keys(&keys),
        crate::kura::tests::startup_two_lane_nexus(),
        LiveQueryStore::start_test(),
    );
    let network_id = state.network_id;
    let kura = state.kura_handle();
    let mut fixture =
        crate::kura::tests::autonomous_lane_startup_fixture_for_carrier(&state, &keys[0], 0, 4);
    assert!(
        Arc::ptr_eq(&kura, &fixture.kura),
        "autonomous payload must use the State-owned two-lane Kura"
    );
    let mut verified = verified_context_for_policy_state(&state, network_id, &keys);
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist height-one context");
    let mut parent = None;
    for height in 1..=4 {
        let context = verified.context().clone();
        assert_eq!(context.height, height);
        let block = if height == 4 {
            autonomous_lane_carrier_block_for_recovery(
                &fixture.payload,
                &context,
                &keys,
                parent,
            )
        } else {
            dummy_block(&keys[0], height, parent)
        };
        parent = Some(block.as_ref().hash());
        if height == 4 {
            fixture.certify_for_carrier(&keys[0], block.as_ref());
            let proposal = &fixture.payload.origin_proposal;
            let descriptor = &proposal.descriptor;
            assert_eq!(descriptor.proposal_height, 4);
            assert_eq!(
                proposal
                    .payload_block_hint
                    .expect("certified proposal owns its global carrier")
                    .proposal_block_hash,
                block.as_ref().hash(),
            );
            assert_eq!(
                kura.read_certified_lane_block_artifact(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                )
                .expect("read exact certified autonomous startup slot")
                .proposal,
                *proposal,
            );
        }
        kura.store_block(block.clone())
            .expect("persist canonical replay fixture block");
        commit_to_state(&state, &block, &context);
        let artifact = authenticated_artifact_for(context, block.as_ref(), &keys);
        persist_complete_height(kura.as_ref(), &state, &artifact);
        if height < 4 {
            let (parent_artifact, parent_receipt) = kura
                .v2_finality_artifact_with_receipt(height)
                .expect("read parent finality")
                .expect("parent finality exists");
            verified =
                build_verified_successor(&state, &store, &parent_artifact, &parent_receipt)
                    .expect("derive exact successor context")
                    .into_parts()
                    .0;
        }
    }

    let plan = plan_v2_startup_replay(kura.as_ref())
        .expect("plan authenticated autonomous four-block replay fixture");
    assert_eq!(plan.durable_height(), 4);
    assert_eq!(plan.complete_prefix_height(), 4);
    assert_eq!(plan.pending_tip_height(), None);
    kura.finish_v2_startup_finality_verification();
}

#[test]
fn startup_finality_planner_deadlock_child() {
    if std::env::var_os(STARTUP_FINALITY_PLANNER_CHILD_ENV).is_none() {
        return;
    }
    run_startup_finality_planner_deadlock_child();
}

#[test]
fn startup_finality_planner_with_certified_autonomous_tip_does_not_deadlock() {
    let mut child = Command::new(
        std::env::current_exe().expect("resolve current iroha_core test executable"),
    )
    .arg("startup_finality_planner_deadlock_child")
    .arg("--nocapture")
    .arg("--test-threads=1")
    .env(STARTUP_FINALITY_PLANNER_CHILD_ENV, "1")
    .spawn()
    .expect("spawn isolated startup planner regression");
    let deadline = Instant::now() + STARTUP_FINALITY_PLANNER_TIMEOUT;
    loop {
        if let Some(status) = child
            .try_wait()
            .expect("poll isolated startup planner regression")
        {
            assert!(status.success(), "startup planner child exited {status}");
            return;
        }
        if Instant::now() >= deadline {
            let kill_result = child.kill();
            let wait_result = child.wait();
            panic!(
                "startup planner deadlocked for thirty seconds; kill={kill_result:?}, wait={wait_result:?}"
            );
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn replay_body_preflight_rejects_a_later_unavailable_evicted_body_without_partial_state() {
    let (mut verified, keys) = verified_context();
    let kura = Kura::blank_kura_for_testing_with_blocks_in_memory(
        NonZeroUsize::new(1).expect("non-zero body retention"),
    );
    let state = state_with_consensus_keys(&kura, verified.context().network_id, &keys);
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist height-one context");
    let mut parent = None;
    for height in 1..=4 {
        let context = verified.context().clone();
        assert_eq!(context.height, height);
        let block = dummy_block(&keys[0], height, parent);
        parent = Some(block.as_ref().hash());
        kura.store_block(block.clone())
            .expect("persist canonical replay fixture block");
        commit_to_state(&state, &block, &context);
        let artifact = authenticated_artifact_for(context, block.as_ref(), &keys);
        persist_complete_height(kura.as_ref(), &state, &artifact);
        if height < 4 {
            let (parent_artifact, parent_receipt) = kura
                .v2_finality_artifact_with_receipt(height)
                .expect("read parent finality")
                .expect("parent finality exists");
            verified =
                build_verified_successor(&state, &store, &parent_artifact, &parent_receipt)
                    .expect("derive exact successor context")
                    .into_parts()
                    .0;
        }
    }
    let evicted_height = NonZeroUsize::new(2).expect("non-zero evicted height");
    let payload_len = kura
        .advertise_required_replicas_for_bench(evicted_height)
        .expect("height two is inline and advertizable");
    assert!(
        kura.evict_block_bodies_for_bench(payload_len)
            .expect("evict finalized historical body")
            >= payload_len
    );
    kura.remove_evicted_block_sidecar_for_testing(evicted_height)
        .expect("remove local DA cache to model remote-only unavailability");
    assert!(
        !kura.is_hash_only_block_height(evicted_height),
        "ordinary eviction retains a non-zero canonical index length"
    );
    assert!(kura.get_block(evicted_height).is_none());
    let plan = plan_v2_startup_replay(kura.as_ref())
        .expect("verified sidecars keep the evicted height finality-complete");
    assert_eq!(plan.complete_prefix_height(), 4);
    for _ in 0..3 {
        state.block_hashes.block_and_revert().commit_for_tests();
    }
    let state_hashes_before = state.committed_block_hashes_snapshot();
    let state_wsv_before = crate::snapshot::canonical_state_snapshot_hash(&state);
    assert_eq!(state.committed_height(), 1);
    assert!(
        crate::state::preflight_v2_replay_body_availability(
            kura.as_ref(),
            &state,
            2,
            plan.complete_prefix_height(),
        )
        .is_err()
    );
    assert_eq!(
        state.committed_block_hashes_snapshot(),
        state_hashes_before,
        "whole-range preflight must fail before replaying any earlier body"
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state),
        state_wsv_before
    );
}
#[test]
fn successor_pops_are_copied_only_from_the_durable_parent_artifact() {
    let (verified, current_keys) = verified_context();
    let current_context = verified.context().clone();
    let block = dummy_block(&current_keys[0], current_context.height, None);
    let parent =
        authenticated_artifact_for(current_context.clone(), block.as_ref(), &current_keys);
    parent.verify().expect("authenticated non-boundary parent");
    assert_eq!(
        successor_proofs_of_possession(&parent),
        parent.validator_set_pops,
        "non-boundary recovery must retain the exact historical PoP bytes"
    );
    let mut next_keys = (21_u8..=24)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                .expect("deterministic next-epoch BLS key")
        })
        .collect::<Vec<_>>();
    next_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let next_roster = next_keys
        .iter()
        .map(|key| wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let next_pops = next_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key()).expect("valid next-epoch PoP")
        })
        .collect::<Vec<_>>();
    let mut boundary_context = current_context;
    boundary_context.epoch_end_height = boundary_context.height;
    boundary_context.next_epoch_snapshot = Some(wire::finality::FinalizedNextEpochSnapshot {
        epoch: boundary_context.epoch + 1,
        epoch_end_height: u64::MAX,
        mode: boundary_context.mode,
        quorum: wire::DualQuorum::from_roster(&next_roster).expect("valid next-epoch quorum"),
        roster: next_roster,
        validator_set_pops: next_pops.clone(),
        leader_seed: [0x73; 32],
    });
    let boundary_parent =
        authenticated_artifact_for(boundary_context, block.as_ref(), &current_keys);
    boundary_parent
        .verify()
        .expect("old roster authenticates the complete boundary snapshot");
    assert_eq!(
        successor_proofs_of_possession(&boundary_parent),
        next_pops,
        "boundary recovery must use the authenticated successor PoPs"
    );
    assert_ne!(
        successor_proofs_of_possession(&boundary_parent),
        boundary_parent.validator_set_pops,
        "next-epoch PoPs must not be reconstructed from the current roster"
    );
}
#[test]
fn durable_block_before_wsv_reopens_only_its_persisted_height_context() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist canonical block");
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist active context");
    let recovered =
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
            .expect("resume interrupted height");
    assert_eq!(recovered.verified_context().context(), &context);
    let pending = recovered
        .pending_kura_apply()
        .expect("durable tip requires replay binding");
    assert_eq!(pending.context_id(), context.id());
    assert_eq!(pending.height(), 1);
    assert_eq!(pending.block_hash(), block.as_ref().hash());
    assert_eq!(state.committed_height(), 0);
    assert_eq!(
        kura.exact_durable_blocks_count()
            .expect("read exact durable height"),
        1
    );
}
#[test]
fn durable_context_recovery_rejects_local_execution_policy_drift() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let mut state = state_for(&kura, context.network_id);
    state.pipeline.overlay_max_bytes = state.pipeline.overlay_max_bytes.saturating_add(1);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block).expect("persist canonical block");
    V2ContextStore::open(kura.sumeragi_v2_storage_root())
        .expect("open context store")
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist active context");
    assert!(matches!(
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone()),
        Err(V2RecoveryError::ExecutionPolicyMismatch { .. })
    ));
}
#[test]
fn durable_context_recovery_rejects_local_autoscale_policy_drift() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let mut state = state_for(&kura, context.network_id);
    let mut nexus = state.nexus_snapshot();
    nexus.autoscale.cooldown_blocks = NonZeroU16::new(
        nexus
            .autoscale
            .cooldown_blocks
            .get()
            .checked_add(1)
            .expect("fixture cooldown remains representable"),
    )
    .expect("fixture cooldown remains non-zero");
    state
        .set_nexus(nexus)
        .expect("pre-genesis autoscale policy drift is structurally valid");
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block).expect("persist canonical block");
    V2ContextStore::open(kura.sumeragi_v2_storage_root())
        .expect("open context store")
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist active context");
    assert!(matches!(
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone()),
        Err(V2RecoveryError::ExecutionPolicyMismatch { .. })
    ));
}
#[test]
fn checkpoint_before_finality_reopens_same_height_without_reapplying() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist canonical block");
    commit_to_state(&state, &block, &context);
    let checkpoint = crate::snapshot::canonical_state_snapshot_hash(&state);
    kura.store_wsv_checkpoint(1, block.as_ref().hash(), checkpoint)
        .expect("persist interrupted post-WSV checkpoint");
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist active context");
    let recovered =
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
            .expect("resume finality sidecar window");
    assert_eq!(recovered.verified_context().context(), &context);
    assert_eq!(
        recovered
            .pending_kura_apply()
            .expect("missing finality requires replay binding")
            .block_hash(),
        block.as_ref().hash()
    );
    assert_eq!(state.committed_height(), 1);
    assert!(
        kura.v2_finality_artifact(1)
            .expect("read finality")
            .is_none()
    );
}
#[test]
fn finality_complete_tip_with_incomplete_lane_completion_reopens_same_height() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let signed_block = lane_owned_block_for_recovery(&state, &context, &keys);
    let block = ValidBlock::committed_from_replay_signed_block(signed_block);
    kura.store_block(block.clone())
        .expect("persist canonical lane-owned block");
    commit_to_state(&state, &block, &context);
    let artifact = authenticated_artifact_for(context.clone(), block.as_ref(), &keys);
    persist_complete_height(kura.as_ref(), &state, &artifact);
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist active context");
    assert!(
        kura.v2_finality_artifact_with_receipt(1)
            .expect("read complete global finality")
            .is_some(),
        "the fixture must cross the complete global finality boundary"
    );
    assert!(
        kura.read_lane_block_artifact(LaneId::SINGLE, 1).is_some(),
        "the canonical ownership sidecar must already be durable"
    );
    assert!(
        kura.read_certified_lane_block_artifact(LaneId::SINGLE, 1)
            .is_none(),
        "the fixture must stop before the lane CommitQC is durable"
    );
    assert!(
        kura.read_lane_block_application_receipt(LaneId::SINGLE, 1)
            .is_none(),
        "the fixture must stop before lane application receipt publication"
    );
    let plan = plan_v2_startup_replay(kura.as_ref())
        .expect("classify missing lane completion as an interrupted durable tip");
    assert_eq!(plan.durable_height(), 1);
    assert_eq!(plan.complete_prefix_height(), 0);
    assert_eq!(plan.pending_tip_height(), Some(1));
    let recovered =
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
            .expect("reopen the exact finalized lane-owned tip");
    assert_eq!(recovered.verified_context().context(), &context);
    let pending = recovered
        .pending_kura_apply()
        .expect("incomplete lane completion must retain the exact Apply binding");
    assert_eq!(pending.context_id(), context.id());
    assert_eq!(pending.height(), 1);
    assert_eq!(pending.state_height(), 1);
    assert_eq!(pending.block_hash(), block.as_ref().hash());
    assert!(recovered.successor_activation().is_none());
    assert!(
        store.load(2).expect("inspect successor context").is_none(),
        "recovery must not derive or persist a successor before lane completion"
    );
}
#[test]
fn applied_tip_without_persisted_checkpoint_fails_closed() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist canonical block");
    commit_to_state(&state, &block, &context);
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist active context");
    assert!(matches!(
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone()),
        Err(V2RecoveryError::AppliedPendingTipWithoutCheckpoint(1))
    ));
}
#[test]
fn finality_without_checkpoint_and_manifest_fails_closed() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist canonical block");
    let artifact = authenticated_artifact_for(context.clone(), block.as_ref(), &keys);
    let _receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist finality");
    assert!(matches!(
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
        Err(V2RecoveryError::StartupReplay(
            V2StartupReplayError::InvalidReplayMetadata { height: 1, .. }
        ))
    ));
}
#[test]
fn parent_finality_and_immutable_context_mismatch_fails_closed() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist canonical block");
    commit_to_state(&state, &block, &context);
    let artifact = authenticated_artifact_for(context.clone(), block.as_ref(), &keys);
    persist_complete_height(kura.as_ref(), &state, &artifact);
    let mut different = context;
    different.leader_seed[0] ^= 0x80;
    let proofs = keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("BLS proof of possession")
        })
        .collect();
    let different = VerifiedHeightContext::genesis(different, proofs)
        .expect("different context is independently valid");
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&different))
        .expect("persist mismatching context");
    assert!(matches!(
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
        Err(V2RecoveryError::ParentContextMismatch(1))
    ));
}
#[test]
fn missing_context_for_interrupted_durable_block_fails_closed() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    kura.store_block(dummy_block(&keys[0], 1, None))
        .expect("persist canonical block");
    assert!(matches!(
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
        Err(V2RecoveryError::MissingActiveContext(1))
    ));
}
#[test]
fn equal_wsv_and_kura_heights_with_different_hashes_fail_closed() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let state_block = dummy_block_with_time(&keys[0], 1, None, 1);
    let kura_block = dummy_block_with_time(&keys[0], 1, None, 2);
    assert_ne!(state_block.as_ref().hash(), kura_block.as_ref().hash());
    commit_to_state(&state, &state_block, verified.context());
    kura.store_block(kura_block)
        .expect("persist conflicting Kura tip");
    assert!(matches!(
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone(),),
        Err(V2RecoveryError::StateKuraHashMismatch { height: 1, .. })
    ));
}
#[test]
fn startup_plan_never_generic_replays_a_kura_first_tip() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let first = dummy_block(&keys[0], 1, None);
    kura.store_block(first.clone())
        .expect("persist first canonical block");
    commit_to_state(&state, &first, &context);
    let artifact = authenticated_artifact_for(context, first.as_ref(), &keys);
    persist_complete_height(kura.as_ref(), &state, &artifact);
    let second = dummy_block(&keys[0], 2, Some(first.as_ref().hash()));
    kura.store_block(second)
        .expect("persist Kura-first successor tip");
    let plan = plan_v2_startup_replay(kura.as_ref()).expect("classify exact pending tip");
    assert_eq!(plan.durable_height(), 2);
    assert_eq!(plan.complete_prefix_height(), 1);
    assert_eq!(plan.pending_tip_height(), Some(2));
    plan.validate_restored_state_height(0)
        .expect("empty state can replay complete prefix");
    plan.validate_restored_state_height(1)
        .expect("complete prefix state is valid");
    plan.validate_restored_state_height(2)
        .expect("checkpointed snapshot may already contain the sole tip");
    assert!(matches!(
        plan.validate_restored_state_height(3),
        Err(V2StartupReplayError::StateHeightOutsidePlan { .. })
    ));
}
#[test]
fn startup_audit_is_reused_by_planning_and_recovery_then_cleared() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_consensus_keys(&kura, context.network_id, &keys);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist audited canonical block");
    commit_to_state(&state, &block, &context);
    let artifact = authenticated_artifact_for(context, block.as_ref(), &keys);
    persist_complete_height(kura.as_ref(), &state, &artifact);
    V2ContextStore::open(kura.sumeragi_v2_storage_root())
        .expect("open startup-audit context store")
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist authenticated parent context");
    kura.clear_v2_finality_verification_cache_for_test();
    kura.reset_v2_finality_crypto_verifications_for_test();
    let first_plan =
        plan_v2_startup_replay(kura.as_ref()).expect("audit and plan complete height");
    assert_eq!(
        kura.v2_finality_crypto_verifications_for_test(),
        1,
        "the startup audit performs the sole cryptographic pass"
    );
    assert_eq!(kura.v2_startup_finality_inventory_len_for_test(), 1);
    kura.clear_v2_finality_verification_cache_for_test();
    kura.reset_startup_replay_historical_payload_reads_for_test();
    let second_plan =
        plan_v2_startup_replay(kura.as_ref()).expect("reuse exact startup inventory");
    assert_eq!(first_plan.complete_prefix_height(), 1);
    assert_eq!(second_plan.complete_prefix_height(), 1);
    assert_eq!(
        kura.v2_finality_crypto_verifications_for_test(),
        1,
        "replanning beyond an empty runtime LRU must reuse the startup audit"
    );
    assert_eq!(
        kura.startup_replay_historical_payload_reads_for_test(),
        0,
        "replanning must consume the authenticated in-memory boundary, index, checkpoint, manifest, finality, and retained-record projections"
    );
    kura.clear_v2_finality_verification_cache_for_test();
    recover_active_height_with_plan(
        kura.as_ref(),
        &state,
        None,
        keys[0].public_key().clone(),
        second_plan,
    )
    .expect("recover successor from authenticated complete tip");
    assert_eq!(
        kura.v2_finality_crypto_verifications_for_test(),
        1,
        "recovery consumes the plan without another finality scan"
    );
    assert_eq!(
        kura.v2_startup_finality_inventory_len_for_test(),
        0,
        "recovery consumes the O(H) startup inventory"
    );
}
#[test]
fn recovery_rejects_post_plan_storage_identity_replacement_and_clears_inventory() {
    for replacement_target in [
        "blocks.data",
        "v2_finality",
        "wsv_checkpoints",
        "commit_manifests",
    ] {
        let (verified, keys) = verified_context();
        let context = verified.context().clone();
        let kura = Kura::blank_kura_for_testing();
        let state = state_with_consensus_keys(&kura, context.network_id, &keys);
        let block = dummy_block(&keys[0], 1, None);
        kura.store_block(block.clone())
            .expect("persist replacement fixture block");
        commit_to_state(&state, &block, &context);
        let artifact = authenticated_artifact_for(context, block.as_ref(), &keys);
        persist_complete_height(kura.as_ref(), &state, &artifact);
        let plan =
            plan_v2_startup_replay(kura.as_ref()).expect("bind exact pre-replacement storage");
        assert_eq!(kura.v2_startup_finality_inventory_len_for_test(), 1);
        let consensus_storage_root = kura.sumeragi_v2_storage_root();
        assert!(
            !consensus_storage_root.exists(),
            "fixture must begin without recovery-owned consensus storage"
        );
        let blocks_dir = primary_lane_blocks_dir(kura.as_ref());
        let path = match replacement_target {
            "blocks.data" => blocks_dir.join("blocks.data"),
            "v2_finality" => blocks_dir
                .join("v2_finality")
                .join("00000000000000000001.norito"),
            "wsv_checkpoints" => blocks_dir
                .join("wsv_checkpoints")
                .join("00000000000000000001.norito"),
            "commit_manifests" => blocks_dir
                .join("commit_manifests")
                .join("00000000000000000001.norito"),
            _ => unreachable!("replacement target list is exhaustive"),
        };
        let replacement = path.with_extension("startup-replacement");
        std::fs::write(
            &replacement,
            std::fs::read(&path).expect("read exact pre-plan bytes"),
        )
        .expect("write equal-byte replacement");
        std::fs::remove_file(&path).expect("unlink validated storage identity");
        std::fs::rename(&replacement, &path).expect("publish equal-byte replacement");
        assert!(matches!(
            recover_active_height_with_plan(
                kura.as_ref(),
                &state,
                None,
                keys[0].public_key().clone(),
                plan,
            ),
            Err(V2RecoveryError::StartupReplay(V2StartupReplayError::Kura(
                _
            )))
        ));
        assert!(
            !consensus_storage_root.exists(),
            "tampered replay binding must fail before recovery creates consensus storage"
        );
        assert_eq!(
            kura.v2_startup_finality_inventory_len_for_test(),
            0,
            "error-path recovery must clear the startup inventory"
        );
    }
}
#[test]
fn startup_plan_propagates_a_corrupt_exact_durable_index_count() {
    let (_verified, keys) = verified_context();
    let kura = Kura::blank_kura_for_testing();
    kura.store_block(dummy_block(&keys[0], 1, None))
        .expect("persist canonical block");
    let index_path = primary_lane_blocks_dir(kura.as_ref()).join("blocks.index");
    let mut index = std::fs::OpenOptions::new()
        .append(true)
        .open(&index_path)
        .expect("open durable index for adversarial corruption");
    index
        .write_all(&[0xA5])
        .expect("append a partial index entry");
    index.sync_all().expect("sync corrupt durable index");
    assert!(matches!(
        plan_v2_startup_replay(kura.as_ref()),
        Err(V2StartupReplayError::Kura(_))
    ));
}
#[test]
fn startup_plan_rejects_a_missing_canonical_file_on_an_empty_chain() {
    for name in [
        "blocks.data",
        "blocks.index",
        "blocks.hashes",
        "blocks.count.norito",
    ] {
        let kura = Kura::blank_kura_for_testing();
        let path = primary_lane_blocks_dir(kura.as_ref()).join(name);
        std::fs::remove_file(&path).expect("remove canonical journal file");
        assert!(matches!(
            plan_v2_startup_replay(kura.as_ref()),
            Err(V2StartupReplayError::Kura(crate::kura::Error::IO(error, failed_path)))
                if error.kind() == std::io::ErrorKind::NotFound && failed_path == path
        ));
        assert_eq!(
            kura.v2_startup_finality_inventory_len_for_test(),
            0,
            "failed planning must clear startup-only inventory after removing {name}"
        );
        assert!(
            kura.begin_v2_startup_finality_verification()
                .expect("inspect failed startup inventory")
                .is_none(),
            "failed planning must leave no reusable startup session after removing {name}"
        );
    }
}
#[test]
fn startup_plan_rejects_an_incomplete_interior_height() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let first = dummy_block(&keys[0], 1, None);
    kura.store_block(first.clone())
        .expect("persist first canonical block");
    commit_to_state(&state, &first, verified.context());
    let artifact =
        authenticated_artifact_for(verified.context().clone(), first.as_ref(), &keys);
    // Model a crash after manifest publication but before finality, followed by an
    // impossible later durable block. The gap is interior and must never be treated as a
    // multi-height recovery suffix.
    persist_checkpoint_and_manifest(kura.as_ref(), &state, &artifact);
    kura.store_block(dummy_block(&keys[0], 2, Some(first.as_ref().hash())))
        .expect("persist impossible later block");
    assert!(matches!(
        plan_v2_startup_replay(kura.as_ref()),
        Err(V2StartupReplayError::IncompleteInteriorHeight {
            height: 1,
            durable_height: 2,
        })
    ));
}
#[test]
fn startup_plan_accepts_each_post_checkpoint_crash_window_as_one_tip() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist canonical block");
    commit_to_state(&state, &block, verified.context());
    let artifact =
        authenticated_artifact_for(verified.context().clone(), block.as_ref(), &keys);
    let checkpoint = crate::snapshot::canonical_state_snapshot_hash(&state);
    kura.store_wsv_checkpoint(1, block.as_ref().hash(), checkpoint)
        .expect("persist checkpoint-only crash image");
    let checkpoint_only =
        plan_v2_startup_replay(kura.as_ref()).expect("checkpoint-only tip is recoverable");
    assert_eq!(checkpoint_only.complete_prefix_height(), 0);
    assert_eq!(checkpoint_only.pending_tip_height(), Some(1));
    kura.store_commit_manifest(
        CommitManifest::new(1, block.as_ref().hash(), None, None, checkpoint, None)
            .with_authenticated_v2_commit_authority(&artifact),
    )
    .expect("persist manifest-only-before-finality crash image");
    let manifest_only =
        plan_v2_startup_replay(kura.as_ref()).expect("manifest tip is recoverable");
    assert_eq!(manifest_only.complete_prefix_height(), 0);
    assert_eq!(manifest_only.pending_tip_height(), Some(1));
    let _commit_receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("complete finality publication");
    let complete = plan_v2_startup_replay(kura.as_ref()).expect("complete tuple is replayable");
    assert_eq!(complete.complete_prefix_height(), 1);
    assert_eq!(complete.pending_tip_height(), None);
}
#[test]
fn deferred_sidecar_recovery_requires_a_fresh_plan_and_snapshot_boundary_authentication() {
    let (kura, state, record, keys) = hash_only_snapshot_boundary(2, true);
    let anchor = record
        .context
        .snapshot_bootstrap
        .as_ref()
        .expect("fixture snapshot anchor");
    let first_full = dummy_block(
        &keys[0],
        record.context.height,
        Some(anchor.snapshot_block_hash),
    );
    kura.store_block(first_full.clone())
        .expect("persist interrupted first post-snapshot block");
    let prefinalization_plan =
        plan_v2_startup_replay(kura.as_ref()).expect("classify pre-finalization crash image");
    assert_eq!(prefinalization_plan.complete_prefix_height(), 2);
    assert_eq!(prefinalization_plan.pending_tip_height(), Some(3));
    let authorization =
        authenticate_v2_snapshot_startup(kura.as_ref(), &state, &prefinalization_plan)
            .expect("authenticate original snapshot boundary")
            .expect("imported prefix mints a finalization authorization");
    // Model deferred stage recovery publishing a complete, internally valid sidecar tuple
    // after the token was minted. The recovered artifact preserves the snapshot anchor, so
    // replay planning alone accepts it, but substitutes another frozen first-height context.
    // Startup must discard the old plan and authenticate the recovered tuple against the
    // original signed snapshot before replay.
    let mut substituted_context = record.context.clone();
    substituted_context.leader_seed[0] ^= 0x80;
    let substituted_artifact =
        authenticated_artifact_for(substituted_context, first_full.as_ref(), &keys);
    persist_complete_height(kura.as_ref(), &state, &substituted_artifact);
    let recovered_plan =
        plan_v2_startup_replay(kura.as_ref()).expect("reclassify recovered sidecar tuple");
    assert_eq!(recovered_plan.complete_prefix_height(), 3);
    assert_eq!(recovered_plan.pending_tip_height(), None);
    assert_ne!(
        recovered_plan, prefinalization_plan,
        "deferred recovery changed the executable replay boundary"
    );
    assert!(matches!(
        authenticate_v2_snapshot_replay_boundary(kura.as_ref(), &state, &recovered_plan),
        Err(V2StartupReplayError::SnapshotBootstrapAuthentication { .. })
    ));
    drop(authorization);
}
#[test]
fn startup_plan_rejects_finality_bound_to_an_unauthenticated_manifest() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_for(&kura, context.network_id);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist canonical block");
    commit_to_state(&state, &block, verified.context());
    let artifact =
        authenticated_artifact_for(verified.context().clone(), block.as_ref(), &keys);
    let checkpoint = crate::snapshot::canonical_state_snapshot_hash(&state);
    kura.store_wsv_checkpoint(1, block.as_ref().hash(), checkpoint)
        .expect("persist WSV checkpoint");
    kura.store_commit_manifest(CommitManifest::new(
        1,
        block.as_ref().hash(),
        None,
        None,
        checkpoint,
        None,
    ))
    .expect("persist checkpoint-bound but authority-free manifest");
    let _commit_receipt = kura
        .store_v2_finality_artifact(&artifact)
        .expect("persist independently authenticated finality");
    assert!(matches!(
        plan_v2_startup_replay(kura.as_ref()),
        Err(V2StartupReplayError::InvalidReplayMetadata { height: 1, .. })
    ));
}
#[test]
fn finalized_tip_derives_one_idempotent_successor_context() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_consensus_keys(&kura, context.network_id, &keys);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist canonical block");
    commit_to_state(&state, &block, &context);
    let artifact = authenticated_artifact_for(context.clone(), block.as_ref(), &keys);
    persist_complete_height(kura.as_ref(), &state, &artifact);
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist parent context");
    let first =
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
            .expect("derive successor");
    assert_eq!(first.verified_context().context().height, 2);
    assert_eq!(
        first.verified_context().context().parent_commit_qc,
        Some(artifact.commit_qc.clone())
    );
    assert!(first.pending_kura_apply().is_none());
    match first
        .successor_activation()
        .expect("complete tip retains typed activation authority")
    {
        RecoveredSuccessorActivationAuthority::CompleteTip(authority) => {
            assert_eq!(authority.predecessor().height(), 1);
            assert_eq!(
                authority.successor_context_id(),
                first.verified_context().context().id()
            );
            assert_eq!(&authority.artifact, &artifact);
            assert_eq!(authority.receipt.height(), artifact.height);
            assert_eq!(authority.receipt.block_hash(), artifact.block_hash);
            assert_eq!(authority.receipt.context_id(), artifact.context_id());
            assert_eq!(authority.receipt.subject(), artifact.subject);
            assert_eq!(authority.receipt.certificate(), artifact.commit_qc.as_ref());
            assert_eq!(authority.receipt.artifact_hash(), HashOf::new(&artifact));
            let lifecycle_root = kura.sumeragi_v2_storage_root().join("lifecycle-v1");
            let predecessor_lifecycle_root =
                lifecycle_root.join(hex::encode(artifact.context_id().0.as_ref()));
            assert_eq!(
                authority.lifecycle_storage.predecessor.root,
                predecessor_lifecycle_root
            );
            assert_eq!(
                authority.lifecycle_storage.successor.root,
                lifecycle_root.join(hex::encode(
                    first.verified_context().context().id().0.as_ref()
                ))
            );
            assert_eq!(
                authority.lifecycle_storage.body_store_root,
                kura.sumeragi_v2_storage_root().join("bodies")
            );
            assert!(matches!(
                &authority.predecessor_signature_policy,
                BlockSignaturePolicy::GenesisAuthority(_)
            ));
            assert_eq!(
                authority.verified_predecessor.context(),
                &artifact.height_context
            );
            assert_eq!(
                authority.verified_predecessor.proofs_of_possession(),
                artifact.validator_set_pops.as_slice()
            );
            assert!(authority.authorizes_predecessor_lifecycle_root(
                &authority.lifecycle_storage.predecessor.root
            ));
            let foreign_kura = Kura::blank_kura_for_testing();
            let foreign_lifecycle_root = foreign_kura
                .sumeragi_v2_storage_root()
                .join("lifecycle-v1")
                .join(hex::encode(artifact.context_id().0.as_ref()));
            assert!(!authority.authorizes_predecessor_lifecycle_root(&foreign_lifecycle_root));
        }
        RecoveredSuccessorActivationAuthority::SnapshotBootstrap(_) => {
            panic!("complete tip must retain durable CommitQC authority")
        }
    }
    let first_context = first.verified_context().context().clone();
    drop(first);
    let repeated =
        recover_active_height(kura.as_ref(), &state, None, keys[0].public_key().clone())
            .expect("reopen identical successor");
    assert_eq!(repeated.verified_context().context(), &first_context);
    assert!(repeated.pending_kura_apply().is_none());
    assert!(matches!(
        repeated.successor_activation(),
        Some(RecoveredSuccessorActivationAuthority::CompleteTip(authority))
            if authority.predecessor().height() == 1
                && authority.successor_context_id() == first_context.id()
    ));
}
#[test]
fn verified_successor_projects_only_its_exact_kura_lifecycle_storage() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_consensus_keys(&kura, context.network_id, &keys);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist canonical predecessor block");
    commit_to_state(&state, &block, &context);
    let artifact = authenticated_artifact_for(context, block.as_ref(), &keys);
    let receipt = crate::kura::KuraV2CommitReceipt::for_test(&artifact);
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist canonical predecessor context");
    let genesis_account = AccountId::new(keys[0].public_key().clone());

    let successor = build_verified_successor(&state, &store, &artifact, &receipt)
        .expect("build exact successor storage projection");
    let successor_context_id = successor.context().id();
    let (successor, activation, _storage) = successor
        .into_parts_with_lifecycle_storage_authority(kura.as_ref(), &genesis_account)
        .expect("project exact successor lifecycle storage authority");
    assert_eq!(successor.context().id(), successor_context_id);
    assert_eq!(activation.successor_context_id(), successor_context_id);

    let foreign_kura = Kura::blank_kura_for_testing();
    let successor = build_verified_successor(&state, &store, &artifact, &receipt)
        .expect("rebuild successor before foreign Kura rejection");
    assert!(matches!(
        successor.into_parts_with_lifecycle_storage_authority(
            foreign_kura.as_ref(),
            &genesis_account,
        ),
        Err(V2RecoveryError::SuccessorLifecycleStorageKuraMismatch { height: 2 })
    ));
}

#[test]
fn successor_rejects_foreign_same_height_predecessor_and_mismatched_receipt() {
    let (verified, keys) = verified_context();
    let context = verified.context().clone();
    let kura = Kura::blank_kura_for_testing();
    let state = state_with_consensus_keys(&kura, context.network_id, &keys);
    let block = dummy_block(&keys[0], 1, None);
    kura.store_block(block.clone())
        .expect("persist canonical predecessor block");
    commit_to_state(&state, &block, &context);
    let artifact = authenticated_artifact_for(context, block.as_ref(), &keys);
    let receipt = crate::kura::KuraV2CommitReceipt::for_test(&artifact);
    let store =
        V2ContextStore::open(kura.sumeragi_v2_storage_root()).expect("open context store");
    store
        .persist(&PersistedHeightContext::from_verified(&verified))
        .expect("persist canonical predecessor context");
    let mut foreign = artifact.clone();
    let foreign_block_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"foreign same-height predecessor block"));
    foreign.subject.block_hash = foreign_block_hash;
    foreign.block_hash = foreign_block_hash;
    foreign.commit_qc.subject = foreign.subject;
    let foreign_receipt = crate::kura::KuraV2CommitReceipt::for_test(&foreign);
    assert!(matches!(
        build_verified_successor(&state, &store, &artifact, &foreign_receipt),
        Err(V2RecoveryError::DurablePredecessorAuthorityMismatch(1))
    ));
    let exact_successor = build_verified_successor(&state, &store, &artifact, &receipt)
        .expect("build exact successor before rejecting foreign receipt retention");
    let exact_successor_context_id = exact_successor.context().id();
    let (_, exact_activation) = exact_successor.into_parts();
    assert!(matches!(
        RecoveredCompleteTipActivationAuthority::authenticate_for_test(
            artifact.clone(),
            foreign_receipt.clone(),
            exact_successor_context_id,
            exact_activation,
        ),
        Err(V2RecoveryError::DurablePredecessorAuthorityMismatch(1))
    ));
    let exact_successor = build_verified_successor(&state, &store, &artifact, &receipt)
        .expect("build exact successor before rejecting foreign artifact retention");
    let (_, exact_activation) = exact_successor.into_parts();
    assert!(matches!(
        RecoveredCompleteTipActivationAuthority::authenticate_for_test(
            foreign.clone(),
            receipt.clone(),
            exact_successor_context_id,
            exact_activation,
        ),
        Err(V2RecoveryError::DurablePredecessorAuthorityMismatch(1))
    ));
    let foreign_activation = super::DurableSuccessorActivationAuthority::for_test(
        super::DurableV2PredecessorIdentity::for_test(1, b"foreign activation predecessor"),
        exact_successor_context_id,
    );
    assert!(matches!(
        RecoveredCompleteTipActivationAuthority::authenticate_for_test(
            artifact.clone(),
            receipt.clone(),
            exact_successor_context_id,
            foreign_activation,
        ),
        Err(V2RecoveryError::DurablePredecessorAuthorityMismatch(1))
    ));
    let exact_successor = build_verified_successor(&state, &store, &artifact, &receipt)
        .expect("build exact successor before rejecting changed successor binding");
    let (_, exact_activation) = exact_successor.into_parts();
    let foreign_successor_context_id =
        wire::HeightContextId(HashOf::<wire::HeightContext>::from_untyped_unchecked(
            Hash::new(b"foreign recovered successor context binding"),
        ));
    assert!(matches!(
        RecoveredCompleteTipActivationAuthority::authenticate_for_test(
            artifact.clone(),
            receipt.clone(),
            foreign_successor_context_id,
            exact_activation,
        ),
        Err(
            V2RecoveryError::RecoveredCompleteTipSuccessorAuthorityMismatch {
                predecessor_height: 1
            }
        )
    ));
    assert!(matches!(
        build_verified_successor(&state, &store, &foreign, &foreign_receipt),
        Err(V2RecoveryError::FinalizedStatePredecessorMismatch {
            expected_height: 1,
            actual_height: 1,
            expected_block_hash,
            actual_block_hash: Some(actual_block_hash),
        }) if expected_block_hash == foreign_block_hash
            && actual_block_hash == receipt.block_hash()
    ));
}
