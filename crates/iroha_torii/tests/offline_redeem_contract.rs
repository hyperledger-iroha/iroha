//! Source-level closure guards for the first-release typed offline redeem command.
const KAGEMUSHA_COMMANDS_SOURCE: &str = include_str!("../src/offline_commands.rs");
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
    let commands = production_source(KAGEMUSHA_COMMANDS_SOURCE);
    assert!(TORII_SOURCE.contains("&route_catalog::offline::REDEEM"));
    assert!(TORII_SOURCE.contains("catalog_post(handler_offline_redeem)"));
    assert!(TORII_SOURCE.contains("offline_api::OfflineRedeemRequest"));
    assert!(TORII_SOURCE.contains("NoritoOnly(request)"));
    assert!(TORII_SOURCE.contains("norito_request_content_type(&headers)"));
    assert!(OFFLINE_API_SOURCE.contains("as OfflineRedeemRequest"));
    assert!(OFFLINE_API_SOURCE.contains("OFFLINE_REDEEM_REQUEST_SCHEMA_NAME"));
    assert_eq!(
        iroha_torii_shared::offline_api::OFFLINE_REDEEM_REQUEST_SCHEMA_NAME,
        "iroha.torii.v1.offline.redeem.request"
    );
    assert!(commands.contains("handle_redeem"));
    assert!(commands.contains("redeem_request: OfflineRedeemRequest"));
    assert!(commands.contains("require_idempotency_key"));
    assert!(commands.contains("OfflineOperationReference"));
    assert!(commands.contains("StatusCode::ACCEPTED"));
    assert!(commands.contains("header::LOCATION"));
    assert!(commands.contains("header::RETRY_AFTER"));
    assert!(commands.contains("header::CACHE_CONTROL"));
}
#[test]
fn offline_operation_polling_preserves_redeem_identity_and_finality_integrity() {
    let commands = production_source(KAGEMUSHA_COMMANDS_SOURCE);
    for marker in [
        "offline_operation_reference_response",
        "offline_operation_status_uri",
        "find_terminal_offline_operation_by_id",
        "ensure_kagemusha_v4_terminal_finality_matches_record",
        "terminal_rejected_or_expired_offline_operation_status",
        "ensure_unproven_pending_window_is_live",
        "OfflineOperationStatus::Applied",
        "OfflineOperationResult::Redeem",
        "identity: record.binding().identity()",
        "known_pending_in_queue",
        "offline_operation_evidence_inconsistent",
    ] {
        assert!(
            commands.contains(marker),
            "missing typed operation-resource/finality marker: {marker}"
        );
    }
    assert!(commands.contains("finality.finalized_block_height == 0"));
    assert!(commands.contains("finality.operation_id == [0; 32]"));
    assert!(commands.contains("anchor_transaction_hash == [0; 32]"));
}
