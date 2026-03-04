#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Ensures the workspace builds with the `fast_dsl` feature enabled.

use std::{path::Path, process::Command};

#[test]
fn workspace_builds_with_fast_dsl_feature() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let status = Command::new("cargo")
        .args(["check", "--workspace", "--features", "fast_dsl"])
        .current_dir(workspace_root)
        .status()
        .expect("failed to run cargo check with fast_dsl");
    assert!(status.success());
}
