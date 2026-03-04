//! Integration-style tests covering SoraFS pin, fetch, restart, quota, and PoR flows.

use std::path::PathBuf;

use blake3::hash;
use sorafs_car::CarBuildPlan;
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, PinPolicy};
use sorafs_node::{NodeHandle, NodeStorageError, config::StorageConfig, store::StorageError};
use tempfile::TempDir;

fn storage_config(temp_dir: &TempDir) -> StorageConfig {
    StorageConfig::builder()
        .enabled(true)
        .data_dir(temp_dir.path().join("storage"))
        .build()
}

fn build_manifest(payload: &[u8]) -> (CarBuildPlan, sorafs_manifest::ManifestV1) {
    let plan = CarBuildPlan::single_file(payload).expect("build plan");
    let digest = hash(payload);
    let manifest = ManifestBuilder::new()
        .root_cid(digest.as_bytes().to_vec())
        .dag_codec(DagCodecId(0x71))
        .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(digest.into())
        .car_size(plan.content_length)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");
    (plan, manifest)
}

fn ingest_payload(handle: &NodeHandle, payload: &[u8]) -> String {
    let (plan, manifest) = build_manifest(payload);
    let mut reader = payload;
    handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest payload")
}

#[test]
fn pin_fetch_roundtrip() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let config = storage_config(&temp_dir);
    let handle = NodeHandle::new(config);

    let payload = b"pin fetch roundtrip payload";
    let manifest_id = ingest_payload(&handle, payload);

    let fetched = handle
        .read_payload_range(&manifest_id, 0, payload.len())
        .expect("fetch payload");
    assert_eq!(fetched, payload);
}

#[test]
fn pin_survives_restart() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let data_dir: PathBuf = temp_dir.path().join("storage");
    let base_config = StorageConfig::builder()
        .enabled(true)
        .data_dir(data_dir.clone())
        .build();

    let payload = b"persist this payload across restart";
    let manifest_id = {
        let handle = NodeHandle::new(base_config.clone());
        ingest_payload(&handle, payload)
    };

    let restarted = NodeHandle::new(base_config);
    let bytes = restarted
        .read_payload_range(&manifest_id, 0, payload.len())
        .expect("fetch after restart");
    assert_eq!(bytes, payload);
}

#[test]
fn pin_quota_rejection() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let config = StorageConfig::builder()
        .enabled(true)
        .data_dir(temp_dir.path().join("storage"))
        .max_pins(1)
        .build();
    let handle = NodeHandle::new(config);

    let first_id = ingest_payload(&handle, b"first payload fits quota");
    assert!(!first_id.is_empty());

    let (plan, manifest) = build_manifest(b"second payload exceeds quota");
    let mut reader = &b"second payload exceeds quota"[..];
    let err = handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect_err("second pin should fail");

    match err {
        NodeStorageError::Storage(StorageError::PinLimitReached { limit }) => {
            assert_eq!(limit, 1);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn por_sampling_returns_verified_proofs() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let config = storage_config(&temp_dir);
    let handle = NodeHandle::new(config);

    let payload = b"SoraFS PoR sampling payload integration test";
    let manifest_id = ingest_payload(&handle, payload);

    let samples = handle
        .sample_por(&manifest_id, 4, 42)
        .expect("sample PoR proofs");
    assert!(!samples.is_empty());

    let storage = handle.storage().expect("storage backend available");
    let stored = storage
        .manifest(&manifest_id)
        .expect("stored manifest present");
    let tree = stored.por_tree();
    let root = tree.root();

    for (_index, proof) in samples {
        assert!(proof.verify(root));
    }
}

#[test]
fn chunk_read_by_digest_returns_bytes() {
    let temp_dir = tempfile::tempdir().expect("temp dir");
    let config = storage_config(&temp_dir);
    let handle = NodeHandle::new(config);

    let payload = b"chunk level retrieval payload";
    let (plan, manifest) = build_manifest(payload);
    let mut reader = &payload[..];
    let manifest_id = handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest payload");

    let chunk = plan.chunks.first().expect("at least one chunk");
    let (record, bytes) = handle
        .read_chunk_by_digest(&manifest_id, &chunk.digest)
        .expect("chunk fetch");
    assert_eq!(record.offset, chunk.offset);
    assert_eq!(record.length, chunk.length);
    assert_eq!(record.digest, chunk.digest);
    assert_eq!(bytes, payload);
}
