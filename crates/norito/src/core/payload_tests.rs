//! Bare payload owners remain usable without a typed frame contract.

use super::*;
use crate::codec::Encode as _;

struct Leaf(u32);

impl SerializePayload for Leaf {
    fn serialize(&self, encoder: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(encoder)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        Some(4)
    }
}

#[derive(crate::SerializePayload)]
struct PayloadRecord<T> {
    value: T,
    items: Vec<Option<T>>,
}

#[derive(crate::NoritoSerialize, crate::NoritoDeserialize, Debug, PartialEq)]
struct FramedRecord<T> {
    value: T,
    items: Vec<Option<T>>,
}

#[derive(crate::SerializePayload)]
enum PayloadVariant<T> {
    Empty,
    Item(T),
    Named { value: T },
}

#[derive(crate::NoritoSerialize)]
enum FramedVariant<T> {
    Empty,
    Item(T),
    Named { value: T },
}

fn layouts() -> [u8; 4] {
    [
        0,
        header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT
            | header_flags::PACKED_SEQ
            | header_flags::FIELD_BITSET
            | header_flags::COMPACT_LEN,
    ]
}

fn assert_same_payload(payload: &dyn SerializePayload, typed: &dyn SerializePayload) {
    let mut actual = Vec::new();
    serialize_to_buffer(payload, &mut actual).unwrap();
    let mut expected = Vec::new();
    serialize_to_buffer(typed, &mut expected).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(encoded_payload_len(payload).unwrap(), actual.len());
    let mut exact = Vec::new();
    serialize_to_writer_exact(payload, &mut exact, actual.len()).unwrap();
    assert_eq!(exact, actual);
}

#[test]
fn payload_only_fields_and_containers_preserve_canonical_bytes() {
    let payload = PayloadRecord {
        value: Leaf(0x1020_3040),
        items: vec![Some(Leaf(7)), None, Some(Leaf(u32::MAX))],
    };
    let typed = FramedRecord {
        value: 0x1020_3040,
        items: vec![Some(7), None, Some(u32::MAX)],
    };
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        assert_same_payload(&payload, &typed);
        let frame = to_bytes(&typed).unwrap();
        let decoded = crate::decode_from_bytes::<FramedRecord<u32>>(&frame).unwrap();
        assert_eq!(decoded, typed);
    }
    assert_eq!(payload.encode(), typed.encode());
}

#[test]
fn payload_only_enum_preserves_each_variant_layout() {
    for flags in layouts() {
        let _flags = DecodeFlagsGuard::enter(flags);
        assert_same_payload(&PayloadVariant::<Leaf>::Empty, &FramedVariant::<u32>::Empty);
        assert_same_payload(&PayloadVariant::Item(Leaf(7)), &FramedVariant::Item(7_u32));
        assert_same_payload(
            &PayloadVariant::Named { value: Leaf(11) },
            &FramedVariant::Named { value: 11_u32 },
        );
    }
}

#[test]
fn erased_payload_writer_propagates_write_failure() {
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("destination failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let result = serialize_to_writer(&Leaf(7), &mut Broken);
    assert!(result.is_err());
}
