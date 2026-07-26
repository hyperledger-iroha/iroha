//! Compact length coverage for core collection serializers.

use std::{
    borrow::Cow,
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque},
    rc::Rc,
    sync::Arc,
};

use norito::{
    NoritoDeserialize, NoritoSerialize,
    core::{self, DecodeFlagsGuard, header_flags},
};

fn serialize_payload<T: NoritoSerialize>(value: &T) -> Vec<u8> {
    let mut out = Vec::new();
    value.serialize(&mut out).expect("serialize");
    out
}

fn assert_seq_header(bytes: &[u8], expected_len: usize) -> usize {
    let (len, hdr) = core::read_seq_len_slice(bytes).expect("seq header");
    assert_eq!(len, expected_len, "unexpected sequence length");
    assert_eq!(hdr, 8, "sequence header should be fixed-width");
    hdr
}

fn assert_len_prefix(bytes: &[u8], expected_len: usize) {
    let (len, hdr) = core::read_len_dyn_slice(bytes).expect("len header");
    assert_eq!(len, expected_len, "unexpected element length");
    assert_eq!(hdr, 1, "length prefix should be varint");
}

#[test]
fn compact_len_serializers_keep_fixed_seq_len() {
    let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);

    assert_eq!(core::seq_len_prefix_len(2), 8);

    let bytes = serialize_payload(&vec![1u8, 2]);
    let hdr = assert_seq_header(&bytes, 2);
    assert_len_prefix(&bytes[hdr..], 1);

    let deque = VecDeque::from(vec![1u8, 2]);
    let bytes = serialize_payload(&deque);
    let hdr = assert_seq_header(&bytes, 2);
    assert_len_prefix(&bytes[hdr..], 1);

    let list: LinkedList<u8> = [1u8, 2].into_iter().collect();
    let bytes = serialize_payload(&list);
    let hdr = assert_seq_header(&bytes, 2);
    assert_len_prefix(&bytes[hdr..], 1);

    let heap = BinaryHeap::from(vec![2u8, 1]);
    let bytes = serialize_payload(&heap);
    let hdr = assert_seq_header(&bytes, 2);
    assert_len_prefix(&bytes[hdr..], 1);

    let set: BTreeSet<u8> = [1u8, 2].into_iter().collect();
    let bytes = serialize_payload(&set);
    let hdr = assert_seq_header(&bytes, 2);
    assert_len_prefix(&bytes[hdr..], 1);

    let set: HashSet<u8> = [1u8, 2].into_iter().collect();
    let bytes = serialize_payload(&set);
    let hdr = assert_seq_header(&bytes, 2);
    assert_len_prefix(&bytes[hdr..], 1);

    let map: BTreeMap<u8, u8> = [(1u8, 2u8)].into_iter().collect();
    let bytes = serialize_payload(&map);
    let hdr = assert_seq_header(&bytes, 1);
    assert_len_prefix(&bytes[hdr..], 1);

    let map: HashMap<u8, u8> = [(1u8, 2u8)].into_iter().collect();
    let bytes = serialize_payload(&map);
    let hdr = assert_seq_header(&bytes, 1);
    assert_len_prefix(&bytes[hdr..], 1);
}

#[test]
fn compact_len_for_result_tuple_array_and_cow() {
    let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);

    let cow: Cow<'_, str> = Cow::Borrowed("a");
    let bytes = serialize_payload(&cow);
    assert_eq!(bytes, vec![1, b'a']);

    let ok: Result<u8, u8> = Ok(7);
    let bytes = serialize_payload(&ok);
    assert_eq!(bytes, vec![0, 1, 7]);

    let arr: [u8; 2] = [9, 10];
    let bytes = serialize_payload(&arr);
    assert_eq!(bytes, vec![1, 9, 1, 10]);

    let tup = (1u8, 2u8);
    let bytes = serialize_payload(&tup);
    assert_eq!(bytes, vec![1, 1, 1, 2]);
}

#[test]
fn owned_exact_lengths_respect_compact_len() {
    let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);

    let boxed = Box::new(7u8);
    let bytes = serialize_payload(&boxed);
    assert_eq!(boxed.encoded_len_exact(), Some(bytes.len()));

    let shared = Rc::new(7u8);
    let bytes = serialize_payload(&shared);
    assert_eq!(shared.encoded_len_exact(), Some(bytes.len()));

    let shared = Arc::new(7u8);
    let bytes = serialize_payload(&shared);
    assert_eq!(shared.encoded_len_exact(), Some(bytes.len()));
}

#[test]
fn binaryheap_compact_roundtrip() {
    let _guard = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);

    let heap = BinaryHeap::from(vec![3u8, 1u8, 2u8]);
    let bytes = core::to_bytes(&heap).expect("encode binary heap");
    let archived = core::from_bytes::<BinaryHeap<u8>>(&bytes).expect("decode header");
    let decoded = BinaryHeap::deserialize(archived);

    assert_eq!(decoded.into_sorted_vec(), heap.into_sorted_vec());
}
