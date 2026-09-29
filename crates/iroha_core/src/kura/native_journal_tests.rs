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
    let block = Arc::clone(chain.committed(1).block());
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
    let first = Arc::clone(chain.committed(1).block());
    let second = Arc::clone(chain.committed(2).block());
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
