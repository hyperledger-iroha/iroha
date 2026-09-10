//! Targeted AoS enum roundtrip tests for Norito derives.
#![allow(clippy::size_of_ref)]
use iroha_schema::IntoSchema;
use norito::{DeserializePayload, from_bytes, to_bytes};
#[derive(
    IntoSchema, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq,
)]
struct TuplePayload {
    value: u64,
    text: String,
}
#[derive(
    IntoSchema, norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq,
)]
struct StructPayload {
    name: String,
    data: Vec<u8>,
    tag: [u8; 4],
}
#[derive(
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    Debug,
    PartialEq,
    norito::NoritoSchema,
)]
#[norito_schema(name = "norito.test.enum_aos.AoSEnum")]
enum AoSEnum {
    Unit,
    Tuple(TuplePayload),
    Struct(StructPayload),
}
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq)]
#[norito(decode_from_slice)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "norito.test.enum_aos.AoSNamedEnum")]
enum AoSNamedEnum {
    StructLike {
        label: String,
        data: Vec<u8>,
        code: u16,
    },
    Unit,
}
#[derive(
    IntoSchema,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    Debug,
    PartialEq,
    norito::NoritoSchema,
)]
#[norito_schema(name = "norito.test.enum_aos.AoSU8ArrayEnum")]
enum AoSU8ArrayEnum {
    Unit,
    Bytes([u8; 12]),
}
#[derive(norito::derive::NoritoSerialize, norito::derive::NoritoDeserialize, Debug, PartialEq)]
enum NamedArrayEnum {
    Raw { prefix: i32, bytes: [u8; 32] },
}
#[derive(
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
    Debug,
    PartialEq,
    norito::NoritoSchema,
)]
#[norito_schema(name = "norito.test.enum_aos.NestedNamedArray")]
struct NestedNamedArray {
    first: String,
    value: NamedArrayEnum,
    last: String,
}
#[test]
fn aos_enum_roundtrip_unit() {
    let v = AoSEnum::Unit;
    let bytes = to_bytes(&v).unwrap();
    let arch = from_bytes::<AoSEnum>(&bytes).unwrap();
    let back = <AoSEnum as DeserializePayload>::deserialize(arch);
    assert_eq!(v, back);
}
#[test]
fn aos_enum_roundtrip_tuple() {
    let v = AoSEnum::Tuple(TuplePayload {
        value: 42,
        text: "hello".to_string(),
    });
    let bytes = to_bytes(&v).unwrap();
    let arch = from_bytes::<AoSEnum>(&bytes).unwrap();
    let back = <AoSEnum as DeserializePayload>::deserialize(arch);
    assert_eq!(v, back);
}
#[test]
fn aos_enum_roundtrip_struct() {
    let v = AoSEnum::Struct(StructPayload {
        name: "abc".to_string(),
        data: vec![1, 2, 3, 4, 5],
        tag: *b"TAG!",
    });
    let bytes = to_bytes(&v).unwrap();
    let arch = from_bytes::<AoSEnum>(&bytes).unwrap();
    let back = <AoSEnum as DeserializePayload>::deserialize(arch);
    assert_eq!(v, back);
}
#[test]
fn aos_enum_roundtrip_named_variant() {
    let v = AoSNamedEnum::StructLike {
        label: "named".to_string(),
        data: vec![1, 2, 3, 4],
        code: 7,
    };
    let bytes = to_bytes(&v).unwrap();
    let view = norito::core::from_bytes_view(&bytes).unwrap();
    let back: AoSNamedEnum = view.decode().expect("decode named enum");
    assert_eq!(v, back);
}
#[test]
fn aos_enum_roundtrip_u8_array_unpacked() {
    let _guard = norito::core::DecodeFlagsGuard::enter(0);
    let v = AoSU8ArrayEnum::Bytes([0xAB; 12]);
    let bytes = to_bytes(&v).unwrap();
    let arch = from_bytes::<AoSU8ArrayEnum>(&bytes).unwrap();
    let back = <AoSU8ArrayEnum as DeserializePayload>::deserialize(arch);
    assert_eq!(v, back);
}
#[test]
fn aos_nested_named_variant_with_u8_array_roundtrips() {
    let v = NestedNamedArray {
        first: "before".to_owned(),
        value: NamedArrayEnum::Raw {
            prefix: -1,
            bytes: [0xAB; 32],
        },
        last: "after".to_owned(),
    };
    let bytes = to_bytes(&v).unwrap();
    let payload = norito::core::from_bytes_view(&bytes).unwrap();
    assert_eq!(
        norito::core::SerializePayload::encoded_len_exact(&v),
        Some(payload.as_bytes().len()),
        "named enum byte arrays must report the raw-byte wire length"
    );
    let back: NestedNamedArray = norito::decode_from_bytes(&bytes).unwrap();
    assert_eq!(v, back);
}

