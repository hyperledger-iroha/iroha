//! Canonical payload decoding and nested field resource boundaries.

use super::*;
use crate::codec::{Decode as _, Encode as _};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Hash)]
struct Leaf(u32);

impl SerializePayload for Leaf {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), Error> {
        self.0.serialize(writer)
    }
}

impl<'de> DeserializePayload<'de> for Leaf {
    fn deserialize(archived: &'de Archived<Self>) -> Self {
        Self::try_deserialize(archived).unwrap()
    }

    fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, Error> {
        u32::try_deserialize(archived.cast()).map(Self)
    }
}

#[derive(Debug, PartialEq, crate::SerializePayload, crate::DeserializePayload)]
struct PayloadRecord<T> {
    value: T,
    items: Vec<Option<T>>,
}

#[derive(Debug, PartialEq, crate::SerializePayload, crate::DeserializePayload)]
enum PayloadVariant<T> {
    Empty,
    Item(T),
    Named { value: T },
}

fn roundtrip<T>(value: T)
where
    T: for<'de> DeserializePayload<'de> + SerializePayload + std::fmt::Debug + PartialEq,
{
    let bare = value.encode();
    assert_eq!(T::decode(&mut bare.as_slice()).unwrap(), value);
    for requested in
        (0..=supported_header_flags()).filter(|flags| validate_header_flags(*flags).is_ok())
    {
        let _requested = DecodeFlagsGuard::enter(requested);
        let (bytes, actual) = encode_bare_with_flags(&value).unwrap();
        let _actual = DecodeFlagsGuard::enter(actual);
        let (decoded, used) = decode_field_canonical::<T>(&bytes).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(used, bytes.len());
    }
}

#[test]
fn bare_records_and_enum_fields_need_only_payload_serialization() {
    roundtrip(PayloadRecord {
        value: Leaf(0x1020_3040),
        items: vec![Some(Leaf(7)), None, Some(Leaf(u32::MAX))],
    });
    roundtrip(PayloadVariant::<Leaf>::Empty);
    roundtrip(PayloadVariant::Item(Leaf(7)));
    roundtrip(PayloadVariant::Named { value: Leaf(11) });
}

#[test]
fn bare_owned_values_and_collections_need_only_payload_serialization() {
    roundtrip(Box::new(Leaf(7)));
    roundtrip(Rc::new(Leaf(7)));
    roundtrip(Arc::new(Leaf(7)));
    roundtrip(Some(Leaf(7)));
    roundtrip(Option::<Leaf>::None);
    roundtrip(vec![Leaf(7), Leaf(11)]);
    roundtrip(VecDeque::from([Leaf(7), Leaf(11)]));
    roundtrip(LinkedList::from([Leaf(7), Leaf(11)]));
    roundtrip(BTreeSet::from([Leaf(7), Leaf(11)]));
    roundtrip(BTreeMap::from([(Leaf(7), Leaf(11))]));
    roundtrip(HashSet::from([Leaf(7), Leaf(11)]));
    roundtrip(HashMap::from([(Leaf(7), Leaf(11))]));
}

#[derive(Debug, PartialEq)]
struct PrefixChild(u8);

impl<'de> DecodeFromSlice<'de> for PrefixChild {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), Error> {
        bytes
            .first()
            .copied()
            .map(|value| (Self(value), 1))
            .ok_or(Error::LengthMismatch)
    }
}

