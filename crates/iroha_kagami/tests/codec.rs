//! Tests for Kagami codec error handling and Norito/JSON roundtrips.

use std::{fmt::Debug, fs};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use iroha_data_model::{account::NewAccount, domain::Domain, trigger::Trigger};
use norito::{
    core::{Error, MAGIC},
    decode_from_bytes,
    json::{JsonDeserializeOwned, JsonSerialize},
    to_bytes,
};

const SAMPLE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/samples/codec");

fn roundtrip<T>(name: &str)
where
    T: JsonSerialize
        + JsonDeserializeOwned
        + PartialEq
        + Debug
        + norito::core::NoritoSerialize
        + for<'de> norito::NoritoDeserialize<'de>,
{
    iroha_genesis::init_instruction_registry();
    let json = fs::read_to_string(format!("{SAMPLE_DIR}/{name}.json")).expect("read json sample");
    let bin = fs::read(format!("{SAMPLE_DIR}/{name}.bin")).expect("read bin sample");

    // Check provided sample binary prefix
    assert_eq!(MAGIC.as_ref(), &bin[..MAGIC.len()]);

    let json_value: norito::json::Value = norito::json::from_str(&json).expect("parse JSON sample");
    if let norito::json::Value::String(encoded) = &json_value {
        let expected_b64 = BASE64.encode(&bin);
        assert_eq!(
            encoded, &expected_b64,
            "base64 JSON sample diverges from binary"
        );
        return;
    }

    // JSON -> Norito
    let from_json: T = norito::json::from_value(json_value.clone()).expect("decode JSON sample");
    let encoded = to_bytes(&from_json).expect("encode sample to Norito");
    assert_eq!(MAGIC.as_ref(), &encoded[..MAGIC.len()]);
    assert_eq!(encoded, bin, "generated Norito does not match sample");

    // Norito -> JSON
    let json_back = norito::json::to_value(&from_json).expect("encode to JSON");
    assert_eq!(json_back, json_value);
}

#[test]
fn account_roundtrip() {
    roundtrip::<NewAccount>("account");
}

#[test]
fn domain_roundtrip() {
    roundtrip::<Domain>("domain");
}

#[test]
fn trigger_roundtrip() {
    roundtrip::<Trigger>("trigger");
}

#[test]
fn decoder_returns_invalid_magic_error_on_bad_prefix() {
    // Craft a binary with an incorrect 4-byte prefix.
    let mut bytes = MAGIC;
    bytes[0] ^= 0xFF;
    let err = decode_from_bytes::<u32>(&bytes).expect_err("decoder should fail on bad magic");
    assert!(matches!(err, Error::InvalidMagic));
}

#[test]
#[ignore = "regenerates codec samples"]
fn regenerate_codec_samples() {
    fn write_bin<T>(name: &str)
    where
        T: JsonSerialize
            + JsonDeserializeOwned
            + norito::core::NoritoSerialize
            + for<'de> norito::NoritoDeserialize<'de>,
    {
        let json =
            fs::read_to_string(format!("{SAMPLE_DIR}/{name}.json")).expect("read json sample");
        let value: T = norito::json::from_str(&json).expect("parse json sample");
        let encoded = to_bytes(&value).expect("encode sample");
        fs::write(format!("{SAMPLE_DIR}/{name}.bin"), encoded).expect("write bin sample");
    }

    iroha_genesis::init_instruction_registry();
    write_bin::<NewAccount>("account");
    write_bin::<Domain>("domain");
}
