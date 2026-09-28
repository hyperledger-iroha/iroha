//! Binary sequence span-planner coverage.
use norito::core::{self, BinarySequenceLayout, SequenceSpan, header_flags, plan_binary_sequence};

#[test]
fn sequence_span_empty_matches_its_saturating_length() {
    for span in [
        SequenceSpan { start: 3, end: 3 },
        SequenceSpan { start: 4, end: 3 },
    ] {
        assert!(span.is_empty());
        assert_eq!(span.len(), 0);
    }

    let span = SequenceSpan { start: 3, end: 4 };
    assert!(!span.is_empty());
    assert_eq!(span.len(), 1);
}
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
fn reserved_layout_flags_are_rejected_before_planning() {
    let mut bytes = fixed_seq_header(1);
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.push(b'a');
    for flags in [0x01, 0x03, 0x04, 0x20] {
        let err = plan_binary_sequence(&bytes, flags, BinarySequenceLayout::LengthPrefixed)
            .expect_err("reserved layout flags must fail");
        assert!(
            matches!(err, core::Error::UnsupportedFeature("layout flag")),
            "flags {flags:#04x}: {err:?}"
        );
    }
}
