//! Fixed framed records retain typed identity without per-record codec allocation.

use std::{
    cell::Cell,
    io::{self, Write},
};

use norito::core::{
    DecodeFlagsGuard, Encoder, Error, FixedFrameLayout, Header, SerializePayload,
    frame_bare_with_header_flags, header_flags, write_bare_frame_with_header_flags,
};

use super::allocations_during;

#[derive(norito::NoritoSchema)]
#[norito_schema(name = "norito.test.fixed_frame.RecordV1")]
struct Record([u8; 144]);

impl SerializePayload for Record {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        writer.write_all(&self.0)?;
        Ok(())
    }
}

#[derive(norito::NoritoSchema)]
#[norito_schema(name = "norito.test.fixed_frame.AlignedV1")]
#[repr(align(64))]
struct Aligned([u8; 144]);

impl SerializePayload for Aligned {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        writer.write_all(&self.0)?;
        Ok(())
    }
}

fn measured<T>(operation: impl FnOnce() -> T) -> T {
    let mut result = None;
    let allocations = allocations_during(|| result = Some(operation()));
    assert_eq!(allocations, 0, "fixed operation allocated codec storage");
    result.unwrap()
}

fn frame<T: norito::NoritoSerialize>(layout: &FixedFrameLayout<T>, payload: &[u8]) -> Vec<u8> {
    let mut result = vec![0; layout.frame_len()];
    let mut destination = result.as_mut_slice();
    measured(|| layout.write(&mut destination, payload)).unwrap();
    assert!(destination.is_empty());
    result
}

#[test]
fn fixed_frame_matches_existing_writers_and_the_declared_golden_header() {
    let payload = [0xa5; 144];
    let layout = FixedFrameLayout::<Record>::new(payload.len(), 0).unwrap();
    assert_eq!(layout.frame_len(), Header::SIZE + payload.len());
    let fixed = frame(&layout, &payload);
    let golden_header = [
        0x4e, 0x52, 0x54, 0x30, 0x00, 0x00, 0xda, 0x0d, 0x0c, 0x92, 0x53, 0x6d, 0x98, 0x76, 0x68,
        0xa8, 0x00, 0xee, 0x8c, 0x82, 0x58, 0x7a, 0x00, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x05, 0x7a, 0x08, 0x3d, 0x00, 0x0d, 0xde, 0xe1, 0x00,
    ];
    assert_eq!(&fixed[..Header::SIZE], &golden_header);
    assert_eq!(&fixed[Header::SIZE..], &payload);
    assert_eq!(fixed, norito::encode_canonical(&Record(payload)).unwrap());
    for flags in [
        0,
        header_flags::COMPACT_LEN,
        header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET,
    ] {
        let layout = FixedFrameLayout::<Record>::new(payload.len(), flags).unwrap();
        let fixed = frame(&layout, &payload);
        let buffered = frame_bare_with_header_flags::<Record>(&payload, flags).unwrap();
        let mut streamed = Vec::new();
        write_bare_frame_with_header_flags::<Record, _>(&mut streamed, &payload, flags).unwrap();
        assert_eq!(fixed, buffered);
        assert_eq!(fixed, streamed);
        let borrowed = measured(|| layout.payload(&fixed)).unwrap();
        assert_eq!(borrowed, &payload);
        assert_eq!(borrowed.as_ptr(), fixed[Header::SIZE..].as_ptr());
    }
}

#[test]
fn fixed_frame_rejects_every_truncation_and_suffix_without_allocation() {
    let layout = FixedFrameLayout::<Record>::new(144, 0).unwrap();
    let bytes = frame(&layout, &[17; 144]);
    for cut in 0..bytes.len() {
        assert!(matches!(
            measured(|| layout.payload(&bytes[..cut])),
            Err(Error::LengthMismatch)
        ));
    }
    for suffix in [0, 1, 255] {
        let mut longer = bytes.clone();
        longer.push(suffix);
        assert!(matches!(
            measured(|| layout.payload(&longer)),
            Err(Error::LengthMismatch)
        ));
    }
}

