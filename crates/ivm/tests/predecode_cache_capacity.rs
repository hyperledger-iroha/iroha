use ivm::{encoding, instruction, ivm_cache};

fn code32(word: u32) -> Vec<u8> {
    word.to_le_bytes().to_vec()
}

#[test]
fn global_capacity_eviction_and_runtime_resize() {
    // Initialize a small global cache
    ivm_cache::init_global_with_capacity(2);
    // Reset counters by reading them (monotonic; we only assert deltas >= baseline)
    let (_h0, _m0, _e0) = ivm_cache::global_counters();

    // Three distinct short streams
    let s1 = code32(encoding::wide::encode_rr(
        instruction::wide::arithmetic::ADD,
        1,
        0,
        1,
    ));
    let s2 = code32(encoding::wide::encode_rr(
        instruction::wide::arithmetic::SUB,
        2,
        1,
        0,
    ));
    let s3 = code32(encoding::wide::encode_rr(
        instruction::wide::arithmetic::XOR,
        3,
        2,
        1,
    ));

    // Use minimal metadata (vmaj/vmin) implied by get_or_predecode with meta in helper
    let meta = ivm::ProgramMetadata {
        version_major: 1,
        version_minor: 1,
        mode: 0,
        vector_length: 0,
        max_cycles: 0,
        abi_version: 1,
    };

    // Insert s1, s2
    let _ = ivm_cache::global_get_with_meta(&s1, &meta).expect("decode s1");
    let _ = ivm_cache::global_get_with_meta(&s2, &meta).expect("decode s2");
    // Access s1 to make it MRU
    let _ = ivm_cache::global_get_with_meta(&s1, &meta).expect("hit s1");
    // Insert s3: should evict s2 (LRU)
    let (_h1, _m1, e1) = ivm_cache::global_counters();
    let _ = ivm_cache::global_get_with_meta(&s3, &meta).expect("decode s3");
    let (_h2, _m2, e2) = ivm_cache::global_counters();
    assert!(e2 > e1, "expected at least one eviction");

    // Resize down to capacity 1: should evict until only MRU remains
    ivm_cache::set_global_capacity(1);
    let (_h3, _m3, e3) = ivm_cache::global_counters();
    assert!(e3 > e2, "expected additional evictions on resize");

    // Access s1 again; s3 may have been evicted by resize, ensure it re-decodes (miss)
    let (h_before, m_before, _e_before) = ivm_cache::global_counters();
    let _ = ivm_cache::global_get_with_meta(&s3, &meta).expect("decode s3 after resize");
    let (h_after, m_after, _e_after) = ivm_cache::global_counters();
    assert!(m_after > m_before, "expected a miss on re-inserting s3");
    assert!(h_after >= h_before, "hits should not decrease");
}
