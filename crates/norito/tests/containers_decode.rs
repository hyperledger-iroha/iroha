//! Roundtrip tests for container decoders using reusable scratch buffers.

use std::collections::{BinaryHeap, LinkedList, VecDeque};

use norito::{core, decode_from_bytes, to_bytes};

#[test]
fn vec_nested_roundtrip_sequential() {
    let mut outer: Vec<Vec<u8>> = Vec::new();
    for i in 0..32u32 {
        let len = match i % 5 {
            0 => 0usize,
            1 => (3 + i) as usize,
            2 => (128 + i) as usize,
            3 => (512 + i * 7) as usize,
            _ => (2048 + i * 9) as usize,
        };
        let inner: Vec<u8> = (0..len)
            .map(|j| (j as u8).wrapping_mul(31) ^ (i as u8))
            .collect();
        outer.push(inner);
    }

    let payload = (outer.clone(), 0xfeed_beefu64);
    let bytes = to_bytes(&payload).expect("encode sequential payload");
    assert_eq!(
        bytes[core::Header::SIZE - 1],
        0,
        "sequential layout must not set header flags"
    );

    let decoded: (Vec<Vec<u8>>, u64) = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(decoded.0, outer);
    assert_eq!(decoded.1, 0xfeed_beefu64);
}

#[test]
fn vecdeque_roundtrip() {
    let mut vd: VecDeque<String> = VecDeque::new();
    for i in 0..256u32 {
        vd.push_back(format!("item-{i}"));
    }
    let bytes = to_bytes(&vd).expect("encode");
    let out: VecDeque<String> = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(vd, out);
}

#[test]
fn linkedlist_roundtrip() {
    let mut ll: LinkedList<String> = LinkedList::new();
    for i in 0..128u32 {
        ll.push_back(format!("ll-{i}"));
    }
    let bytes = to_bytes(&ll).expect("encode");
    let out: LinkedList<String> = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(ll, out);
}

#[test]
fn binaryheap_roundtrip() {
    let mut heap: BinaryHeap<i64> = BinaryHeap::new();
    for i in 0..200i64 {
        heap.push(i * 13 - 7);
    }
    let bytes = to_bytes(&heap).expect("encode");
    let out: BinaryHeap<i64> = decode_from_bytes(&bytes).expect("decode");
    assert_eq!(heap.into_sorted_vec(), out.into_sorted_vec());
}
