//! The dense layout (`norito.md`, "Dense layout (header minor 1)").
//!
//! Golden bytes pin every dense rule next to the current compact bytes of the
//! same value. Round trips, cross-layout equivalence and rejections cover the
//! frame and bare entry points. The single-pass comparison paths run under the
//! dense layout in `compare_tests`.

use std::collections::BTreeMap;

use super::*;
use crate::{NoritoDeserialize, NoritoSerialize, codec};

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, crate::NoritoSchema)]
#[norito_schema(name = "norito.test.core.dense.Leaf")]
struct Leaf {
    id: u32,
    name: String,
    key: [u8; 4],
    digest: Option<[u8; 4]>,
    blob: Vec<u8>,
    keys: Vec<[u8; 2]>,
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, crate::NoritoSchema)]
#[norito_schema(name = "norito.test.core.dense.Choice")]
enum Choice {
    Unit,
    One(Leaf),
    #[codec(index = 300)]
    Far {
        value: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, crate::NoritoSchema)]
#[norito(decode_from_slice)]
#[norito_schema(name = "norito.test.core.dense.Record")]
struct Record {
    choices: Vec<Choice>,
    map: BTreeMap<String, u16>,
    wide: u128,
    nested: Option<Box<Leaf>>,
    tags: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, crate::NoritoSchema)]
#[norito_schema(name = "norito.test.core.dense.KeyField")]
struct KeyField {
    key: [u8; 4],
}

fn leaf(seed: u8, blob_len: usize) -> Leaf {
    Leaf {
        id: u32::from(seed) * 1000,
        name: format!("leaf-{seed}"),
        key: [seed; 4],
        digest: seed.is_multiple_of(2).then_some([seed ^ 0x5a; 4]),
        blob: (0..blob_len).map(|index| (index as u8) ^ seed).collect(),
        keys: (0..seed % 4).map(|index| [index, seed]).collect(),
    }
}

/// Values with empty containers, every enum shape (including a tag above 127),
/// maps, boxes, byte vectors of every count width, element-framed arrays and a
/// 16-aligned field.
fn records() -> Vec<Record> {
    vec![
        Record {
            choices: Vec::new(),
            map: BTreeMap::new(),
            wide: 0,
            nested: None,
            tags: Vec::new(),
        },
        Record {
            choices: vec![
                Choice::Unit,
                Choice::One(leaf(1, 3)),
                Choice::Far { value: u64::MAX },
                Choice::One(leaf(2, 0)),
            ],
            map: [("a".to_owned(), 1), ("bb".to_owned(), 2)].into(),
            wide: u128::MAX,
            nested: Some(Box::new(leaf(4, 200))),
            tags: vec!["x".to_owned(), String::new()],
        },
        Record {
            choices: (0..130).map(|index| Choice::Far { value: index }).collect(),
            map: [("k".repeat(130), 7)].into(),
            wide: 1 << 100,
            nested: Some(Box::new(leaf(6, 70_000))),
            tags: (0..129).map(|index| index.to_string()).collect(),
        },
    ]
}

fn dense<T: SerializePayload>(value: &T) -> Vec<u8> {
    codec::encode_bare_with_layout(value, WireLayout::Dense).expect("dense bare encoding")
}

fn compact<T: SerializePayload>(value: &T) -> Vec<u8> {
    codec::encode_bare_with_layout(value, WireLayout::Compact).expect("compact bare encoding")
}

#[test]
fn byte_vectors_carry_a_varint_count_and_raw_bytes() {
    let value = vec![1_u8, 2, 3];
    assert_eq!(dense(&value), [3, 1, 2, 3]);
    assert_eq!(compact(&value), [3, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3]);
    let long = vec![0xee_u8; 300];
    let encoded = dense(&long);
    assert_eq!(encoded[..2], [0xac, 0x02]);
    assert_eq!(encoded.len(), 302);
    assert!(encoded[2..].iter().all(|&byte| byte == 0xee));
}

