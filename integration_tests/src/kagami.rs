//! Shared `kagami` binary resolution helpers for integration tests.
use crate::process::{build_timeout, output_with_timeout};
use eyre::{Result, WrapErr, ensure, eyre};
use iroha_fs::ProcessCommand as Command;
use iroha_test_network::{
    BinaryKind, ResolvedProgram, development_process_command, resolve_development_binary,
    resolve_release_prebuilt_binary, revalidate_release_prebuilt_binary,
};
use std::{
    env,
    path::{Path, PathBuf},
    sync::OnceLock,
};
const KAGAMI_BIN_ENV: &str = "KAGAMI_BIN";
const IROHA_TEST_SKIP_BUILD_ENV: &str = "IROHA_TEST_SKIP_BUILD";
const IROHA_TEST_TARGET_DIR_ENV: &str = "IROHA_TEST_TARGET_DIR";
const IROHA_TEST_TARGET_SUBDIR: &str = "iroha-test-network";
static KAGAMI_BIN: OnceLock<ResolvedProgram> = OnceLock::new();
/// Resolve a retained `kagami` program for localnet integration tests.
///
/// Release admission precedes caches and developer overrides. Without release anchors,
/// developer resolution uses:
/// 1. `KAGAMI_BIN`
/// 2. `CARGO_BIN_EXE_kagami`
/// 3. a lockfile-constrained build in the isolated test target when
///    `IROHA_TEST_SKIP_BUILD` is not enabled
/// 4. common target roots under `CARGO_TARGET_DIR`, `IROHA_TEST_TARGET_DIR`, and repo `target/`
///
/// # Errors
///
/// Returns an error when no suitable binary can be found or built.
pub fn resolve_kagami_bin() -> Result<ResolvedProgram> {
    // Admit the complete identity before consulting a cache, developer overrides or Cargo.
    if let Some(program) = resolve_release_prebuilt_binary(BinaryKind::Kagami)? {
        if let Some(candidate) = env::var_os(KAGAMI_BIN_ENV) {
            ensure!(
                candidate == program.diagnostic_path()?.as_os_str(),
                "KAGAMI_BIN must preserve the exact manifest-attested native spelling"
            );
        }
        if let Some(cached) = KAGAMI_BIN.get().filter(|cached| cached.is_release()) {
            return revalidate_release_prebuilt_binary(BinaryKind::Kagami, cached)?
                .ok_or_else(|| eyre!("release identity disappeared while rechecking kagami"));
        }
        let _ = KAGAMI_BIN.set(program.clone());
        return Ok(program);
    }
    if let Some(cached) = KAGAMI_BIN.get() {
        // This also refuses a previously qualified selection after its anchors disappear.
        revalidate_release_prebuilt_binary(BinaryKind::Kagami, cached)?;
        if cached.diagnostic_path()?.is_file() {
            return Ok(cached.clone());
        }
    }
    let path = resolve_development_kagami_bin()?;
    let program = resolve_development_binary(&path)?;
    let _ = KAGAMI_BIN.set(program.clone());
    Ok(program)
}
fn resolve_development_kagami_bin() -> Result<PathBuf> {
    if let Some(path) = env::var_os(KAGAMI_BIN_ENV) {
        return canonicalize_repo_relative(PathBuf::from(path))
            .wrap_err_with(|| format!("resolve path from {KAGAMI_BIN_ENV}"));
    }
    if let Some(path) = env::var_os("CARGO_BIN_EXE_kagami") {
        return canonicalize_repo_relative(PathBuf::from(path))
            .wrap_err("resolve path from CARGO_BIN_EXE_kagami");
    }
    let repo = repo_root();
    let profile = env::var("PROFILE").unwrap_or_else(|_| "debug".to_owned());
    let bin = bin_name("kagami");
    let candidates = kagami_candidates(&repo, &profile, &bin);
    if skip_build_enabled() {
        return try_candidates(&candidates).ok_or_else(|| {
            eyre!("kagami binary not found in target roots and {IROHA_TEST_SKIP_BUILD_ENV}=1")
        });
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
    // Prefer the exact root that `build_kagami` validates. In particular, the
    // release corridor sets both target variables to keep the outer test build
    // and the source-bound prebuilt program bundle on separate paths.
    push_root(kagami_build_target_dir(repo));
    if let Ok(path) = env::var("CARGO_TARGET_DIR") {
        push_root(resolve_target_dir(repo, PathBuf::from(path)).join(IROHA_TEST_TARGET_SUBDIR));
    }
    if let Ok(path) = env::var(IROHA_TEST_TARGET_DIR_ENV) {
        push_root(resolve_target_dir(repo, PathBuf::from(path)));
    }
    push_root(default_test_target_dir(repo));
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
fn default_test_target_dir(repo: &Path) -> PathBuf {
    repo.join("target").join(IROHA_TEST_TARGET_SUBDIR)
}
fn kagami_build_target_dir(repo: &Path) -> PathBuf {
    if let Ok(path) = env::var(IROHA_TEST_TARGET_DIR_ENV) {
        return resolve_target_dir(repo, PathBuf::from(path));
    }
    if let Ok(path) = env::var("CARGO_TARGET_DIR") {
        return resolve_target_dir(repo, PathBuf::from(path)).join(IROHA_TEST_TARGET_SUBDIR);
    }
    default_test_target_dir(repo)
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
    let target_dir = kagami_build_target_dir(repo);
    let command = kagami_build_command(
        development_process_command(&cargo)?,
        repo,
        &target_dir,
        profile,
    );
    let output = output_with_timeout(command, build_timeout()).wrap_err("build kagami binary")?;
    ensure!(
        output.status.success(),
        "failed to build kagami: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
fn kagami_build_command(
    mut command: Command,
    repo: &Path,
    target_dir: &Path,
    profile: &str,
) -> Command {
    command
        .current_dir(repo)
        .arg("build")
        .arg("--locked")
        .arg("-p")
        .arg("iroha_kagami")
        .arg("--bin")
        .arg("kagami")
        .env("CARGO_TARGET_DIR", target_dir);
    if profile != "debug" {
        command.arg("--profile").arg(profile);
    }
    command
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kagami_build_command_is_locked_and_uses_isolated_target() {
        let repo = Path::new("/workspace/iroha");
        let target = Path::new("/workspace/target/programs");
        let command = kagami_build_command(
            Command::new(env::current_exe().unwrap()).unwrap(),
            repo,
            target,
            "release",
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            args,
            [
                "build",
                "--locked",
                "-p",
                "iroha_kagami",
                "--bin",
                "kagami",
                "--profile",
                "release"
            ]
        );
        assert_eq!(command.get_current_dir(), Some(repo));
        assert_eq!(
            command
                .get_envs()
                .find(|(name, _)| *name == "CARGO_TARGET_DIR")
                .and_then(|(_, value)| value),
            Some(target.as_os_str())
        );
    }
    #[test]
    fn kagami_debug_build_command_uses_default_profile() {
        let command = kagami_build_command(
            Command::new(env::current_exe().unwrap()).unwrap(),
            Path::new("/workspace/iroha"),
            Path::new("/workspace/target/programs"),
            "debug",
        );
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(!args.iter().any(|arg| arg == "--profile"));
    }
}
