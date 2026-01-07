//! JSON helpers for custom (de)serialization in data-model types.
//!
//! These helpers are intended for app-facing DTOs and are used with Norito's
//! `#[cfg_attr(feature = "json", norito(with = "..."))]` attribute. For base64 encoding, use
//! `#[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]` on `Vec<u8>` fields.

#[cfg(feature = "json")]
use std::collections::BTreeMap;
use std::{format, string::String, vec::Vec};

use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
#[cfg(feature = "json")]
use norito::json::{self, JsonDeserialize, JsonSerialize, Parser, Value};

#[cfg(feature = "json")]
use crate::soranet::privacy_metrics::SoranetPrivacyModeV1;

/// Serialize a `Vec<u8>` as a base64 string and deserialize from base64.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod base64_vec {
    use super::*;

    pub fn serialize(bytes: &[u8], out: &mut String) {
        JsonSerialize::json_serialize(&B64.encode(bytes), out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<Vec<u8>, norito::json::Error> {
        let encoded = parser.parse_string()?;
        B64.decode(encoded.as_bytes())
            .map_err(|err| norito::json::Error::Message(err.to_string()))
    }

    #[allow(dead_code)]
    pub mod option {
        use super::*;

        #[allow(clippy::ref_option)] // Required by Norito serializer signature.
        pub fn serialize(value: &Option<Vec<u8>>, out: &mut String) {
            match value.as_deref() {
                Some(bytes) => super::serialize(bytes, out),
                None => out.push_str("null"),
            }
        }

        pub fn deserialize(
            parser: &mut Parser<'_>,
        ) -> Result<Option<Vec<u8>>, norito::json::Error> {
            parser.skip_ws();
            if parser.try_consume_null()? {
                return Ok(None);
            }
            super::deserialize(parser).map(Some)
        }
    }
}

/// Serialize signed 128-bit integers as decimal strings to satisfy JSON codec expectations.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod i128_string {
    use super::*;

    pub fn serialize(value: &i128, out: &mut String) {
        JsonSerialize::json_serialize(&value.to_string(), out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<i128, norito::json::Error> {
        let raw = parser.parse_string()?;
        raw.parse::<i128>().map_err(|_| {
            norito::json::Error::Message(format!("invalid i128 string representation: {raw}"))
        })
    }
}

/// Helpers for fixed-size byte arrays (`[u8; N]`) and their container variants.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod fixed_bytes {
    use super::*;

    pub fn serialize<const N: usize>(bytes: &[u8; N], out: &mut String) {
        // Encode as a JSON array of byte values to match the historical Serde layout.
        let tmp: Vec<u8> = bytes.as_slice().to_vec();
        JsonSerialize::json_serialize(&tmp, out);
    }

    pub fn deserialize<const N: usize>(parser: &mut Parser<'_>) -> Result<[u8; N], json::Error> {
        let values = Vec::<u8>::json_deserialize(parser)?;
        vec_to_array::<N>(&values)
    }

    #[allow(dead_code)]
    pub mod option {
        use super::*;

        #[allow(clippy::ref_option)] // Norito serializer interface requires `&Option<T>` signature
        pub fn serialize<const N: usize>(value: &Option<[u8; N]>, out: &mut String) {
            match value.as_ref() {
                Some(bytes) => super::serialize(bytes, out),
                None => out.push_str("null"),
            }
        }

        pub fn deserialize<const N: usize>(
            parser: &mut Parser<'_>,
        ) -> Result<Option<[u8; N]>, json::Error> {
            parser.skip_ws();
            if parser.try_consume_null()? {
                return Ok(None);
            }
            super::deserialize(parser).map(Some)
        }
    }

    #[allow(dead_code)]
    pub mod vec {
        use super::*;

        pub fn serialize<const N: usize>(value: &[[u8; N]], out: &mut String) {
            let tmp: Vec<Vec<u8>> = value
                .iter()
                .map(|bytes| bytes.as_slice().to_vec())
                .collect();
            JsonSerialize::json_serialize(&tmp, out);
        }

        pub fn deserialize<const N: usize>(
            parser: &mut Parser<'_>,
        ) -> Result<Vec<[u8; N]>, json::Error> {
            let raw = Vec::<Vec<u8>>::json_deserialize(parser)?;
            raw.into_iter()
                .map(|values| vec_to_array::<N>(&values))
                .collect()
        }
    }

    #[allow(dead_code)]
    pub mod option_vec {
        use super::*;

        #[allow(clippy::ref_option)] // Norito serializer interface requires `&Option<T>` signature
        pub fn serialize<const N: usize>(value: &Option<Vec<[u8; N]>>, out: &mut String) {
            match value.as_deref() {
                Some(items) => vec::serialize(items, out),
                None => out.push_str("null"),
            }
        }

        pub fn deserialize<const N: usize>(
            parser: &mut Parser<'_>,
        ) -> Result<Option<Vec<[u8; N]>>, json::Error> {
            parser.skip_ws();
            if parser.try_consume_null()? {
                return Ok(None);
            }
            vec::deserialize(parser).map(Some)
        }
    }

    fn vec_to_array<const N: usize>(values: &[u8]) -> Result<[u8; N], json::Error> {
        if values.len() != N {
            return Err(json::Error::Message(format!(
                "expected {N} bytes, got {}",
                values.len()
            )));
        }
        let mut array = [0_u8; N];
        array.copy_from_slice(values);
        Ok(array)
    }
}

/// Serialize and deserialize fixed-size byte arrays as hex strings.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod fixed_bytes_hex {
    use super::*;

    pub fn serialize<const N: usize>(bytes: &[u8; N], out: &mut String) {
        let encoded = hex::encode(bytes);
        JsonSerialize::json_serialize(&encoded, out);
    }

    pub fn deserialize<const N: usize>(parser: &mut Parser<'_>) -> Result<[u8; N], json::Error> {
        let raw = parser.parse_string()?;
        parse_hex_bytes::<N>(&raw)
    }

    #[allow(dead_code)]
    pub mod option {
        use super::*;

        #[allow(clippy::ref_option)] // Norito serializer interface requires `&Option<T>` signature
        pub fn serialize<const N: usize>(value: &Option<[u8; N]>, out: &mut String) {
            match value.as_ref() {
                Some(bytes) => super::serialize(bytes, out),
                None => out.push_str("null"),
            }
        }

        pub fn deserialize<const N: usize>(
            parser: &mut Parser<'_>,
        ) -> Result<Option<[u8; N]>, json::Error> {
            parser.skip_ws();
            if parser.try_consume_null()? {
                return Ok(None);
            }
            super::deserialize(parser).map(Some)
        }
    }

    fn parse_hex_bytes<const N: usize>(raw: &str) -> Result<[u8; N], json::Error> {
        let trimmed = raw.trim();
        let without_scheme = if let Some((scheme, rest)) = trimmed.split_once(':') {
            if scheme.is_empty() || scheme.eq_ignore_ascii_case("blake2b32") {
                rest
            } else {
                return Err(json::Error::Message("expected hex string".to_string()));
            }
        } else {
            trimmed
        };
        let mut body = without_scheme.trim();
        if let Some(stripped) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
            body = stripped;
        }
        if body.len() != N * 2 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(json::Error::Message(format!(
                "expected {N}-byte hex string"
            )));
        }
        let mut out = [0_u8; N];
        hex::decode_to_slice(body, &mut out)
            .map_err(|err| json::Error::Message(err.to_string()))?;
        Ok(out)
    }
}