#[test]
fn fixed_byte_arrays_are_raw_wherever_they_appear() {
    assert_eq!(dense(&[1_u8, 2, 3, 4]), [1, 2, 3, 4]);
    assert_eq!(compact(&[1_u8, 2, 3, 4]), [1, 1, 1, 2, 1, 3, 1, 4]);
    let some = Some([9_u8; 4]);
    assert_eq!(dense(&some), [1, 4, 9, 9, 9, 9]);
    assert_eq!(compact(&some), [1, 8, 1, 9, 1, 9, 1, 9, 1, 9]);
    let keys = vec![[7_u8; 2], [8_u8; 2]];
    assert_eq!(dense(&keys), [2, 2, 7, 7, 2, 8, 8]);
    assert_eq!(
        compact(&keys),
        [2, 0, 0, 0, 0, 0, 0, 0, 4, 1, 7, 1, 7, 4, 1, 8, 1, 8]
    );
    // A record field keeps its own field prefix in both layouts.
    let field = KeyField { key: [1, 2, 3, 4] };
    assert_eq!(dense(&field), [4, 1, 2, 3, 4]);
    assert_eq!(compact(&field), [4, 1, 2, 3, 4]);
}

#[test]
fn sequence_and_map_counts_are_varints() {
    let tags = vec!["ab".to_owned()];
    assert_eq!(dense(&tags), [1, 3, 2, b'a', b'b']);
    assert_eq!(compact(&tags), [1, 0, 0, 0, 0, 0, 0, 0, 3, 2, b'a', b'b']);
    let map: BTreeMap<String, u16> = [("k".to_owned(), 5)].into();
    assert_eq!(dense(&map), [1, 2, 1, b'k', 2, 5, 0]);
    assert_eq!(compact(&map), [1, 0, 0, 0, 0, 0, 0, 0, 2, 1, b'k', 2, 5, 0]);
    let empty: Vec<String> = Vec::new();
    assert_eq!(dense(&empty), [0]);
}

