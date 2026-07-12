//! Source-level closure guards for the first-release typed offline redeem command.

const OFFLINE_ISSUER_SOURCE: &str = include_str!("../src/offline_commands.rs");
const OFFLINE_API_SOURCE: &str = include_str!("../../iroha_torii_shared/src/offline_api.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

fn production_source(source: &str) -> &str {
    source
        .split("\nmod tests")
        .next()
        .expect("production source")
}

#[test]
fn typed_offline_redeem_route_accepts_only_the_direct_v2_request() {
    let issuer = production_source(OFFLINE_ISSUER_SOURCE);

    assert!(TORII_SOURCE.contains("&route_catalog::offline::REDEEM"));
    assert!(TORII_SOURCE.contains("catalog_post(handler_offline_redeem)"));
    assert!(TORII_SOURCE.contains("offline_api::OfflineRedeemRequest"));
    assert!(TORII_SOURCE.contains("NoritoJson(request)"));
    assert!(OFFLINE_API_SOURCE.contains("as OfflineRedeemRequest"));
    assert!(OFFLINE_API_SOURCE.contains("OFFLINE_REDEEM_REQUEST_SCHEMA_NAME"));
    assert_eq!(
        iroha_torii_shared::offline_api::OFFLINE_REDEEM_REQUEST_SCHEMA_NAME,
        "iroha.torii.v1.offline.redeem.request"
    );
    assert!(issuer.contains("handle_redeem"));
    assert!(issuer.contains("redeem_request: OfflineRedeemRequest"));
    assert!(issuer.contains("require_idempotency_key"));
    assert!(issuer.contains("OfflineOperationReference"));
    assert!(issuer.contains("StatusCode::ACCEPTED"));
    assert!(issuer.contains("header::LOCATION"));
    assert!(issuer.contains("header::RETRY_AFTER"));
    assert!(issuer.contains("header::CACHE_CONTROL"));
    assert!(!issuer.contains("wait_for_kagemusha_v2_finality"));
    assert!(!issuer.contains("kagemusha_v2_terminal_response"));
}

#[test]
fn typed_offline_redeem_contract_rejects_wrappers_aliases_and_ambiguous_envelopes() {
    let issuer = production_source(OFFLINE_ISSUER_SOURCE);

    for retired_field in [
        "redeem_request_norito_base64",
        "compact_payment_token_norito_base64",
        "projection_verifier_record_norito_base64",
    ] {
        assert!(
            !issuer.contains(retired_field),
            "whole-payload wrapper must be absent: {retired_field}"
        );
    }
    assert!(!issuer.contains("KagemushaRecursiveSpendRedeemRequestV1"));
    for retired_path in [
        "/v1/offline/v2/notes/redeem",
        "/v1/offline/v2/redeem",
        "/v1/offline/notes/redeem",
        "/v1/offline/cash/redeem",
    ] {
        assert!(
            !TORII_SOURCE.contains(&format!(".route(\"{retired_path}\"")),
            "retired route must not be mounted: {retired_path}"
        );
    }
    for retired_handler in [
        "handle_notes_redeem",
        "parse_strict_kagemusha_v2_archive",
        "ParsedOfflineRequest",
    ] {
        assert!(
            !issuer.contains(retired_handler),
            "retired compatibility parser/handler must be absent: {retired_handler}"
        );
    }
}

#[test]
fn offline_operation_polling_preserves_redeem_identity_and_finality_integrity() {
    let issuer = production_source(OFFLINE_ISSUER_SOURCE);

    for marker in [
        "offline_operation_reference_response",
        "offline_operation_status_uri",
        "find_terminal_offline_operation_by_id",
        "ensure_kagemusha_v2_terminal_finality_matches_record",
        "terminal_rejected_or_expired_offline_operation_status",
        "ensure_unproven_pending_window_is_live",
        "OfflineOperationStatus::Applied",
        "OfflineOperationResult::Redeem",
        "operation_id: operation_id_hex.clone()",
        "known_pending_in_queue",
        "offline_operation_index_inconsistent",
    ] {
        assert!(
            issuer.contains(marker),
            "missing typed operation-resource/finality marker: {marker}"
        );
    }
    assert!(issuer.contains("finality.finalized_block_height == 0"));
    assert!(issuer.contains("finality.server_time_ms == 0"));
    assert!(issuer.contains("finality.operation_id == [0; 32]"));
    assert!(issuer.contains("anchor_transaction_hash == [0; 32]"));
}
