//! Metadata: key-value pairs that can be attached to accounts, transactions and assets.

#[cfg(not(feature = "std"))]
use alloc::{collections::btree_map, format, string::String, vec::Vec};
use core::borrow::Borrow;
#[cfg(feature = "std")]
use std::collections::btree_map;

use derive_more::Display;
use iroha_data_model_derive::model;
use iroha_schema::IntoSchema;
use parity_scale_codec::{Decode, Encode};
use serde::{Deserialize, Serialize};

pub use self::model::*;
use crate::{Name, Value};

/// Collection of parameters by their names.
pub type UnlimitedMetadata = btree_map::BTreeMap<Name, Value>;

#[model]
pub mod model {
    use super::*;

    /// Limits for [`Metadata`].
    #[derive(
        Debug,
        Display,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Decode,
        Encode,
        Deserialize,
        Serialize,
        IntoSchema,
    )]
    #[display(fmt = "{max_len},{max_entry_byte_size}_ML")]
    #[ffi_type]
    pub struct Limits {
        /// Maximum number of entries
        pub max_len: u32,
        /// Maximum length of entry
        pub max_entry_byte_size: u32,
    }

    /// Collection of parameters by their names with checked insertion.
    #[derive(
        Debug,
        Display,
        Clone,
        Default,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Deserialize,
        Serialize,
        Decode,
        Encode,
        IntoSchema,
    )]
    #[allow(clippy::multiple_inherent_impl)]
    #[display(fmt = "Metadata")]
    #[ffi_type(opaque)]
    #[serde(transparent)]
    #[repr(transparent)]
    pub struct Metadata {
        pub(super) map: btree_map::BTreeMap<Name, Value>,
    }
}

/// Metadata related errors.
#[derive(
    Debug,
    displaydoc::Display,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
)]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub enum MetadataError {
    /// Metadata entry is too big
    EntryTooBig(#[cfg_attr(feature = "std", source)] SizeError),
    /// Metadata exceeds overall length limit
    OverallSize(#[cfg_attr(feature = "std", source)] SizeError),
    /// Path specification empty
    EmptyPath,
    /// `{0}`: path segment not found, i.e. nothing was found at that key
    MissingSegment(Name),
    /// `{0}`: path segment not an instance of metadata
    InvalidSegment(Name),
}

/// Size limits exhaustion error
#[derive(
    Debug,
    Display,
    Copy,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Decode,
    Encode,
    Deserialize,
    Serialize,
    IntoSchema,
)]
#[display(fmt = "Limits are {limits}, while the actual value is {actual}")]
#[cfg_attr(feature = "std", derive(thiserror::Error))]
pub struct SizeError {
    /// The limits that were set for this entry
    limits: Limits,
    /// The actual *entry* size in bytes
    actual: u64,
}

impl Limits {
    /// Constructor.
    pub const fn new(max_len: u32, max_entry_byte_size: u32) -> Limits {
        Limits {
            max_len,
            max_entry_byte_size,
        }
    }
}

/// A path slice, composed of [`Name`]s.
pub type Path = [Name];

impl Metadata {
    /// Constructor.
    #[inline]
    pub fn new() -> Self {
        Self {
            map: UnlimitedMetadata::new(),
        }
    }

    /// Get the (expensive) cumulative length of all [`Value`]s housed
    /// in this map.
    pub fn nested_len(&self) -> usize {
        self.map.values().map(|v| 1 + v.len()).sum()
    }

    /// Get metadata given path. If the path is malformed, or
    /// incorrect (if e.g. any of interior path segments are not
    /// [`Metadata`] instances return `None`. Else borrow the value
    /// corresponding to that path.
    pub fn nested_get(&self, path: &Path) -> Option<&Value> {
        let key = path.last()?;
        let mut map = &self.map;
        for k in path.iter().take(path.len() - 1) {
            map = match map.get(k)? {
                Value::LimitedMetadata(data) => &data.map,
                _ => return None,
            };
        }
        map.get(key)
    }

    /// Check if the internal map contains the given key.
    pub fn contains(&self, key: &Name) -> bool {
        self.map.contains_key(key)
    }

