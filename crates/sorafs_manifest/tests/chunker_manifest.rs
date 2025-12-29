use sorafs_chunker::{ChunkProfile, chunk_bytes_with_digests};
use sorafs_manifest::{
    BLAKE3_256_MULTIHASH_CODE, CouncilSignature, GovernanceProofs, ManifestBuilder, PinPolicy,
    StorageClass,
};

fn sample_input() -> Vec<u8> {
    let mut buf = Vec::with_capacity(1 << 20);
    let mut state: u64 = 0xDEC0DED;
    for _ in 0..buf.capacity() {
        state = state
            .wrapping_mul(2862933555777941757)
            .wrapping_add(3037000493);
        buf.push((state >> 32) as u8);
    }
    buf
}

#[test]
fn manifest_digest_consistent_with_chunker_fixture() {
    let input = sample_input();
    let chunks = chunk_bytes_with_digests(&input);
    assert_eq!(chunks.len(), 5, "fixture chunk count changed");
    let lengths: Vec<usize> = chunks.iter().map(|c| c.length).collect();
    assert_eq!(
        lengths,
        vec![177_082, 210_377, 403_145, 187_169, 70_803],
        "chunk lengths drifted"
    );

    let manifest = ManifestBuilder::new()
        .root_cid(vec![0x01, 0x55, 0xaa])
        .dag_codec(sorafs_manifest::DagCodecId(0x71))
        .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
        .content_length(input.len() as u64)
        .car_digest([0x42; 32])
        .car_size(1_111_111)
        .pin_policy(PinPolicy {
            min_replicas: 3,
            storage_class: StorageClass::Hot,
            retention_epoch: 0,
        })
        .governance(GovernanceProofs {
            council_signatures: vec![CouncilSignature {
                signer: [0xAB; 32],
                signature: vec![0xCD; 64],
            }],
        })
        .add_metadata("fixture", "sf1-profile-v1")
        .build()
        .expect("build manifest");

    // Pin policy matches expectation and chunking snapshot preserved.
    assert_eq!(manifest.pin_policy.min_replicas, 3);
    assert!(matches!(
        manifest.pin_policy.storage_class,
        StorageClass::Hot
    ));
    let expected = ChunkProfile::DEFAULT;
    assert_eq!(manifest.chunking.min_size, expected.min_size as u32);
    assert_eq!(manifest.chunking.target_size, expected.target_size as u32);
    assert_eq!(manifest.chunking.max_size, expected.max_size as u32);
    assert_eq!(manifest.content_length, input.len() as u64);
    assert_eq!(manifest.metadata.len(), 1);
    assert_eq!(manifest.governance.council_signatures.len(), 1);
}
