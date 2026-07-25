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

/// Serialize unsigned 64-bit integers as canonical decimal strings and reject
/// every non-canonical spelling on input.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod u64_string {
    use super::*;

    fn parse_canonical(raw: &str) -> Result<u64, norito::json::Error> {
        if raw.is_empty()
            || (raw.len() > 1 && raw.starts_with('0'))
            || !raw.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(norito::json::Error::Message(format!(
                "invalid canonical u64 decimal string: {raw}"
            )));
        }
        raw.parse::<u64>().map_err(|_| {
            norito::json::Error::Message(format!("u64 decimal string is out of range: {raw}"))
        })
    }

    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "Norito `with` serializers receive fields by shared reference"
    )]
    pub fn serialize(value: &u64, out: &mut String) {
        JsonSerialize::json_serialize(&value.to_string(), out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<u64, norito::json::Error> {
        parse_canonical(&parser.parse_string()?)
    }

    pub mod option {
        use super::*;

        #[allow(clippy::ref_option)]
        pub fn serialize(value: &Option<u64>, out: &mut String) {
            match value {
                Some(value) => super::serialize(value, out),
                None => out.push_str("null"),
            }
        }

        pub fn deserialize(parser: &mut Parser<'_>) -> Result<Option<u64>, norito::json::Error> {
            parser.skip_ws();
            if parser.try_consume_null()? {
                return Ok(None);
            }
            super::deserialize(parser).map(Some)
        }
    }
}

/// Serialize unsigned 128-bit integers as canonical decimal strings and reject
/// every non-canonical spelling on input.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod u128_string {
    use super::*;

    fn parse_canonical(raw: &str) -> Result<u128, norito::json::Error> {
        if raw.is_empty()
            || (raw.len() > 1 && raw.starts_with('0'))
            || !raw.bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err(norito::json::Error::Message(format!(
                "invalid canonical u128 decimal string: {raw}"
            )));
        }
        raw.parse::<u128>().map_err(|_| {
            norito::json::Error::Message(format!("u128 decimal string is out of range: {raw}"))
        })
    }

    pub fn serialize(value: &u128, out: &mut String) {
        JsonSerialize::json_serialize(&value.to_string(), out);
    }

    pub fn deserialize(parser: &mut Parser<'_>) -> Result<u128, norito::json::Error> {
        parse_canonical(&parser.parse_string()?)
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

/// Serialize fixed-size `u64` limb arrays as canonical JSON arrays.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod fixed_u64_limbs {
    use super::*;

    pub fn serialize<const N: usize>(limbs: &[u64; N], out: &mut String) {
        JsonSerialize::json_serialize(&limbs.as_slice().to_vec(), out);
    }

    pub fn deserialize<const N: usize>(parser: &mut Parser<'_>) -> Result<[u64; N], json::Error> {
        let limbs = Vec::<u64>::json_deserialize(parser)?;
        limbs.try_into().map_err(|limbs: Vec<u64>| {
            json::Error::Message(format!(
                "expected exactly {N} u64 limbs, got {}",
                limbs.len()
            ))
        })
    }
}

