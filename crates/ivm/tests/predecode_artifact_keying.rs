use ivm::{ProgramMetadata, ivm_cache::IvmCache};

#[test]
fn artifact_header_version_in_cache_key() {
    // Same code (HALT) under two different headers: (1,0) vs (1,1)
    let code = ivm::encoding::wide::encode_halt().to_le_bytes().to_vec();
    let mut a10 = ProgramMetadata {
        version_major: 1,
        version_minor: 0,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    }
    .encode();
    a10.extend_from_slice(&code);
    let mut a11 = ProgramMetadata {
        version_minor: 1,
        ..ProgramMetadata::default_for(1, 0, 0)
    }
    .encode();
    a11.extend_from_slice(&code);

    let mut cache = IvmCache::new(8);
    let (_m10, d10) = cache.get_or_predecode_artifact(&a10).expect("decode a10");
    let (_m11, d11) = cache.get_or_predecode_artifact(&a11).expect("decode a11");
    // Must be distinct entries because version_minor differs
    assert!(!std::sync::Arc::ptr_eq(&d10, &d11));
}
