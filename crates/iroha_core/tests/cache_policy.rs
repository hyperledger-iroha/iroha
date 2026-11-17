//! Pipeline cache policy acceptance tests.
//!
//! Covers:
//! - Insert/hit/eviction behavior under bounded capacity
//! - Disabled retention when capacity is zero
use iroha_config::parameters::defaults::pipeline;
use ivm::{encoding, instruction, ivm_cache};

/// Hold ordinary retention limits and a private allocation budget while each test owns its
/// cache entries and counters. Other Core tests may configure or use the process-wide cache.
fn retention_limits() -> ivm_cache::CacheLimitsGuard {
    ivm_cache::CacheLimitsGuard::new(ivm_cache::CacheLimits {
        capacity: pipeline::CACHE_SIZE,
        max_bytes: pipeline::IVM_CACHE_MAX_BYTES,
        max_decoded_ops: pipeline::IVM_CACHE_MAX_DECODED_OPS,
    })
}

fn halt32_code() -> Vec<u8> {
    encoding::wide::encode_halt().to_le_bytes().to_vec()
}
fn second_code() -> Vec<u8> {
    encoding::wide::encode_rr(instruction::wide::arithmetic::SUB, 2, 3, 4)
        .to_le_bytes()
        .to_vec()
}
fn alt_two_c_ops_code() -> Vec<u8> {
    // Use two wide RR instructions with distinct registers to guarantee a unique byte stream.
    let add = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 5, 6, 7).to_le_bytes();
    let xor = encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 8, 9, 10).to_le_bytes();
    [add.to_vec(), xor.to_vec()].concat()
}
#[test]
fn cache_insert_hit_and_eviction_under_capacity() {
    let _limits = retention_limits();
    let mut cache = ivm_cache::IvmCache::new(2);
    assert_eq!(cache.counters(), (0, 0, 0));
    let code1 = halt32_code();
    let code2 = second_code();
    // third distinct stream: use unique wide ops to ensure distinct cache key
    let code3 = alt_two_c_ops_code();
    // Insert two distinct entries (misses)
    let a1 = cache.get_or_predecode(&code1).expect("decode code1");
    let a2_first = cache.get_or_predecode(&code2).expect("decode code2");
    // Re-access code1 to mark it most-recently used (hit)
    let a1_hit = cache.get_or_predecode(&code1).expect("hit code1");
    assert!(
        ivm::cache_memory::SharedAllocation::ptr_eq(&a1, &a1_hit),
        "expected code1 to reuse its cached allocation",
    );
    // Insert third distinct entry; capacity=2 so LRU (code2) should be evicted
    let _a3 = cache.get_or_predecode(&code3).expect("decode code3");
    // Access code2 again; should be a miss and produce a new allocation (re-decoded)
    let a2_second = cache.get_or_predecode(&code2).expect("re-decode code2");
    assert!(
        !ivm::cache_memory::SharedAllocation::ptr_eq(&a2_first, &a2_second),
        "expected code2 to be evicted and re-decoded",
    );
    assert_eq!(
        cache.counters(),
        (1, 4, 2),
        "expected one hit, four misses and two LRU evictions",
    );
}
#[test]
fn zero_capacity_disables_retention() {
    let _limits = retention_limits();
    let mut cache = ivm_cache::IvmCache::new(0);
    let code1 = halt32_code();
    let code2 = second_code();
    let a1_first = cache.get_or_predecode(&code1).expect("decode code1");
    let a1_repeat = cache.get_or_predecode(&code1).expect("repeat code1");
    assert!(
        !ivm::cache_memory::SharedAllocation::ptr_eq(&a1_first, &a1_repeat),
        "zero capacity must not retain even one entry",
    );
    let _a2 = cache.get_or_predecode(&code2).expect("decode code2");
    let a1_second = cache.get_or_predecode(&code1).expect("re-decode code1");
    assert!(
        !ivm::cache_memory::SharedAllocation::ptr_eq(&a1_first, &a1_second),
        "expected code1 to be decoded anew with retention disabled",
    );
    assert_eq!(
        cache.counters(),
        (0, 4, 0),
        "disabled retention must only record misses",
    );
}
