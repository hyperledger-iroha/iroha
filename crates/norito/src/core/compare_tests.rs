//! Single-pass encoding and comparison against their streaming references.
//!
//! Buffers and comparisons frame each child from its own bytes; a streaming
//! writer measures first. These tests hold the single-pass paths to the
//! streaming bytes and, for comparisons, to the streaming verdict on every
//! single-byte change, truncation, extension, insertion and deletion of a
//! valid encoding, in both V1 layouts.

use std::collections::BTreeMap;

use super::*;

#[derive(Clone, crate::SerializePayload, crate::NoritoSchema)]
#[norito_schema(name = "norito.test.core.compare.Leaf")]
struct Leaf {
    id: u32,
    name: String,
    tags: Vec<String>,
    memo: Option<String>,
    key: [u8; 4],
}

#[derive(Clone, crate::SerializePayload, crate::NoritoSchema)]
#[norito_schema(name = "norito.test.core.compare.Choice")]
enum Choice {
    Unit,
    One(Leaf),
    Pair { left: u64, right: Option<Leaf> },
}

/// A leaf carried as a nested frame behind a fixed `u64` length, the way
/// instructions are carried.
#[derive(Clone)]
struct Embedded(Leaf);

impl SerializePayload for Embedded {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        write_frame_with_prefix(&self.0, writer, |writer, length| {
            write_seq_len(
                writer,
                u64::try_from(length).map_err(|_| Error::LengthMismatch)?,
            )?;
            Ok(())
        })
    }
}

#[derive(Clone, crate::SerializePayload, crate::NoritoSchema)]
#[norito_schema(name = "norito.test.core.compare.Record")]
struct Record {
    choices: Vec<Choice>,
    map: BTreeMap<String, u16>,
    nested: Option<Box<Leaf>>,
    bytes: Vec<u8>,
    elements: Vec<u8>,
    embedded: Vec<Embedded>,
}

fn leaf(seed: u8, name_len: usize) -> Leaf {
    Leaf {
        id: u32::from(seed) * 7,
        name: "n".repeat(name_len),
        tags: (0..seed % 3).map(|tag| format!("tag{tag}")).collect(),
        memo: seed.is_multiple_of(2).then(|| "memo".to_owned()),
        key: [seed; 4],
    }
}

/// Values covering empty containers, every enum shape, maps, boxes, raw and
/// element-framed bytes, nested frames and children of at least 128 bytes,
/// whose compact prefixes take two bytes.
fn records() -> Vec<Record> {
    vec![
        Record {
            choices: Vec::new(),
            map: BTreeMap::new(),
            nested: None,
            bytes: Vec::new(),
            elements: Vec::new(),
            embedded: Vec::new(),
        },
        Record {
            choices: vec![
                Choice::Unit,
                Choice::One(leaf(1, 3)),
                Choice::Pair {
                    left: u64::MAX,
                    right: Some(leaf(2, 0)),
                },
                Choice::Pair {
                    left: 5,
                    right: None,
                },
            ],
            map: [("a".to_owned(), 1), ("bb".to_owned(), 2)].into(),
            nested: Some(Box::new(leaf(4, 2))),
            bytes: vec![9; 3],
            elements: vec![1, 2, 3],
            embedded: vec![Embedded(leaf(5, 1))],
        },
        Record {
            choices: vec![Choice::One(leaf(6, 200))],
            map: [("k".repeat(130), 7)].into(),
            nested: Some(Box::new(leaf(8, 140))),
            bytes: vec![0xa5; 300],
            elements: vec![0x5a; 130],
            embedded: vec![Embedded(leaf(10, 150)), Embedded(leaf(11, 0))],
        },
    ]
}

/// The verdict a verification call site reaches from a comparison.
fn verdict(result: Result<(), Error>, exact: &ExactSliceWriter<'_>) -> String {
    if exact.mismatched() {
        return "mismatch".to_owned();
    }
    if let Err(error) = result {
        return format!("error {error:?}");
    }
    if exact.is_complete() {
        "match".to_owned()
    } else {
        format!("prefix of {}", exact.matched_len())
    }
}

