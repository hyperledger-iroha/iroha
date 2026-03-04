use std::path::PathBuf;

use soranet_handshake_harness::verify_fixtures;

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
