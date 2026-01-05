use std::{
    borrow::ToOwned as _, format, hash, marker::PhantomData, num::NonZeroU8, str::FromStr,
    string::String,
};

#[cfg(not(feature = "ffi_import"))]
use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
use derive_more::{Debug, Deref, DerefMut, Display};
use iroha_schema::{IntoSchema, TypeId};
#[cfg(feature = "json")]
use mv::json::JsonKeyCodec;
#[cfg(feature = "json")]
use norito::json::{self, FastJsonWrite, JsonDeserialize};
#[cfg(feature = "json")]
use norito::literal;

use crate::{ParseError, hex_decode};

/// Hash of Iroha entities. Currently supports only blake2b-32.
/// The least significant bit of hash is set to 1.
#[derive(Debug, Display, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, TypeId)]
#[display("{}", hex::encode(self.as_ref()))]
#[debug("{}", hex::encode(self.as_ref()))]
// NOTE: Invariants are maintained in `FromStr`
#[repr(C)]
pub struct Hash {
    more_significant_bits: [u8; Self::LENGTH - 1],
    least_significant_byte: NonZeroU8,
}

impl Hash {
    /// Length of hash
    pub const LENGTH: usize = 32;

    /// Wrap the given bytes; they must be prehashed with `Blake2bVar`
    pub fn prehashed(mut hash: [u8; Self::LENGTH]) -> Self {
        hash[Self::LENGTH - 1] |= 1;
        // SAFETY:
        // - any `u8` value after bitwise or with 1 will be at least 1
        // - `Hash` and `[u8; Hash::LENGTH]` have the same memory layout
        #[allow(unsafe_code)]
        unsafe {
            core::mem::transmute(hash)
        }
    }

    /// Check if least significant bit of `[u8; Hash::LENGTH]` is 1
    fn is_lsb_1(hash: &[u8; Self::LENGTH]) -> bool {
        hash[Self::LENGTH - 1] & 1 == 1
    }
}

impl Hash {
    /// Hash the given bytes.
    #[must_use]
    pub fn new(bytes: impl AsRef<[u8]>) -> Self {
        let vec_hash = Blake2bVar::new(Self::LENGTH)
            .expect("Failed to initialize variable size hash")
            .chain(bytes)
            .finalize_boxed();

        let mut hash = [0; Self::LENGTH];
        hash.copy_from_slice(&vec_hash);

        Hash::prehashed(hash)
    }
}

#[cfg(feature = "json")]
fn ensure_uppercase_hex(candidate: &str, literal: &str) -> Result<(), json::Error> {
    if candidate.chars().any(|c| c.is_ascii_lowercase()) {
        return Err(json::Error::Message(format!(
            "hash literal `{literal}` must use uppercase hex digits"
        )));
    }
    Ok(())
}

#[cfg(feature = "json")]
fn parse_hash_literal(value: &str) -> Result<Hash, json::Error> {
    let body = literal::parse("hash", value)?;
    ensure_uppercase_hex(body, value)?;
    Hash::from_str(body).map_err(|err| json::Error::Message(err.to_string()))
}

impl From<Hash> for [u8; Hash::LENGTH] {
    #[inline]
    fn from(hash: Hash) -> Self {
        #[allow(unsafe_code)]
        // SAFETY: `Hash` and `[u8; Hash::LENGTH]` have the same memory layout
        unsafe {
            core::mem::transmute(hash)
        }
    }
}

impl AsRef<[u8; Hash::LENGTH]> for Hash {
    #[inline]
    fn as_ref(&self) -> &[u8; Hash::LENGTH] {
        #[allow(unsafe_code, trivial_casts)]
        // SAFETY: `Hash` and `[u8; Hash::LENGTH]` have the same memory layout
        unsafe {
            &*(core::ptr::from_ref(self).cast::<[u8; Self::LENGTH]>())
        }
    }
}

#[cfg(feature = "json")]
impl FastJsonWrite for Hash {
    fn write_json(&self, out: &mut String) {
        let body = hex::encode_upper(self.as_ref());
        let literal = literal::format("hash", &body);
        json::write_json_string(&literal, out);
    }
}

