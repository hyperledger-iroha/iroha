//! Source-level contract guards for the sole KAGEMUSHA V1 top-up command.

const COMMANDS_SOURCE: &str = include_str!("../src/kagemusha_commands.rs");
const KAGEMUSHA_API_SOURCE: &str = include_str!("../../iroha_torii_shared/src/kagemusha_api.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

#[test]
fn top_up_is_one_typed_async_kagemusha_v1_command() {
    assert!(
        TORII_SOURCE.contains("TOP_UP => limited_canonical_signed_post(handler_kagemusha_top_up")
    );
    assert!(TORII_SOURCE.contains("extractors::NoritoBytes(body)"));
    assert!(COMMANDS_SOURCE.contains("decode_top_up_signed_transaction_v1(&body)"));
    assert!(COMMANDS_SOURCE.contains("SignedTransaction"));
    assert!(COMMANDS_SOURCE.contains("KagemushaOperationStatusV1"));
    assert!(KAGEMUSHA_API_SOURCE.contains("validate_kagemusha_top_up_signed_transaction_v1"));
    assert!(KAGEMUSHA_API_SOURCE.contains("instructions.len() != 1"));
    assert!(KAGEMUSHA_API_SOURCE.contains("downcast_ref::<TopUpKagemushaV1>()"));
    assert!(KAGEMUSHA_API_SOURCE.contains("transaction.authority() != &request.payer"));
    assert!(KAGEMUSHA_API_SOURCE.contains("&request.network_id != expected_network"));
    assert!(COMMANDS_SOURCE.contains("submit_signed_transaction_for_ingress_strict_durable"));
    assert!(COMMANDS_SOURCE.contains("StatusCode::ACCEPTED"));
    assert!(COMMANDS_SOURCE.contains("header::LOCATION"));
    assert!(COMMANDS_SOURCE.contains("header::RETRY_AFTER"));
    assert!(COMMANDS_SOURCE.contains("header::CACHE_CONTROL"));
    assert_eq!(
        iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_SIGNED_TRANSACTION_SCHEMA_NAME_V1,
        "iroha.torii.v1.kagemusha.top_up.signed_transaction"
    );

    let top_up_handler = COMMANDS_SOURCE
        .split_once("pub(crate) async fn handle_top_up")
        .expect("top-up handler")
        .1
        .split_once("pub(crate) async fn handle_redeem")
        .expect("redemption follows top-up")
        .0;
    let validated = top_up_handler
        .find("validate_top_up_signed_transaction")
        .expect("signed top-up validation");
    let reserved = top_up_handler
        .find("runtime.claim(binding)")
        .expect("top-up reservation");
    let recovered = top_up_handler
        .find("admitted_operation_from_consensus")
        .expect("durable operation recovery");
    let live_snapshot = top_up_handler
        .find("validate_top_up_snapshot")
        .expect("live admission snapshot validation");
    assert!(
        validated < reserved,
        "invalid envelopes must not reserve an operation"
    );
    assert!(
        recovered < live_snapshot,
        "durable exact replay must resolve before mutable admission policy"
    );
    assert!(top_up_handler.contains("top_up_transaction_hash: Some(transaction_hash)"));
    assert!(!top_up_handler.contains("quote_and_sign_transaction"));
    assert!(!top_up_handler.contains("ensure_kagemusha_command_authority_ready"));
    assert!(!top_up_handler.contains("TransactionBuilder::new"));
    assert!(!COMMANDS_SOURCE.contains("kagemusha_top_up_payer_mismatch"));
}

#[test]
fn top_up_has_no_history_dependent_resource_limit() {
    assert_eq!(
        iroha_torii_shared::kagemusha_api::KAGEMUSHA_TOP_UP_REQUEST_MAX_BYTES_V1,
        16 * 1024
    );
    assert!(
        TORII_SOURCE.contains(
            "const fn kagemusha_top_up_body_limit(transaction_max_content_len: usize) -> usize {\n    transaction_max_content_len"
        )
    );
    for retired in ["max_hops", "max_inputs", "top_up_anchor", "note_inventory"] {
        assert!(!KAGEMUSHA_API_SOURCE.contains(retired));
    }
}
