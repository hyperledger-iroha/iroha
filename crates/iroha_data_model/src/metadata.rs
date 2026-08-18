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
    pub struct Metadata(pub(super) BTreeMap<Name, Json>);
}
impl ncore::NoritoSerialize for Metadata {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), ncore::Error> {
        let len = self.0.len();
        ncore::WriteBytesExt::write_u64::<ncore::LittleEndian>(writer, len as u64)?;
        if ncore::use_packed_seq() {
            let mut offsets: Vec<u64> = Vec::with_capacity(len + 1);
            let mut data = Vec::new();
            let mut entry_buf = Vec::new();
            let mut total: u64 = 0;
            for (name, json) in &self.0 {
                offsets.push(total);
                entry_buf.clear();
                serialize_entry(&mut entry_buf, name, json)?;
                total = total.wrapping_add(entry_buf.len() as u64);
                data.extend_from_slice(&entry_buf);
            }
            offsets.push(total);
            let mut offs_bytes = Vec::with_capacity(offsets.len() * 8);
            for off in offsets {
                offs_bytes.extend_from_slice(&off.to_le_bytes());
            }
            std::io::Write::write_all(writer, &offs_bytes)?;
            std::io::Write::write_all(writer, &data)?;
            return Ok(());
        }
        let mut entry_buf = Vec::new();
        for (name, json) in &self.0 {
            entry_buf.clear();
            serialize_entry(&mut entry_buf, name, json)?;
            if ncore::use_compact_len() {
                ncore::write_len(writer, entry_buf.len() as u64)?;
            } else {
                ncore::WriteBytesExt::write_u64::<ncore::LittleEndian>(
                    writer,
                    entry_buf.len() as u64,
                )?;
            }
            std::io::Write::write_all(writer, &entry_buf)?;
        }
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        let mut total: usize = 8;
        if ncore::use_packed_seq() {
            let len = self.0.len();
            let mut data_total = 0usize;
            for (name, json) in &self.0 {
                let entry = entry_len_hint(name, json)?;
                data_total = data_total.saturating_add(entry);
            }
            total = total
                .saturating_add(8usize.saturating_mul(len.saturating_add(1)))
                .saturating_add(data_total);
            return Some(total);
        }
        for (name, json) in &self.0 {
            let entry = entry_len_hint(name, json)?;
            total = total.saturating_add(8).saturating_add(entry);
        }
        Some(total)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        let len = self.0.len();
        let mut total: usize = 8;
        if ncore::use_packed_seq() {
            let mut data_total = 0usize;
            for (name, json) in &self.0 {
                let entry = entry_len_exact(name, json)?;
                data_total = data_total.saturating_add(entry);
            }
            total = total
                .saturating_add(8usize.saturating_mul(len.saturating_add(1)))
                .saturating_add(data_total);
            return Some(total);
        }
        for (name, json) in &self.0 {
            let entry = entry_len_exact(name, json)?;
            total = total
                .saturating_add(ncore::len_prefix_len(entry))
                .saturating_add(entry);
        }
        Some(total)
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
fn serialize_entry<W: std::io::Write>(
    writer: &mut W,
    name: &Name,
    json: &Json,
) -> Result<(), ncore::Error> {
    let current = ncore::get_decode_flags();
    let merged = if current == 0 {
        ncore::default_encode_flags()
    } else {
        current | ncore::default_encode_flags()
    };
    let _guard = ncore::DecodeFlagsGuard::enter(merged);
    let mut buf = ncore::DeriveSmallBuf::new();
    buf.clear();
    ncore::serialize_to_writer(name, &mut buf)?;
    if ncore::use_compact_len() {
        ncore::write_len(writer, buf.len() as u64)?;
    } else {
        ncore::WriteBytesExt::write_u64::<ncore::LittleEndian>(writer, buf.len() as u64)?;
    }
    std::io::Write::write_all(writer, buf.as_slice())?;
    buf.clear();
    ncore::serialize_to_writer(json, &mut buf)?;
    if ncore::use_compact_len() {
        ncore::write_len(writer, buf.len() as u64)?;
    } else {
        ncore::WriteBytesExt::write_u64::<ncore::LittleEndian>(writer, buf.len() as u64)?;
    }
    std::io::Write::write_all(writer, buf.as_slice())?;
    Ok(())
}
fn entry_len_hint(name: &Name, json: &Json) -> Option<usize> {
    let key = <Name as ncore::NoritoSerialize>::encoded_len_hint(name)?;
    let value = <Json as ncore::NoritoSerialize>::encoded_len_hint(json)?;
    Some(
        8usize
            .saturating_add(key)
            .saturating_add(8usize)
            .saturating_add(value),
    )
}
fn entry_len_exact(name: &Name, json: &Json) -> Option<usize> {
    let key = <Name as ncore::NoritoSerialize>::encoded_len_exact(name)?;
    let value = <Json as ncore::NoritoSerialize>::encoded_len_exact(json)?;
    Some(
        ncore::len_prefix_len(key)
            .saturating_add(key)
            .saturating_add(ncore::len_prefix_len(value))
            .saturating_add(value),
    )
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

    /// Remove one crate-internal protocol marker without exposing general
    /// mutation through the opaque public metadata API.
    pub(crate) fn remove_internal(&mut self, key: &Name) -> Option<Json> {
        self.0.remove(key)
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
