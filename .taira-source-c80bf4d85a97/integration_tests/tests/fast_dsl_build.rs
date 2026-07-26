#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Ensures the workspace builds with the `fast_dsl` feature enabled.

use std::{fs, path::Path, process::Command};

use integration_tests::process::{build_timeout, status_with_timeout};

#[test]
fn workspace_builds_with_fast_dsl_feature() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root");
    let target_dir = workspace_root.join("target").join("fast-dsl-check");
    fs::create_dir_all(&target_dir).expect("create isolated fast_dsl target dir");
    let mut command = Command::new("cargo");
    command
        .args(["check", "--workspace", "--features", "fast_dsl"])
        .current_dir(workspace_root)
        // Nested cargo invocations must not share the outer test runner's target dir.
        .env("CARGO_TARGET_DIR", &target_dir);
    let status = status_with_timeout(&mut command, build_timeout())
        .expect("failed to run cargo check with fast_dsl");
    assert!(status.success());
}
