//! Wire-contract guards for the first-release offline operation resource.

use iroha_torii_shared::offline_api::{
    OfflineOperationKind, OfflineOperationReference, OfflineOperationState,
};

const KAGEMUSHA_COMMANDS_SOURCE: &str = include_str!("../src/offline_commands.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

fn operation_reference() -> OfflineOperationReference {
    OfflineOperationReference {
        operation_id: "11".repeat(32),
        kind: OfflineOperationKind::TopUp,
        state: OfflineOperationState::Pending,
        transaction_hash: "22".repeat(32),
        status_uri: format!("/v1/offline/operations/{}", "11".repeat(32)),
        submitted_at_ms: 1_725_000_000_123,
    }
}

fn assert_operation_reference(decoded: &OfflineOperationReference) {
    assert_eq!(decoded.operation_id, "11".repeat(32));
    assert_eq!(decoded.kind, OfflineOperationKind::TopUp);
    assert_eq!(decoded.state, OfflineOperationState::Pending);
    assert_eq!(decoded.transaction_hash, "22".repeat(32));
    assert_eq!(
        decoded.status_uri,
        format!("/v1/offline/operations/{}", "11".repeat(32))
    );
    assert_eq!(decoded.submitted_at_ms, 1_725_000_000_123);
}

#[test]
fn operation_reference_has_direct_json_and_norito_representations() {
    let reference = operation_reference();

    let json = norito::json::to_vec(&reference).expect("encode operation reference as JSON");
    let json_text = std::str::from_utf8(&json).expect("JSON is UTF-8");
    assert!(!json_text.contains("base64"));
    let decoded_json: OfflineOperationReference =
        norito::json::from_slice(&json).expect("decode operation reference JSON");
    assert_operation_reference(&decoded_json);

    let archive = norito::to_bytes(&reference).expect("encode operation reference as Norito");
    let decoded_norito: OfflineOperationReference =
        norito::decode_from_bytes(&archive).expect("decode operation reference Norito");
    assert_operation_reference(&decoded_norito);
}

#[test]
fn operation_status_is_a_pollable_final_route() {
    assert!(TORII_SOURCE.contains("&route_catalog::offline::OPERATION"));
    assert!(TORII_SOURCE.contains("catalog_get(handler_offline_operation_status)"));
    assert!(KAGEMUSHA_COMMANDS_SOURCE.contains("handle_operation_status"));
    assert!(KAGEMUSHA_COMMANDS_SOURCE.contains("OfflineOperationStatus::Pending"));
    assert!(KAGEMUSHA_COMMANDS_SOURCE.contains("OfflineOperationStatus::Applied"));
    assert!(KAGEMUSHA_COMMANDS_SOURCE.contains("OfflineOperationStatus::Rejected"));
    assert!(KAGEMUSHA_COMMANDS_SOURCE.contains("header::CACHE_CONTROL"));
}
