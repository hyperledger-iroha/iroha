//! Binary sequence span-planner coverage.

use norito::core::{self, BinarySequenceLayout, SequenceSpan, header_flags, plan_binary_sequence};

fn fixed_seq_header(count: u64) -> Vec<u8> {
    count.to_le_bytes().to_vec()
}

#[test]
fn length_prefixed_fixed_width_spans() {
    let mut bytes = fixed_seq_header(2);
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.push(b'a');
    bytes.extend_from_slice(&3u64.to_le_bytes());
    bytes.extend_from_slice(b"bcd");

    let plan = plan_binary_sequence(&bytes, 0, BinarySequenceLayout::LengthPrefixed)
        .expect("plan fixed-width length-prefixed sequence");

    assert_eq!(
        plan.spans,
        vec![
            SequenceSpan { start: 16, end: 17 },
            SequenceSpan { start: 25, end: 28 },
        ],
    );
    assert_eq!(plan.used, bytes.len());
}

#[test]
fn length_prefixed_compact_spans_include_multibyte_lengths() {
    let mut bytes = fixed_seq_header(2);
    bytes.push(1);
    bytes.push(b'a');
    bytes.extend_from_slice(&[0x82, 0x01]);
    bytes.extend(std::iter::repeat_n(0x55, 130));

    let plan = plan_binary_sequence(
        &bytes,
        header_flags::COMPACT_LEN,
        BinarySequenceLayout::LengthPrefixed,
    )
    .expect("plan compact length-prefixed sequence");

    assert_eq!(
        plan.spans,
        vec![
            SequenceSpan { start: 9, end: 10 },
            SequenceSpan {
                start: 12,
                end: 142,
            },
        ],
    );
    assert_eq!(plan.used, bytes.len());
}

#[test]
fn packed_fixed_offsets_spans() {
    let mut bytes = fixed_seq_header(3);
    for offset in [0u64, 1, 3, 6] {
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    bytes.extend_from_slice(b"abcdef");

    let plan = plan_binary_sequence(
        &bytes,
        header_flags::PACKED_SEQ,
        BinarySequenceLayout::FixedOffsets,
    )
    .expect("plan packed sequence");

    assert_eq!(
        plan.spans,
        vec![
            SequenceSpan { start: 40, end: 41 },
            SequenceSpan { start: 41, end: 43 },
            SequenceSpan { start: 43, end: 46 },
        ],
    );
    assert_eq!(plan.used, bytes.len());
}

#[test]
fn packed_empty_sequence_consumes_zero_offset_sentinel() {
    let mut bytes = fixed_seq_header(0);
    bytes.extend_from_slice(&0u64.to_le_bytes());

    let plan = plan_binary_sequence(
        &bytes,
        header_flags::PACKED_SEQ,
        BinarySequenceLayout::FixedOffsets,
    )
    .expect("plan empty packed sequence");

    assert!(plan.spans.is_empty());
    assert_eq!(plan.used, 16);
}

#[test]
fn compact_length_rejects_truncated_varint() {
    let mut bytes = fixed_seq_header(1);
    bytes.push(0x80);

    let err = plan_binary_sequence(
        &bytes,
        header_flags::COMPACT_LEN,
        BinarySequenceLayout::LengthPrefixed,
    )
    .expect_err("truncated compact length must fail");

    assert!(matches!(err, core::Error::LengthMismatch));
}

#[test]
fn compact_length_rejects_overlong_varint() {
    let mut bytes = fixed_seq_header(1);
    bytes.extend_from_slice(&[0x81, 0x00]);

    let err = plan_binary_sequence(
        &bytes,
        header_flags::COMPACT_LEN,
        BinarySequenceLayout::LengthPrefixed,
    )
    .expect_err("overlong compact length must fail");

    assert!(matches!(err, core::Error::LengthMismatch));
}

#[test]
fn length_prefixed_rejects_truncated_payload() {
    let mut bytes = fixed_seq_header(1);
    bytes.extend_from_slice(&4u64.to_le_bytes());
    bytes.extend_from_slice(b"abc");

    let err = plan_binary_sequence(&bytes, 0, BinarySequenceLayout::LengthPrefixed)
        .expect_err("truncated payload must fail");

    assert!(matches!(err, core::Error::LengthMismatch));
}

#[test]
fn packed_offsets_reject_non_monotonic_table() {
    let mut bytes = fixed_seq_header(2);
    for offset in [0u64, 5, 4] {
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    bytes.extend_from_slice(b"abcdef");

    let err = plan_binary_sequence(
        &bytes,
        header_flags::PACKED_SEQ,
        BinarySequenceLayout::FixedOffsets,
    )
    .expect_err("non-monotonic offsets must fail");

    assert!(matches!(err, core::Error::LengthMismatch));
}

#[test]
fn packed_offsets_reject_truncated_data() {
    let mut bytes = fixed_seq_header(1);
    for offset in [0u64, 4] {
        bytes.extend_from_slice(&offset.to_le_bytes());
    }
    bytes.extend_from_slice(b"abc");

    let err = plan_binary_sequence(
        &bytes,
        header_flags::PACKED_SEQ,
        BinarySequenceLayout::FixedOffsets,
    )
    .expect_err("truncated packed payload must fail");

    assert!(matches!(err, core::Error::LengthMismatch));
}

#[cfg(feature = "parallel-decode")]
#[test]
fn planned_sequence_parallel_decode_preserves_order() {
    let values = [10u32, 20, 30, 40];
    let mut bytes = fixed_seq_header(values.len() as u64);
    for value in values {
        bytes.extend_from_slice(&4u64.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let plan = plan_binary_sequence(&bytes, 0, BinarySequenceLayout::LengthPrefixed)
        .expect("plan fixed-width u32 sequence");

    let decoded: Vec<u32> = core::decode_planned_sequence_parallel(&bytes, 0, &plan)
        .expect("parallel decode planned sequence");

    assert_eq!(decoded, values);
}

#[cfg(feature = "parallel-decode")]
#[test]
fn vec_decode_large_sequence_uses_parallel_capable_plan() {
    let values: Vec<u64> = (0..16_384).map(|idx| idx as u64).collect();
    let mut bytes = fixed_seq_header(values.len() as u64);
    let flags = core::default_encode_flags();
    for value in &values {
        core::write_len_with_flags(&mut bytes, 8, flags).expect("write element length");
        bytes.extend_from_slice(&value.to_le_bytes());
    }

    let (decoded, used) = <Vec<u64> as core::DecodeFromSlice>::decode_from_slice(&bytes)
        .expect("decode large planned sequence");

    assert_eq!(used, bytes.len());
    assert_eq!(decoded, values);
}
