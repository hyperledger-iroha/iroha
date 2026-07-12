//! Source-level contract guards for the first-release offline top-up command.

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
fn top_up_is_a_typed_async_command_on_the_final_route() {
    let commands = production_source(KAGEMUSHA_COMMANDS_SOURCE);

    assert!(TORII_SOURCE.contains("&route_catalog::offline::TOP_UP"));
    assert!(TORII_SOURCE.contains("post(handler_offline_top_up)"));
    assert!(TORII_SOURCE.contains("offline_api::OfflineTopUpRequest"));
    assert!(TORII_SOURCE.contains("NoritoOnly(request)"));
    assert!(TORII_SOURCE.contains("norito_request_content_type(&headers)"));
    assert!(OFFLINE_API_SOURCE.contains("as OfflineTopUpRequest"));
    assert!(OFFLINE_API_SOURCE.contains("OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME"));
    assert_eq!(
        iroha_torii_shared::offline_api::OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME,
        "iroha.torii.v1.offline.top_up.request"
    );
    assert!(commands.contains("handle_top_up"));
    assert!(commands.contains("topup_request: OfflineTopUpRequest"));
    assert!(commands.contains("require_idempotency_key"));
    assert!(commands.contains("OfflineOperationReference"));
    assert!(commands.contains("StatusCode::ACCEPTED"));
    assert!(commands.contains("header::LOCATION"));
    assert!(commands.contains("header::RETRY_AFTER"));
    assert!(commands.contains("header::CACHE_CONTROL"));
}

#[test]
fn retries_use_bounded_recovery_and_confirmed_admission() {
    let commands = production_source(KAGEMUSHA_COMMANDS_SOURCE);

    assert!(commands.contains("find_pending_offline_operation_by_id"));
    assert!(commands.contains("get_earliest_block_height_by_offline_operation_id"));
    assert!(!commands.contains("while height > 0"));
    assert!(commands.contains("claim_submission"));
    assert!(commands.contains("wait_for_submission_outcome"));
    assert!(commands.contains("let record = submission.accept(tx_hash)"));
}