#[cfg(feature = "json")]
impl JsonDeserialize for Hash {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        let value = parser.parse_string()?;
        parse_hash_literal(&value)
    }
}

#[cfg(feature = "json")]
impl JsonKeyCodec for Hash {
    fn encode_json_key(&self, out: &mut String) {
        FastJsonWrite::write_json(self, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        parse_hash_literal(encoded)
    }
}

impl norito::core::NoritoSerialize for Hash {
    fn serialize<W: std::io::Write>(&self, mut writer: W) -> Result<(), norito::core::Error> {
        writer.write_all(self.as_ref())?;
        Ok(())
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for Hash {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("Hash decode")
    }

    fn try_deserialize(
        archived: &'de norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        #[allow(unsafe_code)]
        let bytes = unsafe { &*core::ptr::from_ref(archived).cast::<[u8; Hash::LENGTH]>() };
        if !Self::is_lsb_1(bytes) {
            return Err(norito::core::Error::Message("invalid hash lsb".into()));
        }
        Ok(Hash::prehashed(*bytes))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for Hash {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        if bytes.len() < Self::LENGTH {
            return Err(norito::core::Error::LengthMismatch);
        }
        let mut b = [0u8; Self::LENGTH];
        b.copy_from_slice(&bytes[..Self::LENGTH]);
        if !Self::is_lsb_1(&b) {
            return Err(norito::core::Error::Message("invalid hash lsb".into()));
        }
        Ok((Hash::prehashed(b), Self::LENGTH))
    }
}

impl FromStr for Hash {
    type Err = ParseError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        let hash: [u8; Self::LENGTH] = hex_decode(key)?.try_into().map_err(|hash_vec| {
            ParseError(format!(
                "Unable to parse {hash_vec:?} as [u8; {}]",
                Self::LENGTH
            ))
        })?;

        Hash::is_lsb_1(&hash)
            .then_some(hash)
            .ok_or_else(|| ParseError("expect least significant bit of hash to be 1".to_owned()))
            .map(Self::prehashed)
    }
}

impl IntoSchema for Hash {
    fn type_name() -> String {
        "Hash".to_owned()
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if !map.contains_key::<Self>() {
            <[u8; Self::LENGTH]>::update_schema_map(map);

            map.insert::<Self>(iroha_schema::Metadata::Tuple(
                iroha_schema::UnnamedFieldsMeta {
                    types: vec![core::any::TypeId::of::<[u8; Self::LENGTH]>()],
                },
            ));
        }
    }
}

impl<T> From<HashOf<T>> for Hash {
    fn from(HashOf(hash, _): HashOf<T>) -> Self {
        hash
    }
}

crate::ffi::ffi_item! {
    /// Represents hash of Iroha entities like `Block` or `Transaction`. Currently supports only blake2b-32.
    #[derive(Debug, Display, Deref, DerefMut, TypeId)]
    #[debug("{{ {} {_0} }}", core::any::type_name::<Self>())]
    #[display("{_0}")]
    #[repr(transparent)]
    pub struct HashOf<T>(
        #[deref]
        #[deref_mut]
        Hash,
        PhantomData<T>,
    );

    // SAFETY: `HashOf` has no trap representation in `Hash`
    ffi_type(unsafe {robust})
}

impl<T> Clone for HashOf<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for HashOf<T> {}

#[allow(clippy::unconditional_recursion)] // False-positive
impl<T> PartialEq for HashOf<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}
impl<T> Eq for HashOf<T> {}

impl<T> PartialOrd for HashOf<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl<T> Ord for HashOf<T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> hash::Hash for HashOf<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T> AsRef<[u8; Hash::LENGTH]> for HashOf<T> {
    fn as_ref(&self) -> &[u8; Hash::LENGTH] {
        self.0.as_ref()
    }
}

/// Archived representation of [`HashOf`].
pub type ArchivedHashOf<T> = norito::core::Archived<HashOf<T>>;

impl<T> norito::core::NoritoSerialize for HashOf<T> {
    fn serialize<W: std::io::Write>(&self, mut writer: W) -> Result<(), norito::core::Error> {
        writer.write_all(self.0.as_ref())?;
        Ok(())
    }
}

impl<'de, T> norito::core::NoritoDeserialize<'de> for HashOf<T> {
    fn deserialize(archived: &'de norito::core::Archived<Self>) -> Self {
        #[allow(unsafe_code)]
        let bytes = unsafe { &*core::ptr::from_ref(archived).cast::<[u8; Hash::LENGTH]>() };
        Self(Hash::prehashed(*bytes), PhantomData)
    }
}

impl<T> HashOf<T> {
    /// Transmutes hash to some specific type.
    /// Don't use this method if not required.
    #[inline]
    #[must_use]
    pub(crate) const fn transmute<F>(self) -> HashOf<F> {
        HashOf(self.0, PhantomData)
    }

