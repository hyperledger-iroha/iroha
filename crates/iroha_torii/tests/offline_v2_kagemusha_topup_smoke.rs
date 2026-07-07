//! Source-level smoke checks for the offline v2 Kagemusha top-up bridge.

const OFFLINE_V2_ISSUER_SOURCE: &str = include_str!("../src/offline_v2_issuer.rs");
const TORII_SOURCE: &str = include_str!("../src/lib.rs");

#[tokio::test]
async fn offline_v2_kagemusha_topup_is_mounted_as_canonical_route() {
    let offline_v2_issuer_production_source = OFFLINE_V2_ISSUER_SOURCE
        .split("\nmod tests")
        .next()
        .expect("offline v2 issuer production source");
    assert!(TORII_SOURCE.contains("/v1/offline/v2/kagemusha/topup"));
    assert!(TORII_SOURCE.contains("handler_offline_v2_kagemusha_topup"));
    assert!(offline_v2_issuer_production_source.contains("handle_kagemusha_topup"));
    assert!(offline_v2_issuer_production_source.contains("KagemushaRecursiveSpendTopUpRequestV1"));
    assert!(offline_v2_issuer_production_source.contains("TopUpKagemushaRecursive::new"));
    assert!(offline_v2_issuer_production_source.contains("topup_request_norito_base64"));
    assert!(offline_v2_issuer_production_source.contains("topup_init_request_norito_base64"));
    assert!(offline_v2_issuer_production_source.contains("OFFLINE_KAGEMUSHA_TOPUP_CHAIN_MISMATCH"));
    assert!(offline_v2_issuer_production_source.contains("OFFLINE_KAGEMUSHA_TOPUP_ASSET_MISMATCH"));
    assert!(
        offline_v2_issuer_production_source.contains("OFFLINE_KAGEMUSHA_TOPUP_ACCOUNT_MISMATCH")
    );
    assert!(offline_v2_issuer_production_source.contains("OFFLINE_KAGEMUSHA_TOPUP_RETIRED_FIELD"));
    assert!(!offline_v2_issuer_production_source.contains("KagemushaRecursiveSpendInitRequestV1"));
}

#[tokio::test]
async fn offline_v2_notes_issue_fails_closed_instead_of_constructing_classic_note() {
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_V2_ISSUE_RETIRED"));
    assert!(!OFFLINE_V2_ISSUER_SOURCE.contains("IssueOfflineNote::new"));
    assert!(!OFFLINE_V2_ISSUER_SOURCE.contains("OfflineNoteIssue {"));
}
