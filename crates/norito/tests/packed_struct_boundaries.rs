//! Packed structures enforce their declared byte boundary and report exact prefixes.

use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
    core::{DecodeFlagsGuard, header_flags},
};

#[derive(Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "norito.test.packed_struct_boundaries.Named")]
struct Named {
    counter: u32,
    label: String,
}

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
#[norito(decode_from_slice)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "norito.test.packed_struct_boundaries.Tuple")]
struct Tuple(u32, String);

#[derive(Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "norito.test.packed_struct_boundaries.EmptyNamed")]
struct EmptyNamed {}

#[derive(Debug, PartialEq, Eq, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "norito.test.packed_struct_boundaries.EmptyTuple")]
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

#[derive(norito::SerializePayload)]
struct EmptyUnit;

#[test]
fn unit_struct_size_hints_match_serialization_for_every_advertised_layout() {
    use norito::SerializePayload as _;

    let value = EmptyUnit;
    for flags in (0..=norito::core::supported_header_flags())
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
    {
        let _flags = DecodeFlagsGuard::enter(flags);
        let mut payload = Vec::new();
        norito::core::serialize_to_buffer(&value, &mut payload).unwrap();
        let expected: &[u8] = if flags & header_flags::PACKED_STRUCT != 0
            && flags & header_flags::FIELD_BITSET == 0
        {
            // The existing zero-field packed record has one zero offset.
            &[0; 8]
        } else {
            &[]
        };
        assert_eq!(
            payload, expected,
            "unit bytes changed for flags {flags:#04x}"
        );
        assert_eq!(
            value.encoded_len_exact(),
            Some(payload.len()),
            "wrong exact unit length for flags {flags:#04x}",
        );
        assert_eq!(
            value.encoded_len_hint(),
            Some(payload.len()),
            "wrong hinted unit length for flags {flags:#04x}",
        );
    }
}

thread_local! {
    static NAMED_SLICE_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

// A bare field has neither a frame identity nor a typed frame codec contract.
#[derive(Debug, PartialEq, norito::SerializePayload, norito::DeserializePayload)]
#[norito(validate = "Self::validate", decode_from_slice)]
struct NamedSlice {
    count: u32,
    values: Vec<u16>,
    tag: Option<u8>,
}

impl NamedSlice {
    fn validate(self) -> Result<Self, norito::Error> {
        NAMED_SLICE_VALIDATIONS.set(NAMED_SLICE_VALIDATIONS.get() + 1);
        if self.count == 0 {
            return Err(norito::Error::Message("zero field count".into()));
        }
        Ok(self)
    }
}

#[derive(Debug, PartialEq, norito::DeserializePayload)]
#[norito(decode_from_slice)]
struct DecoderOnlyNamedSlice {
    count: u32,
    values: Vec<u16>,
    tag: Option<u8>,
}

#[test]
fn named_slice_decoder_obeys_layout_prefix_validation_and_canonical_boundaries() {
    use norito::core::DecodeFromSlice as _;

    let value = NamedSlice {
        count: 7,
        values: vec![3, 5, 11],
        tag: Some(9),
    };
    for requested in (0..=norito::core::supported_header_flags())
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
    {
        let (payload, flags) = {
            let _requested = DecodeFlagsGuard::enter(requested);
            norito::codec::encode_with_header_flags(&value)
        };
        let _flags = DecodeFlagsGuard::enter(flags);
        NAMED_SLICE_VALIDATIONS.set(0);
        let (decoded, used) = NamedSlice::decode_from_slice(&payload).expect("exact named slice");
        assert_eq!(decoded, value);
        assert_eq!(used, payload.len());
        assert_eq!(NAMED_SLICE_VALIDATIONS.get(), 1);

        let mut trailing = payload.clone();
        trailing.extend_from_slice(&[0xF1, 0xB2]);
        NAMED_SLICE_VALIDATIONS.set(0);
        let (decoded, used) = NamedSlice::decode_from_slice(&trailing).expect("named slice prefix");
        assert_eq!(decoded, value);
        assert_eq!(used, payload.len());
        assert_eq!(NAMED_SLICE_VALIDATIONS.get(), 1);
        assert!(norito::core::decode_field_canonical_from_slice::<NamedSlice>(&trailing).is_err());
        assert_eq!(
            norito::core::decode_field_canonical_from_slice::<NamedSlice>(&payload).unwrap(),
            (
                NamedSlice {
                    count: 7,
                    values: vec![3, 5, 11],
                    tag: Some(9)
                },
                payload.len()
            )
        );
        for end in 0..payload.len() {
            assert!(
                NamedSlice::decode_from_slice(&payload[..end]).is_err(),
                "accepted prefix {end} for flags {flags:#04x}"
            );
        }
        assert_eq!(norito::core::effective_decode_flags(), Some(flags));

        let invalid = NamedSlice {
            count: 0,
            values: vec![3, 5, 11],
            tag: Some(9),
        };
        let (invalid, invalid_flags) = norito::codec::encode_with_header_flags(&invalid);
        let _invalid_flags = DecodeFlagsGuard::enter(invalid_flags);
        NAMED_SLICE_VALIDATIONS.set(0);
        assert!(
            matches!(NamedSlice::decode_from_slice(&invalid), Err(norito::Error::Message(message)) if message == "zero field count")
        );
        assert_eq!(NAMED_SLICE_VALIDATIONS.get(), 1);
    }
}

#[test]
fn named_slice_decoder_requires_no_serializer_or_frame_identity() {
    use norito::core::DecodeFromSlice as _;

    let value = NamedSlice {
        count: 7,
        values: vec![3, 5, 11],
        tag: Some(9),
    };
    for requested in (0..=norito::core::supported_header_flags())
        .filter(|flags| norito::core::validate_header_flags(*flags).is_ok())
    {
        let (mut payload, flags) = {
            let _requested = DecodeFlagsGuard::enter(requested);
            norito::codec::encode_with_header_flags(&value)
        };
        let expected_used = payload.len();
        payload.extend_from_slice(&[0xF1, 0xB2]);
        let _flags = DecodeFlagsGuard::enter(flags);
        let (decoded, used) =
            DecoderOnlyNamedSlice::decode_from_slice(&payload).expect("decoder-only prefix");
        assert_eq!(
            decoded,
            DecoderOnlyNamedSlice {
                count: 7,
                values: vec![3, 5, 11],
                tag: Some(9)
            }
        );
        assert_eq!(used, expected_used);
        assert!(
            norito::core::decode_field_canonical_from_slice::<DecoderOnlyNamedSlice>(&payload)
                .is_err()
        );
    }
}
