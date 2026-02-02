//! A vector that silently deduplicates its elements.
//!
//! [`UniqueVec`] wraps a standard [`Vec`] but ensures that each inserted value
//! appears at most once. The accompanying `unique_vec!`
//! macro offers a convenient way to construct such collections inline,
//! discarding any duplicates in the provided list.

use core::borrow::Borrow;
use std::{io::Write, vec::Vec};

use derive_more::{AsRef, Deref};
use iroha_schema::IntoSchema;
use norito::{
    NoritoDeserialize, NoritoSerialize, core as ncore,
    json::{self, JsonDeserialize, JsonSerialize},
};

/// Creates a [`UniqueVec`](crate::unique_vec::UniqueVec) from a list of values.
///
/// Works like [`vec!`] but ignores duplicate entries so the resulting
/// collection contains only distinct elements.
#[macro_export]
macro_rules! unique_vec {
    () => {
        $crate::unique_vec::UniqueVec::new()
    };
    ($($x:expr),+ $(,)?) => {{
        let mut v = $crate::unique_vec::UniqueVec::new();
        $(v.push($x);)+
        v
    }};
}

/// Wrapper type for [`Vec`] which ensures that all elements are unique.
#[derive(Debug, Deref, AsRef, Clone, PartialEq, Eq, PartialOrd, Ord, IntoSchema)]
#[repr(transparent)]
#[schema(transparent)]
pub struct UniqueVec<T>(Vec<T>);

impl<T> UniqueVec<T> {
    /// Create new [`UniqueVec`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Removes the element at the given `index` and returns it.
    ///
    /// # Panics
    ///
    /// Panics if the `index` is out of bounds.
    pub fn remove(&mut self, index: usize) -> T {
        self.0.remove(index)
    }

    /// Clears the [`UniqueVec`], removing all values.
    pub fn clear(&mut self) {
        self.0.clear();
    }
}

/// A result for [`UniqueVec::push`]
#[derive(Debug)]
pub enum PushResult<T> {
    /// The element was pushed into the vec
    Ok,
    /// The element is already contained in the vec
    Duplicate(T),
}

impl<T: PartialEq> UniqueVec<T> {
    /// Push `value` to [`UniqueVec`] if it is not already present.
    pub fn push(&mut self, value: T) -> PushResult<T> {
        if self.contains(&value) {
            PushResult::Duplicate(value)
        } else {
            self.0.push(value);
            PushResult::Ok
        }
    }
}

impl<T> Default for UniqueVec<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T: PartialEq> FromIterator<T> for UniqueVec<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut unique_vec = Self::new();
        unique_vec.extend(iter);
        unique_vec
    }
}

impl<T: PartialEq> Extend<T> for UniqueVec<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }
}

impl<T: PartialEq> From<UniqueVec<T>> for Vec<T> {
    fn from(value: UniqueVec<T>) -> Self {
        value.0
    }
}

impl<T: PartialEq> Borrow<[T]> for UniqueVec<T> {
    fn borrow(&self) -> &[T] {
        self.0.borrow()
    }
}

impl<T: PartialEq> IntoIterator for UniqueVec<T> {
    type Item = T;
    type IntoIter = <Vec<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'ve, T: PartialEq> IntoIterator for &'ve UniqueVec<T> {
    type Item = &'ve T;
    type IntoIter = <&'ve Vec<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T: NoritoSerialize> NoritoSerialize for UniqueVec<T> {
    fn serialize<W: Write>(&self, writer: W) -> Result<(), ncore::Error> {
        self.0.serialize(writer)
    }
}

impl<'a, T> NoritoDeserialize<'a> for UniqueVec<T>
where
    T: NoritoSerialize + PartialEq + for<'de> NoritoDeserialize<'de>,
{
    fn deserialize(archived: &'a ncore::Archived<Self>) -> Self {
        let vec = Vec::<T>::deserialize(archived.cast::<Vec<T>>());
        UniqueVec::from_iter(vec)
    }

    fn try_deserialize(archived: &'a ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let vec = Vec::<T>::try_deserialize(archived.cast::<Vec<T>>())?;
        Ok(UniqueVec::from_iter(vec))
    }
}

impl<T: JsonSerialize> JsonSerialize for UniqueVec<T> {
    fn json_serialize(&self, out: &mut String) {
        out.push('[');
        let mut iter = self.0.iter();
        if let Some(first) = iter.next() {
            first.json_serialize(out);
            for value in iter {
                out.push(',');
                value.json_serialize(out);
            }
        }
        out.push(']');
    }
}

impl<T> JsonDeserialize for UniqueVec<T>
where
    T: JsonDeserialize + PartialEq,
{
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let values = parser.parse_array::<T>()?;
        let mut unique = Vec::with_capacity(values.len());
        for value in values {
            if unique.contains(&value) {
                return Err(json::Error::InvalidField {
                    field: "UniqueVec".into(),
                    message: "duplicate element".into(),
                });
            }
            unique.push(value);
        }
        Ok(UniqueVec(unique))
    }
}

