use ivm::{IvmCache, ProgramMetadata, encoding};

#[test]
fn artifact_predecode_uses_header_minor() {
    let mut cache = IvmCache::new(2);
    let meta_v1 = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    let meta_v1b = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };
    // Code: single HALT32
    let code = encoding::wide::encode_halt().to_le_bytes();
    let mut a1 = meta_v1.encode();
    a1.extend_from_slice(&code);
    let mut a2 = meta_v1b.encode();
    a2.extend_from_slice(&code);

    let (m1, d1) = cache.get_or_predecode_artifact(&a1).expect("decode a1");
    assert_eq!(m1.version_major, 1);
    assert_eq!(m1.version_minor, 0);
    assert_eq!(d1.len(), 1);

    let (m2, d2) = cache.get_or_predecode_artifact(&a2).expect("decode a2");
    assert_eq!(m2.version_major, 1);
    assert_eq!(m2.version_minor, 1);
    assert_eq!(d2.len(), 1);
    // Different header minor should produce distinct cache entries (not pointer-equal)
    assert!(!std::sync::Arc::ptr_eq(&d1, &d2));
}
