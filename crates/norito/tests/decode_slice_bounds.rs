//! Regression tests ensuring decode_from_slice rejects truncated or tampered
//! buffers with `Error::LengthMismatch` instead of panicking.

use std::{
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque},
    convert::TryInto,
};

use norito::{
    NoritoSerialize,
    core::{
        DecodeFlagsGuard, DecodeFromSlice, Error, Header, PayloadCtxGuard, header_flags,
        read_len_dyn, read_seq_len_slice, reset_decode_state,
    },
};

fn packed_seq_supported() -> bool {
    norito::core::default_encode_flags() & header_flags::PACKED_SEQ != 0
}

fn encode_payload<T: NoritoSerialize>(value: &T) -> (u8, Vec<u8>) {
    let bytes = norito::to_bytes(value).expect("encode payload");
    let flags = bytes[Header::SIZE - 1];
    let payload = bytes[Header::SIZE..].to_vec();
    (flags, payload)
}

fn expect_len_mismatch<'a, T>(flags: u8, payload: &'a [u8])
where
    T: DecodeFromSlice<'a>,
{
    assert!(!payload.is_empty(), "payload should contain data");
    let guard = DecodeFlagsGuard::enter(flags);
    let ctx = PayloadCtxGuard::enter(payload);
    let res = <T as DecodeFromSlice>::decode_from_slice(payload);
    drop(ctx);
    drop(guard);
    reset_decode_state();
    assert!(matches!(res, Err(Error::LengthMismatch)));
}

fn encode_payload_with_flags<T: NoritoSerialize>(value: &T, flags: u8) -> Vec<u8> {
    reset_decode_state();
    let mut payload = Vec::new();
    {
        let _guard = DecodeFlagsGuard::enter(flags);
        value.serialize(&mut payload).expect("serialize payload");
    }
    payload
}

#[test]
fn vec_packed_truncated_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let (flags, mut payload) = encode_payload(&vec![1u32, 2, 3]);
    assert!(payload.pop().is_some());
    expect_len_mismatch::<Vec<u32>>(flags, &payload);
}

#[test]
fn vec_fixed_truncated_yields_length_mismatch() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&(1u64).to_le_bytes());
    payload.extend_from_slice(&(4u64).to_le_bytes());
    payload.extend_from_slice(&(1u32).to_le_bytes());
    assert!(payload.pop().is_some());
    expect_len_mismatch::<Vec<u32>>(0, &payload);
}

#[test]
fn array_truncated_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let (flags, mut payload) = encode_payload(&[1u32, 2u32]);
    assert!(payload.pop().is_some());
    expect_len_mismatch::<[u32; 2]>(flags, &payload);
}

#[test]
fn vecdeque_truncated_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let (flags, mut payload) = encode_payload(&VecDeque::from(vec![10u32, 20]));
    assert!(payload.pop().is_some());
    expect_len_mismatch::<VecDeque<u32>>(flags, &payload);
}

#[test]
fn linked_list_truncated_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let mut list = LinkedList::new();
    list.push_back(7u32);
    list.push_back(8u32);
    let (flags, mut payload) = encode_payload(&list);
    assert!(payload.pop().is_some());
    expect_len_mismatch::<LinkedList<u32>>(flags, &payload);
}

#[test]
fn binary_heap_truncated_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let (flags, mut payload) = encode_payload(&BinaryHeap::from(vec![5u32, 6]));
    assert!(payload.pop().is_some());
    expect_len_mismatch::<BinaryHeap<u32>>(flags, &payload);
}

#[test]
fn btree_set_offsets_tampered_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let mut set = BTreeSet::new();
    set.insert(1u32);
    set.insert(2u32);
    let (flags, mut payload) = encode_payload(&set);
    {
        let guard = DecodeFlagsGuard::enter(flags);
        let (len, offset) = read_seq_len_slice(&payload).expect("seq len");
        assert!(len > 0);
        assert!(offset < payload.len());
        payload[offset] = 0x7F; // exaggerate first element size
        drop(guard);
        reset_decode_state();
    }
    expect_len_mismatch::<BTreeSet<u32>>(flags, &payload);
}

#[test]
fn hash_set_offsets_tampered_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let mut set = HashSet::new();
    set.insert(1u32);
    set.insert(2u32);
    let (flags, mut payload) = encode_payload(&set);
    {
        let guard = DecodeFlagsGuard::enter(flags);
        let (len, offset) = read_seq_len_slice(&payload).expect("seq len");
        assert!(len > 0);
        assert!(offset < payload.len());
        payload[offset] = 0x7F;
        drop(guard);
        reset_decode_state();
    }
    expect_len_mismatch::<HashSet<u32>>(flags, &payload);
}

