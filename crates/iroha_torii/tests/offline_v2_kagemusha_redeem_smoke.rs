//! Source-level smoke checks for the offline v2 Kagemusha redeem bridge.

const OFFLINE_V2_ISSUER_SOURCE: &str = include_str!("../src/offline_v2_issuer.rs");

#[tokio::test]
async fn offline_v2_notes_redeem_accepts_kagemusha_recursive_redeem_request() {
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("/v1/offline/v2/notes/redeem"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("redeem_request_norito_base64"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("KagemushaRecursiveSpendRedeemRequestV1"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("RedeemKagemushaRecursive"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("new_with_lineage_witness_and_change"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("required_kagemusha_redeem_archive_string"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("optional_kagemusha_echo_string"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("parse_kagemusha_amount_echo"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("must use canonical Numeric text"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("reject_kagemusha_legacy_redeem_fields"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("reject_kagemusha_auxiliary_redeem_fields"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("must be a canonical base64 string"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("must not contain surrounding whitespace"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_KAGEMUSHA_REDEEM_CHAIN_MISMATCH"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_KAGEMUSHA_REDEEM_ASSET_MISMATCH"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_KAGEMUSHA_REDEEM_AMOUNT_MISMATCH"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_KAGEMUSHA_REDEEM_SOURCE_MISMATCH"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_KAGEMUSHA_REDEEM_LEGACY_FIELD"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_KAGEMUSHA_REDEEM_AUXILIARY_FIELD"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_KAGEMUSHA_REDEEM_INVALID"));
    assert!(
        OFFLINE_V2_ISSUER_SOURCE
            .contains("offline_v2_notes_redeem_rejects_kagemusha_optional_echo_field_shapes")
    );
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains(
        "offline_v2_notes_redeem_rejects_legacy_redemption_smuggled_with_kagemusha_marker"
    ));
    assert!(
        OFFLINE_V2_ISSUER_SOURCE
            .contains("offline_v2_notes_redeem_rejects_legacy_fields_with_kagemusha_archive")
    );
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains(
        "offline_v2_notes_redeem_rejects_auxiliary_kagemusha_fields_with_redeem_archive"
    ));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("let auxiliary_field_values ="));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("Value::Null, Value::Array(Vec::new())"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("Value::Array(Vec::new())"));
}

#[tokio::test]
async fn offline_v2_notes_redeem_rejects_compact_token_without_recursive_redeem_request() {
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("compact_payment_token_norito_base64"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("projection_verifier_record_norito_base64"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_KAGEMUSHA_REDEEM_REQUEST_REQUIRED"));
}
