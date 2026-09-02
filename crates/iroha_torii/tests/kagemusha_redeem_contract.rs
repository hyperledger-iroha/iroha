//! Source-level contract guards for the sole Kagemusha V1 redemption command.

const COMMANDS_SOURCE: &str = include_str!("../src/kagemusha_commands.rs");
const KAGEMUSHA_API_SOURCE: &str = include_str!("../../iroha_torii_shared/src/kagemusha_api.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

#[test]
fn redeem_accepts_only_the_typed_v1_terminal_voucher() {
    assert!(TORII_SOURCE.contains("&route_catalog::kagemusha::REDEEM"));
    assert!(TORII_SOURCE.contains("handler_kagemusha_redeem"));
    assert!(TORII_SOURCE.contains("KagemushaRedemptionRequestV1"));
    assert!(COMMANDS_SOURCE.contains("RedeemKagemushaV1"));
    assert!(COMMANDS_SOURCE.contains("KagemushaOperationStatusV1"));
    assert!(COMMANDS_SOURCE.contains("request.validate()"));
    assert!(KAGEMUSHA_API_SOURCE.contains("decode_kagemusha_redemption_request_v1"));
    assert_eq!(
        iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_SCHEMA_NAME_V1,
        "iroha.torii.v1.kagemusha.redeem.request"
    );
}

#[test]
fn redemption_boundary_is_constant_size_and_has_no_lineage_decoder() {
    assert_eq!(
        iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1,
        8 * 1024
    );
    for retired in ["lineage", "anchor", "hop_count", "branch_path"] {
        assert!(!KAGEMUSHA_API_SOURCE.to_ascii_lowercase().contains(retired));
    }
}
