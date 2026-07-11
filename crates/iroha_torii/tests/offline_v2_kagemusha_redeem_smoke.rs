//! Source-level contract guards for the first-release offline redeem command.

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
fn redeem_is_a_typed_async_command_on_the_final_route() {
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
}

#[test]
fn redeem_has_no_wrapper_or_compatibility_payload() {
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
}

#[test]
fn retired_redeem_routes_are_not_mounted() {
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
}
