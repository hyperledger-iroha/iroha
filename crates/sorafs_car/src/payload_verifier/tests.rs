//! Cross-check streaming native verification against the canonical writer and complete PoR tree.
use super::*;
use crate::{CarChunk, CarWriter, ChunkProfile, FilePlan};
use sorafs_manifest::{DagCodecId, GovernanceProofs, ManifestBuilder, PinPolicy, StorageClass};
use std::io::Cursor;

fn fixture(chunk_count: usize) -> (CarBuildPlan, Vec<u8>, ManifestV1) {
    let size = ChunkProfile::DEFAULT.min_size;
    let payload = (0..size * chunk_count)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let plan = CarBuildPlan {
        chunk_profile: ChunkProfile::DEFAULT,
        payload_digest: blake3::hash(&payload),
        content_length: payload.len() as u64,
        chunks: payload
            .chunks(size)
            .enumerate()
            .map(|(index, chunk)| CarChunk {
                offset: (index * size) as u64,
                length: chunk.len() as u32,
                digest: blake3::hash(chunk).into(),
            })
            .collect(),
        files: vec![FilePlan {
            path: vec!["object.bin".into()],
            first_chunk: 0,
            chunk_count,
            size: payload.len() as u64,
        }],
    };
    let stats = CarWriter::new(&plan, &payload)
        .unwrap()
        .write_to(io::sink())
        .unwrap();
    let manifest = ManifestBuilder::new()
        .root_cid(stats.root_cids[0].clone())
        .dag_codec(DagCodecId(stats.dag_codec))
        .chunking_from_profile(plan.chunk_profile, crate::BLAKE3_256_MULTIHASH_CODE)
        .chunk_digest_sha3_256(crate::compute_chunk_plan_digest_sha3(&plan.chunks))
        .por_root(crate::compute_por_root(&payload, &plan).unwrap())
        .content_length(plan.content_length)
        .car_digest(stats.car_archive_digest.into())
        .car_size(stats.car_size)
        .pin_policy(PinPolicy {
            min_replicas: 1,
            storage_class: StorageClass::Hot,
            retention_epoch: 1,
        })
        .governance(GovernanceProofs::default())
        .build()
        .unwrap();
    (plan, payload, manifest)
}

#[test]
fn logarithmic_por_frontier_matches_complete_trees_and_canonical_car() {
    for count in 1..=17 {
        let (plan, payload, manifest) = fixture(count);
        let verification =
            verify_payload_reader(&manifest, &plan, &mut Cursor::new(&payload)).unwrap();
        let expected = CarWriter::new(&plan, &payload)
            .unwrap()
            .write_to(io::sink())
            .unwrap();
        assert_eq!(verification.stats, expected, "chunk count {count}");
        let mut store = crate::ChunkStore::with_profile(plan.chunk_profile);
        store.ingest_plan(&payload, &plan).unwrap();
        assert_eq!(verification.por_leaf_count, store.por_leaf_count());
    }
}

#[test]
fn streamed_verification_rejects_every_altered_commitment_and_inexact_eof() {
    let (plan, payload, manifest) = fixture(3);
    for field in 0..4 {
        let mut altered = manifest.clone();
        match field {
            0 => altered.car_digest[0] ^= 1,
            1 => altered.por_root[0] ^= 1,
            2 => altered.chunk_digest_sha3_256[0] ^= 1,
            _ => altered.root_cid[10] ^= 1,
        }
        assert!(verify_payload_reader(&altered, &plan, &mut Cursor::new(&payload)).is_err());
    }
    assert!(
        verify_payload_reader(
            &manifest,
            &plan,
            &mut Cursor::new(&payload[..payload.len() - 1])
        )
        .is_err()
    );
    let mut extra = payload.clone();
    extra.push(0);
    assert!(verify_payload_reader(&manifest, &plan, &mut Cursor::new(extra)).is_err());
    let mut corrupted = payload;
    corrupted[0] ^= 1;
    assert!(verify_payload_reader(&manifest, &plan, &mut Cursor::new(corrupted)).is_err());
}

#[test]
fn borrowed_chunk_reader_rewinds_without_assembly() {
    let chunks = vec![vec![1, 2], vec![], vec![3, 4, 5]];
    let mut reader = ChunkPayloadReader::new(&chunks);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, [1, 2, 3, 4, 5]);
    reader.rewind().unwrap();
    let mut prefix = [0; 3];
    reader.read_exact(&mut prefix).unwrap();
    assert_eq!(prefix, [1, 2, 3]);
    assert!(reader.seek(SeekFrom::Start(1)).is_err());
}
