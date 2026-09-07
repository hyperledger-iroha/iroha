//! Metadata: key-value pairs that can be attached to accounts, transactions and assets.
pub use self::model::*;
use crate::prelude::Name;
use iroha_data_model_derive::model;
use iroha_primitives::json::Json;
use norito::core::{self as ncore};
use std::{borrow::Borrow, collections::BTreeMap, format, str::FromStr, string::String, vec::Vec};
/// A path slice, composed of [`Name`]s.
pub type Path = [Name];
#[model]
mod model {
    use super::*;
    use derive_more::Display;
    use iroha_schema::IntoSchema;
    /// Collection of parameters by their names with checked insertion.
    #[derive(Debug, Display, Clone, Default, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
    #[cfg_attr(any(feature = "ffi_export", feature = "ffi_import"), ffi_type(opaque))]
    #[repr(transparent)]
    #[display("Metadata")]
    #[allow(clippy::multiple_inherent_impl)]
    #[derive(norito::NoritoSchema)]
    #[norito_schema(name = "iroha_data_model::metadata::model::Metadata")]
    pub struct Metadata(pub(super) BTreeMap<Name, Json>);
}
impl ncore::NoritoSerialize for Metadata {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::write_seq_len(
            writer,
            u64::try_from(self.0.len()).map_err(|_| ncore::Error::LengthMismatch)?,
        )?;
        if ncore::use_packed_seq() {
            let allocation_bytes = self
                .0
                .len()
                .checked_mul(core::mem::size_of::<usize>())
                .and_then(|bytes| u64::try_from(bytes).ok())
                .ok_or(ncore::Error::LengthMismatch)?;
            let mut lengths = Vec::new();
            lengths.try_reserve_exact(self.0.len()).map_err(|_| {
                ncore::Error::AllocationFailed {
                    bytes: allocation_bytes,
                }
            })?;
            for (name, json) in &self.0 {
                lengths.push(ncore::encoded_payload_len(&MetadataEntryRef(name, json))?);
            }
            ncore::note_fixed_offsets_emitted();
            ncore::write_fixed_offsets(writer, &lengths)?;
            for ((name, json), length) in self.0.iter().zip(lengths) {
                ncore::serialize_to_writer_exact(&MetadataEntryRef(name, json), writer, length)?;
            }
            return Ok(());
        }
        // Use canonical field writers so entry lengths inherit the exact frame layout.
        for (name, json) in &self.0 {
            ncore::write_len_prefixed(writer, &MetadataEntryRef(name, json))?;
        }
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.encoded_len_exact()
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        let packed = ncore::use_packed_seq();
        let overhead = if packed {
            8_usize.checked_add(self.0.len().checked_add(1)?.checked_mul(8)?)?
        } else {
            8
        };
        self.0.iter().try_fold(overhead, |total, (name, json)| {
            let len = MetadataEntryRef(name, json).encoded_len_exact()?;
            let prefix = if packed {
                0
            } else {
                ncore::len_prefix_len(len)
            };
            total.checked_add(prefix)?.checked_add(len)
        })
    }
}
// Payload-only borrowed tuple view; never used as a separately framed wire record.
struct MetadataEntryRef<'a>(&'a Name, &'a Json);

impl ncore::NoritoSerialize for MetadataEntryRef<'_> {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::write_len_prefixed(writer, self.0)?;
        ncore::write_len_prefixed(writer, self.1)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        let name = self.0.encoded_len_exact()?;
        let json = self.1.encoded_len_exact()?;
        ncore::len_prefix_len(name)
            .checked_add(name)?
            .checked_add(ncore::len_prefix_len(json))?
            .checked_add(json)
    }
}

