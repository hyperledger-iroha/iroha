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
        .root_cid(sorafs_manifest::canonical_manifest_root_cid([0xAA; 32]))
        .dag_codec(sorafs_manifest::DagCodecId(0x71))
        .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
        .chunk_digest_sha3_256([0xAC; 32])
        // Exact canonical PoR root for `sample_input()` and the asserted chunk geometry above.
        .por_root([
            0xb6, 0x2a, 0x1d, 0x56, 0xbe, 0xcc, 0x49, 0x5f, 0x94, 0xe9, 0xfb, 0xac, 0xb2, 0xaf,
            0xc7, 0xb9, 0x37, 0x40, 0x12, 0xec, 0xe2, 0xbf, 0x11, 0xc1, 0xf3, 0x75, 0x5b, 0xa2,
            0xb3, 0x91, 0x97, 0x31,
        ])
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
