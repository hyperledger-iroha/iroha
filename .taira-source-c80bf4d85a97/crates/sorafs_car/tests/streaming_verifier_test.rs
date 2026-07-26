//! Integration tests for the SoraFS CAR streaming verifier.

use sorafs_car::{
    CarBuildPlan, compute_chunk_plan_digest_sha3, compute_por_root,
    sorafs_chunker::ChunkProfile,
    streaming_verifier::{StreamingCarVerifier, StreamingVerifierConfig},
    verifier::CarVerifyError,
};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, DagCodecId, GovernanceProofs, ManifestBuilder, ManifestV1,
    PinPolicy, StorageClass,
};

const CARV2_PRAGMA_LEN: usize = 11;
const DATA_OFFSET_FIELD: usize = CARV2_PRAGMA_LEN + 16;
const DATA_SIZE_FIELD: usize = CARV2_PRAGMA_LEN + 24;
const INDEX_OFFSET_FIELD: usize = CARV2_PRAGMA_LEN + 32;

fn sample_payload() -> Vec<u8> {
    let total_bytes = 512 * 1024;
    let mut payload = Vec::with_capacity(total_bytes);
    for idx in 0..total_bytes {
        payload.push((idx % 251) as u8);
    }
    payload
}

fn build_manifest(
    payload: &[u8],
    plan: &CarBuildPlan,
    stats: &sorafs_car::CarWriteStats,
) -> ManifestV1 {
    let mut car_digest = [0u8; 32];
    car_digest.copy_from_slice(stats.car_archive_digest.as_bytes());
    ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, BLAKE3_256_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(compute_por_root(payload, plan).expect("derive canonical fixture PoR root"))
        .content_length(plan.content_length)
        .car_digest(car_digest)
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 1,
        })
        .governance(GovernanceProofs::default())
        .build()
        .expect("manifest")
}

fn build_valid_car() -> (Vec<u8>, ManifestV1) {
    let payload = sample_payload();
    let plan =
        CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT).expect("plan");
    let mut car_bytes = Vec::new();
    let stats = sorafs_car::CarWriter::new(&plan, &payload)
        .expect("writer")
        .write_to(&mut car_bytes)
        .expect("write car");
    let manifest = build_manifest(&payload, &plan, &stats);
    (car_bytes, manifest)
}

fn read_u64_le(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(
        bytes[offset..offset + 8]
            .try_into()
            .expect("fixed-width header field"),
    )
}

fn write_u64_le(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn decode_uleb(bytes: &[u8]) -> (u64, usize) {
    let mut value = 0u64;
    for (index, byte) in bytes.iter().copied().enumerate() {
        value |= u64::from(byte & 0x7f) << (index * 7);
        if byte & 0x80 == 0 {
            return (value, index + 1);
        }
    }
    panic!("test fixture contains a truncated ULEB128 value")
}

fn encode_uleb(mut value: u64) -> Vec<u8> {
    let mut encoded = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        encoded.push(byte);
        if value == 0 {
            return encoded;
        }
    }
}

