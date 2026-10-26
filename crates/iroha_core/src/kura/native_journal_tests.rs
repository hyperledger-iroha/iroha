//! Native journal corruption cannot be treated as permission to rewrite committed history.
use super::*;
use crate::{
    state::World,
    sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
};

fn journal_image(store: &BlockStore) -> Vec<Vec<u8>> {
    [
        DATA_FILE_NAME,
        INDEX_FILE_NAME,
        HASHES_FILE_NAME,
        COUNT_FILE_NAME,
    ]
    .map(|name| std::fs::read(store.path_to_blockchain.join(name)).unwrap())
    .to_vec()
}

#[test]
fn strict_native_journal_audit_preserves_valid_original_bytes() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    // Resolve certified receipts before holding the storage mutex; the receipt
    // reader acquires that same original journal owner.
    let expected = vec![
        chain.committed(1).block_hash(),
        chain.committed(2).block_hash(),
    ];
    let mut store = chain.kura().block_store.lock();
    let original = journal_image(&store);
    let validated = Kura::init_canonical_chain(&mut store, 2).unwrap();
    assert_eq!(validated.hashes, expected);
    assert_eq!(journal_image(&store), original);
}

#[test]
fn strict_native_journal_audit_rejects_corrupt_occupied_frame_without_mutation() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let mut store = chain.kura().block_store.lock();
    let slot = store.read_block_index(1).unwrap();
    let mut bytes = vec![0; usize::try_from(slot.length).unwrap()];
    store.read_block_data(slot.start, &mut bytes).unwrap();
    bytes[0] ^= 1;
    store.write_block_data(slot.start, &bytes).unwrap();
    let corrupted = journal_image(&store);
    assert!(Kura::init_canonical_chain(&mut store, 2).is_err());
    assert_eq!(
        journal_image(&store),
        corrupted,
        "audit neither truncates nor repairs occupied evidence"
    );
    assert_eq!(store.read_exact_durable_index_count().unwrap(), 2);
}

#[test]
fn strict_native_journal_audit_rejects_hash_mismatch_without_rewriting_hashes() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let mut store = chain.kura().block_store.lock();
    let mut slots = vec![BlockIndex::default(); 2];
    store.read_block_indices(0, &mut slots).unwrap();
    let mut expected = store.read_block_hashes(0, 2).unwrap();
    expected[1] = expected[0];
    let original = journal_image(&store);
    assert!(matches!(
        Kura::validate_block_chain(&mut store, &slots, Some(&expected)),
        Err(Error::CanonicalBlockWireMismatch { height: 2 })
    ));
    assert_eq!(journal_image(&store), original);
}

#[test]
fn native_capacity_refusal_preserves_original_journals_and_da_custody() {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let block = chain.committed(1).block().clone();
    let mut kura = Kura::blank_kura_for_testing();
    Arc::get_mut(&mut kura).unwrap().max_disk_usage_bytes = 1;
    let before = journal_image(&kura.block_store.lock());
    let da = kura.block_store.lock().da_blocks_dir.clone();
    std::fs::create_dir_all(&da).unwrap();
    let custody = da.join("capacity-custody.bin");
    let bytes = block.encode_wire().unwrap();
    std::fs::write(&custody, &bytes).unwrap();
    assert!(matches!(
        kura.store_block(block),
        Err(Error::StorageBudgetExceeded { .. })
    ));
    assert_eq!(journal_image(&kura.block_store.lock()), before);
    assert_eq!(std::fs::read(custody).unwrap(), bytes);
    assert_eq!(kura.block_data.lock().len(), 0);
}

#[test]
fn native_pending_capacity_counts_each_original_wire_with_its_index_and_hash() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let kura = Kura::blank_kura_for_testing();
    let first = chain.committed(1).block().clone();
    let second = chain.committed(2).block().clone();
    let expected =
        Kura::block_required_bytes(&first).unwrap() + Kura::block_required_bytes(&second).unwrap();
    {
        let mut pending = kura.block_data.lock();
        pending.push((first.hash(), Some(first)));
        pending.push((second.hash(), Some(second)));
    }
    assert_eq!(kura.pending_block_bytes_raw(0).unwrap(), expected);
    assert_eq!(kura.pending_block_bytes_raw(2).unwrap(), 0);
}

#[test]
fn native_canonical_recovery_preserves_committed_bodies_without_an_eviction_owner() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let kura = chain.kura();
    let before = journal_image(&kura.block_store.lock());
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    kura.resolve_canonical_storage_before_mutation().unwrap();
    assert_eq!(journal_image(&kura.block_store.lock()), before);
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 2);
}