/// Serialize fixed-size `u32` limb arrays as canonical JSON arrays.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod fixed_u32_limbs {
    use super::*;

    pub fn serialize<const N: usize>(limbs: &[u32; N], out: &mut String) {
        out.push('[');
        for (index, limb) in limbs.iter().enumerate() {
            if index != 0 {
                out.push(',');
            }
            JsonSerialize::json_serialize(limb, out);
        }
        out.push(']');
    }

    pub fn deserialize<const N: usize>(parser: &mut Parser<'_>) -> Result<[u32; N], json::Error> {
        parser.expect(b'[')?;
        let mut limbs = [0_u32; N];
        if N == 0 {
            parser.expect(b']')?;
            return Ok(limbs);
        }
        for (index, limb) in limbs.iter_mut().enumerate() {
            parser.skip_ws();
            if parser.try_consume_char(b']')? {
                return Err(json::Error::Message(format!(
                    "expected exactly {N} u32 limbs, got {index}"
                )));
            }
            *limb = u32::json_deserialize(parser)?;
            if index + 1 < N {
                if parser.try_consume_char(b']')? {
                    return Err(json::Error::Message(format!(
                        "expected exactly {N} u32 limbs, got {}",
                        index + 1
                    )));
                }
                parser.expect(b',')?;
            }
        }
        if parser.try_consume_char(b']')? {
            return Ok(limbs);
        }
        if parser.try_consume_char(b',')? {
            return Err(json::Error::Message(format!(
                "expected exactly {N} u32 limbs, got more than {N}"
            )));
        }
        parser.expect(b']')?;
        Ok(limbs)
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
                let account = AccountId::parse_encoded(&key)
                    .map(crate::account::ParsedAccountId::into_account_id)
                    .map_err(|err| norito::json::Error::Message(err.to_string()))?;
                let metadata: Metadata = json::from_value(value)?;
                Ok((account, metadata))
            })
            .collect()
    }
}

/// Serialize Soracloud Inrou guest-image maps as string-keyed JSON objects.
#[cfg(feature = "json")]
#[allow(dead_code)]
pub mod sora_inrou_guest_images_map {
    use super::*;
    use crate::soracloud::{SoraInrouGuestImageV1, SoraInrouGuestIsaV1};

    pub fn serialize(
        value: &BTreeMap<SoraInrouGuestIsaV1, SoraInrouGuestImageV1>,
        out: &mut String,
    ) {
        let string_keyed: BTreeMap<String, SoraInrouGuestImageV1> = value
            .iter()
            .map(|(guest_isa, image)| (guest_isa.as_str().to_owned(), image.clone()))
            .collect();
        JsonSerialize::json_serialize(&string_keyed, out);
    }

    pub fn deserialize(
        parser: &mut Parser<'_>,
    ) -> Result<BTreeMap<SoraInrouGuestIsaV1, SoraInrouGuestImageV1>, norito::json::Error> {
        let value = Value::json_deserialize(parser)?;
        let object = match value {
            Value::Object(map) => map,
            other => {
                return Err(norito::json::Error::Message(format!(
                    "expected object for Soracloud Inrou guest image map, got {other:?}"
                )));
            }
        };

        object
            .into_iter()
            .map(|(key, value)| {
                let guest_isa = SoraInrouGuestIsaV1::parse_key(&key).ok_or_else(|| {
                    norito::json::Error::Message(format!(
                        "unsupported Soracloud Inrou guest ISA key: {key}"
                    ))
                })?;
                let image: SoraInrouGuestImageV1 = json::from_value(value)?;
                Ok((guest_isa, image))
            })
            .collect()
    }
}

#[cfg(all(test, feature = "json"))]
mod tests {
    use norito::json;

    use super::*;
    use crate::soracloud::{
        SoraArtifactDistributionPolicyV1, SoraInrouGuestImageV1, SoraInrouGuestIsaV1,
    };

    #[derive(Debug, PartialEq, Eq, JsonSerialize, crate::DeriveJsonDeserialize)]
    struct Base64Wrapper {
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::base64_vec"))]
        data: Vec<u8>,
    }

