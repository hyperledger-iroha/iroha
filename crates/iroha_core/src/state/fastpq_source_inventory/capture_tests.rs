//! Final witness capture requires intact validator-owned source inventory and applied seals.

use super::{
    tests::{apply_source, cache_canonical_test_transaction_set, delta, header, state},
    *,
};
use iroha_data_model::nexus::LaneId;
use iroha_test_samples::ALICE_ID;

fn assert_no_cached_capture(block: &mut StateBlock<'_>) {
    assert!(block.exec_witness.is_none());
    assert!(block.fastpq_witness_context.is_none());
    assert!(block.parliament_timed_ovn_casting_bindings.is_none());
    assert!(block.take_exec_witness().is_none());
    assert!(block.take_fastpq_witness_context().is_none());
    assert!(block.take_parliament_timed_ovn_casting_bindings().is_none());
}

/// Witness, FASTPQ context and casting bindings, each exercised as the first extraction.
const CAPTURE_EXTRACTION_ORDERS: [[usize; 3]; 6] = [
    [0, 1, 2],
    [0, 2, 1],
    [1, 0, 2],
    [1, 2, 0],
    [2, 0, 1],
    [2, 1, 0],
];

fn cached_output_presence(block: &StateBlock<'_>) -> [bool; 3] {
    [
        block.exec_witness.is_some(),
        block.fastpq_witness_context.is_some(),
        block.parliament_timed_ovn_casting_bindings.is_some(),
    ]
}

fn take_capture_output(block: &mut StateBlock<'_>, output: usize) -> bool {
    match output {
        0 => block.take_exec_witness().is_some(),
        1 => block.take_fastpq_witness_context().is_some(),
        2 => block.take_parliament_timed_ovn_casting_bindings().is_some(),
        _ => unreachable!("only the three capture outputs have extraction accessors"),
    }
}

fn cache_transfer_capture(block: &mut StateBlock<'_>, hash: Hash) {
    apply_source(block, hash, false, None);
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    block.drain_transfer_transcripts_with_pending(None);
    block.capture_exec_witness().unwrap();
    assert_eq!(cached_output_presence(block), [true; 3]);
}

#[test]
fn missing_or_failed_inventory_refuses_capture_before_draining_active_witness() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    for failed_inventory in [false, true] {
        crate::sumeragi::witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        let hash = Hash::new(b"capture requires owned inventory");
        apply_source(&mut block, hash, false, None);
        let construction_error = if failed_inventory {
            block.fastpq_transcripts.get_mut(&hash).unwrap().clear();
            Some(
                block
                    .finalize_fastpq_source_inventory(&[], &[], &[])
                    .unwrap_err(),
            )
        } else {
            None
        };
        let original_archive = block.fastpq_transcripts.clone();
        let capture_error = block.capture_exec_witness().unwrap_err();
        if let Some(expected) = construction_error {
            assert_eq!(capture_error, expected);
        } else {
            assert!(capture_error.contains("no finalized owned inventory"));
        }
        assert_no_cached_capture(&mut block);
        assert_eq!(block.fastpq_transcripts, original_archive);
        let active = crate::sumeragi::witness::drain_exec_witness();
        assert_eq!(active.fastpq_transcripts.len(), 1);
        assert_eq!(active.fastpq_transcripts[0].entry_hash, hash);
    }
}

