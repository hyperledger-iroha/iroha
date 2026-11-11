//! Fixed-width scalar fields decode exactly as the general field decoder does.

use super::*;
use std::fmt::Debug;

/// The same payload as `T`, decoded through the general field machinery:
/// it forwards `T`'s codec and declares no scalar width of its own.
struct General<T>(T);

impl<T: SerializePayload> SerializePayload for General<T> {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(writer)
    }
}

impl<'a, T: DeserializePayload<'a>> DeserializePayload<'a> for General<T> {
    fn deserialize(archived: &'a Archived<Self>) -> Self {
        Self(T::deserialize(archived.cast()))
    }
    fn try_deserialize(archived: &'a Archived<Self>) -> Result<Self, Error> {
        T::try_deserialize(archived.cast()).map(Self)
    }
}

/// Deterministic byte patterns, including invalid Boolean, character and
/// non-zero encodings.
const FILLS: [fn(usize) -> u8; 7] = [
    |_| 0x00,
    |_| 0x01,
    |_| 0x02,
    |_| 0xff,
    |index| index as u8,
    |index| 0x80 | index as u8,
    // Little-endian surrogate `0xD800` in every four-byte window.
    |index| [0x00, 0xd8, 0x00, 0x00][index % 4],
];

/// Limit scopes around a `width`-byte scalar: none, then each ceiling one
/// short of and exactly at the field. Order: sequence elements, field bytes,
/// total elements, total allocation, nesting depth.
fn limit_scopes(width: usize) -> Vec<Option<DecodeLimits>> {
    let wide = usize::MAX;
    vec![
        None,
        Some(DecodeLimits::new(wide, width - 1, wide, wide, wide)),
        Some(DecodeLimits::new(wide, width, wide, wide, wide)),
        Some(DecodeLimits::new(wide, wide, wide, wide, 0)),
        Some(DecodeLimits::new(wide, wide, wide, wide, 1)),
        Some(DecodeLimits::new(wide, width - 1, wide, wide, 0)),
    ]
}

fn under<R>(limits: Option<DecodeLimits>, body: impl FnOnce() -> R) -> R {
    match limits {
        Some(limits) => with_decode_limits_scope(limits, body),
        None => body(),
    }
}

fn outcome<T: Debug, E: Debug>(result: Result<T, Error>, extra: E) -> String {
    match result {
        Ok(value) => format!("ok {value:?} {extra:?}"),
        Err(error) => format!("err {error:?} {extra:?}"),
    }
}

/// Decode `field` as a sequence element or map entry would, inside a larger
/// payload, and report the result with the enclosing payload's recorded access.
fn decode_element<T>(field: &[u8]) -> (Result<(T, usize), Error>, Option<usize>)
where
    T: for<'de> DeserializePayload<'de> + SerializePayload,
{
    let mut parent = vec![0xa5, 0x5a];
    parent.extend_from_slice(field);
    parent.extend_from_slice(&[0xc3, 0x3c, 0x77]);
    let _context = PayloadCtxGuard::enter(&parent);
    let result = decode_field_canonical::<T>(&parent[2..2 + field.len()]);
    (result, payload_ctx_max_access())
}

/// Decode `field` as a derived record decodes a length-prefixed member, and
/// report the result with the advanced offset and the recorded access.
fn decode_member<T>(field: &[u8], flags: u8) -> (Result<T, Error>, usize, Option<usize>)
where
    T: for<'de> DeserializePayload<'de> + SerializePayload,
{
    let _flags = DecodeFlagsGuard::enter(flags);
    let mut parent = vec![0xa5];
    write_len_to_vec_with_flags(&mut parent, field.len() as u64, flags);
    parent.extend_from_slice(field);
    parent.extend_from_slice(&[0xc3, 0x3c]);
    let _context = PayloadCtxGuard::enter(&parent);
    let mut offset = 1;
    let result = decode_context_field_canonical::<T>(parent.as_ptr(), &mut offset);
    (result, offset, payload_ctx_max_access())
}

fn assert_scalar_matches_general<T>(width: usize)
where
    T: for<'de> DeserializePayload<'de> + SerializePayload + Debug,
{
    assert_eq!(
        <T as DeserializePayload<'static>>::__NORITO_SCALAR_WIDTH,
        Some(width),
        "{} declares its wire width",
        core::any::type_name::<T>()
    );
    assert_eq!(
        <General<T> as DeserializePayload<'static>>::__NORITO_SCALAR_WIDTH,
        None,
        "the reference wrapper takes the general decoder"
    );
    for len in [0, 1, width - 1, width, width + 1, width + 3] {
        for fill in FILLS {
            let field: Vec<u8> = (0..len).map(fill).collect();
            for limits in limit_scopes(width) {
                let label = format!(
                    "{} len {len} bytes {field:02x?} limits {limits:?}",
                    core::any::type_name::<T>()
                );
                let (fast, fast_access) = under(limits, || decode_element::<T>(&field));
                let (general, general_access) =
                    under(limits, || decode_element::<General<T>>(&field));
                assert_eq!(
                    outcome(fast, fast_access),
                    outcome(general.map(|(value, used)| (value.0, used)), general_access),
                    "element {label}"
                );
                for flags in [0, header_flags::COMPACT_LEN] {
                    let (fast, fast_offset, fast_access) =
                        under(limits, || decode_member::<T>(&field, flags));
                    let (general, general_offset, general_access) =
                        under(limits, || decode_member::<General<T>>(&field, flags));
                    assert_eq!(
                        outcome(fast, (fast_offset, fast_access)),
                        outcome(
                            general.map(|value| value.0),
                            (general_offset, general_access)
                        ),
                        "member flags {flags:#x} {label}"
                    );
                }
            }
        }
    }
}

