use std::sync::Arc;

use iroha_data_model::{
    ChainId, ValidationFail,
    account::AccountId,
    asset::{AssetDefinition, AssetDefinitionId, AssetId},
    block::{SignedBlock, consensus_v2::ConsensusMode},
    isi::{InstructionBox, Log, Mint, Register, SetKeyValue},
    name::Name,
    nexus::{
        AssetPermissionManifest, DataSpaceCatalog, DataSpaceId, DataSpaceMetadata, LaneCatalog,
        LaneConfig, LaneId, LaneVisibility, ManifestVersion, UniversalAccountId,
    },
    peer::PeerId,
    prelude::{Account, Domain, DomainId},
    transaction::{TransactionBuilder, error::TransactionRejectionReason},
};
use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};

use super::*;

fn run_replay_validation_test_on_stack(name: &'static str, test: fn()) {
    // The full replay pipeline has deep debug-mode stack use; do not depend on libtest's
    // platform-default worker stack for these integration-heavy scenarios.
    let handle = std::thread::Builder::new()
        .name(name.to_owned())
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .expect("spawn replay validation test");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

fn replay_blocks_from_kura(
    kura: &Arc<Kura>,
    state: &mut State,
    topology: &crate::sumeragi::network_topology::Topology,
    block_count: usize,
    fallback_consensus_mode: ConsensusMode,
) -> Result<()> {
    replay_blocks_from_kura_range(
        kura,
        state,
        topology,
        1,
        block_count,
        fallback_consensus_mode,
    )
}

/// Exercise legacy fixture blocks without weakening the production v2 replay boundary.
///
/// Historical unit fixtures predate v2 finality artifacts. Production callers resolve to the
/// parent-module function, which never enters this test-only adapter.
pub(super) fn replay_blocks_from_kura_range(
    kura: &Arc<Kura>,
    state: &mut State,
    topology: &crate::sumeragi::network_topology::Topology,
    start_height: usize,
    block_count: usize,
    fallback_consensus_mode: ConsensusMode,
) -> Result<()> {
    if block_count == 0 || start_height > block_count {
        return Ok(());
    }
    let genesis_account = state
        .view()
        .world()
        .domain(&iroha_genesis::GENESIS_DOMAIN_ID)
        .map_err(|error| eyre!(error))?
        .owned_by()
        .clone();
    let time_source = TimeSource::new_system();
    for height in start_height..=block_count {
        let nz = NonZeroUsize::new(height).expect("test replay height is non-zero");
        let Some(block) = kura.get_block(nz) else {
            if super::hash_only_replay_snapshot_hash(kura, state, nz)?.is_some() {
                continue;
            }
            return Err(eyre!("missing block at height {height} during replay"));
        };
        let signed = block.as_ref().clone();
        let height = signed.header().height().get();
        let checkpoint = kura
            .wsv_checkpoint(height)?
            .ok_or_else(|| eyre!("missing WSV checkpoint for full block #{height}"))?;
        let roster_snapshot = state.commit_roster_snapshot_for_block(height, signed.hash());
        let roster = roster_snapshot
            .as_ref()
            .map(|snapshot| {
                if snapshot.commit_qc.validator_set.is_empty() {
                    snapshot.validator_checkpoint.validator_set.clone()
                } else {
                    snapshot.commit_qc.validator_set.clone()
                }
            })
            .filter(|roster| !roster.is_empty())
            .unwrap_or_else(|| topology.as_ref().to_vec());
        let mut validation_topology =
            crate::sumeragi::network_topology::Topology::new(roster.clone());
        let view = signed.header().view_change_index();
        let (mode, seed) = {
            let state_view = state.view();
            (
                crate::sumeragi::effective_consensus_mode(&state_view, fallback_consensus_mode),
                crate::sumeragi::prf_seed_for_height(&state_view, height),
            )
        };
        match mode {
            ConsensusMode::Permissioned => {
                validation_topology.canonicalize_order();
                validation_topology.shuffle_prf(seed, height);
                validation_topology.nth_rotation(view);
            }
            ConsensusMode::Npos => {
                let leader = validation_topology.leader_index_prf(seed, height, view);
                validation_topology.rotate_preserve_view_to_front(leader);
            }
        }
        let mut voting_block = None;
        let (valid, mut state_block) = ValidBlock::validate_keep_voting_block_for_replay(
            signed.clone(),
            &validation_topology,
            &genesis_account,
            &time_source,
            state,
            &mut voting_block,
            false,
            false,
        )
        .unpack(|_| {})
        .map_err(|(_block, error)| eyre!(error))
        .wrap_err_with(|| format!("failed to validate block #{height} during replay"))?;
        let committed = valid.commit_unchecked().unpack(|_| {});
        ensure_replayed_results_match_committed(height, &signed, committed.as_ref())
            .wrap_err_with(|| {
                format!(
                    "failed to verify replayed block #{height} against committed execution results"
                )
            })?;
        state_block.replay_compatibility = true;
        prune_restored_commit_qcs_for_replay(&mut state_block, height);
        let _ = state_block.apply_without_execution_with_commit_qc_for_replay(
            &committed,
            roster,
            roster_snapshot.as_ref().map(|snapshot| &snapshot.commit_qc),
        );
        state_block.prepare_replay_checkpoint_preview();
        let actual = crate::snapshot::canonical_staged_state_snapshot_hash(&state_block);
        if actual != checkpoint.state_hash() {
            return Err(eyre!(
                "replayed block #{height} WSV checkpoint mismatch: committed={:?} replayed={actual:?}",
                checkpoint.state_hash()
            ));
        }
        state_block
            .commit()
            .map_err(|error| eyre!(error))
            .wrap_err_with(|| format!("failed to commit replayed block #{height}"))?;
    }
    Ok(())
}

fn new_genesis_account(
    account_id: &iroha_data_model::account::AccountId,
) -> iroha_data_model::account::NewAccount {
    Account::new(account_id.clone())
}

fn configure_replay_fixture_parameters(state: &State) {
    let mut parameters = state.world.parameters.block();
    parameters.sumeragi.key_require_hsm = false;
    parameters.set_parameter(iroha_data_model::parameter::system::Parameter::Custom(
        SumeragiNposParameters::default().into_custom_parameter(),
    ));
    parameters.commit();
}

fn rebind_test_execution_context_validators_and_resign(
    block: &mut SignedBlock,
    topology: &crate::sumeragi::network_topology::Topology,
    private_key: &iroha_crypto::PrivateKey,
) {
    let mut context = block
        .execution_context()
        .cloned()
        .expect("state-free block fixture must carry execution context");
    let mut validators = topology.as_ref().to_vec();
    validators.sort();
    validators.dedup();
    let validator_count = u32::try_from(validators.len()).expect("test validator count fits u32");
    let min_quorum = u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
        validators.len(),
    ))
    .expect("test quorum fits u32");
    for ownership in &mut context.lane_payload_ownerships {
        ownership.lane_block_descriptor_validator_set = validators.clone();
        ownership.lane_block_descriptor_validator_count = validator_count;
        ownership.lane_block_descriptor_min_quorum = min_quorum;
        let hashes = ownership
            .compute_replay_hashes()
            .expect("rebind state-free execution-context replay hashes");
        ownership.subject_hash = hashes.subject_hash;
        ownership.payload_ownership_hash = hashes.payload_ownership_hash;
        ownership.rbc_instance_hash = hashes.rbc_instance_hash;
        ownership.lane_block_descriptor_hash = Some(hashes.lane_block_descriptor_hash);
    }
    block.set_execution_context(Some(context));
    let signature = iroha_data_model::block::BlockSignature::new(
        0,
        iroha_crypto::SignatureOf::try_from_hash(private_key, block.header().hash())
            .expect("re-sign rebound test execution context"),
    );
    block
        .replace_signatures(std::collections::BTreeSet::from([signature]))
        .expect("replace rebound block signature");
}

