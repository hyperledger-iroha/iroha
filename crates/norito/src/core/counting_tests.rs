//! Traversal, destination isolation and error contracts for measured encoding.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct Leaf<'a>(&'a AtomicUsize);

impl NoritoSerialize for Leaf<'_> {}
impl SerializePayload for Leaf<'_> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.fetch_add(1, Ordering::Relaxed);
        writer.write_all(&[0xAB])?;
        Ok(())
    }
}

#[derive(crate::SerializePayload)]
struct Layer<T> {
    value: T,
}

fn layouts() -> impl Iterator<Item = u8> {
    (0..=supported_header_flags()).filter(|flags| validate_header_flags(*flags).is_ok())
}

#[test]
fn nested_counting_visits_each_leaf_once_in_every_layout() {
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        let calls = AtomicUsize::new(0);
        let value = Layer {
            value: vec![Layer {
                value: BTreeMap::from([(
                    7_u8,
                    Layer {
                        value: Some(Box::new(Layer {
                            value: vec![Layer {
                                value: Leaf(&calls),
                            }],
                        })),
                    },
                )]),
            }],
        };
        let measured = encoded_payload_len(&value).expect("measure nested payload");
        assert_eq!(
            calls.load(Ordering::Relaxed),
            1,
            "one leaf measurement for flags {flags:#x}"
        );
        calls.store(0, Ordering::Relaxed);
        let mut bytes = Vec::new();
        serialize_to_buffer(&value, &mut bytes).expect("write measured payload");
        assert_eq!(measured, bytes.len());
        // Each containing frame may measure its descendant once while emitting
        // actual bytes. The work is bounded by nesting depth, never 2^depth.
        assert!(
            calls.load(Ordering::Relaxed) <= 12,
            "leaf traversals: {} for {flags:#x}",
            calls.load(Ordering::Relaxed)
        );
    }
}

#[test]
fn counting_preserves_box_rc_arc_and_array_layouts() {
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        let calls = AtomicUsize::new(0);
        let boxed = Box::new(Leaf(&calls));
        let rc = Rc::new(Leaf(&calls));
        let arc = Arc::new(Leaf(&calls));
        let array = [Leaf(&calls), Leaf(&calls)];
        for (value, leaves) in [
            (&boxed as &dyn SerializePayload, 1),
            (&rc as &dyn SerializePayload, 1),
            (&arc as &dyn SerializePayload, 1),
            (&array as &dyn SerializePayload, 2),
        ] {
            calls.store(0, Ordering::Relaxed);
            let measured = encoded_payload_len(value).unwrap();
            assert_eq!(calls.load(Ordering::Relaxed), leaves);
            let mut bytes = Vec::new();
            serialize_to_buffer(value, &mut bytes).unwrap();
            assert_eq!(measured, bytes.len());
        }
    }
}

#[test]
fn counting_never_trusts_public_exact_writer_lengths() {
    struct FalseLength;
    impl NoritoSerialize for FalseLength {}
    impl SerializePayload for FalseLength {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            serialize_to_writer_exact(&0x1234_u16, writer, 1)
        }
    }
    assert!(matches!(
        encoded_payload_len(&FalseLength),
        Err(Error::LengthMismatch)
    ));
}

#[test]
fn nested_buffer_and_checksum_writers_still_receive_real_bytes() {
    struct InnerDigest;
    impl NoritoSerialize for InnerDigest {}
    impl SerializePayload for InnerDigest {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            let mut inner = Vec::new();
            serialize_to_buffer(&Some(0x1234_u16), &mut inner)?;
            assert_eq!(inner, [1, 2, 0x34, 0x12]);
            let mut exact = ExactSliceWriter::new(&inner);
            serialize_to_writer(&Some(0x1234_u16), &mut exact)?;
            assert!(exact.is_complete());
            writer.write_all(&crc64(&inner).to_le_bytes())?;
            Ok(())
        }
    }
    let _flags = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    let measured = encoded_payload_len(&Some(InnerDigest)).unwrap();
    let mut bytes = Vec::new();
    serialize_to_buffer(&Some(InnerDigest), &mut bytes).unwrap();
    assert_eq!(measured, bytes.len());
    assert_eq!(&bytes[2..], crc64(&[1, 2, 0x34, 0x12]).to_le_bytes());
}