#[test]
fn tuple_and_result_reject_unconsumed_child_bytes_with_typed_errors() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let mut exact = Vec::new();
        write_len_with_flags(&mut exact, 1, flags).unwrap();
        exact.push(7);
        let mut valid_tuple = exact.clone();
        valid_tuple.extend_from_slice(&exact);
        assert_eq!(
            <(PrefixChild, u8)>::decode_from_slice(&valid_tuple).unwrap(),
            ((PrefixChild(7), 7), valid_tuple.len())
        );
        for (tag, expected) in [(0, Ok(PrefixChild(7))), (1, Err(PrefixChild(7)))] {
            let mut valid_result = vec![tag];
            valid_result.extend_from_slice(&exact);
            assert_eq!(
                Result::<PrefixChild, PrefixChild>::decode_from_slice(&valid_result).unwrap(),
                (expected, valid_result.len())
            );
        }
        let mut child = Vec::new();
        write_len_with_flags(&mut child, 2, flags).unwrap();
        child.extend_from_slice(&[7, 8]);
        let mut tuple = child.clone();
        write_len_with_flags(&mut tuple, 1, flags).unwrap();
        tuple.push(9);
        assert!(matches!(
            <(PrefixChild, u8)>::decode_from_slice(&tuple),
            Err(Error::LengthMismatch)
        ));
        for tag in [0, 1] {
            let mut result = vec![tag];
            result.extend_from_slice(&child);
            assert!(matches!(
                Result::<PrefixChild, PrefixChild>::decode_from_slice(&result),
                Err(Error::LengthMismatch)
            ));
        }
    }
}

#[test]
fn option_slice_decoder_accepts_exact_child_and_reports_outer_prefix() {
    let _flags = DecodeFlagsGuard::enter(0);
    let mut bytes = vec![1];
    write_len_with_flags(&mut bytes, 4, 0).unwrap();
    bytes.extend_from_slice(&7_u32.to_le_bytes());
    let used = bytes.len();
    bytes.push(0xFF);
    assert_eq!(
        Option::<Leaf>::decode_from_slice(&bytes).unwrap(),
        (Some(Leaf(7)), used)
    );
}

#[test]
fn option_slice_decoder_obeys_child_field_and_depth_limits() {
    let _flags = DecodeFlagsGuard::enter(default_encode_flags());
    let bytes = Some(7_u8).encode();
    let limits = DecodeLimits::new(100, 0, 100, 100, MAX_VALUE_NESTING_DEPTH);
    assert!(matches!(
        with_decode_limits(limits, || Option::<u8>::decode_from_slice(&bytes)),
        Err(Error::FieldLengthExceeded { .. })
    ));
    let limits = DecodeLimits::new(100, 100, 100, 100, 0);
    assert!(matches!(
        with_decode_limits(limits, || Option::<u8>::decode_from_slice(&bytes)),
        Err(Error::NestingDepthExceeded { .. })
    ));
    assert_eq!(
        Option::<u8>::decode_from_slice(&bytes).unwrap(),
        (Some(7), bytes.len())
    );
}

#[test]
fn option_slice_decoder_rejects_unconsumed_child_bytes() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let mut bytes = vec![1];
        write_len_with_flags(&mut bytes, 2, flags).unwrap();
        bytes.extend_from_slice(&[7, 8]);
        assert!(matches!(
            Option::<u8>::decode_from_slice(&bytes),
            Err(Error::LengthMismatch)
        ));
    }
}

#[derive(Debug, PartialEq)]
struct NestedResult(usize);

impl<'de> DecodeFromSlice<'de> for NestedResult {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), Error> {
        let (value, used) = Result::<u8, Self>::decode_from_slice(bytes)?;
        let depth = match value {
            Ok(_) => 1,
            Err(child) => child.0 + 1,
        };
        Ok((Self(depth), used))
    }
}

