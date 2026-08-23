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
    assert!(production.contains("phase23_rns_link_secret_main_v1"));
    assert!(production.contains("phase23_rns_link_secret_nonce_v1"));
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