    #[derive(Debug, PartialEq, Eq, JsonSerialize, crate::DeriveJsonDeserialize)]
    struct FixedU64LimbsWrapper {
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_u64_limbs")
        )]
        limbs: [u64; 4],
    }

    #[derive(Debug, PartialEq, Eq, JsonSerialize, crate::DeriveJsonDeserialize)]
    struct FixedU32LimbsWrapper {
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::fixed_u32_limbs")
        )]
        limbs: [u32; 4],
    }

    #[test]
    fn fixed_u64_limbs_roundtrip_and_reject_wrong_length() {
        let wrapper = FixedU64LimbsWrapper {
            limbs: [0, 1, 42, u64::MAX],
        };

        let encoded = json::to_json(&wrapper).expect("serialize fixed u64 limbs");
        let decoded: FixedU64LimbsWrapper =
            json::from_str(&encoded).expect("decode fixed u64 limbs");
        assert_eq!(decoded, wrapper);

        let error = json::from_str::<FixedU64LimbsWrapper>(r#"{"limbs":[1,2,3]}"#)
            .expect_err("wrong fixed limb count must fail");
        assert!(
            error.to_string().contains("expected exactly 4 u64 limbs"),
            "unexpected fixed-limb error: {error}"
        );
    }

    #[test]
    fn fixed_u32_limbs_stream_exact_length_and_type() {
        let wrapper = FixedU32LimbsWrapper {
            limbs: [0, 1, 42, u32::MAX],
        };
        let encoded = json::to_json(&wrapper).expect("serialize fixed u32 limbs");
        assert_eq!(encoded, r#"{"limbs":[0,1,42,4294967295]}"#);
        let decoded: FixedU32LimbsWrapper =
            json::from_str(&encoded).expect("decode fixed u32 limbs");
        assert_eq!(decoded, wrapper);

        let short = json::from_str::<FixedU32LimbsWrapper>(r#"{"limbs":[1,2,3]}"#)
            .expect_err("short fixed limb array must fail");
        assert!(short.to_string().contains("expected exactly 4 u32 limbs"));

        let long = json::from_str::<FixedU32LimbsWrapper>(r#"{"limbs":[1,2,3,4,5]}"#)
            .expect_err("long fixed limb array must fail before parsing the fifth value");
        assert!(long.to_string().contains("more than 4"));

        json::from_str::<FixedU32LimbsWrapper>(r#"{"limbs":[1,2,-1,4]}"#)
            .expect_err("non-u32 limb must fail");
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

    #[derive(Debug, PartialEq, Eq, JsonSerialize, crate::DeriveJsonDeserialize)]
    struct InrouGuestImagesWrapper {
        #[cfg_attr(
            feature = "json",
            norito(with = "crate::json_helpers::sora_inrou_guest_images_map")
        )]
        guest_images: BTreeMap<SoraInrouGuestIsaV1, SoraInrouGuestImageV1>,
    }

    #[test]
    fn sora_inrou_guest_images_map_roundtrip_serialization() {
        let wrapper = InrouGuestImagesWrapper {
            guest_images: BTreeMap::from([
                (
                    SoraInrouGuestIsaV1::X8664,
                    SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/x86_64/vmlinux".to_owned(),
                        rootfs_image_path: "/inrou/x86_64/rootfs.ext4".to_owned(),
                        initrd_image_path: None,
                        distribution: SoraArtifactDistributionPolicyV1::default(),
                        published_artifact: None,
                    },
                ),
                (
                    SoraInrouGuestIsaV1::Aarch64,
                    SoraInrouGuestImageV1 {
                        kernel_image_path: "/inrou/aarch64/vmlinux".to_owned(),
                        rootfs_image_path: "/inrou/aarch64/rootfs.ext4".to_owned(),
                        initrd_image_path: Some("/inrou/aarch64/initrd.img".to_owned()),
                        distribution: SoraArtifactDistributionPolicyV1::default(),
                        published_artifact: None,
                    },
                ),
            ]),
        };

        let json = json::to_json(&wrapper).expect("serialize to JSON");
        assert!(json.contains("\"x86_64\""));
        assert!(json.contains("\"aarch64\""));

        let decoded: InrouGuestImagesWrapper = json::from_str(&json).expect("decode from JSON");
        assert_eq!(decoded, wrapper);
    }

    #[test]
    fn sora_inrou_guest_images_map_rejects_unknown_keys() {
        let json = r#"{"guest_images":{"riscv64":{"kernel_image_path":"/inrou/riscv64/vmlinux","rootfs_image_path":"/inrou/riscv64/rootfs.ext4","initrd_image_path":null}}}"#;
        let err = json::from_str::<InrouGuestImagesWrapper>(json)
            .expect_err("unknown guest ISA must fail");

        match err {
            norito::json::Error::Message(message) => assert!(
                message.contains("unsupported Soracloud Inrou guest ISA key"),
                "unexpected message: {message}"
            ),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }
}
