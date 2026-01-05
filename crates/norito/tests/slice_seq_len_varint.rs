//! Tests for slice-based decoders honoring COMPACT_SEQ_LEN varint sequence headers.
#![cfg(feature = "compact-len")]

use std::collections::{BinaryHeap, LinkedList, VecDeque};

use norito::core::{self, DecodeFlagsGuard};

fn write_varint_len(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        } else {
            out.push(byte | 0x80);
        }
    }
}

#[test]
fn vecdeque_decode_from_slice_compact_seq_len() {
    let items = ["a", "bbb", "", "z"];
    let mut elem_payloads = Vec::new();
    let mut spans = Vec::new();
    for s in items.iter() {
        let mut payload = Vec::new();
        write_varint_len(&mut payload, s.len() as u64);
        payload.extend_from_slice(s.as_bytes());
        spans.push(payload.len());
        elem_payloads.push(payload);
    }

    let mut buf = Vec::new();
    write_varint_len(&mut buf, items.len() as u64);
    for span in &spans {
        write_varint_len(&mut buf, *span as u64);
    }
    for payload in &elem_payloads {
        buf.extend_from_slice(payload);
    }

    let flags = core::header_flags::PACKED_SEQ
        | core::header_flags::VARINT_OFFSETS
        | core::header_flags::COMPACT_LEN
        | core::header_flags::COMPACT_SEQ_LEN;
    let _fg = DecodeFlagsGuard::enter(flags);
    let (out, used) = <VecDeque<String> as core::DecodeFromSlice>::decode_from_slice(&buf)
        .expect("vecdeque decode");
    assert_eq!(used, buf.len());
    let expected: VecDeque<String> = items.iter().map(|s| (*s).to_string()).collect();
    assert_eq!(out, expected);
}

#[test]
fn linkedlist_decode_from_slice_compact_seq_len() {
    let items = [1u32, 2, 3, 4, 5];
    let mut spans = Vec::new();
    let mut elem_payloads = Vec::new();
    for &v in items.iter() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&v.to_le_bytes());
        spans.push(payload.len());
        elem_payloads.push(payload);
    }

    let mut buf = Vec::new();
    write_varint_len(&mut buf, items.len() as u64);
    for span in &spans {
        write_varint_len(&mut buf, *span as u64);
    }
    for payload in &elem_payloads {
        buf.extend_from_slice(payload);
    }

    let flags = core::header_flags::PACKED_SEQ
        | core::header_flags::VARINT_OFFSETS
        | core::header_flags::COMPACT_LEN
        | core::header_flags::COMPACT_SEQ_LEN;
    let _fg = DecodeFlagsGuard::enter(flags);
    let (out, used) = <LinkedList<u32> as core::DecodeFromSlice>::decode_from_slice(&buf)
        .expect("linkedlist decode");
    assert_eq!(used, buf.len());
    let expected: LinkedList<u32> = items.into_iter().collect();
    assert!(out.into_iter().eq(expected.into_iter()));
}

#[test]
fn binaryheap_decode_from_slice_compact_seq_len() {
    let items = [7u32, 3, 9, 1, 5];
    let mut buf = Vec::new();
    #[cfg(feature = "compact-len")]
    write_varint_len(&mut buf, items.len() as u64);
    #[cfg(not(feature = "compact-len"))]
    buf.extend_from_slice(&(items.len() as u64).to_le_bytes());
    for &v in items.iter() {
        #[cfg(feature = "compact-len")]
        write_varint_len(&mut buf, 4);
        #[cfg(not(feature = "compact-len"))]
        buf.extend_from_slice(&(4u64).to_le_bytes());
        buf.extend_from_slice(&v.to_le_bytes());
    }
    let flags = core::header_flags::COMPACT_LEN | core::header_flags::COMPACT_SEQ_LEN;
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
    // Varint with continuation bit set but no following byte.
    let buf = vec![0x80u8];
    let flags = core::header_flags::PACKED_SEQ
        | core::header_flags::VARINT_OFFSETS
        | core::header_flags::COMPACT_LEN
        | core::header_flags::COMPACT_SEQ_LEN;
    let _fg = DecodeFlagsGuard::enter(flags);
    let err = <VecDeque<u32> as core::DecodeFromSlice>::decode_from_slice(&buf).unwrap_err();
    assert!(matches!(err, core::Error::LengthMismatch));
}

#[test]
fn linkedlist_malformed_seq_len_returns_length_mismatch() {
    let buf = vec![0x80u8];
    let flags = core::header_flags::PACKED_SEQ
        | core::header_flags::VARINT_OFFSETS
        | core::header_flags::COMPACT_LEN
        | core::header_flags::COMPACT_SEQ_LEN;
    let _fg = DecodeFlagsGuard::enter(flags);
    let err = <LinkedList<u32> as core::DecodeFromSlice>::decode_from_slice(&buf).unwrap_err();
    assert!(matches!(err, core::Error::LengthMismatch));
}

#[test]
fn binaryheap_malformed_seq_len_returns_length_mismatch() {
    let buf = vec![0x80u8];
    let flags = core::header_flags::COMPACT_LEN | core::header_flags::COMPACT_SEQ_LEN;
    let _fg = DecodeFlagsGuard::enter(flags);
    let err = <BinaryHeap<u32> as core::DecodeFromSlice>::decode_from_slice(&buf).unwrap_err();
    assert!(matches!(err, core::Error::LengthMismatch));
}
