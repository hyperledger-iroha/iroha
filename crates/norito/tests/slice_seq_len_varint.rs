//! Tests for slice-based decoders with fixed u64 sequence headers.
#![cfg(feature = "compact-len")]

use std::collections::{BinaryHeap, LinkedList, VecDeque};

use norito::core::{self, DecodeFlagsGuard};

fn build_packed_seq(payloads: &[Vec<u8>]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(payloads.len() as u64).to_le_bytes());
    let mut total = 0u64;
    for payload in payloads {
        buf.extend_from_slice(&total.to_le_bytes());
        total = total.saturating_add(payload.len() as u64);
    }
    buf.extend_from_slice(&total.to_le_bytes());
    for payload in payloads {
        buf.extend_from_slice(payload);
    }
    buf
}

#[test]
fn vecdeque_decode_from_slice_fixed_seq_len() {
    let items = ["a", "bbb", "", "z"];
    let _elem_guard = DecodeFlagsGuard::enter(core::header_flags::COMPACT_LEN);
    let mut elem_payloads = Vec::new();
    for s in items.iter() {
        let mut payload = Vec::new();
        core::write_len(&mut payload, s.len() as u64).expect("write len");
        payload.extend_from_slice(s.as_bytes());
        elem_payloads.push(payload);
    }
    drop(_elem_guard);

    let buf = build_packed_seq(&elem_payloads);
    let flags = core::header_flags::PACKED_SEQ | core::header_flags::COMPACT_LEN;
    let _fg = DecodeFlagsGuard::enter(flags);
    let (out, used) = <VecDeque<String> as core::DecodeFromSlice>::decode_from_slice(&buf)
        .expect("vecdeque decode");
    assert_eq!(used, buf.len());
    let expected: VecDeque<String> = items.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(out, expected);
}

#[test]
fn linkedlist_decode_from_slice_fixed_seq_len() {
    let items = [1u32, 2, 3, 4, 5];
    let mut elem_payloads = Vec::new();
    for &v in items.iter() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&v.to_le_bytes());
        elem_payloads.push(payload);
    }

    let buf = build_packed_seq(&elem_payloads);
    let flags = core::header_flags::PACKED_SEQ;
    let _fg = DecodeFlagsGuard::enter(flags);
    let (out, used) = <LinkedList<u32> as core::DecodeFromSlice>::decode_from_slice(&buf)
        .expect("linkedlist decode");
    assert_eq!(used, buf.len());
    let expected: LinkedList<u32> = items.into_iter().collect();
    assert!(out.into_iter().eq(expected.into_iter()));
}

#[test]
fn binaryheap_decode_from_slice_fixed_seq_len() {
    let items = [7u32, 3, 9, 1, 5];
    let mut elem_payloads = Vec::new();
    for &v in items.iter() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&v.to_le_bytes());
        elem_payloads.push(payload);
    }

    let buf = build_packed_seq(&elem_payloads);
    let flags = core::header_flags::PACKED_SEQ;
    let _fg = DecodeFlagsGuard::enter(flags);
    let (out, used) = <BinaryHeap<u32> as core::DecodeFromSlice>::decode_from_slice(&buf)
        .expect("binaryheap decode");
    assert_eq!(used, buf.len());
    let mut expected = BinaryHeap::new();
    for &v in items.iter() {
        expected.push(v);
    }
    assert_eq!(out.clone().into_sorted_vec(), expected.into_sorted_vec());
}

#[test]
fn vecdeque_malformed_seq_len_returns_length_mismatch() {
    // Sequence headers are fixed-width in v1; a short header should fail.
    let buf = vec![0x80u8];
    let flags = core::header_flags::PACKED_SEQ;
    let _fg = DecodeFlagsGuard::enter(flags);
    let err = <VecDeque<u32> as core::DecodeFromSlice>::decode_from_slice(&buf).unwrap_err();
    assert!(matches!(err, core::Error::LengthMismatch));
}
