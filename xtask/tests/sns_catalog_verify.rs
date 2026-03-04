use std::path::PathBuf;

use assert_cmd::cargo::cargo_bin_cmd;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

#[test]
fn sns_catalog_verify_passes_with_defaults() {
    let mut cmd = cargo_bin_cmd!("xtask");
    cmd.current_dir(workspace_root());
    cmd.arg("sns-catalog-verify");
    cmd.assert().success();
}
