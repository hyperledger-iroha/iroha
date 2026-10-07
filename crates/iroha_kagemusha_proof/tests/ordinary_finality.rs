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

fn environment_pin(name: &str) -> [u8; 32] {
    let text = std::env::var(name).expect("independently retained SHA256 pin");
    assert_eq!(text.len(), 64);
    core::array::from_fn(|index| u8::from_str_radix(&text[2 * index..2 * index + 2], 16).unwrap())
}

#[test]
#[ignore = "independently pinned canonical executed setup; validates native H1/H2 without keygen"]
fn canonical_executed_setup_admits_only_exact_source_inputs() {
    let input =
        std::env::var_os("KAGEMUSHA_EXECUTED_LEDGER_SETUP").expect("pinned setup directory");
    driver::source_only::check(
        std::path::Path::new(&input),
        environment_pin("KAGEMUSHA_EXECUTED_LEDGER_SETUP_SHA256"),
    );
}

#[test]
#[ignore = "explicit complete source compilation from canonical executed setup; no Load or proof campaign"]
fn canonical_executed_setup_compiles_complete_finality_sources_without_load() {
    let input =
        std::env::var_os("KAGEMUSHA_EXECUTED_LEDGER_SETUP").expect("pinned setup directory");
    let output = std::env::var_os("KAGEMUSHA_FINALITY_OUTPUT").expect("fresh exclusive output");
    driver::source_only::run(
        std::path::Path::new(&output),
        std::path::Path::new(&input),
        environment_pin("KAGEMUSHA_EXECUTED_LEDGER_SETUP_SHA256"),
        environment_pin("KAGEMUSHA_FINALITY_SOURCE_SHA256"),
    );
}

fn executed_load_selection<'a>(
    paths: &'a [std::path::PathBuf; 3],
) -> driver::executed_load::Selection<'a> {
    driver::executed_load::Selection {
        setup_root: &paths[0],
        setup_sha256: environment_pin("KAGEMUSHA_EXECUTED_LEDGER_SETUP_SHA256"),
        target: &paths[1],
        target_sha256: environment_pin("KAGEMUSHA_NATIVE_LOAD_TARGET_SHA256"),
        capture: &paths[2],
        capture_sha256: environment_pin("KAGEMUSHA_EXECUTED_LOAD_CAPTURE_SHA256"),
        receipt_sha256: environment_pin("KAGEMUSHA_NATIVE_LOAD_RECEIPT_SHA256"),
    }
}
fn executed_load_paths() -> [std::path::PathBuf; 3] {
    [
        "KAGEMUSHA_EXECUTED_LEDGER_SETUP",
        "KAGEMUSHA_NATIVE_LOAD_TARGET",
        "KAGEMUSHA_EXECUTED_LOAD_CAPTURE",
    ]
    .map(|name| {
        std::env::var_os(name)
            .map(std::path::PathBuf::from)
            .expect("explicit independently pinned executed input")
    })
}

#[test]
#[ignore = "requires genuine native A target and actual ledger H1..H5 originals; no keygen or proofs"]
fn executed_load_intake_authenticates_ordered_history_and_counted_event() {
    let paths = executed_load_paths();
    driver::executed_load::check(&executed_load_selection(&paths));
}

#[test]
#[ignore = "full exact H2..H5 recursive finality campaign; completed canonical sources and executed A Load required"]
fn executed_load_proves_complete_history_and_exports_exact_receipt() {
    let paths = executed_load_paths();
    let output =
        std::env::var_os("KAGEMUSHA_FINALITY_OUTPUT").expect("exclusive proof/checkpoint output");
    let sources = std::env::var_os("KAGEMUSHA_CANONICAL_FINALITY_SOURCES")
        .expect("completed source-only directory");
    driver::executed_load::run(
        std::path::Path::new(&output),
        &executed_load_selection(&paths),
        &driver::executed_load::SourceSelection {
            root: std::path::Path::new(&sources),
            completion_sha256: environment_pin("KAGEMUSHA_CANONICAL_FINALITY_COMPLETION_SHA256"),
        },
        environment_pin("KAGEMUSHA_FINALITY_SOURCE_SHA256"),
    );
}
