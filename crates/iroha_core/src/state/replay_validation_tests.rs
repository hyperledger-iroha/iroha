use super::*;
use iroha_data_model::{
    ChainId, ValidationFail,
    account::AccountId,
    block::{SignedBlock, consensus_v2::ConsensusMode},
    isi::Log,
    nexus::{AssetPermissionManifest, DataSpaceId, LaneId, ManifestVersion, UniversalAccountId},
    prelude::{Account, Domain},
    transaction::{TransactionBuilder, error::TransactionRejectionReason},
};
use iroha_test_samples::{SAMPLE_GENESIS_ACCOUNT_ID, SAMPLE_GENESIS_ACCOUNT_KEYPAIR};
use std::sync::Arc;
fn run_replay_validation_test_on_stack(name: &'static str, test: fn()) {
    // The full replay pipeline has deep debug-mode stack use; do not depend on libtest's
    // platform-default worker stack for these integration-heavy scenarios.
    let handle = crate::sumeragi::sumeragi_thread_builder(name)
        .spawn(test)
        .expect("spawn replay validation test");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}
fn new_genesis_account(
    account_id: &iroha_data_model::account::AccountId,
) -> iroha_data_model::account::NewAccount {
    Account::new(account_id.clone())
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
pub(super) fn seed_space_directory_manifest_for_retired_checkpoint_test(
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
    crate::sumeragi::sumeragi_thread_builder("missing-checkpoint-production-fixture")
        .spawn(move || {
            let mut fixture = super::strict_replay_tests::StrictReplayFixture::new();
            let block_count = if checkpoint_exists_only_at_later_height {
                fixture.append_metadata_block();
                assert!(
                    fixture
                        .kura
                        .wsv_checkpoint(2)
                        .expect("later checkpoint")
                        .is_some()
                );
                2
            } else {
                1
            };
            fixture
                .kura
                .remove_wsv_checkpoint_without_binding_for_tests(1)
                .expect("remove only the first exact checkpoint");
            let mut state = fixture.replay_state(Arc::clone(&fixture.kura));
            let before = crate::snapshot::canonical_state_snapshot_bytes_for_tests(&state);
            let err = super::replay_blocks_from_kura(&fixture.kura, &mut state, block_count)
                .expect_err("full-body replay requires an exact WSV checkpoint at every height");
            assert_eq!(
                crate::snapshot::canonical_state_snapshot_bytes_for_tests(&state),
                before
            );
            (err, state.committed_height())
        })
        .expect("spawn checkpoint fixture")
        .join()
        .expect("checkpoint fixture completed")
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
    run_replay_validation_test_on_stack("replay-corrupt-genesis-signature", || {
        let fixture = super::strict_replay_tests::StrictReplayFixture::new();
        let rogue =
            iroha_crypto::KeyPair::try_from_seed(vec![0xA7; 32], iroha_crypto::Algorithm::Ed25519)
                .expect("distinct deterministic signer");
        let kura = fixture.fork_with_signature(0, rogue.private_key());
        let mut state = fixture.replay_state(Arc::clone(&kura));
        let before = crate::snapshot::canonical_state_snapshot_bytes_for_tests(&state);
        let err = super::replay_blocks_from_kura(&kura, &mut state, 1)
            .expect_err("replay must never bypass a corrupt genesis authority signature");
        assert!(
            format!("{err:#}").contains("failed to verify replayed genesis block #1 signatures"),
            "unexpected replay rejection: {err:?}"
        );
        assert_eq!(
            state.committed_height(),
            0,
            "invalid genesis must not mutate WSV"
        );
        assert_eq!(
            crate::snapshot::canonical_state_snapshot_bytes_for_tests(&state),
            before
        );
    });
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
    let mut restored_state = make_state(Arc::clone(&kura));
    restored_state.push_block_hash_for_testing(snapshot_hash);
    assert_eq!(
        super::hash_only_replay_snapshot_hash(&kura, &restored_state, height)
            .expect("matching audited snapshot hash is covered"),
        Some(snapshot_hash)
    );
    let unhydrated_state = make_state(Arc::clone(&kura));
    let missing_snapshot = super::hash_only_replay_snapshot_hash(&kura, &unhydrated_state, height)
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
    let mismatch = super::hash_only_replay_snapshot_hash(&kura, &mismatched_state, height)
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
    let mut fixture = super::strict_replay_tests::StrictReplayFixture::new();
    let second = fixture.append_metadata_block();
    let third = fixture.append_metadata_block();
    let kura = &fixture.kura;
    let manifest = kura
        .commit_manifest(3)
        .expect("read third manifest")
        .expect("third manifest exists");
    let mut state = fixture.replay_state(Arc::clone(kura));
    super::replay_blocks_from_kura(kura, &mut state, 2).expect("replay first two exact tuples");
    assert_eq!(state.committed_height(), 2);
    assert_eq!(state.latest_block_hash_fast(), Some(second.block.hash()));
    let prefix = crate::snapshot::canonical_state_snapshot_bytes_for_tests(&state);
    kura.remove_wsv_checkpoint_without_binding_for_tests(3)
        .expect("remove only the final checkpoint after the earlier prefix is authenticated");
    let missing_checkpoint = super::replay_blocks_from_kura_range(kura, &mut state, 3, 3)
        .expect_err("range replay must reject a missing full-body checkpoint");
    assert!(
        missing_checkpoint
            .to_string()
            .contains("missing WSV checkpoint for full block #3"),
        "{missing_checkpoint:?}"
    );
    assert_eq!(state.committed_height(), 2);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_bytes_for_tests(&state),
        prefix
    );
    kura.store_wsv_checkpoint(3, third.block.hash(), third.checkpoint_hash)
        .expect("restore the original immutable checkpoint");
    kura.store_commit_manifest(manifest)
        .expect("rebind the restored checkpoint to its exact existing manifest");
    super::replay_blocks_from_kura_range(kura, &mut state, 3, 3)
        .expect("replay the remaining exact block");
    assert_eq!(state.committed_height(), 3);
    assert_eq!(state.latest_block_hash_fast(), Some(third.block.hash()));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state),
        third.checkpoint_hash
    );
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
    let mut fixture = super::strict_replay_tests::StrictReplayFixture::new_npos();
    let context = fixture.successor_context();
    assert_eq!(context.mode, ConsensusMode::Npos);
    assert_eq!(
        context.parent_commit_qc.as_ref(),
        Some(&fixture.artifact.commit_qc)
    );
    let view = (0_u64..u64::try_from(context.roster.len()).expect("roster length"))
        .find(|view| context.leader(*view) != 0)
        .expect("the exact frozen schedule must exercise a non-zero leader");
    let leader = context.leader(view);
    assert_ne!(
        leader, 0,
        "replay must preserve the frozen roster's non-zero leader index"
    );
    // Every eligibility record and escrow position below came from signed genesis execution.
    {
        let world = fixture.materialized_state.world_view();
        assert_eq!(world.public_lane_validators().len(), 4);
        assert_eq!(world.public_lane_stake_shares().len(), 4);
        for entry in &context.roster {
            let account = AccountId::new(entry.validator.public_key().clone());
            let record = world
                .public_lane_validators()
                .get(&(LaneId::SINGLE, account.clone()))
                .expect("signed genesis validator");
            assert_eq!(record.peer_id, entry.validator);
            assert_eq!(
                record.status,
                iroha_data_model::nexus::PublicLaneValidatorStatus::Active
            );
            assert_eq!(record.activation_height, 1);
            assert_eq!(
                record.total_stake,
                iroha_primitives::numeric::Quantity::from(1_000_u64)
            );
            assert_eq!(
                world
                    .public_lane_stake_shares()
                    .get(&(LaneId::SINGLE, account.clone(), account))
                    .expect("actual bonded custody")
                    .bonded,
                record.total_stake
            );
        }
    }
    let second = fixture.append_metadata_block_at_view(view);
    let signatures = second.block.signatures().collect::<Vec<_>>();
    assert_eq!(signatures.len(), 1);
    assert_eq!(signatures[0].index(), u64::from(leader));
    signatures[0]
        .signature()
        .verify_hash(
            context.roster[usize::try_from(leader).expect("leader index")]
                .validator
                .public_key(),
            second.block.hash(),
        )
        .expect("the exact non-zero roster index authenticates the body");
    let mut state = fixture.replay_state(Arc::clone(&fixture.kura));
    super::replay_blocks_from_kura(&fixture.kura, &mut state, 2)
        .expect("production replay must consume the signed NPoS authority and frozen leader");
    assert_eq!(state.committed_height(), 2);
    assert_eq!(state.latest_block_hash_fast(), Some(second.block.hash()));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&state),
        second.checkpoint_hash
    );
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
    let fixture = super::strict_replay_tests::StrictReplayFixture::new().into_two_block();
    let mut block = fixture.second_block.clone();
    let leader = fixture
        .second_context
        .leader(block.header().view_change_index());
    let wrong = (usize::try_from(leader).expect("leader index") + 1) % fixture.first.keys.len();
    let signature = iroha_data_model::block::BlockSignature::new(
        u64::from(leader),
        iroha_crypto::SignatureOf::try_from_hash(
            fixture.first.keys[wrong].private_key(),
            block.hash(),
        )
        .expect("sign as a different member while retaining the authoritative index"),
    );
    signature
        .signature()
        .verify_hash(fixture.first.keys[wrong].public_key(), block.hash())
        .expect("the signature is cryptographically valid under the wrong rotation");
    assert!(
        signature
            .signature()
            .verify_hash(
                fixture.second_context.roster[usize::try_from(leader).expect("leader index")]
                    .validator
                    .public_key(),
                block.hash(),
            )
            .is_err(),
        "the original index must reject a signature from another known validator"
    );
    block
        .replace_signatures(std::collections::BTreeSet::from([signature]))
        .expect("install the exact wrong-index signature fixture");
    let mut artifact = fixture.second_artifact.clone();
    artifact.subject.payload_hash = block
        .canonical_proposal_wire_hash()
        .expect("encode wrong-index proposal");
    artifact.commit_qc.subject = artifact.subject;
    artifact
        .commit_qc
        .execution_commitment
        .executed_block_wire_len = u64::try_from(
        block
            .encode_wire()
            .expect("encode wrong-index executed block")
            .len(),
    )
    .expect("wire length fits u64");
    artifact
        .commit_qc
        .execution_commitment
        .executed_block_wire_hash = block
        .executed_block_wire_hash()
        .expect("hash wrong-index executed block");
    super::strict_replay_tests::StrictReplayFixture::resign_certificate(
        &mut artifact.commit_qc,
        &fixture.first.keys,
    );
    let kura = fixture.first.exact_kura_copy();
    let mut state = fixture.first.replay_state(Arc::clone(&kura));
    kura.store_block(Arc::new(block.clone()))
        .expect("retain corrupted height-two body");
    kura.store_wsv_checkpoint(2, block.hash(), fixture.second_checkpoint_hash)
        .expect("retain correlated height-two checkpoint");
    kura.store_commit_manifest(
        crate::kura::CommitManifest::new(
            2,
            block.hash(),
            None,
            None,
            fixture.second_checkpoint_hash,
            None,
        )
        .with_authenticated_v2_commit_authority(&artifact),
    )
    .expect("retain correlated complete manifest");
    let _ = kura
        .store_v2_finality_artifact(&artifact)
        .expect("retain the exact signed CommitQC tuple");
    super::replay_blocks_from_kura(&kura, &mut state, 1)
        .expect("authenticate the exact preceding genesis");
    let prefix = crate::snapshot::canonical_state_snapshot_bytes_for_tests(&state);
    let err = super::replay_blocks_from_kura_range(&kura, &mut state, 2, 2)
        .expect_err("replay must never retry a failed signature under another roster rotation");
    assert!(
        format!("{err:#}").contains("failed to verify replayed block #2 signatures"),
        "unexpected replay rejection: {err:?}"
    );
    assert_eq!(
        state.committed_height(),
        1,
        "wrong-index body must not mutate WSV"
    );
    assert_eq!(
        state.latest_block_hash_fast(),
        Some(fixture.first.block.hash())
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_bytes_for_tests(&state),
        prefix
    );
}
#[test]
fn replay_rejects_committed_execution_result_mismatch_without_mutating_that_block() {
    run_replay_validation_test_on_stack(
        "replay_rejects_result_mismatch",
        replay_rejects_committed_execution_result_mismatch_impl,
    );
}
/// Replay the execution boundary using authority produced by the strict fixture's real Apply.
/// The caller deliberately supplies a result-corrupted body: durable body/finality authentication
/// is tested separately by production replay, so this seam must reach result parity itself.
fn replay_exact_execution_fixture_block(
    fixture: &super::strict_replay_tests::TwoBlockReplayFixture,
    state: &State,
    signed: SignedBlock,
) -> Result<()> {
    let context = &fixture.second_context;
    let height = context.height;
    assert_eq!(state.committed_height(), 1);
    assert_eq!(
        state.latest_block_hash_fast(),
        Some(fixture.first.block.hash())
    );
    assert_eq!(
        context.parent_commit_qc.as_ref(),
        Some(&fixture.first.artifact.commit_qc)
    );
    context
        .validate()
        .expect("exact applied fixture height context");
    ValidBlock::validate_signatures_subset_v2_artifact_exact(&signed, &fixture.second_artifact)
        .map_err(|error| eyre!(error))
        .wrap_err("failed to authenticate execution-fixture signature indices")?;
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let topology = crate::sumeragi::network_topology::Topology::new(roster.clone());
    let mut voting_block = None;
    let (valid, _state_block) = ValidBlock::validate_sumeragi_v2_candidate_keep_voting_block(
        signed.canonical_resultless_proposal(),
        &topology,
        &fixture.first.genesis_account,
        &TimeSource::new_system(),
        Duration::from_secs(1),
        crate::block::valid::SumeragiV2ValidationContext::from_height_context(context),
        state,
        &mut voting_block,
    )
    .unpack(|_| {})
    .map_err(|(_block, error)| eyre!(error))
    .wrap_err_with(|| format!("failed to validate block #{height} during replay"))?;
    let committed = valid.commit_unchecked().unpack(|_| {});
    ensure_replayed_results_match_committed(height, &signed, committed.as_ref()).wrap_err_with(
        || format!("failed to verify replayed block #{height} against committed execution results"),
    )?;
    Ok(())
}
fn replay_rejects_committed_execution_result_mismatch_impl() {
    let fixture = super::strict_replay_tests::StrictReplayFixture::new().into_two_block();
    let mut replay_state = fixture.first.replay_state(Arc::clone(&fixture.first.kura));
    super::replay_blocks_from_kura(&fixture.first.kura, &mut replay_state, 1)
        .expect("authenticate and replay the exact canonical genesis");
    let before_bytes = crate::snapshot::canonical_state_snapshot_bytes(&replay_state);
    replay_exact_execution_fixture_block(&fixture, &replay_state, fixture.second_block.clone())
        .expect("the pristine execution seam must reproduce the exact finalized results");
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_bytes(&replay_state),
        before_bytes,
        "the validation-only execution seam must discard its speculative state",
    );
    let mut signed_block2 = fixture.second_block.clone();
    let entry_hashes = signed_block2
        .external_entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect::<Vec<_>>();
    assert_eq!(entry_hashes.len(), 1);
    assert!(
        signed_block2
            .results()
            .all(|result| result.as_ref().is_ok())
    );
    signed_block2
        .set_transaction_results(
            Vec::new(),
            &entry_hashes,
            vec![Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("forced mismatch".to_owned()),
            ))],
        )
        .expect("change only the exact committed external result");
    let leader = fixture
        .second_context
        .leader(signed_block2.header().view_change_index());
    signed_block2
        .replace_signatures(std::collections::BTreeSet::from([
            iroha_data_model::block::BlockSignature::new(
                u64::from(leader),
                iroha_crypto::SignatureOf::try_from_hash(
                    fixture.first.keys[usize::try_from(leader).expect("leader index")]
                        .private_key(),
                    signed_block2.hash(),
                )
                .expect("sign the deliberately result-corrupted fixture"),
            ),
        ]))
        .expect("preserve the exact authenticated signature index");
    let err = replay_exact_execution_fixture_block(&fixture, &replay_state, signed_block2)
        .expect_err("replay must reject committed execution results that it cannot reproduce");
    assert!(
        format!("{err:#}")
            .contains("failed to verify replayed block #2 against committed execution results"),
        "unexpected replay rejection: {err:?}"
    );
    assert_eq!(
        replay_state.committed_height(),
        1,
        "the result-mismatched block must be discarded atomically"
    );
    assert_eq!(
        replay_state.latest_block_hash_fast(),
        Some(fixture.first.block.hash())
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_bytes(&replay_state),
        before_bytes,
        "result rejection must preserve every canonical state byte"
    );
    // The pristine tuple succeeds from the same pre-state: no earlier validation failure
    // may accidentally satisfy this negative test.
    super::replay_blocks_from_kura_range(&fixture.first.kura, &mut replay_state, 2, 2)
        .expect("the unchanged original tuple must pass production replay after rejection");
    assert_eq!(
        replay_state.latest_block_hash_fast(),
        Some(fixture.second_block.hash())
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
    let fixture = super::strict_replay_tests::StrictReplayFixture::new().into_two_block();
    let kura = &fixture.first.kura;
    let original_manifest = kura
        .commit_manifest(2)
        .expect("read exact manifest")
        .expect("height-two manifest exists");
    let mut replay_state = fixture.first.replay_state(Arc::clone(kura));
    super::replay_blocks_from_kura(kura, &mut replay_state, 1)
        .expect("genesis replay establishes the exact authenticated pre-block state");
    let before_bytes = crate::snapshot::canonical_state_snapshot_bytes(&replay_state);
    let before_height = replay_state.committed_height();
    let before_hash = replay_state.latest_block_hash_fast();
    let before_merge = replay_state.merge_ledger.snapshot();
    let forged_checkpoint = Hash::new(b"not the replayed canonical WSV");
    assert_ne!(forged_checkpoint, fixture.second_checkpoint_hash);
    let forged_manifest = crate::kura::CommitManifest::new(
        2,
        fixture.second_block.hash(),
        None,
        None,
        forged_checkpoint,
        None,
    )
    .with_authenticated_v2_commit_authority(&fixture.second_artifact);
    kura.overwrite_commit_manifest_without_binding_for_tests(&forged_manifest)
        .expect("corrupt only the checkpoint portion of the retained manifest");
    kura.overwrite_wsv_checkpoint_without_validation_for_tests(
        2,
        forged_checkpoint,
        Some(&forged_manifest),
    )
    .expect("preserve tuple binding so replay reaches the actual WSV mismatch");
    let err = super::replay_blocks_from_kura_range(kura, &mut replay_state, 2, 2)
        .expect_err("replay must reject a WSV checkpoint with a forged state hash");
    assert!(
        format!("{err:#}").contains("replayed block #2 WSV checkpoint mismatch"),
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
    kura.overwrite_commit_manifest_without_binding_for_tests(&original_manifest)
        .expect("restore the exact original retained manifest");
    kura.overwrite_wsv_checkpoint_without_validation_for_tests(
        2,
        fixture.second_checkpoint_hash,
        Some(&original_manifest),
    )
    .expect("restore the exact original checkpoint tuple");
    super::replay_blocks_from_kura_range(kura, &mut replay_state, 2, 2)
        .expect("corrected checkpoint must replay successfully after atomic rejection");
    assert_eq!(replay_state.committed_height(), 2);
    assert_eq!(
        replay_state.latest_block_hash_fast(),
        Some(fixture.second_block.hash())
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
    let mut fixture = super::strict_replay_tests::StrictReplayFixture::new_with_space_directory();
    let second = fixture.append_metadata_block();
    let third = fixture.append_metadata_block();
    let kura = &fixture.kura;
    let mut replay_state = fixture.replay_state(Arc::clone(kura));
    super::replay_blocks_from_kura(kura, &mut replay_state, 2)
        .expect("authenticate the exact first-release prefix");
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&replay_state),
        second.checkpoint_hash
    );
    let canonical_prefix = crate::snapshot::canonical_state_snapshot_bytes_for_tests(&replay_state);
    let mut retired_checkpoint_value: norito::json::Value =
        norito::json::from_slice(&crate::snapshot::canonical_state_snapshot_bytes_for_tests(
            fixture.materialized_state.as_ref(),
        ))
        .expect("decode exact first-release WSV fixture");
    assert!(
        !fixture
            .materialized_state
            .world_view()
            .space_directory_manifests()
            .is_empty(),
        "the fixture must retain a real non-empty Space Directory surface"
    );
    retired_checkpoint_value
        .as_object_mut()
        .expect("snapshot object")
        .remove("space_directory_manifests")
        .expect("first-release snapshot must carry Space Directory manifests");
    let retired_checkpoint = Hash::new(
        norito::json::to_json(&retired_checkpoint_value)
            .expect("encode the deliberately retired checkpoint surface"),
    );
    assert_ne!(
        third.checkpoint_hash, retired_checkpoint,
        "the exact first-release WSV must differ from the retired surface"
    );
    let original_manifest = kura
        .commit_manifest(3)
        .expect("read final manifest")
        .expect("final manifest exists");
    let forged_manifest = crate::kura::CommitManifest::new(
        3,
        third.block.hash(),
        None,
        None,
        retired_checkpoint,
        None,
    )
    .with_authenticated_v2_commit_authority(&third.artifact);
    kura.overwrite_commit_manifest_without_binding_for_tests(&forged_manifest)
        .expect("corrupt the correlated retained checkpoint manifest");
    kura.overwrite_wsv_checkpoint_without_validation_for_tests(
        3,
        retired_checkpoint,
        Some(&forged_manifest),
    )
    .expect("retain exact tuple binding so execution reaches the retired-surface mismatch");
    let err = super::replay_blocks_from_kura_range(kura, &mut replay_state, 3, 3)
        .expect_err("the retired checkpoint surface must never authorize replayed state");
    assert!(
        format!("{err:#}").contains("replayed block #3 WSV checkpoint mismatch"),
        "unexpected replay rejection: {err:?}"
    );
    assert_eq!(replay_state.committed_height(), 2);
    assert_eq!(
        replay_state.latest_block_hash_fast(),
        Some(second.block.hash())
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_bytes_for_tests(&replay_state),
        canonical_prefix,
        "rejection must leave the last exactly authenticated prefix committed"
    );
    kura.overwrite_commit_manifest_without_binding_for_tests(&original_manifest)
        .expect("restore the original exact manifest");
    kura.overwrite_wsv_checkpoint_without_validation_for_tests(
        3,
        third.checkpoint_hash,
        Some(&original_manifest),
    )
    .expect("restore the exact first-release checkpoint surface");
    super::replay_blocks_from_kura_range(kura, &mut replay_state, 3, 3)
        .expect("the exact surface must succeed from the unchanged prefix after rejection");
    assert_eq!(
        replay_state.latest_block_hash_fast(),
        Some(third.block.hash())
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(&replay_state),
        third.checkpoint_hash
    );
}

