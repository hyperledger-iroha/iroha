//! Shared `kagami` binary resolution helpers for integration tests.

use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

use eyre::{Result, WrapErr, ensure, eyre};

const KAGAMI_BIN_ENV: &str = "KAGAMI_BIN";
const IROHA_TEST_SKIP_BUILD_ENV: &str = "IROHA_TEST_SKIP_BUILD";
const IROHA_TEST_TARGET_DIR_ENV: &str = "IROHA_TEST_TARGET_DIR";

static KAGAMI_BIN: OnceLock<PathBuf> = OnceLock::new();

/// Resolve the `kagami` binary path for localnet integration tests.
///
/// Resolution order:
/// 1. `KAGAMI_BIN`
/// 2. `CARGO_BIN_EXE_kagami`
/// 3. common target roots under `CARGO_TARGET_DIR`, `IROHA_TEST_TARGET_DIR`, and repo `target/`
/// 4. `cargo build -p iroha_kagami --bin kagami` when `IROHA_TEST_SKIP_BUILD` is not enabled
///
/// # Errors
///
/// Returns an error when no suitable binary can be found or built.
pub fn resolve_kagami_bin() -> Result<PathBuf> {
    if let Some(path) = KAGAMI_BIN.get() {
        return Ok(path.clone());
    }

    let resolved = resolve_kagami_bin_uncached()?;
    let _ = KAGAMI_BIN.set(resolved.clone());
    Ok(resolved)
}

fn resolve_kagami_bin_uncached() -> Result<PathBuf> {
    if let Ok(path) = env::var(KAGAMI_BIN_ENV) {
        return canonicalize_repo_relative(PathBuf::from(path))
            .wrap_err_with(|| format!("resolve path from {KAGAMI_BIN_ENV}"));
    }
    if let Ok(path) = env::var("CARGO_BIN_EXE_kagami") {
        return canonicalize_repo_relative(PathBuf::from(path))
            .wrap_err("resolve path from CARGO_BIN_EXE_kagami");
    }

    let repo = repo_root();
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());
    let bin = bin_name("kagami");
    let candidates = kagami_candidates(&repo, &profile, &bin);

    if let Some(path) = try_candidates(&candidates) {
        return Ok(path);
    }

    if skip_build_enabled() {
        return Err(eyre!(
            "kagami binary not found in target roots and {IROHA_TEST_SKIP_BUILD_ENV}=1"
        ));
    }

    build_kagami(&repo, &profile)?;
    try_candidates(&candidates).ok_or_else(|| eyre!("kagami binary not found after build"))
}

fn kagami_candidates(repo: &Path, profile: &str, bin: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut push_root = |root: PathBuf| {
        for candidate in [
            root.join(format!("{profile}/{bin}")),
            root.join(format!("debug/{bin}")),
            root.join(format!("release/{bin}")),
        ] {
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
    };

    if let Ok(path) = env::var("CARGO_TARGET_DIR") {
        push_root(resolve_target_dir(repo, PathBuf::from(path)));
    }
    if let Ok(path) = env::var(IROHA_TEST_TARGET_DIR_ENV) {
        push_root(resolve_target_dir(repo, PathBuf::from(path)));
    }
    push_root(repo.join("target"));

    candidates
}

fn resolve_target_dir(repo: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        repo.join(path)
    }
}

fn canonicalize_repo_relative(path: PathBuf) -> Result<PathBuf> {
    let candidate = if path.is_absolute() {
        path
    } else {
        repo_root().join(path)
    };
    candidate
        .canonicalize()
        .wrap_err_with(|| format!("canonicalize {}", candidate.display()))
}

fn skip_build_enabled() -> bool {
    env::var(IROHA_TEST_SKIP_BUILD_ENV)
        .ok()
        .is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

fn build_kagami(repo: &Path, profile: &str) -> Result<()> {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut command = Command::new(cargo);
    command
        .current_dir(repo)
        .arg("build")
        .arg("-p")
        .arg("iroha_kagami")
        .arg("--bin")
        .arg("kagami");
    if profile != "debug" {
        command.arg("--profile").arg(profile);
    }
    let output = command.output().wrap_err("build kagami binary")?;
    ensure!(
        output.status.success(),
        "failed to build kagami: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("integration_tests manifest dir should have repo root parent")
        .to_path_buf()
}

fn bin_name(raw: &str) -> String {
    if cfg!(windows) {
        format!("{raw}.exe")
    } else {
        raw.to_owned()
    }
}

fn try_candidates(candidates: &[PathBuf]) -> Option<PathBuf> {
    for candidate in candidates {
        if let Ok(path) = candidate.canonicalize() {
            return Some(path);
        }
    }
    None
}
