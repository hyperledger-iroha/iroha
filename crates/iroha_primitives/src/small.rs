//! Types that store small data inline to avoid heap allocations.
//!
//! This module provides wrappers around the `smallstr` and `smallvec` crates
//! with sensible defaults for Iroha. Short strings fit into a `[u8; 32]`
//! buffer, while [`SmallVec`] can be tuned to store a handful of elements on
//! the stack before spilling onto the heap.
use core::fmt;
use std::{format, io::Write, string::String, vec::Vec};

use iroha_schema::{IntoSchema, TypeId};
use norito::{
    NoritoDeserialize, NoritoSerialize, core as ncore,
    json::{self, FastJsonWrite, JsonDeserialize, JsonSerialize},
};
pub use small_string::SmallStr;
pub use small_vector::SmallVec;
use smallstr::SmallString;
pub use smallvec::{Array, smallvec};

/// The go-to size for `SmallVec`. When in doubt, use this.
pub const SMALL_SIZE: usize = 8_usize;

mod small_string {
    use super::*;

    #[derive(Debug, derive_more::Display, Clone, PartialEq, Eq, IntoSchema)]
    /// Wrapper around the [`smallstr::SmallString`] type, enforcing a
    /// specific size of stack-based strings.
    #[schema(transparent = "String")]
    #[repr(transparent)]
    pub struct SmallStr(SmallString<[u8; 32]>);

    impl SmallStr {
        #[must_use]
        #[inline]
        /// Construct [`Self`] by taking ownership of a [`String`].
        pub fn from_string(other: String) -> Self {
            Self(SmallString::from_string(other))
        }

        #[must_use]
        #[inline]
        #[allow(clippy::should_implement_trait)]
        /// Construct [`Self`] infallibly without taking ownership of a
        /// string slice. This is not an implementation of [`FromStr`](core::str::FromStr),
        /// because the latter implies **fallible** conversion, while this
        /// particular conversion is **infallible**.
        pub fn from_str(other: &str) -> Self {
            Self(SmallString::from_str(other))
        }

        #[inline]
        /// Checks if the specified pattern is the prefix of given string.
        pub fn starts_with(&self, pattern: &str) -> bool {
            self.0.starts_with(pattern)
        }
    }

    impl<A: Array<Item = u8>> From<SmallString<A>> for SmallStr {
        fn from(string: SmallString<A>) -> Self {
            Self(SmallString::from_str(SmallString::as_str(&string)))
        }
    }

    impl AsRef<str> for SmallStr {
        fn as_ref(&self) -> &str {
            self.0.as_str()
        }
    }

    impl SmallStr {
        #[inline]
        fn as_str(&self) -> &str {
            self.0.as_str()
        }
    }

    impl FastJsonWrite for SmallStr {
        fn write_json(&self, out: &mut String) {
            json::write_json_string(self.as_str(), out);
        }
    }

