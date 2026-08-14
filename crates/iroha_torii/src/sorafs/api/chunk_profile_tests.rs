// SoraFS chunk-profile regressions included by the parent test module.
use super::*;
use sorafs_manifest::{BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, PinPolicy};
fn canonical_fixture_car_stats(plan: &CarBuildPlan, payload: &[u8]) -> sorafs_car::CarWriteStats {
    sorafs_car::CarWriter::new(plan, payload)
        .expect("canonical fixture CAR writer")
        .write_to(std::io::sink())
        .expect("derive canonical fixture CAR archive stats")
}
#[test]
fn chunk_profile_for_manifest_accepts_inline_profile() {
    let payload = b"chunk-profile-fixture";
    let content_length = payload.len() as u64;
    let profile = sorafs_chunker::ChunkProfile {
        min_size: 8,
        target_size: 8,
        max_size: 8,
        break_mask: 1,
    };
    let plan =
        CarBuildPlan::single_file_with_profile(payload, profile).expect("canonical chunk plan");
    let car_stats = canonical_fixture_car_stats(&plan, payload);
    let manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(profile, BLAKE3_256_MULTIHASH_CODE)
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(
            sorafs_car::compute_por_root(payload, &plan)
                .expect("derive canonical inline-profile PoR root"),
        )
        .content_length(content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");
    let resolved = chunk_profile_for_manifest(&manifest).expect("inline profile accepted");
    assert_eq!(resolved, profile);
}
#[test]
fn chunk_profile_for_manifest_rejects_unknown_profile_id() {
    let payload = b"chunk-profile-fixture";
    let content_length = payload.len() as u64;
    let plan = CarBuildPlan::single_file(payload).expect("canonical chunk plan");
    let car_stats = canonical_fixture_car_stats(&plan, payload);
    let mut manifest = ManifestBuilder::new()
        .root_cid(car_stats.root_cids[0].clone())
        .dag_codec(DagCodecId(car_stats.dag_codec))
        .chunking_from_profile(
            sorafs_chunker::ChunkProfile::DEFAULT,
            BLAKE3_256_MULTIHASH_CODE,
        )
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(
            sorafs_car::compute_por_root(payload, &plan)
                .expect("derive canonical unknown-profile fixture PoR root"),
        )
        .content_length(content_length)
        .car_digest(car_stats.car_archive_digest.into())
        .car_size(car_stats.car_size)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");
    manifest.chunking.profile_id = sorafs_manifest::ProfileId(u32::MAX);
    manifest.chunking.min_size = 1024;
    manifest.chunking.target_size = 512;
    manifest.chunking.max_size = 2048;
    manifest.chunking.break_mask = 1;
    let err = chunk_profile_for_manifest(&manifest).expect_err("invalid profile should fail");
    assert_eq!(err.status(), StatusCode::BAD_REQUEST);
}
