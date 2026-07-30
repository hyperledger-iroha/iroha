//! Wrapper around immutable, shared text guaranteed to contain valid JSON.
//!
//! [`Json::new`] serializes any [`norito::json::JsonSerialize`] value using the
//! Norito JSON serializer and ensures that parsing it back succeeds. This keeps
//! the canonical, minified representation that other components expect.

use core::{fmt::Write as _, str::FromStr};
use std::{borrow::Cow, string::String, sync::Arc, vec::Vec};

use derive_more::Display;
use iroha_schema::{IntoSchema, MetaMap, Metadata, TypeId, UnnamedFieldsMeta};
use norito::{
    Decode, Encode,
    json::{self, JsonDeserializeOwned, JsonSerialize, Parser, Value},
};

const HEX_DIGITS: &[u8; 16] = b"0123456789ABCDEF";

/// A wrapper around immutable, reference-counted text that is guaranteed to
/// contain a valid JSON document.
///
/// Use [`Json::new`] to serialize a value into a JSON string and ensure the
/// result is well-formed.
#[derive(Debug, Display, Clone, PartialOrd, PartialEq, Ord, Eq)]
#[display("{_0}")]
pub struct Json(Arc<String>);

// These helpers deliberately retain the derived wire layout of the former
// `Json(String)` representation. `Cow<str>` and `String` are both
// self-delimiting Norito fields, so the borrowed serializer writes the same
// bytes without first copying the shared JSON text. Norito does not encode
// field names, so the named owned helper has the same single-field wire layout
// while also getting a strict slice decoder that reports the parsed prefix.
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
        #[cfg(feature = "schema-structural")]
        {
            norito::core::schema_hash_structural::<Self>()
        }
        #[cfg(not(feature = "schema-structural"))]
        {
            norito::core::type_name_schema_hash::<Self>()
        }
    }

    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
        #[cfg(feature = "schema-structural")]
        {
            norito::core::schema_hash_structural::<Self>()
        }
        #[cfg(not(feature = "schema-structural"))]
        {
            norito::core::type_name_schema_hash::<Self>()
        }
    }

    fn deserialize(archived: &'a norito::core::Archived<Self>) -> Self {
        Self::try_deserialize(archived).unwrap_or_else(|error| {
            panic!("norito: fallible deserialize failed for Json: {error:?}")
        })
    }

    fn try_deserialize(
        archived: &'a norito::core::Archived<Self>,
    ) -> Result<Self, norito::core::Error> {
        let wire = <JsonWireOwned as norito::core::NoritoDeserialize>::try_deserialize(
            archived.cast::<JsonWireOwned>(),
        )?;
        json::validate_json(&wire.value).map_err(|error| {
            norito::core::Error::Message(format!("invalid Json payload: {error}"))
        })?;
        Ok(Self(Arc::new(wire.value)))
    }
}

impl Json {
    fn escape_json_string_plain(s: &str, out: &mut String) {
        out.push('"');
        for ch in s.chars() {
            match ch {
                '"' => out.push_str("\\\""),
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                c if (c as u32) < 0x20 => {
                    out.push_str("\\u00");
                    out.push(HEX_DIGITS[((c as u32 >> 4) & 0xF) as usize] as char);
                    out.push(HEX_DIGITS[(c as u32 & 0xF) as usize] as char);
                }
                _ => out.push(ch),
            }
        }
        out.push('"');
    }

