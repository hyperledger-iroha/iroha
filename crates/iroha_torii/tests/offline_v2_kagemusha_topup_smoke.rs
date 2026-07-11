//! Source-level contract guards for the first-release offline top-up command.

const OFFLINE_ISSUER_SOURCE: &str = include_str!("../src/offline_v2_issuer.rs");
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
    let issuer = production_source(OFFLINE_ISSUER_SOURCE);

    assert!(TORII_SOURCE.contains("uri::OFFLINE_TOP_UP, post(handler_offline_top_up)"));
    assert!(TORII_SOURCE.contains("offline_api::OfflineTopUpRequest"));
    assert!(TORII_SOURCE.contains("NoritoJson(request)"));
    assert!(OFFLINE_API_SOURCE.contains("as OfflineTopUpRequest"));
    assert!(OFFLINE_API_SOURCE.contains("OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME"));
    assert_eq!(
        iroha_torii_shared::offline_api::OFFLINE_TOP_UP_REQUEST_SCHEMA_NAME,
        "iroha.torii.v1.offline.top_up.request"
    );
    assert!(issuer.contains("handle_kagemusha_topup"));
    assert!(issuer.contains("topup_request: OfflineTopUpRequest"));
    assert!(issuer.contains("require_idempotency_key"));
    assert!(issuer.contains("OfflineOperationReference"));
    assert!(issuer.contains("StatusCode::ACCEPTED"));
    assert!(issuer.contains("header::LOCATION"));
    assert!(issuer.contains("header::RETRY_AFTER"));
    assert!(issuer.contains("header::CACHE_CONTROL"));
}

#[test]
fn top_up_has_no_wrapper_or_compatibility_payload() {
    let issuer = production_source(OFFLINE_ISSUER_SOURCE);

    for retired_field in [
        "topup_request_norito_base64",
        "topup_init_request_norito_base64",
    ] {
        assert!(
            !issuer.contains(retired_field),
            "whole-payload wrapper must be absent: {retired_field}"
        );
    }
    assert!(!issuer.contains("KagemushaRecursiveSpendTopUpRequestV1"));
    assert!(!issuer.contains("KagemushaRecursiveSpendInitRequestV1"));
}

#[test]
fn retired_top_up_routes_are_not_mounted() {
    for retired_path in [
        "/v1/offline/v2/kagemusha/topup",
        "/v1/offline/v2/top-up",
        "/v1/offline/cash/load",
        "/v1/offline/notes/issue",
    ] {
        assert!(
            !TORII_SOURCE.contains(&format!(".route(\"{retired_path}\"")),
            "retired route must not be mounted: {retired_path}"
        );
    }
}