#[test]
fn result_slice_decoder_enforces_depth_for_both_branches_and_nested_results() {
    for flags in [0, header_flags::COMPACT_LEN] {
        let _flags = DecodeFlagsGuard::enter(flags);
        let limits = DecodeLimits::new(4096, 4096, 4096, 4096, 0);
        for tag in [0, 1] {
            let mut bytes = vec![tag];
            write_len_with_flags(&mut bytes, 1, flags).unwrap();
            bytes.push(7);
            assert!(matches!(
                with_decode_limits(limits, || Result::<u8, u8>::decode_from_slice(&bytes)),
                Err(Error::NestingDepthExceeded { .. })
            ));
            let expected = if tag == 0 { Ok(7) } else { Err(7) };
            assert_eq!(
                Result::<u8, u8>::decode_from_slice(&bytes).unwrap(),
                (expected, bytes.len())
            );
        }
        let mut bytes = vec![0];
        write_len_with_flags(&mut bytes, 1, flags).unwrap();
        bytes.push(7);
        let shallow = bytes.clone();
        for _ in 0..3 {
            let mut parent = vec![1];
            write_len_with_flags(&mut parent, u64::try_from(bytes.len()).unwrap(), flags).unwrap();
            parent.extend_from_slice(&bytes);
            bytes = parent;
        }
        let limits = DecodeLimits::new(4096, 4096, 4096, 4096, 3);
        with_decode_limits(limits, || {
            assert!(matches!(
                NestedResult::decode_from_slice(&bytes),
                Err(Error::NestingDepthExceeded {
                    depth: 4,
                    limit: 3,
                    ..
                })
            ));
            assert_eq!(
                NestedResult::decode_from_slice(&shallow).unwrap(),
                (NestedResult(1), shallow.len())
            );
            Ok(())
        })
        .unwrap();
        let limits = DecodeLimits::new(4096, 4096, 4096, 4096, 1);
        with_decode_limits(limits, || {
            for tag in [0, 1] {
                let mut short_child = shallow.clone();
                short_child[0] = tag;
                assert!(matches!(
                    Result::<u16, u16>::decode_from_slice(&short_child),
                    Err(Error::LengthMismatch)
                ));
                let expected = if tag == 0 { Ok(7) } else { Err(7) };
                assert_eq!(
                    Result::<u8, u8>::decode_from_slice(&short_child).unwrap(),
                    (expected, short_child.len())
                );
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            NestedResult::decode_from_slice(&bytes).unwrap(),
            (NestedResult(4), bytes.len())
        );
    }
}

#[derive(Debug, PartialEq, crate::SerializePayload, crate::DeserializePayload)]
#[norito(validate = "Self::validate_decoded", decode_from_slice)]
struct ValidatedPayload(Leaf);

static PAYLOAD_VALIDATIONS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

impl ValidatedPayload {
    fn validate_decoded(self) -> Result<Self, Error> {
        PAYLOAD_VALIDATIONS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if self.0.0 == 0 {
            return Err(Error::Message("zero payload value is forbidden".into()));
        }
        Ok(self)
    }
}

#[test]
fn payload_only_decoders_preserve_validation_and_exact_consumption() {
    use std::sync::atomic::Ordering::Relaxed;

    for requested in
        (0..=supported_header_flags()).filter(|flags| validate_header_flags(*flags).is_ok())
    {
        let _requested = DecodeFlagsGuard::enter(requested);
        for number in [0, 7] {
            let value = ValidatedPayload(Leaf(number));
            let (bytes, actual) = encode_bare_with_flags(&value).unwrap();
            let _actual = DecodeFlagsGuard::enter(actual);
            let before = PAYLOAD_VALIDATIONS.load(Relaxed);
            let decoded = decode_field_canonical::<ValidatedPayload>(&bytes);
            assert_eq!(PAYLOAD_VALIDATIONS.load(Relaxed), before + 1);
            if number == 0 {
                assert!(
                    matches!(decoded, Err(Error::Message(message)) if message == "zero payload value is forbidden")
                );
            } else {
                assert_eq!(decoded.unwrap(), (value, bytes.len()));
            }
            let before = PAYLOAD_VALIDATIONS.load(Relaxed);
            let decoded = ValidatedPayload::decode_from_slice(&bytes);
            assert_eq!(PAYLOAD_VALIDATIONS.load(Relaxed), before + 1);
            if number == 0 {
                assert!(
                    matches!(decoded, Err(Error::Message(message)) if message == "zero payload value is forbidden")
                );
            } else {
                assert_eq!(
                    decoded.unwrap(),
                    (ValidatedPayload(Leaf(number)), bytes.len())
                );
                let mut trailing = bytes.clone();
                trailing.push(0xFF);
                assert!(matches!(
                    decode_field_canonical::<ValidatedPayload>(&trailing),
                    Err(Error::LengthMismatch)
                ));
            }
            for length in 0..bytes.len() {
                assert!(decode_field_canonical::<ValidatedPayload>(&bytes[..length]).is_err());
            }
        }
    }
}
