//! Source-level smoke checks for the offline v2 Kagemusha redeem bridge.

const OFFLINE_V2_ISSUER_SOURCE: &str = include_str!("../src/offline_v2_issuer.rs");

fn source_section(start: &str, end: &str) -> &'static str {
    let start = OFFLINE_V2_ISSUER_SOURCE
        .find(start)
        .expect("source section start marker must exist");
    let end = OFFLINE_V2_ISSUER_SOURCE[start..]
        .find(end)
        .map(|offset| start + offset)
        .expect("source section end marker must exist");
    &OFFLINE_V2_ISSUER_SOURCE[start..end]
}

#[tokio::test]
async fn offline_v2_notes_redeem_accepts_kagemusha_recursive_redeem_request() {
    assert!(
        OFFLINE_V2_ISSUER_SOURCE
            .contains("const PATH_NOTES_REDEEM: &str = \"/v1/offline/v2/notes/redeem\";")
    );
    assert!(!OFFLINE_V2_ISSUER_SOURCE.contains("/v1/offline/v2/kagemusha/redeem"));

    let handler = source_section(
        "pub(crate) async fn handle_notes_redeem(",
        "async fn handle_kagemusha_recursive_notes_redeem(",
    );
    for marker in [
        "reject_x_iroha_auth_headers(headers)?;",
        "parse_strict_kagemusha_v2_archive::<KagemushaRecursiveSpendRedeemRequestV2>(",
        "redeem_request_norito_base64",
        "redeem_request.validate_public_binding()",
        "validate_kagemusha_v2_redeem_snapshot(&app, &redeem_request)?;",
        "load_kagemusha_v2_redeem_operation_receipt(",
        "let operation_id = redeem_request.authorization.operation_id;",
        "RedeemKagemushaRecursiveV2::new(redeem_request)",
        "wait_for_kagemusha_v2_finality(&app, tx_hash, operation_id).await?",
        "kagemusha_v2_terminal_response(finality, None)",
        "PATH_NOTES_REDEEM",
    ] {
        assert!(
            handler.contains(marker),
            "missing V2 redeem handler marker: {marker}"
        );
    }
    assert!(!handler.contains("KagemushaRecursiveSpendRedeemRequestV1"));
    assert!(!handler.contains("RedeemKagemushaRecursive::"));
}

#[tokio::test]
async fn offline_v2_notes_redeem_rejects_noncanonical_or_ambiguous_v2_envelopes() {
    let parser = source_section(
        "fn parse_strict_kagemusha_v2_archive<T>(",
        "fn kagemusha_v2_snapshot_time_ms(",
    );
    for marker in [
        "if object.len() != 1 || !object.contains_key(field)",
        "must contain exactly",
        "if encoded.is_empty() || encoded.trim() != encoded",
        "must be non-empty with no surrounding whitespace",
        "BASE64_STANDARD.decode(encoded)",
        "BASE64_STANDARD.encode(&bytes) != encoded",
        "is not canonical standard base64",
        "norito::decode_from_bytes(&bytes)",
        "norito::to_bytes(&decoded)",
        "if canonical != bytes",
        "does not round-trip to identical canonical Norito",
    ] {
        assert!(
            parser.contains(marker),
            "missing strict V2 parser marker: {marker}"
        );
    }
}

#[tokio::test]
async fn offline_v2_notes_redeem_uses_direct_receipts_and_preserves_finality_integrity() {
    for marker in [
        "optional_finalized_kagemusha_v2_anchor",
        "finalized_kagemusha_v2_topup_anchor_finality",
        "committed_transaction_height(&transaction_hash)",
        "load_kagemusha_v2_redeem_operation_receipt",
        "OFFLINE_KAGEMUSHA_REDEEM_RECEIPT_UNAVAILABLE",
        "pipeline_status_terminal_or_state_entry",
        "OFFLINE_KAGEMUSHA_FINALITY_INCOMPLETE",
        "ensure_kagemusha_v2_anchor_finality_binding",
        "operation_id: [u8; 32]",
        "newly_applied_kagemusha_v2_redeem_response_preserves_operation_id",
        "replayed_kagemusha_v2_redeem_response_preserves_operation_id",
        "kagemusha_v2_terminal_response_rejects_zero_or_non_applied_finality",
        "kagemusha_v2_terminal_status_rejects_missing_height_or_block_time",
        "kagemusha_v2_terminal_cache_rejection_and_expiry_do_not_timeout",
        "kagemusha_v2_anchor_finality_binding_rejects_operation_hash_or_height_mismatch",
        "refreshed_kagemusha_v2_authorization_keeps_direct_anchor_lookup_key",
    ] {
        assert!(
            OFFLINE_V2_ISSUER_SOURCE.contains(marker),
            "missing bounded replay/finality marker: {marker}"
        );
    }
}