fn replay_result_boundary_proposal(entry_count: usize) -> SignedBlock {
    let transactions = (0..entry_count)
        .map(|_| {
            let transaction = TransactionBuilder::new_genesis(
                SAMPLE_GENESIS_ACCOUNT_ID.clone(),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_instructions([Log::new(
                iroha_logger::Level::INFO,
                "replay result boundary".to_owned(),
            )])
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
            crate::prelude::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(transaction))
        })
        .collect();
    crate::block::BlockBuilder::new(transactions)
        .chain(0, None)
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into()
}
#[test]
fn replay_result_comparison_requires_attached_results_even_for_empty_blocks() {
    for entry_count in [0, 1] {
        let proposal = replay_result_boundary_proposal(entry_count);
        assert!(!proposal.has_results());
        let executed =
            attach_successful_fixture_results(proposal.clone(), &SAMPLE_GENESIS_ACCOUNT_KEYPAIR);
        ensure_replayed_results_match_committed(1, &executed, &executed)
            .expect("identical authenticated execution results must match");
        for replayed in [&proposal, &executed] {
            let error = ensure_replayed_results_match_committed(1, &proposal, replayed)
                .expect_err("a resultless committed body cannot establish replay parity");
            assert!(
                error
                    .to_string()
                    .contains("does not contain stored execution results")
            );
        }
        let error = ensure_replayed_results_match_committed(1, &executed, &proposal)
            .expect_err("a resultless replay cannot establish execution parity");
        assert!(
            error
                .to_string()
                .contains("did not produce execution results")
        );
    }
}
#[test]
fn replay_validation_diagnostics_handle_resultless_and_executed_failures() {
    for entry_count in [0, 1] {
        let proposal = replay_result_boundary_proposal(entry_count);
        assert!(!proposal.has_results());
        assert!(replay_validation_transaction_errors(&proposal).is_empty());
        let executed = attach_successful_fixture_results(proposal, &SAMPLE_GENESIS_ACCOUNT_KEYPAIR);
        assert!(replay_validation_transaction_errors(&executed).is_empty());
    }
    let mut failed = replay_result_boundary_proposal(1);
    let entry_hashes = failed.entrypoint_hashes().collect::<Vec<_>>();
    failed
        .set_transaction_results(
            Vec::new(),
            &entry_hashes,
            vec![Err(TransactionRejectionReason::Validation(
                ValidationFail::NotPermitted("result-boundary rejection".to_owned()),
            ))],
        )
        .expect("attach the actual rejected execution result");
    let errors = replay_validation_transaction_errors(&failed);
    assert_eq!(errors.len(), 1);
    assert!(errors[0].starts_with("tx#0:"));
    assert!(errors[0].contains("result-boundary rejection"));
}
