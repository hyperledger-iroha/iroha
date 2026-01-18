use sorafs_car::{
    CarBuildPlan,
    sorafs_chunker::ChunkProfile,
    streaming_verifier::{StreamingCarVerifier, StreamingVerifierConfig},
    verifier::CarVerifyError,
};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, DagCodecId, GovernanceProofs, ManifestBuilder, ManifestV1,
    PinPolicy, StorageClass,
};

fn sample_payload() -> Vec<u8> {
    let total_bytes = 512 * 1024;
    let mut payload = Vec::with_capacity(total_bytes);
    for idx in 0..total_bytes {
        payload.push((idx % 251) as u8);
    }
    payload
}

fn build_manifest(plan: &CarBuildPlan, stats: &sorafs_car::CarWriteStats) -> ManifestV1 {
    let mut car_digest = [0u8; 32];
    car_digest.copy_from_slice(stats.car_archive_digest.as_bytes());
    ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(car_digest)
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest")
}

#[test]
fn streaming_verifier_consumes_valid_car() {
    let payload = sample_payload();
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let mut car_bytes = Vec::new();
    let stats = sorafs_car::CarWriter::new(&plan, &payload)
        .expect("writer")
        .write_to(&mut car_bytes)
        .expect("write car");
    let manifest = build_manifest(&plan, &stats);

    let mut verifier = StreamingCarVerifier::new(manifest, StreamingVerifierConfig::default());

    // Feed in chunks
    let chunk_size = 1024;
    for chunk in car_bytes.chunks(chunk_size) {
        let consumed = verifier.update(chunk).expect("update");
        assert_eq!(consumed, chunk.len());
    }

    verifier.finalize().expect("finalize");
}

#[test]
fn streaming_verifier_detects_corruption() {
    let payload = sample_payload();
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let mut car_bytes = Vec::new();
    let stats = sorafs_car::CarWriter::new(&plan, &payload)
        .expect("writer")
        .write_to(&mut car_bytes)
        .expect("write car");
    let manifest = build_manifest(&plan, &stats);

    let mut verifier = StreamingCarVerifier::new(manifest, StreamingVerifierConfig::default());

    // Corrupt a byte in the payload (after header)
    // Header length is variable but > 50 bytes.
    let corrupt_idx = 200;
    car_bytes[corrupt_idx] ^= 0xFF;

    let chunk_size = 1024;
    let mut error_found = false;
    for chunk in car_bytes.chunks(chunk_size) {
        if verifier.update(chunk).is_err() {
            error_found = true;
            break;
        }
    }

    assert!(error_found || verifier.finalize().is_err());
}

#[test]
fn streaming_verifier_rejects_root_mismatch() {
    let payload = sample_payload();
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let mut car_bytes = Vec::new();
    let stats = sorafs_car::CarWriter::new(&plan, &payload)
        .expect("writer")
        .write_to(&mut car_bytes)
        .expect("write car");
    let mut manifest = build_manifest(&plan, &stats);
    manifest.root_cid = vec![0u8; manifest.root_cid.len()];

    let mut verifier = StreamingCarVerifier::new(manifest, StreamingVerifierConfig::default());
    let result = verifier.update(&car_bytes);
    assert!(matches!(result, Err(CarVerifyError::ManifestRootMismatch)));
}

#[test]
fn streaming_verifier_rejects_car_size_mismatch() {
    let payload = sample_payload();
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let mut car_bytes = Vec::new();
    let stats = sorafs_car::CarWriter::new(&plan, &payload)
        .expect("writer")
        .write_to(&mut car_bytes)
        .expect("write car");
    let mut manifest = build_manifest(&plan, &stats);
    manifest.car_size += 1;

    let mut verifier = StreamingCarVerifier::new(manifest, StreamingVerifierConfig::default());

    let chunk_size = 1024;
    for chunk in car_bytes.chunks(chunk_size) {
        let consumed = verifier.update(chunk).expect("update");
        assert_eq!(consumed, chunk.len());
    }

    let result = verifier.finalize();
    assert!(matches!(
        result,
        Err(CarVerifyError::ManifestCarSizeMismatch { .. })
    ));
}

#[test]
fn streaming_verifier_rejects_content_length_mismatch() {
    let payload = sample_payload();
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let mut car_bytes = Vec::new();
    let stats = sorafs_car::CarWriter::new(&plan, &payload)
        .expect("writer")
        .write_to(&mut car_bytes)
        .expect("write car");
    let mut manifest = build_manifest(&plan, &stats);
    manifest.content_length = manifest.content_length.saturating_add(1);

    let mut verifier = StreamingCarVerifier::new(manifest, StreamingVerifierConfig::default());

    let chunk_size = 1024;
    for chunk in car_bytes.chunks(chunk_size) {
        let consumed = verifier.update(chunk).expect("update");
        assert_eq!(consumed, chunk.len());
    }

    let result = verifier.finalize();
    assert!(matches!(
        result,
        Err(CarVerifyError::ManifestContentLengthMismatch { .. })
    ));
}

#[test]
fn streaming_verifier_enforces_chunk_size_limit() {
    let payload = sample_payload();
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let mut car_bytes = Vec::new();
    let stats = sorafs_car::CarWriter::new(&plan, &payload)
        .expect("writer")
        .write_to(&mut car_bytes)
        .expect("write car");
    let manifest = build_manifest(&plan, &stats);

    let config = StreamingVerifierConfig { max_chunk_size: 1 };
    let mut verifier = StreamingCarVerifier::new(manifest, config);
    let result = verifier.update(&car_bytes);
    assert!(matches!(
        result,
        Err(CarVerifyError::ChunkSizeExceeded { .. })
    ));
}
