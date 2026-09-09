//! Configuration scalars retain their string field codec without owning a frame identity.

use std::fmt::Display;

use norito::{
    DeserializePayload, SerializePayload,
    codec::Encode,
    core::{DecodeFlagsGuard, Encoder, decode_field_canonical, header_flags},
    json::{JsonDeserialize, JsonSerialize},
};

use crate::{
    kura::{FsyncMode, InitMode},
    logger::{Directives, Format},
    snapshot::Mode,
};

fn explicit_field_payload<T: SerializePayload>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    value
        .serialize(&mut Encoder::for_buffer(&mut bytes))
        .unwrap();
    bytes
}

fn assert_string_payload<T>(value: T, expected: &str)
where
    T: SerializePayload
        + for<'de> DeserializePayload<'de>
        + JsonSerialize
        + JsonDeserialize
        + Display,
{
    assert_eq!(value.to_string(), expected);
    let mut canonical = vec![u8::try_from(expected.len()).unwrap()];
    canonical.extend_from_slice(expected.as_bytes());
    assert_eq!(value.encode(), canonical);
    for flags in [0, header_flags::COMPACT_LEN] {
        let _layout = DecodeFlagsGuard::enter(flags);
        // Encode selects the fixed bare V1 layout regardless of this ambient field layout.
        assert_eq!(value.encode(), canonical);
        assert_eq!(value.encode(), expected.to_owned().encode());
        let bytes = explicit_field_payload(&value);
        assert_eq!(bytes, explicit_field_payload(&expected.to_owned()));
        // These short scalar spellings have either an explicit u64 or one-byte compact length.
        let mut exact = if flags == 0 {
            u64::try_from(expected.len())
                .unwrap()
                .to_le_bytes()
                .to_vec()
        } else {
            vec![u8::try_from(expected.len()).unwrap()]
        };
        exact.extend_from_slice(expected.as_bytes());
        assert_eq!(bytes, exact);
        let (decoded, used) = decode_field_canonical::<T>(&bytes).unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(decoded.to_string(), expected);
        assert_eq!(explicit_field_payload(&decoded), bytes);
        let mut trailing = bytes;
        trailing.push(0);
        assert!(decode_field_canonical::<T>(&trailing).is_err());
    }
    let json = norito::json::to_json(&value).unwrap();
    assert_eq!(json, norito::json::to_json(&expected).unwrap());
    let decoded = norito::json::from_json::<T>(&json).unwrap();
    assert_eq!(decoded.to_string(), expected);
}

fn assert_invalid_string<T>(invalid: &str)
where
    T: SerializePayload + for<'de> DeserializePayload<'de> + JsonDeserialize,
{
    for flags in [0, header_flags::COMPACT_LEN] {
        let _layout = DecodeFlagsGuard::enter(flags);
        assert!(decode_field_canonical::<T>(&explicit_field_payload(&invalid.to_owned())).is_err());
    }
    let json = norito::json::to_json(&invalid).unwrap();
    assert!(norito::json::from_json::<T>(&json).is_err());
}

#[test]
fn kura_init_mode_retains_its_string_payload() {
    assert_string_payload(InitMode::Strict, "strict");
    assert_string_payload(InitMode::Fast, "fast");
    assert_invalid_string::<InitMode>("unknown");
}

#[test]
fn kura_fsync_mode_retains_its_string_payload() {
    assert_string_payload(FsyncMode::Always, "always");
    assert_string_payload(FsyncMode::Batched, "batched");
    for invalid in ["off", "on", "unknown"] {
        assert_invalid_string::<FsyncMode>(invalid);
    }
}

#[test]
fn logger_format_retains_its_string_payload() {
    for (value, text) in [
        (Format::Full, "full"),
        (Format::Compact, "compact"),
        (Format::Pretty, "pretty"),
        (Format::Json, "json"),
    ] {
        assert_string_payload(value, text);
    }
    assert_invalid_string::<Format>("unknown");
}

#[test]
fn logger_directives_retain_their_string_payload() {
    for text in ["", "info", "iroha_core=trace,axum=warn"] {
        assert_string_payload(text.parse::<Directives>().unwrap(), text);
    }
    let invalid = "iroha_core=unknown_level";
    assert!(invalid.parse::<Directives>().is_err());
    assert_invalid_string::<Directives>(invalid);
}

#[test]
fn snapshot_mode_retains_its_string_payload() {
    for (value, text) in [
        (Mode::ReadWrite, "read_write"),
        (Mode::Readonly, "readonly"),
        (Mode::Disabled, "disabled"),
    ] {
        assert_string_payload(value, text);
    }
    assert_invalid_string::<Mode>("unknown");
}