#[test]
fn native_canonical_recovery_cannot_reopen_poisoned_storage() {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let kura = chain.kura();
    let before = journal_image(&kura.block_store.lock());
    kura.poison_canonical_storage("native recovery test", &Error::CanonicalStoragePoisoned);
    let _prune = kura.prune_lock.lock();
    let _canonical = kura.canonical_chain_lock.lock();
    assert!(matches!(
        kura.resolve_canonical_storage_before_mutation(),
        Err(Error::CanonicalStoragePoisoned)
    ));
    assert_eq!(journal_image(&kura.block_store.lock()), before);
}

#[test]
fn native_rewrite_image_requires_the_complete_original_frame() {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let block = chain.committed(1);
    let body = block.block().encode_wire().unwrap();
    let store = BlockStore::new(Path::new(""));
    let mut image = DaBlockRewriteImageV1 {
        height: 1,
        block_hash: block.block_hash(),
        index_start: EVICTED_BLOCK_START,
        index_length: body.len() as u64,
        body,
    };
    store.validate_da_block_rewrite_image(&image).unwrap();
    image.body.clear();
    assert!(store.validate_da_block_rewrite_image(&image).is_err());
    image.index_length = 0;
    assert!(
        store.validate_da_block_rewrite_image(&image).is_err(),
        "an empty evicted frame cannot recreate retired snapshot authority"
    );
}

#[test]
fn canonical_summary_source_token_tracks_exact_journals_and_refuses_poison() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let height = std::num::NonZeroUsize::MIN;
    let first = chain
        .state()
        .canonical_history_summary_source_token(height)
        .unwrap()
        .unwrap();
    assert_eq!(
        chain
            .state()
            .canonical_history_summary_source_token(height)
            .unwrap(),
        Some(first)
    );
    chain.commit(Vec::new());
    let appended = chain
        .state()
        .canonical_history_summary_source_token(height)
        .unwrap()
        .unwrap();
    assert_ne!(
        first, appended,
        "an append invalidates the exact journal image"
    );
    let kura = chain.kura();
    kura.poison_canonical_storage("summary test", &Error::CanonicalStoragePoisoned);
    assert!(
        chain
            .state()
            .canonical_history_summary_source_token(height)
            .is_err()
    );
}

#[test]
fn canonical_summary_source_token_rejects_missing_and_replaced_journals() {
    for (name, replace) in [DATA_FILE_NAME, INDEX_FILE_NAME, HASHES_FILE_NAME]
        .into_iter()
        .flat_map(|name| [false, true].map(|replace| (name, replace)))
    {
        let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        let height = std::num::NonZeroUsize::MIN;
        assert!(
            chain
                .state()
                .canonical_history_summary_source_token(height)
                .unwrap()
                .is_some()
        );
        let path = chain
            .kura()
            .block_store
            .lock()
            .path_to_blockchain
            .join(name);
        let bytes = std::fs::read(&path).unwrap();
        let original = path.with_extension("original-summary-test");
        std::fs::rename(&path, &original).unwrap();
        if replace {
            std::fs::write(&path, &bytes).unwrap();
        }
        assert_eq!(
            chain
                .state()
                .canonical_history_summary_source_token(height)
                .unwrap(),
            None,
            "same bytes in another journal object do not retain summary authority"
        );
    }
}

#[test]
fn canonical_summary_source_token_invalidates_mutated_and_unavailable_bodies() {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let height = std::num::NonZeroUsize::MIN;
    let first = chain
        .state()
        .canonical_history_summary_source_token(height)
        .unwrap()
        .unwrap();
    let kura = chain.kura();
    let wire = kura.canonical_block_wire_bytes_for_testing(height).unwrap();
    kura.overwrite_native_frame_bytes_for_test(height, 0, &[wire[0] ^ 1]);
    assert_ne!(
        chain
            .state()
            .canonical_history_summary_source_token(height)
            .unwrap(),
        Some(first)
    );
    // The unavailable-slot helper publishes a commit marker and validates its
    // journal. Exercise absence on a separate intact source, not the malformed
    // frame above, whose rejection must remain authoritative.
    let unavailable = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    unavailable
        .kura()
        .corrupt_canonical_body_for_testing(height)
        .unwrap();
    assert_eq!(
        unavailable
            .state()
            .canonical_history_summary_source_token(height)
            .unwrap(),
        None
    );
}