#[derive(Clone, Debug, PartialEq, norito::Encode, norito::Decode)]
struct PrefixChild(u32);

#[derive(Clone, Debug, PartialEq, norito::Encode, norito::Decode)]
#[norito(decode_from_slice, validate = "Self::validate")]
enum PrefixEnum<T> {
    Unit,
    Tuple(T, u16),
    Named { value: T, count: u32 },
}

impl<T> PrefixEnum<T> {
    fn validate(self) -> Result<Self, norito::Error> {
        PREFIX_VALIDATIONS.with(|calls| calls.set(calls.get() + 1));
        if matches!(self, Self::Named { count: 0, .. }) {
            Err(norito::Error::Message("zero prefix count".into()))
        } else {
            Ok(self)
        }
    }
}

thread_local! {
    static PREFIX_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn prefix_payload<T: norito::SerializePayload>(value: &T) -> Vec<u8> {
    let mut payload = Vec::new();
    value
        .serialize(&mut norito::core::Encoder::new(&mut payload))
        .unwrap();
    payload
}

fn assert_enum_prefix<T>(value: &T)
where
    T: norito::SerializePayload
        + for<'de> norito::DeserializePayload<'de>
        + for<'de> norito::core::DecodeFromSlice<'de>,
{
    use norito::core as ncore;
    for flags in (0..=ncore::supported_header_flags())
        .filter(|flags| ncore::validate_header_flags(*flags).is_ok())
    {
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        let payload = prefix_payload(value);
        let mut following = payload.clone();
        following.extend_from_slice(&[0xa5, 0x5a]);
        let (decoded, used) = T::decode_from_slice(&following).expect("enum payload prefix");
        assert_eq!(used, payload.len());
        assert_eq!(&following[used..], &[0xa5, 0x5a]);
        assert_eq!(prefix_payload(&decoded), payload);
        assert!(matches!(
            ncore::decode_field_canonical::<T>(&following),
            Err(norito::Error::LengthMismatch)
        ));
        let (decoded, used) = ncore::decode_field_canonical::<T>(&payload).unwrap();
        assert_eq!(used, payload.len());
        assert_eq!(prefix_payload(&decoded), payload);
        for end in 0..payload.len() {
            assert!(
                T::decode_from_slice(&payload[..end]).is_err(),
                "truncated at {end}"
            );
        }
        assert_eq!(ncore::get_decode_flags(), flags);
    }
}

#[test]
fn enum_slice_variants_consume_only_their_prefix_in_every_layout() {
    assert_enum_prefix(&PrefixEnum::<u32>::Unit);
    assert_enum_prefix(&PrefixEnum::Tuple(42_u32, u16::MAX));
    assert_enum_prefix(&PrefixEnum::Named {
        value: 42_u32,
        count: 2,
    });
    assert_enum_prefix(&PrefixEnum::Tuple(PrefixChild(42), 3));
    assert_enum_prefix(&PrefixEnum::Named {
        value: vec![PrefixChild(42)],
        count: 1,
    });
    assert_enum_prefix(&AoSNamedEnum::Unit);
    assert_enum_prefix(&AoSNamedEnum::StructLike {
        label: "prefix 雪".into(),
        data: vec![1, 2, 3],
        code: 9,
    });
}

#[test]
fn enum_slice_containers_keep_inner_boundaries_and_following_bytes() {
    let value = PrefixEnum::Tuple(PrefixChild(42), 7);
    assert_enum_prefix(&Some(value.clone()));
    assert_enum_prefix(&vec![value.clone(), PrefixEnum::Unit, value]);
    assert_enum_prefix(&Option::<PrefixEnum<PrefixChild>>::None);
    assert_enum_prefix(&Vec::<PrefixEnum<PrefixChild>>::new());
}

#[test]
fn enum_slice_validation_runs_once_and_malformed_tags_fail_before_validation() {
    use norito::core::{self as ncore, DecodeFromSlice};
    for flags in [
        0,
        ncore::header_flags::COMPACT_LEN,
        ncore::header_flags::PACKED_STRUCT,
    ] {
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        let mut payload = prefix_payload(&PrefixEnum::Named {
            value: 7_u32,
            count: 1,
        });
        let prefix_len = payload.len();
        payload.extend_from_slice(&[0xa5, 0x5a]);
        PREFIX_VALIDATIONS.set(0);
        let (value, used) = PrefixEnum::<u32>::decode_from_slice(&payload).unwrap();
        assert_eq!(value, PrefixEnum::Named { value: 7, count: 1 });
        assert_eq!(used, prefix_len);
        assert_eq!(PREFIX_VALIDATIONS.get(), 1);
        let mut invalid = prefix_payload(&PrefixEnum::Named {
            value: 7_u32,
            count: 0,
        });
        invalid.extend_from_slice(&[0xa5, 0x5a]);
        PREFIX_VALIDATIONS.set(0);
        assert!(matches!(
            PrefixEnum::<u32>::decode_from_slice(&invalid),
            Err(norito::Error::Message(message)) if message == "zero prefix count"
        ));
        assert_eq!(PREFIX_VALIDATIONS.get(), 1);
        payload[..4].copy_from_slice(&u32::MAX.to_le_bytes());
        PREFIX_VALIDATIONS.set(0);
        assert!(PrefixEnum::<u32>::decode_from_slice(&payload).is_err());
        assert_eq!(PREFIX_VALIDATIONS.get(), 0);
        assert_eq!(ncore::get_decode_flags(), flags);
    }
}

#[test]
fn enum_slice_inherits_allocation_limits_and_restores_outer_state() {
    use norito::core::{self as ncore, DecodeFromSlice};
    for flags in [0, ncore::header_flags::COMPACT_LEN] {
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        let value = PrefixEnum::Tuple("bounded".to_owned(), 7);
        let mut payload = prefix_payload(&value);
        let prefix_len = payload.len();
        payload.extend_from_slice(&[0xa5, 0x5a]);
        let outer = ncore::DecodeLimits::new(1024, 4096, 4096, 65536, 32);
        ncore::with_decode_limits_scope(outer, || {
            let deny = ncore::DecodeLimits::new(1024, 4096, 4096, 0, 32);
            ncore::with_decode_limits_scope(deny, || {
                assert!(matches!(
                    PrefixEnum::<String>::decode_from_slice(&payload),
                    Err(norito::Error::TotalAllocationExceeded { .. })
                ));
            });
            assert_eq!(
                PrefixEnum::<String>::decode_from_slice(&payload).unwrap(),
                (value.clone(), prefix_len)
            );
            assert_eq!(ncore::get_decode_flags(), flags);
        });
        assert_eq!(
            PrefixEnum::<String>::decode_from_slice(&payload).unwrap(),
            (value, prefix_len)
        );
    }
}
