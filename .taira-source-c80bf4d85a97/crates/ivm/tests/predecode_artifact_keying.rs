use ivm::{ProgramMetadata, ivm_cache::IvmCache};

#[test]
fn artifact_cache_key_ignores_non_version_header_fields() {
    // Same code (HALT) under headers that differ only in mode bits.
    let code = ivm::encoding::wide::encode_halt().to_le_bytes().to_vec();
    let mut a_base = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    }
    .encode();
    a_base.extend_from_slice(&code);
    let mut a_mode = ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: ivm::ivm_mode::VECTOR,
        vector_length: 8,
        max_cycles: 0,
        abi_version: 1,
    }
    .encode();
    a_mode.extend_from_slice(&code);

    let mut cache = IvmCache::new(8);
    let (_m_base, d_base) = cache
        .get_or_predecode_artifact(&a_base)
        .expect("decode base");
    let (_m_mode, d_mode) = cache
        .get_or_predecode_artifact(&a_mode)
        .expect("decode mode");
    assert!(std::sync::Arc::ptr_eq(&d_base, &d_mode));
}
