//! Finalized source ownership remains mandatory through state publication.

use super::{
    tests::{apply_source, cache_canonical_test_transaction_set, delta, header, state},
    *,
};
use crate::state::{State, TransactionsBlockError};
use iroha_crypto::HashOf;
use iroha_data_model::state_path::StatePath;
use iroha_test_samples::ALICE_ID;
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;

fn marker() -> StatePath {
    "fastpq/commit-seal-marker".parse().unwrap()
}

fn state_with_marker() -> State {
    let state = state();
    {
        let mut storage = state.world.smart_contract_state.block();
        storage.insert(marker(), vec![0]);
        storage.commit();
    }
    state
}

fn apply_marker(block: &mut StateBlock<'_>, value: u8, source: Option<Hash>) {
    let mut tx = block.transaction();
    tx.world.smart_contract_state.insert(marker(), vec![value]);
    if let Some(hash) = source {
        tx.tx_call_hash = Some(hash);
        tx.record_test_transfer_transcripts(&ALICE_ID, hash, vec![delta()]);
    }
    tx.apply();
}

fn stage_membership(block: &mut StateBlock<'_>, source: Option<Hash>) {
    block
        .stage_canonical_carrier_membership(
            source.map(HashOf::<TransactionEntrypoint>::from_untyped_unchecked),
            nonzero!(1_usize),
        )
        .unwrap();
    block.block_hashes.push(block._curr_block.hash());
}

fn take_all_captured_outputs(block: &mut StateBlock<'_>, with_transfer: bool) {
    assert!(block.take_exec_witness().is_some());
    assert!(block.take_parliament_timed_ovn_casting_bindings().is_some());
    assert_eq!(block.take_fastpq_witness_context().is_some(), with_transfer);
    assert!(block.exec_witness.is_none());
    assert!(block.parliament_timed_ovn_casting_bindings.is_none());
    assert!(block.fastpq_witness_context.is_none());
}

fn assert_unpublished(state: &State) {
    assert_eq!(
        state.world.smart_contract_state.view().get(&marker()),
        Some(&vec![0]),
        "rejected commit must preserve the previously stored value"
    );
    assert_eq!(state.committed_height(), 0);
    assert_eq!(state.transactions.latest_height(), 0);
    assert!(state.latest_block_hash_fast().is_none());
}

#[test]
fn intact_finalized_inventory_commits_after_all_cached_outputs_are_taken() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    for with_transfer in [false, true] {
        for replay in [false, true] {
            let state = state_with_marker();
            crate::sumeragi::witness::start_block();
            let mut block = state.block(header());
            cache_canonical_test_transaction_set(&mut block, &[]);
            let source = with_transfer.then(|| Hash::new(b"committable finalized source"));
            apply_marker(&mut block, 1, source);
            block
                .finalize_fastpq_source_inventory(&[], &[], &[])
                .unwrap();
            block.drain_transfer_transcripts_with_pending(None);
            block.capture_exec_witness().unwrap();
            take_all_captured_outputs(&mut block, with_transfer);
            block.authenticated_replay_commit = replay;
            stage_membership(&mut block, source);
            block.commit().unwrap();
            assert_eq!(
                state.world.smart_contract_state.view().get(&marker()),
                Some(&vec![1]),
            );
            assert_eq!(state.committed_height(), 1);
            assert_eq!(state.transactions.latest_height(), 1);
            assert_eq!(state.latest_block_hash_fast(), Some(header().hash()));
        }
    }
}

#[test]
fn late_applied_source_cannot_commit_after_all_cached_outputs_are_taken() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    for same_key in [false, true] {
        for drain_late in [false, true] {
            for replay in [false, true] {
                let state = state_with_marker();
                crate::sumeragi::witness::start_block();
                let mut block = state.block(header());
                cache_canonical_test_transaction_set(&mut block, &[]);
                let original = Hash::new(b"captured source before extraction");
                apply_marker(&mut block, 1, Some(original));
                block
                    .finalize_fastpq_source_inventory(&[], &[], &[])
                    .unwrap();
                block.drain_transfer_transcripts_with_pending(None);
                block.capture_exec_witness().unwrap();
                take_all_captured_outputs(&mut block, true);
                block.authenticated_replay_commit = replay;
                let late = if same_key {
                    original
                } else {
                    Hash::new(b"late source after extraction")
                };
                apply_marker(&mut block, 2, Some(late));
                assert_eq!(
                    block.world.smart_contract_state.get(&marker()),
                    Some(&vec![2])
                );
                if drain_late {
                    let archive = block.drain_transfer_transcripts_with_pending(None);
                    assert_eq!(archive.len(), 1);
                    assert!(archive.contains_key(&late));
                    assert!(block.fastpq_transcripts.is_empty());
                }
                stage_membership(&mut block, Some(original));
                // No second capture or accessor call can be required to reject publication.
                assert_eq!(
                    block.commit(),
                    Err(TransactionsBlockError::FastpqSourceInventory),
                );
                assert_unpublished(&state);
            }
        }
    }
}

#[test]
fn failed_inventory_construction_prevents_commit_without_publishing_overlay() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    for replay in [false, true] {
        let state = state_with_marker();
        crate::sumeragi::witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        let source = Hash::new(b"failed inventory construction");
        apply_source(&mut block, source, false, None);
        apply_marker(&mut block, 2, None);
        block.fastpq_transcripts.get_mut(&source).unwrap().clear();
        let error = block
            .finalize_fastpq_source_inventory(&[], &[], &[])
            .unwrap_err();
        assert!(
            block
                .finalize_fastpq_source_inventory(&[], &[], &[])
                .unwrap_err()
                .contains("already been finalized"),
        );
        assert_eq!(
            block.fastpq_source_inventory(),
            Err(error.as_str()),
            "failed inventory construction must remain latched"
        );
        assert_eq!(
            block.verified_fastpq_source_inventory_for_capture(),
            Err(error),
        );
        block.authenticated_replay_commit = replay;
        stage_membership(&mut block, Some(source));
        assert_eq!(
            block.commit(),
            Err(TransactionsBlockError::FastpqSourceInventory),
        );
        assert_unpublished(&state);
    }
}

#[test]
fn unfinalized_fixture_commit_does_not_require_source_inventory() {
    let state = state_with_marker();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    apply_marker(&mut block, 1, None);
    assert!(block.fastpq_source_inventory.is_none());
    stage_membership(&mut block, None);
    block.commit().unwrap();
    assert_eq!(
        state.world.smart_contract_state.view().get(&marker()),
        Some(&vec![1]),
    );
    assert_eq!(state.committed_height(), 1);
    assert_eq!(state.transactions.latest_height(), 1);
}