/// Serialize and deserialize [`SoranetPrivacyModeV1`] values as their label strings.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod privacy_mode {
    use super::*;

    #[allow(clippy::trivially_copy_pass_by_ref)] // Norito interface requires `&T` signature.
    pub fn serialize(value: &SoranetPrivacyModeV1, out: &mut String) {
        JsonSerialize::json_serialize(value.as_label(), out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<SoranetPrivacyModeV1, json::Error> {
        let label = parser.parse_string()?;
        match label.as_str() {
            "entry" => Ok(SoranetPrivacyModeV1::Entry),
            "middle" => Ok(SoranetPrivacyModeV1::Middle),
            "exit" => Ok(SoranetPrivacyModeV1::Exit),
            other => Err(json::Error::unknown_field(other)),
        }
    }
}

/// Helper that strips sensitive strings from JSON serialization while retaining internal storage.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod secret_string {
    use super::*;

    pub fn serialize(_value: &str, out: &mut String) {
        JsonSerialize::json_serialize("", out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<String, json::Error> {
        parser.skip_ws();
        if parser.try_consume_null()? {
            return Ok(String::new());
        }
        // Parse and discard the payload; consumers reconstruct empty message for external use.
        let _ignored = String::json_deserialize(parser)?;
        Ok(String::new())
    }
}

/// Serialize a map keyed by [`AccountId`] into a string-keyed JSON object.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod account_metadata_map {
    use std::str::FromStr;

    use super::*;
    use crate::{account::AccountId, metadata::Metadata};

    pub fn serialize(value: &BTreeMap<AccountId, Metadata>, out: &mut String) {
        let string_keyed: BTreeMap<String, Metadata> = value
            .iter()
            .map(|(account, metadata)| (account.to_string(), metadata.clone()))
            .collect();
        JsonSerialize::json_serialize(&string_keyed, out);
    }

    pub fn deserialize(
        parser: &mut Parser<'_>,
    ) -> Result<BTreeMap<AccountId, Metadata>, norito::json::Error> {
        let value = Value::json_deserialize(parser)?;
        let object = match value {
            Value::Object(map) => map,
            other => {
                return Err(norito::json::Error::Message(format!(
                    "expected object for account metadata map, got {other:?}"
                )));
            }
        };

        object
            .into_iter()
            .map(|(key, value)| {
                let account = AccountId::from_str(&key)
                    .map_err(|err| norito::json::Error::Message(err.to_string()))?;
                let metadata: Metadata = json::from_value(value)?;
                Ok((account, metadata))
            })
            .collect()
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use norito::json;

    use super::*;

    #[derive(Debug, PartialEq, Eq, JsonSerialize, crate::DeriveJsonDeserialize)]
    struct Base64Wrapper {
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        data: Vec<u8>,
    }

    #[test]
    fn base64_vec_roundtrip_serialization() {
        let wrapper = Base64Wrapper {
            data: vec![0_u8, 1, 2, 3, 255],
        };

        let json = json::to_json(&wrapper).expect("serialize to JSON");
        assert_eq!(json, "{\"data\":\"AAECA/8=\"}");

        let decoded: Base64Wrapper = json::from_str(&json).expect("decode from JSON");
        assert_eq!(decoded, wrapper);
    }

    #[test]
    fn base64_vec_rejects_invalid_input() {
        let json = "{\"data\":\"not-base64@@\"}";
        let err = json::from_str::<Base64Wrapper>(json).expect_err("invalid base64 must fail");

        match err {
            norito::json::Error::Message(message) => {
                let msg = message.to_ascii_lowercase();
                assert!(msg.contains("invalid"), "unexpected message: {message}");
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[derive(Debug, PartialEq, Eq, JsonSerialize, crate::DeriveJsonDeserialize)]
    struct I128Wrapper {
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::i128_string"))]
        value: i128,
    }

    #[test]
    fn i128_string_roundtrip_serialization() {
        let wrapper = I128Wrapper {
            value: -1_234_567_890_123_456_789,
        };

        let json = json::to_json(&wrapper).expect("serialize to JSON");
        assert_eq!(json, "{\"value\":\"-1234567890123456789\"}");

        let decoded: I128Wrapper = json::from_str(&json).expect("decode from JSON");
        assert_eq!(decoded, wrapper);
    }

    #[test]
    fn i128_string_rejects_invalid_input() {
        let json = "{\"value\":\"not-a-number\"}";
        let err = json::from_str::<I128Wrapper>(json).expect_err("invalid integer must fail");

        match err {
            norito::json::Error::Message(message) => assert!(
                message.contains("invalid i128 string representation"),
                "unexpected message: {message}"
            ),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
