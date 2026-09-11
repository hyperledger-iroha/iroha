use super::*;
use blake3;
use sorafs_car::{CarPlanError, CarWriter, FileEntry, compute_chunk_plan_digest_sha3};
use sorafs_manifest::{DagCodecId, ManifestBuilder, PinPolicy};
use std::{
    fs,
    io::{self, Cursor, Read},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use tempfile::TempDir;
// Keep one target-gated assertion for every ABI branch. Overlapping branches
// fail with duplicate definitions; missing branches fail to resolve the flag.
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
#[test]
fn linux_directory_open_flags_match_low_flag_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x8000);
    assert_eq!(platform_directory_only_flag(), 0x4000);
}
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
#[test]
fn linux_directory_open_flags_match_generic_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x20000);
    assert_eq!(platform_directory_only_flag(), 0x10000);
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "aarch64", target_arch = "arm")
))]
#[test]
fn android_arm_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x8000);
    assert_eq!(platform_directory_only_flag(), 0x4000);
}
#[cfg(all(
    target_os = "android",
    any(target_arch = "x86", target_arch = "x86_64")
))]
#[test]
fn android_x86_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x20000);
    assert_eq!(platform_directory_only_flag(), 0x10000);
}
#[cfg(all(target_os = "android", target_arch = "riscv64"))]
#[test]
fn android_riscv64_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x400000);
    assert_eq!(platform_directory_only_flag(), 0x200000);
}
#[cfg(all(
    target_os = "linux",
    any(target_arch = "riscv32", target_arch = "riscv64")
))]
#[test]
fn linux_riscv_directory_open_flags_remain_generic_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x20000);
    assert_eq!(platform_directory_only_flag(), 0x10000);
}
#[cfg(target_os = "macos")]
#[test]
fn macos_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x2000_0000);
    assert_eq!(platform_directory_only_flag(), 0x0010_0000);
}
#[cfg(target_os = "ios")]
#[test]
fn ios_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
    assert_eq!(platform_directory_only_flag(), 0x0010_0000);
}
#[cfg(target_os = "freebsd")]
#[test]
fn freebsd_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
    assert_eq!(platform_directory_only_flag(), 0x0002_0000);
}
#[cfg(target_os = "dragonfly")]
#[test]
fn dragonfly_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
    assert_eq!(platform_directory_only_flag(), 0x0800_0000);
}
#[cfg(target_os = "openbsd")]
#[test]
fn openbsd_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
    assert_eq!(platform_directory_only_flag(), 0x0002_0000);
}
#[cfg(target_os = "netbsd")]
#[test]
fn netbsd_directory_open_flags_match_target_abi() {
    assert_eq!(platform_no_follow_flag(), 0x100);
    assert_eq!(platform_directory_only_flag(), 0x0020_0000);
}
fn temp_config(temp_dir: &TempDir) -> StorageConfig {
    let temp_path = temp_dir.path().canonicalize().expect("canonical tempdir");
    StorageConfig::builder()
        .enabled(true)
        .data_dir(temp_path.join("storage"))
        .build()
}
fn temp_config_with_pdp_limit(temp_dir: &TempDir, limit: u64) -> StorageConfig {
    let temp_path = temp_dir.path().canonicalize().expect("canonical tempdir");
    StorageConfig::builder()
        .enabled(true)
        .data_dir(temp_path.join("storage"))
        .pdp_tree_memory_limit_bytes(iroha_config::base::util::Bytes(limit))
        .build()
}
fn canonical_temp_path(temp_dir: &TempDir) -> PathBuf {
    temp_dir.path().canonicalize().expect("canonical tempdir")
}
fn single_file_plan(bytes: &[u8]) -> Result<CarBuildPlan, CarPlanError> {
    CarBuildPlan::single_file(bytes)
}
fn manifest_builder_for_plan(payload: &[u8], plan: &CarBuildPlan) -> ManifestBuilder {
    let heap_limit = plan
        .validate()
        .expect("valid manifest fixture plan")
        .estimated_ingest_heap_bytes()
        .max(1);
    let mut chunk_store = ChunkStore::with_profile_and_heap_limit(plan.chunk_profile, heap_limit)
        .expect("bounded manifest fixture chunk store");
    chunk_store
        .ingest_plan(payload, plan)
        .expect("manifest fixture payload matches plan");
    let car_stats = CarWriter::new(plan, payload)
        .expect("prepare canonical fixture CAR")
        .write_to(io::sink())
        .expect("compute canonical fixture CAR");
    ManifestBuilder::new()
        .root_cid(
            car_stats
                .root_cids
                .first()
                .cloned()
                .expect("fixture CAR root"),
        )
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(
            plan.chunk_profile,
            sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
        )
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(*chunk_store.por_tree().root())
        .content_length(plan.content_length)
        .car_digest(*car_stats.car_archive_digest.as_bytes())
        .car_size(car_stats.car_size)
}
fn empty_file_plan() -> CarBuildPlan {
    let plan = CarBuildPlan {
        chunk_profile: ChunkProfile::DEFAULT,
        payload_digest: blake3::hash(&[]),
        content_length: 0,
        chunks: Vec::new(),
        files: vec![FilePlan {
            path: Vec::new(),
            first_chunk: 0,
            chunk_count: 0,
            size: 0,
        }],
    };
    plan.validate().expect("canonical empty plan");
    plan
}
fn test_manifest(payload: &[u8], plan: &CarBuildPlan, fixture_id_byte: u8) -> ManifestV1 {
    manifest_builder_for_plan(payload, plan)
        .add_metadata("test.fixture_id", fixture_id_byte.to_string())
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest")
}
fn ingest_test_payload(
    temp_dir: &TempDir,
    payload: &[u8],
    root_byte: u8,
) -> (StorageConfig, StorageBackend, String) {
    let config = temp_config(temp_dir);
    let backend = StorageBackend::new(config.clone()).expect("backend init");
    let plan = single_file_plan(payload).expect("plan");
    let manifest = test_manifest(payload, &plan, root_byte);
    let mut reader = payload;
    let manifest_id = backend
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest");
    (config, backend, manifest_id)
}
fn rewrite_manifest_record(
    backend: &StorageBackend,
    manifest_id: &str,
    mutate: impl FnOnce(&mut StoredManifestRecord),
) {
    let metadata_path = backend
        .manifests_dir
        .join(manifest_id)
        .join(METADATA_FILE_NAME);
    let bytes = fs::read(&metadata_path).expect("read manifest metadata");
    let mut record: StoredManifestRecord =
        norito::decode_from_bytes(&bytes).expect("decode manifest metadata");
    mutate(&mut record);
    fs::write(
        &metadata_path,
        norito::to_bytes(&record).expect("encode manifest metadata"),
    )
    .expect("rewrite manifest metadata");
}
fn rewrite_manifest_index(backend: &StorageBackend, mutate: impl FnOnce(&mut ManifestIndex)) {
    let bytes = fs::read(&backend.index_path).expect("read manifest index");
    let mut index: ManifestIndex =
        norito::decode_from_bytes(&bytes).expect("decode manifest index");
    mutate(&mut index);
    fs::write(
        &backend.index_path,
        norito::to_bytes(&index).expect("encode manifest index"),
    )
    .expect("rewrite manifest index");
}
fn first_pdp_sample() -> Vec<PdpSampleV1> {
    vec![PdpSampleV1 {
        segment_index: 0,
        hot_leaf_indices: vec![0],
    }]
}
fn replace_with_empty_index(config: &StorageConfig) {
    let index_path = config.data_dir().join("index.norito");
    let bytes = norito::to_bytes(&ManifestIndex::default()).expect("encode empty index");
    write_atomic(&index_path, &bytes).expect("replace storage index");
}
struct GatedReader {
    bytes: Cursor<Vec<u8>>,
    entered: Option<mpsc::Sender<()>>,
    release: mpsc::Receiver<()>,
    fail_after_release: bool,
}
impl Read for GatedReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if let Some(entered) = self.entered.take() {
            entered
                .send(())
                .map_err(|_| io::Error::other("test gate receiver dropped"))?;
            self.release
                .recv()
                .map_err(|_| io::Error::other("test gate sender dropped"))?;
            if self.fail_after_release {
                return Err(io::Error::other("injected ingest reader failure"));
            }
        }
        self.bytes.read(buffer)
    }
}
fn assert_staging_empty(backend: &StorageBackend) {
    let staging_root = backend.root_dir().join(INGEST_STAGING_DIR_NAME);
    if staging_root.exists() {
        assert!(
            fs::read_dir(&staging_root)
                .expect("read staging root")
                .next()
                .is_none(),
            "ingest staging root must not retain attempt directories"
        );
    }
}
#[test]
fn storage_directory_has_single_process_owner() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let config = temp_config(&temp_dir);
    let owner = StorageBackend::new(config.clone()).expect("acquire storage ownership");
    assert!(matches!(
        StorageBackend::new(config.clone()),
        Err(StorageError::StorageDirectoryInUse { .. })
    ));
    drop(owner);
    StorageBackend::new(config).expect("storage ownership releases on drop");
}
#[cfg(unix)]
#[test]
fn storage_lock_rejects_symlink() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let config = temp_config(&temp_dir);
    fs::create_dir_all(config.data_dir()).expect("create storage root");
    let target = temp_dir.path().join("lock-target");
    fs::write(&target, b"must remain untouched").expect("write lock target");
    std::os::unix::fs::symlink(&target, config.data_dir().join(STORAGE_LOCK_FILE_NAME))
        .expect("create storage lock symlink");
    assert!(matches!(
        StorageBackend::new(config),
        Err(StorageError::Io(_))
    ));
    assert_eq!(
        fs::read(&target).expect("read lock target"),
        b"must remain untouched"
    );
}
#[cfg(unix)]
#[test]
fn storage_lock_rejects_hard_link() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let config = temp_config(&temp_dir);
    fs::create_dir_all(config.data_dir()).expect("create storage root");
    let target = temp_dir.path().join("lock-target");
    fs::write(&target, b"must remain untouched").expect("write lock target");
    fs::hard_link(&target, config.data_dir().join(STORAGE_LOCK_FILE_NAME))
        .expect("create storage lock hard link");
    assert!(matches!(
        StorageBackend::new(config),
        Err(StorageError::CorruptStorageState { .. })
    ));
    assert_eq!(
        fs::read(&target).expect("read target"),
        b"must remain untouched"
    );
}