#[test]
fn sealed_empty_and_transferred_inventory_capture_without_reconstruction() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    for with_transfer in [false, true] {
        crate::sumeragi::witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        if with_transfer {
            apply_source(&mut block, Hash::new(b"sealed transfer"), false, None);
        }
        block
            .finalize_fastpq_source_inventory(&[], &[], &[])
            .unwrap();
        let owned = block
            .verified_fastpq_source_inventory_for_capture()
            .unwrap();
        let retained = block
            .fastpq_source_inventory
            .as_ref()
            .unwrap()
            .as_ref()
            .unwrap();
        assert!(Arc::ptr_eq(&owned, retained));
        let archive = block.drain_transfer_transcripts_with_pending(None);
        assert_eq!(archive.len(), usize::from(with_transfer));
        block.capture_exec_witness().unwrap();
        let witness = block.exec_witness.as_ref().unwrap();
        assert_eq!(witness.fastpq_transcripts.len(), usize::from(with_transfer));
        if with_transfer {
            let context = block.fastpq_witness_context.as_ref().unwrap();
            assert!(Arc::ptr_eq(
                context._source_inventory.as_ref().unwrap(),
                &owned
            ));
            assert_eq!(context.tx_set_hash, Some(owned.tx_set_hash()));
        } else {
            assert!(owned.entries().is_empty());
            assert!(block.fastpq_witness_context.is_none());
        }
        // A repeated capture still checks the applied seal while preserving the cached witness.
        block.capture_exec_witness().unwrap();
        assert!(block.exec_witness.is_some());
        assert!(Arc::ptr_eq(
            &block
                .verified_fastpq_source_inventory_for_capture()
                .unwrap(),
            &owned,
        ));
    }
}

#[test]
fn same_or_new_key_late_apply_refuses_capture_even_after_late_data_is_drained() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    for same_key in [false, true] {
        for drain_late in [false, true] {
            crate::sumeragi::witness::start_block();
            let mut block = state.block(header());
            cache_canonical_test_transaction_set(&mut block, &[]);
            let original_hash = Hash::new(b"original sealed source");
            apply_source(&mut block, original_hash, false, None);
            block
                .finalize_fastpq_source_inventory(&[], &[], &[])
                .unwrap();
            let owned = block
                .verified_fastpq_source_inventory_for_capture()
                .unwrap();
            block.drain_transfer_transcripts_with_pending(None);
            let late_hash = if same_key {
                original_hash
            } else {
                Hash::new(b"late new source")
            };
            apply_source(&mut block, late_hash, false, None);
            if drain_late {
                let drained = block.drain_transfer_transcripts_with_pending(None);
                assert_eq!(drained.len(), 1);
                assert!(drained.contains_key(&late_hash));
                assert!(block.fastpq_transcripts.is_empty());
            }
            assert!(
                block
                    .verified_fastpq_source_inventory_for_capture()
                    .is_err()
            );
            assert!(block.capture_exec_witness().is_err());
            assert_no_cached_capture(&mut block);
            // Draining or retrying cannot turn a late applied occurrence into a valid seal.
            block.drain_transfer_transcripts_with_pending(None);
            assert!(block.capture_exec_witness().is_err());
            assert_eq!(
                block.fastpq_source_inventory().unwrap(),
                Some(owned.as_ref())
            );
        }
    }
}

#[test]
fn rolled_back_transfer_and_empty_apply_preserve_sealed_capture() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    crate::sumeragi::witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let original_hash = Hash::new(b"sealed source survives rollback");
    apply_source(&mut block, original_hash, false, None);
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    let owned = block
        .verified_fastpq_source_inventory_for_capture()
        .unwrap();
    block.drain_transfer_transcripts_with_pending(None);
    {
        let witness_overlay = crate::sumeragi::witness::begin_exec_witness_overlay();
        let mut tx = block.transaction();
        tx.tx_call_hash = Some(original_hash);
        tx.current_lane_id = Some(LaneId::new(999));
        tx.record_test_transfer_transcripts(&ALICE_ID, original_hash, vec![delta()]);
        // Transaction rollback discards the unapplied source capture. The recorder
        // overlay separately discards its speculative raw transcript, preserving
        // the finalized global archive copied before this rollback-only attempt.
        drop(tx);
        drop(witness_overlay);
    }
    block.transaction().apply();
    assert!(block.fastpq_transcripts.is_empty());
    assert!(Arc::ptr_eq(
        &block
            .verified_fastpq_source_inventory_for_capture()
            .unwrap(),
        &owned,
    ));
    block.capture_exec_witness().unwrap();
    assert!(block.exec_witness.is_some());
}

