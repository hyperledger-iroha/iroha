//! Tests for accelerated container serialization fast paths and CRC selection.
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use norito::{decode_from_bytes, to_bytes};

#[test]
fn vec_fastpath_roundtrip_and_determinism() {
    let v: Vec<String> = (0..50).map(|i| format!("item-{i}").repeat(2)).collect();
    let a = to_bytes(&v).expect("encode A");
    let b = to_bytes(&v).expect("encode B");
    assert_eq!(a, b, "Vec encoding must be deterministic");
    let out: Vec<String> = decode_from_bytes(&a).expect("decode");
    assert_eq!(v, out);
}

#[test]
fn btreemap_fastpath_roundtrip_and_determinism() {
    let mut m: BTreeMap<String, u64> = BTreeMap::new();
    for i in 0..64u64 {
        m.insert(format!("k{i}"), i * 17);
    }
    let a = to_bytes(&m).expect("encode A");
    let b = to_bytes(&m).expect("encode B");
    assert_eq!(a, b, "BTreeMap encoding must be deterministic");
    let out: BTreeMap<String, u64> = decode_from_bytes(&a).expect("decode");
    assert_eq!(m, out);
}

#[test]
fn hashmap_fastpath_roundtrip_and_determinism() {
    let mut m: HashMap<String, u32> = HashMap::new();
    for i in 0..64u32 {
        m.insert(format!("k{i}"), i.wrapping_mul(31));
    }
    let a = to_bytes(&m).expect("encode A");
    let b = to_bytes(&m).expect("encode B");
    assert_eq!(a, b, "HashMap encoding must be deterministic");
    let out: HashMap<String, u32> = decode_from_bytes(&a).expect("decode");
    assert_eq!(m, out);
}

#[test]
fn sets_fastpath_roundtrip_and_determinism() {
    let mut bs: BTreeSet<String> = BTreeSet::new();
    let mut hs: HashSet<String> = HashSet::new();
    for i in 0..40u64 {
        let s = format!("s{i}");
        bs.insert(s.clone());
        hs.insert(s);
    }
    let bs_a = to_bytes(&bs).expect("encode A");
    let bs_b = to_bytes(&bs).expect("encode B");
    assert_eq!(bs_a, bs_b);
    let bs_out: BTreeSet<String> = decode_from_bytes(&bs_a).expect("decode");
    assert_eq!(bs, bs_out);

    let hs_a = to_bytes(&hs).expect("encode A");
    let hs_b = to_bytes(&hs).expect("encode B");
    assert_eq!(hs_a, hs_b);
    let hs_out: HashSet<String> = decode_from_bytes(&hs_a).expect("decode");
    assert_eq!(hs, hs_out);
}

#[test]
fn hardware_crc64_matches_fallback() {
    use norito::core::{crc64_fallback, hardware_crc64};
    let payloads: &[&[u8]] = &[
        &b""[..],
        &b"a"[..],
        &b"abc"[..],
        &b"The quick brown fox jumps over the lazy dog"[..],
        &[0u8; 64][..],
    ];
    for p in payloads.iter().copied() {
        assert_eq!(hardware_crc64(p), crc64_fallback(p));
    }
}