    /// Iterate over key/value pairs stored in the internal map.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Name, &Value)> {
        self.map.iter()
    }

    /// Get the `Some(&Value)` associated to `key`. Return `None` if not found.
    #[inline]
    pub fn get<K: Ord + ?Sized>(&self, key: &K) -> Option<&Value>
    where
        Name: Borrow<K>,
    {
        self.map.get(key)
    }

    /// Insert the given [`Value`] into the given path. If the path is
    /// complete, check the limits and only then insert. The creation
    /// of the path is the responsibility of the user.
    ///
    /// # Errors
    /// - If the path is empty.
    /// - If one of the intermediate keys is absent.
    /// - If some intermediate key is a leaf node.
    pub fn nested_insert_with_limits(
        &mut self,
        path: &Path,
        value: Value,
        limits: Limits,
    ) -> Result<Option<Value>, MetadataError> {
        if self.map.len() >= limits.max_len as usize {
            return Err(MetadataError::OverallSize(SizeError {
                limits,
                actual: self
                    .map
                    .len()
                    .try_into()
                    .expect("`usize` should always fit into `u64`"),
            }));
        }
        let key = path.last().ok_or(MetadataError::EmptyPath)?;
        let mut layer = self;
        for k in path.iter().take(path.len() - 1) {
            layer = match layer
                .map
                .get_mut(k)
                .ok_or_else(|| MetadataError::MissingSegment(k.clone()))?
            {
                Value::LimitedMetadata(data) => data,
                _ => return Err(MetadataError::InvalidSegment(k.clone())),
            };
        }
        layer.insert_with_limits(key.clone(), value, limits)
    }

    /// Insert [`Value`] under the given key.  Returns `Some(value)`
    /// if the value was already present, `None` otherwise.
    ///
    /// # Errors
    /// Fails if `max_entry_byte_size` or `max_len` from `limits` are exceeded.
    pub fn insert_with_limits(
        &mut self,
        key: Name,
        value: Value,
        limits: Limits,
    ) -> Result<Option<Value>, MetadataError> {
        if self.map.len() >= limits.max_len as usize && !self.map.contains_key(&key) {
            return Err(MetadataError::OverallSize(SizeError {
                limits,
                actual: self
                    .map
                    .len()
                    .try_into()
                    .expect("`usize` should always fit into `u64`"),
            }));
        }
        check_size_limits(&key, value.clone(), limits)?;
        Ok(self.map.insert(key, value))
    }
}

#[cfg(feature = "transparent_api")]
impl Metadata {
    /// Removes a key from the map, returning the owned
    /// `Some(value)` at the key if the key was previously in the
    /// map, else `None`.
    #[inline]
    pub fn remove<K: Ord + ?Sized>(&mut self, key: &K) -> Option<Value>
    where
        Name: Borrow<K>,
    {
        self.map.remove(key)
    }

    /// Remove leaf node in metadata, given path. If the path is
    /// malformed, or incorrect (if e.g. any of interior path segments
    /// are not [`Metadata`] instances) return `None`. Else return the
    /// owned value corresponding to that path.
    pub fn nested_remove(&mut self, path: &Path) -> Option<Value> {
        let key = path.last()?;
        let mut map = &mut self.map;
        for k in path.iter().take(path.len() - 1) {
            map = match map.get_mut(k)? {
                Value::LimitedMetadata(data) => &mut data.map,
                _ => return None,
            };
        }
        map.remove(key)
    }
}

fn check_size_limits(key: &Name, value: Value, limits: Limits) -> Result<(), MetadataError> {
    let entry_bytes: Vec<u8> = (key, value).encode();
    let byte_size = entry_bytes.len();
    if byte_size > limits.max_entry_byte_size as usize {
        return Err(MetadataError::EntryTooBig(SizeError {
            limits,
            actual: byte_size
                .try_into()
                .expect("`usize` should always fit into `u64`"),
        }));
    }
    Ok(())
}

