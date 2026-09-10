//! Final public transcript contents remain bound through witness capture, extraction and commit.

use super::{
    tests::{apply_source, cache_canonical_test_transaction_set, delta, header, state},
    *,
};
use crate::{
    state::{State, TransactionsBlockError},
    sumeragi::witness,
};
use iroha_data_model::{
    asset::AssetId,
    fastpq::{TransferDeltaTranscript, TransferTranscript},
};
use iroha_model_base::state_path::StatePath;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

type TranscriptMap = BTreeMap<Hash, Vec<TransferTranscript>>;

fn finalized_source(block: &mut StateBlock<'_>, source: Hash) -> TranscriptMap {
    apply_source(block, source, false, None);
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    let archive = block.drain_transfer_transcripts_with_pending(None);
    assert!(block.fastpq_transcripts.is_empty());
    assert_eq!(archive.len(), 1);
    assert!(archive[&source][0].poseidon_preimage_digest.is_some());
    archive
}

fn cached_outputs(block: &StateBlock<'_>) -> [bool; 3] {
    [
        block.exec_witness.is_some(),
        block.fastpq_witness_context.is_some(),
        block.parliament_timed_ovn_casting_bindings.is_some(),
    ]
}

fn assert_raw_content_failure(block: &StateBlock<'_>) -> String {
    // Inspect the raw fields before any guarded getter could conceal stale outputs.
    assert_eq!(cached_outputs(block), [false; 3]);
    block
        .fastpq_source_inventory
        .as_ref()
        .expect("content failure must retain an inventory result")
        .as_ref()
        .expect_err("content failure must be latched")
        .clone()
}

fn take_output(block: &mut StateBlock<'_>, output: usize) -> bool {
    match output {
        0 => block.take_exec_witness().is_some(),
        1 => block.take_fastpq_witness_context().is_some(),
        2 => block.take_parliament_timed_ovn_casting_bindings().is_some(),
        _ => unreachable!("there are exactly three captured outputs"),
    }
}

fn assert_getters_refuse(block: &mut StateBlock<'_>, first_error: &str) {
    for output in 0..3 {
        assert!(!take_output(block, output));
        assert_eq!(assert_raw_content_failure(block), first_error);
    }
}

fn assert_recorder_discarded() {
    let remaining = witness::drain_exec_witness();
    assert!(remaining.reads.is_empty());
    assert!(remaining.writes.is_empty());
    assert!(remaining.fastpq_transcripts.is_empty());
    assert!(remaining.fastpq_batches.is_empty());
}

fn change_public_content(transcript: &mut TransferTranscript, missing_digest: bool) {
    if missing_digest {
        assert!(transcript.poseidon_preimage_digest.take().is_some());
    } else {
        transcript.authority_digest = Hash::new(b"substituted final witness authority");
    }
}

fn change_private_paths(delta: &mut TransferDeltaTranscript, seed: u8) {
    for (index, path) in [&mut delta.from_smt_witness, &mut delta.to_smt_witness]
        .into_iter()
        .enumerate()
    {
        let byte = seed + u8::try_from(index).unwrap();
        path.root_before = [byte; 32];
        path.root_after = [byte + 1; 32];
        path.path_bits = vec![byte];
        path.siblings = vec![[byte + 2; 32]];
    }
}

fn marker() -> StatePath {
    "fastpq/content-capture-publication-marker".parse().unwrap()
}

fn stage_marker_and_membership(block: &mut StateBlock<'_>) {
    {
        let mut tx = block.transaction();
        tx.world.smart_contract_state.insert(marker(), vec![1]);
        tx.apply();
    }
    block
        .stage_canonical_carrier_membership(core::iter::empty(), nonzero!(1_usize))
        .unwrap();
    block.block_hashes.push(block._curr_block.hash());
}

fn assert_not_published(state: &State) {
    assert!(
        state
            .world
            .smart_contract_state
            .view()
            .get(&marker())
            .is_none()
    );
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.transactions.latest_height(), 0);
    assert!(state.latest_block_hash_fast().is_none());
}

