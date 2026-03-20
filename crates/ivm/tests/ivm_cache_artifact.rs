use ivm::{IvmCache, ProgramMetadata, encoding};

#[test]
fn artifact_predecode_uses_header_version() {
    let mut cache = IvmCache::new(2);
    let meta_v1 = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    // Code: single HALT32
    let code = encoding::wide::encode_halt().to_le_bytes();
    let mut artifact = meta_v1.encode();
    artifact.extend_from_slice(&code);

    let (m1, d1) = cache
        .get_or_predecode_artifact(&artifact)
        .expect("decode artifact");
    assert_eq!(m1.version_major, 1);
    assert_eq!(m1.version_minor, 1);
    assert_eq!(d1.len(), 1);

    let (_m2, d2) = cache
        .get_or_predecode_artifact(&artifact)
        .expect("decode artifact again");
    assert!(std::sync::Arc::ptr_eq(&d1, &d2));
}