pub mod prelude {
    //! Prelude: re-export most commonly used traits, structs and macros from this module.
    pub use super::{Limits as MetadataLimits, Metadata, UnlimitedMetadata};
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "std"))]
    use alloc::{borrow::ToOwned as _, vec};
    use core::str::FromStr as _;

    use iroha_macro::FromVariant;

    use super::*;
    use crate::ParseError;

    /// Error used in testing to make text more readable using the `?` operator.
    #[derive(Debug, Display, Clone, FromVariant)]
    pub enum TestError {
        Parse(ParseError),
        Metadata(MetadataError),
    }

    #[test]
    fn nested_fns_ignore_empty_path() {
        let mut metadata = Metadata::new();
        let empty_path = vec![];
        assert!(metadata.nested_get(&empty_path).is_none());
        assert!(metadata
            .nested_insert_with_limits(&empty_path, "0".to_owned().into(), Limits::new(12, 12))
            .is_err());
        #[cfg(feature = "transparent_api")]
        assert!(metadata.nested_remove(&empty_path).is_none());
    }

    #[test]
    #[cfg(feature = "transparent_api")]
    fn nesting_inserts_removes() -> Result<(), TestError> {
        let mut metadata = Metadata::new();
        let limits = Limits::new(1024, 1024);
        // TODO: If we allow a `unsafe`, we could create the path.
        metadata
            .insert_with_limits(Name::from_str("0")?, Metadata::new().into(), limits)
            .expect("Valid");
        metadata
            .nested_insert_with_limits(
                &[Name::from_str("0")?, Name::from_str("1")?],
                Metadata::new().into(),
                limits,
            )
            .expect("Valid");
        let path = [
            Name::from_str("0")?,
            Name::from_str("1")?,
            Name::from_str("2")?,
        ];
        metadata
            .nested_insert_with_limits(&path, "Hello World".to_owned().into(), limits)
            .expect("Valid");
        assert_eq!(
            *metadata.nested_get(&path).expect("Valid"),
            Value::from("Hello World".to_owned())
        );
        assert_eq!(metadata.nested_len(), 6); // Three nested path segments.
        metadata.nested_remove(&path);
        assert!(metadata.nested_get(&path).is_none());
        Ok(())
    }

    #[test]
    fn non_existent_path_segment_fails() -> Result<(), TestError> {
        let mut metadata = Metadata::new();
        let limits = Limits::new(10, 15);
        metadata.insert_with_limits(Name::from_str("0")?, Metadata::new().into(), limits)?;
        metadata.nested_insert_with_limits(
            &[Name::from_str("0")?, Name::from_str("1")?],
            Metadata::new().into(),
            limits,
        )?;
        let path = vec![
            Name::from_str("0")?,
            Name::from_str("1")?,
            Name::from_str("2")?,
        ];
        metadata.nested_insert_with_limits(&path, "Hello World".to_owned().into(), limits)?;
        let bad_path = vec![
            Name::from_str("0")?,
            Name::from_str("3")?,
            Name::from_str("2")?,
        ];
        assert!(metadata
            .nested_insert_with_limits(&bad_path, "Hello World".to_owned().into(), limits)
            .is_err());
        assert!(metadata.nested_get(&bad_path).is_none());
        #[cfg(feature = "transparent_api")]
        assert!(metadata.nested_remove(&bad_path).is_none());
        Ok(())
    }

    #[test]
    fn nesting_respects_limits() -> Result<(), TestError> {
        let mut metadata = Metadata::new();
        let limits = Limits::new(10, 14);
        // TODO: If we allow a `unsafe`, we could create the path.
        metadata.insert_with_limits(Name::from_str("0")?, Metadata::new().into(), limits)?;
        metadata
            .nested_insert_with_limits(
                &[Name::from_str("0")?, Name::from_str("1")?],
                Metadata::new().into(),
                limits,
            )
            .expect("Valid");
        let path = vec![
            Name::from_str("0")?,
            Name::from_str("1")?,
            Name::from_str("2")?,
        ];
        let failing_insert =
            metadata.nested_insert_with_limits(&path, "Hello World".to_owned().into(), limits);

        assert!(failing_insert.is_err());
        Ok(())
    }

    #[test]
    fn insert_exceeds_entry_size() -> Result<(), TestError> {
        let mut metadata = Metadata::new();
        let limits = Limits::new(10, 5);
        assert!(metadata
            .insert_with_limits(Name::from_str("1")?, "2".to_owned().into(), limits)
            .is_ok());
        assert!(metadata
            .insert_with_limits(Name::from_str("1")?, "23456".to_owned().into(), limits)
            .is_err());
        Ok(())
    }

    #[test]
    // This test is a good candidate for both property-based and parameterised testing
    fn insert_exceeds_len() -> Result<(), TestError> {
        let mut metadata = Metadata::new();
        let limits = Limits::new(2, 5);
        assert!(metadata
            .insert_with_limits(Name::from_str("1")?, "0".to_owned().into(), limits)
            .is_ok());
        assert!(metadata
            .insert_with_limits(Name::from_str("2")?, "0".to_owned().into(), limits)
            .is_ok());
        assert!(metadata
            .insert_with_limits(Name::from_str("2")?, "1".to_owned().into(), limits)
            .is_ok());
        assert!(metadata
            .insert_with_limits(Name::from_str("3")?, "0".to_owned().into(), limits)
            .is_err());
        Ok(())
    }
}