fn clear_test_execution_context_and_resign(
    block: &mut SignedBlock,
    private_key: &iroha_crypto::PrivateKey,
) {
    block.set_execution_context(None);
    let signature = iroha_data_model::block::BlockSignature::new(
        0,
        iroha_crypto::SignatureOf::try_from_hash(private_key, block.header().hash())
            .expect("re-sign legacy replay fixture"),
    );
    block
        .replace_signatures(std::collections::BTreeSet::from([signature]))
        .expect("replace legacy replay fixture signature");
}

fn previous_roster_evidence_for_parent(
    parent: &SignedBlock,
    roster: &[PeerId],
) -> iroha_data_model::consensus::PreviousRosterEvidence {
    let zero_state_root = iroha_crypto::Hash::prehashed([0_u8; iroha_crypto::Hash::LENGTH]);
    let mut signers_bitmap = vec![0_u8; roster.len().div_ceil(8)];
    if let Some(first_byte) = signers_bitmap.first_mut() {
        *first_byte = 1;
    }
    iroha_data_model::consensus::PreviousRosterEvidence {
        height: parent.header().height().get(),
        block_hash: parent.hash(),
        validator_checkpoint: iroha_data_model::consensus::ValidatorSetCheckpoint::new(
            parent.header().height().get(),
            parent.header().view_change_index(),
            parent.hash(),
            zero_state_root,
            zero_state_root,
            roster.to_vec(),
            signers_bitmap,
            Vec::new(),
            iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            None,
        ),
        stake_snapshot: None,
    }
}

fn commit_replay_validated_block_with_options(
    state: &State,
    topology: &crate::sumeragi::network_topology::Topology,
    block: SignedBlock,
    genesis_account: &AccountId,
    skip_block_signatures: bool,
    store_wsv_checkpoint: bool,
) -> SignedBlock {
    let time_source = TimeSource::new_system();
    let mut voting_block = None;
    let validation = if skip_block_signatures {
        ValidBlock::validate_keep_voting_block_for_replay(
            block,
            topology,
            genesis_account,
            &time_source,
            state,
            &mut voting_block,
            false,
            true,
        )
    } else {
        ValidBlock::validate_keep_voting_block(
            block,
            topology,
            genesis_account,
            &time_source,
            state,
            &mut voting_block,
            false,
        )
    };
    let (valid_block, mut state_block) = validation
        .unpack(|_| {})
        .expect("block validates for replay fixture");
    let committed = valid_block.commit_unchecked().unpack(|_| {});
    let committed_signed = committed.as_ref().clone();
    state
        .kura
        .store_block(Arc::new(committed_signed.clone()))
        .expect("store committed replay fixture block");
    let _events = state_block.apply_without_execution(&committed, topology.as_ref().to_vec());
    state_block.prepare_replay_checkpoint_preview();
    let staged_reference = crate::snapshot::canonical_staged_state_snapshot_bytes(&state_block);
    let staged_hash = crate::snapshot::canonical_staged_state_snapshot_hash(&state_block);
    assert_eq!(
        staged_hash,
        Hash::new(staged_reference),
        "borrowed staged WSV hashing must match the canonical tree reference"
    );
    state_block.commit().expect("commit replay fixture block");
    assert_eq!(
        staged_hash,
        crate::snapshot::canonical_state_snapshot_hash(state),
        "staged canonical snapshot hash must equal the exact committed WSV hash"
    );
    if store_wsv_checkpoint {
        state
            .kura
            .store_wsv_checkpoint(
                committed_signed.header().height().get(),
                committed_signed.hash(),
                crate::snapshot::canonical_state_snapshot_hash(state),
            )
            .expect("store committed replay fixture WSV checkpoint");
    }
    committed_signed
}

fn commit_replay_validated_block_with_signature_mode(
    state: &State,
    topology: &crate::sumeragi::network_topology::Topology,
    block: SignedBlock,
    genesis_account: &AccountId,
    skip_block_signatures: bool,
) -> SignedBlock {
    commit_replay_validated_block_with_options(
        state,
        topology,
        block,
        genesis_account,
        skip_block_signatures,
        true,
    )
}

fn commit_replay_validated_block(
    state: &State,
    topology: &crate::sumeragi::network_topology::Topology,
    block: SignedBlock,
    genesis_account: &AccountId,
) -> SignedBlock {
    commit_replay_validated_block_with_signature_mode(
        state,
        topology,
        block,
        genesis_account,
        false,
    )
}

fn configure_private_replay_route(state: &mut State, lane_id: LaneId, dataspace_id: DataSpaceId) {
    let lane_catalog = LaneCatalog::new(
        std::num::NonZeroU32::new(4).expect("non-zero lane count"),
        vec![
            LaneConfig::default(),
            LaneConfig {
                id: lane_id,
                dataspace_id,
                alias: "private-fixture".to_owned(),
                visibility: LaneVisibility::Restricted,
                ..LaneConfig::default()
            },
        ],
    )
    .expect("lane catalog");
    let dataspace_catalog = DataSpaceCatalog::new(vec![
        DataSpaceMetadata::default(),
        DataSpaceMetadata {
            id: dataspace_id,
            alias: "private-fixture".to_owned(),
            description: Some("private replay fixture dataspace".to_owned()),
            fault_tolerance: 1,
        },
    ])
    .expect("dataspace catalog");
    {
        let nexus = state.nexus.get_mut();
        nexus.enabled = true;
        nexus.lane_catalog = lane_catalog;
        nexus.dataspace_catalog = dataspace_catalog.clone();
        nexus.routing_policy.default_lane = lane_id;
        nexus.routing_policy.default_dataspace = dataspace_id;
    }
}

fn replay_fixture_state(
    kura: Arc<Kura>,
    chain_id: ChainId,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) -> State {
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let world = World::with(
        [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_id)],
        [new_genesis_account(&genesis_id).build(&genesis_id)],
        [],
    );
    let mut state = State::new_with_chain(
        world,
        kura,
        crate::query::store::LiveQueryStore::start_test(),
        chain_id,
    );
    configure_private_replay_route(&mut state, lane_id, dataspace_id);
    let configured_nexus = state.nexus.get_mut().clone();
    state.install_pre_genesis_nexus_for_testing(configured_nexus);
    let manifests = {
        let nexus = state.nexus.get_mut();
        Arc::new(LaneManifestRegistry::empty().rebind(&nexus.lane_catalog, &nexus.governance))
    };
    state.install_lane_manifests(&manifests);
    configure_replay_fixture_parameters(&state);
    state
}

fn seed_space_directory_manifest_for_legacy_checkpoint_test(state: &State, dataspace: DataSpaceId) {
    let uaid = UniversalAccountId::from_hash(iroha_crypto::Hash::new(
        b"strict-replay-legacy-checkpoint-surface",
    ));
    let manifest = AssetPermissionManifest {
        version: ManifestVersion::default(),
        uaid,
        dataspace,
        issued_ms: 0,
        activation_epoch: 1,
        expiry_epoch: None,
        entries: Vec::new(),
    };
    let mut record = crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(manifest);
    record.lifecycle.mark_activated(1);
    let mut set = crate::nexus::space_directory::SpaceDirectoryManifestSet::default();
    set.upsert(record);
    let mut manifests = state.world.space_directory_manifests.block();
    manifests.insert(uaid, set);
    manifests.commit();
}