#[test]
fn canonical_summary_source_token_refuses_recovery_and_evicted_sidecar_reuse() {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let height = std::num::NonZeroUsize::MIN;
    let kura = chain.kura();
    assert!(
        chain
            .state()
            .canonical_history_summary_source_token(height)
            .unwrap()
            .is_some()
    );
    kura.prune_recovery_required.store(true, Ordering::Release);
    assert!(
        chain
            .state()
            .canonical_history_summary_source_token(height)
            .is_err()
    );
    kura.prune_recovery_required.store(false, Ordering::Release);
    kura.evict_native_frame_for_test(height);
    assert_eq!(
        chain
            .state()
            .canonical_history_summary_source_token(height)
            .unwrap(),
        None
    );
    let path = kura.block_store.lock().da_block_path(1);
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        chain
            .state()
            .canonical_history_summary_source_token(height)
            .unwrap(),
        None
    );
}

#[test]
fn canonical_summary_read_retains_required_target_budget_provenance() {
    let chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    let owner = crate::state::CanonicalHistoryReadBudget::new(
        iroha_allocation::AllocationBudget::new(1),
        norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            1_000_000, 1_000_000, 1_000_000, 1_000_000, 64,
        )),
    );
    let error = chain
        .state()
        .read_canonical_history_block(std::num::NonZeroUsize::MIN, &owner)
        .unwrap_err();
    let crate::execution_attempt::ExecutionAttemptError::Deferred(deferred) = error else {
        panic!("required target must retain the original physical refusal")
    };
    assert_eq!(
        deferred.phase(),
        Some(crate::execution_attempt::ExecutionPhase::HistoryTarget)
    );
    assert!(matches!(
        deferred.allocation_refusal(),
        Some(iroha_allocation::AllocationRefusal::ExceedsLimit { .. })
    ));
}

#[test]
fn canonical_summary_read_preserves_exhausted_decoder_without_runtime_projection_panic() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let height = std::num::NonZeroUsize::new(2).unwrap();
    let expected = chain.committed(2).block_hash();
    let frames = iroha_allocation::AllocationBudget::new(8 * 1024 * 1024);
    let exhausted = crate::state::CanonicalHistoryReadBudget::new(
        frames.clone(),
        norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            1_000_000, 1_000_000, 1_000_000, 0, 64,
        )),
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        chain
            .state()
            .read_canonical_history_block(height, &exhausted)
    }))
    .expect("request exhaustion must not invalidate or panic in the runtime projection");
    let crate::execution_attempt::ExecutionAttemptError::Deferred(deferred) = result.unwrap_err()
    else {
        panic!("the original decoder refusal must remain retryable")
    };
    assert_eq!(
        deferred.phase(),
        Some(crate::execution_attempt::ExecutionPhase::HistoryTarget)
    );
    assert!(deferred.decode_refusal().is_some());

    let admitted = crate::state::CanonicalHistoryReadBudget::new(
        frames,
        norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            1_000_000, 1_000_000, 8_000_000, 8_000_000, 64,
        )),
    );
    assert_eq!(
        chain
            .state()
            .read_canonical_history_block(height, &admitted)
            .unwrap()
            .hash(),
        expected
    );
}

#[test]
fn canonical_summary_read_does_not_acquire_unrelated_world_publication() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
    chain.commit(Vec::new());
    let height = std::num::NonZeroUsize::new(2).unwrap();
    let expected = chain.committed(2).block_hash();
    let owner = crate::state::CanonicalHistoryReadBudget::new(
        iroha_allocation::AllocationBudget::new(8 * 1024 * 1024),
        norito::core::DecodeBudgetContext::new(norito::DecodeLimits::new(
            1_000_000, 1_000_000, 8_000_000, 8_000_000, 64,
        )),
    );
    // No history is changed under this genuine publication fence. Release the
    // fence even on regression, so an accidental World view fails instead of
    // leaving the test's reader spinning forever.
    let state = chain.state();
    let completed = std::thread::scope(|scope| {
        state.with_held_view_publication_for_reader_test(|_| {
            let (sender, receiver) = std::sync::mpsc::sync_channel(1);
            scope.spawn(move || {
                let _ = sender.send(state.read_canonical_history_block(height, &owner));
            });
            receiver.recv_timeout(std::time::Duration::from_secs(2))
        })
    });
    let block = completed
        .expect("a block read does not wait for unrelated World publication")
        .unwrap();
    assert_eq!(block.hash(), expected);
}