    fn serialize_json_value_plain(value: &json::Value, out: &mut String) {
        use norito::json::native::Number;

        enum Task<'a> {
            Value(&'a json::Value),
            Escaped(&'a str),
            Byte(char),
        }

        let mut tasks = vec![Task::Value(value)];
        while let Some(task) = tasks.pop() {
            match task {
                Task::Escaped(value) => Self::escape_json_string_plain(value, out),
                Task::Byte(value) => out.push(value),
                Task::Value(value) => match value {
                    json::Value::Null => out.push_str("null"),
                    json::Value::Bool(value) => {
                        out.push_str(if *value { "true" } else { "false" });
                    }
                    json::Value::Number(value) => match value {
                        Number::I64(value) => out.push_str(&value.to_string()),
                        Number::U64(value) => out.push_str(&value.to_string()),
                        Number::F64(value) => {
                            const F64_SAFE_INT: f64 = 9_007_199_254_740_992.0; // 2^53
                            if value.is_finite()
                                && value.fract() == 0.0
                                && value.abs() <= F64_SAFE_INT
                            {
                                let _ = write!(out, "{value:.1}");
                            } else {
                                let _ = write!(out, "{value:?}");
                            }
                        }
                    },
                    json::Value::String(value) => Self::escape_json_string_plain(value, out),
                    json::Value::Array(items) => {
                        out.push('[');
                        tasks.push(Task::Byte(']'));
                        for (index, item) in items.iter().enumerate().rev() {
                            if index + 1 < items.len() {
                                tasks.push(Task::Byte(','));
                            }
                            tasks.push(Task::Value(item));
                        }
                    }
                    json::Value::Object(map) => {
                        out.push('{');
                        tasks.push(Task::Byte('}'));
                        for (index, (key, value)) in map.iter().enumerate().rev() {
                            if index + 1 < map.len() {
                                tasks.push(Task::Byte(','));
                            }
                            tasks.push(Task::Value(value));
                            tasks.push(Task::Byte(':'));
                            tasks.push(Task::Escaped(key));
                        }
                    }
                },
            }
        }
    }

    fn serialize_json_value_plain_str(value: &json::Value) -> String {
        let mut out = String::new();
        Self::serialize_json_value_plain(value, &mut out);
        out
    }

    fn drop_json_value_iteratively(value: Value) {
        let mut pending = vec![value];
        while let Some(value) = pending.pop() {
            match value {
                Value::Array(mut values) => pending.append(&mut values),
                Value::Object(values) => pending.extend(values.into_values()),
                Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
            }
        }
    }

    /// Serializes `payload` into a JSON string.
    ///
    /// # Errors
    /// Serialization can fail if `payload` cannot be converted into JSON,
    /// for example if it contains non-string map keys.
    ///
    /// # Panics
    /// Panics if serialization fails.
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
    /// Returns an error if `payload` cannot be converted into JSON.
    #[allow(clippy::needless_pass_by_value)]
    pub fn try_new<T: JsonSerialize>(payload: T) -> Result<Self, norito::Error> {
        // Primary path: canonical Norito serializer.
        let serialized =
            norito::json::to_json(&payload).map_err(|e| norito::Error::from(e.to_string()))?;
        if norito::json::validate_json(&serialized).is_ok() {
            return Ok(Self::from_string_unchecked(serialized));
        }

        // Fallback: materialize a `Value` and serialize it plainly.
        if let Ok(value) = norito::json::to_value(&payload) {
            let plain = Self::serialize_json_value_plain_str(&value);
            let valid = norito::json::validate_json(&plain).is_ok();
            Self::drop_json_value_iteratively(value);
            if valid {
                return Ok(Self::from_string_unchecked(plain));
            }
        }

        Err(norito::Error::from(
            "Json serializer produced an invalid JSON document",
        ))
    }

    /// Fallible constructor from `&str` using Norito's JSON helper.
    /// Unlike `FromStr`, this helper is strict and rejects non-JSON text.
    ///
    /// # Errors
    /// Returns an error if the input string is not valid JSON.
    pub fn from_str_norito(s: &str) -> Result<Self, norito::Error> {
        let value = json::parse_value(s).map_err(|e| norito::Error::from(e.to_string()))?;
        let result = Self::from_norito_value_ref(&value);
        Self::drop_json_value_iteratively(value);
        result
    }

    /// Creates a `Json` value without validating that the input is well-formed.
    ///
    /// The caller must guarantee that `value` contains valid JSON.
    pub fn from_string_unchecked(value: String) -> Self {
        Self(Arc::new(value))
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
    /// Returns an error if serialization of the value to a string fails.
    pub fn from_norito_value_ref(v: &Value) -> Result<Self, norito::Error> {
        let plain = Self::serialize_json_value_plain_str(v);
        norito::json::validate_json(&plain).map_err(|e| norito::Error::from(e.to_string()))?;
        Ok(Self::from_string_unchecked(plain))
    }
}

impl json::JsonSerialize for Json {
    fn json_serialize(&self, out: &mut String) {
        out.push_str(self.0.as_str());
    }
}

impl json::JsonDeserialize for Json {
    fn json_deserialize(p: &mut Parser<'_>) -> Result<Self, json::Error> {
        p.skip_ws();
        let start = p.position();
        p.skip_value()?;
        let end = p.position();
        let slice = &p.input()[start..end];
        Ok(Json::from_string_unchecked(slice.to_owned()))
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
        Json::drop_json_value_iteratively(value);
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
        value
            .parse::<Json>()
            .unwrap_or_else(|_| panic!("invalid JSON literal: {value}"))
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
        json::parse_value(s).map_or_else(
            |_| Json::from_norito_value_ref(&Value::String(s.to_owned())),
            |value| {
                let result = Json::from_norito_value_ref(&value);
                Json::drop_json_value_iteratively(value);
                result
            },
        )
    }
}

impl Default for Json {
    fn default() -> Self {
        // NOTE: empty string isn't valid JSON
        Self::from_string_unchecked("null".to_owned())
    }
}

// Provide slice-based decoding for Json so it can live inside packed sequences
// and option fields under Norito's strict-safe path.
impl<'a> norito::core::DecodeFromSlice<'a> for Json {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let (wire, consumed) =
            <JsonWireOwned as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        json::validate_json(&wire.value).map_err(|error| {
            norito::core::Error::Message(format!("invalid Json payload: {error}"))
        })?;
        Ok((Self(Arc::new(wire.value)), consumed))
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

    /// Mirrors the derived representation used before `Json` adopted shared
    /// backing. Keeping this local wire oracle makes representation drift
    /// visible without coupling the production type back to owned strings.
    #[derive(Encode, Decode)]
    struct LegacyJsonWire(String);

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
            Json::from_string_unchecked(format!("\"{}\"", "shared-json-payload".repeat(64 * 1024)));
        let cloned = original.clone();
        let equal_but_distinct = Json::from_string_unchecked(original.get().clone());

        assert!(original.ptr_eq(&cloned));
        assert!(!original.ptr_eq(&equal_but_distinct));
        assert_eq!(original, cloned);
        assert_eq!(original, equal_but_distinct);

        // Preserve the existing public accessor signature as well as its value.
        let _: &String = cloned.get();
    }

    #[test]
    fn shared_backing_preserves_legacy_schema() {
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
    fn shared_backing_preserves_legacy_norito_wire() {
        let inputs = [
            "null".to_owned(),
            "{\"a\":1}".to_owned(),
            format!("\"{}\"", "x".repeat(300)),
        ];

        for input in inputs {
            let json = Json::from_string_unchecked(input.clone());
            let encoded = json.encode();

            assert_eq!(encoded, LegacyJsonWire(input.clone()).encode());
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
            Json::from_string_unchecked("{\"a\":1}".to_owned()).encode(),
            [0x08, 0x07, b'{', b'"', b'a', b'"', b':', b'1', b'}']
        );
    }

    #[test]
    fn slice_decode_does_not_consume_trailing_fields() {
        const TRAILING_FIELD: &[u8] = b"\xA5\x5Asecond-field";

        let input = "{\"field\":true}".to_owned();
        let encoded = Json::from_string_unchecked(input.clone()).encode();
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
        let encoded = Json::from_string_unchecked("{\"field\":true}".to_owned()).encode();

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
        let original = Json::from_string_unchecked("{\"framed\":[1,true,null]}".to_owned());

        let encoded = norito::to_bytes(&original).expect("encode framed Json");
        let decoded: Json = norito::decode_from_bytes(&encoded).expect("decode framed Json");

        assert_eq!(decoded, original);
    }

    #[test]
    fn shared_clone_preserves_exact_json_output() {
        let raw = "{ \"order\": [3, 2, 1], \"escaped\": \"a\\nb\" }";
        let json = Json::from_string_unchecked(raw.to_owned());
        let cloned = json.clone();
        let mut serialized = String::new();

        cloned.json_serialize(&mut serialized);

        assert_eq!(serialized, raw);
        assert_eq!(cloned.to_string(), raw);
        assert_eq!(cloned.get(), raw);
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
        // Proper JSON is preserved.
        let j2 = Json::from_str_norito("{\"k\":1}").expect("json object");
        let v: norito::json::Value = norito::json::from_str(j2.get()).expect("parse value");
        assert_eq!(v, norito::json!({"k": 1}));
    }

    #[test]
    fn from_str_normalizes_structured_json_and_wraps_plain_text() {
        let structured: Json = r#" { "items": [1, true, null] } "#
            .parse()
            .expect("parse structured JSON");
        assert_eq!(structured.get(), r#"{"items":[1,true,null]}"#);

        let plain_text: Json = "plain text".parse().expect("wrap plain text");
        assert_eq!(plain_text.get(), r#""plain text""#);
    }

    #[test]
    fn plain_serializer_roundtrips_values() {
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
            let serialized = Json::serialize_json_value_plain_str(&value);
            let reparsed = norito::json::parse_value(&serialized).expect("parse plain");
            assert_eq!(reparsed, value, "mismatch for {serialized}");
        }
    }

    #[test]
    fn plain_serializer_covers_the_full_kotodama_boundary_depth() {
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
        Json::drop_json_value_iteratively(decoded);
        Json::drop_json_value_iteratively(boundary);

        let mut too_deep = norito::json::Value::Null;
        for _ in 0..=norito::json::MAX_JSON_VALUE_NESTING_DEPTH {
            too_deep = norito::json::Value::Array(vec![too_deep]);
        }
        assert!(
            Json::from_norito_value_ref(&too_deep).is_err(),
            "the explicit JSON structural bound must still fail closed"
        );
        Json::drop_json_value_iteratively(too_deep);
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
                let retry = Json::try_new(RetryJson {
                    calls: std::cell::Cell::new(0),
                    valid: at_255.clone(),
                })
                .map_err(|error| error.to_string())?;
                if retry.get() != &at_255 {
                    return Err("try_new fallback changed canonical text".to_owned());
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
                let framed = norito::to_bytes(&Json::from_string_unchecked(at_255.clone()))
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

                let fallback = invalid_256th_wrapper
                    .parse::<Json>()
                    .map_err(|error| error.to_string())?;
                if fallback.as_ref() != invalid_256th_wrapper {
                    return Err("FromStr plain-text fallback behavior changed".to_owned());
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

    struct RetryJson {
        calls: std::cell::Cell<u8>,
        valid: String,
    }

    impl JsonSerialize for RetryJson {
        fn json_serialize(&self, out: &mut String) {
            let call = self.calls.get();
            self.calls.set(call.saturating_add(1));
            if call == 0 {
                out.push_str("invalid fallback trigger");
            } else {
                out.push_str(&self.valid);
            }
        }
    }

    #[test]
    fn try_new_rejects_unparseable_payload() {
        let error = Json::try_new(BadJson).expect_err("invalid JSON serializer must fail closed");
        assert!(
            error.to_string().contains("invalid JSON document"),
            "unexpected serialization error: {error}"
        );
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

            let framed = norito::to_bytes(&Json::from_string_unchecked(invalid.to_owned()))
                .expect("encode hostile Json wire");
            assert!(
                norito::decode_from_bytes::<Json>(&framed).is_err(),
                "framed decoding accepted invalid Json payload: {invalid}"
            );
        }
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