#[test]
fn fixed_frame_malformed_headers_and_payloads_have_fixed_allocation_free_errors() {
    let layout = FixedFrameLayout::<Aligned>::new(144, 0).unwrap();
    let original = frame(&layout, &[19; 144]);
    assert_eq!(layout.frame_len(), 64 + 144);
    for case in 0..15 {
        let mut bytes = original.clone();
        match case {
            0 => bytes[0] ^= 1,
            1 => bytes[4] ^= 1,
            2 => bytes[5] ^= 1,
            3 => bytes[6] ^= 1,
            4 => bytes[22] = 1,
            5 => bytes[22] = 255,
            6 => bytes[23..31].copy_from_slice(&143_u64.to_le_bytes()),
            7 => bytes[23..31].copy_from_slice(&145_u64.to_le_bytes()),
            8 => bytes[23..31].copy_from_slice(&u64::MAX.to_le_bytes()),
            9 => bytes[31] ^= 1,
            10 => bytes[39] = header_flags::COMPACT_LEN,
            11 => bytes[39] = 0x80,
            12 => bytes[39] = header_flags::FIELD_BITSET,
            13 => bytes[Header::SIZE] = 1,
            14 => *bytes.last_mut().unwrap() ^= 1,
            _ => unreachable!(),
        }
        let error = measured(|| layout.payload(&bytes)).unwrap_err();
        assert!(
            match case {
                0 => matches!(error, Error::InvalidMagic),
                1 => matches!(error, Error::UnsupportedVersion { .. }),
                2 => matches!(error, Error::UnsupportedMinorVersion { .. }),
                3 => matches!(error, Error::SchemaMismatch),
                4 | 5 => matches!(error, Error::UnsupportedCompression { .. }),
                6..=8 | 13 => matches!(error, Error::LengthMismatch),
                9 | 14 => matches!(error, Error::ChecksumMismatch),
                10 => matches!(error, Error::NonCanonicalEncoding),
                11 | 12 => matches!(error, Error::UnsupportedFeature(_)),
                _ => false,
            },
            "case {case}"
        );
    }
    // A foreign but otherwise valid typed frame cannot borrow this layout's authority.
    let foreign = frame_bare_with_header_flags::<Record>(&[19; 144], 0).unwrap();
    let byte_layout = FixedFrameLayout::<Record>::new(144, 0).unwrap();
    let mut substituted = foreign;
    substituted[6..22].copy_from_slice(&original[6..22]);
    assert!(matches!(
        measured(|| byte_layout.payload(&substituted)),
        Err(Error::SchemaMismatch)
    ));
}

#[test]
fn fixed_frame_accepts_unaligned_borrows_and_preserves_explicit_layout() {
    let payload = [23; 144];
    let layout = FixedFrameLayout::<Aligned>::new(payload.len(), 0).unwrap();
    let expected = frame_bare_with_header_flags::<Aligned>(&payload, 0).unwrap();
    let _ambient = DecodeFlagsGuard::enter(header_flags::COMPACT_LEN);
    assert_eq!(frame(&layout, &payload), expected);
    let mut storage = [0u8; 64 + 208];
    for offset in 0..64 {
        storage[offset..offset + expected.len()].copy_from_slice(&expected);
        let input = &storage[offset..offset + expected.len()];
        let borrowed = measured(|| layout.payload(input)).unwrap();
        assert_eq!(borrowed, &payload);
        assert_eq!(borrowed.as_ptr(), input[64..].as_ptr());
    }
    let canonical = norito::encode_canonical(&Aligned(payload)).unwrap();
    assert_eq!(expected, canonical);
}

struct PartialWriter {
    bytes: [u8; 256],
    written: usize,
    chunk: usize,
    fail_at: usize,
    zero: bool,
}

impl Write for PartialWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.written == self.fail_at {
            return if self.zero {
                Ok(0)
            } else {
                Err(io::Error::from_raw_os_error(5))
            };
        }
        let count = bytes.len().min(self.chunk).min(self.fail_at - self.written);
        self.bytes[self.written..self.written + count].copy_from_slice(&bytes[..count]);
        self.written += count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn fixed_frame_handles_partial_zero_and_error_writes_without_codec_allocation() {
    let layout = FixedFrameLayout::<Aligned>::new(144, 0).unwrap();
    let payload = [29; 144];
    let expected = frame(&layout, &payload);
    for zero in [false, true] {
        for cut in 0..expected.len() {
            let mut writer = PartialWriter {
                bytes: [0; 256],
                written: 0,
                chunk: 3,
                fail_at: cut,
                zero,
            };
            let error = measured(|| layout.write(&mut writer, &payload)).unwrap_err();
            let Error::Io(error) = error else {
                panic!("preserve writer error");
            };
            if zero {
                assert_eq!(error.kind(), io::ErrorKind::WriteZero);
            } else {
                assert_eq!(error.raw_os_error(), Some(5));
            }
            assert_eq!(writer.written, cut);
            assert_eq!(&writer.bytes[..cut], &expected[..cut]);
        }
    }
    for chunk in [1, 3, 17] {
        let mut writer = PartialWriter {
            bytes: [0; 256],
            written: 0,
            chunk,
            fail_at: 256,
            zero: false,
        };
        measured(|| layout.write(&mut writer, &payload)).unwrap();
        assert_eq!(&writer.bytes[..writer.written], &expected);
    }
}