#[test]
fn raw_recorder_public_change_or_missing_digest_rejects_before_synthetic_capture() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    for missing_digest in [false, true] {
        witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        let source = Hash::new(b"raw recorder public content rejection");
        let original = finalized_source(&mut block, source);
        let mut substituted = original.clone();
        change_public_content(
            &mut substituted.get_mut(&source).unwrap()[0],
            missing_digest,
        );
        witness::synchronize_fastpq_transcripts(&substituted);
        assert_eq!(cached_outputs(&block), [false; 3]);

        let error = block.capture_exec_witness().unwrap_err();
        assert!(error.contains("public content differs"), "{error}");
        assert_eq!(assert_raw_content_failure(&block), error);
        assert_getters_refuse(&mut block, &error);
        assert_recorder_discarded();
        assert_eq!(original[&source][0].deltas, vec![delta()]);
        assert!(original[&source][0].poseidon_preimage_digest.is_some());
    }
}

#[test]
fn inactive_first_capture_rejects_even_empty_inventory_after_ordinary_witness_was_drained() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    witness::start_block();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    assert!(
        block
            .drain_transfer_transcripts_with_pending(None)
            .is_empty()
    );
    let transfer = delta();
    let asset = AssetId::of(transfer.asset_definition, transfer.from_account);
    witness::record_read_asset(&asset, Some(&transfer.from_balance_before));
    witness::record_write_asset(&asset, &transfer.from_balance_after);
    let omitted = witness::drain_exec_witness();
    assert_eq!(omitted.reads.len(), 1);
    assert_eq!(omitted.writes.len(), 1);
    assert!(omitted.fastpq_transcripts.is_empty());

    let error = block.capture_exec_witness().unwrap_err();
    assert_eq!(
        error,
        "ordinary witness capture has no active global recorder"
    );
    assert_eq!(assert_raw_content_failure(&block), error);
    assert_getters_refuse(&mut block, &error);
    assert_recorder_discarded();
}

#[test]
fn content_failure_survives_resynchronization_retry_getters_and_commit() {
    let _guard = witness::exec_witness_guard();
    for replay_at_commit in [false, true] {
        let state = state();
        witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        stage_marker_and_membership(&mut block);
        let source = Hash::new(b"sticky recorder content rejection");
        let original = finalized_source(&mut block, source);
        let mut substituted = original.clone();
        change_public_content(&mut substituted.get_mut(&source).unwrap()[0], false);
        witness::synchronize_fastpq_transcripts(&substituted);
        let first_error = block.capture_exec_witness().unwrap_err();
        assert_eq!(assert_raw_content_failure(&block), first_error);

        // A fresh active recorder with the exact old archive is not permission to repair
        // the block's latched failure or to fabricate a new ordinary witness.
        witness::start_block();
        witness::synchronize_fastpq_transcripts(&original);
        assert_eq!(block.capture_exec_witness(), Err(first_error.clone()));
        assert_eq!(assert_raw_content_failure(&block), first_error);
        assert_getters_refuse(&mut block, &first_error);
        assert_eq!(block.fastpq_source_inventory(), Err(first_error.as_str()));
        block.authenticated_replay_commit = replay_at_commit;
        assert_eq!(
            block.commit(),
            Err(TransactionsBlockError::FastpqSourceInventory)
        );
        assert_not_published(&state);
    }
}

#[test]
fn cached_public_mutation_rejects_repeat_capture_and_each_getter_as_first_operation() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    for missing_digest in [false, true] {
        for first_operation in 0..4 {
            witness::start_block();
            let mut block = state.block(header());
            cache_canonical_test_transaction_set(&mut block, &[]);
            finalized_source(&mut block, Hash::new(b"cached public content mutation"));
            block.capture_exec_witness().unwrap();
            change_public_content(
                &mut block.exec_witness.as_mut().unwrap().fastpq_transcripts[0].transcripts[0],
                missing_digest,
            );
            assert_eq!(cached_outputs(&block), [true; 3]);
            if first_operation == 0 {
                assert!(block.capture_exec_witness().is_err());
            } else {
                assert!(!take_output(&mut block, first_operation - 1));
            }
            let error = assert_raw_content_failure(&block);
            assert!(error.contains("public content differs"), "{error}");
            assert_getters_refuse(&mut block, &error);
            assert_eq!(block.capture_exec_witness(), Err(error.clone()));
            assert_eq!(assert_raw_content_failure(&block), error);
        }
    }
}

