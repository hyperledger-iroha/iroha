//! Genuine four-stage native Load wrapped by the complete Omega predicate.
//! Bootstrap-only or two-terminal catalogs remain explicit component scopes;
//! neither establishes the complete operation catalog or production byte gate.
//! Run each ignored test separately with a fresh `KAGEMUSHA_LOAD_OUTPUT` directory.

#![allow(clippy::duplicate_mod)] // Each chain retains its exact shared Bootstrap source types.

#[path = "common/bootstrap.rs"]
mod bootstrap;
#[path = "common/bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
mod common;
#[path = "common/ordinary_load_fixture.rs"]
mod load_fixture;
#[path = "common/load_objects.rs"]
#[allow(dead_code)]
mod load_objects;
#[path = "common/ordinary_load_receipt.rs"]
mod ordinary_load_receipt;

/// Shared genuine Bootstrap predecessor and four-stage Load continuation fixtures.
#[path = "common/proof_fixtures/a_load_recursive.rs"]
pub mod load_chain;

include!("common/proof_fixtures/load_omega_body.rs");

fn first_load_fixture(
    expected_rebuilds: usize,
) -> (LoadFixture, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
    use std::{
        fs,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    let capture = ordinary_load_receipt::first_capture();
    // Bind scheme, asset, wallet, ordinal and the actual local sigma relation to
    // the Bootstrap fixture before producing any key or proof.
    load_fixture::check_first_capture(&capture);
    let receipt = ordinary_load_receipt::verified_receipt(&capture);
    let output = PathBuf::from(
        std::env::var_os("KAGEMUSHA_LOAD_OUTPUT")
            .expect("fresh exclusive Load Omega original output directory"),
    );
    fs::create_dir(&output).expect("fresh output; no overwrite or implicit resume");
    let rebuilds = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&rebuilds);
    let fixture: LoadFixture = Arc::new(move |rooted| {
        let index = observed.fetch_add(1, Ordering::Relaxed);
        assert!(
            index < expected_rebuilds,
            "unexpected predecessor reconstruction"
        );
        let directory = output.join(format!("predecessor-{index}"));
        fs::create_dir(&directory).expect("fresh originals for each selected predecessor");
        load_fixture::build(rooted, &receipt, &directory)
    });
    (fixture, rebuilds)
}

#[test]
#[ignore = "explicit genuine BLS-admitted Load and diagnostic outer proof; not full52 or carried-key continuity"]
fn native_load_reaches_complete_outer_predicate_from_bls_receipt() {
    let (fixture, rebuilds) = first_load_fixture(1);
    native_load_reaches_complete_outer_predicate(&fixture);
    assert_eq!(rebuilds.load(std::sync::atomic::Ordering::Relaxed), 1);
}

#[test]
#[ignore = "explicit genuine BLS-admitted Bootstrap/Load two-terminal rekey and outer proofs; not full52"]
fn common_bootstrap_load_catalog_rebinds_every_proof_and_key_from_bls_receipt() {
    let (fixture, rebuilds) = first_load_fixture(2);
    common_bootstrap_load_catalog_rebinds_every_proof_and_key(&fixture);
    assert_eq!(rebuilds.load(std::sync::atomic::Ordering::Relaxed), 2);
}
