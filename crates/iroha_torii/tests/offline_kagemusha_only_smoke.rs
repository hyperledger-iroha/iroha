//! Source-level guards for the Kagemusha-only offline payment surface.

const OFFLINE_ISSUER_SOURCE: &str = include_str!("../src/offline_issuer.rs");
const OFFLINE_V2_ISSUER_SOURCE: &str = include_str!("../src/offline_v2_issuer.rs");

#[test]
fn torii_classic_offline_payment_handlers_are_retired() {
    assert!(OFFLINE_ISSUER_SOURCE.contains("OFFLINE_NOTE_ISSUE_RETIRED"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_V2_NOTE_ISSUE_RETIRED"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_V2_REDEEM_RETIRED"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("OFFLINE_V2_AUDIT_RETIRED"));

    for retired_symbol in [
        "IssueOfflineNote,",
        "IssueOfflineNoteV2",
        "RedeemOfflineNoteV2",
        "AuditOfflineNoteV2",
        "IssueOfflineNote::new",
        "IssueOfflineNoteV2::new",
        "RedeemOfflineNoteV2::new",
        "AuditOfflineNoteV2::new",
    ] {
        assert!(
            !OFFLINE_ISSUER_SOURCE.contains(retired_symbol)
                && !OFFLINE_V2_ISSUER_SOURCE.contains(retired_symbol),
            "Torii offline issuer must not expose retired classic payment symbol {retired_symbol}"
        );
    }
}

#[test]
fn torii_keeps_kagemusha_recursive_redeem_path() {
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("RedeemKagemushaRecursive"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("new_with_lineage_witness_and_change"));
    assert!(OFFLINE_V2_ISSUER_SOURCE.contains("handle_kagemusha_recursive_notes_redeem"));
}
