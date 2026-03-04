//! Pipeline cache policy acceptance tests
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//!
//! Covers:
//! - Insert/hit/eviction behavior under bounded capacity
//! - Header version keying (minor/major version invalidation)
//! - Capacity clamp when configured to zero (must behave as capacity = 1)

use std::sync::{Arc, LazyLock, Mutex};

use ivm::{ProgramMetadata, encoding, instruction, ivm_cache};

static CACHE_TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn meta(vmaj: u8, vmin: u8) -> ProgramMetadata {
    ProgramMetadata {
        version_major: vmaj,
        version_minor: vmin,
        ..Default::default()
    }
}

fn halt32_code() -> Vec<u8> {
    encoding::wide::encode_halt().to_le_bytes().to_vec()
}

fn encode_r16(op: u8, rd: u8, rs1: u8, rs2: u8) -> u16 {
    u16::from(op & 0xF) << 12
        | u16::from(rd & 0xF) << 8
        | u16::from(rs1 & 0xF) << 4
        | u16::from(rs2 & 0xF)
}

fn two_c_ops_code() -> Vec<u8> {
    // [C-ADD][C-XOR] (two 16-bit instructions)
    let cadd = encode_r16(0x1, 1, 0, 1).to_le_bytes();
    let cxor = encode_r16(0x6, 2, 3, 4).to_le_bytes();
    [cadd, cxor].concat()
}

fn alt_two_c_ops_code() -> Vec<u8> {
    // Use two wide RR instructions with distinct registers to guarantee a unique byte stream.
    let add = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 5, 6, 7).to_le_bytes();
    let xor = encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 8, 9, 10).to_le_bytes();
    [add.to_vec(), xor.to_vec()].concat()
}

#[test]
fn cache_insert_hit_and_eviction_under_capacity() {
    let _lock = CACHE_TEST_LOCK.lock().unwrap();
    // Ensure a small deterministic capacity and record initial counters.
    ivm_cache::set_global_capacity(2);
    let (h0, m0, e0) = ivm_cache::global_counters();

    let code1 = halt32_code();
    let code2 = two_c_ops_code();
    // third distinct stream: use unique wide ops to ensure distinct cache key
    let code3 = alt_two_c_ops_code();

    let m = meta(2, 0);

    // Insert two distinct entries (misses)
    let _a1 = ivm_cache::global_get_with_meta(&code1, &m).expect("decode code1");
    let a2_first = ivm_cache::global_get_with_meta(&code2, &m).expect("decode code2");
    // Re-access code1 to mark it most-recently used (hit)
    let _a1_hit = ivm_cache::global_get_with_meta(&code1, &m).expect("hit code1");

    // Insert third distinct entry; capacity=2 so LRU (code2) should be evicted
    let _a3 = ivm_cache::global_get_with_meta(&code3, &m).expect("decode code3");

    // Access code2 again; should be a miss and produce a new Arc (re-decoded)
    let a2_second = ivm_cache::global_get_with_meta(&code2, &m).expect("re-decode code2");
    assert!(
        !Arc::ptr_eq(&a2_first, &a2_second),
        "expected code2 to be evicted and re-decoded",
    );

    let (h1, m1, e1) = ivm_cache::global_counters();
    assert!(h1 > h0, "expected at least one cache hit");
    assert!(
        m1 >= m0 + 3,
        "expected at least three misses (first inserts + re-decode)"
    );
    assert!(e1 > e0, "expected at least one eviction due to capacity");
}

#[test]
fn cache_key_includes_header_version() {
    let _lock = CACHE_TEST_LOCK.lock().unwrap();
    ivm_cache::set_global_capacity(4);
    let code = halt32_code();

    let m10 = meta(1, 0);
    let m11 = meta(1, 1);

    let a10 = ivm_cache::global_get_with_meta(&code, &m10).expect("decode v1.0");
    let a11 = ivm_cache::global_get_with_meta(&code, &m11).expect("decode v1.1");
    assert!(
        !Arc::ptr_eq(&a10, &a11),
        "same code with different header versions must not alias cache entries",
    );

    // Re-access both to ensure hits route to the correct entries
    let a10_hit = ivm_cache::global_get_with_meta(&code, &m10).expect("hit v1.0");
    let a11_hit = ivm_cache::global_get_with_meta(&code, &m11).expect("hit v1.1");
    assert!(Arc::ptr_eq(&a10, &a10_hit));
    assert!(Arc::ptr_eq(&a11, &a11_hit));
}

#[test]
fn zero_capacity_is_clamped_to_one() {
    let _lock = CACHE_TEST_LOCK.lock().unwrap();
    // Capacity set to zero must behave as capacity = 1 (no panics; single entry kept)
    ivm_cache::set_global_capacity(0);
    let m = meta(2, 0);
    let code1 = halt32_code();
    let code2 = two_c_ops_code();

    let a1_first = ivm_cache::global_get_with_meta(&code1, &m).expect("decode code1");
    let _a2 = ivm_cache::global_get_with_meta(&code2, &m).expect("decode code2 (evict code1)");
    let a1_second = ivm_cache::global_get_with_meta(&code1, &m).expect("re-decode code1");

    // With effective capacity=1, code1 should have been evicted by code2 and re-decoded now
    assert!(
        !Arc::ptr_eq(&a1_first, &a1_second),
        "expected code1 to be evicted under capacity=1 and re-decoded",
    );
}
