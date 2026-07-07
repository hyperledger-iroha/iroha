//! Source-level guards for the Kagemusha-first offline payment surface.

const OFFLINE_ISSUER_SOURCE: &str = include_str!("../src/offline_issuer.rs");
const OFFLINE_V2_ISSUER_SOURCE: &str = include_str!("../src/offline_v2_issuer.rs");

#[test]
fn torii_legacy_offline_payment_handlers_are_retired() {
    assert!(OFFLINE_ISSUER_SOURCE.contains("OFFLINE_NOTE_ISSUE_RETIRED"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_V2_REDEEM_RETIRED"));

    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_V2_ISSUE_RETIRED"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("handle_kagemusha_topup"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("TopUpKagemushaRecursive::new"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("topup_request_norito_base64"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("accepted_audit_receipt_ids"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("accepted_receipt_ids"));

    for v1_retired_symbol in ["IssueOfflineNote,", "IssueOfflineNote::new"] {
        assert!(
            !OFFLINE_ISSUER_SOURCE.contains(v1_retired_symbol),
            "Torii v1 offline issuer must not expose retired payment symbol {v1_retired_symbol}"
        );
    }

    for retired_symbol in [
        "IssueOfflineNote::new",
        "IssueOfflineNoteV2",
        "RedeemOfflineNoteV2",
        "AuditOfflineNoteV2",
        "IssueOfflineNoteV2::new",
        "RedeemOfflineNoteV2::new",
        "AuditOfflineNoteV2::new",
    ] {
        assert!(
            !OFFLINE_V2_ISSUER_SOURCE.contains(retired_symbol),
            "Torii v2 offline issuer must not expose retired classic payment symbol {retired_symbol}"
        );
    }
}

#[test]
fn torii_keeps_kagemusha_recursive_redeem_path() {
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("RedeemKagemushaRecursive"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("new_with_lineage_witness_and_change"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("handle_kagemusha_recursive_notes_redeem"));
}
