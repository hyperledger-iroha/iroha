//! Wrapper around immutable, shared text in one canonical JSON representation.
//!
//! [`Json::new`] serializes a [`norito::json::JsonSerialize`] value through its
//! checked writer, parses it back into the bounded semantic value, and stores
//! only the canonical compact rendering. Text constructors do the same; Norito
//! decoding rejects alternate lexical spellings instead of normalizing a signed
//! wire payload.

use core::str::FromStr;
use std::{borrow::Cow, string::String, sync::Arc, vec::Vec};

use derive_more::Display;
use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
use norito::{
    Decode, Encode,
    json::{self, JsonDeserializeOwned, JsonSerialize, Parser, Value},
};

/// Maximum UTF-8 byte length of one [`Json`] document.
///
/// This fixed V1 protocol bound matches the ledger's default metadata-value
/// ceiling and applies before a value can enter the data model, independently
/// of any lower context-specific limit.
pub const MAX_JSON_BYTES: usize = 1_048_576;

/// Maximum structural nesting depth of one [`Json`] document.
pub const MAX_JSON_NESTING_DEPTH: usize = json::MAX_JSON_VALUE_NESTING_DEPTH;

/// A wrapper around immutable, reference-counted text that contains exactly one
/// canonical rendering of a valid JSON document.
///
/// Use [`Json::new`] to serialize a value and establish the canonical lexical
/// invariant.
#[derive(Debug, Display, Clone, PartialOrd, PartialEq, Ord, Eq)]
#[display("{_0}")]
pub struct Json(Arc<String>);

// Canonical Json is one self-delimiting string field. The borrowed serializer
// avoids copying the shared text, while the owned helper provides a strict
// slice decoder that reports the parsed prefix.
#[derive(Encode)]
struct JsonWireRef<'a>(Cow<'a, str>);

#[derive(Encode, Decode)]
#[norito(decode_from_slice)]
struct JsonWireOwned {
    value: String,
}

impl TypeId for Json {
    fn id() -> String {
        "Json".to_owned()
    }
}

impl IntoSchema for Json {
    fn type_name() -> String {
        "Json".to_owned()
    }

    fn update_schema_map(map: &mut MetaMap) {
        if !map.contains_key::<Self>() {
            map.insert::<Self>(Metadata::Tuple(UnnamedFieldsMeta {
                types: vec![core::any::TypeId::of::<String>()],
            }));
            String::update_schema_map(map);
        }
    }
}

impl norito::core::NoritoSerialize for Json {
    fn schema_hash() -> [u8; 16] {
        norito::core::type_name_schema_hash::<Self>()
    }

    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let wire = JsonWireRef(Cow::Borrowed(self.0.as_str()));
        norito::core::NoritoSerialize::serialize(&wire, writer)
    }

    fn encoded_len_hint(&self) -> Option<usize> {
        let wire = JsonWireRef(Cow::Borrowed(self.0.as_str()));
        norito::core::NoritoSerialize::encoded_len_hint(&wire)
    }

    fn encoded_len_exact(&self) -> Option<usize> {
        let wire = JsonWireRef(Cow::Borrowed(self.0.as_str()));
        norito::core::NoritoSerialize::encoded_len_exact(&wire)
    }
}

impl<'a> norito::core::NoritoDeserialize<'a> for Json {
    fn schema_hash() -> [u8; 16] {
        norito::core::type_name_schema_hash::<Self>()
    }

    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).unwrap_or_else(|error| {
            panic!("norito: fallible deserialize failed for Json: {error:?}")
        })
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let ptr = core::ptr::from_ref(archived).cast::<u8>();
        if let Ok(payload) = norito::core::payload_slice_from_ptr(ptr) {
            let (value, _) = Self::decode_wire_text(payload)?;
            return Self::try_from_canonical_string(value);
        }

        let wire = <JsonWireOwned as norito::core::NoritoDeserialize>::try_deserialize(
            archived.cast::<JsonWireOwned>(),
        )?;
        let canonical = Self::require_canonical_text(&wire.value).map_err(|error| {
            norito::core::Error::Message(format!("invalid Json payload: {error}"))
        })?;
        drop(wire);
        Self::try_from_canonical_string(canonical)
    }
}

impl Json {
    fn ensure_size(value: &str) -> Result<(), norito::Error> {
        if value.len() > MAX_JSON_BYTES {
            return Err(norito::Error::from(format!(
                "Json payload exceeds the {MAX_JSON_BYTES}-byte UTF-8 limit"
            )));
        }
        Ok(())
    }

    fn try_from_canonical_string(value: String) -> Result<Self, norito::core::Error> {
        norito::core::reserve_decode_arc_allocation::<String>()?;
        Ok(Self(Arc::new(value)))
    }

