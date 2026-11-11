//! Prefix equality, measurement isolation and arithmetic limits for sequence length observations.

use super::*;
use crate::core::{
    self as codec, Encoder, default_encode_flags, header_flags, validate_header_flags,
};
use std::cell::Cell;

#[derive(Clone)]
struct Bytes(Vec<u8>);

impl SerializePayload for Bytes {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        writer.write_all(&self.0)?;
        Ok(())
    }
}

fn layouts() -> impl Iterator<Item = u8> {
    [
        0,
        header_flags::COMPACT_LEN,
        codec::WireLayout::Dense.code(),
    ]
    .into_iter()
}

fn assert_prefixes<T: SerializePayload + Clone>(values: &[T], flags: u8) {
    let mut measured = SequencePayloadLength::new(flags).unwrap();
    for count in 0..=values.len() {
        if count != 0 {
            measured.push(&values[count - 1]).unwrap();
        }
        let _flags = DecodeFlagsGuard::enter(flags);
        let mut actual = Vec::new();
        codec::serialize_to_buffer(&values[..count].to_vec(), &mut actual).unwrap();
        assert_eq!(
            measured.len(),
            actual.len(),
            "prefix {count}, flags {flags:#x}"
        );
        assert_eq!(measured.count(), count);
        assert_eq!(measured.is_empty(), count == 0);
        assert_eq!(measured.flags(), flags);
        let (declared, _) = codec::read_seq_len_slice(&actual).unwrap();
        assert_eq!(declared, count);
    }
}

#[test]
fn every_prefix_matches_real_vec_at_element_length_transitions() {
    let values = [0, 1, 126, 127, 128, 129, 16_383, 16_384, 0]
        .into_iter()
        .map(|size| Bytes(vec![0xA7; size]))
        .collect::<Vec<_>>();
    for flags in layouts() {
        assert_prefixes(&values, flags);
    }
}

#[test]
fn nested_layout_sensitive_elements_match_every_real_prefix() {
    #[derive(Clone, crate::Encode)]
    struct Nested {
        values: Vec<u16>,
        optional: Option<String>,
    }
    let values = [
        Nested {
            values: vec![],
            optional: None,
        },
        Nested {
            values: vec![3, 9],
            optional: Some(String::new()),
        },
        Nested {
            values: vec![7],
            optional: Some("x".repeat(128)),
        },
    ];
    for flags in layouts() {
        assert_prefixes(&values, flags);
    }
}

#[test]
fn zero_length_elements_still_add_their_required_framing() {
    for flags in layouts() {
        let mut measured = SequencePayloadLength::new(flags).unwrap();
        let framing = if flags & header_flags::COMPACT_LEN == 0 {
            8
        } else {
            1
        };
        assert_eq!(
            measured.len(),
            codec::seq_len_prefix_len_with_flags(0, flags)
        );
        for count in 1..=4 {
            let before = measured.len();
            measured.push(&Bytes(vec![])).unwrap();
            assert_eq!(measured.len(), before + framing);
            assert_eq!(measured.count(), count);
        }
        assert_prefixes::<Bytes>(&[], flags);
        assert_prefixes(&vec![Bytes(vec![]); 4], flags);
    }
}

#[test]
fn all_flag_bytes_are_validated_without_silent_sanitizing() {
    let dense = codec::WireLayout::Dense.code();
    for flags in 0..=u8::MAX {
        assert_eq!(
            SequencePayloadLength::new(flags).is_ok(),
            validate_header_flags(flags).is_ok() || flags == dense,
            "flags {flags:#04x}"
        );
    }
}

#[test]
fn dense_counts_are_varints_whose_width_follows_the_count() {
    let dense = codec::WireLayout::Dense.code();
    let mut measured = SequencePayloadLength::new(dense).unwrap();
    assert_eq!(measured.len(), 1);
    let element = Bytes(vec![7; 3]);
    for count in 1..=200_usize {
        measured.push(&element).unwrap();
        let count_width = if count < 128 { 1 } else { 2 };
        assert_eq!(
            measured.len(),
            count_width + count * (1 + 3),
            "count {count}"
        );
    }
    let elements = vec![element; 200];
    let mut encoded = Vec::new();
    {
        let _layout = codec::WireLayoutGuard::enter(codec::WireLayout::Dense);
        codec::write_element_sequence::<Bytes, _>(
            &mut Encoder::for_buffer(&mut encoded),
            elements.iter(),
        )
        .unwrap();
    }
    assert_eq!(encoded.len(), measured.len());
}

#[test]
fn each_push_counts_one_actual_payload_and_never_uses_hints() {
    struct Counted<'a>(&'a Cell<usize>);
    impl SerializePayload for Counted<'_> {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            self.0.set(self.0.get() + 1);
            writer.write_all(&[1, 2, 3])?;
            Ok(())
        }
        fn encoded_len_hint(&self) -> Option<usize> {
            panic!("hint must not be consulted")
        }
        fn encoded_len_exact(&self) -> Option<usize> {
            panic!("exact hint must not be consulted")
        }
    }
    for flags in layouts() {
        let first = Cell::new(0);
        let second = Cell::new(0);
        let mut measured = SequencePayloadLength::new(flags).unwrap();
        measured.push(&Counted(&first)).unwrap();
        let snapshot = measured;
        assert_eq!(first.get(), 1);
        measured.push(&Counted(&second)).unwrap();
        assert_eq!((first.get(), second.get()), (1, 1));
        assert_eq!(snapshot.count(), 1);
        assert_eq!(measured.count(), 2);
        assert!(measured.len() > snapshot.len());
    }
}

