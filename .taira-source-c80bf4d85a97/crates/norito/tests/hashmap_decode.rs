//! Regression tests for HashMap decode performance path.
//! Ensures functional equivalence after optimizing allocations.

use std::collections::{BTreeMap, HashMap, HashSet};

use norito::{decode_from_bytes, to_bytes};
use rand::{Rng, SeedableRng, rngs::StdRng};

fn gen_map(n: usize) -> HashMap<String, u32> {
    let mut rng = StdRng::seed_from_u64(12345);
    let mut hm = HashMap::with_capacity(n);
    for i in 0..n as u64 {
        let k = format!("k-{}-{}", i, rng.random::<u32>());
        let v = (i as u32).wrapping_mul(7).wrapping_add(3);
        hm.insert(k, v);
    }
    hm
}

#[test]
fn hashmap_roundtrip_small() {
    let hm = gen_map(32);
    let bytes = to_bytes(&hm).expect("encode");
    let out: HashMap<String, u32> = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(hm, out);
}

#[test]
fn hashmap_roundtrip_larger() {
    let hm = gen_map(2_000);
    let bytes = to_bytes(&hm).expect("encode");
    let out: HashMap<String, u32> = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(hm, out);
}

#[test]
fn hashmap_roundtrip_empty() {
    let hm: HashMap<String, u32> = HashMap::new();
    let bytes = to_bytes(&hm).expect("encode");
    let out: HashMap<String, u32> = decode_from_bytes(&bytes).expect("decode");
    assert!(out.is_empty());
}

#[test]
fn btreemap_roundtrip() {
    let mut bm: BTreeMap<u64, String> = BTreeMap::new();
    for i in 0..200u64 {
        bm.insert(i * 7 + 1, format!("val-{i}"));
    }
    let bytes = to_bytes(&bm).expect("encode");
    let out: BTreeMap<u64, String> = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(bm, out);
}

#[test]
fn hashset_roundtrip() {
    let mut hs: HashSet<String> = HashSet::new();
    for i in 0..256u32 {
        hs.insert(format!("k{i}"));
    }
    let bytes = to_bytes(&hs).expect("encode");
    let out: HashSet<String> = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(hs, out);
}