fn replay_missing_checkpoint_fixture(
    checkpoint_exists_only_at_later_height: bool,
) -> (eyre::Report, usize) {
    let suffix = if checkpoint_exists_only_at_later_height {
        "before-first-present"
    } else {
        "height-one"
    };
    let chain_id = ChainId::try_from(format!("iroha:test:missing-replay-checkpoint:{suffix}"))
        .expect("canonical replay-checkpoint test chain id");
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let tx = TransactionBuilder::new_genesis(
        genesis_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(iroha_logger::Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let genesis = SignedBlock::genesis(
        vec![tx],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
    );
    let leader = crate::state::checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
    let topology = crate::sumeragi::network_topology::Topology::new(vec![PeerId::new(
        leader.public_key().clone(),
    )]);
    let kura = Kura::blank_kura_for_testing();
    kura.store_block(Arc::new(genesis.clone()))
        .expect("store genesis without checkpoint");

    let block_count = if checkpoint_exists_only_at_later_height {
        let block2 = crate::block::BlockBuilder::new(Vec::new())
            .chain(0, Some(&genesis))
            .sign(leader.private_key())
            .unpack(|_| {});
        let block2: SignedBlock = block2.into();
        kura.store_block(Arc::new(block2.clone()))
            .expect("store later block");
        kura.store_wsv_checkpoint(
            2,
            block2.hash(),
            iroha_crypto::Hash::new(b"unreachable later checkpoint"),
        )
        .expect("store checkpoint only after the missing prefix");
        2
    } else {
        1
    };

    let world = World::with(
        [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_id)],
        [new_genesis_account(&genesis_id).build(&genesis_id)],
        [],
    );
    let mut state = State::new_with_chain(
        world,
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id,
    );
    let err = replay_blocks_from_kura(
        &kura,
        &mut state,
        &topology,
        block_count,
        ConsensusMode::Permissioned,
    )
    .expect_err("full-body replay must require an exact WSV checkpoint at every height");
    let height = state.view().height();
    (err, height)
}

#[test]
fn replay_rejects_missing_wsv_checkpoint_at_height_one() {
    let (err, height) = replay_missing_checkpoint_fixture(false);
    assert_eq!(err.to_string(), "missing WSV checkpoint for full block #1");
    assert_eq!(
        height, 0,
        "missing checkpoint must fail before WSV mutation"
    );
}

#[test]
fn replay_rejects_missing_checkpoint_before_first_present_checkpoint() {
    let (err, height) = replay_missing_checkpoint_fixture(true);
    assert_eq!(err.to_string(), "missing WSV checkpoint for full block #1");
    assert_eq!(
        height, 0,
        "a later checkpoint cannot authorize an unbound prefix"
    );
}

#[test]
fn replay_always_rejects_corrupted_genesis_signature() {
    let genesis_account = iroha_data_model::account::AccountId::new(
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
    );

    let tx = TransactionBuilder::new_genesis(
        genesis_account.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(iroha_logger::Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());

    let rogue_signer = crate::state::checked_keypair();
    let bad_block = SignedBlock::genesis(vec![tx], rogue_signer.private_key(), None, None);

    let kura = Kura::blank_kura_for_testing();
    let block_arc = Arc::new(bad_block);
    kura.store_block(Arc::clone(&block_arc))
        .expect("store corrupted genesis");
    kura.store_wsv_checkpoint(
        1,
        block_arc.hash(),
        iroha_crypto::Hash::new(b"unreachable corrupted genesis checkpoint"),
    )
    .expect("store checkpoint so signature validation is exercised");

    let query_handle = crate::query::store::LiveQueryStore::start_test();
    let world = World::with(
        [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_account)],
        [new_genesis_account(&genesis_account).build(&genesis_account)],
        [],
    );
    let mut state = State::new(world, Arc::clone(&kura), query_handle);

    let leader = crate::state::checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
    let topology = crate::sumeragi::network_topology::Topology::new(vec![
        iroha_data_model::peer::PeerId::new(leader.public_key().clone()),
    ]);

    let err = replay_blocks_from_kura(&kura, &mut state, &topology, 1, ConsensusMode::Permissioned)
        .expect_err("replay must never bypass a corrupt block signature");
    assert!(
        err.to_string()
            .contains("failed to validate block #1 during replay"),
        "unexpected replay rejection: {err:?}"
    );
    assert_eq!(
        state.view().height(),
        0,
        "invalid block must not mutate WSV"
    );
}

#[test]
fn replay_skips_hash_only_blocks_only_when_restored_state_hash_matches() {
    let chain_id = ChainId::from("iroha:test:hash-only-replay");
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let make_state = |kura: Arc<Kura>| {
        let world = World::with(
            [Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_id)],
            [new_genesis_account(&genesis_id).build(&genesis_id)],
            [],
        );
        State::new_with_chain(
            world,
            kura,
            crate::query::store::LiveQueryStore::start_test(),
            chain_id.clone(),
        )
    };

    let kura = Kura::blank_kura_for_testing();
    let snapshot_hash =
        HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x7A; Hash::LENGTH]));
    kura.extend_hash_only_prefix_from_snapshot(&[snapshot_hash])
        .expect("install hash-only snapshot prefix");
    let height = NonZeroUsize::new(1).expect("non-zero test height");
    assert!(kura.is_hash_only_block_height(height));
    assert!(kura.get_block(height).is_none());

    let leader = crate::state::checked_keypair_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
    let topology = crate::sumeragi::network_topology::Topology::new(vec![PeerId::new(
        leader.public_key().clone(),
    )]);
    let mut restored_state = make_state(Arc::clone(&kura));
    restored_state.push_block_hash_for_testing(snapshot_hash);
    replay_blocks_from_kura_range(
        &kura,
        &mut restored_state,
        &topology,
        1,
        1,
        iroha_data_model::block::consensus_v2::ConsensusMode::Permissioned,
    )
    .expect("hash-only block covered by the restored state snapshot should be skipped");

    let mut unhydrated_state = make_state(Arc::clone(&kura));
    let missing_snapshot = replay_blocks_from_kura_range(
        &kura,
        &mut unhydrated_state,
        &topology,
        1,
        1,
        ConsensusMode::Permissioned,
    )
    .expect_err("hash-only replay requires a restored state hash");
    assert!(
        missing_snapshot
            .to_string()
            .contains("not covered by the restored state block-hash list"),
        "{missing_snapshot:?}"
    );

    let mut mismatched_state = make_state(Arc::clone(&kura));
    mismatched_state.push_block_hash_for_testing(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([0x7B; Hash::LENGTH]),
    ));
    let mismatch = replay_blocks_from_kura_range(
        &kura,
        &mut mismatched_state,
        &topology,
        1,
        1,
        ConsensusMode::Permissioned,
    )
    .expect_err("hash-only replay requires the restored state hash to match Kura");
    assert!(
        mismatch
            .to_string()
            .contains("does not match restored state hash"),
        "{mismatch:?}"
    );
}

#[test]
fn replay_from_height_catches_up_state() {
    run_replay_validation_test_on_stack(
        "replay_from_height_catches_up_state",
        replay_from_height_catches_up_state_impl,
    );
}