#[test]
fn false_hints_cannot_change_measured_sequence_lengths() {
    #[derive(Clone)]
    struct Liar(usize);
    impl SerializePayload for Liar {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            writer.write_all(&[0; 128])?;
            Ok(())
        }
        fn encoded_len_hint(&self) -> Option<usize> {
            Some(self.0)
        }
        fn encoded_len_exact(&self) -> Option<usize> {
            Some(self.0)
        }
    }
    for flags in layouts() {
        assert_prefixes(&[Liar(0), Liar(usize::MAX)], flags);
    }
}

#[test]
fn raw_byte_vec_is_explicitly_a_different_payload() {
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        if codec::is_dense_layout_code(flags) {
            // Dense has no per-byte framing: every sequence of `u8` is the raw
            // `Vec<u8>` payload, which this measurement type does not cover.
            let mut generic = Vec::new();
            codec::write_element_sequence::<u8, _>(
                &mut Encoder::for_buffer(&mut generic),
                [1_u8, 2, 3],
            )
            .unwrap();
            let mut raw = Vec::new();
            codec::serialize_to_buffer(&vec![1_u8, 2, 3], &mut raw).unwrap();
            assert_eq!(generic, raw);
            assert_eq!(raw, [3, 1, 2, 3]);
            continue;
        }
        let mut measured = SequencePayloadLength::new(flags).unwrap();
        for byte in [1_u8, 2, 3] {
            measured.push(&byte).unwrap();
        }
        let mut generic = Vec::new();
        codec::write_element_sequence::<u8, _>(
            &mut Encoder::for_buffer(&mut generic),
            [1_u8, 2, 3],
        )
        .unwrap();
        let mut raw = Vec::new();
        codec::serialize_to_buffer(&vec![1_u8, 2, 3], &mut raw).unwrap();
        assert_eq!(measured.len(), generic.len());
        assert_eq!(raw.len(), 11);
        assert_ne!(measured.len(), raw.len());
    }
}

#[test]
fn success_and_serializer_failure_restore_enclosing_flags_and_encode_tracking() {
    struct Probe {
        expected: u8,
        fail: bool,
    }
    impl SerializePayload for Probe {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            assert_eq!(codec::effective_layout_flags(), self.expected);
            codec::note_compact_len_emitted();
            writer.write_all(&[9])?;
            if self.fail {
                Err(Error::NonCanonicalEncoding)
            } else {
                Ok(())
            }
        }
    }
    let payload = [1, 2, 3];
    let _payload = codec::PayloadCtxGuard::enter_with_flags(&payload, header_flags::COMPACT_LEN);
    let payload_context = codec::payload_ctx();
    let _outer_flags = DecodeFlagsGuard::enter(0);
    // An outer marker survives each measurement, and the probe's inner marker never leaks out.
    for outer_marker in [false, true] {
        let _encode = codec::EncodeContextGuard::enter();
        if outer_marker {
            codec::note_compact_len_emitted();
        }
        for flags in layouts() {
            let mut measured = SequencePayloadLength::new(flags).unwrap();
            for fail in [false, true] {
                let snapshot = measured;
                let result = measured.push(&Probe {
                    expected: flags,
                    fail,
                });
                if fail {
                    assert!(matches!(result, Err(Error::NonCanonicalEncoding)));
                    assert_eq!(measured, snapshot);
                } else {
                    result.unwrap();
                }
                assert_eq!(codec::effective_layout_flags(), 0);
                assert_eq!(codec::payload_ctx(), payload_context);
                assert_eq!(codec::compact_len_used(), outer_marker);
            }
        }
    }
}

#[test]
fn measurement_keeps_active_decode_charges_and_limits() {
    struct Charge {
        fail: bool,
    }
    impl SerializePayload for Charge {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            assert!(codec::decode_limits_active());
            codec::reserve_decode_allocation(1)?;
            writer.write_all(&[1])?;
            if self.fail {
                Err(Error::NonCanonicalEncoding)
            } else {
                Ok(())
            }
        }
    }
    codec::with_decode_limits_scope(codec::DecodeLimits::new(10, 10, 10, 0, 10), || {
        let mut measured = SequencePayloadLength::new(default_encode_flags()).unwrap();
        let snapshot = measured;
        assert!(matches!(
            measured.push(&Charge { fail: false }),
            Err(Error::TotalAllocationExceeded {
                attempted: 1,
                limit: 0
            })
        ));
        assert_eq!(measured, snapshot);
    });
    for fail in [false, true] {
        let mut measured = SequencePayloadLength::new(default_encode_flags()).unwrap();
        codec::with_decode_limits_scope(codec::DecodeLimits::new(10, 10, 10, 1, 10), || {
            let result = measured.push(&Charge { fail });
            if fail {
                assert!(matches!(result, Err(Error::NonCanonicalEncoding)));
            } else {
                result.unwrap();
            }
            let snapshot = measured;
            assert!(matches!(
                measured.push(&Charge { fail: false }),
                Err(Error::TotalAllocationExceeded {
                    attempted: 2,
                    limit: 1
                })
            ));
            assert_eq!(measured, snapshot);
            assert_eq!(measured.count(), usize::from(!fail));
            assert!(matches!(
                codec::reserve_decode_allocation(1),
                Err(Error::TotalAllocationExceeded {
                    attempted: 2,
                    limit: 1
                })
            ));
        });
    }
}