#[test]
fn every_scalar_field_decodes_exactly_as_the_general_decoder() {
    assert_scalar_matches_general::<u8>(1);
    assert_scalar_matches_general::<i8>(1);
    assert_scalar_matches_general::<bool>(1);
    assert_scalar_matches_general::<u16>(2);
    assert_scalar_matches_general::<i16>(2);
    assert_scalar_matches_general::<NonZeroU16>(2);
    assert_scalar_matches_general::<u32>(4);
    assert_scalar_matches_general::<i32>(4);
    assert_scalar_matches_general::<f32>(4);
    assert_scalar_matches_general::<char>(4);
    assert_scalar_matches_general::<NonZeroU32>(4);
    assert_scalar_matches_general::<u64>(8);
    assert_scalar_matches_general::<i64>(8);
    assert_scalar_matches_general::<f64>(8);
    assert_scalar_matches_general::<usize>(8);
    assert_scalar_matches_general::<isize>(8);
    assert_scalar_matches_general::<NonZeroU64>(8);
    assert_scalar_matches_general::<u128>(16);
    assert_scalar_matches_general::<i128>(16);
}

#[test]
fn scalar_fields_round_trip_their_extreme_values() {
    fn round_trip<T>(value: T)
    where
        T: for<'de> DeserializePayload<'de> + SerializePayload + Debug + PartialEq,
    {
        let mut field = Vec::new();
        serialize_to_buffer(&value, &mut field).expect("encode scalar");
        let (decoded, used) = decode_field_canonical::<T>(&field).expect("decode scalar");
        assert_eq!(decoded, value);
        assert_eq!(used, field.len());
    }
    round_trip(u8::MAX);
    round_trip(i8::MIN);
    round_trip(true);
    round_trip(false);
    round_trip(u16::MAX);
    round_trip(i16::MIN);
    round_trip(u32::MAX);
    round_trip(i32::MIN);
    round_trip(u64::MAX);
    round_trip(i64::MIN);
    round_trip(u128::MAX);
    round_trip(i128::MIN);
    round_trip(usize::MAX);
    round_trip(isize::MIN);
    round_trip(f32::MIN_POSITIVE);
    round_trip(f64::NEG_INFINITY);
    round_trip('\u{10ffff}');
    round_trip(NonZeroU16::MAX);
    round_trip(NonZeroU32::MIN);
    round_trip(NonZeroU64::MAX);
}

#[test]
fn a_value_error_takes_precedence_over_a_long_scalar_field() {
    // Two bytes in a Boolean field: the invalid tag is reported, not the length.
    assert!(matches!(
        decode_field_canonical::<bool>(&[2, 0]),
        Err(Error::InvalidTag { .. })
    ));
    // A valid tag followed by a stray byte is a non-canonical field.
    assert!(matches!(
        decode_field_canonical::<bool>(&[1, 0]),
        Err(Error::LengthMismatch)
    ));
    assert!(matches!(
        decode_field_canonical::<NonZeroU32>(&[0, 0, 0, 0, 9]),
        Err(Error::InvalidNonZero)
    ));
}

#[test]
fn scalar_fields_do_not_enter_or_leak_a_nesting_level() {
    let baseline = DECODE_NESTING_DEPTH.with(Cell::get);
    with_decode_limits_scope(
        DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 1),
        || {
            for _ in 0..4 {
                decode_field_canonical::<u64>(&7_u64.to_le_bytes()).expect("one level fits");
                assert_eq!(DECODE_NESTING_DEPTH.with(Cell::get), baseline);
            }
            // The level a scalar occupies still counts against the ceiling.
            let _outer = DecodeDepthGuard::enter().expect("one level fits");
            assert!(matches!(
                decode_field_canonical::<u64>(&7_u64.to_le_bytes()),
                Err(Error::NestingDepthExceeded {
                    depth: 2,
                    limit: 1,
                    ..
                })
            ));
        },
    );
    assert_eq!(DECODE_NESTING_DEPTH.with(Cell::get), baseline);
}
