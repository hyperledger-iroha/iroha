//! Embeds source identity into `iroha_core` binaries.

use std::{env, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=IROHA_GIT_COMMIT_HASH");
    if let Some(commit) = env_commit_hash().or_else(git_commit_hash) {
        println!("cargo:rustc-env=GIT_COMMIT_HASH={commit}");
    } else {
        println!(
            "cargo:warning=iroha_core build.rs: unable to determine git commit hash; \
             the Sumeragi v2 build fingerprint will use the `unknown` source marker"
        );
    }
}

fn env_commit_hash() -> Option<String> {
    let commit = env::var("IROHA_GIT_COMMIT_HASH").ok()?;
    let trimmed = commit.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
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
