//! Source-level contract guards for the sole Offline Cash V1 redemption command.

const COMMANDS_SOURCE: &str = include_str!("../src/offline_commands.rs");
const OFFLINE_API_SOURCE: &str = include_str!("../../iroha_torii_shared/src/offline_api.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

#[test]
fn redeem_accepts_only_the_typed_v1_terminal_voucher() {
    assert!(TORII_SOURCE.contains("&route_catalog::offline::REDEEM"));
    assert!(TORII_SOURCE.contains("handler_offline_redeem"));
    assert!(TORII_SOURCE.contains("OfflineCashRedemptionRequestV1"));
    assert!(COMMANDS_SOURCE.contains("RedeemOfflineCashV1"));
    assert!(COMMANDS_SOURCE.contains("OfflineCashOperationStatusV1"));
    assert!(COMMANDS_SOURCE.contains("request.validate()"));
    assert!(OFFLINE_API_SOURCE.contains("decode_offline_cash_redemption_request_v1"));
    assert_eq!(
        iroha_torii_shared::offline_api::OFFLINE_CASH_REDEMPTION_REQUEST_SCHEMA_NAME_V1,
        "iroha.torii.v1.offline_cash.redeem.request"
    );
}

#[test]
fn redemption_boundary_is_constant_size_and_has_no_lineage_decoder() {
    assert_eq!(
        iroha_torii_shared::offline_api::OFFLINE_CASH_REDEMPTION_REQUEST_MAX_BYTES_V1,
        8 * 1024
    );
    for retired in ["lineage", "anchor", "hop_count", "branch_path"] {
        assert!(!OFFLINE_API_SOURCE.to_ascii_lowercase().contains(retired));
    }
}