impl<'de> ncore::NoritoDeserialize<'de> for Metadata {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        let entries: Vec<(Name, Json)> =
            <Vec<(Name, Json)> as ncore::NoritoDeserialize>::deserialize(archived.cast());
        let mut map = BTreeMap::new();
        for (name, json) in entries {
            map.insert(name, json);
        }
        Metadata(map)
    }
    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let entries =
            <Vec<(Name, Json)> as ncore::NoritoDeserialize>::try_deserialize(archived.cast())?;
        let mut map = BTreeMap::new();
        for (name, json) in entries {
            if map.insert(name, json).is_some() {
                return Err(ncore::Error::Message("duplicate metadata key".into()));
            }
        }
        Ok(Metadata(map))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::{decode_adaptive, encode_adaptive};
    #[test]
    fn metadata_serialization_matches_vec_layout() {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("alpha").unwrap(),
            Json::from_raw_json("\"value\"".to_owned()).expect("valid JSON string fixture"),
        );
        metadata.insert(
            Name::from_str("beta").unwrap(),
            Json::from_raw_json("{\"nested\":true}".to_owned()).expect("valid nested JSON fixture"),
        );
        let mut metadata_bytes = Vec::new();
        ncore::serialize_to_buffer(&metadata, &mut metadata_bytes).unwrap();
        let reference: Vec<(Name, Json)> = metadata
            .0
            .iter()
            .map(|(name, json)| (name.clone(), json.clone()))
            .collect();
        let mut vec_bytes = Vec::new();
        ncore::serialize_to_buffer(&reference, &mut vec_bytes).unwrap();
        assert_eq!(metadata_bytes, vec_bytes);
        let hint = <Metadata as ncore::NoritoSerialize>::encoded_len_hint(&metadata)
            .expect("metadata hint");
        assert!(
            hint >= metadata_bytes.len(),
            "encoded_len_hint should not under-estimate"
        );
        assert_eq!(
            <Metadata as ncore::NoritoSerialize>::encoded_len_exact(&metadata),
            Some(metadata_bytes.len())
        );
    }
    #[test]
    fn metadata_roundtrip_preserves_entries() {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("foo").unwrap(),
            Json::from_raw_json("1".to_owned()).expect("valid numeric JSON fixture"),
        );
        metadata.insert(
            Name::from_str("bar").unwrap(),
            Json::from_raw_json("[1,2,3]".to_owned()).expect("valid array JSON fixture"),
        );
        let bytes = encode_adaptive(&metadata);
        let decoded: Metadata = decode_adaptive(&bytes).expect("decode metadata");
        assert_eq!(decoded, metadata);
    }

    #[test]
    fn metadata_entries_preserve_the_enclosing_layout() {
        use ncore::{DecodeFlagsGuard, NoritoSerialize, header_flags};

        let mut metadata = Metadata::default();
        metadata.insert("alpha".parse().unwrap(), Json::new("value"));
        metadata.insert("beta".parse().unwrap(), Json::new(vec![1, 2, 3]));
        let reference: Vec<_> = metadata
            .0
            .iter()
            .map(|(name, json)| (name.clone(), json.clone()))
            .collect();
        for requested in [
            0,
            header_flags::COMPACT_LEN,
            header_flags::PACKED_SEQ,
            header_flags::PACKED_SEQ | header_flags::COMPACT_LEN,
            header_flags::PACKED_STRUCT | header_flags::COMPACT_LEN,
            header_flags::PACKED_STRUCT
                | header_flags::PACKED_SEQ
                | header_flags::COMPACT_LEN
                | header_flags::FIELD_BITSET,
        ] {
            let _layout = DecodeFlagsGuard::enter(requested);
            let (payload, flags) = norito::codec::encode_with_header_flags(&metadata);
            assert_eq!(
                flags & header_flags::COMPACT_LEN,
                requested & header_flags::COMPACT_LEN,
                "metadata entries must not change the enclosing length format"
            );
            assert_eq!(
                (payload.clone(), flags),
                norito::codec::encode_with_header_flags(&reference),
                "metadata must preserve its canonical sequence-of-tuples layout"
            );
            assert_eq!(metadata.encoded_len_exact(), Some(payload.len()));
            assert!(metadata.encoded_len_hint().unwrap() >= payload.len());
            let frame = ncore::frame_bare_with_header_flags::<Metadata>(&payload, flags).unwrap();
            assert_eq!(
                norito::decode_from_bytes::<Metadata>(&frame).unwrap(),
                metadata
            );

            // Siblings on both sides expose a nested serializer changing the frame flags
            // after the first field has already emitted its length prefix.
            let parent = (17_u64, metadata.clone(), vec![3_u8, 5, 7]);
            let bytes = norito::to_bytes(&parent).unwrap();
            let decoded: (u64, Metadata, Vec<u8>) = norito::decode_from_bytes(&bytes).unwrap();
            assert_eq!(decoded, parent);
            assert_eq!(norito::to_bytes(&decoded).unwrap(), bytes);
        }
    }
}
#[cfg(feature = "json")]
impl norito::json::FastJsonWrite for Metadata {
    fn write_json(&self, out: &mut String) {
        out.push('{');
        let mut first = true;
        for (key, value) in &self.0 {
            if first {
                first = false;
            } else {
                out.push(',');
            }
            norito::json::JsonSerialize::json_serialize(key.as_ref(), out);
            out.push(':');
            norito::json::JsonSerialize::json_serialize(value, out);
        }
        out.push('}');
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        out.begin_container()?;
        out.push('{')?;
        for (index, (key, value)) in self.0.iter().enumerate() {
            if index != 0 {
                out.push(',')?;
            }
            norito::json::write_json_string_to(key.as_ref(), out)?;
            out.push(':')?;
            norito::json::JsonSerialize::json_serialize_to(value, out)?;
        }
        out.push('}')?;
        out.end_container();
        Ok(())
    }
}
#[cfg(feature = "json")]
impl norito::json::JsonDeserialize for Metadata {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let value = norito::json::Value::json_deserialize(parser)?;
        let map = match value {
            norito::json::Value::Object(map) => map,
            other => {
                return Err(norito::json::Error::InvalidField {
                    field: String::new(),
                    message: format!("expected object, found {other:?}"),
                });
            }
        };
        let mut out = BTreeMap::new();
        for (key, val) in map {
            let name = Name::from_str(&key).map_err(|err| norito::json::Error::InvalidField {
                field: key.clone(),
                message: err.reason.into(),
            })?;
            let json = Json::from_norito_value_ref(&val)
                .map_err(|e| norito::json::Error::Message(e.to_string()))?;
            out.insert(name, json);
        }
        Ok(Metadata(out))
    }
}
impl Metadata {
    /// Returns `true` when the metadata map has no entries.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Check if the internal map contains the given key.
    pub fn contains(&self, key: &Name) -> bool {
        self.0.contains_key(key)
    }
    /// Iterate over key/value pairs stored in the internal map.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Name, &Json)> {
        self.0.iter()
    }
    /// Get the `Some(&Value)` associated to `key`. Return `None` if not found.
    #[inline]
    pub fn get<K: Ord + ?Sized>(&self, key: &K) -> Option<&Json>
    where
        Name: Borrow<K>,
    {
        self.0.get(key)
    }
    /// Insert [`Json`] under the given key.  Returns `Some(value)`
    /// if the value was already present, `None` otherwise.
    pub fn insert(&mut self, key: Name, value: impl Into<Json>) -> Option<Json> {
        self.0.insert(key, value.into())
    }
}
#[cfg(feature = "transparent_api")]
impl Metadata {
    /// Removes a key from the map, returning the owned `Some(value)` at the key if the key was
    /// previously in the map, else `None`.
    #[inline]
    pub fn remove<K: Ord + ?Sized>(&mut self, key: &K) -> Option<Json>
    where
        Name: Borrow<K>,
    {
        self.0.remove(key)
    }
}
pub mod prelude {
    //! Prelude: re-export most commonly used traits, structs and macros from this module.
    pub use super::Metadata;
}