#[test]
fn counting_nested_frames_preserves_streamed_checksums_and_layout_flags() {
    #[derive(crate::Encode, crate::Decode, Debug, PartialEq)]
    #[cfg_attr(feature = "schema-structural", derive(::iroha_schema::IntoSchema))]
    struct Inner {
        values: Vec<Vec<u16>>,
    }
    struct NestedFrame(Inner);
    impl NoritoSerialize for NestedFrame {}
    impl SerializePayload for NestedFrame {
        fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
            // The inner frame's first pass computes its CRC through a writer
            // over io::sink(), even when this outer destination only counts.
            write_frame_to_writer(&self.0, writer)
        }
    }
    let value = NestedFrame(Inner {
        values: vec![vec![0x1234, 0x5678]],
    });
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        let measured = encoded_payload_len(&value).unwrap();
        let mut frame = Vec::new();
        serialize_to_buffer(&value, &mut frame).unwrap();
        assert_eq!(measured, frame.len());
        let canonical = to_bytes(&value.0).unwrap();
        assert_eq!(frame, canonical, "nested frame flags {flags:#x}");
        let header = Header::read(&mut &frame[..]).unwrap();
        let expected_header = Header::read(&mut &canonical[..]).unwrap();
        let start = Header::SIZE + payload_alignment_padding_for::<Inner>();
        assert_eq!(header.checksum, crc64(&frame[start..]));
        assert_eq!(header.flags, expected_header.flags);
        // A complete nested frame owns its finalized layout. Unused ambient
        // flags may be absent from its header (e.g. compact lengths in a wholly
        // fixed-offset payload), so decode under that advertised context.
        let _frame_flags = DecodeFlagsGuard::enter(header.flags);
        let archived = from_bytes::<Inner>(&frame).unwrap();
        assert_eq!(Inner::try_deserialize(archived).unwrap(), value.0);
    }
}

#[test]
fn counting_propagates_child_errors_and_restores_context() {
    struct Fails;
    impl NoritoSerialize for Fails {}
    impl SerializePayload for Fails {
        fn serialize(&self, _: &mut Encoder<'_>) -> Result<(), Error> {
            Err(Error::NonCanonicalEncoding)
        }
    }
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        for value in [
            &Some(Fails) as &dyn SerializePayload,
            &vec![Fails] as &dyn SerializePayload,
            &BTreeMap::from([(0_u8, Fails)]) as &dyn SerializePayload,
        ] {
            assert!(matches!(
                encoded_payload_len(value),
                Err(Error::NonCanonicalEncoding)
            ));
        }
        assert_eq!(encoded_payload_len(&7_u8).unwrap(), 1);
        assert_eq!(effective_layout_flags(), flags);
    }
}

#[test]
fn count_overflow_is_sticky_even_if_a_serializer_ignores_it() {
    let mut counter = LengthCountingWriter::default();
    {
        let mut encoder = Encoder::for_counting(&mut counter);
        assert!(encoder.count_measured_bytes(usize::MAX).unwrap());
        assert!(encoder.count_measured_bytes(1).is_err());
        assert!(encoder.count_measured_bytes(0).is_err());
        assert!(encoder.write_all(&[]).is_err());
    }
    assert!(matches!(counter.finish(), Err(Error::LengthMismatch)));
}

#[test]
fn element_sequence_bound_includes_offsets_and_rejects_before_the_table() {
    let _flags = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
    let items = [1_u16, 2, 3];
    let limit = 4 * 8 + 3 * 2;
    let mut bytes = Vec::new();
    write_element_sequence::<u16, _>(&mut Encoder::for_buffer(&mut bytes), items.iter(), limit)
        .unwrap();
    assert_eq!(bytes.len(), 8 + usize::try_from(limit).unwrap());
    let mut rejected = Vec::new();
    assert!(matches!(
        write_element_sequence::<u16, _>(&mut Encoder::for_buffer(&mut rejected), items.iter(), limit - 1),
        Err(Error::ArchiveLengthExceeded { length, limit: bound })
            if length == limit && bound == limit - 1
    ));
    assert_eq!(rejected, 3_u64.to_le_bytes());
}

#[test]
fn element_sequence_rejects_oversized_offset_tables_before_visiting_elements() {
    let _flags = DecodeFlagsGuard::enter(header_flags::PACKED_SEQ);
    let calls = AtomicUsize::new(0);
    let items = [Leaf(&calls)];
    let mut bytes = Vec::new();
    assert!(matches!(
        write_element_sequence::<Leaf<'_>, _>(
            &mut Encoder::for_buffer(&mut bytes),
            items.iter(),
            15
        ),
        Err(Error::ArchiveLengthExceeded {
            length: 16,
            limit: 15
        })
    ));
    assert_eq!(calls.load(Ordering::Relaxed), 0);
    assert_eq!(bytes, 1_u64.to_le_bytes());
}

#[test]
fn element_sequence_keeps_individually_framed_bytes_in_every_layout() {
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        let mut bytes = Vec::new();
        write_element_sequence::<u8, _>(&mut Encoder::for_buffer(&mut bytes), [0xAB_u8], u64::MAX)
            .unwrap();
        let mut expected = 1_u64.to_le_bytes().to_vec();
        if use_packed_seq() {
            expected.extend_from_slice(&0_u64.to_le_bytes());
            expected.extend_from_slice(&1_u64.to_le_bytes());
        } else {
            write_len_with_flags(&mut expected, 1, flags).unwrap();
        }
        expected.push(0xAB);
        assert_eq!(bytes, expected, "flags {flags:#x}");
    }
}
