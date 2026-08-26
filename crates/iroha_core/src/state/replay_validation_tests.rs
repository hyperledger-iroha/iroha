use super::*;
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
use std::sync::Arc;
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
    fixture_consensus_mode: ConsensusMode,
) -> Result<()> {
    replay_blocks_from_kura_range(
        kura,
        state,
        topology,
        1,
        block_count,
        fixture_consensus_mode,
    )
}
/// Exercise checkpoint fixtures through the current Sumeragi-v2 validation profile.
///
/// Production replay additionally authenticates the exact durable finality artifact before
/// reaching this execution boundary; that corridor is covered by `strict_replay_tests`.
pub(super) fn replay_blocks_from_kura_range(
    kura: &Arc<Kura>,
    state: &mut State,
    topology: &crate::sumeragi::network_topology::Topology,
    start_height: usize,
    block_count: usize,
    fixture_consensus_mode: ConsensusMode,
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
        let roster = topology.as_ref().to_vec();
        let mut validation_topology =
            crate::sumeragi::network_topology::Topology::new(roster.clone());
        let view = signed.header().view_change_index();
        let seed = {
            let state_view = state.view();
            replay_fixture_leader_seed(&state_view, height, fixture_consensus_mode)
        };
        match fixture_consensus_mode {
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
        let (valid, mut state_block) = ValidBlock::validate_sumeragi_v2_fixture_keep_voting_block(
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
        state_block.authenticated_replay_commit = true;
        let _ = state_block.apply_without_execution(&committed, roster);
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
fn replay_fixture_leader_seed(
    state_view: &StateView<'_>,
    height: u64,
    mode: ConsensusMode,
) -> [u8; 32] {
    match mode {
        ConsensusMode::Permissioned => {
            let mut preimage = b"sumeragi-v2:permissioned-leader-seed".to_vec();
            preimage.extend_from_slice(&state_view.network_id().encode());
            Hash::new(preimage).into()
        }
        ConsensusMode::Npos => {
            let world = state_view.world();
            assert_eq!(
                crate::sumeragi::epoch_for_height_from_world(world, height, mode)
                    .expect("NPoS replay fixture has committed epoch parameters"),
                0,
                "compact replay fixtures remain inside the signed genesis epoch"
            );
            world
                .sumeragi_npos_parameters()
                .expect("NPoS replay fixture requires committed genesis parameters")
                .epoch_seed()
        }
    }
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
fn rebind_test_confidential_features_and_resign(
    block: &mut SignedBlock,
    state: &State,
    private_key: &iroha_crypto::PrivateKey,
) {
    let height = block.header().height().get();
    let digest = {
        let view = state.query_view();
        compute_confidential_feature_digest(view.world(), view.zk(), view.sccp_registry(), height)
    };
    let mut header = block.header();
    header.set_confidential_features((!digest.is_empty()).then_some(digest));
    block.replace_header_for_testing(header);
    let signature = iroha_data_model::block::BlockSignature::new(
        0,
        iroha_crypto::SignatureOf::try_from_hash(private_key, block.hash())
            .expect("re-sign confidential-feature replay fixture"),
    );
    block
        .replace_signatures(std::collections::BTreeSet::from([signature]))
        .expect("replace confidential-feature replay-fixture signature");
}
fn assert_canonical_successful_fixture_results(block: &SignedBlock) {
    assert!(block.has_results(), "committed fixture must carry results");
    let result_count = block.results().len();
    assert_eq!(result_count, block.entrypoint_hashes().len());
    assert!(
        block.results().all(|result| result.as_ref().is_ok()),
        "committed fixture results must all succeed"
    );
    let minimum_committed_fragment_count =
        u64::try_from(result_count).expect("fixture result count fits u64");
    assert!(
        block
            .committed_fragment_count()
            .is_some_and(|count| count >= minimum_committed_fragment_count),
        "committed fragments must cover every successful external result"
    );
    block
        .validate_entrypoint_merkle_cache()
        .expect("committed fixture entrypoint Merkle cache must be canonical");
    block
        .validate_result_merkle_cache()
        .expect("committed fixture result Merkle cache must be canonical");
    assert_eq!(
        block.header().result_merkle_root(),
        block
            .result_merkle_commitment()
            .map(|commitment| *commitment.root())
    );
}
fn attach_successful_fixture_results(
    mut block: SignedBlock,
    signer: &iroha_crypto::KeyPair,
) -> SignedBlock {
    let entrypoint_hashes = block
        .external_entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect::<Vec<_>>();
    let results = entrypoint_hashes
        .iter()
        .map(|_| Ok(iroha_data_model::transaction::DataTriggerSequence::default()))
        .collect();
    block
        .set_transaction_results(Vec::new(), &entrypoint_hashes, results)
        .expect("attach exact successful replay-fixture results");
    let final_signature = iroha_data_model::block::BlockSignature::new(
        0,
        iroha_crypto::SignatureOf::try_from_hash(signer.private_key(), block.hash())
            .expect("sign result-bearing replay fixture"),
    );
    block
        .replace_signatures(std::collections::BTreeSet::from([final_signature]))
        .expect("replace result-bearing replay-fixture signature");
    assert_canonical_successful_fixture_results(&block);
    {
        let mut final_signatures = block.signatures();
        let final_signature = final_signatures.next().expect("replay-fixture signature");
        assert_eq!(final_signature.index(), 0);
        assert!(final_signatures.next().is_none());
        final_signature
            .signature()
            .verify_hash(signer.public_key(), block.hash())
            .expect("verify result-bearing replay-fixture signature");
    }
    block
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
    let validation = ValidBlock::validate_sumeragi_v2_fixture_keep_voting_block(
        block,
        topology,
        genesis_account,
        &time_source,
        state,
        &mut voting_block,
        false,
        skip_block_signatures,
    );
    let (valid_block, mut state_block) = validation
        .unpack(|_| {})
        .expect("block validates for replay fixture");
    let committed = valid_block.commit_unchecked().unpack(|_| {});
    let committed_signed = committed.as_ref().clone();
    assert_canonical_successful_fixture_results(&committed_signed);
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
fn seed_space_directory_manifest_for_retired_checkpoint_test(
    state: &State,
    dataspace: DataSpaceId,
) {
    let uaid = UniversalAccountId::from_hash(iroha_crypto::Hash::new(
        b"strict-replay-retired-checkpoint-surface",
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
    let genesis = attach_successful_fixture_results(
        SignedBlock::genesis(
            vec![tx],
            SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
            None,
            None,
        ),
        &SAMPLE_GENESIS_ACCOUNT_KEYPAIR,
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
        let block2: SignedBlock = attach_successful_fixture_results(block2.into(), &leader);
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
    let bad_block = attach_successful_fixture_results(
        SignedBlock::genesis(vec![tx], rogue_signer.private_key(), None, None),
        &rogue_signer,
    );
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
    use iroha_crypto::Algorithm;
    use iroha_data_model::peer::PeerId;
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use std::borrow::Cow;
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
    use iroha_crypto::Algorithm;
    use iroha_data_model::{
        events::time::{ExecutionTime, TimeEventFilter},
        parameter::system::{Parameter, SumeragiConsensusMode, SumeragiNposParameters},
        peer::PeerId,
        trigger::{
            Trigger,
            action::{Action, Repeats},
        },
    };
    use iroha_genesis::{GENESIS_DOMAIN_ID, GenesisBuilder, GenesisTopologyEntry};
    use iroha_test_samples::{
        SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR, gen_account_in,
    };
    let chain_id = ChainId::from("iroha:test:npos-replay");
    let peer_keypairs = (0..4)
        .map(|_| crate::state::checked_keypair_with_algorithm(Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    let peers = peer_keypairs
        .iter()
        .map(|keypair| PeerId::new(keypair.public_key().clone()))
        .collect::<Vec<_>>();
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
    let topology_entries = peer_keypairs
        .iter()
        .map(|keypair| {
            GenesisTopologyEntry::new(
                PeerId::new(keypair.public_key().clone()),
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("generate validator proof of possession"),
            )
        })
        .collect::<Vec<_>>();
    let (user_id, user_keypair) = gen_account_in("wonderland");
    let mut genesis_builder =
        GenesisBuilder::new_without_executor(chain_id.clone(), "ivm/libs/not/installed")
            .set_topology(topology_entries)
            .append_parameter(Parameter::Custom(npos_params.into_custom_parameter()));
    genesis_builder = genesis_builder
        .domain(DomainId::try_new("wonderland", "universal").expect("domain id"))
        .account(user_keypair.public_key().clone())
        .finish_domain();
    let heartbeat_trigger = Trigger::new(
        "npos_replay_heartbeat".parse().expect("trigger id"),
        Action::new(
            vec![InstructionBox::from(Log::new(
                iroha_data_model::Level::INFO,
                "advance the NPoS replay fixture clock".to_owned(),
            ))],
            Repeats::Exactly(1),
            user_id,
            TimeEventFilter::new(ExecutionTime::PreCommit),
        )
        .expect("heartbeat trigger action"),
    );
    genesis_builder = genesis_builder.append_instruction(Register::trigger(heartbeat_trigger));
    let genesis_block = genesis_builder
        .build_raw()
        .with_consensus_mode(SumeragiConsensusMode::Npos)
        .with_consensus_meta()
        .build_and_sign(&SAMPLE_GENESIS_ACCOUNT_KEYPAIR)
        .expect("genesis");
    let mut genesis_signed = genesis_block.0.clone();
    let kura = Kura::blank_kura_for_testing();
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let validator_accounts = peer_keypairs
        .iter()
        .map(|keypair| AccountId::new(keypair.public_key().clone()))
        .collect::<Vec<_>>();
    let make_world = || {
        let accounts = std::iter::once(new_genesis_account(&genesis_id).build(&genesis_id))
            .chain(
                validator_accounts
                    .iter()
                    .cloned()
                    .map(|validator| Account::new(validator).build(&genesis_id)),
            )
            .collect::<Vec<_>>();
        let world = World::with(
            [Domain::new(GENESIS_DOMAIN_ID.clone()).build(&genesis_id)],
            accounts,
            [],
        );
        {
            let mut block = world.block();
            for (validator, keypair) in validator_accounts.iter().zip(&peer_keypairs) {
                let peer_id = PeerId::new(keypair.public_key().clone());
                block.public_lane_validators.insert(
                    (LaneId::SINGLE, validator.clone()),
                    iroha_data_model::nexus::PublicLaneValidatorRecord {
                        lane_id: LaneId::SINGLE,
                        validator: validator.clone(),
                        peer_id,
                        stake_account: validator.clone(),
                        total_stake: iroha_primitives::numeric::Quantity::from(1_000_u64),
                        self_stake: iroha_primitives::numeric::Quantity::from(1_000_u64),
                        metadata: iroha_data_model::metadata::Metadata::default(),
                        status: iroha_data_model::nexus::PublicLaneValidatorStatus::Active,
                        activation_epoch: None,
                        activation_height: None,
                        last_reward_epoch: None,
                    },
                );
            }
            block.commit();
        }
        world
    };
    let materialize_state = State::new_with_chain(
        make_world(),
        Arc::clone(&kura),
        crate::query::store::LiveQueryStore::start_test(),
        chain_id.clone(),
    );
    configure_replay_fixture_parameters(&materialize_state);
    rebind_test_confidential_features_and_resign(
        &mut genesis_signed,
        &materialize_state,
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
    );
    let mut base_topology = crate::sumeragi::network_topology::Topology::new(peers.clone());
    base_topology.block_committed(peers.clone(), genesis_signed.hash());
    let leader_peer = base_topology
        .as_ref()
        .get(leader_index)
        .expect("leader index within topology");
    let signer = peer_keypairs
        .iter()
        .find(|keypair| keypair.public_key() == leader_peer.public_key())
        .expect("selected leader belongs to the exact validator committee")
        .private_key();
    let new_block = crate::block::BlockBuilder::new(Vec::new())
        .chain(0, Some(&genesis_signed))
        .sign(signer)
        .unpack(|_| {});
    let mut signed_block: SignedBlock = new_block.into();
    let mut validation_topology = crate::sumeragi::network_topology::Topology::new(peers.clone());
    validation_topology.rotate_preserve_view_to_front(leader_index);
    rebind_test_confidential_features_and_resign(&mut signed_block, &materialize_state, signer);
    let genesis_signed =
        commit_replay_validated_block(&materialize_state, &topology, genesis_signed, &genesis_id);
    let signed_block = commit_replay_validated_block(
        &materialize_state,
        &validation_topology,
        signed_block,
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
fn replay_rejects_non_authoritative_signature_topology_rotation() {
    run_replay_validation_test_on_stack(
        "replay_rejects_non_authoritative_signature_rotation",
        replay_rejects_non_authoritative_signature_topology_rotation_impl,
    );
}
#[allow(clippy::too_many_lines)]
fn replay_rejects_non_authoritative_signature_topology_rotation_impl() {
    use iroha_crypto::Algorithm;
    use iroha_data_model::{DomainId, account::AccountId, peer::PeerId};
    use iroha_genesis::GENESIS_DOMAIN_ID;
    use std::borrow::Cow;
    let chain_id = ChainId::from("iroha:test:replay-signature-rotation-recovery");
    let genesis_id = (*SAMPLE_GENESIS_ACCOUNT_ID).clone();
    let user_keypair = crate::state::checked_keypair_with_algorithm(Algorithm::Ed25519);
    let user_domain: DomainId = DomainId::try_new("users", "universal").expect("domain id");
    let user_id = AccountId::new(user_keypair.public_key().clone());
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
        replay_fixture_leader_seed(&state_view, height, ConsensusMode::Permissioned)
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
    use iroha_crypto::{Algorithm, Hash};
    use iroha_data_model::transaction::signed::TransactionResultInner;
    use std::borrow::Cow;
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
    use iroha_crypto::{Algorithm, Hash};
    use std::borrow::Cow;
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
fn replay_rejects_retired_space_directory_checkpoint_surface() {
    run_replay_validation_test_on_stack(
        "replay_rejects_retired_checkpoint_surface",
        replay_rejects_retired_space_directory_checkpoint_surface_impl,
    );
}
#[allow(clippy::too_many_lines)]
fn replay_rejects_retired_space_directory_checkpoint_surface_impl() {
    use iroha_crypto::Algorithm;
    use iroha_primitives::json::Json;
    use std::borrow::Cow;
    let chain_id = ChainId::from("iroha:test:retired-checkpoint-rejection");
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
    seed_space_directory_manifest_for_retired_checkpoint_test(&original_state, dataspace_id);
    let proof_policies = |height| {
        crate::da::active_proof_policy_bundle_at_height(&original_state.nexus_snapshot(), height)
    };
    let tx_genesis = TransactionBuilder::new_genesis(
        genesis_id.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(iroha_logger::Level::INFO, "genesis".to_owned())])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let genesis_block = SignedBlock::try_genesis_with_da_proof_policies(
        vec![tx_genesis],
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
        None,
        None,
        Some(proof_policies(1)),
    )
    .expect("genesis fixture should sign with explicit DA proof policies");
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
    let mut block2: SignedBlock = block.into();
    rebind_test_execution_context_validators_and_resign(
        &mut block2,
        &topology,
        leader.private_key(),
    );
    let block2 = commit_replay_validated_block(&original_state, &topology, block2, &genesis_id);
    assert!(block2.has_results());
    kura.store_block(Arc::new(block2.clone()))
        .expect("store current block");
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
        .chain(0, Some(&block2))
        .with_da_proof_policies(Some(proof_policies(3)))
        .sign(leader.private_key())
        .unpack(|_| {});
    let mut block3: SignedBlock = block3.into();
    rebind_test_execution_context_validators_and_resign(
        &mut block3,
        &topology,
        leader.private_key(),
    );
    let block3 = commit_replay_validated_block(&original_state, &topology, block3, &genesis_id);
    assert!(block3.has_results());
    kura.store_block(Arc::new(block3.clone()))
        .expect("store second current block");
    let canonical_checkpoint = crate::snapshot::canonical_state_snapshot_hash(&original_state);
    let retired_checkpoint =
        crate::snapshot::retired_state_snapshot_hash_without_space_directory_manifests(
            &original_state,
        );
    assert_ne!(
        canonical_checkpoint, retired_checkpoint,
        "test fixture must distinguish the exact first-release WSV from the retired surface"
    );
    kura.overwrite_wsv_checkpoint_without_validation_for_tests(3, retired_checkpoint, None)
        .expect("overwrite final WSV checkpoint with retired surface hash");
    let replay_kura = Kura::blank_kura_for_testing();
    let mut replay_state =
        replay_fixture_state(Arc::clone(&replay_kura), chain_id, lane_id, dataspace_id);
    seed_space_directory_manifest_for_retired_checkpoint_test(&replay_state, dataspace_id);
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
