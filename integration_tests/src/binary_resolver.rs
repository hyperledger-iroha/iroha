//! Shared binary resolution helpers for CLI-oriented integration tests.

use std::{
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    sync::Once,
    time::SystemTime,
};

/// Resolve the `iroha` CLI binary, preferring already-built targets when available.
pub fn iroha_program() -> eyre::Result<PathBuf> {
    prepare_iroha_cli_test_environment();
    iroha_test_network::Program::Iroha
        .resolve()
        .map_err(Into::into)
}

/// Prepare CLI integration tests to reuse already-built binaries when requested.
pub fn prepare_iroha_cli_test_environment() {
    enable_reentrant_builds_for_tests();
    configure_program_overrides_from_existing_binaries();
}

/// Return the build-profile override used by CLI integration tests.
pub fn iroha_cli_test_build_profile_override(current: Option<&str>) -> Option<&'static str> {
    current
        .is_none_or(|value| value.trim().is_empty())
        .then_some("debug")
}

/// Decide whether CLI integration tests should reuse already-built binaries.
pub fn should_reuse_existing_cli_binary_for_tests() -> bool {
    should_reuse_existing_cli_binary_for_tests_from_value(
        std::env::var("IROHA_TEST_SKIP_BUILD").ok().as_deref(),
    )
}

/// Parse the `IROHA_TEST_SKIP_BUILD` knob into a boolean.
pub fn should_reuse_existing_cli_binary_for_tests_from_value(value: Option<&str>) -> bool {
    value.is_some_and(|value| value == "1" || value.eq_ignore_ascii_case("true"))
}

/// Find an existing `iroha` binary under the supplied target roots.
pub fn find_existing_cli_binary_path_from_roots(
    target_roots: &[PathBuf],
    profiles: &[String],
) -> Option<PathBuf> {
    find_existing_binary_path_from_roots(target_roots, profiles, cli_binary_name())
}

/// Find an existing `irohad` binary under the current target roots.
pub fn find_existing_irohad_binary_path() -> Option<PathBuf> {
    let target_roots = default_target_roots();
    let profiles = default_profiles();
    find_existing_binary_path_from_roots(&target_roots, &profiles, irohad_binary_name())
}

/// Prefer the daemon binary alongside the currently running test binary.
pub fn find_primary_target_irohad_binary_path() -> Option<PathBuf> {
    let mut target_roots = Vec::new();
    if let Some(target_root) = current_test_binary_target_root() {
        target_roots.push(target_root);
    }
    if let Some(target_root) = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from)
        && !target_roots.contains(&target_root)
    {
        target_roots.push(target_root);
    }
    let workspace_target = workspace_root().join("target");
    if !target_roots.contains(&workspace_target) {
        target_roots.push(workspace_target);
    }

    find_existing_binary_path_from_roots(&target_roots, &default_profiles(), irohad_binary_name())
}

/// Resolve the sibling `irohad` binary next to a known CLI path.
pub fn matching_irohad_binary_path_from_cli_path(path: &Path) -> Option<PathBuf> {
    let candidate = path.parent()?.join(irohad_binary_name());
    candidate.is_file().then_some(candidate)
}

/// Pick the newest on-disk binary from the candidate set.
pub fn newest_existing_binary_path(
    paths: impl IntoIterator<Item = Option<PathBuf>>,
) -> Option<PathBuf> {
    let mut first_match = None;
    let mut newest_match: Option<(SystemTime, PathBuf)> = None;

    for candidate in paths.into_iter().flatten() {
        if !candidate.is_file() {
            continue;
        }
        if first_match.is_none() {
            first_match = Some(candidate.clone());
        }
        if let Some(modified_at) = binary_modified_at(&candidate) {
            let replace = newest_match
                .as_ref()
                .is_none_or(|(current_modified_at, _)| modified_at > *current_modified_at);
            if replace {
                newest_match = Some((modified_at, candidate));
            }
        }
    }

    newest_match.map(|(_, path)| path).or(first_match)
}

/// Scan target roots and profiles for an existing binary with the given name.
pub fn find_existing_binary_path_from_roots(
    target_roots: &[PathBuf],
    profiles: &[String],
    binary_name: &str,
) -> Option<PathBuf> {
    let candidates = target_roots.iter().flat_map(|root| {
        profiles
            .iter()
            .map(move |profile| root.join(profile).join(binary_name))
    });
    newest_existing_binary_path(candidates.map(Some))
}