/// Valid `encoding` and its single-byte changes, truncations, extensions,
/// insertions and deletions.
fn variants(encoding: &[u8]) -> Vec<Vec<u8>> {
    let mut variants = vec![encoding.to_vec()];
    for index in 0..encoding.len() {
        for replacement in [encoding[index] ^ 0x01, encoding[index] ^ 0x80, 0x00, 0xff] {
            if replacement != encoding[index] {
                let mut changed = encoding.to_vec();
                changed[index] = replacement;
                variants.push(changed);
            }
        }
        variants.push(encoding[..index].to_vec());
        let mut inserted = encoding.to_vec();
        inserted.insert(index, 0x01);
        variants.push(inserted);
        let mut deleted = encoding.to_vec();
        deleted.remove(index);
        variants.push(deleted);
    }
    for suffix in [&[0x00][..], &[0x01, 0x02, 0x03]] {
        let mut extended = encoding.to_vec();
        extended.extend_from_slice(suffix);
        variants.push(extended);
    }
    variants
}

#[test]
fn buffers_place_frames_and_prefixes_exactly_where_streaming_writes_them() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for record in records() {
            let mut buffered = Vec::new();
            serialize_to_buffer(&record, &mut buffered).unwrap();
            let mut streamed = Vec::new();
            serialize_to_writer(&record, &mut streamed).unwrap();
            assert_eq!(buffered, streamed, "payload, flags {flags:#x}");
            assert_eq!(
                encoded_payload_len(&record).unwrap(),
                streamed.len(),
                "counted length, flags {flags:#x}"
            );
            let frame = to_bytes(&record).unwrap();
            let mut streamed_frame = Vec::new();
            write_frame_to_writer(&record, &mut streamed_frame).unwrap();
            assert_eq!(frame, streamed_frame, "frame, flags {flags:#x}");
        }
    }
}

#[test]
fn payload_comparison_reaches_the_streaming_verdict_on_every_variant() {
    let mut cases = 0;
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for record in records() {
            let mut encoding = Vec::new();
            serialize_to_writer(&record, &mut encoding).unwrap();
            for expected in variants(&encoding) {
                let mut streamed = ExactSliceWriter::new(&expected);
                let streamed_result = serialize_to_writer(&record, &mut streamed);
                let (result, compared) = compare_payload(&record, &expected);
                assert_eq!(
                    verdict(result, &compared),
                    verdict(streamed_result, &streamed),
                    "flags {flags:#x}, expected {expected:02x?}"
                );
                cases += 1;
            }
        }
    }
    assert!(cases > 10_000, "only {cases} variants were compared");
}

#[test]
fn frame_comparison_reaches_the_streaming_verdict_on_every_variant() {
    let mut cases = 0;
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for record in records() {
            let mut encoding = Vec::new();
            encode_frames::write_typed_payload_frame::<Record, Record, _>(
                &record,
                &mut encoding,
                flags,
            )
            .unwrap();
            for expected in variants(&encoding) {
                let mut streamed = ExactSliceWriter::new(&expected);
                let streamed_result = encode_frames::write_typed_payload_frame::<Record, Record, _>(
                    &record,
                    &mut streamed,
                    flags,
                );
                let (result, compared) =
                    compare_typed_frame::<Record, Record>(&record, &expected, flags);
                assert_eq!(
                    verdict(result, &compared),
                    verdict(streamed_result, &streamed),
                    "flags {flags:#x}, expected {expected:02x?}"
                );
                cases += 1;
            }
        }
    }
    assert!(cases > 10_000, "only {cases} variants were compared");
}

#[test]
fn comparisons_keep_the_measured_path_inside_bounded_encode_work() {
    let record = records().pop().unwrap();
    let encoding = to_bytes(&record).unwrap();
    let limits = EncodeWorkLimits {
        max_frame_bytes: encoding.len(),
        max_byte_visits: usize::MAX,
        max_operations: usize::MAX,
        max_depth: MAX_VALUE_NESTING_DEPTH,
    };
    let verdicts = with_bounded_encode_work(limits, || {
        let (result, exact) =
            compare_typed_frame::<Record, Record>(&record, &encoding, default_encode_flags());
        Ok(verdict(result, &exact))
    });
    assert!(matches!(verdicts, Ok(ref verdict) if verdict == "match"));
}