    /// Adds type information to the hash. Be careful about using this function
    /// since it is not possible to validate the correctness of the conversion.
    /// Prefer creating new hashes with [`HashOf::new`] whenever possible
    #[must_use]
    pub const fn from_untyped_unchecked(hash: Hash) -> Self {
        HashOf(hash, PhantomData)
    }
}

impl<T: norito::codec::Encode> HashOf<T> {
    /// Construct typed hash
    #[must_use]
    pub fn new(value: &T) -> Self {
        let bytes = norito::codec::Encode::encode(value);
        Self(Hash::new(bytes), PhantomData)
    }
}

impl<T> FromStr for HashOf<T> {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<Hash>().map(Self::from_untyped_unchecked)
    }
}

#[cfg(feature = "json")]
impl<T> FastJsonWrite for HashOf<T> {
    fn write_json(&self, out: &mut String) {
        self.0.write_json(out);
    }
}

#[cfg(feature = "json")]
impl<T> JsonDeserialize for HashOf<T> {
    fn json_deserialize(parser: &mut json::Parser<'_>) -> Result<Self, json::Error> {
        Hash::json_deserialize(parser).map(|hash| HashOf(hash, PhantomData))
    }
}

#[cfg(feature = "json")]
impl<T> JsonKeyCodec for HashOf<T> {
    fn encode_json_key(&self, out: &mut String) {
        FastJsonWrite::write_json(&self.0, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        parse_hash_literal(encoded).map(|hash| HashOf(hash, PhantomData))
    }
}

impl<T: IntoSchema> IntoSchema for HashOf<T> {
    fn type_name() -> String {
        format!("HashOf<{}>", T::type_name())
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        if !map.contains_key::<Self>() {
            Hash::update_schema_map(map);

            map.insert::<Self>(iroha_schema::Metadata::Tuple(
                iroha_schema::UnnamedFieldsMeta {
                    types: vec![core::any::TypeId::of::<Hash>()],
                },
            ));
        }
    }
}

// Provide slice-based decoding for HashOf<T> so it can be used inside
// packed sequences and option fields with Norito's strict-safe path.
impl<'a, T> norito::core::DecodeFromSlice<'a> for HashOf<T> {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let mut cursor = std::io::Cursor::new(bytes);
        let v: Self = <Self as norito::codec::Decode>::decode(&mut cursor)?;
        Ok((v, bytes.len()))
    }
}

#[cfg(any(feature = "ffi_export", feature = "ffi_import"))]
mod ffi {
    //! Manual implementations of FFI related functionality

    use super::*;

    // NOTE: Hash is FFI serialized as an array (a pointer in a function call, by value when part of a struct)
    iroha_ffi::ffi_type! {
        unsafe impl Transparent for Hash {
            type Target = [u8; Hash::LENGTH];

            validation_fn=unsafe {Hash::is_lsb_1},
            niche_value = [0; Hash::LENGTH]
        }
    }

    impl iroha_ffi::WrapperTypeOf<Hash> for [u8; Hash::LENGTH] {
        type Type = Hash;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blake2_32b() {
        let mut hasher = Blake2bVar::new(32).unwrap();
        hasher.update(&hex_literal::hex!("6920616d2064617461"));
        assert_eq!(
            hasher.finalize_boxed().as_ref(),
            &hex_literal::hex!("BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B2")
                [..]
        );
    }

