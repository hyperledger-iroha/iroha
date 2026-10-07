//! Exact native-capture finality driver; no synthetic funding or skipped source spans.

#[path = "common/finality_driver.rs"]
mod driver;

#[test]
fn exact_native_capture_has_the_complete_minimal_height_two_inputs() {
    driver::check_fixture();
}

#[test]
fn checkpoint_files_preserve_exact_originals_and_distinguish_absence() {
    driver::check_checkpoint_files();
}

#[test]
fn offline_driver_directory_is_exclusive_and_rejects_links() {
    driver::check_directory_custody();
}

#[test]
#[ignore = "explicit offline full8944-proof run; requires output directory and recorded source manifest"]
fn genuine_captured_ordinary_receipt_closes_every_source_and_history_owner() {
    let output = std::env::var_os("KAGEMUSHA_FINALITY_OUTPUT")
        .expect("select an exclusive driver-owned output directory");
    let manifest = std::env::var("KAGEMUSHA_FINALITY_SOURCE_SHA256")
        .expect("record the captured executable's canonical source manifest SHA256");
    driver::run(std::path::Path::new(&output), &manifest);
}

#[test]
#[ignore = "completed immutable first-Load capture required; derives verifier keys only, never PKs"]
fn completed_first_load_receipt_restores_without_reproving_finality() {
    use driver::restore::{CaptureIdentity, load_completed_receipt};
    fn pin(name: &str) -> [u8; 32] {
        let text = std::env::var(name).expect("independently retained capture pin");
        assert_eq!(text.len(), 64);
        core::array::from_fn(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
    }
    let root = std::env::var_os("KAGEMUSHA_FINALITY_OUTPUT").expect("completed capture output");
    let capture = include_str!("../../../fixtures/kagemusha/ordinary_first_load_receipt_v1.json");
    let restored = load_completed_receipt(
        std::path::Path::new(&root),
        capture,
        CaptureIdentity {
            producer: pin("KAGEMUSHA_FINALITY_PRODUCER_SHA256"),
            sources: pin("KAGEMUSHA_FINALITY_SOURCE_SHA256"),
            fixture: pin("KAGEMUSHA_FINALITY_FIXTURE_SHA256"),
            inventory: pin("KAGEMUSHA_FINALITY_INVENTORY_SHA256"),
        },
    )
    .unwrap()
    .expect("complete exact receipt checkpoint");
    assert_eq!(restored.finalized.anchor, *restored.verifier.anchor());
    assert_eq!(
        restored.finalized.source.binding(),
        restored.verifier.source().binding()
    );
    assert_eq!(restored.finalized.receipt[130..146], 0_u128.to_le_bytes());
    assert_eq!(restored.finalized.receipt[146..162], 100_u128.to_le_bytes());
    assert!(restored.verifier_reads > 0);
    eprintln!(
        "FINALITY_RESTORED complete_verifier_catalog=true exact_receipt=true both_claims=true verifier_reads={} pk_reads=0 new_proofs=0 component_only=true",
        restored.verifier_reads
    );
}
