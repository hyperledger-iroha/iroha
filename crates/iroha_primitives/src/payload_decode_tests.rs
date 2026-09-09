//! Collection decoding requires payload serialization, without a frame serializer.

use crate::{const_vec::ConstVec, unique_vec::UniqueVec};
use norito::{
    DeserializePayload, SerializePayload,
    codec::{Decode as _, Encode as _},
    core::{self, Archived, Encoder},
};

#[derive(Debug, PartialEq)]
struct Leaf(u32);

impl SerializePayload for Leaf {
    fn serialize(&self, writer: &mut Encoder<'_>) -> Result<(), norito::Error> {
        self.0.serialize(writer)
    }
}

impl<'de> DeserializePayload<'de> for Leaf {
    fn deserialize(archived: &'de Archived<Self>) -> Self {
        Self::try_deserialize(archived).unwrap()
    }

    fn try_deserialize(archived: &'de Archived<Self>) -> Result<Self, norito::Error> {
        u32::try_deserialize(archived.cast()).map(Self)
    }
}

fn roundtrip<T>(value: &T)
where
    T: for<'de> DeserializePayload<'de> + SerializePayload + std::fmt::Debug + PartialEq,
{
    assert_eq!(&T::decode(&mut value.encode().as_slice()).unwrap(), value);
    for requested in (0..=core::supported_header_flags())
        .filter(|flags| core::validate_header_flags(*flags).is_ok())
    {
        let _requested = core::DecodeFlagsGuard::enter(requested);
        let (bytes, actual) = norito::codec::encode_with_header_flags(value);
        let _actual = core::DecodeFlagsGuard::enter(actual);
        let (decoded, used) = core::decode_field_canonical::<T>(&bytes).unwrap();
        assert_eq!(&decoded, value);
        assert_eq!(used, bytes.len());
    }
}

#[test]
fn const_vec_decodes_payload_only_elements_in_every_layout() {
    roundtrip(&ConstVec::<Leaf>::new_empty());
    roundtrip(&ConstVec::new(vec![Leaf(7), Leaf(11)]));
}

#[test]
fn unique_vec_decodes_payload_only_elements_in_every_layout() {
    roundtrip(&UniqueVec::<Leaf>::new());
    roundtrip(&[Leaf(7), Leaf(11)].into_iter().collect::<UniqueVec<_>>());
}