#[cfg(test)]
mod tests {
    use norito::codec::{Decode, Encode};

    use super::*;

    // Tests covering the behaviour of `UniqueVec` to ensure it maintains
    // uniqueness of elements and behaves like a typical vector otherwise.

    // Verify that `UniqueVec` uses the `Default` trait to create an empty
    // collection.
    #[test]
    fn default_creates_empty_vec() {
        let unique_vec = UniqueVec::<u32>::default();
        assert!(unique_vec.is_empty());
    }

    // `new` should produce the same empty state as `default`.
    #[test]
    fn new_creates_empty_vec() {
        let unique_vec = UniqueVec::<u32>::new();
        assert!(unique_vec.is_empty());
    }

    // Pushing a value that isn't present should succeed.
    #[test]
    fn push_returns_true_if_value_is_unique() {
        let mut unique_vec = unique_vec![1, 3, 4];
        assert!(matches!(unique_vec.push(2), PushResult::Ok));
    }

    // Attempting to push a duplicate should return the offending value.
    #[test]
    fn push_returns_false_if_value_is_not_unique() {
        let mut unique_vec = unique_vec![1, 2, 3];
        assert!(matches!(unique_vec.push(1), PushResult::Duplicate(1)));
    }

    // Removing by index should yield the element that was stored at that position.
    #[test]
    fn remove_returns_value_at_index() {
        let mut unique_vec = unique_vec![1, 2, 3];
        assert_eq!(unique_vec.remove(1), 2);
    }

    // Removing outside the bounds of the collection should panic.
    #[test]
    #[should_panic(expected = "removal index (is 3) should be < len (is 3)")]
    fn remove_out_of_bounds_panics() {
        let mut unique_vec = unique_vec![1, 2, 3];
        unique_vec.remove(3);
    }

    // `clear` should drop all elements and leave the collection empty.
    #[test]
    fn clear_removes_all_values() {
        let mut unique_vec = unique_vec![1, 2, 3];
        unique_vec.clear();
        assert!(unique_vec.is_empty());
    }

    // Collecting from an iterator should discard duplicates.
    #[test]
    fn from_iter_creates_unique_vec() {
        let unique_vec = UniqueVec::from_iter([1, 1, 2, 3, 2]);
        assert_eq!(unique_vec, unique_vec![1, 2, 3]);
    }

    // Extending with a list of values should only add new unique items.
    #[test]
    fn extend_adds_unique_values() {
        let mut unique_vec = unique_vec![1, 2, 3];
        unique_vec.extend([1, 2, 3, 4, 5]);
        assert_eq!(unique_vec, unique_vec![1, 2, 3, 4, 5]);
    }

    // The serialization codec should produce data that can be decoded back into
    // an equivalent `UniqueVec`.
    #[test]
    fn encode_decode_round_trip() {
        let unique_vec = unique_vec![1u32, 2, 3];
        let encoded = unique_vec.encode();
        let decoded = UniqueVec::decode(&mut encoded.as_slice()).expect("decode");
        assert_eq!(unique_vec, decoded);
    }

    // Decoding should deduplicate serialized data that contains duplicates.
    #[test]
    fn decode_deduplicates_duplicates() {
        use norito::{Compression, serialize_into};

        let unique_vec = UniqueVec(vec![1u32, 2, 2]);
        let mut bytes = Vec::new();
        serialize_into(&mut bytes, &unique_vec, Compression::None).expect("serialize");

        let decoded: UniqueVec<u32> =
            norito::deserialize_from(bytes.as_slice()).expect("decode duplicates");
        assert_eq!(decoded, unique_vec![1u32, 2]);
    }

    // JSON serialization and deserialization should round-trip and detect
    // duplicates during deserialization.
    #[test]
    fn json_round_trip() {
        let unique_vec = unique_vec![1u32, 2, 3];
        let serialized = norito::json::to_json(&unique_vec).expect("serialize");
        let deserialized: UniqueVec<u32> =
            norito::json::from_str(&serialized).expect("deserialize");
        assert_eq!(unique_vec, deserialized);
    }

    #[test]
    fn json_fails_on_duplicates() {
        let json = "[1, 2, 1]";
        let result: Result<UniqueVec<u32>, _> = norito::json::from_str(json);
        assert!(result.is_err());
    }
}