#[allow(clippy::too_many_lines)]
fn replay_from_height_catches_up_state_impl() {
    use std::borrow::Cow;

    use iroha_crypto::Algorithm;
    use iroha_data_model::peer::PeerId;
    use iroha_genesis::GENESIS_DOMAIN_ID;

    let chain_id = ChainId::from("iroha:test:partial-replay");
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let leader = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = crate::sumeragi::network_topology::Topology::new(vec![PeerId::new(
        leader.public_key().clone(),
    )]);

    let user_keypair = crate::state::checked_keypair_with_algorithm(Algorithm::Ed25519);
    let user_domain_id: DomainId = DomainId::try_new("users", "universal").expect("domain id");
    let user_id = iroha_data_model::account::AccountId::new(user_keypair.public_key().clone());
    let tx_genesis = TransactionBuilder::new_genesis(
        genesis_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(iroha_logger::Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let genesis_block = SignedBlock::genesis(
        vec![tx_genesis],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
    );

    let tx_block2 = TransactionBuilder::new(
        *DEFAULT_TEST_NETWORK_ID,
        user_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(iroha_logger::Level::INFO, "block2".to_owned())])
    .sign(user_keypair.private_key());
    let accepted_block2 = crate::prelude::AcceptedTransaction::new_unchecked(Cow::Owned(tx_block2));
    let block2 = crate::block::BlockBuilder::new(vec![accepted_block2])
        .chain(0, Some(&genesis_block))
        .sign(leader.private_key())
        .unpack(|_| {});
    let signed_block2: SignedBlock = block2.into();

    let tx_block3 = TransactionBuilder::new(
        *DEFAULT_TEST_NETWORK_ID,
        user_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(iroha_logger::Level::INFO, "block3".to_owned())])
    .sign(user_keypair.private_key());
    let accepted_block3 = crate::prelude::AcceptedTransaction::new_unchecked(Cow::Owned(tx_block3));
    let block3 = crate::block::BlockBuilder::new(vec![accepted_block3])
        .chain(0, Some(&signed_block2))
        .with_previous_roster_evidence(Some(previous_roster_evidence_for_parent(
            &signed_block2,
            topology.as_ref(),
        )))
        .sign(leader.private_key())
        .unpack(|_| {});
    let signed_block3: SignedBlock = block3.into();

    let make_world = || {
        World::with(
            [
                Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_id),
                Domain::new(user_domain_id.clone()).build(&genesis_id),
            ],
            [
                new_genesis_account(&genesis_id).build(&genesis_id),
                Account::new(user_id.clone()).build(&genesis_id),
            ],
            [],
        )
    };
    let kura = Kura::blank_kura_for_testing();
    let materialize_state = State::new_with_chain(
        make_world(),
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id.clone(),
    );
    configure_replay_fixture_parameters(&materialize_state);
    let genesis_block =
        commit_replay_validated_block(&materialize_state, &topology, genesis_block, &genesis_id);
    let signed_block2 =
        commit_replay_validated_block(&materialize_state, &topology, signed_block2, &genesis_id);
    let signed_block3 = commit_replay_validated_block_with_options(
        &materialize_state,
        &topology,
        signed_block3,
        &genesis_id,
        false,
        false,
    );
    kura.store_block(Arc::new(genesis_block))
        .expect("store genesis");
    kura.store_block(Arc::new(signed_block2.clone()))
        .expect("store block2");
    kura.store_block(Arc::new(signed_block3.clone()))
        .expect("store block3");

    let mut state = State::new_with_chain(
        make_world(),
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id.clone(),
    );
    configure_replay_fixture_parameters(&state);

    replay_blocks_from_kura(&kura, &mut state, &topology, 2, ConsensusMode::Permissioned)
        .expect("replay first two blocks");
    assert_eq!(state.view().height(), 2);

    let missing_checkpoint = replay_blocks_from_kura_range(
        &kura,
        &mut state,
        &topology,
        3,
        3,
        ConsensusMode::Permissioned,
    )
    .expect_err("range replay must reject a missing full-body checkpoint");
    assert!(
        missing_checkpoint
            .to_string()
            .contains("missing WSV checkpoint for full block #3"),
        "{missing_checkpoint:?}"
    );
    assert_eq!(state.view().height(), 2);

    kura.store_wsv_checkpoint(
        3,
        signed_block3.hash(),
        crate::snapshot::canonical_state_snapshot_hash(&materialize_state),
    )
    .expect("store block3 checkpoint");
    replay_blocks_from_kura_range(
        &kura,
        &mut state,
        &topology,
        3,
        3,
        ConsensusMode::Permissioned,
    )
    .expect("replay remaining block");
    let view = state.view();
    assert_eq!(view.height(), 3);
    assert_eq!(view.latest_block_hash(), Some(signed_block3.hash()));
}

#[test]
fn replay_rotates_topology_for_npos_prf_leader() {
    run_replay_validation_test_on_stack(
        "replay_rotates_topology_for_npos_prf_leader",
        replay_rotates_topology_for_npos_prf_leader_impl,
    );
}

#[allow(clippy::too_many_lines)]
fn replay_rotates_topology_for_npos_prf_leader_impl() {
    use std::borrow::Cow;

    use iroha_crypto::Algorithm;
    use iroha_data_model::{
        parameter::system::{Parameter, SumeragiConsensusMode, SumeragiNposParameters},
        peer::PeerId,
    };
    use iroha_genesis::{GENESIS_DOMAIN_ID, GenesisBuilder, GenesisTopologyEntry};
    use iroha_test_samples::{
        SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR, gen_account_in,
    };

    let chain_id = ChainId::from("iroha:test:npos-replay");
    let peer_a = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let peer_b = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let peers = vec![
        PeerId::new(peer_a.public_key().clone()),
        PeerId::new(peer_b.public_key().clone()),
    ];
    let topology = crate::sumeragi::network_topology::Topology::new(peers.clone());

    let height = 2;
    let view = 0u64;
    let seed = (1u8..=255)
        .map(|byte| [byte; 32])
        .find(|candidate| topology.leader_index_prf(*candidate, height, view) != 0)
        .expect("seed should select non-zero leader index");
    let leader_index = topology.leader_index_prf(seed, height, view);
    assert_ne!(leader_index, 0, "leader rotation must be exercised");

    let npos_params = SumeragiNposParameters {
        epoch_seed: seed,
        ..Default::default()
    };

    let topology_entries = vec![
        GenesisTopologyEntry::new(
            PeerId::new(peer_a.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(peer_a.private_key()).expect("generate pop a"),
        ),
        GenesisTopologyEntry::new(
            PeerId::new(peer_b.public_key().clone()),
            iroha_crypto::bls_normal_pop_prove(peer_b.private_key()).expect("generate pop b"),
        ),
    ];

    let (user_id, user_keypair) = gen_account_in("wonderland");
    let mut genesis_builder =
        GenesisBuilder::new_without_executor(chain_id.clone(), "ivm/libs/not/installed")
            .set_topology(topology_entries)
            .append_parameter(Parameter::Custom(npos_params.into_custom_parameter()));
    genesis_builder = genesis_builder
        .domain(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .account(user_keypair.public_key().clone())
        .finish_domain();
    let genesis_block = genesis_builder
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Npos)
        .with_consensus_meta()
        .build_and_sign(&SAMPLE_GENESIS_ACCOUNT_KEYPAIR)
        .expect("genesis");
    let genesis_signed = genesis_block.0.clone();

    let kura = Kura::blank_kura_for_testing();
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let make_world = || {
        World::with(
            [Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_id)],
            [new_genesis_account(&genesis_id).build(&genesis_id)],
            [],
        )
    };

    let tx = TransactionBuilder::new(
        *DEFAULT_TEST_NETWORK_ID,
        user_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        iroha_logger::Level::INFO,
        "npos replay".to_owned(),
    )])
    .sign(user_keypair.private_key());
    let accepted = crate::prelude::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let mut base_topology = crate::sumeragi::network_topology::Topology::new(peers.clone());
    base_topology.block_committed(peers.clone(), genesis_signed.hash());
    let leader_peer = base_topology
        .as_ref()
        .get(leader_index)
        .expect("leader index within topology");
    let signer = if leader_peer.public_key() == peer_a.public_key() {
        peer_a.private_key()
    } else {
        peer_b.private_key()
    };
    let new_block = crate::block::BlockBuilder::new(vec![accepted])
        .chain(0, Some(&genesis_signed))
        .sign(signer)
        .unpack(|_| {});
    let mut signed_block: SignedBlock = new_block.into();
    let mut validation_topology = crate::sumeragi::network_topology::Topology::new(peers.clone());
    validation_topology.rotate_preserve_view_to_front(leader_index);
    rebind_test_execution_context_validators_and_resign(
        &mut signed_block,
        &validation_topology,
        signer,
    );
    let block_arc = Arc::new(signed_block);
    let materialize_state = State::new_with_chain(
        make_world(),
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id.clone(),
    );
    configure_replay_fixture_parameters(&materialize_state);
    let genesis_signed =
        commit_replay_validated_block(&materialize_state, &topology, genesis_signed, &genesis_id);
    let signed_block = commit_replay_validated_block(
        &materialize_state,
        &validation_topology,
        (*block_arc).clone(),
        &genesis_id,
    );
    kura.store_block(Arc::new(genesis_signed))
        .expect("store genesis");
    kura.store_block(Arc::new(signed_block))
        .expect("store block");

    let mut state = State::new_with_chain(
        make_world(),
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id.clone(),
    );
    {
        let mut params_block = state.world.parameters.block();
        params_block.sumeragi.key_require_hsm = false;
        params_block.commit();
    }

    replay_blocks_from_kura(&kura, &mut state, &topology, 2, ConsensusMode::Npos)
        .expect("replay should validate prf leader");
    assert_eq!(state.view().height(), 2);
}

