//! Explicit long-running native-capture ordinary finality followed by genuine Load.
//! Synthetic ledger execution identities remain component-only; every finality
//! source/proof and every native Load stage is real and independently verified.

#![allow(clippy::duplicate_mod)] // The shared Bootstrap chain retains its exact source types.

#[path = "common/bootstrap.rs"]
mod bootstrap;
#[path = "common/bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
mod common;
#[path = "common/finality_driver.rs"]
#[allow(dead_code)]
mod driver;
/// The same native Load five-A/four-W producer used by subsequent chain fixtures.
#[path = "a_load_recursive.rs"]
pub mod load_chain;
#[path = "common/ordinary_load_fixture.rs"]
mod load_fixture;
#[path = "common/load_objects.rs"]
#[allow(dead_code)]
mod load_objects;

use ff::Field;
use iroha_pasta::Fp;
use std::{fs, path::PathBuf, sync::Arc};

const CAPTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/kagemusha/ordinary_first_load_receipt_v1.json"
);
fn first_capture() -> String {
    let metadata =
        fs::symlink_metadata(CAPTURE).expect("exact separately generated first-Load capture");
    assert!(metadata.file_type().is_file() && metadata.len() <= 1 << 20);
    fs::read_to_string(CAPTURE).unwrap()
}

#[test]
fn exact_first_load_capture_matches_bootstrap_identity_without_relabeling() {
    load_fixture::check_first_capture(&first_capture());
}

#[test]
#[ignore = "explicit resumable ordinary-finality8944-proof execution, then genuine5A4W Load"]
fn exact_first_load_finality_then_native_load() {
    let output = std::env::var_os("KAGEMUSHA_FINALITY_OUTPUT")
        .expect("select an exclusive driver-owned output directory");
    let manifest = std::env::var("KAGEMUSHA_FINALITY_SOURCE_SHA256")
        .expect("record the captured executable's canonical source manifest SHA256");
    let output = PathBuf::from(output);
    let capture = first_capture();
    driver::run_capture(&output, &manifest, &capture, |receipt| {
        eprintln!("FINALITY_PHASE genuine_Load_fixture");
        let rooted = load_chain::bootstrap_outer::compact_bootstrap::rooted_compact_bootstrap();
        let originals = output.join("load-originals");
        let fixture: load_chain::LoadFixture =
            Arc::new(move |rooted| load_fixture::build(rooted, &receipt, &originals));
        let terminal = load_chain::authenticated_load(&rooted, &fixture);
        assert_eq!(
            terminal.state.core[iroha_kagemusha_proof::witness::core_index::BALANCE],
            Fp::from(100)
        );
        assert_eq!(
            terminal.state.core[iroha_kagemusha_proof::witness::core_index::NEXT_LOAD],
            Fp::ONE
        );
        eprintln!(
            "ORDINARY_FIRST_LOAD complete_finality=true all5A4W=true exact_ordinal0_amount100_charge0=true synthetic_execution_capture=true Load_terminal_Omega=false full_wallet_catalog=false"
        );
    });
}