#[test]
fn later_applied_transfer_invalidates_and_clears_previously_cached_capture() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    crate::sumeragi::witness::start_block();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    let hash = Hash::new(b"cached source capture");
    apply_source(&mut block, hash, false, None);
    block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap();
    block.drain_transfer_transcripts_with_pending(None);
    block.capture_exec_witness().unwrap();
    assert!(block.exec_witness.is_some());
    assert!(block.fastpq_witness_context.is_some());
    apply_source(&mut block, hash, false, None);
    block.drain_transfer_transcripts_with_pending(None);
    assert!(block.capture_exec_witness().is_err());
    assert_no_cached_capture(&mut block);
    assert!(block.capture_exec_witness().is_err());
    assert_no_cached_capture(&mut block);
}

#[test]
fn capture_rejects_unsealed_replaced_contexts_and_changed_source_caches() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    for mutation in 0..8 {
        crate::sumeragi::witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        let hash = Hash::new(b"capture consistency");
        apply_source(&mut block, hash, false, None);
        block
            .finalize_fastpq_source_inventory(&[], &[], &[])
            .unwrap();
        block.drain_transfer_transcripts_with_pending(None);
        match mutation {
            0 => block.fastpq_source_captures = Default::default(),
            1 => block.fastpq_source_context = None,
            2 => {
                Arc::make_mut(block.fastpq_source_context.as_mut().unwrap())
                    .source
                    .height += 1;
            }
            3 => block.fastpq_tx_set_hash = Some([42; 32]),
            4 => block.fastpq_entry_dataspaces.clear(),
            5 => {
                block
                    .fastpq_entry_dataspaces
                    .insert(hash, DataSpaceId::new(23));
            }
            6 | 7 => {
                let replacement_hash = if mutation == 6 {
                    Hash::new(b"replacement sealed key")
                } else {
                    hash
                };
                let captured = block
                    .fastpq_source_context
                    .as_ref()
                    .unwrap()
                    .capture_transcript(
                        Some(replacement_hash),
                        replacement_hash,
                        Some(LaneId::SINGLE),
                        Some(DataSpaceId::UNIVERSAL),
                        0,
                    );
                let mut replacement = crate::fastpq::FastpqSourceCaptureAccumulator::default();
                replacement.record(captured);
                replacement.seal().unwrap();
                block.fastpq_source_captures = replacement;
            }
            _ => unreachable!(),
        }
        assert!(
            block
                .verified_fastpq_source_inventory_for_capture()
                .is_err(),
            "mutation {mutation}"
        );
        assert!(block.capture_exec_witness().is_err(), "mutation {mutation}");
        assert_no_cached_capture(&mut block);
    }
}

#[test]
fn applied_accumulator_seal_failure_publishes_no_owned_inventory_or_caches() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    let mut block = state.block(header());
    cache_canonical_test_transaction_set(&mut block, &[]);
    apply_source(
        &mut block,
        Hash::new(b"already sealed captures"),
        false,
        None,
    );
    block.fastpq_source_captures.seal().unwrap();
    let error = block
        .finalize_fastpq_source_inventory(&[], &[], &[])
        .unwrap_err();
    assert_eq!(block.fastpq_source_inventory(), Err(error.as_str()));
    assert_eq!(
        block.fastpq_tx_set_hash,
        Some(
            iroha_data_model::nexus::axt_ordered_transaction_set_digest_v1(std::iter::empty::<
                &TransactionEntrypoint,
            >(),)
            .unwrap()
            .into()
        )
    );
    assert!(block.fastpq_entry_dataspaces.is_empty());
    assert_eq!(
        block
            .verified_fastpq_source_inventory_for_capture()
            .unwrap_err(),
        error
    );
}

