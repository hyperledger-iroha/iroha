// Repository-fixture coverage for deterministic bundled rANS tables.

#[test]
fn load_bundle_tables_accepts_repo_fixture() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../codec/rans/tables/rans_seed0.toml");
    let tables = load_bundle_tables_from_toml(&path)
        .unwrap_or_else(|err| panic!("failed to load {}: {err}", path.display()));
    assert!(
        tables.max_width() >= 2,
        "expected deterministic tables fixture to expose bundled widths"
    );
}
