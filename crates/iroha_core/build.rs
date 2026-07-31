//! Embeds source identity into `iroha_core` binaries.
//!
//! Ordinary builds retain the lightweight Git-commit marker used by RBC. The
//! opt-in Kagemusha candidate-build feature additionally requires and verifies
//! an independently pinned reviewed source closure supplied by the
//! dedicated build helper.

use std::{env, fs, path::Path, process::Command};

const KAGEMUSHA_EMBEDDED_SOURCE_IDENTITY_FILE: &str = "kagemusha_embedded_source_identity_v1.json";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=IROHA_GIT_COMMIT_HASH");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_SOURCE_COMMIT");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_SOURCE_TREE_SHA256");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_GPG_EXECUTABLE");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_GNUPGHOME");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_GIT_EXECUTABLE");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_GIT_EXEC_PATH");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_BUILD_SOURCE_SIGNING_KEY_FINGERPRINT");
    println!("cargo:rerun-if-env-changed=KAGEMUSHA_SOURCE_SEAL_PYTHON");
    let out_dir =
        env::var_os("OUT_DIR").unwrap_or_else(|| panic!("OUT_DIR is required for iroha_core"));
    fs::write(
        Path::new(&out_dir).join(KAGEMUSHA_EMBEDDED_SOURCE_IDENTITY_FILE),
        [],
    )
    .unwrap_or_else(|error| panic!("failed to initialize embedded source identity: {error}"));
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
    let gpg = required_canonical_absolute_path_env("KAGEMUSHA_BUILD_GPG_EXECUTABLE", false);
    let gnupghome = required_canonical_absolute_path_env("KAGEMUSHA_BUILD_GNUPGHOME", true);
    let git = required_canonical_absolute_path_env("KAGEMUSHA_BUILD_GIT_EXECUTABLE", false);
    let git_exec_path = required_canonical_absolute_path_env("KAGEMUSHA_BUILD_GIT_EXEC_PATH", true);
    let source_signing_key_fingerprint =
        required_upper_hex_fingerprint_env("KAGEMUSHA_BUILD_SOURCE_SIGNING_KEY_FINGERPRINT");
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

    let first_identity = command_bytes(
        Command::new(&python)
            .arg("-I")
            .arg(&seal_script)
            .arg("identity")
            .arg("--root")
            .arg(&repository_root)
            .arg("--reviewed-source-closure")
            .arg(reviewed_closure_path)
            .arg("--reviewed-source-closure-sha256")
            .arg(&reviewed_closure_sha256),
        "Kagemusha reviewed source-closure identity",
    );
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
        trusted_git_command(&git, &git_exec_path)
            .arg("-C")
            .arg(&repository_root)
            .args(["rev-parse", "--verify", "HEAD^{commit}"]),
        "Kagemusha source commit",
    );
    verify_exact_signed_commit(
        &repository_root,
        &expected_commit,
        &git,
        &git_exec_path,
        &gpg,
        &gnupghome,
        &source_signing_key_fingerprint,
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
    let second_identity = command_bytes(
        Command::new(&python)
            .arg("-I")
            .arg(&seal_script)
            .arg("identity")
            .arg("--root")
            .arg(&repository_root)
            .arg("--reviewed-source-closure")
            .arg(reviewed_closure_path)
            .arg("--reviewed-source-closure-sha256")
            .arg(&reviewed_closure_sha256),
        "Kagemusha reviewed source-closure identity recheck",
    );
    if first_identity != second_identity
        || first_identity.is_empty()
        || first_identity.len() > 16 * 1024 * 1024
        || !first_identity.ends_with(b"\n")
        || first_identity.contains(&0)
    {
        panic!("sealed Kagemusha source identity changed or is not canonical");
    }
    let out_dir =
        env::var_os("OUT_DIR").unwrap_or_else(|| panic!("OUT_DIR is required for iroha_core"));
    fs::write(
        Path::new(&out_dir).join(KAGEMUSHA_EMBEDDED_SOURCE_IDENTITY_FILE),
        &first_identity,
    )
    .unwrap_or_else(|error| panic!("failed to embed reviewed source identity: {error}"));
    println!("cargo:rustc-env=KAGEMUSHA_BUILD_SOURCE_COMMIT={expected_commit}");
    println!("cargo:rustc-env=KAGEMUSHA_BUILD_SOURCE_TREE_SHA256={expected_tree}");
    println!("cargo:rerun-if-changed={reviewed_closure}");
}

fn trusted_git_command(git: &str, git_exec_path: &str) -> Command {
    let mut command = Command::new(git);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_EXEC_PATH", git_exec_path)
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LANG", "C")
        .env("LC_ALL", "C");
    command
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

fn required_upper_hex_fingerprint_env(name: &str) -> String {
    let value = env::var(name)
        .unwrap_or_else(|_| panic!("{name} is required for a sealed Kagemusha candidate build"));
    if !matches!(value.len(), 40 | 64)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'A'..=b'F').contains(&byte))
    {
        panic!("{name} is not a canonical upper-case OpenPGP fingerprint");
    }
    value
}

fn required_canonical_absolute_path_env(name: &str, directory: bool) -> String {
    let value = env::var(name)
        .unwrap_or_else(|_| panic!("{name} is required for a sealed Kagemusha candidate build"));
    let path = Path::new(&value);
    let metadata = path
        .symlink_metadata()
        .unwrap_or_else(|error| panic!("{name} metadata is unavailable: {error}"));
    let canonical = path
        .canonicalize()
        .unwrap_or_else(|error| panic!("{name} is unavailable: {error}"));
    if !path.is_absolute()
        || canonical != path
        || metadata.file_type().is_symlink()
        || (directory && !metadata.is_dir())
        || (!directory && !metadata.is_file())
    {
        panic!("{name} must be one canonical absolute nonsymlink path");
    }
    value
}

fn verify_exact_signed_commit(
    repository_root: &Path,
    expected_commit: &str,
    git: &str,
    git_exec_path: &str,
    gpg: &str,
    gnupghome: &str,
    expected_fingerprint: &str,
) {
    let output = trusted_git_command(git, git_exec_path)
        .env("GNUPGHOME", gnupghome)
        .arg("-C")
        .arg(repository_root)
        .arg("-c")
        .arg("core.fileMode=true")
        .arg("-c")
        .arg(format!("gpg.program={gpg}"))
        .arg("-c")
        .arg("gpg.format=openpgp")
        .args(["verify-commit", "--raw", expected_commit])
        .output()
        .unwrap_or_else(|error| panic!("failed to run admitted GPG verifier: {error}"));
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        panic!("Kagemusha signed source commit verification failed: {detail}");
    }
    let prefix = b"[GNUPG:] VALIDSIG ";
    let fingerprints = output
        .stderr
        .split(|byte| *byte == b'\n')
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|status| {
            status
                .split(|byte| byte.is_ascii_whitespace())
                .next()
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    if fingerprints.as_slice() != [expected_fingerprint.as_bytes()] {
        panic!("Kagemusha signed source commit signer differs from the pinned fingerprint");
    }
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

fn command_bytes(command: &mut Command, description: &str) -> Vec<u8> {
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("failed to run {description}: {error}"));
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr);
        panic!("{description} failed: {detail}");
    }
    output.stdout
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
