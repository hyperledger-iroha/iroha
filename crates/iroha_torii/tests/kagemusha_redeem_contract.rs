//! Source-level contract guards for the sole KAGEMUSHA V1 redemption command.

const COMMANDS_SOURCE: &str = include_str!("../src/kagemusha_commands.rs");
const KAGEMUSHA_API_SOURCE: &str = include_str!("../../iroha_torii_shared/src/kagemusha_api.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

#[test]
fn redeem_accepts_only_the_typed_v1_terminal_voucher() {
    assert!(
        TORII_SOURCE.contains("REDEEM => limited_canonical_signed_post(handler_kagemusha_redeem")
    );
    assert!(TORII_SOURCE.contains("KagemushaRedemptionRequestV1"));
    assert!(COMMANDS_SOURCE.contains("RedeemKagemushaV1"));
    assert!(COMMANDS_SOURCE.contains("KagemushaOperationStatusV1"));
    assert!(COMMANDS_SOURCE.contains("request.validate_shape()"));
    assert!(KAGEMUSHA_API_SOURCE.contains("decode_kagemusha_redemption_request_v1"));
    assert_eq!(
        iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_SCHEMA_NAME_V1,
        "iroha.torii.v1.kagemusha.redeem.request"
    );

    let redemption_handler = COMMANDS_SOURCE
        .split_once("pub(crate) async fn handle_redeem")
        .expect("redemption handler")
        .1
        .split_once("pub(crate) fn handle_operation_status")
        .expect("operation status follows redemption")
        .0;
    assert!(redemption_handler.contains("accept: Option<crate::utils::extractors::ExtractAccept>"));
    assert!(redemption_handler.contains("TransactionAdmissionIntent::QueuePlanSynced"));
    assert!(redemption_handler.contains("submit_signed_transaction_for_ingress_strict_durable"));
    assert!(redemption_handler.contains("response.status() != StatusCode::ACCEPTED"));
    assert!(!redemption_handler.contains("routing::handle_transaction_with_metrics"));
}

#[test]
fn redemption_boundary_is_constant_size_and_has_no_lineage_decoder() {
    assert_eq!(
        iroha_torii_shared::kagemusha_api::KAGEMUSHA_REDEMPTION_REQUEST_MAX_BYTES_V1,
        8 * 1024
    );
    for retired in ["lineage", "top_up_anchor", "hop_count", "branch_path"] {
        assert!(!KAGEMUSHA_API_SOURCE.to_ascii_lowercase().contains(retired));
    }
}