    impl JsonDeserialize for SmallStr {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let value = parser.parse_string()?;
            Ok(Self::from_str(&value))
        }
    }

    impl NoritoSerialize for SmallStr {
        fn serialize<W: Write>(&self, writer: W) -> Result<(), ncore::Error> {
            <&str as NoritoSerialize>::serialize(&self.as_str(), writer)
        }
    }

    impl<'a> NoritoDeserialize<'a> for SmallStr {
        fn deserialize(archived: &'a ncore::Archived<Self>) -> Self {
            let archived_str: &ncore::Archived<String> = archived.cast();
            let string = <String as NoritoDeserialize>::deserialize(archived_str);
            Self::from_str(&string)
        }
    }

    impl<'a> ncore::DecodeFromSlice<'a> for SmallStr {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
            let (value, used) = <&str as ncore::DecodeFromSlice>::decode_from_slice(bytes)?;
            Ok((Self::from_str(value), used))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use norito::{
        NoritoDeserialize, NoritoSerialize, decode_from_bytes, to_bytes,
        codec::{Decode, Encode},
        core as ncore, json,
    };

    use super::*;

    // Encoding and decoding a `SmallVec` should produce an identical vector.
    #[test]
    fn smallvec_encode_decode_round_trip() {
        let vec: SmallVec<[u32; 4]> = SmallVec(smallvec![1, 2, 3]);
        let bytes = vec.encode();
        let decoded = SmallVec::<[u32; 4]>::decode(&mut bytes.as_slice()).expect("decode");
        assert_eq!(vec, decoded);
    }

    #[test]
    fn smallvec_decode_heap_allocation() {
        let vec: SmallVec<[u32; 4]> = SmallVec(smallvec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
        let bytes = vec.encode();
        let decoded = SmallVec::<[u32; 4]>::decode(&mut bytes.as_slice()).expect("decode");
        assert_eq!(vec, decoded);
    }

    #[test]
    fn smallstr_json_roundtrip() {
        for sample in ["", "abc", "Δfire🔥"] {
            let small = SmallStr::from_str(sample);
            let json_repr = json::to_json(&small).expect("serialize SmallStr");
            let expected = json::to_json(&sample.to_string()).expect("serialize string");
            assert_eq!(json_repr, expected);
            let decoded: SmallStr = json::from_json(&json_repr).expect("deserialize SmallStr");
            assert_eq!(decoded, small);
        }
    }

    #[test]
    fn smallvec_json_roundtrip() {
        let vec: SmallVec<[u32; 4]> = SmallVec(smallvec![1, 2, 3, 4]);
        let json_repr = json::to_json(&vec).expect("serialize SmallVec");
        assert_eq!(json_repr, "[1,2,3,4]");
        let decoded: SmallVec<[u32; 4]> =
            json::from_json(&json_repr).expect("deserialize SmallVec");
        assert_eq!(decoded, vec);
    }

    #[test]
    fn smallstr_norito_roundtrip() {
        let value = SmallStr::from_str("tiny");
        let bytes = to_bytes(&value).expect("encode SmallStr");
        let decoded: SmallStr = decode_from_bytes(&bytes).expect("decode SmallStr");
        assert_eq!(decoded, value);
    }

    #[test]
    fn smallvec_norito_roundtrip() {
        let value: SmallVec<[u32; 4]> = SmallVec(smallvec![4, 3, 2, 1]);
        let bytes = to_bytes(&value).expect("encode SmallVec");
        let decoded: SmallVec<[u32; 4]> = decode_from_bytes(&bytes).expect("decode SmallVec");
        assert_eq!(decoded, value);
    }

    #[test]
    fn smallvec_zero_sized_round_trip() {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        struct Zst;

        impl NoritoSerialize for Zst {
            fn serialize<W: Write>(&self, _writer: W) -> Result<(), ncore::Error> {
                Ok(())
            }
        }

        impl<'a> NoritoDeserialize<'a> for Zst {
            fn deserialize(_: &'a ncore::Archived<Self>) -> Self {
                Self
            }
        }

        impl<'a> ncore::DecodeFromSlice<'a> for Zst {
            fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
                if !bytes.is_empty() {
                    return Err(ncore::Error::LengthMismatch);
                }
                Ok((Self, 0))
            }
        }

        let vec: SmallVec<[Zst; 4]> = SmallVec(smallvec![Zst, Zst, Zst]);
        let bytes = vec.encode();
        let decoded = SmallVec::<[Zst; 4]>::decode(&mut bytes.as_slice()).expect("decode");
        assert_eq!(decoded, vec);
    }
}

mod small_vector {
    use super::*;

    /// Wrapper struct around [`smallvec::SmallVec`] type. Keeps `N`
    /// elements on the stack if `self.len()` is less than `N`, if not,
    /// produces a heap-allocated vector.
    ///
    /// To instantiate a vector with `N` stack elements,
    /// ```ignore
    /// use iroha_data_model::small::SmallVec;
    ///
    /// let a: SmallVec<[u8; 24]> = SmallVec(smallvec::smallvec![32]);
    /// ```
    #[repr(transparent)]
    pub struct SmallVec<A: Array>(pub smallvec::SmallVec<A>);

    impl<A: Array> Default for SmallVec<A> {
        fn default() -> Self {
            Self(smallvec::SmallVec::new())
        }
    }

    impl<A: Array> Clone for SmallVec<A>
    where
        A::Item: Clone,
    {
        fn clone(&self) -> Self {
            Self(self.0.clone())
        }
    }