#[test]
fn btree_map_offsets_tampered_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let map = BTreeMap::from([(1u32, 10u32), (2u32, 20u32)]);
    let (flags, mut payload) = encode_payload(&map);
    {
        let guard = DecodeFlagsGuard::enter(flags);
        let (len, offset) = read_seq_len_slice(&payload).expect("seq len");
        assert!(len > 0);
        assert!(offset < payload.len());
        payload[offset] = 0x7F;
        drop(guard);
        reset_decode_state();
    }
    expect_len_mismatch::<BTreeMap<u32, u32>>(flags, &payload);
}

#[test]
fn hash_map_offsets_tampered_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let map = HashMap::from([(1u32, 42u32), (2u32, 84u32)]);
    let (flags, mut payload) = encode_payload(&map);
    {
        let guard = DecodeFlagsGuard::enter(flags);
        let (len, offset) = read_seq_len_slice(&payload).expect("seq len");
        assert!(len > 0);
        assert!(offset < payload.len());
        payload[offset] = 0x7F;
        drop(guard);
        reset_decode_state();
    }
    expect_len_mismatch::<HashMap<u32, u32>>(flags, &payload);
}

#[test]
fn btree_set_fixed_offsets_tampered_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let mut set = BTreeSet::new();
    set.insert(10u32);
    set.insert(20u32);
    set.insert(30u32);
    let flags = header_flags::PACKED_SEQ;
    let mut payload = encode_payload_with_flags(&set, flags);
    let len = u64::from_le_bytes(payload[0..8].try_into().expect("len bytes")) as usize;
    assert_eq!(len, set.len());
    let offsets_start = 8;
    let offsets_end = offsets_start + (len + 1) * 8;
    assert!(offsets_end <= payload.len());
    let bogus = (payload.len() as u64).saturating_add(16);
    payload[offsets_start + 8..offsets_start + 16].copy_from_slice(&bogus.to_le_bytes());
    expect_len_mismatch::<BTreeSet<u32>>(flags, &payload);
}

#[test]
fn btree_map_fixed_offsets_tampered_yields_length_mismatch() {
    if !packed_seq_supported() {
        return;
    }
    let map = BTreeMap::from([(1u32, 100u32), (2u32, 200u32)]);
    let flags = header_flags::PACKED_SEQ;
    let mut payload = encode_payload_with_flags(&map, flags);
    let len = u64::from_le_bytes(payload[0..8].try_into().expect("len bytes")) as usize;
    assert_eq!(len, map.len());
    let key_offsets_start = 8;
    let key_offsets_end = key_offsets_start + (len + 1) * 8;
    assert!(key_offsets_end <= payload.len());
    let bogus_key = (payload.len() as u64).saturating_add(32);
    payload[key_offsets_start + 8..key_offsets_start + 16]
        .copy_from_slice(&bogus_key.to_le_bytes());
    expect_len_mismatch::<BTreeMap<u32, u32>>(flags, &payload);

    let mut payload_val = encode_payload_with_flags(&map, flags);
    let len_val = u64::from_le_bytes(payload_val[0..8].try_into().expect("len bytes")) as usize;
    assert_eq!(len_val, map.len());
    let value_offsets_start = 8 + (len_val + 1) * 8;
    let value_offsets_end = value_offsets_start + (len_val + 1) * 8;
    assert!(value_offsets_end <= payload_val.len());
    let bogus_val = (payload_val.len() as u64).saturating_add(48);
    payload_val[value_offsets_start + 8..value_offsets_start + 16]
        .copy_from_slice(&bogus_val.to_le_bytes());
    expect_len_mismatch::<BTreeMap<u32, u32>>(flags, &payload_val);
}

#[test]
fn pointer_truncated_varint_errors() {
    if (norito::core::default_encode_flags() & header_flags::COMPACT_LEN) == 0 {
        return;
    }
    let bytes = [0x81u8];
    let flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    let ctx = PayloadCtxGuard::enter(&bytes);
    let res = unsafe { read_len_dyn(bytes.as_ptr()) };
    drop(ctx);
    drop(flags);
    reset_decode_state();
    assert!(matches!(res, Err(Error::LengthMismatch)));
}

#[test]
fn pointer_before_payload_is_rejected() {
    if (norito::core::default_encode_flags() & header_flags::COMPACT_LEN) == 0 {
        return;
    }
    let mut backing = [0u8; 8];
    // Provide a valid payload context on a subslice near the end.
    let payload = &mut backing[2..];
    payload[0] = 0x01; // single-byte len for sanity
    let flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    let ctx = PayloadCtxGuard::enter(payload);
    // Pointer references earlier address before payload base.
    let misplaced = backing.as_ptr();
    let res = unsafe { read_len_dyn(misplaced) };
    drop(ctx);
    drop(flags);
    reset_decode_state();
    assert!(matches!(res, Err(Error::LengthMismatch)));
}
