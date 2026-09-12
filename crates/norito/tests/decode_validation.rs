//! Binary derive validation owns every reconstructed value and preserves typed errors.

use std::cell::Cell;

use norito::{Decode, Encode, Error, NoritoSchema, codec::Encode as _, core::DecodeFromSlice};

thread_local! {
    static CALLS: Cell<usize> = const { Cell::new(0) };
    static REJECT_UNIT: Cell<bool> = const { Cell::new(false) };
}

fn checked(value: u8) -> Result<(), Error> {
    CALLS.set(CALLS.get() + 1);
    match value {
        0 => Err(Error::Message("zero member weight".into())),
        255 => Err(Error::FieldLengthExceeded {
            length: 255,
            limit: 254,
        }),
        _ => Ok(()),
    }
}

#[derive(Debug, PartialEq, Encode, Decode, NoritoSchema)]
#[norito_schema(name = "norito::tests::decode_validation::Named")]
#[norito(validate = "Self::validate", decode_from_slice)]
struct Named {
    value: u8,
}
impl Named {
    fn validate(self) -> Result<Self, Error> {
        checked(self.value)?;
        Ok(self)
    }
}

#[derive(Debug, PartialEq, Encode, Decode, NoritoSchema)]
#[norito_schema(name = "norito::tests::decode_validation::Tuple")]
#[norito(validate = "Self::validate", decode_from_slice)]
struct Tuple(u8);
impl Tuple {
    fn validate(self) -> Result<Self, Error> {
        checked(self.0)?;
        Ok(self)
    }
}

#[derive(Debug, PartialEq, Encode, Decode, NoritoSchema)]
#[norito_schema(name = "norito::tests::decode_validation::Choice")]
#[norito(validate = "Self::validate", decode_from_slice)]
enum Choice {
    Unit,
    Tuple(u8),
    Named { value: u8 },
}
impl Choice {
    fn validate(self) -> Result<Self, Error> {
        checked(match self {
            Self::Unit => 1,
            Self::Tuple(value) | Self::Named { value } => value,
        })?;
        Ok(self)
    }
}

#[derive(Debug, PartialEq, Encode, Decode, NoritoSchema)]
#[norito_schema(name = "norito::tests::decode_validation::Unit")]
#[norito(validate = "Self::validate", decode_from_slice)]
struct Unit;
impl Unit {
    fn validate(self) -> Result<Self, Error> {
        checked(if REJECT_UNIT.get() { 0 } else { 1 })?;
        Ok(self)
    }
}

trait BorrowScope<'a> {}
impl<'a> BorrowScope<'a> for u8 {}

#[derive(Debug, PartialEq, Encode, Decode, NoritoSchema)]
#[norito_schema(name = "norito::tests::decode_validation::Generic")]
#[norito(validate = "Self::validate", decode_from_slice)]
struct Generic<T>
where
    T: for<'__norito_slice> BorrowScope<'__norito_slice>,
{
    value: T,
}
impl<T> Generic<T>
where
    T: for<'__norito_slice> BorrowScope<'__norito_slice>,
{
    fn validate(self) -> Result<Self, Error> {
        checked(1)?;
        Ok(self)
    }
}

fn layout_frame<T: norito::NoritoSerialize>(value: &T, requested: u8) -> Vec<u8> {
    let _requested = norito::core::DecodeFlagsGuard::enter(requested);
    let calls = CALLS.get();
    let (payload, actual) = norito::codec::encode_with_header_flags(value);
    let frame = norito::core::frame_bare_with_header_flags::<T>(&payload, actual).unwrap();
    assert_eq!(
        CALLS.get(),
        calls,
        "serialization must not invoke validation"
    );
    frame
}

fn decode_frame<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: norito::NoritoSerialize + for<'de> norito::NoritoDeserialize<'de>,
{
    let header = norito::core::Header::read(bytes)?;
    let _flags = norito::core::DecodeFlagsGuard::enter(header.flags);
    norito::decode_from_bytes(bytes)
}

fn frame_roundtrip<T>(value: &T, requested: u8)
where
    T: norito::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>
        + PartialEq
        + std::fmt::Debug,
{
    let bytes = layout_frame(value, requested);
    CALLS.set(0);
    let decoded = decode_frame::<T>(&bytes).unwrap_or_else(|error| {
        panic!(
            "{} requested flags {requested:#04x}: {error:?}",
            std::any::type_name::<T>()
        )
    });
    assert_eq!(&decoded, value);
    assert_eq!(CALLS.get(), 1);
}

#[test]
fn validated_derives_accept_all_shapes_once_under_advertised_layouts() {
    for flags in [0, 1, 2, 3, 4, 5, 6, 7, 0x1b, 0x3f] {
        frame_roundtrip(&Named { value: 7 }, flags);
        frame_roundtrip(&Tuple(7), flags);
        frame_roundtrip(&Choice::Unit, flags);
        frame_roundtrip(&Choice::Tuple(7), flags);
        frame_roundtrip(&Choice::Named { value: 7 }, flags);
        frame_roundtrip(&Unit, flags);
        frame_roundtrip(&Generic { value: 7_u8 }, flags);
    }
}

fn slice_roundtrip<T>(value: &T)
where
    T: norito::SerializePayload + for<'de> DecodeFromSlice<'de> + PartialEq + std::fmt::Debug,
{
    let payload = value.encode();
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    CALLS.set(0);
    let (decoded, used) = T::decode_from_slice(&payload).unwrap();
    assert_eq!(&decoded, value);
    assert_eq!(used, payload.len());
    assert_eq!(CALLS.get(), 1);
}