#[test]
fn nested_decode_depth_is_not_reset_by_measurement() {
    struct NeedsDepth;
    impl SerializePayload for NeedsDepth {
        fn serialize(&self, _: &mut Encoder<'_>) -> Result<(), Error> {
            let _depth = codec::DecodeDepthGuard::enter()?;
            Ok(())
        }
    }
    codec::with_decode_limits_scope(codec::DecodeLimits::new(10, 10, 10, 10, 1), || {
        let mut measured = SequencePayloadLength::new(default_encode_flags()).unwrap();
        {
            let _outer = codec::DecodeDepthGuard::enter().unwrap();
            assert!(matches!(
                measured.push(&NeedsDepth),
                Err(Error::NestingDepthExceeded {
                    depth: 2,
                    limit: 1,
                    ..
                })
            ));
            assert_eq!(measured.count(), 0);
        }
        measured.push(&NeedsDepth).unwrap();
        assert_eq!(measured.count(), 1);
    });
}

#[test]
fn stateful_serialization_is_an_observation_not_a_later_byte_certificate() {
    struct Growing<'a>(&'a Cell<usize>);
    impl SerializePayload for Growing<'_> {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            let size = self.0.get();
            self.0.set(size + 1);
            writer.write_all(&[7; 8][..size])?;
            Ok(())
        }
    }
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        let calls = Cell::new(1);
        let mut measured = SequencePayloadLength::new(flags).unwrap();
        measured.push(&Growing(&calls)).unwrap();
        let snapshot = measured;
        assert_eq!(calls.get(), 2);
        let mut actual = Vec::new();
        assert!(matches!(
            codec::serialize_to_writer(&vec![Growing(&calls)], &mut actual),
            Err(Error::LengthMismatch)
        ));
        assert_eq!(measured, snapshot);
        assert_eq!(calls.get(), 4);
        // A buffer frames the bytes of its single visit, which already differ
        // from the earlier observation.
        let mut framed = Vec::new();
        codec::serialize_to_buffer(&vec![Growing(&calls)], &mut framed).unwrap();
        assert_eq!(calls.get(), 5);
        assert_ne!(framed.len(), measured.len());
        assert_eq!(measured, snapshot);
    }
}

#[test]
fn checked_arithmetic_accepts_the_last_representable_length_then_rejects() {
    for flags in layouts() {
        let initial = SequencePayloadLength::new(flags).unwrap();
        let framing = codec::seq_len_prefix_len_with_flags(1, flags)
            + codec::len_prefix_len_with_flags(usize::MAX - 32, flags);
        // Synthetic arithmetic boundaries are private test inputs, never public measured evidence.
        let last = initial.checked_append(usize::MAX - framing).unwrap();
        assert_eq!(last.len(), usize::MAX);
        assert!(matches!(last.checked_append(0), Err(Error::LengthMismatch)));
        assert_eq!(last.len(), usize::MAX);
        assert!(matches!(
            initial.checked_append(usize::MAX - framing + 1),
            Err(Error::LengthMismatch)
        ));
        assert!(matches!(
            initial.checked_append(usize::MAX),
            Err(Error::LengthMismatch)
        ));
    }
}

#[test]
fn checked_count_overflow_does_not_mutate() {
    let initial = SequencePayloadLength::new(default_encode_flags()).unwrap();
    let mut count_full = SequencePayloadLength {
        count: usize::MAX,
        ..initial
    };
    let snapshot = count_full;
    assert!(matches!(
        count_full.push(&Bytes(vec![])),
        Err(Error::LengthMismatch)
    ));
    assert_eq!(count_full, snapshot);
}

#[test]
fn serializer_error_precedes_arithmetic_failure_without_mutating_snapshot() {
    struct Fails;
    impl SerializePayload for Fails {
        fn serialize(&self, _: &mut Encoder<'_>) -> Result<(), Error> {
            Err(Error::NonCanonicalEncoding)
        }
    }
    let initial = SequencePayloadLength::new(default_encode_flags()).unwrap();
    let mut full = SequencePayloadLength {
        count: usize::MAX,
        ..initial
    };
    let snapshot = full;
    assert!(matches!(
        full.push(&Fails),
        Err(Error::NonCanonicalEncoding)
    ));
    assert_eq!(full, snapshot);
}
