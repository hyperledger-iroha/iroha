//! JSON and Norito round-trip tests for the `Mintable` enum and helpers.

use iroha_data_model::{
    asset::definition::{MintabilityTokens, Mintable},
    isi::error::MintabilityError,
};
use norito::{
    codec::{Decode, Encode},
    json::{self, Value},
};

fn limited_tokens(count: u32) -> MintabilityTokens {
    MintabilityTokens::try_new(count).expect("non-zero tokens")
}

#[test]
fn mintable_basic_variants_serialize_as_strings() {
    let variants = [
        (Mintable::Infinitely, "Infinitely"),
        (Mintable::Once, "Once"),
        (Mintable::Not, "Not"),
    ];

    for (variant, expected) in variants {
        let json_value = json::to_value(&variant).expect("serialize");
        assert_eq!(
            json_value,
            Value::String(expected.to_owned()),
            "unexpected JSON for {expected}"
        );
        let decoded: Mintable = json::from_value(json_value).expect("deserialize");
        assert_eq!(decoded, variant);
    }
}

#[test]
fn mintable_limited_serialize_as_string_label() {
    let mintable = Mintable::limited(limited_tokens(7));
    let json_value = json::to_value(&mintable).expect("serialize");
    assert_eq!(
        json_value,
        Value::String("Limited(7)".to_owned()),
        "limited variant should encode as string label"
    );
    let decoded: Mintable = json::from_value(json_value).expect("deserialize");
    assert!(matches!(decoded, Mintable::Limited(tokens) if tokens.value() == 7));
}

#[test]
fn mintable_limited_accepts_object_representation() {
    let object = norito::json!({
        "kind": "Limited",
        "tokens": 5
    });
    let mintable: Mintable = json::from_value(object).expect("deserialize object form");
    assert!(matches!(mintable, Mintable::Limited(tokens) if tokens.value() == 5));
}

#[test]
fn mintable_limited_accepts_string_tokens() {
    let object = norito::json!({
        "kind": "Limited",
        "value": "3"
    });
    let mintable: Mintable = json::from_value(object).expect("deserialize string tokens");
    assert!(matches!(mintable, Mintable::Limited(tokens) if tokens.value() == 3));
}

#[test]
fn mintable_rejects_zero_tokens_in_limited_object() {
    let object = norito::json!({
        "kind": "Limited",
        "tokens": 0
    });
    let err = json::from_value::<Mintable>(object).expect_err("zero tokens should fail");
    let message = err.to_string();
    assert!(
        message.contains("tokens") || message.contains("Limited"),
        "unexpected error message: {message}"
    );
}

#[test]
fn mintability_tokens_constructor_handles_zero() {
    assert!(MintabilityTokens::new(1).is_some());
    assert!(MintabilityTokens::new(0).is_none());
    let err = MintabilityTokens::try_new(0).expect_err("zero value rejected");
    assert!(matches!(err, MintabilityError::InvalidMintabilityTokens(0)));
}

#[test]
fn mintable_consume_sequence_for_once_and_limited() {
    let mut once = Mintable::Once;
    let consumed = once.consume_one().expect("first mint ok");
    assert!(consumed, "Once should report consumption");
    assert!(matches!(once, Mintable::Not));
    let err = once.consume_one().expect_err("second mint forbidden");
    assert!(matches!(err, MintabilityError::MintUnmintable));

    let mut limited = Mintable::limited(limited_tokens(2));
    assert_eq!(limited.remaining_tokens().map(Into::into), Some(2));
    let consumed = limited.consume_one().expect("first limited mint ok");
    assert!(!consumed, "budget should remain after first mint");
    assert_eq!(limited.remaining_tokens().map(Into::into), Some(1));
    let consumed = limited.consume_one().expect("second limited mint ok");
    assert!(consumed, "budget exhausted after final mint");
    assert!(matches!(limited, Mintable::Not));
    let err = limited.consume_one().expect_err("limited exhausted");
    assert!(matches!(err, MintabilityError::MintUnmintable));
}

#[test]
fn mintable_rejects_zero_limited_budget() {
    let err = Mintable::limited_from_u32(0).expect_err("zero budget");
    assert!(matches!(err, MintabilityError::InvalidMintabilityTokens(0)));
}

#[test]
fn mintable_limited_roundtrip_via_norito() {
    let original = Mintable::limited(limited_tokens(7));
    let encoded = Mintable::encode(&original);
    let mut cursor = &encoded[..];
    let decoded = Mintable::decode(&mut cursor).expect("decode Norito payload");
    assert_eq!(decoded, original);
    assert!(
        cursor.is_empty(),
        "decoder should consume entire payload (remaining {cursor:?})"
    );
}