    #[test]
    fn hash_of_roundtrip() {
        use norito::codec::{Decode, Encode};

        let original = HashOf::<()>::from_untyped_unchecked(Hash::prehashed([1; Hash::LENGTH]));
        let bytes = original.encode();
        let decoded = HashOf::<()>::decode(&mut &bytes[..]).expect("failed to decode HashOf");
        assert_eq!(original, decoded);
    }

    #[test]
    fn from_str_rejects_even_lsb() {
        // Hex string with the final byte ending in `0`, so its least significant bit is not set.
        let invalid_hex = "BA67336EFD6A3DF3A70EEB757860763036785C182FF4CF587541A0068D09F5B0";

        assert!(Hash::from_str(invalid_hex).is_err());
    }

    #[test]
    fn hash_try_deserialize_rejects_invalid_lsb() {
        let bytes = [0u8; Hash::LENGTH];
        let framed = norito::core::frame_bare_with_header_flags::<Hash>(&bytes, 0).expect("frame");
        let archived = norito::from_bytes::<Hash>(&framed).expect("archive");
        let err = <Hash as norito::core::NoritoDeserialize>::try_deserialize(archived)
            .expect_err("invalid lsb");
        assert!(matches!(err, norito::core::Error::Message(_)));
    }
}

#[cfg(all(test, feature = "json"))]
mod json_tests {
    use norito::{json::FastJsonWrite, literal};

    use super::*;

    #[test]
    fn hash_json_roundtrip() {
        let hash = Hash::new(b"hash-json-roundtrip");
        let json = norito::json::to_json(&hash).expect("serialize hash");
        let body = hex::encode_upper(hash.as_ref());
        let expected = literal::format("hash", &body);
        assert_eq!(json, format!("\"{expected}\""));

        let decoded: Hash = norito::json::from_str(&json).expect("deserialize hash");
        assert_eq!(decoded, hash);
    }

    #[test]
    fn hash_json_rejects_invalid_length() {
        let err =
            norito::json::from_str::<Hash>("\"deadbeef\"").expect_err("hash must be 32 bytes");
        match err {
            norito::json::Error::Message(message) => {
                assert!(
                    message.contains("uppercase hex")
                        || message.contains("must start with `hash:`"),
                    "unexpected message: {message}"
                );
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn hash_of_json_roundtrip() {
        let hash = Hash::new(b"hash-of-json");
        let hash_of = HashOf::<()>::from_untyped_unchecked(hash);
        let mut json = String::new();
        hash_of.write_json(&mut json);
        let decoded: HashOf<()> = norito::json::from_json(&json).expect("deserialize hash_of");
        assert_eq!(decoded, hash_of);
    }

    #[test]
    fn hash_literal_rejects_bad_checksum() {
        let body = "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF";
        // Compute a canonical literal then corrupt the checksum.
        let mut literal = literal::format("hash", body);
        literal.truncate(literal.len() - 4);
        literal.push_str("0000");
        let json_literal = format!("\"{literal}\"");
        let err = norito::json::from_str::<Hash>(&json_literal).expect_err("checksum mismatch");
        match err {
            norito::json::Error::InvalidField { .. } | norito::json::Error::Message(_) => {}
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn hash_literal_rejects_lowercase_hex() {
        let body = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let literal = literal::format("hash", body);
        let json_literal = format!("\"{literal}\"");
        let err = norito::json::from_str::<Hash>(&json_literal).expect_err("lowercase rejected");
        match err {
            norito::json::Error::InvalidField { .. } | norito::json::Error::Message(_) => {}
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn hash_raw_literal_is_rejected() {
        let raw = "\"0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF\"";
        let err = norito::json::from_str::<Hash>(raw).expect_err("raw hash literal must fail");
        match err {
            norito::json::Error::InvalidField { .. } | norito::json::Error::Message(_) => {}
            other => panic!("unexpected error: {other}"),
        }
    }
}

#[cfg(test)]
mod prehashed_tests {
    use super::*;

    #[test]
    fn prehashed_sets_lsb() {
        let mut bytes = [0xff; Hash::LENGTH];
        bytes[Hash::LENGTH - 1] &= !1;
        let hash = Hash::prehashed(bytes);
        assert_eq!(hash.as_ref()[Hash::LENGTH - 1] & 1, 1);
    }
}
