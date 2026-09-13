//! Canonical compute policy strings, bounded writers, and removed-shape rejection.

use super::*;

fn check_policy<T>(value: T, label: &str, tag: &str, discriminant: u32)
where
    T: json::JsonSerialize
        + json::JsonDeserialize
        + norito::NoritoSerialize
        + for<'a> norito::NoritoDeserialize<'a>
        + core::fmt::Debug
        + PartialEq,
{
    let expected = format!("\"{label}\"");
    assert_eq!(json::to_string(&value).unwrap(), expected);
    assert_eq!(
        json::to_value(&value).unwrap(),
        json::Value::String(label.into())
    );
    assert_eq!(json::from_str::<T>(&expected).unwrap(), value);
    assert_eq!(
        json::from_value::<T>(json::Value::String(label.into())).unwrap(),
        value,
    );
    assert_eq!(
        json::to_json_bounded(&value, expected.len()).unwrap(),
        expected,
    );
    assert!(matches!(
        json::to_json_bounded(&value, expected.len() - 1),
        Err(json::BoundedJsonError::BodyTooLarge),
    ));
    // JSON representation changes neither the enum payload nor its existing
    // nominal frame identity. The captured owner suite checks the frame hash.
    let (payload, flags) = norito::codec::encode_with_header_flags(&value);
    assert_eq!(payload, discriminant.to_le_bytes());
    let framed = norito::core::frame_bare_with_header_flags::<T>(&payload, flags).unwrap();
    assert_eq!(norito::decode_from_bytes::<T>(&framed).unwrap(), value);

    let removed_envelope = format!(r#"{{"{tag}":"{label}","value":null}}"#);
    let removed_tag_only = format!(r#"{{"{tag}":"{label}"}}"#);
    let unknown = format!(r#""{label}_unknown""#);
    let lower = format!("\"{}\"", label.to_ascii_lowercase());
    let padded = format!("\" {label}\"");
    for rejected in [
        removed_envelope.as_str(),
        removed_tag_only.as_str(),
        unknown.as_str(),
        lower.as_str(),
        padded.as_str(),
        "{}",
        "[]",
        "null",
        "false",
        "0",
    ] {
        assert!(json::from_str::<T>(rejected).is_err(), "{rejected}");
        assert!(
            json::from_value::<T>(json::parse_value(rejected).unwrap()).is_err(),
            "{rejected}",
        );
    }
}

#[test]
fn randomness_policy_uses_exact_strings_and_rejects_removed_envelopes() {
    check_policy(ComputeRandomnessPolicy::None, "None", "randomness", 0);
    check_policy(
        ComputeRandomnessPolicy::SeededFromRequest,
        "SeededFromRequest",
        "randomness",
        1,
    );
}

#[test]
fn storage_policy_uses_exact_strings_and_rejects_removed_envelopes() {
    check_policy(ComputeStorageAccess::ReadOnly, "ReadOnly", "storage", 0);
    check_policy(ComputeStorageAccess::ReadWrite, "ReadWrite", "storage", 1);
}

#[test]
fn authentication_policy_uses_exact_strings_and_rejects_removed_envelopes() {
    check_policy(ComputeAuthPolicy::PublicOnly, "PublicOnly", "mode", 0);
    check_policy(
        ComputeAuthPolicy::AuthenticatedOnly,
        "AuthenticatedOnly",
        "mode",
        1,
    );
    check_policy(ComputeAuthPolicy::Either, "Either", "mode", 2);
}

#[test]
fn price_risk_class_uses_exact_strings_and_rejects_removed_envelopes() {
    check_policy(ComputePriceRiskClass::Low, "Low", "class", 0);
    check_policy(ComputePriceRiskClass::Balanced, "Balanced", "class", 1);
    check_policy(ComputePriceRiskClass::High, "High", "class", 2);
}
