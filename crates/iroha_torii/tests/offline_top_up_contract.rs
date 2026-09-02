//! Source-level contract guards for the sole Offline Cash V1 top-up command.

const COMMANDS_SOURCE: &str = include_str!("../src/offline_commands.rs");
const OFFLINE_API_SOURCE: &str = include_str!("../../iroha_torii_shared/src/offline_api.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

#[test]
fn top_up_is_one_typed_async_offline_cash_v1_command() {
    assert!(TORII_SOURCE.contains("&route_catalog::offline::TOP_UP"));
    assert!(TORII_SOURCE.contains("post(handler_offline_top_up)"));
    assert!(TORII_SOURCE.contains("OfflineCashTopUpRequestV1"));
    assert!(COMMANDS_SOURCE.contains("TopUpOfflineCashV1"));
    assert!(COMMANDS_SOURCE.contains("OfflineCashOperationStatusV1"));
    assert!(COMMANDS_SOURCE.contains("request.validate()"));
    assert!(COMMANDS_SOURCE.contains("StatusCode::ACCEPTED"));
    assert!(COMMANDS_SOURCE.contains("header::LOCATION"));
    assert!(COMMANDS_SOURCE.contains("header::RETRY_AFTER"));
    assert!(COMMANDS_SOURCE.contains("header::CACHE_CONTROL"));
    assert!(OFFLINE_API_SOURCE.contains("decode_offline_cash_top_up_request_v1"));
    assert_eq!(
        iroha_torii_shared::offline_api::OFFLINE_CASH_TOP_UP_REQUEST_SCHEMA_NAME_V1,
        "iroha.torii.v1.offline_cash.top_up.request"
    );
}

#[test]
fn top_up_has_no_history_dependent_resource_limit() {
    assert_eq!(
        iroha_torii_shared::offline_api::OFFLINE_CASH_TOP_UP_REQUEST_MAX_BYTES_V1,
        4 * 1024
    );
    for retired in ["max_hops", "max_inputs", "top_up_anchor", "note_inventory"] {
        assert!(!OFFLINE_API_SOURCE.contains(retired));
    }
}