    fn decode_wire_text(bytes: &[u8]) -> Result<(String, usize), norito::core::Error> {
        let (field_len, field_header_len) = norito::core::inspect_len_from_slice(bytes)?;
        let field_end = field_header_len
            .checked_add(field_len)
            .ok_or(norito::core::Error::LengthMismatch)?;
        let field = bytes
            .get(field_header_len..field_end)
            .ok_or(norito::core::Error::LengthMismatch)?;
        let (len, header_len) = norito::core::inspect_len_from_slice(field)?;
        if len > MAX_JSON_BYTES {
            return Err(norito::core::Error::Message(format!(
                "Json payload exceeds the {MAX_JSON_BYTES}-byte UTF-8 limit"
            )));
        }
        let end = header_len
            .checked_add(len)
            .ok_or(norito::core::Error::LengthMismatch)?;
        if end != field.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        let raw = field
            .get(header_len..end)
            .ok_or(norito::core::Error::LengthMismatch)?;
        let value = core::str::from_utf8(raw).map_err(|_| norito::core::Error::InvalidUtf8)?;
        let canonical = Self::require_canonical_text(value).map_err(|error| {
            norito::core::Error::Message(format!("invalid Json payload: {error}"))
        })?;
        norito::core::note_payload_access(bytes, field_end);
        Ok((canonical, field_end))
    }

    fn serialize_canonical_value(value: &json::Value) -> Result<String, norito::Error> {
        // `Value`'s checked Norito writer orders object keys through its
        // `BTreeMap` and is the codec's single authority for JSON string and
        // finite-f64 spelling. Reusing it here prevents the ledger wrapper from
        // drifting from the JSON emitted by every other Norito component.
        json::to_json_bounded(value, MAX_JSON_BYTES)
            .map_err(|error| norito::Error::Message(error.to_string()))
    }

    fn canonicalize_text(value: &str) -> Result<String, norito::Error> {
        Self::ensure_size(value)?;
        let parsed =
            json::parse_value(value).map_err(|error| norito::Error::from(error.to_string()))?;
        let canonical = Self::serialize_canonical_value(&parsed);
        json::drop_json_value_iteratively(parsed);
        canonical
    }

    fn require_canonical_text(value: &str) -> Result<String, norito::Error> {
        let canonical = Self::canonicalize_text(value)?;
        if canonical == value {
            Ok(canonical)
        } else {
            Err(norito::Error::from(
                "Json payload is valid but not in canonical lexical form",
            ))
        }
    }

    /// Serializes `payload` into a JSON string.
    ///
    /// # Errors
    /// Serialization can fail if `payload` has no checked writer or cannot be
    /// converted into JSON, for example if it contains non-string map keys.
    ///
    /// # Panics
    /// Panics if serialization fails or if the serializer produces a document
    /// that violates the [`Json`] size or structural-depth invariant.
    #[allow(clippy::needless_pass_by_value)]
    pub fn new<T: JsonSerialize>(payload: T) -> Self {
        Self::try_new(payload).expect("serialization of Json always succeeds")
    }

    /// Deserializes the JSON string into any type that implements
    /// [`norito::json::JsonDeserializeOwned`].
    ///
    /// # Errors
    /// Returns an error if the string does not represent `T`.
    pub fn try_into_any<T: JsonDeserializeOwned>(&self) -> Result<T, norito::Error> {
        norito::json::from_str::<T>(self.0.as_str()).map_err(|e| norito::Error::from(e.to_string()))
    }

    /// Deserializes the JSON string into any type using Norito's JSON helper,
    /// returning `norito::Error` for convenience.
    ///
    /// # Errors
    /// Returns an error if the string does not represent `T`.
    pub fn try_into_any_norito<T: JsonDeserializeOwned>(&self) -> Result<T, norito::Error> {
        norito::json::from_str::<T>(self.0.as_str()).map_err(|e| norito::Error::from(e.to_string()))
    }

    /// Fallible constructor: serialize `payload` to JSON using Norito's helper.
    ///
    /// # Errors
    /// Returns an error if `payload` has no checked writer, cannot be converted
    /// into one valid JSON document, is too deeply nested, or exceeds
    /// [`MAX_JSON_BYTES`].
    #[allow(clippy::needless_pass_by_value)]
    pub fn try_new<T: JsonSerialize>(payload: T) -> Result<Self, norito::Error> {
        let serialized = norito::json::to_json_bounded(&payload, MAX_JSON_BYTES)
            .map_err(|error| norito::Error::Message(error.to_string()))?;
        let canonical = Self::canonicalize_text(&serialized).map_err(|error| {
            norito::Error::from(format!(
                "Json serializer produced an invalid JSON document: {error}"
            ))
        })?;
        drop(serialized);
        Self::try_from_canonical_string(canonical)
    }

    /// Fallible constructor from `&str` using Norito's JSON helper.
    ///
    /// Like `FromStr`, this helper is strict and rejects non-JSON text. Valid
    /// input is normalized into the single compact, key-ordered representation.
    ///
    /// # Errors
    /// Returns an error if the input string is not valid, is too deeply
    /// nested, or exceeds [`MAX_JSON_BYTES`].
    pub fn from_str_norito(s: &str) -> Result<Self, norito::Error> {
        let canonical = Self::canonicalize_text(s)?;
        Self::try_from_canonical_string(canonical)
    }

