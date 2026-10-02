//! Sole action codec preserving original topology field bytes and schema while expiry is boxed.

use super::*;
use norito::core as ncore;

impl iroha_schema::TypeId for TopologyActionV1 {
    fn id() -> iroha_schema::Ident {
        "TopologyActionV1".to_owned()
    }
}

impl iroha_schema::IntoSchema for TopologyActionV1 {
    fn type_name() -> iroha_schema::Ident {
        "TopologyActionV1".to_owned()
    }

    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if map.contains_key::<Self>() {
            return;
        }
        macro_rules! variant {
            ($tag:literal, $index:literal, $ty:ty) => {
                iroha_schema::EnumVariant {
                    tag: $tag.to_owned(),
                    discriminant: $index,
                    ty: Some(core::any::TypeId::of::<$ty>()),
                }
            };
        }
        map.insert::<Self>(iroha_schema::Metadata::Enum(iroha_schema::EnumMeta {
            variants: vec![
                variant!("Configure", 0, Vec<u8>),
                variant!("Enroll", 1, Vec<u8>),
                variant!("Revoke", 2, TopologyRevocationV1),
                variant!("Reserve", 3, Box<TopologyReserveV1>),
                variant!("Complete", 4, Box<TopologyCompleteV1>),
                variant!("Expire", 5, TopologyExpireV1),
                variant!("Check", 6, Box<TopologyCheckV1>),
            ],
        }));
        <Vec<u8> as iroha_schema::IntoSchema>::update_schema_map(map);
        <TopologyRevocationV1 as iroha_schema::IntoSchema>::update_schema_map(map);
        <Box<TopologyReserveV1> as iroha_schema::IntoSchema>::update_schema_map(map);
        <Box<TopologyCompleteV1> as iroha_schema::IntoSchema>::update_schema_map(map);
        <TopologyExpireV1 as iroha_schema::IntoSchema>::update_schema_map(map);
        <Box<TopologyCheckV1> as iroha_schema::IntoSchema>::update_schema_map(map);
    }
}

impl TopologyActionV1 {
    fn tag_and_payload(&self) -> (u32, &dyn ncore::SerializePayload) {
        match self {
            Self::Configure(value) => (0, value),
            Self::Enroll(value) => (1, value),
            Self::Revoke(value) => (2, value),
            Self::Reserve(value) => (3, value),
            Self::Complete(value) => (4, value),
            // Only expiry changed its memory ownership; the other boxed variants
            // retain their existing owned-value prefix without reinterpretation.
            Self::Expire(value) => (5, value.as_ref()),
            Self::Check(value) => (6, value),
        }
    }
}

impl ncore::SerializePayload for TopologyActionV1 {
    fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        let (tag, payload) = self.tag_and_payload();
        ncore::SerializePayload::serialize(&tag, writer)?;
        ncore::write_len_prefixed(writer, payload)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        let (_, payload) = self.tag_and_payload();
        let length = payload.encoded_len_hint()?;
        4usize
            .checked_add(ncore::len_prefix_len(length))?
            .checked_add(length)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        let (_, payload) = self.tag_and_payload();
        let length = payload.encoded_len_exact()?;
        4usize
            .checked_add(ncore::len_prefix_len(length))?
            .checked_add(length)
    }
}

impl<'de> ncore::DeserializePayload<'de> for TopologyActionV1 {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical topology action")
    }

    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let bytes = ncore::payload_slice_from_ptr(core::ptr::from_ref(archived).cast::<u8>())?;
        let (value, used) = <Self as ncore::DecodeFromSlice>::decode_from_slice(bytes)?;
        if used != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok(value)
    }
}

impl<'de> ncore::DecodeFromSlice<'de> for TopologyActionV1 {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), ncore::Error> {
        fn decode<T>(field: &[u8]) -> Result<T, ncore::Error>
        where
            T: ncore::SerializePayload + for<'a> ncore::DeserializePayload<'a>,
        {
            let (value, used) = ncore::decode_field_canonical::<T>(field)?;
            if used != field.len() {
                return Err(ncore::Error::LengthMismatch);
            }
            Ok(value)
        }

        let tag_bytes = bytes.get(..4).ok_or(ncore::Error::LengthMismatch)?;
        let tag = u32::from_le_bytes(
            tag_bytes
                .try_into()
                .map_err(|_| ncore::Error::LengthMismatch)?,
        );
        if tag > 6 {
            return Err(ncore::Error::Message(format!(
                "invalid topology action tag {tag}"
            )));
        }
        let (length, prefix) = ncore::read_len_from_slice(&bytes[4..])?;
        let start = 4usize
            .checked_add(prefix)
            .ok_or(ncore::Error::LengthMismatch)?;
        let end = start
            .checked_add(length)
            .ok_or(ncore::Error::LengthMismatch)?;
        if end != bytes.len() {
            return Err(ncore::Error::LengthMismatch);
        }
        let field = bytes.get(start..end).ok_or(ncore::Error::LengthMismatch)?;
        let value = match tag {
            0 => Self::Configure(decode(field)?),
            1 => Self::Enroll(decode(field)?),
            2 => Self::Revoke(decode(field)?),
            3 => Self::Reserve(decode(field)?),
            4 => Self::Complete(decode(field)?),
            5 => {
                let value = decode::<TopologyExpireV1>(field)?;
                ncore::reserve_decode_box_allocation::<TopologyExpireV1>()?;
                Self::Expire(Box::new(value))
            }
            6 => Self::Check(decode(field)?),
            _ => unreachable!("tag checked before field decoding"),
        };
        ncore::note_payload_access(bytes, end);
        Ok((value, end))
    }
}

#[cfg(test)]
mod tests;