    impl<A: Array> fmt::Debug for SmallVec<A>
    where
        A::Item: fmt::Debug,
    {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.debug_tuple("SmallVec").field(&self.0).finish()
        }
    }

    impl<A: Array> FromIterator<A::Item> for SmallVec<A> {
        fn from_iter<T: IntoIterator<Item = A::Item>>(iter: T) -> Self {
            Self(iter.into_iter().collect())
        }
    }

    #[allow(clippy::unconditional_recursion)] // False-positive
    impl<A: Array> PartialEq for SmallVec<A>
    where
        A::Item: PartialEq,
    {
        fn eq(&self, other: &Self) -> bool {
            self.0.eq(&other.0)
        }
    }

    impl<A: Array> PartialOrd for SmallVec<A>
    where
        A::Item: PartialOrd,
    {
        fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
            self.0.partial_cmp(&other.0)
        }
    }

    impl<A: Array> Ord for SmallVec<A>
    where
        A::Item: Ord,
    {
        fn cmp(&self, other: &Self) -> core::cmp::Ordering {
            self.0.cmp(&other.0)
        }
    }

    impl<A: Array> core::ops::Deref for SmallVec<A> {
        type Target = <smallvec::SmallVec<A> as core::ops::Deref>::Target;

        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl<A: Array> core::ops::DerefMut for SmallVec<A> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl<A: Array> Eq for SmallVec<A> where A::Item: Eq {}

    impl<A: Array> SmallVec<A> {
        /// Construct new empty [`SmallVec`]
        #[inline]
        #[must_use]
        pub fn new() -> Self {
            Self(smallvec::SmallVec::new())
        }

        /// Append an item to the vector.
        #[inline]
        pub fn push(&mut self, value: A::Item) {
            self.0.push(value);
        }

        /// Remove all elements from the vector without altering capacity.
        #[inline]
        pub fn clear(&mut self) {
            self.0.clear();
        }

        /// Remove and return the element at position `index`, shifting all elements after it to the
        /// left.
        ///
        /// Panics if `index` is out of bounds.
        #[inline]
        pub fn remove(&mut self, index: usize) -> A::Item {
            self.0.remove(index)
        }

        /// Convert a [`SmallVec`] to a [`Vec`], without reallocating if the [`SmallVec`]
        /// has already spilled onto the heap.
        #[inline]
        #[must_use]
        pub fn into_vec(self) -> Vec<A::Item> {
            self.0.into_vec()
        }
    }

    impl<A: Array> From<Vec<A::Item>> for SmallVec<A> {
        fn from(vec: Vec<A::Item>) -> Self {
            Self(vec.into_iter().collect())
        }
    }

    impl<A: Array> IntoIterator for SmallVec<A> {
        type Item = <A as smallvec::Array>::Item;

        type IntoIter = <smallvec::SmallVec<A> as IntoIterator>::IntoIter;

        fn into_iter(self) -> Self::IntoIter {
            self.0.into_iter()
        }
    }

    impl<A: smallvec::Array + 'static> TypeId for SmallVec<A>
    where
        A::Item: TypeId,
    {
        #[inline]
        fn id() -> String {
            Vec::<A::Item>::id()
        }
    }
    impl<A: smallvec::Array + 'static> IntoSchema for SmallVec<A>
    where
        A::Item: IntoSchema,
    {
        #[inline]
        fn type_name() -> String {
            Vec::<A::Item>::type_name()
        }

        #[inline]
        fn update_schema_map(map: &mut iroha_schema::MetaMap) {
            if !map.contains_key::<Self>() {
                if !map.contains_key::<Vec<A::Item>>() {
                    Vec::<A::Item>::update_schema_map(map);
                }

                if let Some(schema) = map.get::<Vec<A::Item>>() {
                    map.insert::<Self>(schema.clone());
                }
            }
        }
    }

    impl<A: smallvec::Array> Extend<A::Item> for SmallVec<A> {
        fn extend<T: IntoIterator<Item = A::Item>>(&mut self, iter: T) {
            self.0.extend(iter);
        }
    }

    impl<A: smallvec::Array> FastJsonWrite for SmallVec<A>
    where
        A::Item: JsonSerialize,
    {
        fn write_json(&self, out: &mut String) {
            out.push('[');
            let mut iter = self.0.iter();
            if let Some(first) = iter.next() {
                first.json_serialize(out);
                for item in iter {
                    out.push(',');
                    item.json_serialize(out);
                }
            }
            out.push(']');
        }
    }

    impl<A: smallvec::Array> JsonDeserialize for SmallVec<A>
    where
        A::Item: JsonDeserialize,
    {
        fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
            let values = parser.parse_array::<A::Item>()?;
            let mut out = smallvec::SmallVec::<A>::with_capacity(values.len());
            out.extend(values);
            Ok(Self(out))
        }
    }

    impl<A: Array> NoritoSerialize for SmallVec<A>
    where
        A::Item: NoritoSerialize,
    {
        fn serialize<W: Write>(&self, mut writer: W) -> Result<(), ncore::Error> {
            use ncore::WriteBytesExt;
            writer.write_u64::<ncore::LittleEndian>(self.0.len() as u64)?;
            for item in &self.0 {
                let mut buf = Vec::new();
                item.serialize(&mut buf)?;
                writer.write_u64::<ncore::LittleEndian>(buf.len() as u64)?;
                writer.write_all(&buf)?;
            }
            Ok(())
        }
    }

    impl<'a, A: Array> NoritoDeserialize<'a> for SmallVec<A>
    where
        A::Item: NoritoDeserialize<'a> + for<'slice> ncore::DecodeFromSlice<'slice>,
    {
        fn deserialize(archived: &'a ncore::Archived<Self>) -> Self {
            Self::try_deserialize(archived).unwrap_or_else(|err| {
                panic!(
                    "SmallVec<{}> decode failed: {err:?}",
                    core::any::type_name::<A::Item>()
                )
            })
        }

        fn try_deserialize(archived: &'a ncore::Archived<Self>) -> Result<Self, ncore::Error> {
            if let Some((_, len)) = ncore::payload_ctx()
                && len == 0
            {
                return Ok(Self::new());
            }
            let ptr = core::ptr::from_ref(archived).cast::<u8>();
            let ctx_len = ncore::payload_ctx().map(|(_, len)| len);
            let bytes_full = ncore::payload_slice_from_ptr(ptr)?;
            let bytes = ctx_len
                .and_then(|len| bytes_full.get(..len))
                .unwrap_or(bytes_full);
            let (value, _used) = <Self as ncore::DecodeFromSlice>::decode_from_slice(bytes)?;
            Ok(value)
        }
    }

    impl<'a, A: Array> ncore::DecodeFromSlice<'a> for SmallVec<A>
    where
        A::Item: ncore::DecodeFromSlice<'a>,
    {
        fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), ncore::Error> {
            let (len, mut offset) = <u64 as ncore::DecodeFromSlice>::decode_from_slice(bytes)?;
            let len = usize::try_from(len).map_err(|_| ncore::Error::LengthMismatch)?;
            let mut out = smallvec::SmallVec::<A>::with_capacity(len);

            for _ in 0..len {
                let (elem_len, used_len) =
                    <u64 as ncore::DecodeFromSlice>::decode_from_slice(&bytes[offset..])?;
                offset = offset.checked_add(used_len).ok_or(ncore::Error::LengthMismatch)?;
                let elem_len = usize::try_from(elem_len).map_err(|_| ncore::Error::LengthMismatch)?;

                if elem_len == 0 {
                    if core::mem::size_of::<ncore::Archived<A::Item>>() != 0 {
                        return Err(ncore::Error::LengthMismatch);
                    }
                    let (value, used) =
                        <A::Item as ncore::DecodeFromSlice>::decode_from_slice(&[])?;
                    if used != 0 {
                        return Err(ncore::Error::LengthMismatch);
                    }
                    out.push(value);
                    continue;
                }

                let end = offset.checked_add(elem_len).ok_or(ncore::Error::LengthMismatch)?;
                let slice = bytes.get(offset..end).ok_or(ncore::Error::LengthMismatch)?;
                let (value, used) =
                    <A::Item as ncore::DecodeFromSlice>::decode_from_slice(slice)?;
                if used != elem_len {
                    return Err(ncore::Error::LengthMismatch);
                }
                out.push(value);
                offset = end;
            }

            Ok((Self(out), offset))
        }
    }
}