    /// Creates a [`Json`] value from an already serialized JSON document.
    ///
    /// The supplied text is parsed as one bounded semantic value and stored in
    /// canonical compact form. Insignificant whitespace, alternate escapes,
    /// number spellings, and object insertion order are never retained.
    ///
    /// # Errors
    /// Returns an error if `value` is not exactly one valid JSON document, is
    /// too deeply nested, or exceeds [`MAX_JSON_BYTES`].
    pub fn from_raw_json(value: String) -> Result<Self, norito::Error> {
        let canonical = Self::canonicalize_text(&value)?;
        drop(value);
        Self::try_from_canonical_string(canonical)
    }

    /// Returns a reference to the inner JSON string.
    pub fn get(&self) -> &String {
        self.0.as_ref()
    }

    /// Returns `true` when `self` and `other` share the same immutable backing
    /// allocation.
    ///
    /// This is an allocation-identity check, not a value comparison: two
    /// independently constructed [`Json`] values can compare equal while this
    /// method returns `false`.
    #[must_use]
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Convert a Norito JSON value into this JSON wrapper by normalizing it
    /// into a compact string representation.
    ///
    /// # Errors
    /// Returns an error if serialization of the value fails, if the result is
    /// too deeply nested, or if it exceeds [`MAX_JSON_BYTES`].
    pub fn from_norito_value_ref(v: &Value) -> Result<Self, norito::Error> {
        let canonical = Self::serialize_canonical_value(v)?;
        Self::try_from_canonical_string(canonical)
    }
}

impl json::JsonSerialize for Json {
    fn json_serialize(&self, out: &mut String) {
        out.push_str(self.0.as_str());
    }

    fn json_serialize_to(
        &self,
        out: &mut dyn json::JsonWriteSink,
    ) -> Result<(), json::BoundedJsonError> {
        json::write_validated_json_to(self.0.as_str(), out)
    }
}

impl json::JsonDeserialize for Json {
    fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, json::Error> {
        let slice = p.raw_value_slice()?;
        let canonical = Json::canonicalize_text(slice)
            .map_err(|error| json::Error::Message(error.to_string()))?;
        Json::try_from_canonical_string(canonical).map_err(json::Error::from_decode_resource)
    }

    fn json_from_value(value: &Value) -> Result<Self, json::Error> {
        let canonical = Json::serialize_canonical_value(value)
            .map_err(|error| json::Error::Message(error.to_string()))?;
        Json::try_from_canonical_string(canonical).map_err(json::Error::from_decode_resource)
    }

    fn json_from_map_key(key: &str) -> Result<Self, json::Error> {
        let canonical = json::to_json_bounded(key, MAX_JSON_BYTES)
            .map_err(|error| json::Error::Message(error.to_string()))?;
        Json::try_from_canonical_string(canonical).map_err(json::Error::from_decode_resource)
    }
}

impl From<&Value> for Json {
    fn from(value: &Value) -> Self {
        Json::from_norito_value_ref(value).expect("json to_string")
    }
}

impl From<Value> for Json {
    fn from(value: Value) -> Self {
        let result = Json::from_norito_value_ref(&value);
        json::drop_json_value_iteratively(value);
        result.expect("json to_string")
    }
}

impl From<u32> for Json {
    fn from(value: u32) -> Self {
        Json::new(value)
    }
}

impl From<u64> for Json {
    fn from(value: u64) -> Self {
        Json::new(value)
    }
}

impl From<f64> for Json {
    fn from(value: f64) -> Self {
        Json::new(value)
    }
}

impl From<bool> for Json {
    fn from(value: bool) -> Self {
        Json::new(value)
    }
}

impl From<&str> for Json {
    fn from(value: &str) -> Self {
        Json::new(value)
    }
}

impl<T: Into<Json> + JsonSerialize> From<Vec<T>> for Json {
    fn from(value: Vec<T>) -> Self {
        Json::new(value)
    }
}

impl FromStr for Json {
    type Err = norito::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_str_norito(s)
    }
}

impl Default for Json {
    fn default() -> Self {
        Self(Arc::new("null".to_owned()))
    }
}

// Provide slice-based decoding for Json so it can live inside packed sequences
// and option fields under Norito's strict-safe path.
impl<'a> norito::core::DecodeFromSlice<'a> for Json {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (value, consumed) = Self::decode_wire_text(bytes)?;
        Ok((Self::try_from_canonical_string(value)?, consumed))
    }
}