#[test]
fn replay_uses_commit_roster_journal_for_signature_order() {
    run_replay_validation_test_on_stack(
        "replay_uses_commit_roster_journal_for_signature_order",
        replay_uses_commit_roster_journal_for_signature_order_impl,
    );
}

#[allow(clippy::too_many_lines)]
fn replay_uses_commit_roster_journal_for_signature_order_impl() {
    use std::borrow::Cow;

    use iroha_config::{
        base::WithOrigin,
        kura::InitMode,
        parameters::actual::{Kura as KuraConfig, LaneConfig as RuntimeLaneConfig},
    };
    use iroha_crypto::Algorithm;
    use iroha_data_model::{consensus::VALIDATOR_SET_HASH_VERSION_V1, peer::PeerId};
    use iroha_genesis::{GENESIS_DOMAIN_ID, GenesisBuilder, GenesisTopologyEntry};
    use iroha_test_samples::{
        SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR, gen_account_in,
    };

    let chain_id = ChainId::from("iroha:test:replay-roster-journal");
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let kura_cfg = KuraConfig {
        init_mode: InitMode::Strict,
        store_dir: WithOrigin::inline(temp_dir.path().join("kura")),
        max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory: iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity:
            iroha_config::parameters::defaults::kura::MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: iroha_config::kura::FsyncMode::Batched,
        fsync_interval: iroha_config::parameters::defaults::kura::FSYNC_INTERVAL,
        block_sync_roster_retention:
            iroha_config::parameters::defaults::kura::BLOCK_SYNC_ROSTER_RETENTION,
        roster_sidecar_retention:
            iroha_config::parameters::defaults::kura::ROSTER_SIDECAR_RETENTION,
        replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
    };
    let (kura, _) = Kura::new(&kura_cfg, &RuntimeLaneConfig::default()).expect("init kura");

    let peer_a = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let peer_b = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let roster = vec![
        PeerId::new(peer_b.public_key().clone()),
        PeerId::new(peer_a.public_key().clone()),
    ];
    let topology_entries = vec![
        GenesisTopologyEntry::new(
            roster[0].clone(),
            iroha_crypto::bls_normal_pop_prove(peer_b.private_key()).expect("generate pop b"),
        ),
        GenesisTopologyEntry::new(
            roster[1].clone(),
            iroha_crypto::bls_normal_pop_prove(peer_a.private_key()).expect("generate pop a"),
        ),
    ];

    let (user_id, user_keypair) = gen_account_in("wonderland");
    let mut genesis_builder =
        GenesisBuilder::new_without_executor(chain_id.clone(), "ivm/libs/not/installed")
            .set_topology(topology_entries);
    genesis_builder = genesis_builder
        .domain(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .account(user_keypair.public_key().clone())
        .finish_domain();
    let genesis_block = genesis_builder
        .build_and_sign(&SAMPLE_GENESIS_ACCOUNT_KEYPAIR)
        .expect("genesis");
    let genesis_signed = genesis_block.0.clone();

    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let world = World::with(
        [Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_id)],
        [new_genesis_account(&genesis_id).build(&genesis_id)],
        [],
    );
    let state = State::new_with_chain(
        world,
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id.clone(),
    );
    {
        let mut params_block = state.world.parameters.block();
        params_block.sumeragi.key_require_hsm = false;
        params_block.commit();
    }
    let genesis_signed = commit_replay_validated_block_with_options(
        &state,
        &crate::sumeragi::network_topology::Topology::new(roster.clone()),
        genesis_signed,
        &genesis_id,
        false,
        true,
    );
    kura.store_block(Arc::new(genesis_signed.clone()))
        .expect("store genesis");
    configure_replay_fixture_parameters(&state);

    let tx = TransactionBuilder::new(
        state.network_id,
        user_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        iroha_logger::Level::INFO,
        "replay roster journal".to_owned(),
    )])
    .sign(user_keypair.private_key());
    let accepted = crate::prelude::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let new_block = crate::block::BlockBuilder::new(vec![accepted])
        .chain(0, Some(&genesis_signed))
        .sign(peer_b.private_key())
        .unpack(|_| {});
    let mut signed_block: SignedBlock = new_block.into();
    let height = signed_block.header().height().get();
    let view = signed_block.header().view_change_index();
    let prf_seed = {
        let view = state.view();
        crate::sumeragi::prf_seed_for_height(&view, height)
    };
    // Align the block signature with replay's PRF-rotated topology.
    let mut signature_topology = crate::sumeragi::network_topology::Topology::new(roster.clone());
    signature_topology.canonicalize_order();
    signature_topology.shuffle_prf(prf_seed, height);
    signature_topology.nth_rotation(view);
    let signer_key = if signature_topology.leader().public_key() == peer_a.public_key() {
        peer_a.private_key()
    } else {
        peer_b.private_key()
    };
    rebind_test_execution_context_validators_and_resign(
        &mut signed_block,
        &signature_topology,
        signer_key,
    );

    let signed_block = commit_replay_validated_block_with_options(
        &state,
        &signature_topology,
        signed_block,
        &genesis_id,
        false,
        true,
    );
    kura.store_block(Arc::new(signed_block.clone()))
        .expect("store block");

    let signatures: Vec<_> = signed_block.signatures().cloned().collect();
    let mut signers_bitmap = vec![0u8; roster.len().div_ceil(8)];
    for signature in &signatures {
        let idx = usize::try_from(signature.index()).unwrap_or(usize::MAX);
        if idx < roster.len() {
            signers_bitmap[idx / 8] |= 1u8 << (idx % 8);
        }
    }
    let block_hash = signed_block.hash();
    let validator_set_hash = HashOf::new(&roster);
    let commit_cert = Qc {
        phase: crate::sumeragi::consensus::Phase::Commit,
        subject_block_hash: block_hash,
        parent_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
        post_state_root: iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]),
        height,
        view,
        epoch: 0,
        chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
        rechain_seq: 0,
        mode_tag: crate::sumeragi::consensus::PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash,
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set: roster.clone(),
        aggregate: crate::sumeragi::consensus::QcAggregate {
            signers_bitmap: signers_bitmap.clone(),
            bls_aggregate_signature: Vec::new(),
        },
    };
    let checkpoint = ValidatorSetCheckpoint::new(
        height,
        view,
        block_hash,
        commit_cert.parent_state_root,
        commit_cert.post_state_root,
        roster,
        signers_bitmap,
        Vec::new(),
        VALIDATOR_SET_HASH_VERSION_V1,
        None,
    );
    state
        .record_commit_roster(&commit_cert, &checkpoint, None)
        .expect("commit-roster test setup should retain known journal durability");

    let replay_world = World::with(
        [Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_id)],
        [new_genesis_account(&genesis_id).build(&genesis_id)],
        [],
    );
    let mut replay_state = State::new_with_chain(
        replay_world,
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id.clone(),
    );
    {
        let mut params_block = replay_state.world.parameters.block();
        params_block.sumeragi.key_require_hsm = false;
        params_block.commit();
    }

    let fallback_topology = crate::sumeragi::network_topology::Topology::new(vec![
        PeerId::new(peer_a.public_key().clone()),
        PeerId::new(peer_b.public_key().clone()),
    ]);

    replay_blocks_from_kura_range(
        &kura,
        &mut replay_state,
        &fallback_topology,
        1,
        1,
        ConsensusMode::Permissioned,
    )
    .expect("replay permissioned genesis before installing test-only penalty parameters");
    configure_replay_fixture_parameters(&replay_state);
    replay_blocks_from_kura_range(
        &kura,
        &mut replay_state,
        &fallback_topology,
        2,
        2,
        ConsensusMode::Permissioned,
    )
    .expect("replay should honor commit roster journal ordering");
    assert_eq!(replay_state.view().height(), 2);
}

