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
    assert!(commands.contains("resolve_pending_offline_operation_by_id"));
    assert!(commands.contains("pending_kagemusha_operation"));
    assert!(!commands.contains("all_transactions"));
    assert!(commands.contains("kagemusha_operation_outcome_v4"));
    assert!(commands.contains("get_merge_entry_by_carrier_height"));
    assert!(commands.contains("signed_transaction_wire_hash_v4"));
    assert!(commands.contains("pending.signed_transaction_wire_hash()"));
    assert!(!commands.contains("outcome.signed_transaction_hash"));
    assert!(!commands.contains("get_earliest_block_height_by_offline_operation_id"));
    assert!(!commands.contains("while height > 0"));
    assert!(commands.contains("claim_submission"));
    assert!(commands.contains("wait_for_submission_outcome"));
    assert!(commands.contains("let record = submission.accept(tx_hash)"));
    let resolver = commands
        .split("fn resolve_pending_offline_operation_by_id")
        .nth(1)
        .expect("typed pending resolver")
        .split("fn find_existing_offline_operation")
        .next()
        .expect("typed pending resolver body");
    assert!(
        resolver
            .find("pending_kagemusha_operation")
            .expect("Queue lookup")
            < resolver
                .find("find_terminal_offline_operation_by_id")
                .expect("terminal recheck after Queue lookup")
    );
    assert_eq!(
        resolver
            .matches("find_terminal_offline_operation_by_id")
            .count(),
        2,
        "Queue miss and transient unavailability must each recheck terminal consensus state"
    );
    assert!(resolver.contains("PendingKagemushaOperationLookupError::Inconsistent"));
}
#[test]
fn status_checks_consensus_outcome_before_process_local_hints() {
    let commands = production_source(KAGEMUSHA_COMMANDS_SOURCE);
    let status = commands
        .split("pub(crate) fn handle_operation_status")
        .nth(1)
        .expect("operation status handler")
        .split("struct KagemushaV2CommittedFinality")
        .next()
        .expect("operation status handler body");
    let outcome = status
        .find("find_terminal_offline_operation_by_id")
        .expect("terminal outcome preflight");
    let admission = status
        .find("issuer.admission.lock")
        .expect("process-local admission lookup");
    let pending = status
        .find("resolve_pending_offline_operation_by_id")
        .expect("typed Queue pending lookup");
    assert!(outcome < admission && outcome < pending);
}
