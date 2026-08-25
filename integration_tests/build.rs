//! Build script that stages deterministic IVM sample bytecode for integration tests.
//!
//! The fixtures are versioned under `integration_tests/fixtures/ivm` and copied
//! into `crates/ivm/target/prebuilt/samples` so existing test helpers keep
//! working without compiling Kotodama/IVM logic in the build script. Sealed
//! read-only source mirrors fall back to Cargo's writable `OUT_DIR` because
//! build-only consumers do not need the legacy runtime fixture location.
use std::{
    env, fs,
    path::{Path, PathBuf},
};
const SAMPLE_MANIFEST: &str = include_str!("../crates/ivm/prebuilt_samples.txt");
fn prebuilt_sample_names() -> Vec<&'static str> {
    SAMPLE_MANIFEST
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}
fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .expect("integration_tests must be in workspace root")
}
fn sample_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(name).with_extension("to")
}
fn write_file_if_changed(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Ok(existing) = fs::read(path)
        && existing == bytes
    {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}
fn stage_prebuilt_samples(
    fixtures_dir: &Path,
    prebuilt_dir: &Path,
    profile: &str,
    sample_names: &[&str],
) -> std::io::Result<()> {
    let samples_dir = prebuilt_dir.join("samples");
    let config = format!("profile = \"{profile}\"\n");
    write_file_if_changed(&prebuilt_dir.join("build_config.toml"), config.as_bytes())?;
    fs::create_dir_all(&samples_dir)?;
    for name in sample_names {
        let source = sample_path(fixtures_dir, name);
        let destination = sample_path(&samples_dir, name);
        let bytes = fs::read(&source)?;
        write_file_if_changed(&destination, &bytes)?;
    }
    Ok(())
}
fn stage_prebuilt_samples_with_fallback(
    fixtures_dir: &Path,
    preferred_dir: &Path,
    fallback_dir: &Path,
    profile: &str,
    sample_names: &[&str],
) -> std::io::Result<bool> {
    match stage_prebuilt_samples(fixtures_dir, preferred_dir, profile, sample_names) {
        Ok(()) => Ok(false),
        Err(preferred_error) => {
            stage_prebuilt_samples(fixtures_dir, fallback_dir, profile, sample_names)
                .map(|()| true)
                .map_err(|fallback_error| {
                    std::io::Error::other(format!(
                        "preferred prebuilt staging failed: {preferred_error}; OUT_DIR fallback failed: {fallback_error}"
                    ))
                })
        }
    }
}
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../crates/ivm/prebuilt_samples.txt");
    let root = workspace_root();
    let fixtures_dir = root.join("integration_tests/fixtures/ivm");
    let prebuilt_dir = root.join("crates/ivm/target/prebuilt");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR must be set by Cargo"));
    let fallback_prebuilt_dir = out_dir.join("ivm-prebuilt");
    if let Ok(entries) = fs::read_dir(&fixtures_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "to") {
                println!("cargo:rerun-if-changed={}", path.display());
            }
        }
    }
    let profile = if env::var("PROFILE").ok().as_deref() == Some("release") {
        "Release"
    } else {
        "Debug"
    };
    let mut sample_names = prebuilt_sample_names();
    if env::var("IROHA_TEST_PREBUILD_DEFAULT_EXECUTOR")
        .ok()
        .as_deref()
        == Some("1")
    {
        sample_names.push("default_executor");
    }
    let used_fallback = stage_prebuilt_samples_with_fallback(
        &fixtures_dir,
        &prebuilt_dir,
        &fallback_prebuilt_dir,
        profile,
        &sample_names,
    )
    .expect("failed to stage prebuilt IVM samples");
    if used_fallback {
        println!(
            "cargo:warning=integration_tests: source fixture staging unavailable; used Cargo OUT_DIR"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn unavailable_source_staging_uses_out_dir_fallback() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock must follow the Unix epoch")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "iroha-integration-build-script-{}-{nonce}",
            std::process::id()
        ));
        let fixtures_dir = root.join("fixtures");
        fs::create_dir_all(&fixtures_dir).expect("create fixture directory");
        fs::write(sample_path(&fixtures_dir, "sample"), b"sample bytecode")
            .expect("write sample fixture");

        let source_blocker = root.join("sealed-source");
        fs::write(&source_blocker, b"not a directory").expect("write source blocker");
        let preferred_dir = source_blocker.join("prebuilt");
        let fallback_dir = root.join("out/prebuilt");
        let used_fallback = stage_prebuilt_samples_with_fallback(
            &fixtures_dir,
            &preferred_dir,
            &fallback_dir,
            "Debug",
            &["sample"],
        )
        .expect("fallback staging must succeed");

        assert!(used_fallback);
        assert_eq!(
            fs::read_to_string(fallback_dir.join("build_config.toml"))
                .expect("read fallback build config"),
            "profile = \"Debug\"\n"
        );
        assert_eq!(
            fs::read(sample_path(&fallback_dir.join("samples"), "sample"))
                .expect("read fallback sample"),
            b"sample bytecode"
        );
        fs::remove_dir_all(root).expect("remove build-script test directory");
    }
}