#[test]
fn replay_rejects_non_authoritative_signature_topology_rotation() {
    run_replay_validation_test_on_stack(
        "replay_rejects_non_authoritative_signature_rotation",
        replay_rejects_non_authoritative_signature_topology_rotation_impl,
    );
}

#[allow(clippy::too_many_lines)]
fn replay_rejects_non_authoritative_signature_topology_rotation_impl() {
    use std::borrow::Cow;

    use iroha_crypto::Algorithm;
    use iroha_data_model::{DomainId, account::AccountId, peer::PeerId};
    use iroha_genesis::GENESIS_DOMAIN_ID;

    let chain_id = ChainId::from("iroha:test:replay-signature-rotation-recovery");
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let user_keypair = crate::state::checked_keypair_with_algorithm(Algorithm::Ed25519);
    let user_domain: DomainId = DomainId::try_new("users", "universal").expect("domain id");
    let user_id = AccountId::new(user_keypair.public_key().clone());

    crate::sumeragi::status::reset_commit_certs_for_tests();
    crate::sumeragi::status::reset_validator_checkpoints_for_tests();

    let peer_a = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let peer_b = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let fallback_peers = vec![
        PeerId::new(peer_a.public_key().clone()),
        PeerId::new(peer_b.public_key().clone()),
    ];
    let fallback_topology =
        crate::sumeragi::network_topology::Topology::new(fallback_peers.clone());

    let tx_genesis = TransactionBuilder::new_genesis(
        genesis_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(iroha_logger::Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let genesis_block = SignedBlock::genesis(
        vec![tx_genesis],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
    );

    let world = World::with(
        [
            Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_id),
            Domain::new(user_domain.clone()).build(&genesis_id),
        ],
        [
            new_genesis_account(&genesis_id).build(&genesis_id),
            Account::new(user_id.clone()).build(&genesis_id),
        ],
        [],
    );
    let kura = Kura::blank_kura_for_testing();
    let query = crate::query::store::LiveQueryStore::start_test();
    let state = State::new_with_chain(world, Arc::clone(&kura), query, chain_id.clone());
    configure_replay_fixture_parameters(&state);
    let genesis_block =
        commit_replay_validated_block(&state, &fallback_topology, genesis_block, &genesis_id);
    kura.store_block(Arc::new(genesis_block.clone()))
        .expect("store genesis");

    let height = 2_u64;
    let view = 0_u64;
    let prf_seed = {
        let state_view = state.view();
        crate::sumeragi::prf_seed_for_height(&state_view, height)
    };
    let mut expected_topology = crate::sumeragi::network_topology::Topology::new(fallback_peers);
    expected_topology.canonicalize_order();
    expected_topology.shuffle_prf(prf_seed, height);
    expected_topology.nth_rotation(view);
    let leader_is_peer_a = expected_topology.leader().public_key() == peer_a.public_key();
    let mismatched_signer = if leader_is_peer_a {
        peer_b.private_key()
    } else {
        peer_a.private_key()
    };

    let tx_block2 = TransactionBuilder::new(
        state.network_id,
        user_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        iroha_logger::Level::INFO,
        "signature-rotation-replay".to_owned(),
    )])
    .sign(user_keypair.private_key());
    let accepted_block2 = crate::prelude::AcceptedTransaction::new_unchecked(Cow::Owned(tx_block2));
    let block2 = crate::block::BlockBuilder::new(vec![accepted_block2])
        .chain(0, Some(&genesis_block))
        // Produce a block then rewrite signatures to a deterministic index/signer mismatch.
        .sign(mismatched_signer)
        .unpack(|_| {});
    let mut signed_block2: SignedBlock = block2.into();
    rebind_test_execution_context_validators_and_resign(
        &mut signed_block2,
        &expected_topology,
        mismatched_signer,
    );

    let signed_block2 = commit_replay_validated_block_with_signature_mode(
        &state,
        &expected_topology,
        signed_block2,
        &genesis_id,
        true,
    );
    kura.store_block(Arc::new(signed_block2.clone()))
        .expect("store block2");

    let replay_world = World::with(
        [
            Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_id),
            Domain::new(user_domain.clone()).build(&genesis_id),
        ],
        [
            new_genesis_account(&genesis_id).build(&genesis_id),
            Account::new(user_id.clone()).build(&genesis_id),
        ],
        [],
    );
    let mut replay_state = State::new_with_chain(
        replay_world,
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id,
    );
    configure_replay_fixture_parameters(&replay_state);

    let err = replay_blocks_from_kura(
        &kura,
        &mut replay_state,
        &fallback_topology,
        2,
        ConsensusMode::Permissioned,
    )
    .expect_err("replay must not retry a failed block under non-authoritative rotations");
    assert_eq!(
        replay_state.view().height(),
        1,
        "wrong-leader block must not mutate WSV"
    );
    assert_eq!(
        replay_state.view().latest_block_hash(),
        Some(genesis_block.hash())
    );
    assert!(
        err.to_string()
            .contains("failed to validate block #2 during replay"),
        "unexpected replay rejection: {err:?}"
    );
}

#[test]
fn replay_rejects_committed_execution_result_mismatch_without_mutating_that_block() {
    run_replay_validation_test_on_stack(
        "replay_rejects_result_mismatch",
        replay_rejects_committed_execution_result_mismatch_impl,
    );
}

