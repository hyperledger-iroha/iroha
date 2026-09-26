//! Payload-independent canonical manifest layout validation.

use super::*;
use sorafs_manifest::{DagCodecId, ManifestBuilder, PinPolicy};

fn native_metadata_fixture() -> (CarBuildPlan, Vec<u8>, sorafs_manifest::ManifestV1) {
    let (plan, payload) = CarBuildPlan::from_files_with_profile(
        vec![
            FileEntry {
                path: vec!["a".into()],
                data: b"same chunk".to_vec(),
            },
            FileEntry {
                path: vec!["b".into()],
                data: b"same chunk".to_vec(),
            },
        ],
        ChunkProfile::DEFAULT,
    )
    .unwrap();
    let stats = CarWriter::new(&plan, &payload)
        .unwrap()
        .write_to(io::sink())
        .unwrap();
    let mut chunks = ChunkStore::new();
    chunks.ingest_plan(&payload, &plan).unwrap();
    let manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(
            plan.chunk_profile,
            sorafs_manifest::BLAKE3_256_MULTIHASH_CODE,
        )
        .chunk_digest_sha3_256(compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(*chunks.por_tree().root())
        .content_length(plan.content_length)
        .car_digest(*stats.car_archive_digest.as_bytes())
        .car_size(stats.car_size)
        .pin_policy(PinPolicy::default())
        .build()
        .unwrap();
    (plan, payload, manifest)
}

#[test]
fn metadata_validation_binds_directory_layout_without_payload_io() {
    let (plan, _, manifest) = native_metadata_fixture();
    plan.verify_manifest_metadata(&manifest).unwrap();
    let mut renamed = plan.clone();
    renamed.files[1].path[0] = "c".into();
    assert!(matches!(
        renamed.verify_manifest_metadata(&manifest),
        Err(CarWriteError::RootMismatch)
    ));
    for field in [
        "version",
        "content_length",
        "chunking",
        "chunk_digest_sha3_256",
        "dag_codec",
        "car_size",
    ] {
        let mut changed = manifest.clone();
        match field {
            "version" => changed.version += 1,
            "content_length" => changed.content_length += 1,
            "chunking" => changed.chunking.multihash_code ^= 1,
            "chunk_digest_sha3_256" => changed.chunk_digest_sha3_256[0] ^= 1,
            "dag_codec" => changed.dag_codec.0 ^= 1,
            "car_size" => changed.car_size += 1,
            _ => unreachable!(),
        }
        assert!(
            matches!(plan.verify_manifest_metadata(&changed), Err(CarWriteError::ManifestMetadataMismatch { field: actual }) if actual == field)
        );
    }
}

#[test]
fn publisher_source_metadata_roundtrip_rejects_substituted_paths_and_frames() {
    use crate::publisher::{
        PublisherSourceChunkRequestV1, PublisherSourceHeaderV1, PublisherSourceRequestV1,
        PublisherSourceUploadV1,
    };
    let (plan, payload, manifest) = native_metadata_fixture();
    let header = PublisherSourceHeaderV1::new([1; 32], [2; 32], 1, &manifest, &plan).unwrap();
    let bytes = norito::to_bytes(&header).unwrap();
    let decoded = PublisherSourceHeaderV1::decode(&bytes).unwrap();
    assert_eq!(decoded, header);
    assert_eq!(decoded.verify().unwrap(), (manifest.clone(), plan.clone()));
    let mut trailing = bytes;
    trailing.push(0);
    assert!(PublisherSourceHeaderV1::decode(&trailing).is_err());
    let mut renamed = header.clone();
    renamed.files[0].path[0] = "substitute".into();
    assert!(renamed.verify().is_err());
    let digest = header.canonical_digest().unwrap();
    assert_eq!(
        digest,
        *blake3::hash(&norito::encode_canonical(&header).unwrap()).as_bytes()
    );
    let mut changed_revision = header.clone();
    changed_revision.assignment_revision += 1;
    assert_ne!(changed_revision.canonical_digest().unwrap(), digest);
    let chunk = PublisherSourceChunkRequestV1 {
        provider_id: header.provider_id,
        order_id: header.order_id,
        assignment_revision: header.assignment_revision,
        manifest_digest: *manifest.digest().unwrap().as_bytes(),
        header_digest: digest,
        upload: PublisherSourceUploadV1 {
            index: 0,
            bytes: payload[..plan.chunks[0].length as usize].to_vec(),
        },
    };
    for request in [
        PublisherSourceRequestV1::Metadata(header),
        PublisherSourceRequestV1::Chunk(chunk.clone()),
    ] {
        let encoded = norito::encode_canonical(&request).unwrap();
        assert_eq!(PublisherSourceRequestV1::decode(&encoded).unwrap(), request);
        if matches!(request, PublisherSourceRequestV1::Chunk(_)) {
            assert!(
                encoded.len() < chunk.upload.bytes.len() + 512,
                "chunk uploads must not repeat the manifest or file/chunk inventories"
            );
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert!(PublisherSourceRequestV1::decode(&trailing).is_err());
    }
    for field in [
        "provider", "order", "revision", "manifest", "header", "empty", "oversize",
    ] {
        let mut invalid = chunk.clone();
        match field {
            "provider" => invalid.provider_id = [0; 32],
            "order" => invalid.order_id = [0; 32],
            "revision" => invalid.assignment_revision = 0,
            "manifest" => invalid.manifest_digest = [0; 32],
            "header" => invalid.header_digest = [0; 32],
            "empty" => invalid.upload.bytes.clear(),
            "oversize" => {
                invalid.upload.bytes = vec![0; crate::CHUNK_STORE_MAX_CHUNK_BYTES as usize + 1]
            }
            _ => unreachable!(),
        }
        assert!(
            PublisherSourceRequestV1::decode(
                &norito::encode_canonical(&PublisherSourceRequestV1::Chunk(invalid)).unwrap()
            )
            .is_err(),
            "invalid {field}"
        );
    }
}
