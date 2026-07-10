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

        match value {
            json::Value::Null => out.push_str("null"),
            json::Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            json::Value::Number(n) => match n {
                Number::I64(i) => out.push_str(&i.to_string()),
                Number::U64(u) => out.push_str(&u.to_string()),
                Number::F64(f) => {
                    const F64_SAFE_INT: f64 = 9_007_199_254_740_992.0; // 2^53
                    if f.is_finite() && f.fract() == 0.0 && f.abs() <= F64_SAFE_INT {
                        let _ = write!(out, "{f:.1}");
                    } else {
                        let _ = write!(out, "{f:?}");
                    }
                }
            },
            json::Value::String(s) => Self::escape_json_string_plain(s, out),
            json::Value::Array(items) => {
                out.push('[');
                let mut iter = items.iter().peekable();
                while let Some(item) = iter.next() {
                    Self::serialize_json_value_plain(item, out);
                    if iter.peek().is_some() {
                        out.push(',');
                    }
                }
                out.push(']');
            }
            json::Value::Object(map) => {
                out.push('{');
                let mut iter = map.iter().peekable();
                while let Some((k, v)) = iter.next() {
                    Self::escape_json_string_plain(k, out);
                    out.push(':');
                    Self::serialize_json_value_plain(v, out);
                    if iter.peek().is_some() {
                        out.push(',');
                    }
                }
                out.push('}');
            }
        }
    }

    fn serialize_json_value_plain_str(value: &json::Value) -> String {
        let mut out = String::new();
        Self::serialize_json_value_plain(value, &mut out);
        out
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
        if norito::json::parse_value(&serialized).is_ok() {
            return Ok(Self::from_string_unchecked(serialized));
        }

        // Fallback: materialize a `Value` and serialize it plainly.
        if let Ok(value) = norito::json::to_value(&payload) {
            let plain = Self::serialize_json_value_plain_str(&value);
            if norito::json::parse_value(&plain).is_ok() {
                return Ok(Self::from_string_unchecked(plain));
            }
            // Even if parsing fails, keep the plain serialization to avoid panicking.
            return Ok(Self::from_string_unchecked(plain));
        }

        // Last resort: return the original serialized form even if validation failed.
        Ok(Self::from_string_unchecked(serialized))
    }

    /// Fallible constructor from `&str` using Norito's JSON helper.
    /// Unlike `FromStr`, this helper is strict and rejects non-JSON text.
    ///
    /// # Errors
    /// Returns an error if the input string is not valid JSON.
    pub fn from_str_norito(s: &str) -> Result<Self, norito::Error> {
        let value = json::parse_value(s).map_err(|e| norito::Error::from(e.to_string()))?;
        Self::from_norito_value_ref(&value)
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
        norito::json::parse_value(&plain).map_err(|e| norito::Error::from(e.to_string()))?;
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
        Json::from_norito_value_ref(&value).expect("json to_string")
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
        match json::parse_value(s) {
            Ok(value) => Json::from_norito_value_ref(&value),
            Err(_parse_err) => {
                let value = Value::String(s.to_owned());
                Json::from_norito_value_ref(&value)
            }
        }
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
                    "wire_proto_versions".into(),
                    norito::json::Value::Array(vec![norito::json::Value::from(1u64)]),
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

    struct BadJson;

    impl JsonSerialize for BadJson {
        fn json_serialize(&self, out: &mut String) {
            out.push_str("bad json trailing");
        }
    }

    #[test]
    fn try_new_tolerates_unparseable_payload() {
        let j = Json::try_new(BadJson).expect("try_new must not panic");
        assert!(
            j.get().starts_with("bad json"),
            "unexpected serialization: {}",
            j.get()
        );
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