#[test]
fn cached_capture_rejects_restarted_active_recorder_even_with_empty_fastpq() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    for with_transfer in [false, true] {
        witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        if with_transfer {
            finalized_source(&mut block, Hash::new(b"cached recorder restart"));
        } else {
            block
                .finalize_fastpq_source_inventory(&[], &[], &[])
                .unwrap();
            assert!(
                block
                    .drain_transfer_transcripts_with_pending(None)
                    .is_empty()
            );
        }
        block.capture_exec_witness().unwrap();
        assert_eq!(cached_outputs(&block), [true, with_transfer, true]);
        witness::start_block();

        let error = block.capture_exec_witness().unwrap_err();
        assert_eq!(
            error,
            "cached witness capture has an unexpected active global recorder"
        );
        assert_eq!(assert_raw_content_failure(&block), error);
        assert_recorder_discarded();
        assert_eq!(block.capture_exec_witness(), Err(error.clone()));
        assert_getters_refuse(&mut block, &error);
    }
}

#[test]
fn pending_overlay_prevents_first_or_cached_capture_and_leaves_failure_sticky() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    for already_captured in [false, true] {
        witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        let original = finalized_source(&mut block, Hash::new(b"pending witness overlay"));
        if already_captured {
            block.capture_exec_witness().unwrap();
            assert_eq!(cached_outputs(&block), [true; 3]);
        }
        let overlay = witness::begin_exec_witness_overlay();
        let error = block.capture_exec_witness().unwrap_err();
        assert!(error.contains("pending current-thread overlay"), "{error}");
        assert_eq!(assert_raw_content_failure(&block), error);
        drop(overlay);
        witness::start_block();
        witness::synchronize_fastpq_transcripts(&original);
        assert_eq!(block.capture_exec_witness(), Err(error.clone()));
        assert_getters_refuse(&mut block, &error);
    }
}

#[test]
fn private_path_only_changes_survive_first_repeat_and_ordered_capture_extraction() {
    let _guard = witness::exec_witness_guard();
    let state = state();
    for order in [
        [0, 1, 2],
        [0, 2, 1],
        [1, 0, 2],
        [1, 2, 0],
        [2, 0, 1],
        [2, 1, 0],
    ] {
        witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        let source = Hash::new(b"private paths do not change public source seal");
        let mut substituted = finalized_source(&mut block, source);
        let owned = block
            .verified_fastpq_source_inventory_for_capture()
            .unwrap();
        change_private_paths(&mut substituted.get_mut(&source).unwrap()[0].deltas[0], 10);
        witness::synchronize_fastpq_transcripts(&substituted);
        block.capture_exec_witness().unwrap();
        assert_eq!(cached_outputs(&block), [true; 3]);
        assert_eq!(
            block.exec_witness.as_ref().unwrap().fastpq_transcripts[0].transcripts,
            substituted[&source],
        );
        assert!(
            block
                .exec_witness
                .as_ref()
                .unwrap()
                .fastpq_batches
                .is_empty()
        );
        change_private_paths(
            &mut block.exec_witness.as_mut().unwrap().fastpq_transcripts[0].transcripts[0].deltas
                [0],
            20,
        );
        block.capture_exec_witness().unwrap();
        assert_eq!(cached_outputs(&block), [true; 3]);
        assert!(Arc::ptr_eq(
            block
                .fastpq_witness_context
                .as_ref()
                .unwrap()
                ._source_inventory
                .as_ref()
                .unwrap(),
            &owned,
        ));
        let mut remaining = [true; 3];
        for output in order {
            assert!(
                take_output(&mut block, output),
                "extraction order {order:?}"
            );
            remaining[output] = false;
            assert_eq!(cached_outputs(&block), remaining);
        }
        assert_eq!(
            block.fastpq_source_inventory().unwrap(),
            Some(owned.as_ref())
        );
    }
}

#[test]
fn directly_mutated_cached_public_bundles_cannot_commit_without_recapture_or_getters() {
    let _guard = witness::exec_witness_guard();
    for missing_digest in [false, true] {
        for replay_at_commit in [false, true] {
            let state = state();
            witness::start_block();
            let mut block = state.block(header());
            cache_canonical_test_transaction_set(&mut block, &[]);
            stage_marker_and_membership(&mut block);
            finalized_source(&mut block, Hash::new(b"cached public mutation at commit"));
            block.capture_exec_witness().unwrap();
            change_public_content(
                &mut block.exec_witness.as_mut().unwrap().fastpq_transcripts[0].transcripts[0],
                missing_digest,
            );
            assert_eq!(cached_outputs(&block), [true; 3]);
            assert!(block.fastpq_source_inventory.as_ref().unwrap().is_ok());
            block.authenticated_replay_commit = replay_at_commit;
            assert_eq!(
                block.commit(),
                Err(TransactionsBlockError::FastpqSourceInventory)
            );
            assert_not_published(&state);
        }
    }
}