#[test]
fn generated_slice_decoders_validate_named_tuple_unit_and_enum_exactly_once() {
    slice_roundtrip(&Named { value: 7 });
    slice_roundtrip(&Tuple(7));
    slice_roundtrip(&Generic { value: 7_u8 });
    slice_roundtrip(&Choice::Unit);
    slice_roundtrip(&Choice::Tuple(7));
    slice_roundtrip(&Choice::Named { value: 7 });
    slice_roundtrip(&Unit);

    let mut prefix = Named { value: 7 }.encode();
    let expected_used = prefix.len();
    prefix.push(0xff);
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    CALLS.set(0);
    assert_eq!(
        Named::decode_from_slice(&prefix).unwrap(),
        (Named { value: 7 }, expected_used)
    );
    assert_eq!(CALLS.get(), 1);

    let payload = Named { value: 0 }.encode();
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    CALLS.set(0);
    assert!(
        matches!(Named::decode_from_slice(&payload), Err(Error::Message(message)) if message == "zero member weight")
    );
    assert_eq!(CALLS.get(), 1);
    slice_roundtrip(&Named { value: 7 });
}

#[test]
fn validation_errors_preserve_kind_and_message_and_allow_later_valid_decodes() {
    for flags in [0, 1, 2, 3, 4, 5, 6, 7, 0x1b, 0x3f] {
        let invalid = layout_frame(&Named { value: 0 }, flags);
        CALLS.set(0);
        assert!(
            matches!(decode_frame::<Named>(&invalid), Err(Error::Message(message)) if message == "zero member weight")
        );
        assert_eq!(CALLS.get(), 1);
        let typed_error = layout_frame(&Tuple(255), flags);
        CALLS.set(0);
        assert!(matches!(
            decode_frame::<Tuple>(&typed_error),
            Err(Error::FieldLengthExceeded {
                length: 255,
                limit: 254
            })
        ));
        assert_eq!(CALLS.get(), 1);
        let invalid_enum = layout_frame(&Choice::Named { value: 0 }, flags);
        CALLS.set(0);
        assert!(
            matches!(decode_frame::<Choice>(&invalid_enum), Err(Error::Message(message)) if message == "zero member weight")
        );
        assert_eq!(CALLS.get(), 1);
        frame_roundtrip(&Named { value: 7 }, flags);
    }
}

#[test]
fn malformed_fields_reject_before_validation_and_unit_failures_are_fallible() {
    let mut payload = Named { value: 7 }.encode();
    payload.pop().unwrap();
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    CALLS.set(0);
    assert!(Named::decode_from_slice(&payload).is_err());
    assert_eq!(CALLS.get(), 0);
    let framed = norito::core::frame_bare_with_header_flags::<Named>(
        &payload,
        norito::core::default_encode_flags(),
    )
    .unwrap();
    CALLS.set(0);
    assert!(decode_frame::<Named>(&framed).is_err());
    assert_eq!(CALLS.get(), 0);

    let malformed_unit = norito::core::frame_bare_with_header_flags::<Unit>(&[1], 0).unwrap();
    CALLS.set(0);
    assert!(matches!(
        decode_frame::<Unit>(&malformed_unit),
        Err(Error::LengthMismatch)
    ));
    // Unit has no fields to reject before reconstruction; the enclosing
    // canonical byte comparator rejects its malformed layout afterward.
    assert_eq!(CALLS.get(), 1);

    let valid_unit = layout_frame(&Unit, 0);
    REJECT_UNIT.set(true);
    CALLS.set(0);
    assert!(
        matches!(decode_frame::<Unit>(&valid_unit), Err(Error::Message(message)) if message == "zero member weight")
    );
    assert_eq!(CALLS.get(), 1);
    CALLS.set(0);
    assert!(Unit::decode_from_slice(&[]).is_err());
    assert_eq!(CALLS.get(), 1);
    REJECT_UNIT.set(false);
    slice_roundtrip(&Unit);
}

#[test]
fn unit_validation_preserves_empty_archives_and_canonical_packed_metadata() {
    let empty = layout_frame(&Unit, 0);
    CALLS.set(0);
    let archived = norito::from_bytes::<Unit>(&empty).unwrap();
    assert_eq!(
        <Unit as norito::DeserializePayload>::try_deserialize(archived).unwrap(),
        Unit
    );
    assert_eq!(CALLS.get(), 1);

    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::header_flags::PACKED_STRUCT);
    let (payload, actual) = norito::codec::encode_with_header_flags(&Unit);
    assert_eq!(actual, norito::core::header_flags::PACKED_STRUCT);
    assert_eq!(payload, [0; 8]);
    let mut wrong_offset = payload.clone();
    wrong_offset[0] = 1;
    let mut trailing = payload.clone();
    trailing.push(0xff);
    for malformed in [wrong_offset, trailing] {
        let frame = norito::core::frame_bare_with_header_flags::<Unit>(&malformed, actual).unwrap();
        CALLS.set(0);
        assert!(matches!(
            decode_frame::<Unit>(&frame),
            Err(Error::LengthMismatch)
        ));
        assert_eq!(CALLS.get(), 1);
        frame_roundtrip(&Unit, actual);
    }
}