impl AsRef<str> for Json {
    fn as_ref(&self) -> &str {
        let s = self.0.as_str();
        // If the underlying JSON is a string literal, `self.0` holds it quoted.
        // Return a view without the outer quotes to make simple string comparisons ergonomic.
        // Note: this does not unescape inner sequences; for structured use, prefer `try_into_any`.
        if s.len() >= 2 && s.as_bytes().first() == Some(&b'"') && s.as_bytes().last() == Some(&b'"')
        {
            // Safety: slicing preserves UTF-8 boundaries because JSON strings are valid UTF-8 and quotes are single-byte ASCII
            &s[1..s.len() - 1]
        } else {
            s
        }
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use super::*;
    use norito::codec::{Decode as _, Encode as _};

    /// Wire oracle for the canonical single-string representation.
    #[derive(Encode, Decode)]
    struct CanonicalJsonWire(String);

    #[derive(
        Debug,
        PartialEq,
        Eq,
        crate::json_macros::JsonSerialize,
        crate::json_macros::JsonDeserialize,
        Clone,
    )]
    struct SerdeStruct {
        a: u32,
        b: String,
    }

    #[test]
    fn clones_share_immutable_backing() {
        let original =
            Json::from_raw_json(format!("\"{}\"", "shared-json-payload".repeat(32 * 1024)))
                .expect("valid shared JSON fixture");
        let cloned = original.clone();
        let equal_but_distinct =
            Json::from_raw_json(original.get().clone()).expect("valid shared JSON fixture");

        assert!(original.ptr_eq(&cloned));
        assert!(!original.ptr_eq(&equal_but_distinct));
        assert_eq!(original, cloned);
        assert_eq!(original, equal_but_distinct);

        // Preserve the existing public accessor signature as well as its value.
        let _: &String = cloned.get();
    }

    #[test]
    fn shared_backing_has_the_canonical_string_schema() {
        let schema = Json::schema();

        assert_eq!(<Json as TypeId>::id(), "Json");
        assert_eq!(<Json as IntoSchema>::type_name(), "Json");
        assert_eq!(
            schema.get::<Json>(),
            Some(&Metadata::Tuple(UnnamedFieldsMeta {
                types: vec![core::any::TypeId::of::<String>()],
            }))
        );
        assert_eq!(schema.get::<String>(), Some(&Metadata::String));
        assert_eq!(schema.iter().count(), 2);
    }

    #[test]
    fn canonical_json_roundtrips_the_single_string_norito_wire() {
        let inputs = [
            "null".to_owned(),
            "{\"a\":1}".to_owned(),
            format!("\"{}\"", "x".repeat(300)),
        ];

        for input in inputs {
            let json = Json::from_raw_json(input.clone()).expect("valid JSON wire fixture");
            let encoded = json.encode();

            assert_eq!(encoded, CanonicalJsonWire(input.clone()).encode());
            assert_eq!(
                encoded,
                JsonWireOwned {
                    value: input.clone()
                }
                .encode()
            );

            let mut bytes = encoded.as_slice();
            let decoded = Json::decode(&mut bytes).expect("decode shared Json");
            assert_eq!(decoded.get(), &input);
            assert!(bytes.is_empty(), "decoder must consume the entire payload");
        }

        assert_eq!(
            Json::from_raw_json("{\"a\":1}".to_owned())
                .expect("valid JSON wire fixture")
                .encode(),
            [0x08, 0x07, b'{', b'"', b'a', b'"', b':', b'1', b'}']
        );
    }

    #[test]
    fn slice_decode_does_not_consume_trailing_fields() {
        const TRAILING_FIELD: &[u8] = b"\xA5\x5Asecond-field";

        let input = "{\"field\":true}".to_owned();
        let encoded = Json::from_raw_json(input.clone())
            .expect("valid JSON wire fixture")
            .encode();
        let mut packed_fields = encoded.clone();
        packed_fields.extend_from_slice(TRAILING_FIELD);

        let (decoded, consumed) =
            <Json as norito::core::DecodeFromSlice>::decode_from_slice(&packed_fields)
                .expect("decode Json prefix");

        assert_eq!(decoded.get(), &input);
        assert_eq!(consumed, encoded.len());
        assert_eq!(&packed_fields[consumed..], TRAILING_FIELD);
    }

    #[test]
    fn slice_decode_rejects_truncated_and_invalid_utf8_wire() {
        let encoded = Json::from_raw_json("{\"field\":true}".to_owned())
            .expect("valid JSON wire fixture")
            .encode();

        for end in 0..encoded.len() {
            assert!(
                <Json as norito::core::DecodeFromSlice>::decode_from_slice(&encoded[..end])
                    .is_err(),
                "truncated payload of {end} bytes must fail"
            );
        }

        let mut invalid_utf8 = encoded;
        *invalid_utf8.last_mut().expect("non-empty encoded Json") = 0xFF;
        assert!(
            <Json as norito::core::DecodeFromSlice>::decode_from_slice(&invalid_utf8).is_err(),
            "invalid UTF-8 must not enter Json backing storage"
        );
    }

    #[test]
    fn shared_backing_roundtrips_framed_norito() {
        let original = Json::from_raw_json("{\"framed\":[1,true,null]}".to_owned())
            .expect("valid framed JSON fixture");

        let encoded = norito::to_bytes(&original).expect("encode framed Json");
        let decoded: Json = norito::decode_from_bytes(&encoded).expect("decode framed Json");

        assert_eq!(decoded, original);
    }

    #[test]
    fn shared_clone_preserves_canonical_json_output() {
        let raw = "{ \"order\": [3, 2, 1], \"escaped\": \"a\\nb\" }";
        let json = Json::from_raw_json(raw.to_owned()).expect("valid raw JSON fixture");
        let cloned = json.clone();
        let mut serialized = String::new();

        cloned.json_serialize(&mut serialized);

        let canonical = r#"{"escaped":"a\nb","order":[3,2,1]}"#;
        assert_eq!(serialized, canonical);
        assert_eq!(cloned.to_string(), canonical);
        assert_eq!(cloned.get(), canonical);
    }

    #[test]
    fn text_boundaries_canonicalize_every_accepted_lexical_variant() {
        for (raw, canonical) in [
            ("1 ", "1"),
            (r#"{"z":0,"a":1}"#, r#"{"a":1,"z":0}"#),
            (r#""\u0061""#, r#""a""#),
            ("1e0", "1.0"),
            ("-0", "-0.0"),
        ] {
            let constructed =
                Json::from_raw_json(raw.to_owned()).expect("valid JSON must canonicalize");
            let parsed: Json = raw.parse().expect("FromStr must canonicalize valid JSON");
            let decoded: Json =
                norito::json::from_json(raw).expect("JSON decoding must canonicalize valid JSON");

            assert_eq!(constructed.get(), canonical, "raw constructor: {raw}");
            assert_eq!(parsed.get(), canonical, "FromStr: {raw}");
            assert_eq!(decoded.get(), canonical, "JSON decoder: {raw}");
        }
    }

    #[test]
    fn norito_decoders_reject_noncanonical_json_spellings() {
        for raw in ["1 ", r#"{"z":0,"a":1}"#, r#""\u0061""#, "1e0", "-0"] {
            let encoded = CanonicalJsonWire(raw.to_owned()).encode();
            let error = <Json as norito::core::DecodeFromSlice>::decode_from_slice(&encoded)
                .expect_err("binary Json must have one lexical representation");
            assert!(
                error.to_string().contains("canonical lexical form"),
                "unexpected slice error for {raw}: {error}"
            );

            let framed = norito::to_bytes(&Json(Arc::new(raw.to_owned())))
                .expect("encode hostile noncanonical Json fixture");
            let error = norito::decode_from_bytes::<Json>(&framed)
                .expect_err("framed binary Json must reject alternate spelling");
            assert!(
                error.to_string().contains("canonical lexical form"),
                "unexpected framed error for {raw}: {error}"
            );
        }
    }

    #[test]
    fn as_ref_dequotes_string_values() {
        let j = Json::from("value");
        assert_eq!(j.as_ref(), "value");

        // Non-string JSON stays as-is
        let num = Json::from(42u32);
        assert_eq!(num.as_ref(), "42");
        let boolean = Json::from(true);
        assert_eq!(boolean.as_ref(), "true");
        let array = Json::new(vec![1u32, 2u32]);
        assert_eq!(array.as_ref(), "[1,2]");
    }

    #[test]
    fn try_new_and_try_into_any_roundtrip() {
        let v = SerdeStruct {
            a: 7,
            b: "x".to_string(),
        };
        let j = Json::try_new(v.clone()).expect("try_new");
        let back: SerdeStruct = j.try_into_any().expect("try_into_any");
        assert_eq!(v, back);
    }

    #[test]
    fn norito_value_roundtrip() {
        let value = norito::json!({"a": 1u64, "b": [true, false], "s": "x"});
        let j = Json::from_norito_value_ref(&value).expect("to json");
        let parsed: norito::json::Value = norito::json::from_str(j.get()).expect("parse back");
        assert_eq!(parsed, value);
    }

    #[test]
    fn semantic_value_and_map_key_conversions_use_canonical_bounded_output() {
        let value = norito::json!({"z": 2u64, "a": [true, null]});
        let converted = <Json as json::JsonDeserialize>::json_from_value(&value)
            .expect("convert semantic JSON value");
        assert_eq!(converted.get(), r#"{"a":[true,null],"z":2}"#);

        let key = <Json as json::JsonDeserialize>::json_from_map_key("quote\"line\n")
            .expect("convert JSON map key");
        assert_eq!(key.get(), r#""quote\"line\n""#);
    }

    #[test]
    fn json_wrapper_has_closed_output_bound_without_reparsing() {
        let value = Json::from_norito_value_ref(&norito::json!({
            "text": "quoted\nvalue",
            "items": [1u64, 2, 3]
        }))
        .expect("construct canonical JSON wrapper");
        let expected = value.get();
        assert_eq!(
            norito::json::to_json_bounded(&value, expected.len())
                .expect("serialize Json at exact bound"),
            expected
        );
        assert_eq!(
            norito::json::to_json_bounded(&value, expected.len() - 1),
            Err(norito::json::BoundedJsonError::BodyTooLarge)
        );
    }

    #[test]
    fn try_into_any_norito_value_roundtrip() {
        let value = norito::json!({"n": 1u64, "flag": true});
        let j = Json::from_norito_value_ref(&value).expect("to json");
        let back: norito::json::Value = j.try_into_any_norito().expect("norito decode");
        assert_eq!(back, value);
    }

    #[test]
    fn from_str_norito_rejects_plain_text_and_accepts_json() {
        let err = Json::from_str_norito("hello").expect_err("plain text must fail");
        assert!(
            err.to_string().contains("JSON error"),
            "unexpected parse error: {err}"
        );
        // Proper JSON is canonicalized.
        let j2 = Json::from_str_norito("{\"k\":1}").expect("json object");
        let v: norito::json::Value = norito::json::from_str(j2.get()).expect("parse value");
        assert_eq!(v, norito::json!({"k": 1}));
    }

    #[test]
    fn from_str_is_strict_and_from_string_value_wraps_plain_text() {
        let structured: Json = r#" { "items": [1, true, null] } "#
            .parse()
            .expect("parse structured JSON");
        assert_eq!(structured.get(), r#"{"items":[1,true,null]}"#);

        assert!(
            "plain text".parse::<Json>().is_err(),
            "FromStr must reject non-JSON text"
        );
        let plain_text = Json::from("plain text");
        assert_eq!(plain_text.get(), r#""plain text""#);
        let json_looking_text = Json::from(r#"{"not":"raw"}"#);
        assert_eq!(json_looking_text.get(), r#""{\"not\":\"raw\"}""#);
    }

    #[test]
    fn canonical_value_serializer_roundtrips_values() {
        let values = [
            norito::json::Value::String("00000000-0000-0000-0000-000000000000".to_owned()),
            norito::json::Value::String("addr:127.0.0.1:33337#D694".to_owned()),
            norito::json::Value::Array(vec![
                norito::json::Value::from(1u64),
                norito::json::Value::from(2u64),
            ]),
            norito::json::Value::Object({
                let mut map = norito::json::native::Map::new();
                map.insert("mode".into(), norito::json::Value::from("Permissioned"));
                map.insert(
                    "wire_protocol_version".into(),
                    norito::json::Value::from(2u64),
                );
                map
            }),
            norito::json::Value::String("quote\"slash\\newline\nctrl\u{001f}".to_owned()),
            norito::json::Value::Array(vec![
                norito::json::Value::Number(norito::json::native::Number::F64(1.5)),
                norito::json::Value::Bool(true),
                norito::json::Value::Null,
            ]),
        ];

        for value in values {
            let serialized =
                Json::serialize_canonical_value(&value).expect("serialize bounded JSON value");
            let reparsed = norito::json::parse_value(&serialized).expect("parse plain");
            assert_eq!(reparsed, value, "mismatch for {serialized}");
        }
    }

    #[test]
    fn norito_value_conversion_normalizes_non_finite_numbers_without_panicking() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let json = Json::from(norito::json::Value::Number(
                norito::json::native::Number::F64(value),
            ));
            assert_eq!(json.get(), "null");
        }
    }

    #[test]
    fn canonical_value_serializer_covers_the_full_kotodama_boundary_depth() {
        let levels = norito::core::MAX_OWNED_VALUE_DECODE_DEPTH - 1;
        let mut nested = norito::json::Value::from(7_u64);
        for _ in 0..levels {
            nested = norito::json::Value::Array(vec![nested]);
        }
        let mut object = norito::json::native::Map::new();
        object.insert("value".to_owned(), nested);
        let boundary = norito::json::Value::Object(object);

        let encoded = Json::from_norito_value_ref(&boundary)
            .expect("the full V1 type depth plus its parameter object must serialize");
        let decoded: norito::json::Value = encoded
            .try_into_any_norito()
            .expect("the full V1 boundary must parse back");
        let mut cursor = decoded
            .as_object()
            .and_then(|map| map.get("value"))
            .expect("boundary object contains value");
        for _ in 0..levels {
            let items = cursor.as_array().expect("nested boundary list");
            assert_eq!(items.len(), 1);
            cursor = &items[0];
        }
        assert_eq!(cursor.as_u64(), Some(7));
        json::drop_json_value_iteratively(decoded);
        json::drop_json_value_iteratively(boundary);

        let mut too_deep = norito::json::Value::Null;
        for _ in 0..=norito::json::MAX_JSON_VALUE_NESTING_DEPTH {
            too_deep = norito::json::Value::Array(vec![too_deep]);
        }
        assert!(
            Json::from_norito_value_ref(&too_deep).is_err(),
            "the explicit JSON structural bound must still fail closed"
        );
        json::drop_json_value_iteratively(too_deep);
    }

    #[test]
    fn json_validation_and_normalization_fit_a_128k_stack() {
        let worker = std::thread::Builder::new()
            .name("iroha-json-iterative-boundary".into())
            .stack_size(128 * 1024)
            .spawn(|| -> Result<(), String> {
                let wrappers = norito::core::MAX_OWNED_VALUE_DECODE_DEPTH - 1;
                let at_255 = format!("{}null{}", "[".repeat(wrappers), "]".repeat(wrappers));
                let normalized =
                    Json::from_str_norito(&at_255).map_err(|error| error.to_string())?;
                if normalized.get() != &at_255 {
                    return Err("deep Json normalization changed canonical text".to_owned());
                }
                let direct =
                    norito::json::from_json::<Json>(&at_255).map_err(|error| error.to_string())?;
                if direct.get() != &at_255 {
                    return Err("deep direct Json decode changed the input slice".to_owned());
                }
                let value =
                    norito::json::parse_value(&at_255).map_err(|error| error.to_string())?;
                let converted = Json::from(value);
                if converted.get() != &at_255 {
                    return Err("owned Value conversion changed canonical text".to_owned());
                }
                let encoded = JsonWireOwned {
                    value: at_255.clone(),
                }
                .encode();
                let (decoded, consumed) =
                    <Json as norito::core::DecodeFromSlice>::decode_from_slice(&encoded)
                        .map_err(|error| error.to_string())?;
                if consumed != encoded.len() || decoded.get() != &at_255 {
                    return Err("deep Json slice decode changed the wire payload".to_owned());
                }
                let framed = norito::to_bytes(
                    &Json::from_raw_json(at_255.clone()).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                let decoded = norito::decode_from_bytes::<Json>(&framed)
                    .map_err(|error| error.to_string())?;
                if decoded.get() != &at_255 {
                    return Err("deep framed Json decode changed the wire payload".to_owned());
                }

                let invalid_256th_wrapper = format!("[{at_255},]");
                if Json::from_str_norito(&invalid_256th_wrapper).is_ok() {
                    return Err("strict Json constructor accepted a trailing comma".to_owned());
                }
                let invalid_wire = JsonWireOwned {
                    value: invalid_256th_wrapper.clone(),
                }
                .encode();
                if <Json as norito::core::DecodeFromSlice>::decode_from_slice(&invalid_wire).is_ok()
                {
                    return Err("Json wire decoder accepted a trailing comma".to_owned());
                }

                if invalid_256th_wrapper.parse::<Json>().is_ok() {
                    return Err("FromStr accepted an invalid JSON document".to_owned());
                }
                Ok(())
            })
            .expect("spawn 128KiB Json boundary test");
        worker
            .join()
            .expect("128KiB Json boundary thread")
            .expect("iterative Json boundary");
    }

    struct BadJson;

    impl JsonSerialize for BadJson {
        fn json_serialize(&self, out: &mut String) {
            out.push_str("bad json trailing");
        }
    }

    #[test]
    fn try_new_rejects_manual_serializer_without_checked_writer() {
        assert_eq!(
            norito::json::to_json(&BadJson).expect("legacy serializer remains available"),
            "bad json trailing"
        );
        let error = Json::try_new(BadJson).expect_err("unchecked serializer must fail closed");
        assert_eq!(
            error.to_string(),
            "bounded JSON serialization is unsupported"
        );
    }

    struct CheckedBadJson;

    impl JsonSerialize for CheckedBadJson {
        fn json_serialize(&self, out: &mut String) {
            out.push_str("bad json trailing");
        }

        fn json_serialize_to(
            &self,
            out: &mut dyn json::JsonWriteSink,
        ) -> Result<(), json::BoundedJsonError> {
            out.push_str("bad json trailing")
        }
    }

    #[test]
    fn try_new_rejects_unparseable_checked_payload() {
        let error =
            Json::try_new(CheckedBadJson).expect_err("invalid JSON serializer must fail closed");
        assert!(
            error.to_string().contains("invalid JSON document"),
            "unexpected serialization error: {error}"
        );
    }

    #[test]
    fn shared_json_backing_charges_the_arc_allocation() {
        let allocation = norito::core::owned_arc_allocation_bytes::<String>()
            .expect("Arc<String> allocation layout");
        let limits = norito::core::DecodeLimits::new(
            usize::MAX,
            usize::MAX,
            usize::MAX,
            allocation - 1,
            usize::MAX,
        );
        let canonical = "null".to_owned();

        let error =
            norito::core::with_decode_limits(limits, || Json::try_from_canonical_string(canonical))
                .expect_err("Arc allocation must be charged before construction");
        assert!(matches!(
            error,
            norito::core::Error::TotalAllocationExceeded { attempted, limit }
                if attempted == allocation as u64 && limit == (allocation - 1) as u64
        ));
    }

    #[test]
    fn json_boundaries_reject_duplicate_keys_and_invalid_norito_payloads() {
        let duplicate = r#"{"owner":"alice","owner":"bob"}"#;
        assert!(
            norito::json::from_json::<Json>(duplicate).is_err(),
            "direct JSON decoding must reject duplicate object keys"
        );

        for invalid in [duplicate, "not-json"] {
            let encoded = JsonWireOwned {
                value: invalid.to_owned(),
            }
            .encode();
            assert!(
                <Json as norito::core::DecodeFromSlice>::decode_from_slice(&encoded).is_err(),
                "slice decoding accepted invalid Json payload: {invalid}"
            );

            let framed = norito::to_bytes(&Json(Arc::new(invalid.to_owned())))
                .expect("encode hostile Json wire");
            assert!(
                norito::decode_from_bytes::<Json>(&framed).is_err(),
                "framed decoding accepted invalid Json payload: {invalid}"
            );
        }
    }

    #[test]
    fn json_size_limit_covers_constructors_and_wire_decoders() {
        let boundary = format!("\"{}\"", "x".repeat(MAX_JSON_BYTES - 2));
        let over_limit = format!("\"{}\"", "x".repeat(MAX_JSON_BYTES - 1));
        assert_eq!(boundary.len(), MAX_JSON_BYTES);
        assert_eq!(over_limit.len(), MAX_JSON_BYTES + 1);

        let raw =
            Json::from_raw_json(boundary.clone()).expect("raw constructor accepts byte boundary");
        assert_eq!(raw.get().len(), MAX_JSON_BYTES);
        assert!(
            Json::from_raw_json(over_limit.clone()).is_err(),
            "raw constructor accepted an oversized document"
        );

        let parsed = boundary
            .parse::<Json>()
            .expect("FromStr accepts byte boundary");
        assert_eq!(parsed.get().len(), MAX_JSON_BYTES);
        assert!(
            over_limit.parse::<Json>().is_err(),
            "FromStr accepted an oversized document"
        );

        let direct = norito::json::from_str::<Json>(&boundary)
            .expect("JSON deserializer accepts byte boundary");
        assert_eq!(direct.get().len(), MAX_JSON_BYTES);
        assert!(
            norito::json::from_str::<Json>(&over_limit).is_err(),
            "JSON deserializer accepted an oversized document"
        );

        let serialized = Json::try_new("x".repeat(MAX_JSON_BYTES - 2))
            .expect("serializer accepts byte boundary");
        assert_eq!(serialized.get().len(), MAX_JSON_BYTES);
        assert!(
            Json::try_new("x".repeat(MAX_JSON_BYTES - 1)).is_err(),
            "serializer accepted an oversized document"
        );

        let boundary_wire = JsonWireOwned {
            value: boundary.clone(),
        }
        .encode();
        let (decoded, consumed) =
            <Json as norito::core::DecodeFromSlice>::decode_from_slice(&boundary_wire)
                .expect("slice decoder accepts byte boundary");
        assert_eq!(decoded.get().len(), MAX_JSON_BYTES);
        assert_eq!(consumed, boundary_wire.len());

        let oversized_wire = JsonWireOwned {
            value: over_limit.clone(),
        }
        .encode();
        assert!(
            <Json as norito::core::DecodeFromSlice>::decode_from_slice(&oversized_wire).is_err(),
            "slice decoder accepted an oversized document"
        );

        let mut oversized_inner_header = Vec::new();
        norito::core::write_len_to_vec(
            &mut oversized_inner_header,
            u64::try_from(MAX_JSON_BYTES + 1).expect("JSON limit fits u64"),
        );
        let mut declared_oversize = Vec::new();
        norito::core::write_len_to_vec(
            &mut declared_oversize,
            u64::try_from(oversized_inner_header.len()).expect("length header fits u64"),
        );
        declared_oversize.extend_from_slice(&oversized_inner_header);
        let error = <Json as norito::core::DecodeFromSlice>::decode_from_slice(&declared_oversize)
            .expect_err("oversized declared Json must fail before its missing body is read");
        assert!(
            error.to_string().contains("1048576-byte"),
            "decoder reached a generic truncation error before the Json limit: {error}"
        );

        let oversized_frame = norito::to_bytes(&Json(Arc::new(over_limit)))
            .expect("encode oversized hostile Json fixture");
        assert!(
            norito::decode_from_bytes::<Json>(&oversized_frame).is_err(),
            "framed decoder accepted an oversized document"
        );
    }

    #[test]
    fn raw_constructor_enforces_the_structural_depth_limit() {
        let boundary = format!(
            "{}null{}",
            "[".repeat(MAX_JSON_NESTING_DEPTH - 1),
            "]".repeat(MAX_JSON_NESTING_DEPTH - 1)
        );
        let over_limit = format!("[{boundary}]");

        Json::from_raw_json(boundary).expect("raw constructor accepts depth boundary");
        assert!(
            Json::from_raw_json(over_limit).is_err(),
            "raw constructor accepted a document beyond the V1 depth limit"
        );
    }

    #[test]
    fn typed_quantity_conversion_accepts_only_canonical_nominal_json() {
        let canonical = Json::from_str_norito(r#""12.5""#).expect("canonical quantity JSON");
        assert_eq!(
            canonical
                .try_into_any_norito::<crate::numeric::Quantity>()
                .expect("typed quantity conversion")
                .to_string(),
            "12.5"
        );

        for invalid in [r#""12.50""#, r#""-1""#, "12.5"] {
            let json = Json::from_str_norito(invalid).expect("valid JSON document");
            assert!(
                json.try_into_any_norito::<crate::numeric::Quantity>()
                    .is_err(),
                "typed quantity conversion accepted {invalid}"
            );
        }
    }

    #[test]
    fn try_new_prefers_valid_path_when_available() {
        let good = Json::try_new(norito::json!({"a":1,"b":[true,false]}))
            .expect("valid payload must succeed");
        let parsed: norito::json::Value =
            norito::json::parse_value(good.get()).expect("must parse");
        assert_eq!(
            parsed,
            norito::json!({"a":1,"b":[true,false]}),
            "unexpected payload"
        );
    }
}
