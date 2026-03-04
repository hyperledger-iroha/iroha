use std::{io::Write as _, path::PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use tempfile::NamedTempFile;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn iso_bridge_lint_defaults_pass() {
    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.arg("iso-bridge-lint");
    cmd.assert().success();
}

#[test]
fn iso_bridge_lint_rejects_unknown_instrument_fixture() {
    let mut fixture = NamedTempFile::new().expect("fixture file");
    writeln!(fixture, "{{\"instruments\":[\"US0000000001\"]}}").expect("write fixture");

    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.args([
        "iso-bridge-lint",
        "--isin",
        "fixtures/iso_bridge/isin_crosswalk.sample.json",
        "--fixtures",
        fixture.path().to_str().unwrap(),
    ]);
    cmd.assert().failure();
}
