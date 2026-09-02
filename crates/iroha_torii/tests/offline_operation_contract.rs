//! Wire-contract guards for the sole Offline Cash V1 operation resource.

use iroha_torii_shared::offline_api::{
    OFFLINE_CASH_CHAIN_VERSION_V1, OfflineCashOperationKindV1, OfflineCashOperationStateV1,
    OfflineCashOperationStatusV1,
};

const COMMANDS_SOURCE: &str = include_str!("../src/offline_commands.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

fn pending_status() -> OfflineCashOperationStatusV1 {
    OfflineCashOperationStatusV1 {
        version: OFFLINE_CASH_CHAIN_VERSION_V1,
        operation_id: [0x11; 32],
        kind: OfflineCashOperationKindV1::TopUp,
        state: OfflineCashOperationStateV1::Pending,
        result: None,
        rejection: None,
    }
}

#[test]
fn pending_status_has_direct_json_and_norito_representations() {
    let status = pending_status();
    status.validate().expect("valid pending status");
    let json = norito::json::to_vec(&status).expect("encode operation status as JSON");
    let decoded_json: OfflineCashOperationStatusV1 =
        norito::json::from_slice(&json).expect("decode operation status JSON");
    assert_eq!(decoded_json, status);
    let archive = norito::encode_canonical(&status).expect("encode operation status as Norito");
    let decoded_norito =
        iroha_torii_shared::offline_api::decode_unverified_offline_cash_operation_status_v1(
            &archive,
        )
        .expect("decode bounded operation status");
    assert_eq!(decoded_norito.operation_id(), status.operation_id);
    assert_eq!(decoded_norito.state(), OfflineCashOperationStateV1::Pending);
}

#[test]
fn operation_status_is_the_only_pollable_v1_resource() {
    assert!(TORII_SOURCE.contains("&route_catalog::offline::OPERATION"));
    assert!(TORII_SOURCE.contains("handler_offline_operation_status"));
    assert!(COMMANDS_SOURCE.contains("handle_operation_status"));
    assert!(COMMANDS_SOURCE.contains("OfflineCashOperationStatusV1"));
    assert!(COMMANDS_SOURCE.contains("header::CACHE_CONTROL"));
}