/// Return the CLI binary name for the current platform.
pub const fn cli_binary_name() -> &'static str {
    if cfg!(windows) { "iroha.exe" } else { "iroha" }
}

/// Return the daemon binary name for the current platform.
pub const fn irohad_binary_name() -> &'static str {
    if cfg!(windows) {
        "iroha3d.exe"
    } else {
        "iroha3d"
    }
}

/// Check whether an existing CLI binary exposes the training-job command surface.
pub fn binary_supports_training_job_commands(path: &Path) -> bool {
    let output = ProcessCommand::new(path)
        .arg("app")
        .arg("soracloud")
        .arg("--help")
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("training-job-start") && stdout.contains("hf-deploy")
}

/// Return the workspace root derived from the integration-tests manifest path.
pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn enable_reentrant_builds_for_tests() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        set_env_var("IROHA_TEST_ALLOW_REENTRANT_BUILD", "1");
        if let Some(profile) = iroha_cli_test_build_profile_override(
            std::env::var("IROHA_TEST_BUILD_PROFILE").ok().as_deref(),
        ) {
            set_env_var("IROHA_TEST_BUILD_PROFILE", profile);
        }
    });
}

fn configure_program_overrides_from_existing_binaries() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        const TEST_NETWORK_BIN_IROHA: &str = "TEST_NETWORK_BIN_IROHA";
        const TEST_NETWORK_BIN_IROHAD: &str = "TEST_NETWORK_BIN_IROHAD";
        if !should_reuse_existing_cli_binary_for_tests() {
            return;
        }

        let cli_path =
            if let Some(path) = std::env::var_os(TEST_NETWORK_BIN_IROHA).map(PathBuf::from) {
                Some(path)
            } else {
                find_existing_cli_binary_path()
            };

        if std::env::var_os(TEST_NETWORK_BIN_IROHA).is_none()
            && let Some(path) = cli_path.as_ref()
        {
            let value = path.to_string_lossy().into_owned();
            set_env_var(TEST_NETWORK_BIN_IROHA, &value);
        }

        if std::env::var_os(TEST_NETWORK_BIN_IROHAD).is_none()
            && let Some(path) = find_primary_target_irohad_binary_path()
                .or_else(find_existing_irohad_binary_path)
                .or_else(|| {
                    cli_path
                        .as_deref()
                        .and_then(matching_irohad_binary_path_from_cli_path)
                })
        {
            let value = path.to_string_lossy().into_owned();
            set_env_var(TEST_NETWORK_BIN_IROHAD, &value);
        }
    });
}

fn find_existing_cli_binary_path() -> Option<PathBuf> {
    let target_roots = default_target_roots();
    let profiles = default_profiles();
    find_existing_cli_binary_path_from_roots(&target_roots, &profiles)
        .filter(|path| binary_supports_training_job_commands(path.as_path()))
}

fn default_target_roots() -> Vec<PathBuf> {
    let mut target_roots = Vec::new();
    if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
        let target_dir = PathBuf::from(target_dir);
        target_roots.push(target_dir.join("iroha-test-network"));
        target_roots.push(target_dir);
    }
    let workspace_target = workspace_root().join("target");
    target_roots.push(workspace_target.join("iroha-test-network"));
    target_roots.push(workspace_target);
    target_roots
}

fn default_profiles() -> Vec<String> {
    let mut profiles = Vec::new();
    if let Ok(profile) = std::env::var("PROFILE")
        && !profile.trim().is_empty()
    {
        profiles.push(profile);
    }
    if !profiles.iter().any(|value| value == "debug") {
        profiles.push("debug".to_owned());
    }
    if !profiles.iter().any(|value| value == "release") {
        profiles.push("release".to_owned());
    }
    profiles
}

fn current_test_binary_target_root() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut directory = exe.parent()?;
    if directory.file_name().is_some_and(|value| value == "deps") {
        directory = directory.parent()?;
    }
    directory.parent().map(Path::to_path_buf)
}

fn binary_modified_at(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

#[allow(unsafe_code)]
fn set_env_var(key: &str, value: &str) {
    unsafe {
        std::env::set_var(key, value);
    }
}
