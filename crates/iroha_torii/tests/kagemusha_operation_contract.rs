//! Wire-contract guards for the sole KAGEMUSHA V1 operation resource.

use iroha_torii_shared::kagemusha_api::{
    KAGEMUSHA_CHAIN_VERSION_V1, KagemushaOperationKindV1, KagemushaOperationStateV1,
    KagemushaOperationStatusV1,
};
use norito::json::Value;

const COMMANDS_SOURCE: &str = include_str!("../src/kagemusha_commands.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

fn pending_status() -> KagemushaOperationStatusV1 {
    KagemushaOperationStatusV1 {
        version: KAGEMUSHA_CHAIN_VERSION_V1,
        operation_id: [0x11; 32],
        kind: KagemushaOperationKindV1::TopUp,
        state: KagemushaOperationStateV1::Pending,
        result: None,
        rejection: None,
    }
}

#[test]
fn pending_status_has_direct_json_and_norito_representations() {
    let status = pending_status();
    status.validate().expect("valid pending status");
    let json = norito::json::to_vec(&status).expect("encode operation status as JSON");
    let decoded_json: KagemushaOperationStatusV1 =
        norito::json::from_slice(&json).expect("decode operation status JSON");
    assert_eq!(decoded_json, status);
    let archive = norito::encode_canonical(&status).expect("encode operation status as Norito");
    let decoded_norito =
        iroha_torii_shared::kagemusha_api::decode_unverified_kagemusha_operation_status_v1(
            &archive,
        )
        .expect("decode bounded operation status");
    assert_eq!(decoded_norito.operation_id(), status.operation_id);
    assert_eq!(decoded_norito.state(), KagemushaOperationStateV1::Pending);
}

#[test]
fn operation_status_is_the_only_pollable_v1_resource() {
    assert!(TORII_SOURCE.contains("OPERATION => public_get(handler_kagemusha_operation_status)"));
    assert!(COMMANDS_SOURCE.contains("handle_operation_status"));
    assert!(COMMANDS_SOURCE.contains("KagemushaOperationStatusV1"));
    assert!(COMMANDS_SOURCE.contains("header::CACHE_CONTROL"));
}

#[test]
fn openapi_exposes_only_four_generic_routes_and_exact_operation_ids() {
    const OPERATION_ID_PATTERN: &str = "^(?!0{64}$)[0-9a-f]{64}$";
    const LOCATION_PATTERN: &str = "^/v1/kagemusha/operations/(?!0{64}$)[0-9a-f]{64}$";

    let document = iroha_torii::openapi::generate_spec();
    let paths = document
        .get("paths")
        .and_then(Value::as_object)
        .expect("OpenAPI paths");
    let mut kagemusha_paths = paths
        .keys()
        .filter(|path| path.starts_with("/v1/kagemusha"))
        .map(String::as_str)
        .collect::<Vec<_>>();
    kagemusha_paths.sort_unstable();
    assert_eq!(
        kagemusha_paths,
        [
            "/v1/kagemusha/operations/{operation_id}",
            "/v1/kagemusha/readiness",
            "/v1/kagemusha/redeem",
            "/v1/kagemusha/top-up",
        ]
    );
    let retired_product = ["line", "off"].into_iter().rev().collect::<String>();
    let retired_prefix = format!("/v1/{retired_product}");
    assert!(paths.keys().all(|path| !path.starts_with(&retired_prefix)));
    for retired in ["lifecycle", "anchor", "lineage"] {
        assert!(kagemusha_paths.iter().all(|path| !path.contains(retired)));
    }

    for path in ["/v1/kagemusha/top-up", "/v1/kagemusha/redeem"] {
        let operation = &paths[path]["post"];
        assert_eq!(
            operation["parameters"][0]["schema"]["pattern"].as_str(),
            Some(OPERATION_ID_PATTERN)
        );
        assert_eq!(
            operation["responses"]["202"]["headers"]["Location"]["schema"]["pattern"].as_str(),
            Some(LOCATION_PATTERN)
        );
    }
    assert_eq!(
        paths["/v1/kagemusha/operations/{operation_id}"]["get"]["parameters"][0]["schema"]["pattern"]
            .as_str(),
        Some(OPERATION_ID_PATTERN)
    );
}