fn replay_rejects_committed_execution_result_mismatch_impl() {
    use std::borrow::Cow;

    use iroha_crypto::{Algorithm, Hash};
    use iroha_data_model::transaction::signed::TransactionResultInner;

    let chain_id = ChainId::from("iroha:test:replay-result-mismatch");
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let leader = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = crate::sumeragi::network_topology::Topology::new(vec![PeerId::new(
        leader.public_key().clone(),
    )]);
    let user_keypair = crate::state::checked_keypair_with_algorithm(Algorithm::Ed25519);
    let user_domain_id: DomainId = DomainId::try_new("users", "universal").expect("domain id");
    let user_id = AccountId::new(user_keypair.public_key().clone());
    let make_world = || {
        World::with(
            [
                Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_id),
                Domain::new(user_domain_id.clone()).build(&genesis_id),
            ],
            [
                new_genesis_account(&genesis_id).build(&genesis_id),
                Account::new(user_id.clone()).build(&genesis_id),
            ],
            [],
        )
    };

    let kura = Kura::blank_kura_for_testing();
    let materialize_state = State::new_with_chain(
        make_world(),
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id.clone(),
    );
    configure_replay_fixture_parameters(&materialize_state);

    let tx_genesis = TransactionBuilder::new_genesis(
        genesis_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(iroha_logger::Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let genesis_block = SignedBlock::genesis(
        vec![tx_genesis],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
    );
    let genesis_block =
        commit_replay_validated_block(&materialize_state, &topology, genesis_block, &genesis_id);
    kura.store_block(Arc::new(genesis_block.clone()))
        .expect("store genesis");

    let tx_block2 = TransactionBuilder::new(
        materialize_state.network_id,
        user_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        iroha_logger::Level::INFO,
        "result mismatch".to_owned(),
    )])
    .sign(user_keypair.private_key());
    let accepted_block2 = crate::prelude::AcceptedTransaction::new_unchecked(Cow::Owned(tx_block2));
    let block2 = crate::block::BlockBuilder::new(vec![accepted_block2])
        .chain(0, Some(&genesis_block))
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut signed_block2: SignedBlock = block2.into();
    let entry_hashes = signed_block2
        .external_entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect::<Vec<_>>();
    let bad_result: TransactionResultInner = Err(TransactionRejectionReason::Validation(
        ValidationFail::NotPermitted("forced mismatch".to_owned()),
    ));
    signed_block2
        .set_transaction_results(Vec::new(), &entry_hashes, vec![bad_result])
        .expect("test block entrypoint hash should match payload");
    let block2_hash = signed_block2.hash();
    kura.store_block(Arc::new(signed_block2))
        .expect("store mismatched block");
    kura.store_wsv_checkpoint(2, block2_hash, Hash::new(b"not the replayed WSV"))
        .expect("store mismatched block WSV checkpoint");

    let mut replay_state = State::new_with_chain(
        make_world(),
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id,
    );
    configure_replay_fixture_parameters(&replay_state);

    let err = replay_blocks_from_kura(
        &kura,
        &mut replay_state,
        &topology,
        2,
        ConsensusMode::Permissioned,
    )
    .expect_err("replay must reject committed execution results that it cannot reproduce");
    assert!(
        err.to_string()
            .contains("failed to verify replayed block #2 against committed execution results"),
        "unexpected replay rejection: {err:?}"
    );
    assert_eq!(
        replay_state.view().height(),
        1,
        "the result-mismatched block must be discarded atomically"
    );
    assert_eq!(
        replay_state.view().latest_block_hash(),
        Some(genesis_block.hash())
    );
}

#[test]
fn replay_rejects_exact_wsv_checkpoint_mismatch() {
    run_replay_validation_test_on_stack(
        "replay_rejects_wsv_checkpoint_mismatch",
        replay_rejects_exact_wsv_checkpoint_mismatch_impl,
    );
}

fn replay_rejects_exact_wsv_checkpoint_mismatch_impl() {
    use std::borrow::Cow;

    use iroha_crypto::{Algorithm, Hash};

    let chain_id = ChainId::from("iroha:test:replay-wsv-checkpoint-mismatch");
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let leader = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = crate::sumeragi::network_topology::Topology::new(vec![PeerId::new(
        leader.public_key().clone(),
    )]);
    let user_keypair = crate::state::checked_keypair_with_algorithm(Algorithm::Ed25519);
    let user_domain_id: DomainId = DomainId::try_new("users", "universal").expect("domain id");
    let user_id = AccountId::new(user_keypair.public_key().clone());
    let make_world = || {
        World::with(
            [
                Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_id),
                Domain::new(user_domain_id.clone()).build(&genesis_id),
            ],
            [
                new_genesis_account(&genesis_id).build(&genesis_id),
                Account::new(user_id.clone()).build(&genesis_id),
            ],
            [],
        )
    };

    let kura = Kura::blank_kura_for_testing();
    let materialize_state = State::new_with_chain(
        make_world(),
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id.clone(),
    );
    configure_replay_fixture_parameters(&materialize_state);

    let tx_genesis = TransactionBuilder::new_genesis(
        genesis_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(iroha_logger::Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let genesis_block = SignedBlock::genesis(
        vec![tx_genesis],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
    );
    let genesis_block =
        commit_replay_validated_block(&materialize_state, &topology, genesis_block, &genesis_id);
    kura.store_block(Arc::new(genesis_block.clone()))
        .expect("store genesis");

    let tx_block2 = TransactionBuilder::new(
        materialize_state.network_id,
        user_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        iroha_logger::Level::INFO,
        "checkpoint mismatch".to_owned(),
    )])
    .sign(user_keypair.private_key());
    let accepted_block2 = crate::prelude::AcceptedTransaction::new_unchecked(Cow::Owned(tx_block2));
    let block2 = crate::block::BlockBuilder::new(vec![accepted_block2])
        .chain(0, Some(&genesis_block))
        .sign(leader.private_key())
        .unpack(|_| {});
    let signed_block2 =
        commit_replay_validated_block(&materialize_state, &topology, block2.into(), &genesis_id);
    kura.store_block(Arc::new(signed_block2.clone()))
        .expect("store block2");
    let correct_checkpoint = crate::snapshot::canonical_state_snapshot_hash(&materialize_state);
    kura.overwrite_wsv_checkpoint_without_validation_for_tests(
        2,
        Hash::new(b"not the replayed canonical WSV"),
        None,
    )
    .expect("overwrite block2 WSV checkpoint");

    let mut replay_state = State::new_with_chain(
        make_world(),
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id,
    );
    configure_replay_fixture_parameters(&replay_state);

    replay_blocks_from_kura_range(
        &kura,
        &mut replay_state,
        &topology,
        1,
        1,
        ConsensusMode::Permissioned,
    )
    .expect("genesis replay establishes the exact pre-block state");
    let before_bytes = crate::snapshot::canonical_state_snapshot_bytes(&replay_state);
    let before_height = replay_state.committed_height();
    let before_hash = replay_state.latest_block_hash_fast();
    let before_merge = replay_state.merge_ledger.snapshot();

    let err = replay_blocks_from_kura_range(
        &kura,
        &mut replay_state,
        &topology,
        2,
        2,
        ConsensusMode::Permissioned,
    )
    .expect_err("replay must reject a WSV checkpoint with a forged state hash");
    assert!(
        err.to_string()
            .contains("replayed block #2 WSV checkpoint mismatch"),
        "unexpected replay rejection: {err:?}"
    );
    assert_eq!(replay_state.committed_height(), before_height);
    assert_eq!(replay_state.latest_block_hash_fast(), before_hash);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_bytes(&replay_state),
        before_bytes,
        "checkpoint rejection must leave the live WSV byte-for-byte unchanged"
    );
    let after_merge = replay_state.merge_ledger.snapshot();
    assert_eq!(after_merge.len(), before_merge.len());
    assert!(
        after_merge
            .iter()
            .zip(&before_merge)
            .all(|(after, before)| after.as_ref() == before.as_ref()),
        "checkpoint rejection must not publish merge-cache entries"
    );

    kura.overwrite_wsv_checkpoint_without_validation_for_tests(2, correct_checkpoint, None)
        .expect("replace unbound forged checkpoint with the exact canonical state hash");
    replay_blocks_from_kura_range(
        &kura,
        &mut replay_state,
        &topology,
        2,
        2,
        ConsensusMode::Permissioned,
    )
    .expect("corrected checkpoint must replay successfully after atomic rejection");
    assert_eq!(replay_state.committed_height(), 2);
    assert_eq!(
        replay_state.latest_block_hash_fast(),
        Some(signed_block2.hash())
    );
}

#[test]
fn replay_rejects_legacy_space_directory_checkpoint_surface() {
    run_replay_validation_test_on_stack(
        "replay_rejects_legacy_checkpoint_surface",
        replay_rejects_legacy_space_directory_checkpoint_surface_impl,
    );
}

