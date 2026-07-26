//! Embeds source identity into `iroha_core` binaries.
//!
//! Ordinary builds retain the lightweight Git-commit marker used by RBC. The
//! opt-in Kagemusha candidate-build feature additionally requires and verifies
//! an independently pinned reviewed dirty source closure supplied by the
//! dedicated build helper.

use std::{env, path::Path, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=IROHA_GIT_COMMIT_HASH");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_SOURCE_COMMIT");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_SOURCE_TREE_SHA256");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_SOURCE_SEAL_PYTHON");
    if let Some(commit) = env_commit_hash().or_else(git_commit_hash) {
        println!("cargo:rustc-env=GIT_COMMIT_HASH={commit}");
    } else {
        println!(
            "cargo:warning=iroha_core build.rs: unable to determine git commit hash; \
             persisted RBC sessions will be discarded across restarts"
        );
    }
    if env::var_os("CARGO_FEATURE_KAGEMUSHA_CANDIDATE_SOURCE_SEAL").is_some() {
        embed_exact_kagemusha_source_seal();
    }
}

fn embed_exact_kagemusha_source_seal() {
    let expected_commit = required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_COMMIT", 40);
    let expected_tree = required_lower_hex_env("KAGEMUSHA_BUILD_SOURCE_TREE_SHA256", 64);
    let reviewed_closure_sha256 =
        required_lower_hex_env("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256", 64);
    let reviewed_closure =
        env::var("KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE").unwrap_or_else(|_| {
            panic!(
                "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE is required for a sealed candidate build"
            )
        });
    let reviewed_closure_path = Path::new(&reviewed_closure);
    if !reviewed_closure_path.is_absolute()
        || reviewed_closure_path.canonicalize().ok().as_deref() != Some(reviewed_closure_path)
    {
        panic!(
            "KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE must be one canonical absolute nonsymlink path"
        );
    }
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .unwrap_or_else(|_| panic!("CARGO_MANIFEST_DIR is required for a sealed candidate build"));
    let repository_root = Path::new(&manifest_dir).join("../..");
    let seal_script = repository_root.join("scripts/kagemusha_source_tree_seal.py");
    let python = env::var("KAGEMUSHA_SOURCE_SEAL_PYTHON").unwrap_or_else(|_| "python3".to_owned());

    let first_tree = command_text(
        Command::new(&python)
            .arg("-I")
            .arg(&seal_script)
            .arg("fingerprint")
            .arg("--root")
            .arg(&repository_root)
            .arg("--reviewed-source-closure")
            .arg(reviewed_closure_path)
            .arg("--reviewed-source-closure-sha256")
            .arg(&reviewed_closure_sha256),
        "Kagemusha reviewed source-closure seal",
    );
    let actual_commit = command_text(
        Command::new("git").arg("-C").arg(&repository_root).args([
            "rev-parse",
            "--verify",
            "HEAD^{commit}",
        ]),
        "Kagemusha source commit",
    );
    let second_tree = command_text(
        Command::new(&python)
            .arg("-I")
            .arg(&seal_script)
            .arg("fingerprint")
            .arg("--root")
            .arg(&repository_root)
            .arg("--reviewed-source-closure")
            .arg(reviewed_closure_path)
            .arg("--reviewed-source-closure-sha256")
            .arg(&reviewed_closure_sha256),
        "Kagemusha reviewed source-closure seal recheck",
    );
    if actual_commit != expected_commit
        || first_tree != expected_tree
        || second_tree != expected_tree
    {
        panic!(
            "sealed Kagemusha candidate build source changed or differs from the requested identity"
        );
    }
    println!("cargo:rustc-env=KAGEMUSHA_BUILD_SOURCE_COMMIT={expected_commit}");
    println!("cargo:rustc-env=KAGEMUSHA_BUILD_SOURCE_TREE_SHA256={expected_tree}");
    println!("cargo:rustc-env=KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE={reviewed_closure}");
    println!(
        "cargo:rustc-env=KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256={reviewed_closure_sha256}"
    );
    println!("cargo:rerun-if-changed={reviewed_closure}");
}

fn required_lower_hex_env(name: &str, expected_len: usize) -> String {
    let value = env::var(name)
        .unwrap_or_else(|_| panic!("{name} is required for a sealed Kagemusha candidate build"));
    if value.len() != expected_len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        panic!("{name} is not canonical lower-case hexadecimal");
    }
    value
}

fn command_text(command: &mut Command, description: &str) -> String {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("failed to run {description}: {error}"));
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        panic!("{description} failed: {detail}");
    }
    let value = String::from_utf8(output.stdout)
        .unwrap_or_else(|_| panic!("{description} output is not UTF-8"));
    let trimmed = value.trim_end_matches(['\r', '\n']);
    assert!(
        !(trimmed.is_empty() || trimmed.contains(char::is_whitespace)),
        "{description} output is not one canonical value"
    );
    trimmed.to_owned()
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