fn unexpected_prebuilt_batch() -> iroha_data_model::fastpq::FastpqTransitionBatch {
    // Even an otherwise empty DTO is outside ordinary recorder ownership.
    iroha_data_model::fastpq::FastpqTransitionBatch {
        parameter: "unexpected-prebuilt-ordinary-batch".to_owned(),
        public_inputs: iroha_data_model::fastpq::FastpqPublicInputs {
            dsid: [0; 16],
            slot: 0,
            old_root: [0; 32],
            new_root: [0; 32],
            perm_root: [0; 32],
            tx_set_hash: [0; 32],
        },
        transitions: Vec::new(),
        metadata: BTreeMap::new(),
    }
}

#[test]
fn cached_prebuilt_batches_reject_recapture_and_every_first_getter_with_sticky_failure() {
    let _guard = witness::exec_witness_guard();
    for with_transfer in [false, true] {
        for first_operation in 0..4 {
            let state = state();
            witness::start_block();
            let mut block = state.block(header());
            cache_canonical_test_transaction_set(&mut block, &[]);
            stage_marker_and_membership(&mut block);
            let original = if with_transfer {
                finalized_source(&mut block, Hash::new(b"cached unexpected prebuilt batches"))
            } else {
                block
                    .finalize_fastpq_source_inventory(&[], &[], &[])
                    .unwrap();
                block.drain_transfer_transcripts_with_pending(None)
            };
            block.capture_exec_witness().unwrap();
            assert_eq!(cached_outputs(&block), [true, with_transfer, true]);
            block
                .exec_witness
                .as_mut()
                .unwrap()
                .fastpq_batches
                .push(unexpected_prebuilt_batch());
            if first_operation == 0 {
                assert!(block.capture_exec_witness().is_err());
            } else {
                assert!(!take_output(&mut block, first_operation - 1));
            }
            let error = assert_raw_content_failure(&block);
            assert_eq!(
                error,
                "ordinary captured witness contains prebuilt FASTPQ batches"
            );
            assert_getters_refuse(&mut block, &error);
            // Restarting the recorder with the original archive cannot repair the
            // StateBlock content failure after the unauthorized batch was observed.
            witness::start_block();
            witness::synchronize_fastpq_transcripts(&original);
            assert_eq!(block.capture_exec_witness(), Err(error.clone()));
            assert_eq!(assert_raw_content_failure(&block), error);
            assert_eq!(
                block.commit(),
                Err(TransactionsBlockError::FastpqSourceInventory)
            );
            assert_not_published(&state);
        }
    }
}

#[test]
fn cached_prebuilt_batches_cannot_commit_without_recapture_or_getters() {
    let _guard = witness::exec_witness_guard();
    for with_transfer in [false, true] {
        for replay_at_commit in [false, true] {
            let state = state();
            witness::start_block();
            let mut block = state.block(header());
            cache_canonical_test_transaction_set(&mut block, &[]);
            stage_marker_and_membership(&mut block);
            if with_transfer {
                finalized_source(
                    &mut block,
                    Hash::new(b"direct commit of injected prebuilt batch"),
                );
            } else {
                block
                    .finalize_fastpq_source_inventory(&[], &[], &[])
                    .unwrap();
                assert!(
                    block
                        .drain_transfer_transcripts_with_pending(None)
                        .is_empty()
                );
            }
            block.capture_exec_witness().unwrap();
            block
                .exec_witness
                .as_mut()
                .unwrap()
                .fastpq_batches
                .push(unexpected_prebuilt_batch());
            assert_eq!(cached_outputs(&block), [true, with_transfer, true]);
            assert!(block.fastpq_source_inventory.as_ref().unwrap().is_ok());
            block.authenticated_replay_commit = replay_at_commit;
            assert_eq!(
                block.commit(),
                Err(TransactionsBlockError::FastpqSourceInventory)
            );
            assert_not_published(&state);
        }
    }
}
