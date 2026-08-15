//! Regression test for reading blocks from large sparse files without loading the entire file into memory.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
use iroha_config::kura::FsyncMode;
use iroha_core::kura::BlockStore;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::BlockHeader;
use std::time::{Duration, Instant};
use tempfile::tempdir;
#[test]
#[cfg_attr(
    not(target_pointer_width = "64"),
    ignore = "requires 64-bit address space"
)]
fn block_bytes_sparse_file_reads_requested_slice() {
    const FILE_LEN: u64 = 16 * 1024 * 1024 * 1024; // 16 GiB
    let temp_dir = tempdir().expect("create temp dir");
    // Zero-interval batching exercises the production durable-write policy.
    let mut store = BlockStore::with_fsync(temp_dir.path(), FsyncMode::Batched, Duration::ZERO);
    store
        .create_files_if_they_do_not_exist()
        .expect("initialize block store");
    let payload = b"block-bytes-regression";
    let offset = FILE_LEN - payload.len() as u64;
    store
        .write_block_data(offset, payload)
        .expect("write sparse payload");
    store
        .write_block_index(0, offset, payload.len() as u64)
        .expect("write sparse index");
    store
        .write_block_hash(
            0,
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x5A; Hash::LENGTH])),
        )
        .expect("write sparse hash");
    let start = Instant::now();
    let slice = store
        .block_bytes(offset, payload.len() as u64)
        .expect("read sparse payload");
    let elapsed = start.elapsed();
    assert_eq!(slice, payload);
    assert!(
        elapsed < Duration::from_secs(5),
        "reading sparse payload took {elapsed:?}",
    );
}