#[test]
fn fixed_frame_rejects_wrong_payload_length_before_touching_the_writer() {
    let layout = FixedFrameLayout::<Record>::new(144, 0).unwrap();
    for length in [0, 143, 145] {
        let payload = [31; 145];
        let mut writer = PartialWriter {
            bytes: [0; 256],
            written: 0,
            chunk: 1,
            fail_at: 0,
            zero: false,
        };
        assert!(matches!(
            measured(|| layout.write(&mut writer, &payload[..length])),
            Err(Error::LengthMismatch)
        ));
        assert_eq!(writer.written, 0);
    }
}

thread_local! { static SCHEMA_CALLS: Cell<usize> = const { Cell::new(0) }; }
struct Counted;
impl norito::NoritoSchema for Counted {
    fn nominal_name() -> String {
        SCHEMA_CALLS.with(|calls| calls.set(calls.get() + 1));
        "norito.test.fixed_frame.CountedV1".to_owned()
    }
}
impl SerializePayload for Counted {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        writer.write_all(&[0; 3])?;
        Ok(())
    }
}

#[test]
fn fixed_frame_constructor_checks_bounds_before_resolving_one_cached_identity() {
    SCHEMA_CALLS.with(|calls| calls.set(0));
    assert!(matches!(
        measured(|| FixedFrameLayout::<Counted>::new(usize::MAX, 0)),
        Err(Error::LengthMismatch)
    ));
    assert!(matches!(
        measured(|| FixedFrameLayout::<Counted>::new(3, 0x80)),
        Err(Error::UnsupportedFeature(_))
    ));
    assert!(matches!(
        measured(|| FixedFrameLayout::<Counted>::new(3, header_flags::FIELD_BITSET)),
        Err(Error::UnsupportedFeature(_))
    ));
    let maximum = usize::try_from(norito::core::max_archive_len()).unwrap();
    if let Some(excess) = maximum
        .checked_add(1)
        .filter(|len| len.checked_add(Header::SIZE).is_some())
    {
        assert!(matches!(
            measured(|| FixedFrameLayout::<Counted>::new(excess, 0)),
            Err(Error::ArchiveLengthExceeded { .. })
        ));
    }
    assert_eq!(SCHEMA_CALLS.with(Cell::get), 0);
    let layout = FixedFrameLayout::<Counted>::new(3, 0).unwrap();
    assert_eq!(SCHEMA_CALLS.with(Cell::get), 1);
    for _ in 0..3 {
        let bytes = frame(&layout, &[0; 3]);
        assert_eq!(measured(|| layout.payload(&bytes)).unwrap(), &[0; 3]);
    }
    assert_eq!(SCHEMA_CALLS.with(Cell::get), 1);
}

#[test]
fn fixed_frame_zero_length_payload_has_exact_typed_padding_and_no_tail() {
    for aligned in [false, true] {
        let expected = if aligned {
            frame_bare_with_header_flags::<Aligned>(&[], 0).unwrap()
        } else {
            frame_bare_with_header_flags::<Record>(&[], 0).unwrap()
        };
        if aligned {
            let layout = FixedFrameLayout::<Aligned>::new(0, 0).unwrap();
            assert_eq!(layout.frame_len(), 64);
            assert_eq!(frame(&layout, &[]), expected);
            assert!(measured(|| layout.payload(&expected)).unwrap().is_empty());
        } else {
            let layout = FixedFrameLayout::<Record>::new(0, 0).unwrap();
            assert_eq!(layout.frame_len(), Header::SIZE);
            assert_eq!(frame(&layout, &[]), expected);
            assert!(measured(|| layout.payload(&expected)).unwrap().is_empty());
        }
    }
}
