//! Regression test for reading blocks from large sparse files without loading the entire file into memory.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]

use std::time::{Duration, Instant};

use iroha_config::kura::FsyncMode;
use iroha_core::kura::BlockStore;
use tempfile::tempdir;

#[test]
#[cfg_attr(
    not(target_pointer_width = "64"),
    ignore = "requires 64-bit address space"
)]
fn block_bytes_sparse_file_reads_requested_slice() {
    const FILE_LEN: u64 = 16 * 1024 * 1024 * 1024; // 16 GiB
    let temp_dir = tempdir().expect("create temp dir");
    // Use fsync off so init does not prune entries based on a stale commit marker,
    // which is irrelevant for this sparse-read regression test.
    let mut store = BlockStore::with_fsync(temp_dir.path(), FsyncMode::Off, Duration::ZERO);
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

    let _ = slice;
    drop(store);

    let mut reopened = BlockStore::with_fsync(temp_dir.path(), FsyncMode::Off, Duration::ZERO);
    reopened
        .create_files_if_they_do_not_exist()
        .expect("reopen block store");
    let reopened_slice = reopened
        .block_bytes(offset, payload.len() as u64)
        .expect("read sparse payload after reopen");
    assert_eq!(reopened_slice, payload);
}
