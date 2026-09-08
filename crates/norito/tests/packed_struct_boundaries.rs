//! Packed structures enforce their declared byte boundary and report exact prefixes.

use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
    core::{DecodeFlagsGuard, header_flags},
};

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
struct Named {
    counter: u32,
    label: String,
}

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
struct Tuple(u32, String);

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
struct EmptyNamed {}

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
struct EmptyTuple();

fn layouts() -> [u8; 4] {
    [
        header_flags::PACKED_STRUCT,
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
        header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN | header_flags::FIELD_BITSET,
        header_flags::PACKED_STRUCT
            | header_flags::PACKED_SEQ
            | header_flags::COMPACT_LEN
            | header_flags::FIELD_BITSET,
    ]
}

fn assert_boundaries<T>(value: &T)
where
    T: NoritoSerialize + for<'de> NoritoDeserialize<'de> + PartialEq + std::fmt::Debug,
{
    for requested in layouts() {
        let (payload, flags) = {
            let _flags = DecodeFlagsGuard::enter(requested);
            norito::codec::encode_with_header_flags(value)
        };
        let frame = norito::core::frame_bare_with_header_flags::<T>(&payload, flags)
            .expect("frame declared packed structure");
        assert_eq!(
            &norito::decode_from_bytes::<T>(&frame).expect("decode packed structure"),
            value
        );

        let mut trailing = payload.clone();
        trailing.extend_from_slice(&[0xA5, 0x5A]);
        let forged = norito::core::frame_bare_with_header_flags::<T>(&trailing, flags)
            .expect("frame trailing payload with a valid checksum");
        assert!(
            norito::decode_from_bytes::<T>(&forged).is_err(),
            "accepted trailing packed structure for layout {flags:#x}"
        );

        let _flags = DecodeFlagsGuard::enter(flags);
        let (prefix, used) = norito::core::decode_field_prefix::<T>(&trailing)
            .expect("explicit prefix boundary permits following fields");
        assert_eq!(&prefix, value);
        assert_eq!(used, payload.len());
        for end in 0..payload.len() {
            assert!(
                norito::core::decode_field_canonical::<T>(&payload[..end]).is_err(),
                "accepted truncated packed structure at {end}, layout {flags:#x}"
            );
        }
    }
}

#[test]
fn named_packed_struct_enforces_boundary() {
    assert_boundaries(&Named {
        counter: 0x1234_5678,
        label: "bounded packed payload".into(),
    });
}

#[test]
fn tuple_packed_struct_enforces_boundary() {
    assert_boundaries(&Tuple(0x1234_5678, "bounded packed payload".into()));
}

#[test]
fn empty_packed_structs_consume_the_declared_header() {
    assert_boundaries(&EmptyNamed {});
    assert_boundaries(&EmptyTuple());
}

#[test]
fn tuple_slice_decoder_rejects_trailing_packed_bytes() {
    use norito::core::DecodeFromSlice;

    let value = Tuple(7, "tuple slice".into());
    for requested in layouts() {
        let (mut payload, flags) = {
            let _flags = DecodeFlagsGuard::enter(requested);
            norito::codec::encode_with_header_flags(&value)
        };
        let _flags = DecodeFlagsGuard::enter(flags);
        let (decoded, used) = Tuple::decode_from_slice(&payload).expect("decode exact tuple");
        assert_eq!(decoded, value);
        assert_eq!(used, payload.len());
        payload.push(0);
        assert!(Tuple::decode_from_slice(&payload).is_err());
    }
}