#[test]
fn authenticated_replay_clears_active_witness_without_fabricating_inventory() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    for failed_inventory in [false, true] {
        crate::sumeragi::witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        apply_source(&mut block, Hash::new(b"replay active witness"), false, None);
        if failed_inventory {
            block.fastpq_source_inventory = Some(Err("retained local construction error".into()));
        }
        let original_inventory = block.fastpq_source_inventory.clone();
        block.authenticated_replay_commit = true;
        block.capture_exec_witness().unwrap();
        assert_no_cached_capture(&mut block);
        assert_eq!(block.fastpq_source_inventory, original_inventory);
        let active = crate::sumeragi::witness::drain_exec_witness();
        assert!(active.reads.is_empty());
        assert!(active.writes.is_empty());
        assert!(active.fastpq_transcripts.is_empty());
    }
}

#[test]
fn intact_capture_outputs_can_be_taken_in_every_order_without_recapture() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    for order in CAPTURE_EXTRACTION_ORDERS {
        crate::sumeragi::witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        cache_transfer_capture(&mut block, Hash::new(b"healthy capture extraction order"));
        let mut expected_presence = [true; 3];
        for output in order {
            assert!(
                take_capture_output(&mut block, output),
                "order {order:?}, output {output}"
            );
            expected_presence[output] = false;
            assert_eq!(cached_output_presence(&block), expected_presence);
        }
        assert_no_cached_capture(&mut block);
        assert!(block.verified_fastpq_source_inventory_for_capture().is_ok());
    }
}

#[test]
fn each_extraction_accessor_first_rejects_late_applies_without_recapture() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    for same_key in [false, true] {
        for order in CAPTURE_EXTRACTION_ORDERS {
            crate::sumeragi::witness::start_block();
            let mut block = state.block(header());
            cache_canonical_test_transaction_set(&mut block, &[]);
            let original = Hash::new(b"captured before direct extraction");
            cache_transfer_capture(&mut block, original);
            let late = if same_key {
                original
            } else {
                Hash::new(b"late source before direct extraction")
            };
            apply_source(&mut block, late, false, None);
            let late_archive = block.drain_transfer_transcripts_with_pending(None);
            assert!(late_archive.contains_key(&late));
            assert!(block.fastpq_transcripts.is_empty());
            // All stale outputs are still present. The first getter must invalidate them
            // without relying on a second capture or on any particular extraction order.
            assert_eq!(cached_output_presence(&block), [true; 3]);
            for output in order {
                assert!(
                    !take_capture_output(&mut block, output),
                    "same_key {same_key}, order {order:?}, output {output}"
                );
                assert_eq!(cached_output_presence(&block), [false; 3]);
            }
            assert_no_cached_capture(&mut block);
            assert!(
                block
                    .verified_fastpq_source_inventory_for_capture()
                    .is_err()
            );
        }
    }
}

#[test]
fn authenticated_replay_capture_discards_all_previously_cached_outputs() {
    let _guard = crate::sumeragi::witness::exec_witness_guard();
    let state = state();
    for order in CAPTURE_EXTRACTION_ORDERS {
        crate::sumeragi::witness::start_block();
        let mut block = state.block(header());
        cache_canonical_test_transaction_set(&mut block, &[]);
        cache_transfer_capture(&mut block, Hash::new(b"cached before replay transition"));
        let original_inventory = block.fastpq_source_inventory.clone();
        block.authenticated_replay_commit = true;
        block.capture_exec_witness().unwrap();
        // Inspect raw fields before calling guarded getters, so getters cannot mask a
        // replay capture that forgot to discard an old ordinary witness or its context.
        assert_eq!(cached_output_presence(&block), [false; 3]);
        for output in order {
            assert!(!take_capture_output(&mut block, output), "order {order:?}");
            assert_eq!(cached_output_presence(&block), [false; 3]);
        }
        assert_eq!(block.fastpq_source_inventory, original_inventory);
        let active = crate::sumeragi::witness::drain_exec_witness();
        assert!(active.reads.is_empty());
        assert!(active.writes.is_empty());
        assert!(active.fastpq_transcripts.is_empty());
    }
}
