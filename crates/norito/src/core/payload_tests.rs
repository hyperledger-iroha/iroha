//! Bare payload owners remain usable without a typed frame contract.

use super::*;
use crate::codec::Encode as _;

#[test]
fn borrowed_bytes_match_owned_payload_and_borrow_the_decoded_backing() {
    for flags in [0, default_encode_flags()] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for bytes in [b"".as_slice(), b"bytes", &[0, 0xff, 0x80]] {
            let mut payload = Vec::new();
            serialize_to_buffer(&bytes, &mut payload).unwrap();
            let mut owned_payload = Vec::new();
            serialize_to_buffer(&bytes.to_vec(), &mut owned_payload).unwrap();
            let mut golden = u64::try_from(bytes.len()).unwrap().to_le_bytes().to_vec();
            golden.extend_from_slice(bytes);
            assert_eq!(payload, golden);
            assert_eq!(payload, owned_payload);
            assert_eq!(bytes.encoded_len_hint(), Some(payload.len()));
            assert_eq!(bytes.encoded_len_exact(), Some(payload.len()));
            let (decoded, used) = <&[u8] as DecodeFromSlice>::decode_from_slice(&payload).unwrap();
            assert_eq!(decoded, bytes);
            assert_eq!(used, payload.len());
            assert_eq!(decoded.as_ptr(), payload[8..].as_ptr());
        }
    }
}

#[test]
fn borrowed_bytes_reject_truncation_and_enforce_sequence_limits() {
    let bytes = b"bytes".as_slice();
    let payload = bytes.encode();
    for flags in [0, default_encode_flags()] {
        let _flags = DecodeFlagsGuard::enter(flags);
        for length in 0..payload.len() {
            assert!(<&[u8] as DecodeFromSlice>::decode_from_slice(&payload[..length]).is_err());
        }
        let limits = DecodeLimits::new(4, usize::MAX, usize::MAX, usize::MAX, 16);
        assert!(
            with_decode_limits_scope(limits, || {
                <&[u8] as DecodeFromSlice>::decode_from_slice(&payload)
            })
            .is_err()
        );
        assert!(<&[u8] as DecodeFromSlice>::decode_from_slice(b"\x05bytes").is_err());
    }
}

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
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::IntoSchema))]
#[derive(crate::NoritoSchema)]
#[norito_schema(name = "norito.test.core.payload_tests.FramedRecord")]
struct FramedRecord<T: iroha_schema::IntoSchema> {
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
#[cfg_attr(feature = "schema-structural", derive(iroha_schema::TypeId))]
enum FramedVariant<T: iroha_schema::IntoSchema> {
    Empty,
    Item(T),
    Named { value: T },
}

// Schema metadata for the named payload; this is not another codec owner.
#[cfg(feature = "schema-structural")]
#[derive(iroha_schema::IntoSchema)]
#[allow(dead_code)]
struct FramedVariantNamedFields<T: iroha_schema::IntoSchema> {
    value: T,
}

#[cfg(feature = "schema-structural")]
impl<T: iroha_schema::IntoSchema> iroha_schema::IntoSchema for FramedVariant<T> {
    fn type_name() -> String {
        format!("FramedVariant<{}>", T::type_name())
    }

    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![
                iroha_schema::EnumVariant {
                    tag: "Empty".to_owned(),
                    discriminant: 0,
                    ty: None,
                },
                iroha_schema::EnumVariant {
                    tag: "Item".to_owned(),
                    discriminant: 1,
                    ty: Some(std::any::TypeId::of::<T>()),
                },
                iroha_schema::EnumVariant {
                    tag: "Named".to_owned(),
                    discriminant: 2,
                    ty: Some(std::any::TypeId::of::<FramedVariantNamedFields<T>>()),
                },
            ],
        }));
        T::update_schema_map(map);
        FramedVariantNamedFields::<T>::update_schema_map(map);
    }
}

fn layouts() -> [u8; 2] {
    [0, header_flags::COMPACT_LEN]
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
