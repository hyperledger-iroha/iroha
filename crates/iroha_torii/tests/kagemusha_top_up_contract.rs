//! Source-level contract guards for the sole Kagemusha V1 top-up command.

const COMMANDS_SOURCE: &str = include_str!("../src/kagemusha_commands.rs");
const KAGEMUSHA_API_SOURCE: &str = include_str!("../../iroha_torii_shared/src/kagemusha_api.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

#[test]
fn top_up_is_one_typed_async_kagemusha_v1_command() {
    assert!(TORII_SOURCE.contains("&route_catalog::kagemusha::TOP_UP"));
    assert!(TORII_SOURCE.contains("post(handler_kagemusha_top_up)"));
    assert!(TORII_SOURCE.contains("KagemushaTopUpRequestV1"));
    assert!(COMMANDS_SOURCE.contains("TopUpKagemushaV1"));
    assert!(COMMANDS_SOURCE.contains("KagemushaOperationStatusV1"));
    assert!(COMMANDS_SOURCE.contains("request.validate()"));
    assert!(COMMANDS_SOURCE.contains("StatusCode::ACCEPTED"));
    assert!(COMMANDS_SOURCE.contains("header::LOCATION"));
    assert!(COMMANDS_SOURCE.contains("header::RETRY_AFTER"));
    assert!(COMMANDS_SOURCE.contains("header::CACHE_CONTROL"));
    assert!(KAGEMUSHA_API_SOURCE.contains("decode_kagemusha_top_up_request_v1"));
    assert_eq!(
        iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_SCHEMA_NAME_V1,
        "iroha.torii.v1.kagemusha.top_up.request"
    );
}

#[test]
fn top_up_has_no_history_dependent_resource_limit() {
    assert_eq!(
        iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1,
        4 * 1024
    );
    for retired in ["max_hops", "max_inputs", "top_up_anchor", "note_inventory"] {
        assert!(!KAGEMUSHA_API_SOURCE.contains(retired));
    }
}
