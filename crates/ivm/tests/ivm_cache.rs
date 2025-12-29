use ivm::{IvmCache, encoding, instruction};

#[test]
fn cache_decodes_mixed_stream() {
    let mut cache = IvmCache::new(4);
    // Build code: [ADD][XOR][HALT]
    let add = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 1, 0, 1);
    let xor = encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 2, 3, 4);
    let halt = encoding::wide::encode_halt();
    let mut code = Vec::new();
    code.extend_from_slice(&add.to_le_bytes());
    code.extend_from_slice(&xor.to_le_bytes());
    code.extend_from_slice(&halt.to_le_bytes());

    let ops = cache.get_or_predecode(&code, 0, 0).expect("decode");
    assert_eq!(ops.len(), 3);
    // All instructions are 32-bit words
    assert_eq!(ops[0].pc, 0);
    assert_eq!(ops[0].len, 4);
    assert_eq!(ops[1].pc, 4);
    assert_eq!(ops[1].len, 4);
    assert_eq!(ops[2].pc, 8);
    assert_eq!(ops[2].len, 4);
    assert_eq!(ops[2].inst, encoding::wide::encode_halt());
}

#[test]
fn cache_hit_and_eviction_lru() {
    let mut cache = IvmCache::new(2);
    // Three distinct codes to trigger eviction
    let s1 = encoding::wide::encode_rr(instruction::wide::arithmetic::ADD, 1, 0, 1)
        .to_le_bytes()
        .to_vec();
    let s2 = encoding::wide::encode_rr(instruction::wide::arithmetic::SUB, 2, 1, 0)
        .to_le_bytes()
        .to_vec();
    let s3 = encoding::wide::encode_rr(instruction::wide::arithmetic::XOR, 3, 2, 1)
        .to_le_bytes()
        .to_vec();

    let a1 = cache.get_or_predecode(&s1, 0, 0).unwrap();
    let a2 = cache.get_or_predecode(&s2, 0, 0).unwrap();
    // Access s1 again to make it most-recently used
    let a1b = cache.get_or_predecode(&s1, 0, 0).unwrap();
    assert!(Arc::ptr_eq(&a1, &a1b));
    // Insert s3 — should evict s2 (least recently used)
    let _a3 = cache.get_or_predecode(&s3, 0, 0).unwrap();
    // Re-inserting s2 decodes anew (not pointer-equal to previous a2 arc)
    let a2_new = cache.get_or_predecode(&s2, 0, 0).unwrap();
    assert!(!Arc::ptr_eq(&a2, &a2_new));
}

use std::sync::Arc;

#[test]
fn cache_key_version_affects_entries() {
    let mut cache = IvmCache::new(4);
    let s = encoding::wide::encode_halt().to_le_bytes().to_vec();
    let v1 = cache.get_or_predecode(&s, 1, 0).unwrap();
    let v2 = cache.get_or_predecode(&s, 2, 0).unwrap();
    assert!(!Arc::ptr_eq(&v1, &v2));
}

#[test]
fn cache_key_minor_version_affects_entries() {
    let mut cache = IvmCache::new(4);
    let s = encoding::wide::encode_halt().to_le_bytes().to_vec();
    let v10 = cache.get_or_predecode(&s, 2, 0).unwrap();
    let v11 = cache.get_or_predecode(&s, 2, 1).unwrap();
    assert!(!Arc::ptr_eq(&v10, &v11));
}
