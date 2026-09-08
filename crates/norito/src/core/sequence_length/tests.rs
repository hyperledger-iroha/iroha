//! Prefix equality, measurement isolation and arithmetic limits for sequence length observations.

use super::*;
use crate::core::{self as codec, Encoder};
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
    (0..=u8::MAX).filter(|flags| validate_header_flags(*flags).is_ok())
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
        assert_eq!(&actual[..8], &u64::try_from(count).unwrap().to_le_bytes());
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
        let packed = flags & header_flags::PACKED_SEQ != 0;
        let framing = if packed || flags & header_flags::COMPACT_LEN == 0 {
            8
        } else {
            1
        };
        assert_eq!(measured.len(), if packed { 16 } else { 8 });
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
    for flags in 0..=u8::MAX {
        assert_eq!(
            SequencePayloadLength::new(flags).is_ok(),
            validate_header_flags(flags).is_ok()
        );
    }
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
        let mut measured = SequencePayloadLength::new(flags).unwrap();
        for byte in [1_u8, 2, 3] {
            measured.push(&byte).unwrap();
        }
        let mut generic = Vec::new();
        codec::write_element_sequence::<u8, _>(
            &mut Encoder::for_buffer(&mut generic),
            [1_u8, 2, 3],
            u64::MAX,
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
fn incompatible_sequential_override_rejects_before_measurement() {
    struct Never;
    impl SerializePayload for Never {
        fn serialize(&self, _: &mut Encoder<'_>) -> Result<(), Error> {
            panic!("must reject before counting")
        }
    }
    for flags in layouts() {
        let mut measured = SequencePayloadLength::new(flags).unwrap();
        let snapshot = measured;
        {
            let _sequential = codec::SequentialOverrideGuard::enter();
            if flags == default_encode_flags() {
                let mut compatible = SequencePayloadLength::new(flags).unwrap();
                compatible.push(&1_u16).unwrap();
                assert_eq!(compatible.len(), 11);
            } else {
                assert!(matches!(
                    SequencePayloadLength::new(flags),
                    Err(Error::UnsupportedFeature("sequence length layout override"))
                ));
                assert!(matches!(
                    measured.push(&Never),
                    Err(Error::UnsupportedFeature("sequence length layout override"))
                ));
                assert_eq!(measured, snapshot);
            }
            assert!(matches!(
                SequencePayloadLength::new(0x80),
                Err(Error::UnsupportedFeature("layout flag"))
            ));
        }
        measured.push(&1_u16).unwrap();
        assert_eq!(measured.count(), 1);
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
            codec::note_fixed_offsets_emitted();
            codec::mark_field_bitset_used_if_encoding();
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
    let _payload = codec::PayloadCtxGuard::enter_with_flags(&payload, header_flags::PACKED_SEQ);
    let payload_context = codec::payload_ctx();
    let _outer_flags = DecodeFlagsGuard::enter(header_flags::PACKED_STRUCT);
    let _encode = codec::EncodeContextGuard::enter();
    codec::note_fixed_offsets_emitted();
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
            assert_eq!(codec::effective_layout_flags(), header_flags::PACKED_STRUCT);
            assert_eq!(codec::payload_ctx(), payload_context);
            assert!(codec::fixed_offsets_used());
            assert!(!codec::field_bitset_used());
            assert!(!codec::compact_len_used());
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
            codec::serialize_to_buffer(&vec![Growing(&calls)], &mut actual),
            Err(Error::LengthMismatch)
        ));
        assert_eq!(measured, snapshot);
        assert_eq!(calls.get(), 4);
    }
}

#[test]
fn checked_arithmetic_accepts_the_last_representable_length_then_rejects() {
    for flags in layouts() {
        let initial = SequencePayloadLength::new(flags).unwrap();
        let framing = if flags & header_flags::PACKED_SEQ != 0 {
            24
        } else {
            8 + codec::len_prefix_len_with_flags(usize::MAX - 32, flags)
        };
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
fn checked_count_payload_and_offset_table_overflows_do_not_mutate() {
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
    let payload_full = SequencePayloadLength {
        payload_bytes: usize::MAX,
        ..initial
    };
    assert!(matches!(
        payload_full.checked_append(1),
        Err(Error::LengthMismatch)
    ));
    let final_count = usize::MAX / 8 - 1;
    assert_eq!(
        packed_sequence_table_len(final_count).unwrap(),
        (final_count + 1) * 8
    );
    assert!(matches!(
        packed_sequence_table_len(final_count + 1),
        Err(Error::LengthMismatch)
    ));
    assert!(matches!(
        packed_sequence_table_len(usize::MAX),
        Err(Error::LengthMismatch)
    ));
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

#[test]
fn an_override_retained_by_the_serializer_cannot_change_frozen_framing() {
    struct RetainsOverride<'a> {
        guard: &'a std::cell::RefCell<Option<codec::SequentialOverrideGuard>>,
        calls: &'a Cell<usize>,
    }
    impl SerializePayload for RetainsOverride<'_> {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            self.calls.set(self.calls.get() + 1);
            *self.guard.borrow_mut() = Some(codec::SequentialOverrideGuard::enter());
            writer.write_all(&[1])?;
            Ok(())
        }
    }
    let guard = std::cell::RefCell::new(None);
    let calls = Cell::new(0);
    let mut measured = SequencePayloadLength::new(0).unwrap();
    let snapshot = measured;
    assert!(matches!(
        measured.push(&RetainsOverride {
            guard: &guard,
            calls: &calls
        }),
        Err(Error::UnsupportedFeature("sequence length layout override"))
    ));
    assert_eq!(calls.get(), 1);
    assert_eq!(measured, snapshot);
    // The serializer owns its retained side effect; measurement does not discard that guard.
    drop(guard.borrow_mut().take());
    measured.push(&1_u8).unwrap();
    assert_eq!(measured.len(), 17);
}
