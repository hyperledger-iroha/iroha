//! Canonical spool ownership, authenticated storage, and exact source geometry controls.

use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT_TEST_DIRECTORY_V1: AtomicU64 = AtomicU64::new(0);
struct TestDirectoryV1(PathBuf);
impl TestDirectoryV1 {
    fn new_v1(label: &str) -> Self {
        let ordinal = NEXT_TEST_DIRECTORY_V1.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "iroha-phase23-rns-link-external-spool-{label}-{}-{ordinal}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectoryV1 {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn test_writer_v1(
    directory: &Path,
    main_slots: u64,
    nonce_slots: u64,
) -> RnsLinkSecretSpoolWriterV1 {
    let main_context = [0x44; 32];
    let nonce_context = [0x55; 32];
    let main_layout = ConfidentialSpoolLayoutV1::new_v1(main_slots, 32, main_context).unwrap();
    let nonce_layout = ConfidentialSpoolLayoutV1::new_v1(nonce_slots, 32, nonce_context).unwrap();
    RnsLinkSecretSpoolWriterV1::create_with_layouts_v1(
        directory,
        main_layout,
        nonce_layout,
        [0x11; 32],
        [0x22; 32],
        [0x33; 32],
        main_context,
        nonce_context,
    )
    .unwrap()
}
fn chunk_v1(fill: u8) -> ConfidentialSpoolChunkV1 {
    let mut chunk = ConfidentialSpoolChunkV1::new_zeroed_v1(32).unwrap();
    chunk.as_mut_slice_v1().fill(fill);
    chunk
}
#[test]
#[cfg(unix)]
fn tiny_pair_roundtrip_is_authenticated_and_owns_both_snapshots() {
    let directory = TestDirectoryV1::new_v1("roundtrip");
    let mut writer = test_writer_v1(&directory.0, 1, 1);
    let writer_identity = writer.writer_identity_v1();
    writer.write_main_v1(0, chunk_v1(0xA5)).unwrap();
    writer.write_nonce_v1(0, chunk_v1(0x5A)).unwrap();
    let mut snapshots = writer.seal_v1([0x66; 32]).unwrap();
    assert_eq!(snapshots.writer_identity_v1(), writer_identity);
    assert_ne!(snapshots.provider_identity_v1(), writer_identity);
    assert_ne!(snapshots.snapshot_identity_v1(), writer_identity);
    assert_ne!(
        snapshots.publication_identity_v1(),
        snapshots.provider_identity_v1()
    );
    assert!(matches!(
        snapshots.read_main_v1(1),
        Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
    ));
    assert_eq!(
        snapshots.read_main_v1(0).unwrap().as_slice_v1(),
        &[0xA5; 32]
    );
    assert_eq!(
        snapshots.read_nonce_v1(0).unwrap().as_slice_v1(),
        &[0x5A; 32]
    );
}
#[test]
#[cfg(unix)]
fn pair_rejects_order_and_missing_slots_without_minting_snapshots() {
    let directory = TestDirectoryV1::new_v1("order");
    let mut writer = test_writer_v1(&directory.0, 2, 1);
    assert_eq!(
        writer.write_main_v1(1, chunk_v1(0x11)),
        Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
    );
    assert_eq!(
        writer.write_main_v1(0, chunk_v1(0x22)),
        Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
    );
    assert!(matches!(
        writer.seal_v1([0x77; 32]),
        Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
    ));
    let missing_directory = TestDirectoryV1::new_v1("missing");
    let mut missing = test_writer_v1(&missing_directory.0, 2, 1);
    missing.write_main_v1(0, chunk_v1(0x44)).unwrap();
    missing.write_nonce_v1(0, chunk_v1(0x55)).unwrap();
    assert!(matches!(
        missing.seal_v1([0x77; 32]),
        Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
    ));
}
#[test]
#[cfg(unix)]
fn writer_drop_during_unwind_releases_unlinked_owners() {
    let directory = TestDirectoryV1::new_v1("unwind");
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _writer = test_writer_v1(&directory.0, 1, 1);
        panic!("test unwind");
    }));
    assert!(result.is_err());
    assert!(directory.0.read_dir().unwrap().next().is_none());
}
#[test]
fn production_adapter_surface_has_no_path_key_or_raw_snapshot_escape() {
    let source = include_str!("phase23_rns_link_external_spool.rs");
    let production = source
        .split("#[cfg(test)]\n#[path = \"phase23_rns_link_external_spool_tests.rs\"]\nmod tests;")
        .next()
        .expect("production source prefix");
    assert!(source.lines().count() <= 400);
    assert!(source.len() <= 16_000);
    assert!(production.contains("ConfidentialSpoolWriterV1"));
    assert!(production.contains("ConfidentialSpoolSnapshotV1"));
    assert!(production.contains("canonical_source_layouts_v1("));
    assert!(production.contains("ConfidentialSpoolLayoutV1::new_v1("));
    assert!(production.contains("SECRET_MAIN_SLOT_COUNT_V1"));
    assert!(production.contains("SECRET_MAIN_PLAINTEXT_BYTES_V1"));
    assert!(production.contains("SECRET_NONCE_SLOT_COUNT_V1"));
    assert!(production.contains("SECRET_NONCE_PLAINTEXT_BYTES_V1"));
    assert!(production.contains("live: Option<LiveRnsLinkSecretSpoolWriterV1>"));
    assert!(production.matches(".live\n            .take()").count() >= 2);
    assert!(!production.contains("pub fn"));
    for forbidden in [
        "Vec<", "Box<", "path_v1", "key_v1", "file_v1", "codec", "impl Fn", "dyn Fn", "serde",
        "Norito",
    ] {
        assert!(
            !production.contains(forbidden),
            "forbidden surface: {forbidden}"
        );
    }
}

#[test]
fn canonical_source_layouts_validate_exact_tuples_without_filesystem_effects() {
    let (main, nonce) = canonical_source_layouts_v1([0x31; 32], [0x32; 32]).unwrap();
    assert_eq!(main.slot_count_v1(), 38_528);
    assert_eq!(main.plaintext_len_v1(), 8_192);
    assert_eq!(main.ciphertext_record_len_v1(), 8_208);
    assert_eq!(main.file_len_v1(), 316_237_824);
    assert_eq!(nonce.slot_count_v1(), 43);
    assert_eq!(nonce.plaintext_len_v1(), 32);
    assert_eq!(nonce.ciphertext_record_len_v1(), 48);
    assert_eq!(nonce.file_len_v1(), 2_064);
    assert_ne!(main, nonce);
    let changed = canonical_source_layouts_v1([0x33; 32], [0x32; 32]).unwrap();
    assert_ne!(main, changed.0);
    assert_eq!(nonce, changed.1);
    for (main_context, nonce_context) in [([0; 32], [0x32; 32]), ([0x31; 32], [0; 32])] {
        assert_eq!(
            canonical_source_layouts_v1(main_context, nonce_context),
            Err(ZkAmsMkheErrorV1::InvalidPhase23Fold)
        );
    }
}