#[test]
fn enum_tags_are_varints_of_the_discriminant() {
    assert_eq!(dense(&Choice::Unit), [0]);
    assert_eq!(compact(&Choice::Unit), [0, 0, 0, 0]);
    let far = Choice::Far { value: 1 };
    assert_eq!(dense(&far), [0xac, 0x02, 8, 1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(compact(&far), [0x2c, 0x01, 0, 0, 8, 1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(dense(&Some(true)), [1, 1, 1]);
    let result: Result<u8, u8> = Err(7);
    assert_eq!(dense(&result), [1, 1, 7]);
}

#[test]
fn dense_frames_declare_minor_one_with_no_flags_and_no_padding() {
    let record = records().swap_remove(1);
    let frame = crate::encode_canonical_with_layout(&record, WireLayout::Dense).unwrap();
    let payload = dense(&record);
    assert_eq!(frame[..4], MAGIC);
    assert_eq!(frame[4], VERSION_MAJOR);
    assert_eq!(frame[5], VERSION_MINOR_DENSE);
    assert_eq!(frame[Header::SIZE - 1], 0);
    assert_eq!(frame.len(), Header::SIZE + payload.len());
    assert_eq!(frame[Header::SIZE..], payload[..]);
    let header = Header::read(&frame[..]).unwrap();
    assert_eq!(header.layout(), WireLayout::Dense);
    assert_eq!(
        crate::canonical_frame_len_with_layout(&record, WireLayout::Dense).unwrap(),
        frame.len()
    );

    // The current layout pads a 16-aligned record and keeps minor 0.
    let current = crate::encode_canonical(&record).unwrap();
    assert_eq!(current[5], VERSION_MINOR);
    assert_eq!(current[Header::SIZE - 1], header_flags::COMPACT_LEN);
    let padding = payload_alignment_padding_for::<Record>();
    assert_eq!(
        current.len(),
        Header::SIZE + padding + compact(&record).len()
    );
    assert_eq!(
        crate::encode_canonical_with_layout(&record, WireLayout::Compact).unwrap(),
        current
    );
}

#[test]
fn every_value_round_trips_in_both_layouts_and_dense_is_never_larger() {
    for record in records() {
        let frame = crate::encode_canonical_with_layout(&record, WireLayout::Dense).unwrap();
        let decoded: Record = crate::decode_canonical_with_layout(&frame, WireLayout::Dense)
            .expect("dense canonical decode");
        assert_eq!(decoded, record);
        let viewed: Record = decode_from_bytes(&frame).expect("dense frame through a view");
        assert_eq!(viewed, record);
        let bare = dense(&record);
        let decoded: Record =
            codec::decode_bare_with_layout(&bare, WireLayout::Dense).expect("dense bare decode");
        assert_eq!(decoded, record);
        let current = crate::encode_canonical(&record).unwrap();
        let decoded: Record = crate::decode_canonical(&current).unwrap();
        assert_eq!(decoded, record);
        assert!(frame.len() <= current.len());
        assert_eq!(
            encoded_payload_len_in(&record, WireLayout::Dense),
            bare.len()
        );
        codec::verify_exact_payload_with_layout(&record, &bare, WireLayout::Dense).unwrap();
        crate::verify_exact_canonical_frame_with_layout(&record, &frame, WireLayout::Dense)
            .unwrap();
    }
}

#[test]
fn element_framed_arrays_shrink_by_half() {
    // Keys and digests travel as fixed arrays; the current layout frames
    // every byte. `Vec<u8>` was already raw, so only its count shrinks.
    let keys: Vec<[u8; 32]> = (0..8).map(|index| [index; 32]).collect();
    assert_eq!(compact(&keys).len(), 8 + 8 * (1 + 64));
    assert_eq!(dense(&keys).len(), 1 + 8 * (1 + 32));
    let blob = vec![0_u8; 200];
    assert_eq!(compact(&blob).len(), 8 + 200);
    assert_eq!(dense(&blob).len(), 2 + 200);
}

fn encoded_payload_len_in(value: &dyn SerializePayload, layout: WireLayout) -> usize {
    let _layout = WireLayoutGuard::enter(layout);
    encoded_payload_len(value).expect("counted dense payload")
}

#[test]
fn a_frame_never_decodes_as_the_other_layout() {
    let record = records().swap_remove(1);
    let dense_frame = crate::encode_canonical_with_layout(&record, WireLayout::Dense).unwrap();
    let current = crate::encode_canonical(&record).unwrap();
    assert!(crate::decode_canonical::<Record>(&dense_frame).is_err());
    assert!(matches!(
        crate::decode_canonical_with_layout::<Record>(&current, WireLayout::Dense),
        Err(Error::NonCanonicalEncoding)
    ));
    assert!(crate::verify_exact_canonical_frame(&record, &dense_frame).is_err());
    assert!(
        crate::verify_exact_canonical_frame_with_layout(&record, &current, WireLayout::Dense)
            .is_err()
    );
}

#[test]
fn dense_headers_reject_flags_unknown_minors_and_padding() {
    let record = records().swap_remove(1);
    let frame = crate::encode_canonical_with_layout(&record, WireLayout::Dense).unwrap();

    let mut flagged = frame.clone();
    flagged[Header::SIZE - 1] = header_flags::COMPACT_LEN;
    assert!(matches!(
        Header::read(&flagged[..]),
        Err(Error::UnsupportedFeature("layout flag"))
    ));
    assert!(decode_from_bytes::<Record>(&flagged).is_err());

    let mut future = frame.clone();
    future[5] = VERSION_MINOR_DENSE + 1;
    assert!(matches!(
        Header::read(&future[..]),
        Err(Error::UnsupportedMinorVersion { .. })
    ));

    let mut padded = frame[..Header::SIZE].to_vec();
    padded.extend_from_slice(&[0; 8]);
    padded.extend_from_slice(&frame[Header::SIZE..]);
    assert!(from_bytes_view(&padded).is_err());
    assert!(decode_from_bytes::<Record>(&padded).is_err());
    assert!(crate::decode_canonical_with_layout::<Record>(&padded, WireLayout::Dense).is_err());

    assert_eq!(
        WireLayout::from_header(VERSION_MINOR_DENSE, 0).unwrap(),
        WireLayout::Dense
    );
    assert!(WireLayout::from_header(VERSION_MINOR_DENSE, 0x80).is_err());
    assert!(WireLayout::from_header(VERSION_MINOR, 0x80).is_err());
}

#[test]
fn dense_counts_tags_and_arrays_reject_noncanonical_and_short_input() {
    let bare = |bytes: &[u8]| codec::decode_bare_with_layout::<Vec<u8>>(bytes, WireLayout::Dense);
    assert_eq!(bare(&[3, 1, 2, 3]).unwrap(), [1, 2, 3]);
    // Overlong count, truncated body, trailing byte.
    assert!(bare(&[0x83, 0x00, 1, 2, 3]).is_err());
    assert!(bare(&[3, 1, 2]).is_err());
    assert!(bare(&[3, 1, 2, 3, 0]).is_err());

    let choice = |bytes: &[u8]| codec::decode_bare_with_layout::<Choice>(bytes, WireLayout::Dense);
    assert_eq!(choice(&[0]).unwrap(), Choice::Unit);
    // Overlong tag, unknown tag, tag beyond `u32`, compact-layout tag width.
    assert!(choice(&[0x80, 0x00]).is_err());
    assert!(choice(&[2]).is_err());
    assert!(choice(&[0x80, 0x80, 0x80, 0x80, 0x10]).is_err());
    assert!(choice(&[0, 0, 0, 0]).is_err());

    let array = |bytes: &[u8]| codec::decode_bare_with_layout::<[u8; 4]>(bytes, WireLayout::Dense);
    assert_eq!(array(&[1, 2, 3, 4]).unwrap(), [1, 2, 3, 4]);
    assert!(array(&[1, 2, 3]).is_err());
    assert!(array(&[1, 1, 1, 2, 1, 3, 1, 4]).is_err());
}

#[test]
fn layout_codes_round_trip_and_guards_restore_the_previous_selection() {
    for layout in [WireLayout::Fixed, WireLayout::Compact, WireLayout::Dense] {
        assert_eq!(WireLayout::from_code(layout.code()), layout);
        validate_layout_code(layout.code()).unwrap();
    }
    assert!(validate_layout_code(DENSE_LAYOUT_BIT).is_err());
    assert_eq!(effective_layout(), WireLayout::Compact);
    {
        let _dense = WireLayoutGuard::enter(WireLayout::Dense);
        assert!(dense_layout_active());
        {
            let _fixed = WireLayoutGuard::enter(WireLayout::Fixed);
            assert_eq!(effective_layout(), WireLayout::Fixed);
        }
        assert_eq!(effective_layout(), WireLayout::Dense);
    }
    assert!(!dense_layout_active());
}

#[test]
fn explicit_layout_readers_refuse_a_disagreeing_context() {
    let strings = vec!["a".to_owned(), "bc".to_owned()];
    let dense_bytes = dense(&strings);
    let compact_bytes = compact(&strings);
    {
        let _dense = WireLayoutGuard::enter(WireLayout::Dense);
        let plan = plan_binary_sequence(&dense_bytes, DENSE_LAYOUT_CODE).unwrap();
        assert_eq!(plan.used, dense_bytes.len());
        assert_eq!(plan.spans.len(), 2);
        // A compact code under a dense context would read a varint count
        // followed by compact element prefixes of another layout.
        assert!(matches!(
            plan_binary_sequence(&dense_bytes, header_flags::COMPACT_LEN),
            Err(Error::DecodeFlagsMismatch { .. })
        ));
    }
    let plan = plan_binary_sequence(&compact_bytes, header_flags::COMPACT_LEN).unwrap();
    assert_eq!(plan.used, compact_bytes.len());
    assert!(matches!(
        plan_binary_sequence(&compact_bytes, DENSE_LAYOUT_CODE),
        Err(Error::DecodeFlagsMismatch { .. })
    ));
}

#[test]
fn only_the_exact_dense_code_selects_the_dense_layout() {
    for byte in 0..=u8::MAX {
        let expected = if byte == DENSE_LAYOUT_CODE {
            WireLayout::Dense
        } else if byte & header_flags::COMPACT_LEN != 0 {
            WireLayout::Compact
        } else {
            WireLayout::Fixed
        };
        assert_eq!(WireLayout::from_code(byte), expected, "code {byte:#04x}");
        let _guard = DecodeFlagsGuard::enter(byte);
        assert_eq!(effective_layout(), expected, "guard {byte:#04x}");
    }
    // A reserved flag byte cannot frame a payload as dense.
    let payload = dense(&vec![1_u8, 2, 3]);
    for byte in [DENSE_LAYOUT_BIT, 0x81, 0x83, 0xff] {
        assert!(frame_bare_with_header_flags::<Vec<u8>>(&payload, byte).is_err());
    }
    let framed = frame_bare_with_header_flags::<Vec<u8>>(&payload, DENSE_LAYOUT_CODE).unwrap();
    assert_eq!(framed[5], VERSION_MINOR_DENSE);
    assert_eq!(decode_from_bytes::<Vec<u8>>(&framed).unwrap(), [1, 2, 3]);
}

#[test]
fn dense_size_hints_equal_the_bytes_written() {
    let _dense = WireLayoutGuard::enter(WireLayout::Dense);
    let far = Choice::Far { value: 1 };
    assert_eq!(far.encoded_len_exact(), Some(dense(&far).len()));
    assert_eq!(far.encoded_len_hint(), Some(dense(&far).len()));
    assert_eq!(Choice::Unit.encoded_len_exact(), Some(1));
    let one = Choice::One(leaf(3, 5));
    if let Some(exact) = one.encoded_len_exact() {
        assert_eq!(exact, dense(&one).len());
    }
    let array = [5_u8; 32];
    assert_eq!(array.encoded_len_exact(), Some(32));
    let keys = vec![[1_u8; 2]; 3];
    assert_eq!(keys.encoded_len_exact(), Some(dense(&keys).len()));
}

#[test]
fn dense_byte_element_sequences_decode_their_raw_bytes() {
    let _dense = WireLayoutGuard::enter(WireLayout::Dense);
    let mut encoded = Vec::new();
    write_element_sequence::<u8, _>(&mut Encoder::for_buffer(&mut encoded), [1_u8, 2, 3]).unwrap();
    assert_eq!(encoded, [3, 1, 2, 3]);
    let (decoded, used) = decode_element_sequence_from_slice_serial::<u8>(&encoded).unwrap();
    assert_eq!((decoded, used), (vec![1, 2, 3], 4));
    let mut destination = [0_u8; 4];
    assert_eq!(
        decode_byte_element_sequence_into(&encoded, &mut destination).unwrap(),
        (3, 4)
    );
    assert_eq!(destination[..3], [1, 2, 3]);
    assert!(decode_element_sequence_from_slice_serial::<u8>(&[3, 1, 2]).is_err());
}

#[test]
fn public_padding_helper_follows_the_layout() {
    assert_eq!(payload_padding_for_layout::<Record>(WireLayout::Dense), 0);
    assert_eq!(
        payload_padding_for_layout::<Record>(WireLayout::Compact),
        payload_alignment_padding_for::<Record>()
    );
}