fn refresh_manifest_archive_fields(manifest: &mut ManifestV1, car_bytes: &[u8]) {
    manifest.car_size = car_bytes.len() as u64;
    manifest
        .car_digest
        .copy_from_slice(blake3::hash(car_bytes).as_bytes());
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
    let manifest = build_manifest(&payload, &plan, &stats);

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
fn streaming_verifier_consumes_index_when_boundary_splits_update() {
    let (car_bytes, manifest) = build_valid_car();
    let index_offset = read_u64_le(&car_bytes, INDEX_OFFSET_FIELD) as usize;
    let split = index_offset
        .checked_sub(1)
        .expect("valid CAR data region should be non-empty");

    let mut verifier = StreamingCarVerifier::new(manifest, StreamingVerifierConfig::default());
    assert_eq!(
        verifier.update(&car_bytes[..split]).expect("first update"),
        split
    );
    assert_eq!(
        verifier.update(&car_bytes[split..]).expect("second update"),
        car_bytes.len() - split
    );
    verifier.finalize().expect("finalize");
}

#[test]
fn streaming_verifier_leaves_bytes_after_exact_archive_unconsumed() {
    let (mut car_bytes, manifest) = build_valid_car();
    let car_len = car_bytes.len();
    car_bytes.extend_from_slice(b"next-protocol-frame");

    let mut verifier = StreamingCarVerifier::new(manifest, StreamingVerifierConfig::default());
    assert_eq!(
        verifier.update(&car_bytes).expect("verify exact archive"),
        car_len,
        "stream verifier must not absorb bytes belonging to the next frame"
    );
    verifier.finalize().expect("finalize exact archive");
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
    let manifest = build_manifest(&payload, &plan, &stats);

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
    let mut manifest = build_manifest(&payload, &plan, &stats);
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
    let mut manifest = build_manifest(&payload, &plan, &stats);
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
    let mut manifest = build_manifest(&payload, &plan, &stats);
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
    let manifest = build_manifest(&payload, &plan, &stats);

    let config = StreamingVerifierConfig { max_chunk_size: 1 };
    let mut verifier = StreamingCarVerifier::new(manifest, config);
    let result = verifier.update(&car_bytes);
    assert!(matches!(
        result,
        Err(CarVerifyError::ChunkSizeExceeded { .. })
    ));
}

#[test]
fn streaming_verifier_enforces_manifest_chunk_ceiling() {
    let (car_bytes, mut manifest) = build_valid_car();
    manifest.chunking.max_size = 1;

    let mut verifier = StreamingCarVerifier::new(
        manifest,
        StreamingVerifierConfig {
            max_chunk_size: usize::MAX,
        },
    );
    assert!(matches!(
        verifier.update(&car_bytes),
        Err(CarVerifyError::ChunkSizeExceeded { max: 1, .. })
    ));
}

#[test]
fn streaming_verifier_bounds_incomplete_cid_buffer() {
    let (car_bytes, mut manifest) = build_valid_car();
    let data_offset = read_u64_le(&car_bytes, DATA_OFFSET_FIELD) as usize;
    let (header_len, header_len_bytes) = decode_uleb(&car_bytes[data_offset..]);
    let first_section = data_offset + header_len_bytes + header_len as usize;
    let declared_section_len = 10_000u64;
    let section_len = encode_uleb(declared_section_len);
    let declared_data_size = u64::try_from(first_section - data_offset)
        .expect("header span fits u64")
        .checked_add(section_len.len() as u64)
        .and_then(|value| value.checked_add(declared_section_len))
        .expect("declared data size");

    let mut forged = car_bytes[..first_section].to_vec();
    write_u64_le(&mut forged, DATA_SIZE_FIELD, declared_data_size);
    write_u64_le(
        &mut forged,
        INDEX_OFFSET_FIELD,
        (data_offset as u64) + declared_data_size,
    );
    forged.extend_from_slice(&section_len);
    forged.extend(std::iter::repeat_n(0x80, 128));
    manifest.car_size = (data_offset as u64) + declared_data_size;

    let mut verifier = StreamingCarVerifier::new(manifest, StreamingVerifierConfig::default());
    assert!(matches!(
        verifier.update(&forged),
        Err(CarVerifyError::TruncatedCid { section_index: 0 })
    ));
}

#[test]
fn streaming_verifier_bounds_dag_section_buffering() {
    let payload = [0x5a];
    let profile = ChunkProfile {
        min_size: 1,
        target_size: 1,
        max_size: 1,
        break_mask: 1,
    };
    let plan = CarBuildPlan::single_file_with_profile(&payload, profile).expect("plan");
    let mut car_bytes = Vec::new();
    let stats = sorafs_car::CarWriter::new(&plan, &payload)
        .expect("writer")
        .write_to(&mut car_bytes)
        .expect("write car");
    let manifest = build_manifest(&payload, &plan, &stats);

    let config = StreamingVerifierConfig { max_chunk_size: 1 };
    let mut verifier = StreamingCarVerifier::new(manifest, config);
    let result = verifier.update(&car_bytes);
    assert!(matches!(
        result,
        Err(CarVerifyError::ChunkSizeExceeded {
            section_index: 1,
            ..
        })
    ));
}

#[test]
fn streaming_verifier_rejects_zero_length_section_with_matching_manifest_digest() {
    let (mut car_bytes, mut manifest) = build_valid_car();
    let data_size = read_u64_le(&car_bytes, DATA_SIZE_FIELD);
    let index_offset = read_u64_le(&car_bytes, INDEX_OFFSET_FIELD);

    car_bytes.insert(index_offset as usize, 0);
    write_u64_le(&mut car_bytes, DATA_SIZE_FIELD, data_size + 1);
    write_u64_le(&mut car_bytes, INDEX_OFFSET_FIELD, index_offset + 1);
    refresh_manifest_archive_fields(&mut manifest, &car_bytes);

    let mut verifier = StreamingCarVerifier::new(manifest, StreamingVerifierConfig::default());
    let result = verifier.update(&car_bytes);
    assert!(matches!(
        result,
        Err(CarVerifyError::TruncatedSection { .. })
    ));
}

#[test]
fn streaming_verifier_rejects_header_outside_declared_data_region() {
    let (mut car_bytes, mut manifest) = build_valid_car();
    let data_offset = read_u64_le(&car_bytes, DATA_OFFSET_FIELD);

    write_u64_le(&mut car_bytes, DATA_SIZE_FIELD, 0);
    write_u64_le(&mut car_bytes, INDEX_OFFSET_FIELD, data_offset);
    refresh_manifest_archive_fields(&mut manifest, &car_bytes);

    let mut verifier = StreamingCarVerifier::new(manifest, StreamingVerifierConfig::default());
    let result = verifier.update(&car_bytes);
    assert!(matches!(result, Err(CarVerifyError::HeaderTruncated)));
}
