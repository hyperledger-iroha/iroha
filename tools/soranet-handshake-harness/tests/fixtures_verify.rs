#[path = "interop_parity.rs"]
mod interop_parity;
#[path = "perf_gate.rs"]
mod perf_gate;
#[path = "simulate_cli.rs"]
mod simulate_cli;
use soranet_handshake_harness::verify_fixtures;
use std::path::PathBuf;
fn serial_guard() -> std::sync::MutexGuard<'static, ()> {
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
    SERIAL
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
fn workspace_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .expect("crate is nested under tools")
        .parent()
        .expect("workspace root should exist")
        .to_path_buf()
}
#[test]
fn canonical_fixtures_match_generator_output() {
    let _serial = crate::serial_guard();
    let root = workspace_root();
    let bundles = [
        root.join("tests/interop/soranet/capabilities"),
        root.join("fixtures/soranet_handshake/capabilities"),
    ];
    for bundle in bundles {
        assert!(
            bundle.exists(),
            "expected fixture bundle {} to exist",
            bundle.display()
        );
        verify_fixtures(&bundle).unwrap_or_else(|err| {
            panic!(
                "fixture verification failed for {}: {err}",
                bundle.display()
            );
        });
    }
}
