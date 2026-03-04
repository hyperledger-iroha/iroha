//! Embeds the current git commit hash into `iroha_core` so RBC persistence can
//! verify snapshots belong to the running binary.

use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if let Some(commit) = git_commit_hash() {
        println!("cargo:rustc-env=GIT_COMMIT_HASH={commit}");
    } else {
        println!(
            "cargo:warning=iroha_core build.rs: unable to determine git commit hash; \
             persisted RBC sessions will be discarded across restarts"
        );
    }
}

fn git_commit_hash() -> Option<String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").ok()?;
    let output = Command::new("git")
        .args(["-C", &manifest_dir, "rev-parse", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let hash = String::from_utf8(output.stdout).ok()?;
    let trimmed = hash.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}