#[allow(clippy::too_many_lines)]
fn replay_rejects_legacy_space_directory_checkpoint_surface_impl() {
    use std::borrow::Cow;

    use iroha_crypto::Algorithm;
    use iroha_primitives::json::Json;

    let chain_id = ChainId::from("iroha:test:legacy-route-replay");
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let lane_id = LaneId::new(3);
    let dataspace_id = DataSpaceId::new(10);
    let leader = crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let topology = crate::sumeragi::network_topology::Topology::new(vec![PeerId::new(
        leader.public_key().clone(),
    )]);
    let kura = Kura::blank_kura_for_testing();
    let original_state =
        replay_fixture_state(Arc::clone(&kura), chain_id.clone(), lane_id, dataspace_id);
    seed_space_directory_manifest_for_legacy_checkpoint_test(&original_state, dataspace_id);
    let proof_policies = |height| {
        crate::da::active_proof_policy_bundle_at_height(&original_state.nexus_snapshot(), height)
    };

    let tx_genesis = TransactionBuilder::new_genesis(
        genesis_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(iroha_logger::Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let genesis_block = SignedBlock::genesis_with_da_proof_policies(
        vec![tx_genesis],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
        Some(proof_policies(1)),
    );
    let genesis_block =
        commit_replay_validated_block(&original_state, &topology, genesis_block, &genesis_id);
    kura.store_block(Arc::new(genesis_block.clone()))
        .expect("store genesis");

    let user_keypair = crate::state::checked_keypair_with_algorithm(Algorithm::Ed25519);
    let user_id = AccountId::new(user_keypair.public_key().clone());
    let domain_id = DomainId::try_new("settlement", "private-fixture").expect("domain id");
    let asset_definition_id = AssetDefinitionId::derive_from_components(
        domain_id.clone(),
        "credit".parse().expect("asset definition name"),
    );
    let asset_id = AssetId::of(asset_definition_id.clone(), user_id.clone());
    let instructions = vec![
        InstructionBox::from(Register::domain(Domain::new(domain_id.clone()))),
        InstructionBox::from(Register::account(Account::new(user_id.clone()))),
        InstructionBox::from(Register::asset_definition(AssetDefinition::numeric(
            asset_definition_id.clone(),
            "credit".to_owned(),
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))),
        InstructionBox::from(Mint::asset_quantity(7_u32, asset_id.clone())),
        InstructionBox::from(SetKeyValue::account(
            user_id.clone(),
            "tier".parse::<Name>().expect("account metadata key"),
            Json::new("preferred"),
        )),
        InstructionBox::from(SetKeyValue::domain(
            domain_id.clone(),
            "quota".parse::<Name>().expect("domain metadata key"),
            Json::new(7_u32),
        )),
        InstructionBox::from(SetKeyValue::asset_definition(
            asset_definition_id,
            "class".parse::<Name>().expect("asset metadata key"),
            Json::new("retail"),
        )),
    ];
    let tx = TransactionBuilder::new(
        original_state.network_id,
        genesis_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions::<InstructionBox>(instructions)
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let accepted = crate::prelude::AcceptedTransaction::new_unchecked(Cow::Owned(tx));
    let block = crate::block::BlockBuilder::new(vec![accepted])
        .chain(0, Some(&genesis_block))
        .with_da_proof_policies(Some(proof_policies(2)))
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut legacy_block: SignedBlock = block.into();
    clear_test_execution_context_and_resign(&mut legacy_block, leader.private_key());
    assert!(
        legacy_block.execution_context().is_none(),
        "legacy fixture must exercise replay compatibility without an execution context"
    );
    let legacy_block = commit_replay_validated_block_with_signature_mode(
        &original_state,
        &topology,
        legacy_block,
        &genesis_id,
        true,
    );
    assert!(legacy_block.has_results());
    kura.store_block(Arc::new(legacy_block.clone()))
        .expect("store legacy block");
    let canonical_prefix =
        crate::snapshot::canonical_state_snapshot_bytes_for_tests(&original_state);

    let block3_instructions = vec![
        InstructionBox::from(Mint::asset_quantity(5_u32, asset_id.clone())),
        InstructionBox::from(SetKeyValue::account(
            user_id.clone(),
            "status".parse::<Name>().expect("account metadata key"),
            Json::new("settled"),
        )),
        InstructionBox::from(SetKeyValue::domain(
            domain_id.clone(),
            "window".parse::<Name>().expect("domain metadata key"),
            Json::new(2_u32),
        )),
    ];
    let block3_tx = TransactionBuilder::new(
        original_state.network_id,
        genesis_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions::<InstructionBox>(block3_instructions)
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let block3_accepted = crate::prelude::AcceptedTransaction::new_unchecked(Cow::Owned(block3_tx));
    let block3 = crate::block::BlockBuilder::new(vec![block3_accepted])
        .chain(0, Some(&legacy_block))
        .with_previous_roster_evidence(Some(previous_roster_evidence_for_parent(
            &legacy_block,
            topology.as_ref(),
        )))
        .with_da_proof_policies(Some(proof_policies(3)))
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut legacy_block3: SignedBlock = block3.into();
    clear_test_execution_context_and_resign(&mut legacy_block3, leader.private_key());
    assert!(
        legacy_block3.execution_context().is_none(),
        "multi-block legacy fixture must retain missing-context replay compatibility"
    );
    let legacy_block3 = commit_replay_validated_block_with_signature_mode(
        &original_state,
        &topology,
        legacy_block3,
        &genesis_id,
        true,
    );
    assert!(legacy_block3.has_results());
    kura.store_block(Arc::new(legacy_block3.clone()))
        .expect("store second legacy block");
    let canonical_checkpoint = crate::snapshot::canonical_state_snapshot_hash(&original_state);
    let legacy_checkpoint =
        crate::snapshot::legacy_state_snapshot_hash_without_space_directory_manifests(
            &original_state,
        );
    assert_ne!(
        canonical_checkpoint, legacy_checkpoint,
        "test fixture must distinguish the exact first-release WSV from the retired surface"
    );
    kura.overwrite_wsv_checkpoint_without_validation_for_tests(3, legacy_checkpoint, None)
        .expect("overwrite final WSV checkpoint with retired surface hash");

    let replay_kura = Kura::blank_kura_for_testing();
    let mut replay_state =
        replay_fixture_state(Arc::clone(&replay_kura), chain_id, lane_id, dataspace_id);
    seed_space_directory_manifest_for_legacy_checkpoint_test(&replay_state, dataspace_id);
    for height in 1..=3 {
        let height_index = NonZeroUsize::new(height).expect("replay height is non-zero");
        let block = kura
            .get_block(height_index)
            .expect("source replay block exists");
        let checkpoint = kura
            .wsv_checkpoint(u64::try_from(height).expect("test height fits u64"))
            .expect("read source replay checkpoint")
            .expect("source replay checkpoint exists");
        replay_kura
            .store_block(Arc::clone(&block))
            .expect("copy replay block after pre-genesis Nexus installation");
        replay_kura
            .store_wsv_checkpoint(
                u64::try_from(height).expect("test height fits u64"),
                block.hash(),
                checkpoint.state_hash(),
            )
            .expect("copy replay checkpoint");
    }

    let err = replay_blocks_from_kura(
        &replay_kura,
        &mut replay_state,
        &topology,
        3,
        ConsensusMode::Permissioned,
    )
    .expect_err("the retired checkpoint surface must never authorize replayed state");
    assert!(
        err.to_string()
            .contains("replayed block #3 WSV checkpoint mismatch"),
        "unexpected replay rejection: {err:?}"
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_bytes_for_tests(&replay_state),
        canonical_prefix,
        "checkpoint rejection must leave the last exactly authenticated prefix committed"
    );
}
